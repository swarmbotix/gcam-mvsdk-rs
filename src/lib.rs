//! Rust bindings for the HuaTeng Vision MVSDK (GigE and USB industrial
//! cameras), loaded at runtime.
//!
//! The crate contains no vendor code and does not link against the SDK at
//! build time. [`Sdk::load`] finds `libMVSDK.so` (Linux) or
//! `MVCAMSDK_X64.dll` / `MVCAMSDK.dll` (Windows) at runtime, in this order:
//!
//! 1. `$GCAM_MVSDK_DIR/lib/<arch>`, `$GCAM_MVSDK_DIR/lib`, `$GCAM_MVSDK_DIR`
//!    (only these, when the variable is set);
//! 2. `./vendor/<version>/lib/<arch>` (developer checkout);
//! 3. `$GCAM_MVSDK_HOME/<version>/lib/<arch>`, where the SDK home defaults to
//!    `~/.gcam-mvsdk` (written by `gcam-mvsdk-rs setup`);
//! 4. the system loader path.
//!
//! ```no_run
//! use std::time::Duration;
//! use gcam_mvsdk_rs::{OutputFormat, Sdk};
//!
//! # fn main() -> gcam_mvsdk_rs::Result<()> {
//! let sdk = Sdk::load()?;
//! for dev in sdk.enumerate()? {
//!     println!("{} SN {} ({})", dev.friendly_name, dev.serial, dev.port_type);
//! }
//! let dev = sdk.find_device("sb00")?;
//! let cam = sdk.open(&dev)?;
//! cam.set_output_format(OutputFormat::Mono8)?;
//! cam.set_auto_exposure(false)?;
//! cam.set_exposure_us(5000.0)?;
//! cam.play()?;
//! let frame = cam.grab(Duration::from_millis(1000))?;
//! let image = frame.process()?;
//! println!("{}x{}, {} bytes", image.info.width, image.info.height, image.data.len());
//! # Ok(())
//! # }
//! ```
//!
//! The raw function table is available as [`ffi::Api`] through
//! [`Sdk::api`] for functions that have no safe wrapper yet.

pub mod camera;
pub mod doctor;
pub mod error;
pub mod ffi;
pub mod sdk;
#[cfg(feature = "setup")]
pub mod setup;

pub use camera::*;
pub use error::*;
pub use sdk::*;
