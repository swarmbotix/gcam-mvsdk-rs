//! Hardware integration tests, for the bench machine with cameras attached.
//!
//! Every test returns immediately unless `GCAM_MVSDK_HW_TEST=1`. The tests
//! run as a normal user; GigE discovery needs no privileges.
//!
//! ```text
//! GCAM_MVSDK_HW_TEST=1 cargo test --test hw -- --nocapture
//! ```
//!
//! Optional variables:
//! - `GCAM_MVSDK_HW_EXPECT=sb00,sb01,sb02,sb03`: user-set names that must be present.
//! - `GCAM_MVSDK_HW_CAMERA=sb00`: camera used by the grab test (default: the first one).
//! - `GCAM_MVSDK_HW_RENAME_SN=<serial>`: enables the rename round trip on that camera.
//!   The original name is written back at the end.

use std::sync::Mutex;
use std::time::Duration;

use gcam_mvsdk_rs::{OutputFormat, Sdk, TriggerMode};

/// The SDK handles one camera per process at a time in these tests.
static SERIAL: Mutex<()> = Mutex::new(());

fn enabled() -> bool {
    std::env::var(gcam_mvsdk_rs::ENV_HW_TEST).map(|v| v == "1").unwrap_or(false)
}

macro_rules! require_hw {
    () => {
        if !enabled() {
            eprintln!("skipped: set GCAM_MVSDK_HW_TEST=1 to run hardware tests");
            return;
        }
    };
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

fn sdk() -> Sdk {
    Sdk::load().expect("the MVSDK library must be installed (gcam-mvsdk-rs setup or GCAM_MVSDK_DIR)")
}

#[test]
fn sdk_loads_and_reports_version() {
    require_hw!();
    let _g = lock();
    let sdk = sdk();
    let version = sdk.version().expect("CameraSdkGetVersionString");
    eprintln!("SDK {version} from {}", sdk.library_path().display());
    assert!(version.starts_with("2."), "unexpected version {version}");
}

#[test]
fn enumerate_finds_expected_cameras() {
    require_hw!();
    let _g = lock();
    let sdk = sdk();
    let devs = sdk.enumerate().expect("enumerate");
    assert!(!devs.is_empty(), "no cameras found; check power, cabling, rp_filter and firewall (gcam-mvsdk-rs doctor)");
    let mut names = Vec::new();
    for d in &devs {
        assert!(!d.serial.is_empty());
        let name = sdk.open(d).and_then(|c| c.user_name()).unwrap_or_default();
        eprintln!("SN {}  user-set name {name:?}  model {:?}  port {:?}", d.serial, d.friendly_name, d.port_type);
        names.push(name);
    }
    if let Ok(expect) = std::env::var("GCAM_MVSDK_HW_EXPECT") {
        for want in expect.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            assert!(names.iter().any(|n| n == want), "camera {want:?} not found among {names:?}");
        }
    }
}

#[test]
fn gige_ip_and_mac_are_readable() {
    require_hw!();
    let _g = lock();
    let sdk = sdk();
    for d in sdk.enumerate().expect("enumerate").iter().filter(|d| d.is_gige()) {
        let ip = d.gige_ip().expect("CameraGigeGetIp");
        let mac = d.gige_mac().expect("CameraGigeGetMac");
        eprintln!("SN {}  camera {}  host {}  MAC {} / {}", d.serial, ip.camera, ip.host, mac.camera, mac.host);
        assert!(!ip.camera.ip.is_unspecified());
        assert!(gcam_mvsdk_rs::normalize_mac(&mac.camera).is_some(), "MAC {:?}", mac.camera);
        assert!(ip.same_subnet(), "camera {} is outside the host NIC subnet", ip.camera.ip);
    }
}

#[test]
fn rename_round_trip() {
    require_hw!();
    let Ok(serial) = std::env::var("GCAM_MVSDK_HW_RENAME_SN") else {
        eprintln!("skipped: set GCAM_MVSDK_HW_RENAME_SN=<serial> to write a camera name");
        return;
    };
    let _g = lock();
    let sdk = sdk();
    let devs = sdk.enumerate().expect("enumerate");
    let dev = devs.iter().find(|d| d.serial == serial).expect("camera with GCAM_MVSDK_HW_RENAME_SN");
    let cam = sdk.open(dev).expect("open");
    let original = cam.user_name().expect("read name");
    let temp = format!("hwtest{}", std::process::id() % 10_000);
    cam.set_user_name(&temp).expect("write temporary name");
    let read_back = cam.user_name().expect("read temporary name");
    // Restore before asserting so a failure does not leave the camera renamed.
    let restore = if original.is_empty() { None } else { Some(cam.set_user_name(&original)) };
    assert_eq!(read_back, temp);
    if let Some(r) = restore {
        r.expect("restore original name");
        assert_eq!(cam.user_name().expect("read restored name"), original);
    }
}

#[test]
fn control_getters_answer() {
    // Read-only smoke test of the controls added for docs/CAPABILITIES.md: a wrong FFI
    // signature would crash here. Models may reject a getter with an SDK status, which is fine.
    require_hw!();
    let _g = lock();
    let sdk = sdk();
    let dev = sdk.enumerate().expect("enumerate").into_iter().next().expect("at least one camera");
    let cam = sdk.open(&dev).expect("open");
    eprintln!("ae_target {:?}", cam.ae_target());
    eprintln!("anti_flick {:?}", cam.anti_flick());
    eprintln!("light_frequency {:?}", cam.light_frequency());
    eprintln!("mirror h {:?}", cam.mirror(gcam_mvsdk_rs::MirrorDirection::Horizontal));
    eprintln!("rotation {:?}", cam.rotation());
    eprintln!("black_level {:?}", cam.black_level());
    eprintln!("auto_white_balance {:?}", cam.auto_white_balance());
    eprintln!("strobe_mode {:?}", cam.strobe_mode());
    eprintln!("parameter_mode {:?}", cam.parameter_mode());
    eprintln!("current_parameter_team {:?}", cam.current_parameter_team());
    let cap = cam.capability().expect("capability");
    if cap.input_io_count > 0 {
        eprintln!("input 0 {:?}", cam.input_io_state(0));
    }
    if cap.output_io_count > 0 {
        eprintln!("output 0 {:?}", cam.output_io_state(0));
    }
    let res = cam.image_resolution().expect("image_resolution");
    cam.set_image_resolution(&res).expect("re-applying the current resolution");
}

#[test]
fn grab_produces_mono8_frame() {
    require_hw!();
    let _g = lock();
    let sdk = sdk();
    let dev = match std::env::var("GCAM_MVSDK_HW_CAMERA") {
        Ok(q) => sdk.find_device(&q).expect("GCAM_MVSDK_HW_CAMERA"),
        Err(_) => sdk.enumerate().expect("enumerate").into_iter().next().expect("at least one camera"),
    };
    let cam = sdk.open(&dev).expect("open");
    let cap = cam.capability().expect("capability");
    cam.set_output_format(OutputFormat::Mono8).expect("Mono8");
    cam.set_trigger_mode(TriggerMode::Continuous).expect("continuous");
    if let Ok(mtu) = std::env::var("GIGE_MTU") {
        if let Ok(mtu) = mtu.parse() {
            cam.set_gige_packet_size(mtu).expect("packet size");
        }
    }
    let expected = cam.image_resolution().ok();
    cam.play().expect("play");
    let mut last_err = None;
    for _ in 0..5 {
        match cam.grab(Duration::from_millis(3000)) {
            Ok(frame) => {
                assert!(!frame.as_slice().is_empty());
                let image = frame.process().expect("process");
                let (w, h) = (image.info.width as usize, image.info.height as usize);
                assert!(w > 0 && h > 0);
                assert!(w as i32 <= cap.max_width() && h as i32 <= cap.max_height());
                if let Some(r) = &expected {
                    if r.width > 0 && r.height > 0 {
                        assert_eq!((w as i32, h as i32), (r.width, r.height));
                    }
                }
                assert_eq!(image.info.media_type, gcam_mvsdk_rs::ffi::CAMERA_MEDIA_TYPE_MONO8);
                assert_eq!(image.data.len(), w * h);
                eprintln!("SN {}: {w}x{h} Mono8, ts {:?}", dev.serial, frame.timestamp());
                cam.pause().expect("pause");
                return;
            }
            Err(e) if e.is_timeout() => last_err = Some(e),
            Err(e) => panic!("grab failed: {e}"),
        }
    }
    panic!("no frame within 5 attempts: {last_err:?}");
}
