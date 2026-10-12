//! Camera controls beyond the basics in [`crate::camera`]: gain, auto exposure,
//! trigger timing, geometry, image processing, digital I/O and strobe, GigE
//! transport, and parameter storage.
//!
//! Every method calls one SDK function. Whether a camera supports it, and in
//! which range, depends on the model: [`Camera::capability`] reports the ranges
//! and feature flags, and an unsupported call returns an SDK error.
//! `docs/CAPABILITIES.md` lists each control with its vendor function.

use std::ffi::{c_int, c_uint};

use crate::camera::{Camera, ParameterTeam, Resolution};
use crate::error::{Error, Result};
use crate::ffi::{self, CameraHandle, CameraSdkStatus};

type SetI32 = unsafe extern "system" fn(CameraHandle, c_int) -> CameraSdkStatus;
type SetU32 = unsafe extern "system" fn(CameraHandle, c_uint) -> CameraSdkStatus;
type Call0 = unsafe extern "system" fn(CameraHandle) -> CameraSdkStatus;

/// Mirror direction (`emSdkMirrorDirection`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirrorDirection {
    Horizontal,
    Vertical,
}

impl MirrorDirection {
    /// SDK value: 0 horizontal, 1 vertical.
    pub fn code(self) -> c_int {
        match self {
            MirrorDirection::Horizontal => 0,
            MirrorDirection::Vertical => 1,
        }
    }
}

/// Counterclockwise image rotation (`emSdkRotateDirection`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
    None,
    Ccw90,
    Ccw180,
    Ccw270,
}

impl Rotation {
    /// SDK value: 0 to 3.
    pub fn code(self) -> c_int {
        match self {
            Rotation::None => 0,
            Rotation::Ccw90 => 1,
            Rotation::Ccw180 => 2,
            Rotation::Ccw270 => 3,
        }
    }

    /// Maps an SDK value back to a rotation.
    pub fn from_code(code: c_int) -> Option<Self> {
        match code {
            0 => Some(Rotation::None),
            1 => Some(Rotation::Ccw90),
            2 => Some(Rotation::Ccw180),
            3 => Some(Rotation::Ccw270),
            _ => None,
        }
    }
}

/// Mains frequency the anti-flicker function compensates (`emSdkLightFrequency`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightFrequency {
    Hz50,
    Hz60,
}

impl LightFrequency {
    /// SDK value: 0 for 50 Hz, 1 for 60 Hz.
    pub fn code(self) -> c_int {
        match self {
            LightFrequency::Hz50 => 0,
            LightFrequency::Hz60 => 1,
        }
    }

    /// Maps an SDK value back to a frequency.
    pub fn from_code(code: c_int) -> Option<Self> {
        match code {
            0 => Some(LightFrequency::Hz50),
            1 => Some(LightFrequency::Hz60),
            _ => None,
        }
    }
}

/// Where parameter groups A to D are stored and looked up (`emSdkParameterMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterMode {
    /// Files shared by all cameras of the same model.
    ByModel,
    /// Files shared by cameras with the same nickname.
    ByName,
    /// Files per serial number.
    BySerial,
    /// The camera's own storage (only when `Capability::param_in_device`).
    InDevice,
}

impl ParameterMode {
    /// SDK value: 0 to 3.
    pub fn code(self) -> c_int {
        match self {
            ParameterMode::ByModel => ffi::PARAM_MODE_BY_MODEL,
            ParameterMode::ByName => ffi::PARAM_MODE_BY_NAME,
            ParameterMode::BySerial => ffi::PARAM_MODE_BY_SN,
            ParameterMode::InDevice => ffi::PARAM_MODE_IN_DEVICE,
        }
    }

    /// Maps an SDK value back to a mode.
    pub fn from_code(code: c_int) -> Option<Self> {
        match code {
            ffi::PARAM_MODE_BY_MODEL => Some(ParameterMode::ByModel),
            ffi::PARAM_MODE_BY_NAME => Some(ParameterMode::ByName),
            ffi::PARAM_MODE_BY_SN => Some(ParameterMode::BySerial),
            ffi::PARAM_MODE_IN_DEVICE => Some(ParameterMode::InDevice),
            _ => None,
        }
    }
}

/// STROBE output mode (`emStrobeControl`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrobeMode {
    /// Follows the trigger: the signal is generated automatically during exposure.
    TriggerAuto,
    /// Follows the trigger after the strobe delay, for the strobe pulse width.
    TriggerManual,
    /// Always high; other strobe settings are ignored.
    AlwaysHigh,
    /// Always low; other strobe settings are ignored.
    AlwaysLow,
}

impl StrobeMode {
    /// SDK value: 0 to 3.
    pub fn code(self) -> c_int {
        match self {
            StrobeMode::TriggerAuto => 0,
            StrobeMode::TriggerManual => 1,
            StrobeMode::AlwaysHigh => 2,
            StrobeMode::AlwaysLow => 3,
        }
    }

    /// Maps an SDK value back to a mode.
    pub fn from_code(code: c_int) -> Option<Self> {
        match code {
            0 => Some(StrobeMode::TriggerAuto),
            1 => Some(StrobeMode::TriggerManual),
            2 => Some(StrobeMode::AlwaysHigh),
            3 => Some(StrobeMode::AlwaysLow),
            _ => None,
        }
    }
}

/// Pixel combination for a custom region of interest. The factor `n` means
/// `n x n` pixels (2 to 17); which factors a camera supports is reported by the
/// masks in `Capability::resolution_range`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binning {
    /// Full resolution.
    None,
    /// Binning by summing.
    Sum(u8),
    /// Binning by averaging.
    Average(u8),
    /// Skipping (decimation).
    Skip(u8),
    /// Hardware resampling.
    Resample(u8),
}

impl Binning {
    /// `(Mode, ModeSize)` for `CameraSetImageResolutionEx`: mode 0 normal,
    /// 1 sum, 2 average, 3 skip, 4 resample; bit 0 of the size mask is 2x2,
    /// bit 1 is 3x3, and so on.
    pub fn mode_and_mask(self) -> Result<(c_int, c_uint)> {
        let (mode, n) = match self {
            Binning::None => return Ok((0, 0)),
            Binning::Sum(n) => (1, n),
            Binning::Average(n) => (2, n),
            Binning::Skip(n) => (3, n),
            Binning::Resample(n) => (4, n),
        };
        if !(2..=17).contains(&n) {
            return Err(Error::InvalidArgument(format!("binning factor must be 2 to 17, got {n}")));
        }
        Ok((mode, 1 << (n - 2)))
    }
}

/// Custom region of interest for [`Camera::set_roi`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Roi {
    /// Horizontal offset in pixels.
    pub x: i32,
    /// Vertical offset in pixels.
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub binning: Binning,
}

impl Roi {
    fn validate(&self) -> Result<()> {
        if self.x < 0 || self.y < 0 || self.width <= 0 || self.height <= 0 {
            return Err(Error::InvalidArgument(format!(
                "ROI needs x, y >= 0 and width, height > 0, got {self:?}"
            )));
        }
        Ok(())
    }
}

/// Fails with `InvalidArgument` unless `lo <= value <= hi`.
pub(crate) fn check_range(what: &str, value: i64, lo: i64, hi: i64) -> Result<()> {
    if value < lo || value > hi {
        return Err(Error::InvalidArgument(format!("{what} must be {lo} to {hi}, got {value}")));
    }
    Ok(())
}

impl Camera {
    fn set_i32(&self, function: &'static str, f: Option<SetI32>, value: i32) -> Result<()> {
        let f = f.ok_or(Error::Unsupported(function))?;
        // SAFETY: open handle; `f` has the (handle, int) signature of `function`.
        let status = unsafe { f(self.handle(), value) };
        self.sdk().check(function, status)
    }

    fn set_u32(&self, function: &'static str, f: Option<SetU32>, value: u32) -> Result<()> {
        let f = f.ok_or(Error::Unsupported(function))?;
        // SAFETY: open handle; `f` has the (handle, unsigned int) signature of `function`.
        let status = unsafe { f(self.handle(), value) };
        self.sdk().check(function, status)
    }

    fn call0(&self, function: &'static str, f: Option<Call0>) -> Result<()> {
        let f = f.ok_or(Error::Unsupported(function))?;
        // SAFETY: open handle; `f` takes only the handle.
        let status = unsafe { f(self.handle()) };
        self.sdk().check(function, status)
    }

    // -- Exposure and gain ---------------------------------------------------

    /// Sets the analog gain in SDK units (`CameraSetAnalogGain`). The factor is
    /// `value * Capability::expose.fAnalogGainStep`; the range is
    /// `expose.uiAnalogGainMin..=uiAnalogGainMax`.
    pub fn set_analog_gain(&self, value: i32) -> Result<()> {
        self.set_i32("CameraSetAnalogGain", self.sdk().api().CameraSetAnalogGain, value)
    }

    /// Auto-exposure brightness target (`CameraGetAeTarget`).
    pub fn ae_target(&self) -> Result<i32> {
        self.get_i32("CameraGetAeTarget", self.sdk().api().CameraGetAeTarget)
    }

    /// Sets the auto-exposure brightness target (`CameraSetAeTarget`); range
    /// `Capability::expose.uiTargetMin..=uiTargetMax`.
    pub fn set_ae_target(&self, target: i32) -> Result<()> {
        self.set_i32("CameraSetAeTarget", self.sdk().api().CameraSetAeTarget, target)
    }

    /// Sets the digital R, G, B gains (`CameraSetGain`); the factor is
    /// `value / 100`, the range `Capability::rgb_gain_range`.
    pub fn set_rgb_gain(&self, r: i32, g: i32, b: i32) -> Result<()> {
        let f = self.sdk().api().CameraSetGain.ok_or(Error::Unsupported("CameraSetGain"))?;
        // SAFETY: open handle, plain values.
        let status = unsafe { f(self.handle(), r, g, b) };
        self.sdk().check("CameraSetGain", status)
    }

    /// Whether anti-flicker is on (`CameraGetAntiFlick`).
    pub fn anti_flick(&self) -> Result<bool> {
        self.get_i32("CameraGetAntiFlick", self.sdk().api().CameraGetAntiFlick).map(|v| v != 0)
    }

    /// Turns anti-flicker on or off (`CameraSetAntiFlick`). It acts only in
    /// automatic exposure mode.
    pub fn set_anti_flick(&self, on: bool) -> Result<()> {
        self.set_i32("CameraSetAntiFlick", self.sdk().api().CameraSetAntiFlick, i32::from(on))
    }

    /// Mains frequency used by anti-flicker (`CameraGetLightFrequency`).
    pub fn light_frequency(&self) -> Result<LightFrequency> {
        let code = self.get_i32("CameraGetLightFrequency", self.sdk().api().CameraGetLightFrequency)?;
        LightFrequency::from_code(code)
            .ok_or_else(|| Error::InvalidResponse(format!("unknown light frequency code {code}")))
    }

    /// Selects the mains frequency used by anti-flicker (`CameraSetLightFrequency`).
    pub fn set_light_frequency(&self, frequency: LightFrequency) -> Result<()> {
        self.set_i32("CameraSetLightFrequency", self.sdk().api().CameraSetLightFrequency, frequency.code())
    }

    // -- Trigger and timing --------------------------------------------------

    /// Sets the number of frames captured per trigger (`CameraSetTriggerCount`;
    /// default 1), for software and hardware trigger.
    pub fn set_trigger_count(&self, frames: i32) -> Result<()> {
        check_range("trigger count", i64::from(frames), 1, i64::from(i32::MAX))?;
        self.set_i32("CameraSetTriggerCount", self.sdk().api().CameraSetTriggerCount, frames)
    }

    /// Sets the delay between a hardware trigger and the start of capture, in
    /// microseconds (`CameraSetTriggerDelayTime`; only some models).
    pub fn set_trigger_delay_us(&self, us: u32) -> Result<()> {
        self.set_u32("CameraSetTriggerDelayTime", self.sdk().api().CameraSetTriggerDelayTime, us)
    }

    // -- Image geometry -----------------------------------------------------

    /// Selects a resolution (`CameraSetImageResolution`), usually an entry of
    /// [`crate::Capability::image_sizes`] or a value read with
    /// [`Camera::image_resolution`]. Its `raw` record is passed to the SDK.
    pub fn set_image_resolution(&self, resolution: &Resolution) -> Result<()> {
        let f = self
            .sdk()
            .api()
            .CameraSetImageResolution
            .ok_or(Error::Unsupported("CameraSetImageResolution"))?;
        let mut raw = resolution.raw;
        // SAFETY: open handle and a struct of the correct layout, owned by this frame.
        let status = unsafe { f(self.handle(), &mut raw) };
        self.sdk().check("CameraSetImageResolution", status)
    }

    /// Selects a custom region of interest with optional binning, skipping or
    /// resampling (`CameraSetImageResolutionEx` with index 0xFF). The header
    /// does not say whether `width`/`height` are measured before or after
    /// binning; read [`Camera::image_resolution`] afterwards.
    pub fn set_roi(&self, roi: Roi) -> Result<()> {
        roi.validate()?;
        let (mode, mask) = roi.binning.mode_and_mask()?;
        let f = self
            .sdk()
            .api()
            .CameraSetImageResolutionEx
            .ok_or(Error::Unsupported("CameraSetImageResolutionEx"))?;
        // SAFETY: open handle, plain values; zoom 0 means no scaling.
        let status = unsafe { f(self.handle(), 0xff, mode, mask, roi.x, roi.y, roi.width, roi.height, 0, 0) };
        self.sdk().check("CameraSetImageResolutionEx", status)
    }

    /// Whether the ISP mirror is on in one direction (`CameraGetMirror`).
    pub fn mirror(&self, direction: MirrorDirection) -> Result<bool> {
        let f = self.sdk().api().CameraGetMirror.ok_or(Error::Unsupported("CameraGetMirror"))?;
        let mut on: c_int = 0;
        // SAFETY: open handle and a valid out pointer.
        let status = unsafe { f(self.handle(), direction.code(), &mut on) };
        self.sdk().check("CameraGetMirror", status)?;
        Ok(on != 0)
    }

    /// Turns the ISP mirror on or off in one direction (`CameraSetMirror`).
    pub fn set_mirror(&self, direction: MirrorDirection, on: bool) -> Result<()> {
        let f = self.sdk().api().CameraSetMirror.ok_or(Error::Unsupported("CameraSetMirror"))?;
        // SAFETY: open handle, plain values.
        let status = unsafe { f(self.handle(), direction.code(), c_int::from(on)) };
        self.sdk().check("CameraSetMirror", status)
    }

    /// Current rotation (`CameraGetRotate`).
    pub fn rotation(&self) -> Result<Rotation> {
        let code = self.get_i32("CameraGetRotate", self.sdk().api().CameraGetRotate)?;
        Rotation::from_code(code).ok_or_else(|| Error::InvalidResponse(format!("unknown rotation code {code}")))
    }

    /// Rotates the image counterclockwise (`CameraSetRotate`).
    pub fn set_rotation(&self, rotation: Rotation) -> Result<()> {
        self.set_i32("CameraSetRotate", self.sdk().api().CameraSetRotate, rotation.code())
    }

    // -- Image processing ----------------------------------------------------

    /// Sets gamma (`CameraSetGamma`); range `Capability::gamma_range`. Takes
    /// effect only in the parameter-generated LUT mode.
    pub fn set_gamma(&self, gamma: i32) -> Result<()> {
        self.set_i32("CameraSetGamma", self.sdk().api().CameraSetGamma, gamma)
    }

    /// Sets contrast (`CameraSetContrast`); range `Capability::contrast_range`.
    /// Takes effect only in the parameter-generated LUT mode.
    pub fn set_contrast(&self, contrast: i32) -> Result<()> {
        self.set_i32("CameraSetContrast", self.sdk().api().CameraSetContrast, contrast)
    }

    /// Sets saturation (`CameraSetSaturation`); 100 leaves colors unchanged,
    /// range `Capability::saturation_range`, no effect on mono cameras.
    pub fn set_saturation(&self, saturation: i32) -> Result<()> {
        self.set_i32("CameraSetSaturation", self.sdk().api().CameraSetSaturation, saturation)
    }

    /// Sets sharpening (`CameraSetSharpness`); 0 disables it, range
    /// `Capability::sharpness_range`.
    pub fn set_sharpness(&self, sharpness: i32) -> Result<()> {
        self.set_i32("CameraSetSharpness", self.sdk().api().CameraSetSharpness, sharpness)
    }

    /// Black level, 0 to 255 (`CameraGetBlackLevel`).
    pub fn black_level(&self) -> Result<i32> {
        self.get_i32("CameraGetBlackLevel", self.sdk().api().CameraGetBlackLevel)
    }

    /// Sets the black level, 0 to 255, default 0 (`CameraSetBlackLevel`).
    pub fn set_black_level(&self, level: i32) -> Result<()> {
        check_range("black level", i64::from(level), 0, 255)?;
        self.set_i32("CameraSetBlackLevel", self.sdk().api().CameraSetBlackLevel, level)
    }

    /// Whether automatic white balance is on (`CameraGetWbMode`).
    pub fn auto_white_balance(&self) -> Result<bool> {
        self.get_i32("CameraGetWbMode", self.sdk().api().CameraGetWbMode).map(|v| v != 0)
    }

    /// Selects automatic or manual white balance (`CameraSetWbMode`).
    pub fn set_auto_white_balance(&self, on: bool) -> Result<()> {
        self.set_i32("CameraSetWbMode", self.sdk().api().CameraSetWbMode, i32::from(on))
    }

    /// Runs one white balance in manual mode; it applies from the next frame
    /// (`CameraSetOnceWB`).
    pub fn white_balance_once(&self) -> Result<()> {
        self.call0("CameraSetOnceWB", self.sdk().api().CameraSetOnceWB)
    }

    // -- Digital I/O and strobe ---------------------------------------------

    /// Level of an input line, `true` for high (`CameraGetIOStateEx`). Lines
    /// are numbered from 0 to `Capability::input_io_count - 1`.
    pub fn input_io_state(&self, index: i32) -> Result<bool> {
        self.io_state("CameraGetIOStateEx", self.sdk().api().CameraGetIOStateEx, index)
    }

    /// Level of an output line, `true` for high (`CameraGetOutPutIOStateEx`).
    pub fn output_io_state(&self, index: i32) -> Result<bool> {
        self.io_state("CameraGetOutPutIOStateEx", self.sdk().api().CameraGetOutPutIOStateEx, index)
    }

    fn io_state(
        &self,
        function: &'static str,
        f: Option<unsafe extern "system" fn(CameraHandle, c_int, *mut c_uint) -> CameraSdkStatus>,
        index: i32,
    ) -> Result<bool> {
        check_range("I/O line index", i64::from(index), 0, i64::from(i32::MAX))?;
        let f = f.ok_or(Error::Unsupported(function))?;
        let mut state: c_uint = 0;
        // SAFETY: open handle and a valid out pointer.
        let status = unsafe { f(self.handle(), index, &mut state) };
        self.sdk().check(function, status)?;
        Ok(state != 0)
    }

    /// Drives an output line high or low (`CameraSetIOStateEx`). Lines are
    /// numbered from 0 to `Capability::output_io_count - 1`; the line must be
    /// in general-purpose output mode.
    pub fn set_output_io_state(&self, index: i32, high: bool) -> Result<()> {
        check_range("I/O line index", i64::from(index), 0, i64::from(i32::MAX))?;
        let f = self.sdk().api().CameraSetIOStateEx.ok_or(Error::Unsupported("CameraSetIOStateEx"))?;
        // SAFETY: open handle, plain values.
        let status = unsafe { f(self.handle(), index, c_uint::from(high)) };
        self.sdk().check("CameraSetIOStateEx", status)
    }

    /// STROBE output mode (`CameraGetStrobeMode`).
    pub fn strobe_mode(&self) -> Result<StrobeMode> {
        let code = self.get_i32("CameraGetStrobeMode", self.sdk().api().CameraGetStrobeMode)?;
        StrobeMode::from_code(code).ok_or_else(|| Error::InvalidResponse(format!("unknown strobe mode {code}")))
    }

    /// Sets the STROBE output mode (`CameraSetStrobeMode`).
    pub fn set_strobe_mode(&self, mode: StrobeMode) -> Result<()> {
        self.set_i32("CameraSetStrobeMode", self.sdk().api().CameraSetStrobeMode, mode.code())
    }

    /// Sets the STROBE active level in trigger-synchronized modes
    /// (`CameraSetStrobePolarity`; default active high).
    pub fn set_strobe_polarity(&self, active_high: bool) -> Result<()> {
        self.set_i32("CameraSetStrobePolarity", self.sdk().api().CameraSetStrobePolarity, i32::from(active_high))
    }

    /// Sets the STROBE delay after the trigger, in microseconds
    /// (`CameraSetStrobeDelayTime`; [`StrobeMode::TriggerManual`]).
    pub fn set_strobe_delay_us(&self, us: u32) -> Result<()> {
        self.set_u32("CameraSetStrobeDelayTime", self.sdk().api().CameraSetStrobeDelayTime, us)
    }

    /// Sets the STROBE pulse width in microseconds
    /// (`CameraSetStrobePulseWidth`; [`StrobeMode::TriggerManual`]).
    pub fn set_strobe_pulse_width_us(&self, us: u32) -> Result<()> {
        self.set_u32("CameraSetStrobePulseWidth", self.sdk().api().CameraSetStrobePulseWidth, us)
    }

    // -- GigE transport ------------------------------------------------------

    /// Sets how often a GigE camera resends a frame the host did not confirm,
    /// in trigger mode; `<= 0` disables resends (`CameraSetFrameResendCount`).
    /// The SDK has no getter for this value.
    pub fn set_frame_resend_count(&self, count: i32) -> Result<()> {
        self.set_i32("CameraSetFrameResendCount", self.sdk().api().CameraSetFrameResendCount, count)
    }

    // -- Parameter persistence ----------------------------------------------

    /// Where parameter groups are stored (`CameraGetParameterMode`).
    pub fn parameter_mode(&self) -> Result<ParameterMode> {
        let code = self.get_i32("CameraGetParameterMode", self.sdk().api().CameraGetParameterMode)?;
        ParameterMode::from_code(code)
            .ok_or_else(|| Error::InvalidResponse(format!("unknown parameter mode {code}")))
    }

    /// Selects where parameter groups are stored (`CameraSetParameterMode`).
    pub fn set_parameter_mode(&self, mode: ParameterMode) -> Result<()> {
        self.set_i32("CameraSetParameterMode", self.sdk().api().CameraSetParameterMode, mode.code())
    }

    /// The parameter group currently in use (`CameraGetCurrentParameterGroup`).
    pub fn current_parameter_team(&self) -> Result<ParameterTeam> {
        let code = self.get_i32("CameraGetCurrentParameterGroup", self.sdk().api().CameraGetCurrentParameterGroup)?;
        ParameterTeam::from_code(code)
            .ok_or_else(|| Error::InvalidResponse(format!("unknown parameter group {code}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_codes() {
        for r in [Rotation::None, Rotation::Ccw90, Rotation::Ccw180, Rotation::Ccw270] {
            assert_eq!(Rotation::from_code(r.code()), Some(r));
        }
        assert_eq!(Rotation::Ccw270.code(), 3);
        assert_eq!(Rotation::from_code(4), None);
    }

    #[test]
    fn mirror_codes() {
        assert_eq!(MirrorDirection::Horizontal.code(), 0);
        assert_eq!(MirrorDirection::Vertical.code(), 1);
    }

    #[test]
    fn light_frequency_codes() {
        assert_eq!(LightFrequency::Hz50.code(), 0);
        assert_eq!(LightFrequency::Hz60.code(), 1);
        assert_eq!(LightFrequency::from_code(1), Some(LightFrequency::Hz60));
        assert_eq!(LightFrequency::from_code(2), None);
    }

    #[test]
    fn parameter_mode_codes() {
        for m in [ParameterMode::ByModel, ParameterMode::ByName, ParameterMode::BySerial, ParameterMode::InDevice] {
            assert_eq!(ParameterMode::from_code(m.code()), Some(m));
        }
        assert_eq!(ParameterMode::BySerial.code(), 2);
        assert_eq!(ParameterMode::from_code(9), None);
    }

    #[test]
    fn strobe_codes() {
        for m in [StrobeMode::TriggerAuto, StrobeMode::TriggerManual, StrobeMode::AlwaysHigh, StrobeMode::AlwaysLow] {
            assert_eq!(StrobeMode::from_code(m.code()), Some(m));
        }
        assert_eq!(StrobeMode::AlwaysLow.code(), 3);
        assert_eq!(StrobeMode::from_code(-1), None);
    }

    #[test]
    fn binning_masks() {
        assert_eq!(Binning::None.mode_and_mask().unwrap(), (0, 0));
        assert_eq!(Binning::Sum(2).mode_and_mask().unwrap(), (1, 0b1));
        assert_eq!(Binning::Average(3).mode_and_mask().unwrap(), (2, 0b10));
        assert_eq!(Binning::Skip(4).mode_and_mask().unwrap(), (3, 0b100));
        assert_eq!(Binning::Resample(17).mode_and_mask().unwrap(), (4, 1 << 15));
        assert!(Binning::Sum(1).mode_and_mask().is_err());
        assert!(Binning::Skip(18).mode_and_mask().is_err());
    }

    #[test]
    fn roi_validation() {
        let ok = Roi { x: 0, y: 0, width: 640, height: 480, binning: Binning::None };
        assert!(ok.validate().is_ok());
        assert!(Roi { x: -1, ..ok }.validate().is_err());
        assert!(Roi { width: 0, ..ok }.validate().is_err());
        assert!(Roi { height: -4, ..ok }.validate().is_err());
    }

    #[test]
    fn range_checks() {
        assert!(check_range("black level", 0, 0, 255).is_ok());
        assert!(check_range("black level", 255, 0, 255).is_ok());
        let err = check_range("black level", 256, 0, 255).unwrap_err().to_string();
        assert!(err.contains("black level must be 0 to 255, got 256"), "{err}");
        assert!(check_range("trigger count", 0, 1, i64::from(i32::MAX)).is_err());
    }
}
