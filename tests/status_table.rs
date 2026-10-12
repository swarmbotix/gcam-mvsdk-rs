//! Status code table and error formatting (no SDK needed).

use std::collections::HashSet;

use gcam_mvsdk_rs::ffi::{self, status_info, STATUS_TABLE};
use gcam_mvsdk_rs::{Error, MvError};

#[test]
fn table_covers_every_sdk_status_code() {
    // CameraStatus.h defines 0 down to -57 without gaps.
    for code in -57..=0 {
        let info = status_info(code).unwrap_or_else(|| panic!("status {code} missing"));
        assert!(info.name.starts_with("CAMERA_STATUS_"), "{}", info.name);
        assert!(!info.description.is_empty());
    }
}

#[test]
fn codes_and_names_are_unique() {
    let codes: HashSet<i32> = STATUS_TABLE.iter().map(|s| s.code).collect();
    let names: HashSet<&str> = STATUS_TABLE.iter().map(|s| s.name).collect();
    assert_eq!(codes.len(), STATUS_TABLE.len());
    assert_eq!(names.len(), STATUS_TABLE.len());
}

#[test]
fn well_known_codes() {
    assert_eq!(ffi::CAMERA_STATUS_SUCCESS, 0);
    assert_eq!(ffi::CAMERA_STATUS_TIME_OUT, -12);
    assert_eq!(ffi::CAMERA_STATUS_NO_DEVICE_FOUND, -16);
    assert_eq!(ffi::CAMERA_STATUS_ACCESS_DENY, -45);
    assert_eq!(ffi::CAMERA_STATUS_INVALID_FRIENDLY_NAME, -53);
    assert_eq!(ffi::CAMERA_AIA_NOT_IMPLEMENTED, 0x8001);
    assert_eq!(status_info(-12).unwrap().name, "CAMERA_STATUS_TIME_OUT");
    assert_eq!(status_info(0x8006).unwrap().name, "CAMERA_AIA_ACCESS_DENIED");
    assert!(status_info(-1000).is_none());
}

#[test]
fn mv_error_formatting() {
    let e = MvError::new("CameraGetImageBuffer", ffi::CAMERA_STATUS_TIME_OUT);
    assert!(e.is_timeout());
    assert_eq!(e.status_name(), Some("CAMERA_STATUS_TIME_OUT"));
    let text = e.to_string();
    assert!(text.contains("CameraGetImageBuffer"), "{text}");
    assert!(text.contains("-12"), "{text}");
    assert!(text.contains("CAMERA_STATUS_TIME_OUT"), "{text}");

    let unknown = MvError::new("CameraPlay", -999);
    assert_eq!(unknown.status_name(), None);
    assert!(unknown.to_string().contains("-999"));

    let err: Error = e.into();
    assert!(err.is_timeout());
    assert_eq!(err.status(), Some(-12));
}
