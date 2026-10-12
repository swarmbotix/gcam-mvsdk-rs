//! Verification and extraction against a synthetic vendor tarball.

#![cfg(feature = "setup")]

use std::fs::{self, File};
use std::path::{Path, PathBuf};

use gcam_mvsdk_rs::setup::{
    extract_sdk_tarball, install_from_file, read_install_manifest, sha256_file, verify_archive, Manifest,
    SetupOptions, SetupOutcome,
};

const TOP: &str = "linuxSDK_V9.9.9.9(000000000000)";

fn regular(name: &str, data: &[u8]) -> (String, Vec<u8>) {
    (name.to_string(), data.to_vec())
}

/// Writes a .tar.gz shaped like the vendor archive. `raw_names` are written
/// into the header bytes directly, bypassing the tar crate's path checks.
fn build_tarball(path: &Path, files: &[(String, Vec<u8>)], raw_names: &[&str], symlink: Option<(&str, &str)>) {
    let f = File::create(path).unwrap();
    let gz = flate2::write::GzEncoder::new(f, flate2::Compression::fast());
    let mut b = tar::Builder::new(gz);
    for dir in [format!("{TOP}/"), format!("{TOP}/lib/"), format!("{TOP}/lib/x64/")] {
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(tar::EntryType::Directory);
        h.set_mode(0o777);
        h.set_size(0);
        b.append_data(&mut h, dir, std::io::empty()).unwrap();
    }
    for (name, data) in files {
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(tar::EntryType::Regular);
        h.set_mode(0o666);
        h.set_size(data.len() as u64);
        b.append_data(&mut h, name, data.as_slice()).unwrap();
    }
    if let Some((name, target)) = symlink {
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(tar::EntryType::Symlink);
        h.set_size(0);
        h.set_mode(0o777);
        b.append_link(&mut h, name, target).unwrap();
    }
    for raw in raw_names {
        let data = b"evil";
        let mut h = tar::Header::new_old();
        h.set_entry_type(tar::EntryType::Regular);
        h.set_mode(0o644);
        h.set_size(data.len() as u64);
        let name = &mut h.as_old_mut().name;
        name[..raw.len()].copy_from_slice(raw.as_bytes());
        h.set_cksum();
        b.append(&h, &data[..]).unwrap();
    }
    b.into_inner().unwrap().finish().unwrap();
}

fn sdk_files() -> Vec<(String, Vec<u8>)> {
    vec![
        regular(&format!("{TOP}/include/CameraApi.h"), b"/* api */"),
        regular(&format!("{TOP}/lib/x64/libMVSDK.so"), b"x64 library bytes"),
        regular(&format!("{TOP}/lib/arm64/libMVSDK.so"), b"arm64 library bytes"),
        regular(&format!("{TOP}/tools/GeConfigCmd/x64/GeConfigCmd"), b"tool"),
        regular(&format!("{TOP}/tools/GTKGeConfigTools/GeConfigTools"), b"gui tool"),
        regular(&format!("{TOP}/demo/python_demo/mvsdk.py"), b"print()"),
        regular(&format!("{TOP}/88-mvusb.rules"), b"rule"),
        regular(&format!("{TOP}/install.sh"), b"#!/bin/sh"),
    ]
}

fn manifest_for(archive: &Path, version: &str) -> Manifest {
    let size = fs::metadata(archive).unwrap().len();
    let sha = sha256_file(archive).unwrap();
    Manifest::parse(&format!(
        "[[release]]\nversion = \"{version}\"\nos = \"linux\"\nurl = \"http://example.invalid/x.tar.gz\"\n\
         size = {size}\nsha256 = \"{sha}\"\nlayout = \"tar\"\n"
    ))
    .unwrap()
}

fn opts(dest: PathBuf) -> SetupOptions {
    let mut o = SetupOptions::new(dest);
    o.progress = false;
    o
}

#[test]
fn extraction_keeps_only_the_needed_tree() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("sdk.tar.gz");
    build_tarball(&archive, &sdk_files(), &[], Some((&format!("{TOP}/lib/x64/link.so"), "libMVSDK.so")));
    let out = tmp.path().join("out");
    let summary = extract_sdk_tarball(&archive, &out).unwrap();
    assert_eq!(summary.files, 5);
    assert_eq!(fs::read(out.join("lib/x64/libMVSDK.so")).unwrap(), b"x64 library bytes");
    assert!(out.join("lib/arm64/libMVSDK.so").is_file());
    assert!(out.join("include/CameraApi.h").is_file());
    assert!(out.join("tools/GeConfigCmd/x64/GeConfigCmd").is_file());
    assert!(out.join("udev/88-mvusb.rules").is_file());
    assert!(!out.join("tools/GTKGeConfigTools").exists());
    assert!(!out.join("demo").exists());
    assert!(!out.join("install.sh").exists());
    assert!(!out.join("lib/x64/link.so").exists(), "symlinks are skipped");
}

#[test]
#[cfg(unix)]
fn extracted_files_get_explicit_modes() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("sdk.tar.gz");
    build_tarball(&archive, &sdk_files(), &[], None);
    let out = tmp.path().join("out");
    extract_sdk_tarball(&archive, &out).unwrap();
    let mode = |p: &str| fs::metadata(out.join(p)).unwrap().permissions().mode() & 0o777;
    // Some file systems (for example NTFS mounts) ignore modes; only check
    // when the file system honours them.
    let probe = tmp.path().join("probe");
    fs::write(&probe, b"").unwrap();
    fs::set_permissions(&probe, fs::Permissions::from_mode(0o600)).unwrap();
    if fs::metadata(&probe).unwrap().permissions().mode() & 0o777 == 0o600 {
        assert_eq!(mode("include/CameraApi.h"), 0o644);
        assert_eq!(mode("lib/x64/libMVSDK.so"), 0o755);
        assert_eq!(mode("tools/GeConfigCmd/x64/GeConfigCmd"), 0o755);
    }
}

#[test]
fn path_traversal_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("evil.tar.gz");
    build_tarball(&archive, &sdk_files(), &[&format!("{TOP}/lib/../../../escaped")], None);
    let out = tmp.path().join("deep/out");
    let err = extract_sdk_tarball(&archive, &out).unwrap_err().to_string();
    assert!(err.contains("unsafe path"), "{err}");
    assert!(!tmp.path().join("escaped").exists());
}

#[test]
fn install_from_file_verifies_installs_and_is_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("sdk.tar.gz");
    build_tarball(&archive, &sdk_files(), &[], None);
    let manifest = manifest_for(&archive, "9.9.9.9");
    let dest = tmp.path().join("vendor");
    let o = opts(dest.clone());

    let out = install_from_file(&manifest, &archive, None, "linux", &o).unwrap();
    let version_dir = dest.join("9.9.9.9");
    match &out {
        SetupOutcome::Installed { version_dir: v, summary, .. } => {
            assert_eq!(v, &version_dir);
            assert_eq!(summary.files, 5);
        }
        other => panic!("unexpected {other:?}"),
    }
    let m = read_install_manifest(&version_dir).unwrap();
    assert_eq!(m.version, "9.9.9.9");
    assert_eq!(m.sha256, sha256_file(&archive).unwrap());
    assert_eq!(m.source_url, "http://example.invalid/x.tar.gz");
    assert!(m.source_file.is_some());
    assert!(!m.installed_at.is_empty());
    // No staging directory is left behind.
    let leftovers: Vec<_> = fs::read_dir(&dest).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(leftovers, vec![std::ffi::OsString::from("9.9.9.9")]);

    // Second run: nothing to do.
    let again = install_from_file(&manifest, &archive, None, "linux", &o).unwrap();
    assert!(matches!(again, SetupOutcome::AlreadyInstalled { .. }));

    // --force replaces the installation.
    fs::write(version_dir.join("stale"), b"x").unwrap();
    let mut forced = o.clone();
    forced.force = true;
    let out = install_from_file(&manifest, &archive, Some("9.9.9.9"), "linux", &forced).unwrap();
    assert!(matches!(out, SetupOutcome::Installed { .. }));
    assert!(!version_dir.join("stale").exists());
}

#[test]
fn foreign_directory_is_not_overwritten_without_force() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("sdk.tar.gz");
    build_tarball(&archive, &sdk_files(), &[], None);
    let manifest = manifest_for(&archive, "9.9.9.9");
    let dest = tmp.path().join("vendor");
    fs::create_dir_all(dest.join("9.9.9.9")).unwrap();
    let err = install_from_file(&manifest, &archive, None, "linux", &opts(dest)).unwrap_err().to_string();
    assert!(err.contains("--force"), "{err}");
}

#[test]
fn unknown_or_tampered_archives_are_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("sdk.tar.gz");
    build_tarball(&archive, &sdk_files(), &[], None);
    let manifest = manifest_for(&archive, "9.9.9.9");
    let dest = tmp.path().join("vendor");

    // A different file: its hash matches no release.
    let other = tmp.path().join("other.tar.gz");
    build_tarball(&other, &sdk_files()[..2], &[], None);
    let err = install_from_file(&manifest, &other, None, "linux", &opts(dest.clone())).unwrap_err().to_string();
    assert!(err.contains("does not match any release"), "{err}");

    // Naming the version explicitly does not bypass the checksum.
    let err = install_from_file(&manifest, &other, Some("9.9.9.9"), "linux", &opts(dest.clone()))
        .unwrap_err()
        .to_string();
    assert!(err.contains("bytes") || err.contains("SHA-256 mismatch"), "{err}");
    assert!(!dest.join("9.9.9.9").exists());

    // Same size, different content.
    let mut bytes = fs::read(&archive).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    let tampered = tmp.path().join("tampered.tar.gz");
    fs::write(&tampered, &bytes).unwrap();
    let release = &manifest.releases[0];
    let err = verify_archive(release, &tampered, false).unwrap_err().to_string();
    assert!(err.contains("SHA-256 mismatch"), "{err}");
    assert!(verify_archive(release, &archive, false).is_ok());
}

#[test]
fn releases_without_checksum_need_explicit_acceptance() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("sdk.tar.gz");
    build_tarball(&archive, &sdk_files(), &[], None);
    let size = fs::metadata(&archive).unwrap().len();
    let manifest = Manifest::parse(&format!(
        "[[release]]\nversion = \"9.9.9.9\"\nos = \"linux\"\nurl = \"http://example.invalid/x.tar.gz\"\n\
         size = {size}\nsha256 = \"\"\nlayout = \"tar\"\n"
    ))
    .unwrap();
    let dest = tmp.path().join("vendor");
    let err = install_from_file(&manifest, &archive, Some("9.9.9.9"), "linux", &opts(dest.clone()))
        .unwrap_err()
        .to_string();
    assert!(err.contains("--accept-unverified"), "{err}");
    let mut o = opts(dest);
    o.accept_unverified = true;
    assert!(install_from_file(&manifest, &archive, Some("9.9.9.9"), "linux", &o).is_ok());
}
