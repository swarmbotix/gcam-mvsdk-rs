//! An open camera and the frames it produces.

use std::cell::Cell;
use std::ffi::{c_char, c_int, c_uint};
use std::fmt;
use std::ptr::NonNull;
use std::time::Duration;

use crate::error::{Error, Result};
use crate::ffi::{self, CameraHandle, CameraSdkStatus};
use crate::sdk::{DeviceInfo, Sdk};

/// Maximum length in bytes of a user-set name, excluding the terminating NUL.
pub const MAX_USER_NAME_LEN: usize = 31;
/// Characters the SDK rejects in a user-set name.
pub const FORBIDDEN_NAME_CHARS: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

/// ISP output format selected with [`Camera::set_output_format`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// 8-bit grayscale, one byte per pixel.
    Mono8,
    /// 16-bit grayscale, two bytes per pixel.
    Mono16,
    /// 8-bit RGB, three bytes per pixel.
    Rgb8,
    /// 8-bit BGR, three bytes per pixel (the SDK default for color sensors).
    Bgr8,
    /// 8-bit RGBA, four bytes per pixel.
    Rgba8,
    /// 8-bit BGRA, four bytes per pixel.
    Bgra8,
    /// Any other `CAMERA_MEDIA_TYPE_*` value.
    Other(u32),
}

impl OutputFormat {
    /// The `CAMERA_MEDIA_TYPE_*` code.
    pub fn media_type(self) -> u32 {
        match self {
            OutputFormat::Mono8 => ffi::CAMERA_MEDIA_TYPE_MONO8,
            OutputFormat::Mono16 => ffi::CAMERA_MEDIA_TYPE_MONO16,
            OutputFormat::Rgb8 => ffi::CAMERA_MEDIA_TYPE_RGB8,
            OutputFormat::Bgr8 => ffi::CAMERA_MEDIA_TYPE_BGR8,
            OutputFormat::Rgba8 => ffi::CAMERA_MEDIA_TYPE_RGBA8,
            OutputFormat::Bgra8 => ffi::CAMERA_MEDIA_TYPE_BGRA8,
            OutputFormat::Other(v) => v,
        }
    }

    /// Maps a `CAMERA_MEDIA_TYPE_*` code back to a variant.
    pub fn from_media_type(code: u32) -> Self {
        match code {
            ffi::CAMERA_MEDIA_TYPE_MONO8 => OutputFormat::Mono8,
            ffi::CAMERA_MEDIA_TYPE_MONO16 => OutputFormat::Mono16,
            ffi::CAMERA_MEDIA_TYPE_RGB8 => OutputFormat::Rgb8,
            ffi::CAMERA_MEDIA_TYPE_BGR8 => OutputFormat::Bgr8,
            ffi::CAMERA_MEDIA_TYPE_RGBA8 => OutputFormat::Rgba8,
            ffi::CAMERA_MEDIA_TYPE_BGRA8 => OutputFormat::Bgra8,
            other => OutputFormat::Other(other),
        }
    }

    /// Bytes per pixel, rounded up; `None` when the code carries no pixel size.
    pub fn bytes_per_pixel(self) -> Option<usize> {
        match ffi::media_type_bits_per_pixel(self.media_type()) {
            0 => None,
            bits => Some((bits as usize).div_ceil(8)),
        }
    }
}

/// Acquisition trigger mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerMode {
    /// Free-running acquisition.
    Continuous,
    /// One acquisition per [`Camera::soft_trigger`].
    Software,
    /// One acquisition per external trigger signal.
    Hardware,
    /// Any other SDK value (line-scan encoder modes).
    Other(i32),
}

impl TriggerMode {
    /// SDK value for `CameraSetTriggerMode`.
    pub fn code(self) -> i32 {
        match self {
            TriggerMode::Continuous => 0,
            TriggerMode::Software => 1,
            TriggerMode::Hardware => 2,
            TriggerMode::Other(v) => v,
        }
    }

    /// Maps an SDK value back to a variant.
    pub fn from_code(code: i32) -> Self {
        match code {
            0 => TriggerMode::Continuous,
            1 => TriggerMode::Software,
            2 => TriggerMode::Hardware,
            v => TriggerMode::Other(v),
        }
    }
}

impl fmt::Display for TriggerMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TriggerMode::Continuous => f.write_str("continuous (free-run)"),
            TriggerMode::Software => f.write_str("software trigger"),
            TriggerMode::Hardware => f.write_str("hardware trigger"),
            TriggerMode::Other(v) => write!(f, "mode {v}"),
        }
    }
}

/// Parameter set used by [`Camera::save_parameter`] / [`Camera::load_parameter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterTeam {
    A,
    B,
    C,
    D,
    /// Factory defaults (load only).
    Default,
}

impl ParameterTeam {
    fn code(self) -> c_int {
        match self {
            ParameterTeam::A => ffi::PARAMETER_TEAM_A,
            ParameterTeam::B => ffi::PARAMETER_TEAM_B,
            ParameterTeam::C => ffi::PARAMETER_TEAM_C,
            ParameterTeam::D => ffi::PARAMETER_TEAM_D,
            ParameterTeam::Default => ffi::PARAMETER_TEAM_DEFAULT,
        }
    }
}

/// Entry of an indexed descriptor table (trigger modes, frame speeds, ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Indexed {
    pub index: i32,
    pub description: String,
}

/// GigE transport packet size choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackLength {
    pub index: i32,
    pub description: String,
    /// Packet size in bytes.
    pub size: u32,
}

/// Output format choice advertised by the camera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaTypeDesc {
    pub index: i32,
    pub description: String,
    pub media_type: u32,
}

/// Preset or current resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    pub index: i32,
    pub description: String,
    pub width: i32,
    pub height: i32,
    pub fov_offset_x: i32,
    pub fov_offset_y: i32,
    pub fov_width: i32,
    pub fov_height: i32,
    pub bin_sum_mode: u32,
    pub bin_average_mode: u32,
    pub skip_mode: u32,
}

impl From<&ffi::tSdkImageResolution> for Resolution {
    fn from(r: &ffi::tSdkImageResolution) -> Self {
        Resolution {
            index: r.iIndex,
            description: ffi::decode_c_chars(&r.acDescription),
            width: r.iWidth,
            height: r.iHeight,
            fov_offset_x: r.iHOffsetFOV,
            fov_offset_y: r.iVOffsetFOV,
            fov_width: r.iWidthFOV,
            fov_height: r.iHeightFOV,
            bin_sum_mode: r.uBinSumMode,
            bin_average_mode: r.uBinAverageMode,
            skip_mode: r.uSkipMode,
        }
    }
}

/// Owned copy of `tSdkCameraCapbility`, with the descriptor arrays copied out
/// of SDK memory.
#[derive(Debug, Clone, PartialEq)]
pub struct Capability {
    pub triggers: Vec<Indexed>,
    pub image_sizes: Vec<Resolution>,
    pub color_temperatures: Vec<Indexed>,
    pub media_types: Vec<MediaTypeDesc>,
    pub frame_speeds: Vec<Indexed>,
    pub pack_lengths: Vec<PackLength>,
    pub output_io_count: i32,
    pub input_io_count: i32,
    pub preset_luts: Vec<Indexed>,
    pub user_data_max_len: i32,
    pub param_in_device: bool,
    pub ae_algorithms_sw: Vec<Indexed>,
    pub ae_algorithms_hw: Vec<Indexed>,
    pub bayer_algorithms_sw: Vec<Indexed>,
    pub bayer_algorithms_hw: Vec<Indexed>,
    pub expose: ffi::tSdkExpose,
    pub resolution_range: ffi::tSdkResolutionRange,
    pub rgb_gain_range: ffi::tRgbGainRange,
    pub saturation_range: ffi::tSaturationRange,
    pub gamma_range: ffi::tGammaRange,
    pub contrast_range: ffi::tContrastRange,
    pub sharpness_range: ffi::tSharpnessRange,
    pub isp: ffi::tSdkIspCapacity,
}

impl Capability {
    /// Whether the sensor is monochrome.
    pub fn is_mono(&self) -> bool {
        self.isp.bMonoSensor != 0
    }

    /// Largest output width in pixels.
    pub fn max_width(&self) -> i32 {
        self.resolution_range.iWidthMax
    }

    /// Largest output height in pixels.
    pub fn max_height(&self) -> i32 {
        self.resolution_range.iHeightMax
    }

    fn from_raw(c: &ffi::tSdkCameraCapbility) -> Self {
        // SAFETY (all copy_table calls): the pointers and counts come from a
        // successful CameraGetCapability on an open handle; the SDK keeps the
        // tables alive while the handle is open, and we copy them immediately.
        unsafe {
            let indexed = |p: *const ffi::tSdkIndexedDescription, n: c_int| -> Vec<Indexed> {
                copy_table(p, n)
                    .iter()
                    .map(|d| Indexed { index: d.iIndex, description: ffi::decode_c_chars(&d.acDescription) })
                    .collect()
            };
            Capability {
                triggers: indexed(c.pTriggerDesc, c.iTriggerDesc),
                image_sizes: copy_table(c.pImageSizeDesc, c.iImageSizeDesc).iter().map(Resolution::from).collect(),
                color_temperatures: indexed(c.pClrTempDesc, c.iClrTempDesc),
                media_types: copy_table(c.pMediaTypeDesc, c.iMediaTypdeDesc)
                    .iter()
                    .map(|m| MediaTypeDesc {
                        index: m.iIndex,
                        description: ffi::decode_c_chars(&m.acDescription),
                        media_type: m.iMediaType,
                    })
                    .collect(),
                frame_speeds: indexed(c.pFrameSpeedDesc, c.iFrameSpeedDesc),
                pack_lengths: copy_table(c.pPackLenDesc, c.iPackLenDesc)
                    .iter()
                    .map(|p| PackLength {
                        index: p.iIndex,
                        description: ffi::decode_c_chars(&p.acDescription),
                        size: p.iPackSize,
                    })
                    .collect(),
                output_io_count: c.iOutputIoCounts,
                input_io_count: c.iInputIoCounts,
                preset_luts: indexed(c.pPresetLutDesc, c.iPresetLut),
                user_data_max_len: c.iUserDataMaxLen,
                param_in_device: c.bParamInDevice != 0,
                ae_algorithms_sw: indexed(c.pAeAlmSwDesc, c.iAeAlmSwDesc),
                ae_algorithms_hw: indexed(c.pAeAlmHdDesc, c.iAeAlmHdDesc),
                bayer_algorithms_sw: indexed(c.pBayerDecAlmSwDesc, c.iBayerDecAlmSwDesc),
                bayer_algorithms_hw: indexed(c.pBayerDecAlmHdDesc, c.iBayerDecAlmHdDesc),
                expose: c.sExposeDesc,
                resolution_range: c.sResolutionRange,
                rgb_gain_range: c.sRgbGainRange,
                saturation_range: c.sSaturationRange,
                gamma_range: c.sGammaRange,
                contrast_range: c.sContrastRange,
                sharpness_range: c.sSharpnessRange,
                isp: c.sIspCapacity,
            }
        }
    }
}

/// Upper bound on descriptor table lengths, as a guard against garbage counts.
const MAX_TABLE_LEN: c_int = 4096;

/// Copies `n` elements from an SDK-owned table.
///
/// # Safety
/// `ptr` must be null or valid for reads of `n` elements.
unsafe fn copy_table<T: Copy>(ptr: *const T, n: c_int) -> Vec<T> {
    if ptr.is_null() || n <= 0 {
        return Vec::new();
    }
    std::slice::from_raw_parts(ptr, n.min(MAX_TABLE_LEN) as usize).to_vec()
}

type GetI32 = unsafe extern "system" fn(CameraHandle, *mut c_int) -> CameraSdkStatus;

/// An open camera. Dropping it calls `CameraUnInit`.
///
/// `Camera` is `Send` but not `Sync`: the SDK does not document per-handle
/// thread safety, so a handle may move between threads but is used from one
/// thread at a time.
pub struct Camera {
    sdk: Sdk,
    handle: CameraHandle,
    serial: String,
    // ISP output format, cached so frames can size their output buffers.
    // `Cell` also makes `Camera` !Sync.
    out_format: Cell<u32>,
}

impl fmt::Debug for Camera {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Camera")
            .field("handle", &self.handle)
            .field("serial", &self.serial)
            .field("output_format", &OutputFormat::from_media_type(self.out_format.get()))
            .finish()
    }
}

impl Camera {
    /// Opens a device with `CameraInit`, reusing the last parameter set.
    pub fn open(sdk: &Sdk, dev: &DeviceInfo) -> Result<Camera> {
        let mut raw = *dev.raw();
        let mut handle: CameraHandle = 0;
        // SAFETY: the record comes from enumeration; out pointer is valid.
        let status = unsafe { (sdk.api().CameraInit)(&mut raw, -1, -1, &mut handle) };
        sdk.check("CameraInit", status)?;
        let cam = Camera { sdk: sdk.clone(), handle, serial: dev.serial.clone(), out_format: Cell::new(0) };
        let fmt = match cam.query_output_format() {
            Ok(f) => f,
            Err(_) => match cam.capability() {
                Ok(c) if c.is_mono() => ffi::CAMERA_MEDIA_TYPE_MONO8,
                _ => ffi::CAMERA_MEDIA_TYPE_BGR8,
            },
        };
        cam.out_format.set(fmt);
        Ok(cam)
    }

    /// Raw SDK handle, for calls through [`Sdk::api`].
    pub fn handle(&self) -> CameraHandle {
        self.handle
    }

    /// The SDK this camera belongs to.
    pub fn sdk(&self) -> &Sdk {
        &self.sdk
    }

    /// Serial number of the device this camera was opened from.
    pub fn serial(&self) -> &str {
        &self.serial
    }

    fn check(&self, function: &'static str, status: CameraSdkStatus) -> Result<()> {
        self.sdk.check(function, status)
    }

    fn get_i32(&self, function: &'static str, f: Option<GetI32>) -> Result<i32> {
        let f = f.ok_or(Error::Unsupported(function))?;
        let mut v: c_int = 0;
        // SAFETY: open handle and valid out pointer.
        let status = unsafe { f(self.handle, &mut v) };
        self.check(function, status)?;
        Ok(v)
    }

    /// Capability description (`CameraGetCapability`).
    pub fn capability(&self) -> Result<Capability> {
        let mut raw = ffi::tSdkCameraCapbility::default();
        // SAFETY: open handle and a zeroed struct of the correct layout.
        let status = unsafe { (self.sdk.api().CameraGetCapability)(self.handle, &mut raw) };
        self.check("CameraGetCapability", status)?;
        Ok(Capability::from_raw(&raw))
    }

    /// The user-set name stored in the camera (`CameraGetFriendlyName`).
    pub fn user_name(&self) -> Result<String> {
        let mut buf = [0 as c_char; 64];
        // SAFETY: the SDK writes at most 32 bytes; the buffer holds 64.
        let status = unsafe { (self.sdk.api().CameraGetFriendlyName)(self.handle, buf.as_mut_ptr()) };
        self.check("CameraGetFriendlyName", status)?;
        Ok(ffi::decode_c_chars(&buf))
    }

    /// Writes the user-set name to the camera's non-volatile memory
    /// (`CameraSetFriendlyName`). The name survives power cycles; enumeration
    /// reports it after the camera restarts.
    pub fn set_user_name(&self, name: &str) -> Result<()> {
        validate_user_name(name)?;
        let bytes = ffi::encode_sdk_text(name)
            .ok_or_else(|| Error::InvalidArgument(format!("name {name:?} cannot be encoded for the camera")))?;
        let mut buf = [0 as c_char; 64];
        for (dst, &b) in buf.iter_mut().zip(&bytes) {
            *dst = b as c_char;
        }
        // SAFETY: NUL-terminated buffer of 64 bytes, name at most 31 bytes.
        let status = unsafe { (self.sdk.api().CameraSetFriendlyName)(self.handle, buf.as_mut_ptr()) };
        self.check("CameraSetFriendlyName", status)
    }

    /// Firmware version string.
    pub fn firmware_version(&self) -> Result<String> {
        let f = self
            .sdk
            .api()
            .CameraGetFirmwareVersion
            .ok_or(Error::Unsupported("CameraGetFirmwareVersion"))?;
        let mut buf = [0 as c_char; 64];
        // SAFETY: buffer of 64 bytes; the SDK requires at least 32.
        let status = unsafe { f(self.handle, buf.as_mut_ptr()) };
        self.check("CameraGetFirmwareVersion", status)?;
        Ok(ffi::decode_c_chars(&buf))
    }

    /// Exposure time in microseconds.
    pub fn exposure_us(&self) -> Result<f64> {
        let mut v = 0.0f64;
        // SAFETY: open handle and valid out pointer.
        let status = unsafe { (self.sdk.api().CameraGetExposureTime)(self.handle, &mut v) };
        self.check("CameraGetExposureTime", status)?;
        Ok(v)
    }

    /// Sets the exposure time in microseconds. Has an effect only while
    /// automatic exposure is off (see [`Camera::set_auto_exposure`]).
    pub fn set_exposure_us(&self, us: f64) -> Result<()> {
        if !us.is_finite() || us <= 0.0 {
            return Err(Error::InvalidArgument(format!("exposure must be positive, got {us}")));
        }
        // SAFETY: open handle, plain value.
        let status = unsafe { (self.sdk.api().CameraSetExposureTime)(self.handle, us) };
        self.check("CameraSetExposureTime", status)
    }

    /// Exposure range `(min, max, step)` in microseconds.
    pub fn exposure_range_us(&self) -> Result<(f64, f64, f64)> {
        let f = self
            .sdk
            .api()
            .CameraGetExposureTimeRange
            .ok_or(Error::Unsupported("CameraGetExposureTimeRange"))?;
        let (mut lo, mut hi, mut step) = (0.0, 0.0, 0.0);
        // SAFETY: open handle and valid out pointers.
        let status = unsafe { f(self.handle, &mut lo, &mut hi, &mut step) };
        self.check("CameraGetExposureTimeRange", status)?;
        Ok((lo, hi, step))
    }

    /// Duration of one sensor line in microseconds.
    pub fn exposure_line_time_us(&self) -> Result<f64> {
        let f = self
            .sdk
            .api()
            .CameraGetExposureLineTime
            .ok_or(Error::Unsupported("CameraGetExposureLineTime"))?;
        let mut v = 0.0;
        // SAFETY: open handle and valid out pointer.
        let status = unsafe { f(self.handle, &mut v) };
        self.check("CameraGetExposureLineTime", status)?;
        Ok(v)
    }

    /// Whether automatic exposure is on.
    pub fn auto_exposure(&self) -> Result<bool> {
        self.get_i32("CameraGetAeState", self.sdk.api().CameraGetAeState).map(|v| v != 0)
    }

    /// Turns automatic exposure on or off.
    pub fn set_auto_exposure(&self, on: bool) -> Result<()> {
        // SAFETY: open handle, plain value.
        let status = unsafe { (self.sdk.api().CameraSetAeState)(self.handle, c_int::from(on)) };
        self.check("CameraSetAeState", status)
    }

    /// Caps the frame rate in Hz; `hz <= 0` removes the cap.
    pub fn set_frame_rate(&self, hz: i32) -> Result<()> {
        // SAFETY: open handle, plain value.
        let status = unsafe { (self.sdk.api().CameraSetFrameRate)(self.handle, hz) };
        self.check("CameraSetFrameRate", status)
    }

    /// Current frame-rate cap in Hz.
    pub fn frame_rate(&self) -> Result<i32> {
        self.get_i32("CameraGetFrameRate", self.sdk.api().CameraGetFrameRate)
    }

    /// Selects a frame-speed tier (index into [`Capability::frame_speeds`]).
    pub fn set_frame_speed(&self, index: i32) -> Result<()> {
        let f = self.sdk.api().CameraSetFrameSpeed.ok_or(Error::Unsupported("CameraSetFrameSpeed"))?;
        // SAFETY: open handle, plain value.
        let status = unsafe { f(self.handle, index) };
        self.check("CameraSetFrameSpeed", status)
    }

    /// Current frame-speed tier index.
    pub fn frame_speed(&self) -> Result<i32> {
        self.get_i32("CameraGetFrameSpeed", self.sdk.api().CameraGetFrameSpeed)
    }

    /// Selects the GigE packet size by index into [`Capability::pack_lengths`].
    pub fn set_trans_pack_len(&self, index: i32) -> Result<()> {
        // SAFETY: open handle, plain value.
        let status = unsafe { (self.sdk.api().CameraSetTransPackLen)(self.handle, index) };
        self.check("CameraSetTransPackLen", status)
    }

    /// Selects the largest GigE packet size the camera offers that does not
    /// exceed `max_bytes` (normally the NIC MTU) and returns it.
    pub fn set_gige_packet_size(&self, max_bytes: u32) -> Result<u32> {
        let cap = self.capability()?;
        let best = cap.pack_lengths.iter().filter(|p| p.size <= max_bytes).max_by_key(|p| p.size);
        match best {
            Some(p) => {
                self.set_trans_pack_len(p.index)?;
                Ok(p.size)
            }
            None => {
                let offered: Vec<String> = cap.pack_lengths.iter().map(|p| p.size.to_string()).collect();
                Err(Error::InvalidArgument(format!(
                    "the camera offers no packet size <= {max_bytes} bytes (offered: {})",
                    if offered.is_empty() { "none".to_string() } else { offered.join(", ") }
                )))
            }
        }
    }

    fn query_output_format(&self) -> Result<u32> {
        let f = self.sdk.api().CameraGetIspOutFormat.ok_or(Error::Unsupported("CameraGetIspOutFormat"))?;
        let mut v: c_uint = 0;
        // SAFETY: open handle and valid out pointer.
        let status = unsafe { f(self.handle, &mut v) };
        self.check("CameraGetIspOutFormat", status)?;
        Ok(v)
    }

    /// Current ISP output format.
    pub fn output_format(&self) -> OutputFormat {
        OutputFormat::from_media_type(self.out_format.get())
    }

    /// Selects the ISP output format used by [`Frame::process_into`].
    pub fn set_output_format(&self, format: OutputFormat) -> Result<()> {
        let code = format.media_type();
        if format.bytes_per_pixel().is_none() {
            return Err(Error::InvalidArgument(format!("media type {code:#010x} has no pixel size")));
        }
        // SAFETY: open handle, plain value.
        let status = unsafe { (self.sdk.api().CameraSetIspOutFormat)(self.handle, code) };
        self.check("CameraSetIspOutFormat", status)?;
        self.out_format.set(code);
        Ok(())
    }

    /// Selects the trigger mode.
    pub fn set_trigger_mode(&self, mode: TriggerMode) -> Result<()> {
        // SAFETY: open handle, plain value.
        let status = unsafe { (self.sdk.api().CameraSetTriggerMode)(self.handle, mode.code()) };
        self.check("CameraSetTriggerMode", status)
    }

    /// Current trigger mode.
    pub fn trigger_mode(&self) -> Result<TriggerMode> {
        self.get_i32("CameraGetTriggerMode", self.sdk.api().CameraGetTriggerMode)
            .map(TriggerMode::from_code)
    }

    /// Trigger delay in microseconds.
    pub fn trigger_delay_us(&self) -> Result<u32> {
        let f = self
            .sdk
            .api()
            .CameraGetTriggerDelayTime
            .ok_or(Error::Unsupported("CameraGetTriggerDelayTime"))?;
        let mut v: c_uint = 0;
        // SAFETY: open handle and valid out pointer.
        let status = unsafe { f(self.handle, &mut v) };
        self.check("CameraGetTriggerDelayTime", status)?;
        Ok(v)
    }

    /// Frames captured per trigger.
    pub fn trigger_count(&self) -> Result<i32> {
        self.get_i32("CameraGetTriggerCount", self.sdk.api().CameraGetTriggerCount)
    }

    /// Issues a software trigger (in [`TriggerMode::Software`]).
    pub fn soft_trigger(&self) -> Result<()> {
        let f = self.sdk.api().CameraSoftTrigger.ok_or(Error::Unsupported("CameraSoftTrigger"))?;
        // SAFETY: open handle.
        let status = unsafe { f(self.handle) };
        self.check("CameraSoftTrigger", status)
    }

    /// Analog gain in SDK units.
    pub fn analog_gain(&self) -> Result<i32> {
        self.get_i32("CameraGetAnalogGain", self.sdk.api().CameraGetAnalogGain)
    }

    /// Digital R, G, B gains (100 means 1.0x).
    pub fn rgb_gain(&self) -> Result<(i32, i32, i32)> {
        let f = self.sdk.api().CameraGetGain.ok_or(Error::Unsupported("CameraGetGain"))?;
        let (mut r, mut g, mut b) = (0, 0, 0);
        // SAFETY: open handle and valid out pointers.
        let status = unsafe { f(self.handle, &mut r, &mut g, &mut b) };
        self.check("CameraGetGain", status)?;
        Ok((r, g, b))
    }

    /// Gamma setting.
    pub fn gamma(&self) -> Result<i32> {
        self.get_i32("CameraGetGamma", self.sdk.api().CameraGetGamma)
    }

    /// Contrast setting.
    pub fn contrast(&self) -> Result<i32> {
        self.get_i32("CameraGetContrast", self.sdk.api().CameraGetContrast)
    }

    /// Saturation setting.
    pub fn saturation(&self) -> Result<i32> {
        self.get_i32("CameraGetSaturation", self.sdk.api().CameraGetSaturation)
    }

    /// Sharpness setting.
    pub fn sharpness(&self) -> Result<i32> {
        self.get_i32("CameraGetSharpness", self.sdk.api().CameraGetSharpness)
    }

    /// Current output resolution.
    pub fn image_resolution(&self) -> Result<Resolution> {
        let f = self
            .sdk
            .api()
            .CameraGetImageResolution
            .ok_or(Error::Unsupported("CameraGetImageResolution"))?;
        let mut r = ffi::tSdkImageResolution::default();
        // SAFETY: open handle and a struct of the correct layout.
        let status = unsafe { f(self.handle, &mut r) };
        self.check("CameraGetImageResolution", status)?;
        Ok(Resolution::from(&r))
    }

    /// Acquisition statistics `(total, captured, lost)`.
    pub fn frame_statistic(&self) -> Result<(i32, i32, i32)> {
        let f = self
            .sdk
            .api()
            .CameraGetFrameStatistic
            .ok_or(Error::Unsupported("CameraGetFrameStatistic"))?;
        let mut s = ffi::tSdkFrameStatistic::default();
        // SAFETY: open handle and a struct of the correct layout.
        let status = unsafe { f(self.handle, &mut s) };
        self.check("CameraGetFrameStatistic", status)?;
        Ok((s.iTotal, s.iCapture, s.iLost))
    }

    /// Saves the current parameters into a parameter team.
    pub fn save_parameter(&self, team: ParameterTeam) -> Result<()> {
        // SAFETY: open handle, plain value.
        let status = unsafe { (self.sdk.api().CameraSaveParameter)(self.handle, team.code()) };
        self.check("CameraSaveParameter", status)
    }

    /// Loads parameters from a parameter team.
    pub fn load_parameter(&self, team: ParameterTeam) -> Result<()> {
        // SAFETY: open handle, plain value.
        let status = unsafe { (self.sdk.api().CameraLoadParameter)(self.handle, team.code()) };
        self.check("CameraLoadParameter", status)?;
        if let Ok(f) = self.query_output_format() {
            self.out_format.set(f);
        }
        Ok(())
    }

    /// Starts streaming.
    pub fn play(&self) -> Result<()> {
        // SAFETY: open handle.
        let status = unsafe { (self.sdk.api().CameraPlay)(self.handle) };
        self.check("CameraPlay", status)
    }

    /// Pauses streaming.
    pub fn pause(&self) -> Result<()> {
        // SAFETY: open handle.
        let status = unsafe { (self.sdk.api().CameraPause)(self.handle) };
        self.check("CameraPause", status)
    }

    /// Waits up to `timeout` for the next raw frame (`CameraGetImageBuffer`).
    ///
    /// The returned [`Frame`] borrows the camera and gives the buffer back to
    /// the SDK when dropped. On time-out the error satisfies
    /// [`Error::is_timeout`].
    pub fn grab(&self, timeout: Duration) -> Result<Frame<'_>> {
        let ms = timeout.as_millis().min(u128::from(c_uint::MAX)) as c_uint;
        let mut head = ffi::tSdkFrameHead::default();
        let mut buf: *mut u8 = std::ptr::null_mut();
        // SAFETY: open handle and valid out pointers.
        let status = unsafe { (self.sdk.api().CameraGetImageBuffer)(self.handle, &mut head, &mut buf, ms) };
        self.check("CameraGetImageBuffer", status)?;
        let data = NonNull::new(buf)
            .ok_or_else(|| Error::InvalidResponse("CameraGetImageBuffer returned a null buffer".into()))?;
        Ok(Frame { camera: self, head, data })
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        // SAFETY: the handle came from CameraInit and is closed exactly once.
        unsafe {
            (self.sdk.api().CameraUnInit)(self.handle);
        }
    }
}

/// Checks a user-set name before it is sent to the camera.
pub fn validate_user_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::InvalidArgument("name must not be empty".into()));
    }
    if let Some(c) = name.chars().find(|c| FORBIDDEN_NAME_CHARS.contains(c) || c.is_control()) {
        return Err(Error::InvalidArgument(format!(
            "name {name:?} contains {c:?}; the camera rejects \\ / : * ? \" < > | and control characters"
        )));
    }
    let len = ffi::encode_sdk_text(name)
        .ok_or_else(|| Error::InvalidArgument(format!("name {name:?} cannot be encoded for the camera")))?
        .len();
    if len > MAX_USER_NAME_LEN {
        return Err(Error::InvalidArgument(format!(
            "name {name:?} is {len} bytes; the camera stores at most {MAX_USER_NAME_LEN}"
        )));
    }
    Ok(())
}

/// Size and format of an image produced by [`Frame::process_into`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    /// `CAMERA_MEDIA_TYPE_*` code of the processed pixels.
    pub media_type: u32,
    /// Number of meaningful bytes written to the output buffer.
    pub len: usize,
}

/// An owned processed image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub info: ImageInfo,
    pub data: Vec<u8>,
}

/// A raw frame held by the SDK. Dropping it calls `CameraReleaseImageBuffer`.
pub struct Frame<'a> {
    camera: &'a Camera,
    head: ffi::tSdkFrameHead,
    data: NonNull<u8>,
}

impl fmt::Debug for Frame<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Frame")
            .field("width", &self.width())
            .field("height", &self.height())
            .field("media_type", &format_args!("{:#010x}", self.media_type()))
            .field("bytes", &self.len())
            .field("timestamp", &self.timestamp())
            .finish()
    }
}

impl Frame<'_> {
    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.head.iWidth.max(0) as u32
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.head.iHeight.max(0) as u32
    }

    /// `CAMERA_MEDIA_TYPE_*` code of the raw data.
    pub fn media_type(&self) -> u32 {
        self.head.uiMediaType
    }

    /// Number of raw bytes.
    pub fn len(&self) -> usize {
        self.head.uBytes as usize
    }

    /// Whether the raw buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Camera time stamp in the SDK's native unit of 0.1 ms.
    pub fn timestamp_100us(&self) -> u32 {
        self.head.uiTimeStamp
    }

    /// Camera time stamp as a duration since an arbitrary camera epoch.
    pub fn timestamp(&self) -> Duration {
        Duration::from_micros(u64::from(self.head.uiTimeStamp) * 100)
    }

    /// Exposure of this frame in microseconds.
    pub fn exposure_us(&self) -> u32 {
        self.head.uiExpTime
    }

    /// Analog gain factor of this frame.
    pub fn analog_gain(&self) -> f32 {
        self.head.fAnalogGain
    }

    /// Whether the frame was produced by a trigger.
    pub fn is_trigger(&self) -> bool {
        self.head.bIsTrigger != 0
    }

    /// The raw frame header.
    pub fn head(&self) -> &ffi::tSdkFrameHead {
        &self.head
    }

    /// The raw sensor data, valid until the frame is dropped.
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: the SDK guarantees `uBytes` readable bytes at the buffer until
        // it is released in Drop; `self` borrows the camera so it stays open.
        unsafe { std::slice::from_raw_parts(self.data.as_ptr(), self.len()) }
    }

    /// Bytes [`Frame::process_into`] needs for the camera's output format.
    pub fn processed_len(&self) -> Result<usize> {
        let format = self.camera.output_format();
        let bpp = format.bytes_per_pixel().ok_or_else(|| {
            Error::InvalidResponse(format!("ISP output format {:#010x} has no pixel size", format.media_type()))
        })?;
        let w = self.head.iWidth.max(self.head.iWidthZoomSw).max(0) as usize;
        let h = self.head.iHeight.max(self.head.iHeightZoomSw).max(0) as usize;
        Ok(w * h * bpp)
    }

    /// Runs the SDK ISP (`CameraImageProcess`) into `out`, which must hold at
    /// least [`Frame::processed_len`] bytes.
    pub fn process_into(&self, out: &mut [u8]) -> Result<ImageInfo> {
        let need = self.processed_len()?;
        if out.len() < need {
            return Err(Error::InvalidArgument(format!(
                "output buffer holds {} bytes, {need} required",
                out.len()
            )));
        }
        let mut head = self.head;
        // SAFETY: input is the SDK's own buffer; output was checked above for
        // the size implied by the output format; header copy is writable.
        let status = unsafe {
            (self.camera.sdk.api().CameraImageProcess)(
                self.camera.handle,
                self.data.as_ptr(),
                out.as_mut_ptr(),
                &mut head,
            )
        };
        self.camera.check("CameraImageProcess", status)?;
        let width = head.iWidth.max(0) as u32;
        let height = head.iHeight.max(0) as u32;
        let media_type = self.camera.out_format.get();
        let bpp = OutputFormat::from_media_type(media_type).bytes_per_pixel().unwrap_or(1);
        let len = (width as usize * height as usize * bpp).min(need);
        Ok(ImageInfo { width, height, media_type, len })
    }

    /// Like [`Frame::process_into`] with a freshly allocated buffer.
    pub fn process(&self) -> Result<Image> {
        let mut data = vec![0u8; self.processed_len()?];
        let info = self.process_into(&mut data)?;
        data.truncate(info.len);
        Ok(Image { info, data })
    }
}

impl Drop for Frame<'_> {
    fn drop(&mut self) {
        // SAFETY: buffer obtained from CameraGetImageBuffer on this handle and
        // released exactly once.
        unsafe {
            (self.camera.sdk.api().CameraReleaseImageBuffer)(self.camera.handle, self.data.as_ptr());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_format_codes() {
        for f in [
            OutputFormat::Mono8,
            OutputFormat::Mono16,
            OutputFormat::Rgb8,
            OutputFormat::Bgr8,
            OutputFormat::Rgba8,
            OutputFormat::Bgra8,
        ] {
            assert_eq!(OutputFormat::from_media_type(f.media_type()), f);
        }
        assert_eq!(OutputFormat::Mono8.bytes_per_pixel(), Some(1));
        assert_eq!(OutputFormat::Bgr8.bytes_per_pixel(), Some(3));
        assert_eq!(OutputFormat::Rgba8.bytes_per_pixel(), Some(4));
        assert_eq!(OutputFormat::Mono16.bytes_per_pixel(), Some(2));
        assert_eq!(OutputFormat::Other(0x0100_0001).bytes_per_pixel(), None);
    }

    #[test]
    fn trigger_codes() {
        assert_eq!(TriggerMode::from_code(0), TriggerMode::Continuous);
        assert_eq!(TriggerMode::Hardware.code(), 2);
        assert_eq!(TriggerMode::from_code(4), TriggerMode::Other(4));
    }

    #[test]
    fn user_name_validation() {
        assert!(validate_user_name("sb03").is_ok());
        assert!(validate_user_name("").is_err());
        assert!(validate_user_name("a/b").is_err());
        assert!(validate_user_name("a\"b").is_err());
        assert!(validate_user_name(&"x".repeat(31)).is_ok());
        assert!(validate_user_name(&"x".repeat(32)).is_err());
        // 16 GBK characters are 32 bytes.
        assert!(validate_user_name(&"中".repeat(16)).is_err());
        assert!(validate_user_name(&"中".repeat(15)).is_ok());
    }
}
