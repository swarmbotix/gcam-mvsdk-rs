//! Resolves every SDK function name this crate declares or documents against a locally installed
//! SDK (./vendor, $GCAM_MVSDK_HOME or ~/.gcam-mvsdk, or $GCAM_MVSDK_DIR). Skips without one, as
//! in CI. Loading the library resolves symbols only; it does not initialise the SDK.

use std::collections::BTreeSet;
use std::path::Path;

use gcam_mvsdk_rs::ffi::Api;
use gcam_mvsdk_rs::{resolve_library, Candidate, SearchConfig};

fn local_sdk() -> Option<Api> {
    let cfg = SearchConfig::from_env().ok()?;
    match resolve_library(&cfg).ok()?.candidate {
        // SAFETY: a file found in the documented SDK locations.
        Candidate::Path(p) => unsafe { Api::load(p) }.ok(),
        Candidate::SystemName(_) => None,
    }
}

/// Vendor function names (`Camera` followed by an upper-case letter) quoted in a source file.
fn quoted_function_names(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for piece in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
        let is_name = piece.len() > 6
            && piece.starts_with("Camera")
            && piece[6..].starts_with(|c: char| c.is_ascii_uppercase())
            && !matches!(piece, "CameraHandle" | "CameraSdkStatus" | "CameraSnapProc")
            && !matches!(piece, "CameraApi" | "CameraDefine" | "CameraStatus") // header file names
            && !piece.ends_with("Callback") // callback type aliases, not functions
            || matches!(piece, "CameraSetFrameEventCallback" | "CameraSetConnectionStatusCallback");
        if is_name {
            out.insert(piece.to_string());
        }
    }
    out
}

#[test]
fn every_declared_symbol_is_exported() {
    let Some(api) = local_sdk() else {
        eprintln!("skipped: no local SDK (run ./scripts/fetch-sdk.sh or set GCAM_MVSDK_DIR)");
        return;
    };
    assert_eq!(api.missing_optional_symbols(), Vec::<&str>::new(), "in {}", api.path().display());
}

#[test]
fn every_documented_function_is_exported() {
    let Some(api) = local_sdk() else {
        eprintln!("skipped: no local SDK (run ./scripts/fetch-sdk.sh or set GCAM_MVSDK_DIR)");
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut names = BTreeSet::new();
    for rel in ["examples/capabilities.rs", "docs/CAPABILITIES.md", "src/controls.rs"] {
        if let Ok(text) = std::fs::read_to_string(root.join(rel)) {
            names.extend(quoted_function_names(&text));
        }
    }
    assert!(names.len() > 100, "found only {} names", names.len());
    let missing: Vec<&String> = names
        .iter()
        // SAFETY: only resolves the address; the pointer is never called.
        .filter(|n| unsafe { api.symbol::<unsafe extern "system" fn()>(n) }.is_err())
        .collect();
    assert!(missing.is_empty(), "not exported by {}: {missing:?}", api.path().display());
}
