//! `sdk-manifest.toml` parsing and release selection.

#![cfg(feature = "setup")]

use gcam_mvsdk_rs::setup::{
    read_install_manifest, write_install_manifest, InstallManifest, Layout, Manifest, MANIFEST_TOML,
};

const LINUX_SHA: &str = "9624d193a908d94a9850400346cf1ab8095437a040e381b2fd39029b0183dd8a";

/// The manifest is shared byte for byte with the Python package; any edit
/// must be made in both repositories.
const EXPECTED_MANIFEST: &str = r#"[[release]]
version = "2.1.0.49"
os      = "linux"
url     = "http://huatengvision.com/attachment/down/13/linuxSDK_V2.1.0.49%28202511141513%29.tar.gz"
size    = 57978497
sha256  = "9624d193a908d94a9850400346cf1ab8095437a040e381b2fd39029b0183dd8a"
layout  = "tar"

[[release]]
version = "2.1.10.195"
os      = "windows"
url     = "http://huatengvision.com/attachment/down/24/HuaTengVision%20Camera%20Platform%20Setup%282.1.10.195_20260616%29.rar"
size    = 128081184
sha256  = ""
layout  = "rar-installer"
"#;

#[test]
fn embedded_manifest_is_byte_identical() {
    assert_eq!(MANIFEST_TOML, EXPECTED_MANIFEST);
}

#[test]
fn embedded_manifest_contents() {
    let m = Manifest::embedded();
    assert_eq!(m.releases.len(), 2);

    let linux = m.latest("linux").unwrap();
    assert_eq!(linux.version, "2.1.0.49");
    assert_eq!(linux.size, 57_978_497);
    assert_eq!(linux.sha256, LINUX_SHA);
    assert_eq!(linux.layout, Layout::Tar);
    assert!(linux.has_checksum());
    assert_eq!(linux.file_name(), "linuxSDK_V2.1.0.49(202511141513).tar.gz");

    let win = m.latest("windows").unwrap();
    assert_eq!(win.version, "2.1.10.195");
    assert_eq!(win.size, 128_081_184);
    assert_eq!(win.layout, Layout::RarInstaller);
    assert!(!win.has_checksum());
    assert_eq!(win.file_name(), "HuaTengVision Camera Platform Setup(2.1.10.195_20260616).rar");

    assert!(m.latest("macos").is_none());
    assert_eq!(m.find_by_sha256(LINUX_SHA).unwrap().version, "2.1.0.49");
    assert_eq!(m.find_by_sha256(&LINUX_SHA.to_uppercase()).unwrap().version, "2.1.0.49");
    // An empty recorded checksum never matches.
    assert!(m.find_by_sha256("").is_none());
    assert!(m.find("2.1.0.49", "windows").is_none());
    assert!(m.find("2.1.0.49", "linux").is_some());
}

#[test]
fn latest_compares_versions_numerically() {
    let text = r#"
[[release]]
version = "2.1.9.0"
os = "linux"
url = "http://example.invalid/a.tar.gz"
size = 1
layout = "tar"

[[release]]
version = "2.1.10.0"
os = "linux"
url = "http://example.invalid/b.tar.gz"
size = 1
layout = "tar"
"#;
    let m = Manifest::parse(text).unwrap();
    assert_eq!(m.latest("linux").unwrap().version, "2.1.10.0");
}

#[test]
fn invalid_manifests_are_rejected() {
    let base = |field: &str| {
        format!(
            "[[release]]\nversion = \"1.0\"\nos = \"linux\"\nurl = \"http://x/y.tar.gz\"\nsize = 10\nlayout = \"tar\"\n{field}\n"
        )
    };
    assert!(Manifest::parse(&base("")).is_ok());
    assert!(Manifest::parse(&base("sha256 = \"abc\"")).is_err());
    assert!(Manifest::parse(&base(&format!("sha256 = \"{}\"", LINUX_SHA.to_uppercase()))).is_err());
    assert!(Manifest::parse(&base(&format!("sha256 = \"{LINUX_SHA}\""))).is_ok());
    assert!(Manifest::parse(&base("").replace("linux", "beos")).is_err());
    assert!(Manifest::parse(&base("").replace("http://", "ftp://")).is_err());
    assert!(Manifest::parse(&base("").replace("size = 10", "size = 0")).is_err());
    assert!(Manifest::parse("not toml at all [").is_err());
    // Unknown layouts parse (the file is shared with the Python package) but cannot be installed.
    let m = Manifest::parse(&base("").replace("\"tar\"", "\"zip\"")).unwrap();
    assert_eq!(m.releases[0].layout, Layout::Unknown);
}

#[test]
fn install_manifest_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let m = InstallManifest {
        version: "2.1.0.49".into(),
        source_url: "http://example.invalid/sdk.tar.gz".into(),
        sha256: LINUX_SHA.into(),
        installed_at: "2026-10-12T00:00:00Z".into(),
        os: Some("linux".into()),
        source_file: None,
        installer: Some("gcam-mvsdk-rs 0.1.0".into()),
    };
    write_install_manifest(dir.path(), &m).unwrap();
    assert_eq!(read_install_manifest(dir.path()).unwrap(), m);

    // The four fields the Python package relies on are plain top-level keys.
    let text = std::fs::read_to_string(dir.path().join("manifest.json")).unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    for key in ["version", "source_url", "sha256", "installed_at"] {
        assert!(v.get(key).and_then(|x| x.as_str()).is_some(), "missing {key}");
    }
    assert!(v.get("source_file").is_none());
}
