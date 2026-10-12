//! Downloader and installer for the vendor SDK.
//!
//! The crate never redistributes the SDK. This module fetches the vendor's own
//! archive from the URL recorded in `sdk-manifest.toml`, verifies its size and
//! SHA-256, and extracts the parts the wrapper needs into
//! `<dest>/<version>/` (default `$GCAM_MVSDK_HOME/<version>/`, where the SDK
//! home defaults to `~/.gcam-mvsdk`):
//!
//! ```text
//! <dest>/<version>/include/          vendor headers
//! <dest>/<version>/lib/<arch>/       libMVSDK.so for every architecture
//! <dest>/<version>/tools/GeConfigCmd vendor GigE configuration tool
//! <dest>/<version>/udev/             USB udev rules (not installed automatically)
//! <dest>/<version>/manifest.json     version, source_url, sha256, installed_at
//! ```
//!
//! On Windows the vendor ships an installer inside a `.rar`. The archive is
//! downloaded and verified, `Setup.exe` is extracted when `7z` is available,
//! and the user runs the installer (it also installs the GigE filter driver).

use std::fs::{self, File};
use std::io::{self, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::sdk::{host_arch_dir, host_os, library_file_name, sdk_home_root, TargetOs};

/// The release table compiled into this binary (`sdk-manifest.toml`).
pub const MANIFEST_TOML: &str = include_str!("../sdk-manifest.toml");
/// File written next to an installed SDK version.
pub const INSTALL_MANIFEST_FILE: &str = "manifest.json";
/// Subdirectory of the destination root that holds downloaded archives.
pub const DOWNLOADS_DIR: &str = "downloads";

// ---------------------------------------------------------------------------
// Release table
// ---------------------------------------------------------------------------

/// How a vendor archive is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Layout {
    /// `.tar.gz` with a top-level directory holding `include/`, `lib/<arch>/`, `tools/`.
    Tar,
    /// `.rar` holding a Windows installer executable.
    RarInstaller,
    /// A layout this version of the crate does not know how to install.
    #[serde(other)]
    Unknown,
}

/// One entry of `sdk-manifest.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Release {
    pub version: String,
    pub os: String,
    pub url: String,
    pub size: u64,
    /// Lowercase hex SHA-256; empty when not yet recorded.
    #[serde(default)]
    pub sha256: String,
    pub layout: Layout,
}

impl Release {
    /// Archive file name, taken from the URL and percent-decoded.
    pub fn file_name(&self) -> String {
        let last = self.url.rsplit('/').next().unwrap_or("sdk-archive");
        let decoded = percent_decode(last);
        // Keep the name usable as a single path component.
        let cleaned: String = decoded
            .chars()
            .map(|c| if matches!(c, '/' | '\\' | ':' | '\0') { '_' } else { c })
            .collect();
        if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
            "sdk-archive".to_string()
        } else {
            cleaned
        }
    }

    /// Whether a SHA-256 is recorded for this release.
    pub fn has_checksum(&self) -> bool {
        !self.sha256.is_empty()
    }
}

#[derive(Deserialize)]
struct ManifestFile {
    #[serde(default)]
    release: Vec<Release>,
}

/// Parsed `sdk-manifest.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub releases: Vec<Release>,
}

impl Manifest {
    /// The table compiled into this crate.
    pub fn embedded() -> Manifest {
        Manifest::parse(MANIFEST_TOML).expect("the embedded sdk-manifest.toml is valid")
    }

    /// Parses and validates manifest text.
    pub fn parse(text: &str) -> Result<Manifest> {
        let file: ManifestFile =
            toml::from_str(text).map_err(|e| Error::Setup(format!("invalid sdk-manifest.toml: {e}")))?;
        for r in &file.release {
            if r.version.trim().is_empty() {
                return Err(Error::Setup("sdk-manifest.toml: a release has an empty version".into()));
            }
            if !matches!(r.os.as_str(), "linux" | "windows" | "macos") {
                return Err(Error::Setup(format!(
                    "sdk-manifest.toml: release {} has unknown os {:?}",
                    r.version, r.os
                )));
            }
            if !(r.url.starts_with("http://") || r.url.starts_with("https://")) {
                return Err(Error::Setup(format!(
                    "sdk-manifest.toml: release {} has a non-HTTP url",
                    r.version
                )));
            }
            if !r.sha256.is_empty() && !is_sha256_hex(&r.sha256) {
                return Err(Error::Setup(format!(
                    "sdk-manifest.toml: release {} has a malformed sha256 (expected 64 lowercase hex digits)",
                    r.version
                )));
            }
            if r.size == 0 {
                return Err(Error::Setup(format!("sdk-manifest.toml: release {} has size 0", r.version)));
            }
        }
        Ok(Manifest { releases: file.release })
    }

    /// Newest release for an OS (`linux`, `windows`, `macos`).
    pub fn latest(&self, os: &str) -> Option<&Release> {
        self.releases
            .iter()
            .filter(|r| r.os == os)
            .max_by(|a, b| crate::sdk::compare_versions(&a.version, &b.version))
    }

    /// Release with this version for this OS.
    pub fn find(&self, version: &str, os: &str) -> Option<&Release> {
        self.releases.iter().find(|r| r.version == version && r.os == os)
    }

    /// Release whose recorded SHA-256 equals `sha256`.
    pub fn find_by_sha256(&self, sha256: &str) -> Option<&Release> {
        let sha = sha256.to_ascii_lowercase();
        self.releases.iter().find(|r| r.has_checksum() && r.sha256 == sha)
    }
}

fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

/// Decodes `%XX` escapes; malformed escapes are kept literally.
pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let hex_val = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push(h << 4 | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ---------------------------------------------------------------------------
// manifest.json written next to an installation
// ---------------------------------------------------------------------------

/// Contents of `<dest>/<version>/manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallManifest {
    pub version: String,
    pub source_url: String,
    pub sha256: String,
    /// RFC 3339 UTC time stamp.
    pub installed_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
    /// Local archive used with `setup --from`, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_file: Option<String>,
    /// Tool and version that performed the installation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installer: Option<String>,
}

/// Reads `<version_dir>/manifest.json`.
pub fn read_install_manifest(version_dir: &Path) -> Result<InstallManifest> {
    let path = version_dir.join(INSTALL_MANIFEST_FILE);
    let text = fs::read_to_string(&path).map_err(|e| Error::io(format!("reading {}", path.display()), e))?;
    serde_json::from_str(&text).map_err(|e| Error::Setup(format!("invalid {}: {e}", path.display())))
}

/// Writes `<version_dir>/manifest.json`.
pub fn write_install_manifest(version_dir: &Path, m: &InstallManifest) -> Result<()> {
    let path = version_dir.join(INSTALL_MANIFEST_FILE);
    let text = serde_json::to_string_pretty(m).expect("manifest serializes");
    fs::write(&path, text + "\n").map_err(|e| Error::io(format!("writing {}", path.display()), e))
}

/// Records the location of an installed library in `<root>/manifest.json`
/// (used on Windows, where the vendor installer chooses the directory).
pub fn record_library(root: &Path, library: &Path) -> Result<()> {
    fs::create_dir_all(root).map_err(|e| Error::io(format!("creating {}", root.display()), e))?;
    let path = root.join(INSTALL_MANIFEST_FILE);
    let mut value = fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}));
    value["library"] = serde_json::Value::String(library.display().to_string());
    value["recorded_at"] = serde_json::Value::String(now_rfc3339());
    let text = serde_json::to_string_pretty(&value).expect("json serializes");
    fs::write(&path, text + "\n").map_err(|e| Error::io(format!("writing {}", path.display()), e))
}

fn now_rfc3339() -> String {
    humantime::format_rfc3339_seconds(SystemTime::now()).to_string()
}

fn installer_tag() -> String {
    concat!("gcam-mvsdk-rs ", env!("CARGO_PKG_VERSION")).to_string()
}

// ---------------------------------------------------------------------------
// Hashing and verification
// ---------------------------------------------------------------------------

/// Lowercase hex SHA-256 of a file.
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut f = File::open(path).map_err(|e| Error::io(format!("opening {}", path.display()), e))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = f.read(&mut buf).map_err(|e| Error::io(format!("reading {}", path.display()), e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Checks an archive against a release: size always, SHA-256 when recorded.
///
/// A missing checksum is accepted only with `accept_unverified`, because the
/// vendor serves files over plain HTTP. Returns the computed SHA-256.
pub fn verify_archive(release: &Release, archive: &Path, accept_unverified: bool) -> Result<String> {
    let size = fs::metadata(archive)
        .map_err(|e| Error::io(format!("reading {}", archive.display()), e))?
        .len();
    if size != release.size {
        return Err(Error::Setup(format!(
            "{} is {size} bytes, but release {} ({}) is recorded as {} bytes; refusing to install",
            archive.display(),
            release.version,
            release.os,
            release.size
        )));
    }
    let sha = sha256_file(archive)?;
    check_checksum(release, &sha, accept_unverified)?;
    Ok(sha)
}

fn check_checksum(release: &Release, sha: &str, accept_unverified: bool) -> Result<()> {
    if release.has_checksum() {
        if sha != release.sha256 {
            return Err(Error::Setup(format!(
                "SHA-256 mismatch for release {} ({}): expected {}, got {sha}; refusing to install",
                release.version, release.os, release.sha256
            )));
        }
    } else if !accept_unverified {
        return Err(Error::Setup(format!(
            "no SHA-256 is recorded for release {} ({}) in sdk-manifest.toml.\n\
             The downloaded file has SHA-256 {sha}.\n\
             Verify it independently, then either add it to sdk-manifest.toml or rerun with --accept-unverified.",
            release.version, release.os
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Download
// ---------------------------------------------------------------------------

struct Progress {
    total: u64,
    done: u64,
    started: Instant,
    last: Instant,
    tty: bool,
    enabled: bool,
    last_decile: u64,
}

impl Progress {
    fn new(total: u64, enabled: bool) -> Self {
        use std::io::IsTerminal;
        let now = Instant::now();
        Progress {
            total,
            done: 0,
            started: now,
            last: now,
            tty: io::stderr().is_terminal(),
            enabled,
            last_decile: 0,
        }
    }

    fn advance(&mut self, n: u64) {
        self.done += n;
        if !self.enabled {
            return;
        }
        let pct = (self.done * 100).checked_div(self.total).unwrap_or(0);
        if self.tty {
            if self.last.elapsed() >= Duration::from_millis(200) || self.done >= self.total {
                self.last = Instant::now();
                eprint!("\r  {}", self.line(pct));
                let _ = io::stderr().flush();
            }
        } else if pct / 10 > self.last_decile {
            self.last_decile = pct / 10;
            eprintln!("  {}", self.line(pct));
        }
    }

    fn line(&self, pct: u64) -> String {
        let mib = |b: u64| b as f64 / (1024.0 * 1024.0);
        let secs = self.started.elapsed().as_secs_f64().max(0.001);
        format!(
            "{:6.1} / {:.1} MiB ({pct:3}%)  {:5.2} MiB/s",
            mib(self.done),
            mib(self.total),
            mib(self.done) / secs
        )
    }

    fn finish(&self) {
        if self.enabled && self.tty {
            eprintln!();
        }
    }
}

fn http_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .timeout_recv_body(Some(Duration::from_secs(60 * 60)))
        .user_agent(concat!("gcam-mvsdk-rs/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn url_changed_hint(release: &Release) -> String {
    format!(
        "The vendor may have published a new version, which changes the download URL.\n\
         Download the SDK by hand from http://huatengvision.com/download/1/ and run\n\
         `gcam-mvsdk-rs setup --from <file>`; if it is a new version, add it to sdk-manifest.toml.\n\
         Recorded URL: {}",
        release.url
    )
}

/// Sends `HEAD` to the recorded URL and checks for `200` and the recorded
/// `Content-Length`.
pub fn head_check(release: &Release) -> Result<()> {
    let resp = http_agent()
        .head(&release.url)
        .call()
        .map_err(|e| Error::Setup(format!("HEAD request failed: {e}\n{}", url_changed_hint(release))))?;
    if resp.status().as_u16() != 200 {
        return Err(Error::Setup(format!(
            "HEAD returned status {} instead of 200.\n{}",
            resp.status(),
            url_changed_hint(release)
        )));
    }
    let len = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok());
    match len {
        Some(n) if n == release.size => Ok(()),
        Some(n) => Err(Error::Setup(format!(
            "the server reports {n} bytes, but release {} is recorded as {} bytes.\n{}",
            release.version,
            release.size,
            url_changed_hint(release)
        ))),
        None => Err(Error::Setup(format!(
            "the server did not report a Content-Length.\n{}",
            url_changed_hint(release)
        ))),
    }
}

/// Downloads a release to `dest_file`, computing the SHA-256 while streaming.
/// The file appears under its final name only when the size matches and, if
/// a checksum is recorded, the SHA-256 matches too. Returns the SHA-256; the
/// caller decides whether a release without a recorded checksum is acceptable.
pub fn download(release: &Release, dest_file: &Path, progress: bool) -> Result<String> {
    if let Some(parent) = dest_file.parent() {
        fs::create_dir_all(parent).map_err(|e| Error::io(format!("creating {}", parent.display()), e))?;
    }
    let part = dest_file.with_extension(format!(
        "{}part",
        dest_file.extension().map(|e| format!("{}.", e.to_string_lossy())).unwrap_or_default()
    ));
    let result = (|| -> Result<String> {
        let resp = http_agent()
            .get(&release.url)
            .call()
            .map_err(|e| Error::Setup(format!("GET request failed: {e}\n{}", url_changed_hint(release))))?;
        let mut reader = resp.into_body().into_reader();
        let mut out = File::create(&part).map_err(|e| Error::io(format!("creating {}", part.display()), e))?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 256 * 1024];
        let mut bar = Progress::new(release.size, progress);
        loop {
            let n = reader
                .read(&mut buf)
                .map_err(|e| Error::io(format!("downloading {}", release.url), e))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            out.write_all(&buf[..n])
                .map_err(|e| Error::io(format!("writing {}", part.display()), e))?;
            bar.advance(n as u64);
            if bar.done > release.size {
                bar.finish();
                return Err(Error::Setup(format!(
                    "the server sent more than the recorded {} bytes; aborting",
                    release.size
                )));
            }
        }
        bar.finish();
        out.sync_all().map_err(|e| Error::io(format!("writing {}", part.display()), e))?;
        if bar.done != release.size {
            return Err(Error::Setup(format!(
                "download ended after {} of {} bytes",
                bar.done, release.size
            )));
        }
        let sha = hex(&hasher.finalize());
        if release.has_checksum() {
            check_checksum(release, &sha, false)?;
        }
        Ok(sha)
    })();
    match result {
        Ok(sha) => {
            fs::rename(&part, dest_file).map_err(|e| Error::io(format!("renaming {}", part.display()), e))?;
            Ok(sha)
        }
        Err(e) => {
            let _ = fs::remove_file(&part);
            Err(e)
        }
    }
}

// ---------------------------------------------------------------------------
// Extraction
// ---------------------------------------------------------------------------

/// Statistics from [`extract_sdk_tarball`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtractSummary {
    /// Number of files written.
    pub files: usize,
    /// Number of bytes written.
    pub bytes: u64,
}

/// Maps a path inside the vendor tarball to a destination path relative to
/// the version directory, or `None` for entries that are not installed.
///
/// Kept: `include/`, `lib/`, `tools/GeConfigCmd/`, and the two USB udev rule
/// files (placed under `udev/`). The vendor's top-level directory is
/// stripped. Paths with `..`, absolute paths, or drive prefixes are errors.
pub fn select_sdk_entry(path: &Path) -> Result<Option<PathBuf>> {
    let mut parts: Vec<String> = Vec::new();
    for c in path.components() {
        match c {
            Component::Normal(s) => parts.push(s.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(Error::Setup(format!(
                    "the archive contains an unsafe path {:?}; refusing to extract",
                    path
                )));
            }
        }
    }
    if parts.is_empty() {
        return Ok(None);
    }
    // Strip the vendor's top-level directory unless the archive was repacked
    // with the SDK tree at its root.
    let rel: &[String] = if matches!(parts[0].as_str(), "include" | "lib" | "tools") {
        &parts[..]
    } else {
        &parts[1..]
    };
    let Some(first) = rel.first() else {
        return Ok(None);
    };
    let keep = match first.as_str() {
        "include" | "lib" => true,
        "tools" => rel.get(1).map(|s| s == "GeConfigCmd").unwrap_or(false),
        name if rel.len() == 1 && name.ends_with(".rules") => {
            return Ok(Some(Path::new("udev").join(name)));
        }
        _ => false,
    };
    Ok(keep.then(|| rel.iter().collect()))
}

/// Extracts the parts of a vendor `.tar.gz` that the wrapper uses into
/// `target` (created if needed). Symbolic links are skipped.
pub fn extract_sdk_tarball(archive: &Path, target: &Path) -> Result<ExtractSummary> {
    let file = File::open(archive).map_err(|e| Error::io(format!("opening {}", archive.display()), e))?;
    let gz = flate2::read::GzDecoder::new(BufReader::new(file));
    let mut tar = tar::Archive::new(gz);
    fs::create_dir_all(target).map_err(|e| Error::io(format!("creating {}", target.display()), e))?;
    let mut summary = ExtractSummary::default();
    let bad = |e: io::Error| Error::io(format!("reading {}", archive.display()), e);
    for entry in tar.entries().map_err(bad)? {
        let mut entry = entry.map_err(bad)?;
        let path = entry.path().map_err(bad)?.into_owned();
        let Some(rel) = select_sdk_entry(&path)? else {
            continue;
        };
        let dest = target.join(&rel);
        match entry.header().entry_type() {
            tar::EntryType::Directory => {
                fs::create_dir_all(&dest).map_err(|e| Error::io(format!("creating {}", dest.display()), e))?;
            }
            tar::EntryType::Regular | tar::EntryType::Continuous => {
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| Error::io(format!("creating {}", parent.display()), e))?;
                }
                let mut out =
                    File::create(&dest).map_err(|e| Error::io(format!("creating {}", dest.display()), e))?;
                let n = io::copy(&mut entry, &mut out).map_err(|e| Error::io(format!("writing {}", dest.display()), e))?;
                set_mode(&dest, executable_entry(&rel))?;
                summary.files += 1;
                summary.bytes += n;
            }
            _ => {}
        }
    }
    Ok(summary)
}

fn executable_entry(rel: &Path) -> bool {
    rel.starts_with("lib") || rel.starts_with("tools")
}

#[cfg(unix)]
fn set_mode(path: &Path, executable: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = if executable { 0o755 } else { 0o644 };
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|e| Error::io(format!("setting permissions on {}", path.display()), e))
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _executable: bool) -> Result<()> {
    Ok(())
}

// ---------------------------------------------------------------------------
// Installation
// ---------------------------------------------------------------------------

/// Options shared by every setup path.
#[derive(Debug, Clone)]
pub struct SetupOptions {
    /// Root directory; the SDK goes to `<dest_root>/<version>/`.
    pub dest_root: PathBuf,
    /// Replace an existing installation of the same version.
    pub force: bool,
    /// Keep the downloaded archive in `<dest_root>/downloads/`.
    pub keep_archive: bool,
    /// Allow installing a release that has no recorded SHA-256.
    pub accept_unverified: bool,
    /// Show download progress on stderr.
    pub progress: bool,
}

impl SetupOptions {
    /// Default options for installing into `dest_root`.
    pub fn new(dest_root: PathBuf) -> Self {
        SetupOptions { dest_root, force: false, keep_archive: false, accept_unverified: false, progress: true }
    }
}

/// The default destination: the SDK home, `$GCAM_MVSDK_HOME`, else `~/.gcam-mvsdk`.
pub fn default_dest_root() -> Result<PathBuf> {
    sdk_home_root().ok_or_else(|| {
        Error::Setup("cannot determine the home directory; set GCAM_MVSDK_HOME or pass --dest".into())
    })
}

/// What a setup run achieved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupOutcome {
    /// The SDK was extracted into `version_dir`.
    Installed {
        version_dir: PathBuf,
        /// The library for this host, if the archive contained one.
        library: Option<PathBuf>,
        summary: ExtractSummary,
    },
    /// The same release was already installed; nothing changed.
    AlreadyInstalled { version_dir: PathBuf, library: Option<PathBuf> },
    /// Windows: the installer archive is verified; the user must run the installer.
    WindowsInstallerReady {
        archive: PathBuf,
        /// Extracted installer executable, when `7z` was available.
        installer: Option<PathBuf>,
        /// An already installed DLL found under Program Files, if any.
        installed_library: Option<PathBuf>,
        /// Text to show the user.
        instructions: String,
    },
}

/// Library path for this host inside an installed version directory.
pub fn host_library_in(version_dir: &Path) -> Option<PathBuf> {
    let arch = host_arch_dir()?;
    let name = library_file_name(host_os(), arch)?;
    let p = version_dir.join("lib").join(arch).join(name);
    p.is_file().then_some(p)
}

fn already_installed(release: &Release, version_dir: &Path) -> bool {
    match read_install_manifest(version_dir) {
        Ok(m) => m.version == release.version && release.has_checksum() && m.sha256 == release.sha256,
        Err(_) => false,
    }
}

/// Installs a release from the network: `HEAD` check, download with
/// verification (or reuse of a verified archive in `downloads/`), extraction.
pub fn install_from_network(release: &Release, opts: &SetupOptions) -> Result<SetupOutcome> {
    let version_dir = opts.dest_root.join(&release.version);
    if release.layout == Layout::Tar && !opts.force && already_installed(release, &version_dir) {
        return Ok(SetupOutcome::AlreadyInstalled { library: host_library_in(&version_dir), version_dir });
    }
    let archive = opts.dest_root.join(DOWNLOADS_DIR).join(release.file_name());
    // A previous download is reused when its size (and recorded checksum, if
    // any) still match. A release without a recorded checksum keeps its
    // download, so a rerun with --accept-unverified does not fetch it again.
    let reused = if archive.is_file() { verify_archive(release, &archive, true).ok() } else { None };
    let sha = match reused {
        Some(sha) => {
            eprintln!("Using previously downloaded {}", archive.display());
            sha
        }
        None => {
            eprintln!("Checking {}", release.url);
            head_check(release)?;
            eprintln!("Downloading {} ({} bytes)", release.file_name(), release.size);
            download(release, &archive, opts.progress)?
        }
    };
    check_checksum(release, &sha, opts.accept_unverified)?;
    if release.has_checksum() {
        eprintln!("SHA-256 verified: {sha}");
    }
    let outcome = install_archive(release, &archive, None, opts)?;
    if release.layout == Layout::Tar && !opts.keep_archive {
        let _ = fs::remove_file(&archive);
        let _ = fs::remove_dir(opts.dest_root.join(DOWNLOADS_DIR));
    }
    Ok(outcome)
}

/// Installs from a local archive (`setup --from`). The release is identified
/// by `version` when given, otherwise by the archive's SHA-256.
pub fn install_from_file(
    manifest: &Manifest,
    file: &Path,
    version: Option<&str>,
    os: &str,
    opts: &SetupOptions,
) -> Result<SetupOutcome> {
    if !file.is_file() {
        return Err(Error::Setup(format!("{} is not a file", file.display())));
    }
    let sha = sha256_file(file)?;
    let release = match version {
        Some(v) => manifest
            .find(v, os)
            .or_else(|| manifest.releases.iter().find(|r| r.version == v))
            .ok_or_else(|| Error::Setup(format!("version {v} is not listed in sdk-manifest.toml")))?,
        None => manifest.find_by_sha256(&sha).ok_or_else(|| {
            let known: Vec<String> = manifest
                .releases
                .iter()
                .filter(|r| r.has_checksum())
                .map(|r| format!("  {} {} {}", r.os, r.version, r.sha256))
                .collect();
            Error::Setup(format!(
                "{} (SHA-256 {sha}) does not match any release in sdk-manifest.toml.\nKnown releases:\n{}\n\
                 Pass --version to name the release explicitly.",
                file.display(),
                known.join("\n")
            ))
        })?,
    };
    let size = fs::metadata(file).map_err(|e| Error::io(format!("reading {}", file.display()), e))?.len();
    if size != release.size {
        return Err(Error::Setup(format!(
            "{} is {size} bytes, but release {} ({}) is recorded as {} bytes; refusing to install",
            file.display(),
            release.version,
            release.os,
            release.size
        )));
    }
    check_checksum(release, &sha, opts.accept_unverified)?;
    eprintln!("Verified {} as release {} ({}), SHA-256 {sha}", file.display(), release.version, release.os);
    install_archive(release, file, Some(file), opts)
}

/// Installs an archive that has already been verified against `release`.
pub fn install_archive(
    release: &Release,
    archive: &Path,
    source_file: Option<&Path>,
    opts: &SetupOptions,
) -> Result<SetupOutcome> {
    match release.layout {
        Layout::Tar => install_tarball(release, archive, source_file, opts),
        Layout::RarInstaller => prepare_windows_installer(release, archive, opts),
        Layout::Unknown => Err(Error::Setup(format!(
            "release {} uses an archive layout this version of gcam-mvsdk-rs cannot install",
            release.version
        ))),
    }
}

fn install_tarball(
    release: &Release,
    archive: &Path,
    source_file: Option<&Path>,
    opts: &SetupOptions,
) -> Result<SetupOutcome> {
    let root = &opts.dest_root;
    fs::create_dir_all(root).map_err(|e| Error::io(format!("creating {}", root.display()), e))?;
    let version_dir = root.join(&release.version);
    if version_dir.exists() && !opts.force {
        if already_installed(release, &version_dir) {
            return Ok(SetupOutcome::AlreadyInstalled { library: host_library_in(&version_dir), version_dir });
        }
        return Err(Error::Setup(format!(
            "{} already exists; pass --force to replace it",
            version_dir.display()
        )));
    }
    let staging = root.join(format!(".{}.staging-{}", release.version, std::process::id()));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|e| Error::io(format!("removing {}", staging.display()), e))?;
    }
    let result = (|| -> Result<ExtractSummary> {
        eprintln!("Extracting into {}", version_dir.display());
        let summary = extract_sdk_tarball(archive, &staging)?;
        if !staging.join("lib").is_dir() {
            return Err(Error::Setup(format!("{} contains no lib/ directory", archive.display())));
        }
        write_install_manifest(
            &staging,
            &InstallManifest {
                version: release.version.clone(),
                source_url: release.url.clone(),
                sha256: release.sha256.clone(),
                installed_at: now_rfc3339(),
                os: Some(release.os.clone()),
                source_file: source_file.map(|p| p.display().to_string()),
                installer: Some(installer_tag()),
            },
        )?;
        if version_dir.exists() {
            fs::remove_dir_all(&version_dir)
                .map_err(|e| Error::io(format!("removing {}", version_dir.display()), e))?;
        }
        fs::rename(&staging, &version_dir)
            .map_err(|e| Error::io(format!("moving {} into place", staging.display()), e))?;
        Ok(summary)
    })();
    match result {
        Ok(summary) => Ok(SetupOutcome::Installed { library: host_library_in(&version_dir), version_dir, summary }),
        Err(e) => {
            let _ = fs::remove_dir_all(&staging);
            Err(e)
        }
    }
}

/// Finds an executable on `PATH`.
pub fn find_in_path(names: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for name in names {
            let p = dir.join(name);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn find_7z() -> Option<PathBuf> {
    find_in_path(&["7z", "7z.exe", "7za", "7za.exe", "7zz"]).or_else(|| {
        windows_program_dirs()
            .into_iter()
            .map(|d| d.join("7-Zip").join("7z.exe"))
            .find(|p| p.is_file())
    })
}

/// `Program Files` directories from the environment (Windows only; empty
/// elsewhere).
pub fn windows_program_dirs() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Some(v) = std::env::var_os(var) {
            let p = PathBuf::from(v);
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// Searches `roots` for `file_name`, descending at most `max_depth` levels and
/// only into first-level directories whose names suggest the vendor
/// (`HuaTeng`, `MindVision`, `MVSDK`, `Camera`).
pub fn locate_library_under(roots: &[PathBuf], file_name: &str, max_depth: usize) -> Option<PathBuf> {
    fn walk(dir: &Path, file_name: &str, depth: usize) -> Option<PathBuf> {
        let entries = fs::read_dir(dir).ok()?;
        let mut subdirs = Vec::new();
        for e in entries.flatten() {
            let p = e.path();
            let Ok(t) = e.file_type() else { continue };
            if t.is_file() && e.file_name().to_string_lossy().eq_ignore_ascii_case(file_name) {
                return Some(p);
            }
            if t.is_dir() {
                subdirs.push(p);
            }
        }
        if depth == 0 {
            return None;
        }
        subdirs.sort();
        subdirs.into_iter().find_map(|d| walk(&d, file_name, depth - 1))
    }
    for root in roots {
        let Ok(entries) = fs::read_dir(root) else { continue };
        let mut vendor_dirs: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .filter(|e| {
                let n = e.file_name().to_string_lossy().to_ascii_lowercase();
                ["huateng", "mindvision", "mvsdk", "camera"].iter().any(|k| n.contains(k))
            })
            .map(|e| e.path())
            .collect();
        vendor_dirs.sort();
        if let Some(found) = vendor_dirs.iter().find_map(|d| walk(d, file_name, max_depth.saturating_sub(1))) {
            return Some(found);
        }
    }
    None
}

/// Looks for an installed vendor DLL under the Program Files directories.
pub fn locate_installed_windows_library() -> Option<PathBuf> {
    let arch = host_arch_dir().unwrap_or("x64");
    let name = library_file_name(TargetOs::Windows, arch)?;
    locate_library_under(&windows_program_dirs(), name, 5)
}

fn prepare_windows_installer(release: &Release, archive: &Path, opts: &SetupOptions) -> Result<SetupOutcome> {
    let version_dir = opts.dest_root.join(&release.version);
    let installer_dir = version_dir.join("installer");
    let mut installer = None;
    let mut lines = Vec::new();
    match find_7z() {
        Some(seven) => {
            fs::create_dir_all(&installer_dir)
                .map_err(|e| Error::io(format!("creating {}", installer_dir.display()), e))?;
            eprintln!("Extracting the installer with {}", seven.display());
            let status = Command::new(&seven)
                .arg("x")
                .arg("-y")
                .arg(format!("-o{}", installer_dir.display()))
                .arg(archive)
                .status()
                .map_err(|e| Error::io(format!("running {}", seven.display()), e))?;
            if !status.success() {
                return Err(Error::Setup(format!("{} failed with {status}", seven.display())));
            }
            installer = find_executable(&installer_dir, 3);
            match &installer {
                Some(exe) => lines.push(format!("Installer extracted to: {}", exe.display())),
                None => lines.push(format!("No .exe was found in {}", installer_dir.display())),
            }
        }
        None => lines.push(format!(
            "7-Zip was not found on PATH. Extract {} with 7-Zip or WinRAR.",
            archive.display()
        )),
    }
    lines.push(
        "Run the installer as Administrator. It installs the SDK DLLs together with the GigE \
         filter driver and the USB driver, which only the vendor installer can provide."
            .to_string(),
    );
    lines.push("Afterwards run `gcam-mvsdk-rs setup` or `gcam-mvsdk-rs doctor` again to locate and record the DLL.".to_string());
    let installed_library = locate_installed_windows_library();
    if let Some(dll) = &installed_library {
        record_library(&opts.dest_root, dll)?;
        lines.push(format!("An installed SDK was found and recorded: {}", dll.display()));
    }
    Ok(SetupOutcome::WindowsInstallerReady {
        archive: archive.to_path_buf(),
        installer,
        installed_library,
        instructions: lines.join("\n"),
    })
}

fn find_executable(dir: &Path, depth: usize) -> Option<PathBuf> {
    let mut entries: Vec<_> = fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).collect();
    entries.sort();
    if let Some(exe) = entries
        .iter()
        .find(|p| p.is_file() && p.extension().map(|e| e.eq_ignore_ascii_case("exe")).unwrap_or(false))
    {
        return Some(exe.clone());
    }
    if depth == 0 {
        return None;
    }
    entries.iter().filter(|p| p.is_dir()).find_map(|d| find_executable(d, depth - 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decoding() {
        assert_eq!(
            percent_decode("linuxSDK_V2.1.0.49%28202511141513%29.tar.gz"),
            "linuxSDK_V2.1.0.49(202511141513).tar.gz"
        );
        assert_eq!(percent_decode("a%20b"), "a b");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
    }

    #[test]
    fn entry_selection() {
        let top = "linuxSDK_V2.1.0.49(202511141513)";
        let sel = |p: &str| select_sdk_entry(Path::new(p)).unwrap();
        assert_eq!(sel(&format!("{top}/lib/x64/libMVSDK.so")), Some(PathBuf::from("lib/x64/libMVSDK.so")));
        assert_eq!(sel(&format!("{top}/include/CameraApi.h")), Some(PathBuf::from("include/CameraApi.h")));
        assert_eq!(
            sel(&format!("{top}/tools/GeConfigCmd/x64/GeConfigCmd")),
            Some(PathBuf::from("tools/GeConfigCmd/x64/GeConfigCmd"))
        );
        assert_eq!(sel(&format!("{top}/88-mvusb.rules")), Some(PathBuf::from("udev/88-mvusb.rules")));
        assert_eq!(sel(&format!("{top}/tools/QGeConfigTools/AppRun")), None);
        assert_eq!(sel(&format!("{top}/demo/python_demo/mvsdk.py")), None);
        assert_eq!(sel(&format!("{top}/install.sh")), None);
        assert_eq!(sel(&format!("{top}/")), None);
        assert_eq!(sel("lib/x64/libMVSDK.so"), Some(PathBuf::from("lib/x64/libMVSDK.so")));
        assert!(select_sdk_entry(Path::new("top/../../etc/passwd")).is_err());
        assert!(select_sdk_entry(Path::new("/etc/passwd")).is_err());
    }
}
