//! `gcam-mvsdk-rs view`: one camera live in an OpenCV window with an FPS overlay, and PNG
//! recording on request. Built only with `--features view`, because the `opencv` crate needs
//! OpenCV and libclang at build time; every other part of the crate builds without them.
//!
//! Keys: r starts a recording, c stops it early, q or Esc quits; closing the window also quits.
//! A recording saves `rate_hz` images per second for `duration_s` seconds into
//! `<data_dir>/<YYYYMMDD_HHMMSS>_<camera>/0.png, 1.png, ...`. The three values come from
//! `--config FILE` (YAML, see `examples/record.yaml`) and default to 5 Hz, 30 s and the SDK data
//! directory. The fastest frame-speed tier the camera lists is selected unless `--frame-speed`
//! names one; nothing is persisted in the camera.
//! Tests (no camera, display or OpenCV window): `cargo test --features view`.

use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use clap::Args;
use gcam_mvsdk_rs::{Error, OutputFormat, Result, Sdk, TriggerMode};
use opencv::core::{Mat, Point, Scalar, Vector};
use opencv::{highgui, imgcodecs, imgproc};
use serde::Deserialize;

const WINDOW: &str = "gcam-mvsdk-rs view";

#[derive(Args)]
pub struct ViewArgs {
    /// User-set name or serial number; may be omitted when exactly one camera is present
    #[arg(value_name = "NAME_OR_SN")]
    pub camera: Option<String>,
    /// Exposure time in microseconds (automatic exposure is turned off)
    #[arg(long, default_value_t = 5000.0, value_name = "US")]
    pub exposure_us: f64,
    /// Frame-speed tier index from `info` [default: the fastest the camera lists]
    #[arg(long, value_name = "IDX")]
    pub frame_speed: Option<i32>,
    /// Largest GigE packet size to select, normally the NIC MTU
    /// [default: $GIGE_MTU if set, otherwise keep the camera setting]
    #[arg(long, value_name = "BYTES")]
    pub mtu: Option<u32>,
    /// Recording options, a YAML file with duration_s, rate_hz and data_dir (see examples/record.yaml)
    /// [default: 5 images/s for 30 s into the SDK data directory]
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,
}

/// Recording options. Missing keys take the defaults below.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordConfig {
    /// A recording stops after this many seconds.
    #[serde(default = "RecordConfig::default_duration_s")]
    duration_s: f64,
    /// Images saved per second; frames in between are shown but not saved.
    #[serde(default = "RecordConfig::default_rate_hz")]
    rate_hz: f64,
    /// Root for the session folders; `None` means the SDK data directory.
    #[serde(default)]
    data_dir: Option<PathBuf>,
}

impl Default for RecordConfig {
    fn default() -> Self {
        RecordConfig { duration_s: Self::default_duration_s(), rate_hz: Self::default_rate_hz(), data_dir: None }
    }
}

impl RecordConfig {
    fn default_duration_s() -> f64 {
        30.0
    }

    fn default_rate_hz() -> f64 {
        5.0
    }

    /// Parses YAML text; unknown keys and non-positive numbers are errors.
    fn parse(text: &str) -> std::result::Result<Self, String> {
        let cfg: RecordConfig = if text.trim().is_empty() {
            RecordConfig::default()
        } else {
            serde_yaml_ng::from_str(text).map_err(|e| e.to_string())?
        };
        if !(cfg.duration_s > 0.0 && cfg.duration_s.is_finite()) {
            return Err(format!("duration_s must be a positive number, got {}", cfg.duration_s));
        }
        if !(cfg.rate_hz > 0.0 && cfg.rate_hz.is_finite()) {
            return Err(format!("rate_hz must be a positive number, got {}", cfg.rate_hz));
        }
        Ok(cfg)
    }

    /// Reads and parses `path`.
    fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path).map_err(|e| Error::io(format!("reading {}", path.display()), e))?;
        Self::parse(&text).map_err(|e| Error::InvalidArgument(format!("{}: {e}", path.display())))
    }

    /// Root for the session folders: `data_dir` with a leading `~` expanded, else `fallback`.
    fn data_dir(&self, fallback: Option<&Path>) -> Result<PathBuf> {
        match (&self.data_dir, fallback) {
            (Some(dir), _) => Ok(expand_tilde(dir)),
            (None, Some(dir)) => Ok(dir.to_path_buf()),
            (None, None) => Err(Error::InvalidArgument(
                "cannot determine the SDK data directory; set data_dir in the config file or pass --data-dir".into(),
            )),
        }
    }

    fn interval(&self) -> Duration {
        Duration::from_secs_f64(1.0 / self.rate_hz)
    }

    fn duration(&self) -> Duration {
        Duration::from_secs_f64(self.duration_s)
    }
}

fn expand_tilde(path: &Path) -> PathBuf {
    let home = || std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    match path.to_str() {
        Some("~") => home().unwrap_or_else(|| path.to_path_buf()),
        Some(text) if text.starts_with("~/") => home().map(|h| h.join(&text[2..])).unwrap_or_else(|| path.to_path_buf()),
        _ => path.to_path_buf(),
    }
}

/// What a key press asks for, given whether a recording is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Ignore,
    Start,
    Stop,
    Quit,
}

/// The key hint shown at the bottom of the window; only the keys that do something now.
fn key_hints(recording: bool) -> &'static str {
    if recording {
        "c: stop recording   q/Esc: quit"
    } else {
        "r: record   q/Esc: quit"
    }
}

/// `r`/`R` start a recording, `c`/`C` stop one, `q` or Esc quit. Everything else, and a start
/// while recording or a stop while idle, is ignored.
fn action_for_key(key: i32, recording: bool) -> Action {
    match u8::try_from(key) {
        Ok(b'r' | b'R') if !recording => Action::Start,
        Ok(b'c' | b'C') if recording => Action::Stop,
        Ok(b'q' | 27) => Action::Quit,
        _ => Action::Ignore,
    }
}

/// `<YYYYMMDD_HHMMSS>_<camera>` with the camera name reduced to file-safe characters.
fn session_dir_name(local_time: &str, camera: &str) -> String {
    let safe: String = camera
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '_' })
        .collect();
    format!("{local_time}_{}", if safe.is_empty() { "camera" } else { &safe })
}

/// One recording: decides which frames to save and when to stop.
struct Session {
    dir: PathBuf,
    started: Instant,
    next_due: Instant,
    interval: Duration,
    duration: Duration,
    saved: u32,
}

impl Session {
    fn new(dir: PathBuf, now: Instant, cfg: &RecordConfig) -> Self {
        Session { dir, started: now, next_due: now, interval: cfg.interval(), duration: cfg.duration(), saved: 0 }
    }

    /// Path for a frame that arrived at `now`, when one is due; the first frame is always due.
    fn next_path(&mut self, now: Instant) -> Option<PathBuf> {
        if now < self.next_due {
            return None;
        }
        let path = self.dir.join(format!("{}.png", self.saved));
        self.saved += 1;
        // Keep the schedule anchored to the start; after a stall, continue from now instead of bursting.
        self.next_due = (self.next_due + self.interval).max(now);
        Some(path)
    }

    fn expired(&self, now: Instant) -> bool {
        now.duration_since(self.started) >= self.duration
    }

    fn elapsed(&self, now: Instant) -> Duration {
        now.duration_since(self.started)
    }

    fn finish(self) {
        println!("recording stopped: {} images in {}", self.saved, self.dir.display());
    }
}

fn cv(e: opencv::Error) -> Error {
    Error::InvalidResponse(format!("OpenCV: {e}"))
}

/// Runs the viewer on `sdk`, which must already have its data directory set; `data_dir` is that
/// directory, the default root for recordings.
pub fn run(sdk: &Sdk, data_dir: Option<&Path>, args: ViewArgs) -> Result<ExitCode> {
    let cfg = match &args.config {
        Some(path) => RecordConfig::load(path)?,
        None => RecordConfig::default(),
    };
    let record_root = cfg.data_dir(data_dir)?;
    println!(
        "recording: {} images/s for {} s into {}{}",
        cfg.rate_hz,
        cfg.duration_s,
        record_root.display(),
        if args.config.is_none() { " (defaults; --config FILE changes them)" } else { "" }
    );

    let dev = match &args.camera {
        Some(query) => sdk.find_device(query)?,
        None => {
            let mut devs = sdk.enumerate()?;
            if devs.len() != 1 {
                return Err(Error::InvalidArgument(format!(
                    "{} cameras found; pass a name or serial (see `gcam-mvsdk-rs list`)",
                    devs.len()
                )));
            }
            devs.remove(0)
        }
    };
    let cam = sdk.open(&dev)?;
    let camera_name = match cam.user_name() {
        Ok(name) if !name.is_empty() => name,
        _ => dev.serial.clone(),
    };
    cam.set_output_format(OutputFormat::Mono8)?;
    cam.set_trigger_mode(TriggerMode::Continuous)?;
    // The fastest frame-speed tier unless one is given: the camera's stored default may be a
    // slower tier, and nothing is persisted here, so the viewer sets what it needs on every start.
    let caps = cam.capability()?;
    if let Some(tier) = args.frame_speed.or_else(|| caps.frame_speeds.iter().map(|s| s.index).max()) {
        cam.set_frame_speed(tier)?;
        let name = caps.frame_speeds.iter().find(|s| s.index == tier).map(|s| s.description.as_str()).unwrap_or("?");
        println!("frame-speed tier {tier} ({name}) for this run; --frame-speed IDX selects another");
    }
    cam.set_auto_exposure(false)?;
    cam.set_exposure_us(args.exposure_us)?;
    let mtu = args.mtu.or_else(|| std::env::var("GIGE_MTU").ok().and_then(|v| v.trim().parse().ok()));
    if let (Some(mtu), true) = (mtu, dev.is_gige()) {
        println!("GigE packet size: {} bytes (limit {mtu})", cam.set_gige_packet_size(mtu)?);
    }
    let cap = match cam.frame_rate() {
        Ok(hz) if hz > 0 => format!("cap {hz} Hz"),
        Ok(_) => "cap none".to_string(),
        Err(_) => "cap unknown".to_string(),
    };
    highgui::named_window(WINDOW, highgui::WINDOW_NORMAL).map_err(cv)?;
    cam.play()?;

    let mut pixels: Vec<u8> = Vec::new(); // ISP output, reused; the Mat below views it without a copy
    let mut stamps: VecDeque<Instant> = VecDeque::new();
    let mut session: Option<Session> = None;
    loop {
        let frame = match cam.grab(Duration::from_secs(1)) {
            Ok(frame) => frame,
            Err(e) if e.is_timeout() => {
                eprintln!("no frame within 1 s (check --mtu, the firewall and rp_filter)");
                continue;
            }
            Err(e) => return Err(e),
        };
        pixels.resize(frame.processed_len()?, 0);
        let info = frame.process_into(&mut pixels)?;
        drop(frame); // return the raw buffer to the SDK before drawing

        let now = Instant::now();
        stamps.push_back(now);
        while stamps.front().is_some_and(|t| now.duration_since(*t) > Duration::from_secs(1)) {
            stamps.pop_front();
        }
        let (w, h) = (info.width as i32, info.height as i32);
        let mut view = Mat::new_rows_cols_with_data_mut(h, w, &mut pixels[..info.len]).map_err(cv)?; // CV_8UC1, zero copy

        // Save before the overlay is drawn, so the files hold the plain camera image.
        let mut rec_text = None;
        if let Some(s) = session.as_mut() {
            if let Some(path) = s.next_path(now) {
                if !imgcodecs::imwrite(&path, &view, &Vector::new()).map_err(cv)? {
                    return Err(Error::InvalidResponse(format!("could not write {}", path.display())));
                }
            }
            if s.expired(now) {
                if let Some(done) = session.take() {
                    done.finish();
                }
            } else {
                rec_text = Some(format!("REC {} saved  {:.1} s", s.saved, s.elapsed(now).as_secs_f64()));
            }
        }

        // Frames returned by grab in the last second, over the time they span.
        let span = now.duration_since(stamps[0]).as_secs_f64();
        let fps = if span > 0.0 { (stamps.len() - 1) as f64 / span } else { 0.0 };
        let status = format!("measured {fps:.1} fps, {cap}");
        let hints = key_hints(session.is_some());
        let lines = [
            (status.as_str(), Point::new(12, 36)),
            (rec_text.as_deref().unwrap_or(""), Point::new(12, 76)),
            (hints, Point::new(12, h - 16)), // bottom left, out of the way of the status lines
        ];
        for (text, origin) in lines.iter().filter(|(text, _)| !text.is_empty()) {
            for (shade, thickness) in [(0.0, 4), (255.0, 2)] {
                // Dark outline first, then light text, so the overlay reads on any background.
                let (font, color) = (imgproc::FONT_HERSHEY_SIMPLEX, Scalar::all(shade));
                imgproc::put_text(&mut view, text, *origin, font, 1.0, color, thickness, imgproc::LINE_AA, false)
                    .map_err(cv)?;
            }
        }
        highgui::imshow(WINDOW, &view).map_err(cv)?;

        let key = highgui::wait_key(1).map_err(cv)? & 0xff;
        let closed = highgui::get_window_property(WINDOW, highgui::WND_PROP_VISIBLE).map_err(cv)? < 1.0;
        match action_for_key(key, session.is_some()) {
            Action::Start => {
                let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
                let dir = record_root.join(session_dir_name(&stamp, &camera_name));
                fs::create_dir_all(&dir).map_err(|e| Error::io(format!("creating {}", dir.display()), e))?;
                println!("recording to {}", dir.display());
                session = Some(Session::new(dir, Instant::now(), &cfg));
            }
            Action::Stop => {
                if let Some(done) = session.take() {
                    done.finish();
                }
            }
            Action::Quit => break,
            Action::Ignore => {}
        }
        if closed {
            break;
        }
    }
    if let Some(done) = session.take() {
        done.finish();
    }
    cam.pause()?;
    Ok(ExitCode::SUCCESS) // `cam` is closed by Drop
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hints_name_only_the_keys_that_act() {
        assert_eq!(key_hints(false), "r: record   q/Esc: quit");
        assert_eq!(key_hints(true), "c: stop recording   q/Esc: quit");
    }

    #[test]
    fn keys_depend_on_recording_state() {
        assert_eq!(action_for_key(b'r' as i32, false), Action::Start);
        assert_eq!(action_for_key(b'r' as i32, true), Action::Ignore);
        assert_eq!(action_for_key(b'c' as i32, true), Action::Stop);
        assert_eq!(action_for_key(b'c' as i32, false), Action::Ignore);
        assert_eq!(action_for_key(27, false), Action::Quit);
        assert_eq!(action_for_key(b'q' as i32, true), Action::Quit);
        assert_eq!(action_for_key(-1, false), Action::Ignore);
    }

    #[test]
    fn config_parses_and_validates() {
        assert_eq!(RecordConfig::parse("").unwrap(), RecordConfig::default());
        let cfg = RecordConfig::parse("duration_s: 2\nrate_hz: 10\ndata_dir: /tmp/x\n").unwrap();
        assert_eq!((cfg.duration_s, cfg.rate_hz, cfg.data_dir.as_deref()), (2.0, 10.0, Some(Path::new("/tmp/x"))));
        assert_eq!(cfg.interval(), Duration::from_millis(100));
        assert!(RecordConfig::parse("rate_hz: 0").unwrap_err().contains("rate_hz"));
        assert!(RecordConfig::parse("duration_s: -1").unwrap_err().contains("duration_s"));
        assert!(RecordConfig::parse("fps: 5").is_err());
        let fallback = Path::new("/sdk/data");
        assert_eq!(RecordConfig::default().data_dir(Some(fallback)).unwrap(), fallback);
        assert_eq!(cfg.data_dir(Some(fallback)).unwrap(), Path::new("/tmp/x"));
        assert!(RecordConfig::default().data_dir(None).is_err());
    }

    #[test]
    fn session_samples_at_the_configured_rate() {
        let cfg = RecordConfig { duration_s: 1.0, rate_hz: 10.0, data_dir: None };
        let t0 = Instant::now();
        let mut s = Session::new(PathBuf::from("/r"), t0, &cfg);
        assert_eq!(s.next_path(t0), Some(PathBuf::from("/r/0.png")));
        assert_eq!(s.next_path(t0 + Duration::from_millis(50)), None);
        assert_eq!(s.next_path(t0 + Duration::from_millis(100)), Some(PathBuf::from("/r/1.png")));
        // After a stall the schedule continues from now instead of bursting: the frame at 500 ms
        // is due, the one at 550 ms too (the stall ended at 500 ms), the one at 590 ms not yet.
        assert_eq!(s.next_path(t0 + Duration::from_millis(500)), Some(PathBuf::from("/r/2.png")));
        assert_eq!(s.next_path(t0 + Duration::from_millis(550)), Some(PathBuf::from("/r/3.png")));
        assert_eq!(s.next_path(t0 + Duration::from_millis(590)), None);
        assert_eq!(s.next_path(t0 + Duration::from_millis(650)), Some(PathBuf::from("/r/4.png")));
        assert!(!s.expired(t0 + Duration::from_millis(999)));
        assert!(s.expired(t0 + Duration::from_secs(1)));
        assert_eq!(session_dir_name("20260101_120000", "sb 0/1"), "20260101_120000_sb_0_1");
        assert_eq!(session_dir_name("20260101_120000", ""), "20260101_120000_camera");
    }
}
