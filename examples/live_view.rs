//! Live view with recording: shows the frames of one camera in an OpenCV window with an FPS
//! overlay and saves PNG images on request.
//!
//! Build requirements on Ubuntu (the opencv crate generates its bindings with libclang):
//!     sudo apt-get install -y libopencv-dev clang libclang-dev
//! Run:
//!     cargo run --release --features view --example live_view -- [NAME_OR_SN] [--exposure-us 5000] [--mtu 1500] [--config record.yaml]
//! Keys: r starts a recording, c stops it early, q or Esc quits; closing the window also quits.
//! A recording saves `rate_hz` images per second for `duration_s` seconds (examples/record.yaml)
//! into `<data_dir>/<YYYYMMDD_HHMMSS>_<camera>/0.png, 1.png, ...`; `data_dir` defaults to
//! `<sdk home>/data`.
//! Tests (no camera or display): cargo test --features view --example live_view

use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gcam_mvsdk_rs::{OutputFormat, Sdk, TriggerMode};
use opencv::core::{Mat, Point, Scalar, Vector};
use opencv::{highgui, imgcodecs, imgproc};
use serde::Deserialize;

const WINDOW: &str = "gcam-mvsdk-rs live view";
/// The config file shipped next to this example; `--config` selects another.
const DEFAULT_CONFIG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/record.yaml");

/// Recording options, read from `record.yaml`. Missing keys take the defaults below.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordConfig {
    /// A recording stops after this many seconds.
    #[serde(default = "RecordConfig::default_duration_s")]
    duration_s: f64,
    /// Images saved per second; frames in between are shown but not saved.
    #[serde(default = "RecordConfig::default_rate_hz")]
    rate_hz: f64,
    /// Root for the session folders; `None` means `<sdk home>/data`.
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
    fn parse(text: &str) -> Result<Self, String> {
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

    /// Reads `path`; a missing file gives the defaults and `false`.
    fn load(path: &Path) -> Result<(Self, bool), String> {
        match fs::read_to_string(path) {
            Ok(text) => Ok((Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?, true)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok((Self::default(), false)),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// Root for the session folders: `data_dir` with a leading `~` expanded, else `<sdk home>/data`.
    fn data_dir(&self) -> Result<PathBuf, String> {
        match &self.data_dir {
            Some(dir) => Ok(expand_tilde(dir)),
            None => gcam_mvsdk_rs::sdk_home()
                .map(|(home, _)| home.join("data"))
                .ok_or_else(|| "cannot determine the SDK home; set data_dir in record.yaml".to_string()),
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut camera, mut exposure_us, mut mtu) = (None, 5000.0, None);
    let mut config = PathBuf::from(DEFAULT_CONFIG);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--exposure-us" => exposure_us = args.next().ok_or("--exposure-us needs a value")?.parse()?,
            "--mtu" => mtu = Some(args.next().ok_or("--mtu needs a value")?.parse::<u32>()?),
            "--config" => config = PathBuf::from(args.next().ok_or("--config needs a path")?),
            "-h" | "--help" => {
                println!("usage: live_view [NAME_OR_SN] [--exposure-us US] [--mtu BYTES] [--config record.yaml]");
                println!("keys: r start recording, c stop recording, q or Esc quit");
                return Ok(());
            }
            _ => camera = Some(arg),
        }
    }

    let (cfg, found) = RecordConfig::load(&config)?;
    if !found {
        println!("{} not found; recording with defaults", config.display());
    }
    let data_dir = cfg.data_dir()?;
    println!("recording: {} images/s for {} s into {}", cfg.rate_hz, cfg.duration_s, data_dir.display());

    let sdk = Sdk::load()?;
    let dev = match camera {
        Some(query) => sdk.find_device(&query)?,
        None => {
            let mut devs = sdk.enumerate()?;
            if devs.len() != 1 {
                eprintln!("{} cameras found; pass a name or serial (see `gcam-mvsdk-rs list`)", devs.len());
                std::process::exit(2);
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
    cam.set_auto_exposure(false)?;
    cam.set_exposure_us(exposure_us)?;
    if let (Some(mtu), true) = (mtu, dev.is_gige()) {
        println!("GigE packet size: {} bytes", cam.set_gige_packet_size(mtu)?);
    }
    let cap = match cam.frame_rate() {
        Ok(hz) if hz > 0 => format!("cap {hz} Hz"),
        Ok(_) => "cap none".to_string(),
        Err(_) => "cap unknown".to_string(),
    };
    highgui::named_window(WINDOW, highgui::WINDOW_NORMAL)?;
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
            Err(e) => return Err(e.into()),
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
        let mut view = Mat::new_rows_cols_with_data_mut(h, w, &mut pixels[..info.len])?; // CV_8UC1, zero copy

        // Save before the overlay is drawn, so the files hold the plain camera image.
        let mut rec_text = None;
        if let Some(s) = session.as_mut() {
            if let Some(path) = s.next_path(now) {
                if !imgcodecs::imwrite(&path, &view, &Vector::new())? {
                    return Err(format!("could not write {}", path.display()).into());
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
        let lines = [Some(format!("measured {fps:.1} fps, {cap}")), rec_text];
        for (i, text) in lines.iter().flatten().enumerate() {
            for (shade, thickness) in [(0.0, 4), (255.0, 2)] {
                // Dark outline first, then light text, so the overlay reads on any background.
                let (font, color) = (imgproc::FONT_HERSHEY_SIMPLEX, Scalar::all(shade));
                let origin = Point::new(12, 36 + 40 * i as i32);
                imgproc::put_text(&mut view, text, origin, font, 1.0, color, thickness, imgproc::LINE_AA, false)?;
            }
        }
        highgui::imshow(WINDOW, &view)?;

        let key = highgui::wait_key(1)? & 0xff;
        let closed = highgui::get_window_property(WINDOW, highgui::WND_PROP_VISIBLE)? < 1.0;
        match action_for_key(key, session.is_some()) {
            Action::Start => {
                let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
                let dir = data_dir.join(session_dir_name(&stamp, &camera_name));
                fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
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
    Ok(()) // `cam` is closed by Drop
}


