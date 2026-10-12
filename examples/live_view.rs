//! Minimal live view: shows the frames of one camera in an OpenCV window with an FPS overlay.
//!
//! Build requirements on Ubuntu (the opencv crate generates its bindings with libclang):
//!     sudo apt-get install -y libopencv-dev clang libclang-dev
//! Run:
//!     cargo run --release --features view --example live_view -- [NAME_OR_SN] [--exposure-us 5000] [--mtu 1500]
//! Keys: q or Esc quits; closing the window also quits. GigE discovery needs CAP_NET_RAW:
//!     sudo setcap cap_net_raw,cap_net_admin+ep target/release/examples/live_view

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use gcam_mvsdk_rs::{OutputFormat, Sdk, TriggerMode};
use opencv::core::{Mat, Point, Scalar};
use opencv::{highgui, imgproc};

const WINDOW: &str = "gcam-mvsdk-rs live view";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut camera, mut exposure_us, mut mtu) = (None, 5000.0, None);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--exposure-us" => exposure_us = args.next().ok_or("--exposure-us needs a value")?.parse()?,
            "--mtu" => mtu = Some(args.next().ok_or("--mtu needs a value")?.parse::<u32>()?),
            "-h" | "--help" => {
                println!("usage: live_view [NAME_OR_SN] [--exposure-us US] [--mtu BYTES]");
                return Ok(());
            }
            _ => camera = Some(arg),
        }
    }

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
    cam.set_output_format(OutputFormat::Mono8)?;
    cam.set_trigger_mode(TriggerMode::Continuous)?;
    cam.set_auto_exposure(false)?;
    cam.set_exposure_us(exposure_us)?;
    if let (Some(mtu), true) = (mtu, dev.is_gige()) {
        println!("GigE packet size: {} bytes", cam.set_gige_packet_size(mtu)?);
    }
    highgui::named_window(WINDOW, highgui::WINDOW_NORMAL)?;
    cam.play()?;

    let mut pixels: Vec<u8> = Vec::new(); // ISP output, reused; the Mat below views it without a copy
    let mut stamps: VecDeque<Instant> = VecDeque::new();
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
        let text = format!("{} fps", stamps.len());
        for (shade, thickness) in [(0.0, 4), (255.0, 2)] {
            // Dark outline first, then light text, so the overlay reads on any background.
            let (font, color) = (imgproc::FONT_HERSHEY_SIMPLEX, Scalar::all(shade));
            imgproc::put_text(&mut view, &text, Point::new(12, 36), font, 1.0, color, thickness, imgproc::LINE_AA, false)?;
        }
        highgui::imshow(WINDOW, &view)?;

        let key = highgui::wait_key(1)? & 0xff;
        let closed = highgui::get_window_property(WINDOW, highgui::WND_PROP_VISIBLE)? < 1.0;
        if key == i32::from(b'q') || key == 27 || closed {
            break;
        }
    }
    cam.pause()?;
    Ok(()) // `cam` is closed by Drop
}
