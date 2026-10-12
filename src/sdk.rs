//! Locating and loading the vendor library, device enumeration, and GigE
//! network configuration.

use std::cmp::Ordering;
use std::ffi::{c_char, c_int, CString, OsStr, OsString};
use std::fmt;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::camera::Camera;
use crate::error::{Error, LoadError, MvError, Result};
use crate::ffi::{self, tSdkCameraDevInfo, Api, CameraSdkStatus};

/// Environment variable naming an extracted SDK directory (the one that
/// contains `lib/`). When set, only that directory is searched.
pub const ENV_SDK_DIR: &str = "GCAM_MVSDK_DIR";
/// Environment variable overriding the vendor architecture directory name
/// (`x64`, `x86`, `arm64`, `arm`, `arm_softfp`).
pub const ENV_ARCH: &str = "GCAM_MVSDK_ARCH";
/// Environment variable that enables the hardware integration tests.
pub const ENV_HW_TEST: &str = "GCAM_MVSDK_HW_TEST";
/// Environment variable naming the SDK home, the directory that holds
/// `<version>/` installations. Default: `~/.gcam-mvsdk`.
pub const ENV_HOME: &str = "GCAM_MVSDK_HOME";
/// Name of the default SDK home inside the user's home directory.
pub const HOME_DIR_NAME: &str = ".gcam-mvsdk";
/// Name of the developer-checkout SDK directory inside the working directory.
pub const VENDOR_DIR_NAME: &str = "vendor";
/// Value passed to `CameraSdkInit`, as in the vendor demos.
pub const SDK_INIT_LANGUAGE: c_int = 1;
/// Maximum number of devices requested from `CameraEnumerateDevice`.
pub const MAX_DEVICES: usize = 64;

// ---------------------------------------------------------------------------
// Platform mapping
// ---------------------------------------------------------------------------

/// Operating system families the vendor ships builds for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetOs {
    Linux,
    Windows,
    MacOs,
    Other,
}

impl TargetOs {
    /// Name used in `sdk-manifest.toml` (`linux`, `windows`, `macos`).
    pub fn manifest_name(self) -> &'static str {
        match self {
            TargetOs::Linux => "linux",
            TargetOs::Windows => "windows",
            TargetOs::MacOs => "macos",
            TargetOs::Other => "other",
        }
    }
}

impl fmt::Display for TargetOs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.manifest_name())
    }
}

/// Operating system this binary was compiled for.
pub fn host_os() -> TargetOs {
    if cfg!(target_os = "linux") {
        TargetOs::Linux
    } else if cfg!(target_os = "windows") {
        TargetOs::Windows
    } else if cfg!(target_os = "macos") {
        TargetOs::MacOs
    } else {
        TargetOs::Other
    }
}

/// Maps a Rust `target_arch` (plus hard-float flag for 32-bit ARM) to the
/// vendor's `lib/<arch>` directory name.
pub fn arch_dir_for(os: TargetOs, target_arch: &str, arm_hard_float: bool) -> Option<&'static str> {
    match (os, target_arch) {
        (TargetOs::Linux | TargetOs::Windows | TargetOs::MacOs, "x86_64") => Some("x64"),
        (TargetOs::Linux | TargetOs::Windows, "x86") => Some("x86"),
        (TargetOs::Linux | TargetOs::MacOs, "aarch64") => Some("arm64"),
        (TargetOs::Linux, "arm") if arm_hard_float => Some("arm"),
        (TargetOs::Linux, "arm") => Some("arm_softfp"),
        _ => None,
    }
}

/// Vendor architecture directory for this binary, if the vendor ships one.
pub fn host_arch_dir() -> Option<&'static str> {
    let hard_float = cfg!(target_abi = "eabihf");
    arch_dir_for(host_os(), std::env::consts::ARCH, hard_float)
}

/// File name of the vendor library for an OS and architecture directory.
pub fn library_file_name(os: TargetOs, arch_dir: &str) -> Option<&'static str> {
    match os {
        TargetOs::Linux => Some("libMVSDK.so"),
        TargetOs::Windows if arch_dir == "x86" => Some("MVCAMSDK.dll"),
        TargetOs::Windows => Some("MVCAMSDK_X64.dll"),
        TargetOs::MacOs => Some("libMVSDK.dylib"),
        TargetOs::Other => None,
    }
}

fn user_home_dir() -> Option<PathBuf> {
    #[allow(deprecated)]
    let home = std::env::home_dir();
    home.filter(|h| !h.as_os_str().is_empty())
}

/// `~/.gcam-mvsdk`, the SDK home used when `GCAM_MVSDK_HOME` is not set.
pub fn default_home_root() -> Option<PathBuf> {
    user_home_dir().map(|h| h.join(HOME_DIR_NAME))
}

/// Where the SDK home directory came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeSource {
    /// `$GCAM_MVSDK_HOME`.
    Env,
    /// The default `~/.gcam-mvsdk`.
    Default,
}

impl fmt::Display for HomeSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HomeSource::Env => write!(f, "from ${ENV_HOME}"),
            HomeSource::Default => f.write_str("default"),
        }
    }
}

/// Resolves the SDK home from the value of `GCAM_MVSDK_HOME` and the user's
/// home directory, as the Python package does: the value is trimmed, an empty
/// value counts as unset, and a leading `~` is expanded. Without the variable
/// the result is `<user home>/.gcam-mvsdk`.
pub fn resolve_sdk_home(env_value: Option<&OsStr>, user_home: Option<&Path>) -> Option<(PathBuf, HomeSource)> {
    if let Some(raw) = env_value {
        let path = match raw.to_str() {
            Some(text) => {
                let text = text.trim();
                if text.is_empty() {
                    None
                } else if text == "~" {
                    user_home.map(Path::to_path_buf)
                } else if let Some(rest) = text.strip_prefix("~/") {
                    user_home.map(|h| h.join(rest))
                } else {
                    Some(PathBuf::from(text))
                }
            }
            None if raw.is_empty() => None,
            None => Some(PathBuf::from(raw)),
        };
        if let Some(p) = path {
            return Some((p, HomeSource::Env));
        }
    }
    user_home.map(|h| (h.join(HOME_DIR_NAME), HomeSource::Default))
}

/// The SDK home for this process: `$GCAM_MVSDK_HOME`, else `~/.gcam-mvsdk`.
pub fn sdk_home() -> Option<(PathBuf, HomeSource)> {
    resolve_sdk_home(std::env::var_os(ENV_HOME).as_deref(), user_home_dir().as_deref())
}

/// The SDK home directory for this process (see [`sdk_home`]).
pub fn sdk_home_root() -> Option<PathBuf> {
    sdk_home().map(|(p, _)| p)
}

/// Compares dotted version strings numerically (`2.1.0.49 < 2.1.10.195`).
/// Non-numeric components compare as text.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let mut ai = a.split(['.', '-', '_']);
    let mut bi = b.split(['.', '-', '_']);
    loop {
        match (ai.next(), bi.next()) {
            (None, None) => return Ordering::Equal,
            (Some(_), None) => return Ordering::Greater,
            (None, Some(_)) => return Ordering::Less,
            (Some(x), Some(y)) => {
                let ord = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(xn), Ok(yn)) => xn.cmp(&yn),
                    _ => x.cmp(y),
                };
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Search path
// ---------------------------------------------------------------------------

/// Inputs to the library search. [`SearchConfig::from_env`] fills it from the
/// environment; tests build it by hand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchConfig {
    /// `GCAM_MVSDK_DIR`. When set, nothing else is searched.
    pub sdk_dir: Option<PathBuf>,
    /// Vendor architecture directory (`x64`, `arm64`, ...).
    pub arch_dir: String,
    /// Library file name (`libMVSDK.so`, `MVCAMSDK_X64.dll`).
    pub library_name: String,
    /// `./vendor` of a developer checkout.
    pub vendor_root: Option<PathBuf>,
    /// SDK home: `$GCAM_MVSDK_HOME`, else `~/.gcam-mvsdk`.
    pub home_root: Option<PathBuf>,
    /// Library path recorded in `<home>/manifest.json` (used on Windows,
    /// where the vendor installer chooses the location).
    pub recorded_library: Option<PathBuf>,
    /// Whether to fall back to the system loader with the bare file name.
    pub use_system_path: bool,
}

impl SearchConfig {
    /// Builds the configuration from `GCAM_MVSDK_DIR`, `GCAM_MVSDK_ARCH`,
    /// `GCAM_MVSDK_HOME`, the working directory and the home directory.
    pub fn from_env() -> Result<SearchConfig, LoadError> {
        Self::from_lookup(|name| std::env::var_os(name), std::env::current_dir().ok(), user_home_dir())
    }

    /// Like [`SearchConfig::from_env`], with the environment, working
    /// directory and user home directory supplied by the caller (for tests and
    /// for embedding programs that manage their own configuration).
    pub fn from_lookup(
        env: impl Fn(&str) -> Option<OsString>,
        cwd: Option<PathBuf>,
        user_home: Option<PathBuf>,
    ) -> Result<SearchConfig, LoadError> {
        let os = host_os();
        let arch_override = env(ENV_ARCH).and_then(|v| v.to_str().map(|s| s.trim().to_string()));
        let arch_dir = match arch_override {
            Some(a) if !a.is_empty() => a,
            _ => host_arch_dir()
                .ok_or_else(|| {
                    LoadError::UnsupportedPlatform(format!(
                        "the vendor ships no MVSDK build for {} on {}; set {ENV_ARCH} to override",
                        std::env::consts::ARCH,
                        os
                    ))
                })?
                .to_string(),
        };
        let library_name = library_file_name(os, &arch_dir)
            .ok_or_else(|| LoadError::UnsupportedPlatform(format!("no MVSDK library name known for {os}")))?
            .to_string();
        let sdk_dir = env(ENV_SDK_DIR).filter(|v| !v.is_empty()).map(PathBuf::from);
        let vendor_root = cwd.map(|d| d.join(VENDOR_DIR_NAME));
        let home_root = resolve_sdk_home(env(ENV_HOME).as_deref(), user_home.as_deref()).map(|(p, _)| p);
        let recorded_library = home_root.as_deref().and_then(read_recorded_library);
        Ok(SearchConfig {
            sdk_dir,
            arch_dir,
            library_name,
            vendor_root,
            home_root,
            recorded_library,
            use_system_path: true,
        })
    }
}

/// Reads the `library` entry of `<root>/manifest.json`, if present.
pub fn read_recorded_library(root: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(root.join("manifest.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value.get("library")?.as_str().map(PathBuf::from)
}

/// One place the loader tries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Candidate {
    /// A concrete file path.
    Path(PathBuf),
    /// A bare file name handed to the system loader (`dlopen` / `LoadLibrary`).
    SystemName(String),
}

/// Outcome of [`resolve_library`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// What to load.
    pub candidate: Candidate,
    /// Human-readable list of locations checked before this one was chosen.
    pub searched: Vec<String>,
}

/// Version subdirectories of `root`, newest first.
pub fn version_dirs(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut dirs: Vec<(String, PathBuf)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            name.starts_with(|c: char| c.is_ascii_digit()).then(|| (name, e.path()))
        })
        .collect();
    dirs.sort_by(|a, b| compare_versions(&b.0, &a.0));
    dirs.into_iter().map(|(_, p)| p).collect()
}

/// One entry of the search order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchEntry {
    /// What would be loaded, or `None` for a location that holds no SDK
    /// version at all (listed only so error messages show it was checked).
    pub candidate: Option<Candidate>,
    /// Human-readable description for error messages.
    pub description: String,
}

/// Every location in search order.
///
/// Order: `$GCAM_MVSDK_DIR/lib/<arch>`, `$GCAM_MVSDK_DIR/lib`,
/// `$GCAM_MVSDK_DIR` (exclusively, when the variable is set); otherwise
/// `./vendor/<version>/lib/<arch>`, `<sdk home>/<version>/lib/<arch>` (newest
/// version first; the SDK home is `$GCAM_MVSDK_HOME`, else `~/.gcam-mvsdk`),
/// the recorded library path, then the system loader.
pub fn search_entries(cfg: &SearchConfig) -> Vec<SearchEntry> {
    let lib = cfg.library_name.as_str();
    let arch = cfg.arch_dir.as_str();
    let mut out = Vec::new();
    let path_entry = |p: PathBuf| SearchEntry { description: p.display().to_string(), candidate: Some(Candidate::Path(p)) };

    if let Some(dir) = &cfg.sdk_dir {
        if dir.is_file() {
            out.push(path_entry(dir.clone()));
        } else {
            out.push(path_entry(dir.join("lib").join(arch).join(lib)));
            out.push(path_entry(dir.join("lib").join(lib)));
            out.push(path_entry(dir.join(lib)));
        }
        return out;
    }

    for root in [&cfg.vendor_root, &cfg.home_root].into_iter().flatten() {
        let versions = version_dirs(root);
        if versions.is_empty() {
            out.push(SearchEntry {
                candidate: None,
                description: format!(
                    "{} (no SDK version installed)",
                    root.join("<version>").join("lib").join(arch).join(lib).display()
                ),
            });
        }
        for v in versions {
            out.push(path_entry(v.join("lib").join(arch).join(lib)));
        }
    }

    if let Some(p) = &cfg.recorded_library {
        out.push(SearchEntry {
            description: format!("{} (recorded in manifest.json)", p.display()),
            candidate: Some(Candidate::Path(p.clone())),
        });
    }

    if cfg.use_system_path {
        out.push(SearchEntry {
            candidate: Some(Candidate::SystemName(lib.to_string())),
            description: format!("{lib} via the system loader path"),
        });
    }
    out
}

/// Picks the first candidate that exists on disk, or the system loader
/// fallback. Fails with [`LoadError::NotFound`] when neither applies.
pub fn resolve_library(cfg: &SearchConfig) -> Result<Resolved, LoadError> {
    let mut searched = Vec::new();
    for entry in search_entries(cfg) {
        match entry.candidate {
            Some(Candidate::Path(p)) if p.is_file() => {
                return Ok(Resolved { candidate: Candidate::Path(p), searched });
            }
            Some(Candidate::SystemName(name)) => {
                return Ok(Resolved { candidate: Candidate::SystemName(name), searched });
            }
            _ => searched.push(entry.description),
        }
    }
    Err(LoadError::NotFound {
        library: cfg.library_name.clone(),
        searched,
        system_error: None,
    })
}

// ---------------------------------------------------------------------------
// Sdk
// ---------------------------------------------------------------------------

static LOADED: Mutex<Option<Sdk>> = Mutex::new(None);

/// Handle to the loaded and initialised SDK. Cheap to clone.
///
/// The library is loaded and `CameraSdkInit` is called at most once per
/// process; later calls to the `load*` functions return the same instance.
#[derive(Clone)]
pub struct Sdk {
    api: Arc<Api>,
}

impl fmt::Debug for Sdk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sdk").field("library", &self.api.path()).finish()
    }
}

impl Sdk {
    /// Locates the library using [`SearchConfig::from_env`], loads it, and
    /// calls `CameraSdkInit` once.
    pub fn load() -> Result<Sdk> {
        if let Some(sdk) = Self::loaded() {
            return Ok(sdk);
        }
        let cfg = SearchConfig::from_env()?;
        Self::load_with(&cfg)
    }

    /// Like [`Sdk::load`] with an explicit search configuration.
    pub fn load_with(cfg: &SearchConfig) -> Result<Sdk> {
        let mut guard = LOADED.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(sdk) = guard.as_ref() {
            return Ok(sdk.clone());
        }
        let resolved = resolve_library(cfg)?;
        let api = match &resolved.candidate {
            // SAFETY: the file was found in one of the documented SDK locations.
            Candidate::Path(p) => unsafe { Api::load(p) }?,
            Candidate::SystemName(name) => match unsafe { Api::load(name) } {
                Ok(api) => api,
                Err(LoadError::Open { source, .. }) => {
                    return Err(LoadError::NotFound {
                        library: cfg.library_name.clone(),
                        searched: resolved.searched,
                        system_error: Some(source.to_string()),
                    }
                    .into())
                }
                Err(e) => return Err(e.into()),
            },
        };
        let sdk = Sdk::init(api)?;
        *guard = Some(sdk.clone());
        Ok(sdk)
    }

    /// Loads the library from an explicit path.
    pub fn load_from(path: impl AsRef<Path>) -> Result<Sdk> {
        let path = path.as_ref();
        let mut guard = LOADED.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(sdk) = guard.as_ref() {
            if sdk.library_path() == path {
                return Ok(sdk.clone());
            }
            return Err(LoadError::AlreadyLoaded {
                loaded: sdk.library_path().to_path_buf(),
                requested: path.to_path_buf(),
            }
            .into());
        }
        // SAFETY: the caller chose this file as the MVSDK library.
        let api = unsafe { Api::load(path) }?;
        let sdk = Sdk::init(api)?;
        *guard = Some(sdk.clone());
        Ok(sdk)
    }

    /// The instance loaded earlier in this process, if any.
    pub fn loaded() -> Option<Sdk> {
        LOADED.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn init(api: Api) -> Result<Sdk> {
        let sdk = Sdk { api: Arc::new(api) };
        // SAFETY: plain integer argument; called once per process (guarded by LOADED).
        let status = unsafe { (sdk.api.CameraSdkInit)(SDK_INIT_LANGUAGE) };
        sdk.check("CameraSdkInit", status)?;
        Ok(sdk)
    }

    /// Raw function table, for SDK functions without a safe wrapper.
    pub fn api(&self) -> &Api {
        &self.api
    }

    /// Path (or bare name) the library was loaded from.
    pub fn library_path(&self) -> &Path {
        self.api.path()
    }

    /// SDK version string reported by the library, such as `2.1.0.49`.
    pub fn version(&self) -> Option<String> {
        let f = self.api.CameraSdkGetVersionString?;
        let mut buf = [0 as c_char; 256];
        // SAFETY: the SDK requires a buffer larger than 32 bytes.
        let status = unsafe { f(buf.as_mut_ptr()) };
        (status == ffi::CAMERA_STATUS_SUCCESS).then(|| ffi::decode_c_chars(&buf))
    }

    /// Text for a status code from `CameraGetErrorString`, falling back to the
    /// built-in table.
    pub fn error_text(&self, status: CameraSdkStatus) -> String {
        // SAFETY: returns a pointer to a static string or null.
        let text = unsafe { ffi::decode_c_str((self.api.CameraGetErrorString)(status)) };
        match text {
            Some(t) if !t.trim().is_empty() => t.trim().to_string(),
            _ => MvError::new("", status).message,
        }
    }

    /// Converts a status code into `Ok(())` or an [`MvError`].
    pub fn check(&self, function: &'static str, status: CameraSdkStatus) -> Result<()> {
        if status == ffi::CAMERA_STATUS_SUCCESS {
            Ok(())
        } else {
            Err(MvError { status, function, message: self.error_text(status) }.into())
        }
    }

    /// Directs the per-camera data files the SDK writes on `CameraInit`
    /// (`Camera/Data/<serial>.mvdat`, parameter files) to `dir` instead of the
    /// working directory. Call before opening cameras.
    pub fn set_data_directory(&self, dir: &Path) -> Result<()> {
        let f = self
            .api
            .CameraSetDataDirectory
            .ok_or(Error::Unsupported("CameraSetDataDirectory"))?;
        let c = path_to_cstring(dir)?;
        // SAFETY: NUL-terminated path string valid for the call.
        let status = unsafe { f(c.as_ptr()) };
        self.check("CameraSetDataDirectory", status)
    }

    /// Lists every camera the SDK can see (GigE discovery is a broadcast on
    /// each host NIC; USB devices are listed too).
    pub fn enumerate(&self) -> Result<Vec<DeviceInfo>> {
        let mut list = vec![tSdkCameraDevInfo::default(); MAX_DEVICES];
        let mut count: c_int = MAX_DEVICES as c_int;
        // SAFETY: `count` holds the capacity of `list` on entry.
        let status = unsafe { (self.api.CameraEnumerateDevice)(list.as_mut_ptr(), &mut count) };
        if status == ffi::CAMERA_STATUS_NO_DEVICE_FOUND {
            return Ok(Vec::new());
        }
        self.check("CameraEnumerateDevice", status)?;
        let n = (count.max(0) as usize).min(MAX_DEVICES);
        Ok(list[..n].iter().map(|raw| DeviceInfo::from_raw(self.clone(), *raw)).collect())
    }

    /// Enumerates GigE cameras by unicast to the given addresses, for cameras
    /// that are not on the same L2 segment as a host NIC.
    pub fn enumerate_gige_at(&self, addresses: &[Ipv4Addr]) -> Result<Vec<DeviceInfo>> {
        let f = self
            .api
            .CameraGigeEnumerateDevice
            .ok_or(Error::Unsupported("CameraGigeEnumerateDevice"))?;
        let strings: Vec<CString> = addresses
            .iter()
            .map(|a| CString::new(a.to_string()).expect("IPv4 text has no NUL"))
            .collect();
        let ptrs: Vec<*const c_char> = strings.iter().map(|s| s.as_ptr()).collect();
        let mut list = vec![tSdkCameraDevInfo::default(); MAX_DEVICES];
        let mut count: c_int = MAX_DEVICES as c_int;
        // SAFETY: pointer array and strings outlive the call; `count` is the capacity.
        let status = unsafe { f(ptrs.as_ptr(), ptrs.len() as c_int, list.as_mut_ptr(), &mut count) };
        if status == ffi::CAMERA_STATUS_NO_DEVICE_FOUND {
            return Ok(Vec::new());
        }
        self.check("CameraGigeEnumerateDevice", status)?;
        let n = (count.max(0) as usize).min(MAX_DEVICES);
        Ok(list[..n].iter().map(|raw| DeviceInfo::from_raw(self.clone(), *raw)).collect())
    }

    /// Assigns a GigE camera's IP configuration (GigE Vision FORCEIP), which
    /// works even when the camera is on a different subnet than the host.
    ///
    /// With `persistent = true` the address is stored in the camera as a
    /// static IP. With `persistent = false` the camera is switched to
    /// automatic addressing and the address arguments are ignored.
    pub fn gige_set_ip(
        &self,
        dev: &DeviceInfo,
        ip: Ipv4Addr,
        mask: Ipv4Addr,
        gateway: Ipv4Addr,
        persistent: bool,
    ) -> Result<()> {
        let ip = CString::new(ip.to_string()).expect("no NUL");
        let mask = CString::new(mask.to_string()).expect("no NUL");
        let gw = CString::new(gateway.to_string()).expect("no NUL");
        let mut raw = dev.raw;
        // SAFETY: device record copied from enumeration; strings outlive the call.
        let status = unsafe {
            (self.api.CameraGigeSetIp)(&mut raw, ip.as_ptr(), mask.as_ptr(), gw.as_ptr(), c_int::from(persistent))
        };
        self.check("CameraGigeSetIp", status)
    }

    /// Sets a process-wide GigE transport option, for example
    /// `("NumBuffers", "8")`.
    pub fn gige_set_option(&self, name: &str, value: &str) -> Result<()> {
        let f = self
            .api
            .CameraGigeSetOption
            .ok_or(Error::Unsupported("CameraGigeSetOption"))?;
        let n = CString::new(name).map_err(|_| Error::InvalidArgument("option name contains NUL".into()))?;
        let v = CString::new(value).map_err(|_| Error::InvalidArgument("option value contains NUL".into()))?;
        // SAFETY: both strings are NUL-terminated and outlive the call.
        let status = unsafe { f(n.as_ptr(), v.as_ptr()) };
        self.check("CameraGigeSetOption", status)
    }

    /// Whether any process currently has the device open.
    pub fn is_opened(&self, dev: &DeviceInfo) -> Result<bool> {
        let f = self.api.CameraIsOpened.ok_or(Error::Unsupported("CameraIsOpened"))?;
        let mut raw = dev.raw;
        let mut opened: c_int = 0;
        // SAFETY: valid device record and out pointer.
        let status = unsafe { f(&mut raw, &mut opened) };
        self.check("CameraIsOpened", status)?;
        Ok(opened != 0)
    }

    /// Opens a camera. Equivalent to [`Camera::open`].
    pub fn open(&self, dev: &DeviceInfo) -> Result<Camera> {
        Camera::open(self, dev)
    }

    /// Finds one camera by serial number, enumerated name, or user-set name.
    ///
    /// Serial numbers and enumerated names are matched without opening any
    /// camera. If neither matches, each camera is opened briefly to read its
    /// user-set name (`CameraGetFriendlyName`), as `camera_info.py` does.
    pub fn find_device(&self, query: &str) -> Result<DeviceInfo> {
        let devices = self.enumerate()?;
        if let Some(d) = devices.iter().find(|d| d.serial == query) {
            return Ok(d.clone());
        }
        let by_name: Vec<&DeviceInfo> = devices.iter().filter(|d| d.friendly_name == query).collect();
        if by_name.len() == 1 {
            return Ok(by_name[0].clone());
        }
        let mut available = Vec::new();
        for d in &devices {
            let user = Camera::open(self, d).and_then(|c| c.user_name());
            match user {
                Ok(name) if name == query => return Ok(d.clone()),
                Ok(name) => available.push(format!("{name:?} (SN {})", d.serial)),
                Err(e) => available.push(format!("<could not open: {e}> (SN {})", d.serial)),
            }
        }
        Err(Error::DeviceNotFound { query: query.to_string(), available })
    }
}

#[cfg(unix)]
fn path_to_cstring(path: &Path) -> Result<CString> {
    use std::os::unix::ffi::OsStrExt;
    CString::new(path.as_os_str().as_bytes()).map_err(|_| Error::InvalidArgument("path contains NUL".into()))
}

#[cfg(not(unix))]
fn path_to_cstring(path: &Path) -> Result<CString> {
    let s = path
        .to_str()
        .ok_or_else(|| Error::InvalidArgument(format!("path is not valid Unicode: {}", path.display())))?;
    let bytes = ffi::encode_sdk_text(s)
        .ok_or_else(|| Error::InvalidArgument(format!("path cannot be passed to the SDK: {s}")))?;
    CString::new(bytes).map_err(|_| Error::InvalidArgument("path contains NUL".into()))
}

// ---------------------------------------------------------------------------
// DeviceInfo
// ---------------------------------------------------------------------------

/// One enumerated device. Text fields are decoded as ASCII with GBK fallback.
#[derive(Clone)]
pub struct DeviceInfo {
    /// Product series.
    pub product_series: String,
    /// Internal product name.
    pub product_name: String,
    /// Name reported by enumeration (`acFriendlyName`). The vendor describes it
    /// as the device nickname; it follows the user-set name after the camera
    /// restarts.
    pub friendly_name: String,
    /// Kernel link name (internal use).
    pub link_name: String,
    /// Driver version.
    pub driver_version: String,
    /// Sensor type.
    pub sensor_type: String,
    /// Interface type, for example a GigE or USB description.
    pub port_type: String,
    /// Factory serial number (unique per camera).
    pub serial: String,
    /// Instance index among cameras of the same model on this host.
    pub instance: u32,
    raw: tSdkCameraDevInfo,
    sdk: Sdk,
}

impl fmt::Debug for DeviceInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceInfo")
            .field("product_series", &self.product_series)
            .field("product_name", &self.product_name)
            .field("friendly_name", &self.friendly_name)
            .field("port_type", &self.port_type)
            .field("serial", &self.serial)
            .field("instance", &self.instance)
            .finish_non_exhaustive()
    }
}

impl DeviceInfo {
    fn from_raw(sdk: Sdk, raw: tSdkCameraDevInfo) -> Self {
        DeviceInfo {
            product_series: ffi::decode_c_chars(&raw.acProductSeries),
            product_name: ffi::decode_c_chars(&raw.acProductName),
            friendly_name: ffi::decode_c_chars(&raw.acFriendlyName),
            link_name: ffi::decode_c_chars(&raw.acLinkName),
            driver_version: ffi::decode_c_chars(&raw.acDriverVersion),
            sensor_type: ffi::decode_c_chars(&raw.acSensorType),
            port_type: ffi::decode_c_chars(&raw.acPortType),
            serial: ffi::decode_c_chars(&raw.acSn),
            instance: raw.uInstance,
            raw,
            sdk,
        }
    }

    /// The raw enumeration record, as passed to `CameraInit`.
    pub fn raw(&self) -> &tSdkCameraDevInfo {
        &self.raw
    }

    /// The SDK this device was enumerated with.
    pub fn sdk(&self) -> &Sdk {
        &self.sdk
    }

    /// Whether the device is a GigE camera, judged from its port type.
    pub fn is_gige(&self) -> bool {
        is_gige_port_type(&self.port_type)
    }

    /// Camera and host-NIC IP configuration (`CameraGigeGetIp`).
    pub fn gige_ip(&self) -> Result<GigeIp> {
        const N: usize = 32;
        let mut bufs = [[0 as c_char; N]; 6];
        let mut raw = self.raw;
        let [a, b, c, d, e, f] = &mut bufs;
        // SAFETY: each buffer is 32 bytes; the SDK requires at least 16.
        let status = unsafe {
            (self.sdk.api.CameraGigeGetIp)(
                &mut raw,
                a.as_mut_ptr(),
                b.as_mut_ptr(),
                c.as_mut_ptr(),
                d.as_mut_ptr(),
                e.as_mut_ptr(),
                f.as_mut_ptr(),
            )
        };
        self.sdk.check("CameraGigeGetIp", status)?;
        let s: Vec<String> = bufs.iter().map(|b| ffi::decode_c_chars(b)).collect();
        Ok(GigeIp {
            camera: IpConfig {
                ip: parse_ipv4_lenient(&s[0])?,
                mask: parse_ipv4_lenient(&s[1])?,
                gateway: parse_ipv4_lenient(&s[2])?,
            },
            host: IpConfig {
                ip: parse_ipv4_lenient(&s[3])?,
                mask: parse_ipv4_lenient(&s[4])?,
                gateway: parse_ipv4_lenient(&s[5])?,
            },
        })
    }

    /// Camera and host-NIC MAC addresses (`CameraGigeGetMac`), as formatted
    /// by the SDK.
    pub fn gige_mac(&self) -> Result<GigeMac> {
        let mut cam = [0 as c_char; 32];
        let mut host = [0 as c_char; 32];
        let mut raw = self.raw;
        // SAFETY: each buffer is 32 bytes; the SDK requires at least 18.
        let status = unsafe { (self.sdk.api.CameraGigeGetMac)(&mut raw, cam.as_mut_ptr(), host.as_mut_ptr()) };
        self.sdk.check("CameraGigeGetMac", status)?;
        Ok(GigeMac { camera: ffi::decode_c_chars(&cam), host: ffi::decode_c_chars(&host) })
    }
}

/// Heuristic used by the vendor's Python tools: the port type mentions GigE.
pub fn is_gige_port_type(port_type: &str) -> bool {
    let p = port_type.to_ascii_lowercase();
    p.contains("gige") || p.starts_with("net")
}

/// IPv4 address, subnet mask and gateway.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpConfig {
    pub ip: Ipv4Addr,
    pub mask: Ipv4Addr,
    pub gateway: Ipv4Addr,
}

impl IpConfig {
    /// Number of leading one bits in the mask.
    pub fn prefix_len(&self) -> u32 {
        u32::from(self.mask).leading_ones()
    }

    /// Network address (`ip & mask`).
    pub fn network(&self) -> Ipv4Addr {
        Ipv4Addr::from(u32::from(self.ip) & u32::from(self.mask))
    }

    /// Whether `addr` lies in this subnet.
    pub fn contains(&self, addr: Ipv4Addr) -> bool {
        let m = u32::from(self.mask);
        u32::from(addr) & m == u32::from(self.ip) & m
    }
}

impl fmt::Display for IpConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "IP {}  Mask {}  GW {}", self.ip, self.mask, self.gateway)
    }
}

/// Result of [`DeviceInfo::gige_ip`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GigeIp {
    /// The camera's own configuration.
    pub camera: IpConfig,
    /// The host NIC through which the camera was discovered.
    pub host: IpConfig,
}

impl GigeIp {
    /// Whether the camera address lies in the host NIC's subnet. When it does
    /// not, streaming will fail until the camera is moved with `setip`.
    pub fn same_subnet(&self) -> bool {
        self.host.contains(self.camera.ip)
    }
}

/// Result of [`DeviceInfo::gige_mac`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GigeMac {
    pub camera: String,
    pub host: String,
}

/// Parses an IPv4 address returned by the SDK; empty text maps to `0.0.0.0`.
pub fn parse_ipv4_lenient(text: &str) -> Result<Ipv4Addr> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(Ipv4Addr::UNSPECIFIED);
    }
    t.parse()
        .map_err(|_| Error::InvalidResponse(format!("not an IPv4 address: {t:?}")))
}

/// Reduces a MAC address in any common notation (`00-1A-2B-...`,
/// `00:1a:2b:...`, `001a.2b3c.4d5e`) to 12 lowercase hex digits.
pub fn normalize_mac(text: &str) -> Option<String> {
    let hex: String = text
        .chars()
        .filter(|c| !matches!(c, ':' | '-' | '.' | ' '))
        .map(|c| c.to_ascii_lowercase())
        .collect();
    (hex.len() == 12 && hex.chars().all(|c| c.is_ascii_hexdigit())).then_some(hex)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdk_home_resolution() {
        let home = Path::new("/home/u");
        let r = |v: Option<&str>| resolve_sdk_home(v.map(OsStr::new), Some(home));
        assert_eq!(r(None), Some((PathBuf::from("/home/u/.gcam-mvsdk"), HomeSource::Default)));
        assert_eq!(r(Some("")), Some((PathBuf::from("/home/u/.gcam-mvsdk"), HomeSource::Default)));
        assert_eq!(r(Some("   ")), Some((PathBuf::from("/home/u/.gcam-mvsdk"), HomeSource::Default)));
        assert_eq!(r(Some("/opt/sdk")), Some((PathBuf::from("/opt/sdk"), HomeSource::Env)));
        assert_eq!(r(Some(" /opt/sdk ")), Some((PathBuf::from("/opt/sdk"), HomeSource::Env)));
        assert_eq!(r(Some("~/cams")), Some((PathBuf::from("/home/u/cams"), HomeSource::Env)));
        assert_eq!(r(Some("~")), Some((PathBuf::from("/home/u"), HomeSource::Env)));
        assert_eq!(r(Some("rel/dir")), Some((PathBuf::from("rel/dir"), HomeSource::Env)));
        assert_eq!(resolve_sdk_home(None, None), None);
        assert_eq!(
            resolve_sdk_home(Some(OsStr::new("/opt/sdk")), None),
            Some((PathBuf::from("/opt/sdk"), HomeSource::Env))
        );
        assert_eq!(HomeSource::Env.to_string(), "from $GCAM_MVSDK_HOME");
        assert_eq!(HomeSource::Default.to_string(), "default");
    }

    #[test]
    fn version_ordering() {
        assert_eq!(compare_versions("2.1.0.49", "2.1.10.195"), Ordering::Less);
        assert_eq!(compare_versions("2.1.0.49", "2.1.0.49"), Ordering::Equal);
        assert_eq!(compare_versions("2.1.0.50", "2.1.0.49"), Ordering::Greater);
        assert_eq!(compare_versions("2.1", "2.1.0"), Ordering::Less);
    }

    #[test]
    fn mac_normalisation() {
        assert_eq!(normalize_mac("00-1A-2B-3C-4D-5E").as_deref(), Some("001a2b3c4d5e"));
        assert_eq!(normalize_mac("00:1a:2b:3c:4d:5e").as_deref(), Some("001a2b3c4d5e"));
        assert_eq!(normalize_mac("001a.2b3c.4d5e").as_deref(), Some("001a2b3c4d5e"));
        assert_eq!(normalize_mac("00:1a:2b"), None);
        assert_eq!(normalize_mac("zz:1a:2b:3c:4d:5e"), None);
    }

    #[test]
    fn ip_config_subnet() {
        let host = IpConfig {
            ip: "192.168.10.1".parse().unwrap(),
            mask: "255.255.255.0".parse().unwrap(),
            gateway: Ipv4Addr::UNSPECIFIED,
        };
        assert_eq!(host.prefix_len(), 24);
        assert_eq!(host.network(), "192.168.10.0".parse::<Ipv4Addr>().unwrap());
        assert!(host.contains("192.168.10.77".parse().unwrap()));
        assert!(!host.contains("192.168.11.77".parse().unwrap()));
    }

    #[test]
    fn lenient_ip_parsing() {
        assert_eq!(parse_ipv4_lenient("").unwrap(), Ipv4Addr::UNSPECIFIED);
        assert_eq!(parse_ipv4_lenient(" 10.0.0.2 ").unwrap(), Ipv4Addr::new(10, 0, 0, 2));
        assert!(parse_ipv4_lenient("10.0.0").is_err());
    }

    #[test]
    fn gige_port_type_heuristic() {
        assert!(is_gige_port_type("GigE"));
        assert!(is_gige_port_type("NET-1000M"));
        assert!(!is_gige_port_type("USB3.0"));
    }
}
