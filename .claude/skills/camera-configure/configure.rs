//! Template: read a camera's settings and ranges, apply new settings, optionally save them.
//!
//! Setup (once):
//!     cargo new --bin cam-config && cd cam-config
//!     cargo add gcam-mvsdk-rs --path /path/to/gcam-mvsdk-rs --no-default-features
//!     cp /path/to/gcam-mvsdk-rs/.claude/skills/camera-configure/configure.rs src/main.rs
//! Use:
//!     cargo run --release -- <NAME_OR_SN>                    # read only
//!     cargo run --release -- <NAME_OR_SN> --apply            # apply the values in configure()
//!     cargo run --release -- <NAME_OR_SN> --apply --save B   # and store them in parameter group B

use std::error::Error;

use gcam_mvsdk_rs::{Camera, MirrorDirection, ParameterTeam, Sdk, TriggerMode};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let camera = args.first().ok_or("usage: cam-config <NAME_OR_SN> [--apply] [--save A|B|C|D]")?;
    let apply = args.iter().any(|a| a == "--apply");
    let save = match args.iter().position(|a| a == "--save") {
        Some(i) => Some(parse_team(args.get(i + 1).map(String::as_str).unwrap_or(""))?),
        None => None,
    };
    let sdk = Sdk::load()?;
    let cam = sdk.open(&sdk.find_device(camera)?)?;
    println!("-- current settings");
    report(&cam)?;
    if apply {
        configure(&cam)?;
        println!("-- after applying");
        report(&cam)?;
    }
    if let Some(team) = save {
        cam.save_parameter(team)?;
        println!("saved to parameter group {team:?}");
    }
    Ok(())
}

/// The settings to apply. Read the ranges printed by `report` first and edit the values.
fn configure(cam: &Camera) -> Result<(), Box<dyn Error>> {
    let cap = cam.capability()?;
    cam.set_auto_exposure(false)?;
    cam.set_exposure_us(5000.0)?; // microseconds, within the printed range
    cam.set_analog_gain(cap.expose.uiAnalogGainMin as i32)?; // lowest gain, least noise
    cam.set_trigger_mode(TriggerMode::Continuous)?;
    // Uncomment what you need (all are methods of gcam_mvsdk_rs::Camera):
    // cam.set_frame_rate(30)?; // some GigE models only; 0 or less removes the cap
    // cam.set_roi(gcam_mvsdk_rs::Roi { x: 0, y: 0, width: 1280, height: 720, binning: gcam_mvsdk_rs::Binning::None })?;
    // cam.set_mirror(MirrorDirection::Horizontal, true)?;
    // cam.set_rotation(gcam_mvsdk_rs::Rotation::Ccw180)?;
    // cam.set_auto_white_balance(true)?; // color cameras only
    Ok(())
}

/// Prints the current values and the model's ranges. Values the model does not support print as errors.
fn report(cam: &Camera) -> Result<(), Box<dyn Error>> {
    let cap = cam.capability()?;
    let (lo, hi, step) = cam.exposure_range_us()?;
    println!("exposure     {:.1} us (range {lo:.1} to {hi:.1}, step {step:.2}), auto {}", cam.exposure_us()?, cam.auto_exposure()?);
    let e = cap.expose;
    println!("analog gain  {} (range {} to {}, x{} per unit)", cam.analog_gain()?, e.uiAnalogGainMin, e.uiAnalogGainMax, e.fAnalogGainStep);
    println!("frame rate   cap {:?} Hz, speed tier {:?}", cam.frame_rate(), cam.frame_speed());
    println!("trigger      {:?}", cam.trigger_mode()?);
    let r = cam.image_resolution()?;
    println!("resolution   {}x{} at ({}, {}) '{}'", r.width, r.height, r.fov_offset_x, r.fov_offset_y, r.description);
    println!("mirror       h {:?} v {:?}, rotation {:?}", cam.mirror(MirrorDirection::Horizontal), cam.mirror(MirrorDirection::Vertical), cam.rotation());
    if !cap.is_mono() {
        println!("white bal.   auto {:?}", cam.auto_white_balance());
    }
    println!("parameters   group {:?}, storage {:?}, in-camera storage available: {}", cam.current_parameter_team(), cam.parameter_mode(), cap.param_in_device);
    Ok(())
}

fn parse_team(text: &str) -> Result<ParameterTeam, Box<dyn Error>> {
    match text {
        "A" => Ok(ParameterTeam::A),
        "B" => Ok(ParameterTeam::B),
        "C" => Ok(ParameterTeam::C),
        "D" => Ok(ParameterTeam::D),
        _ => Err(format!("--save needs A, B, C or D, got {text:?}").into()),
    }
}
