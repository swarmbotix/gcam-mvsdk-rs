//! Error types.

use std::fmt;
use std::path::PathBuf;

use crate::ffi::{status_info, CameraSdkStatus, CAMERA_STATUS_TIME_OUT};

/// A non-zero status code returned by an SDK function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MvError {
    /// Raw status code (see [`crate::ffi::STATUS_TABLE`]).
    pub status: CameraSdkStatus,
    /// Name of the SDK function that returned the code.
    pub function: &'static str,
    /// Text from `CameraGetErrorString`, or the built-in description when the
    /// SDK returned none.
    pub message: String,
}

impl MvError {
    /// Builds an error with the built-in description for `status`.
    pub fn new(function: &'static str, status: CameraSdkStatus) -> Self {
        let message = status_info(status)
            .map(|s| s.description.to_string())
            .unwrap_or_else(|| "unknown status code".to_string());
        MvError { status, function, message }
    }

    /// Symbolic name of the status code, such as `CAMERA_STATUS_TIME_OUT`.
    pub fn status_name(&self) -> Option<&'static str> {
        status_info(self.status).map(|s| s.name)
    }

    /// Whether the code is `CAMERA_STATUS_TIME_OUT`.
    pub fn is_timeout(&self) -> bool {
        self.status == CAMERA_STATUS_TIME_OUT
    }
}

impl fmt::Display for MvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.status_name() {
            Some(name) => write!(
                f,
                "{} failed with status {} ({}): {}",
                self.function, self.status, name, self.message
            ),
            None => write!(f, "{} failed with status {}: {}", self.function, self.status, self.message),
        }
    }
}

impl std::error::Error for MvError {}

/// Failure to locate or open the vendor library.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LoadError {
    /// The library file exists (or was requested by name) but could not be opened.
    #[error("could not open the MVSDK library {}: {source}", path.display())]
    Open {
        path: PathBuf,
        #[source]
        source: libloading::Error,
    },
    /// The library opened but lacks a function this crate requires.
    #[error("{} does not export the required function {symbol}: {source}", path.display())]
    MissingSymbol {
        path: PathBuf,
        symbol: &'static str,
        #[source]
        source: libloading::Error,
    },
    /// No candidate location contained the library.
    #[error("{}", not_found_message(library, searched, system_error.as_deref()))]
    NotFound {
        /// File name that was searched for.
        library: String,
        /// Every location that was checked, in order.
        searched: Vec<String>,
        /// Error from the system loader, when the bare name was tried.
        system_error: Option<String>,
    },
    /// The host platform has no vendor build.
    #[error("unsupported platform: {0}")]
    UnsupportedPlatform(String),
    /// A different library was already loaded in this process.
    #[error(
        "the MVSDK library is already loaded from {} in this process; cannot also load {}",
        loaded.display(),
        requested.display()
    )]
    AlreadyLoaded { loaded: PathBuf, requested: PathBuf },
}

fn not_found_message(library: &str, searched: &[String], system_error: Option<&str>) -> String {
    let mut msg = format!("could not find the HuaTeng MVSDK library ({library}). Looked in:\n");
    for s in searched {
        msg.push_str("  - ");
        msg.push_str(s);
        msg.push('\n');
    }
    if let Some(e) = system_error {
        msg.push_str("  system loader: ");
        msg.push_str(e);
        msg.push('\n');
    }
    msg.push_str(
        "Fix: run `gcam-mvsdk-rs setup` to download and verify the vendor SDK, \
         or set GCAM_MVSDK_DIR to an extracted SDK directory (the one containing lib/).",
    );
    msg
}

/// Error type for every fallible operation in this crate.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// An SDK function returned a non-zero status.
    #[error(transparent)]
    Sdk(#[from] MvError),
    /// The vendor library could not be located or opened.
    #[error(transparent)]
    Load(#[from] LoadError),
    /// The loaded SDK build does not export a function this call needs.
    #[error("{0} is not exported by the loaded MVSDK library")]
    Unsupported(&'static str),
    /// No device matched a name or serial number.
    #[error("no camera matches {query:?}; available: {}", if available.is_empty() { "none".to_string() } else { available.join(", ") })]
    DeviceNotFound { query: String, available: Vec<String> },
    /// An argument was rejected before reaching the SDK.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    /// The SDK returned data that could not be interpreted.
    #[error("unexpected data from the SDK: {0}")]
    InvalidResponse(String),
    /// File system or other I/O failure.
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    /// Failure while downloading, verifying or installing the vendor SDK.
    #[error("{0}")]
    Setup(String),
}

impl Error {
    /// Wraps an I/O error with a description of what was being done.
    pub fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Error::Io { context: context.into(), source }
    }

    /// The SDK status code, when this error came from an SDK call.
    pub fn status(&self) -> Option<CameraSdkStatus> {
        match self {
            Error::Sdk(e) => Some(e.status),
            _ => None,
        }
    }

    /// Whether this is an SDK time-out (for example from `Camera::grab`).
    pub fn is_timeout(&self) -> bool {
        matches!(self, Error::Sdk(e) if e.is_timeout())
    }
}

/// Result alias using [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;
