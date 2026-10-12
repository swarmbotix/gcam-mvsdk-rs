//! Raw FFI layer for the HuaTeng Vision MVSDK.
//!
//! Nothing in this module links against the vendor library at build time. The
//! [`Api`] struct is filled at runtime by [`Api::load`], which opens
//! `libMVSDK.so` (Linux) or `MVCAMSDK_X64.dll` / `MVCAMSDK.dll` (Windows) with
//! `libloading` and resolves every symbol once.
//!
//! The struct layouts below are written by hand from the public interface of
//! the SDK, in the same way the vendor's own ctypes wrapper describes them.
//! Their sizes are checked by unit tests against values measured with a C
//! compiler (see `tests/struct_sizes.rs`).
//!
//! All function pointers use `extern "system"`, which is the C calling
//! convention everywhere except 32-bit Windows, where the vendor DLL uses
//! `stdcall`.

#![allow(non_camel_case_types, non_snake_case)]

use std::ffi::{c_char, c_int, c_uint, c_void};
use std::path::{Path, PathBuf};

pub use crate::error::LoadError;

// ---------------------------------------------------------------------------
// Scalar types
// ---------------------------------------------------------------------------

/// Camera handle returned by `CameraInit`.
pub type CameraHandle = c_int;
/// Status code returned by most SDK functions (`0` means success).
pub type CameraSdkStatus = c_int;

/// Frame callback type used by `CameraSetCallbackFunction`.
pub type CameraSnapProc = Option<
    unsafe extern "system" fn(
        hCamera: CameraHandle,
        pFrameBuffer: *mut u8,
        pFrameHead: *mut tSdkFrameHead,
        pContext: *mut c_void,
    ),
>;

/// Frame event callback type used by `CameraSetFrameEventCallback`.
pub type CameraFrameEventCallback =
    Option<unsafe extern "system" fn(hCamera: CameraHandle, pEvent: *mut tSdkFrameEvent, pContext: *mut c_void)>;

/// Connection status callback type used by `CameraSetConnectionStatusCallback`.
/// `MSG` is 0 when the connection is lost and 1 when it is restored.
pub type CameraConnectionStatusCallback =
    Option<unsafe extern "system" fn(hCamera: CameraHandle, MSG: c_uint, uParam: c_uint, pContext: *mut c_void)>;

// ---------------------------------------------------------------------------
// Status codes
// ---------------------------------------------------------------------------

pub const CAMERA_STATUS_SUCCESS: CameraSdkStatus = 0;
pub const CAMERA_STATUS_FAILED: CameraSdkStatus = -1;
pub const CAMERA_STATUS_INTERNAL_ERROR: CameraSdkStatus = -2;
pub const CAMERA_STATUS_UNKNOW: CameraSdkStatus = -3;
pub const CAMERA_STATUS_NOT_SUPPORTED: CameraSdkStatus = -4;
pub const CAMERA_STATUS_NOT_INITIALIZED: CameraSdkStatus = -5;
pub const CAMERA_STATUS_PARAMETER_INVALID: CameraSdkStatus = -6;
pub const CAMERA_STATUS_PARAMETER_OUT_OF_BOUND: CameraSdkStatus = -7;
pub const CAMERA_STATUS_UNENABLED: CameraSdkStatus = -8;
pub const CAMERA_STATUS_USER_CANCEL: CameraSdkStatus = -9;
pub const CAMERA_STATUS_PATH_NOT_FOUND: CameraSdkStatus = -10;
pub const CAMERA_STATUS_SIZE_DISMATCH: CameraSdkStatus = -11;
pub const CAMERA_STATUS_TIME_OUT: CameraSdkStatus = -12;
pub const CAMERA_STATUS_IO_ERROR: CameraSdkStatus = -13;
pub const CAMERA_STATUS_COMM_ERROR: CameraSdkStatus = -14;
pub const CAMERA_STATUS_BUS_ERROR: CameraSdkStatus = -15;
pub const CAMERA_STATUS_NO_DEVICE_FOUND: CameraSdkStatus = -16;
pub const CAMERA_STATUS_NO_LOGIC_DEVICE_FOUND: CameraSdkStatus = -17;
pub const CAMERA_STATUS_DEVICE_IS_OPENED: CameraSdkStatus = -18;
pub const CAMERA_STATUS_DEVICE_IS_CLOSED: CameraSdkStatus = -19;
pub const CAMERA_STATUS_DEVICE_VEDIO_CLOSED: CameraSdkStatus = -20;
pub const CAMERA_STATUS_NO_MEMORY: CameraSdkStatus = -21;
pub const CAMERA_STATUS_FILE_CREATE_FAILED: CameraSdkStatus = -22;
pub const CAMERA_STATUS_FILE_INVALID: CameraSdkStatus = -23;
pub const CAMERA_STATUS_WRITE_PROTECTED: CameraSdkStatus = -24;
pub const CAMERA_STATUS_GRAB_FAILED: CameraSdkStatus = -25;
pub const CAMERA_STATUS_LOST_DATA: CameraSdkStatus = -26;
pub const CAMERA_STATUS_EOF_ERROR: CameraSdkStatus = -27;
pub const CAMERA_STATUS_BUSY: CameraSdkStatus = -28;
pub const CAMERA_STATUS_WAIT: CameraSdkStatus = -29;
pub const CAMERA_STATUS_IN_PROCESS: CameraSdkStatus = -30;
pub const CAMERA_STATUS_IIC_ERROR: CameraSdkStatus = -31;
pub const CAMERA_STATUS_SPI_ERROR: CameraSdkStatus = -32;
pub const CAMERA_STATUS_USB_CONTROL_ERROR: CameraSdkStatus = -33;
pub const CAMERA_STATUS_USB_BULK_ERROR: CameraSdkStatus = -34;
pub const CAMERA_STATUS_SOCKET_INIT_ERROR: CameraSdkStatus = -35;
pub const CAMERA_STATUS_GIGE_FILTER_INIT_ERROR: CameraSdkStatus = -36;
pub const CAMERA_STATUS_NET_SEND_ERROR: CameraSdkStatus = -37;
pub const CAMERA_STATUS_DEVICE_LOST: CameraSdkStatus = -38;
pub const CAMERA_STATUS_DATA_RECV_LESS: CameraSdkStatus = -39;
pub const CAMERA_STATUS_FUNCTION_LOAD_FAILED: CameraSdkStatus = -40;
pub const CAMERA_STATUS_CRITICAL_FILE_LOST: CameraSdkStatus = -41;
pub const CAMERA_STATUS_SENSOR_ID_DISMATCH: CameraSdkStatus = -42;
pub const CAMERA_STATUS_OUT_OF_RANGE: CameraSdkStatus = -43;
pub const CAMERA_STATUS_REGISTRY_ERROR: CameraSdkStatus = -44;
pub const CAMERA_STATUS_ACCESS_DENY: CameraSdkStatus = -45;
pub const CAMERA_STATUS_CAMERA_NEED_RESET: CameraSdkStatus = -46;
pub const CAMERA_STATUS_ISP_MOUDLE_NOT_INITIALIZED: CameraSdkStatus = -47;
pub const CAMERA_STATUS_ISP_DATA_CRC_ERROR: CameraSdkStatus = -48;
pub const CAMERA_STATUS_MV_TEST_FAILED: CameraSdkStatus = -49;
pub const CAMERA_STATUS_INTERNAL_ERR1: CameraSdkStatus = -50;
pub const CAMERA_STATUS_U3V_NO_CONTROL_EP: CameraSdkStatus = -51;
pub const CAMERA_STATUS_U3V_CONTROL_ERROR: CameraSdkStatus = -52;
pub const CAMERA_STATUS_INVALID_FRIENDLY_NAME: CameraSdkStatus = -53;
pub const CAMERA_STATUS_FORMAT_ERROR: CameraSdkStatus = -54;
pub const CAMERA_STATUS_PCIE_OPEN_ERROR: CameraSdkStatus = -55;
pub const CAMERA_STATUS_PCIE_COMM_ERROR: CameraSdkStatus = -56;
pub const CAMERA_STATUS_PCIE_DDR_ERROR: CameraSdkStatus = -57;

// GigE Vision (AIA) device status codes, passed through by the SDK.
pub const CAMERA_AIA_PACKET_RESEND: CameraSdkStatus = 0x0100;
pub const CAMERA_AIA_NOT_IMPLEMENTED: CameraSdkStatus = 0x8001;
pub const CAMERA_AIA_INVALID_PARAMETER: CameraSdkStatus = 0x8002;
pub const CAMERA_AIA_INVALID_ADDRESS: CameraSdkStatus = 0x8003;
pub const CAMERA_AIA_WRITE_PROTECT: CameraSdkStatus = 0x8004;
pub const CAMERA_AIA_BAD_ALIGNMENT: CameraSdkStatus = 0x8005;
pub const CAMERA_AIA_ACCESS_DENIED: CameraSdkStatus = 0x8006;
pub const CAMERA_AIA_BUSY: CameraSdkStatus = 0x8007;
pub const CAMERA_AIA_DEPRECATED: CameraSdkStatus = 0x8008;
pub const CAMERA_AIA_PACKET_UNAVAILABLE: CameraSdkStatus = 0x800C;
pub const CAMERA_AIA_DATA_OVERRUN: CameraSdkStatus = 0x800D;
pub const CAMERA_AIA_INVALID_HEADER: CameraSdkStatus = 0x800E;
pub const CAMERA_AIA_PACKET_NOT_YET_AVAILABLE: CameraSdkStatus = 0x8010;
pub const CAMERA_AIA_PACKET_AND_PREV_REMOVED_FROM_MEMORY: CameraSdkStatus = 0x8011;
pub const CAMERA_AIA_PACKET_REMOVED_FROM_MEMORY: CameraSdkStatus = 0x8012;
pub const CAMERA_AIA_NO_REF_TIME: CameraSdkStatus = 0x0813;
pub const CAMERA_AIA_PACKET_TEMPORARILY_UNAVAILABLE: CameraSdkStatus = 0x0814;
pub const CAMERA_AIA_OVERFLOW: CameraSdkStatus = 0x0815;
pub const CAMERA_AIA_ACTION_LATE: CameraSdkStatus = 0x0816;
pub const CAMERA_AIA_ERROR: CameraSdkStatus = 0x8FFF;

/// One row of the status code table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusInfo {
    /// Numeric status code as returned by the SDK.
    pub code: CameraSdkStatus,
    /// Symbolic name of the code.
    pub name: &'static str,
    /// Short English description, used when the SDK cannot supply one.
    pub description: &'static str,
}

macro_rules! status_table {
    ($($name:ident => $desc:literal),* $(,)?) => {
        /// Every status code known to this crate, in SDK order.
        pub const STATUS_TABLE: &[StatusInfo] = &[
            $(StatusInfo { code: $name, name: stringify!($name), description: $desc },)*
        ];
    };
}

status_table! {
    CAMERA_STATUS_SUCCESS => "operation succeeded",
    CAMERA_STATUS_FAILED => "operation failed",
    CAMERA_STATUS_INTERNAL_ERROR => "internal error",
    CAMERA_STATUS_UNKNOW => "unknown error",
    CAMERA_STATUS_NOT_SUPPORTED => "function not supported",
    CAMERA_STATUS_NOT_INITIALIZED => "initialization not complete",
    CAMERA_STATUS_PARAMETER_INVALID => "invalid parameter",
    CAMERA_STATUS_PARAMETER_OUT_OF_BOUND => "parameter out of bounds",
    CAMERA_STATUS_UNENABLED => "not enabled",
    CAMERA_STATUS_USER_CANCEL => "cancelled by the user",
    CAMERA_STATUS_PATH_NOT_FOUND => "path not found in the registry",
    CAMERA_STATUS_SIZE_DISMATCH => "image data length does not match the expected size",
    CAMERA_STATUS_TIME_OUT => "timed out",
    CAMERA_STATUS_IO_ERROR => "hardware I/O error",
    CAMERA_STATUS_COMM_ERROR => "communication error",
    CAMERA_STATUS_BUS_ERROR => "bus error",
    CAMERA_STATUS_NO_DEVICE_FOUND => "no device found",
    CAMERA_STATUS_NO_LOGIC_DEVICE_FOUND => "logical device not found",
    CAMERA_STATUS_DEVICE_IS_OPENED => "device is already open",
    CAMERA_STATUS_DEVICE_IS_CLOSED => "device is closed",
    CAMERA_STATUS_DEVICE_VEDIO_CLOSED => "device video stream is not open",
    CAMERA_STATUS_NO_MEMORY => "not enough system memory",
    CAMERA_STATUS_FILE_CREATE_FAILED => "failed to create file",
    CAMERA_STATUS_FILE_INVALID => "invalid file format",
    CAMERA_STATUS_WRITE_PROTECTED => "write protected",
    CAMERA_STATUS_GRAB_FAILED => "image acquisition failed",
    CAMERA_STATUS_LOST_DATA => "data lost or incomplete",
    CAMERA_STATUS_EOF_ERROR => "end-of-frame marker not received",
    CAMERA_STATUS_BUSY => "busy, a previous operation is still in progress",
    CAMERA_STATUS_WAIT => "condition not met yet, retry later",
    CAMERA_STATUS_IN_PROCESS => "operation already in progress",
    CAMERA_STATUS_IIC_ERROR => "I2C transfer error",
    CAMERA_STATUS_SPI_ERROR => "SPI transfer error",
    CAMERA_STATUS_USB_CONTROL_ERROR => "USB control transfer error",
    CAMERA_STATUS_USB_BULK_ERROR => "USB bulk transfer error",
    CAMERA_STATUS_SOCKET_INIT_ERROR => "network socket initialization failed",
    CAMERA_STATUS_GIGE_FILTER_INIT_ERROR => "GigE filter driver initialization failed; reinstall the driver",
    CAMERA_STATUS_NET_SEND_ERROR => "network send error",
    CAMERA_STATUS_DEVICE_LOST => "connection to the GigE camera lost (heartbeat timeout)",
    CAMERA_STATUS_DATA_RECV_LESS => "received fewer bytes than requested",
    CAMERA_STATUS_FUNCTION_LOAD_FAILED => "failed to load function from file",
    CAMERA_STATUS_CRITICAL_FILE_LOST => "a file required by the SDK is missing",
    CAMERA_STATUS_SENSOR_ID_DISMATCH => "firmware does not match the device",
    CAMERA_STATUS_OUT_OF_RANGE => "parameter out of valid range",
    CAMERA_STATUS_REGISTRY_ERROR => "installer registration error; reinstall the SDK",
    CAMERA_STATUS_ACCESS_DENY => "access denied; the camera is in use by another program",
    CAMERA_STATUS_CAMERA_NEED_RESET => "camera must be reset (power cycle the camera)",
    CAMERA_STATUS_ISP_MOUDLE_NOT_INITIALIZED => "ISP module not initialized",
    CAMERA_STATUS_ISP_DATA_CRC_ERROR => "data checksum error",
    CAMERA_STATUS_MV_TEST_FAILED => "data test failed",
    CAMERA_STATUS_INTERNAL_ERR1 => "internal error 1",
    CAMERA_STATUS_U3V_NO_CONTROL_EP => "USB3 Vision control endpoint not found",
    CAMERA_STATUS_U3V_CONTROL_ERROR => "USB3 Vision control communication error",
    CAMERA_STATUS_INVALID_FRIENDLY_NAME => "invalid device name; it must not contain \\ / : * ? \" < > |",
    CAMERA_STATUS_FORMAT_ERROR => "format error",
    CAMERA_STATUS_PCIE_OPEN_ERROR => "failed to open PCIe device",
    CAMERA_STATUS_PCIE_COMM_ERROR => "PCIe device communication failed",
    CAMERA_STATUS_PCIE_DDR_ERROR => "PCIe DDR error",
    CAMERA_AIA_PACKET_RESEND => "GigE Vision: packet must be resent",
    CAMERA_AIA_NOT_IMPLEMENTED => "GigE Vision: command not implemented by the device",
    CAMERA_AIA_INVALID_PARAMETER => "GigE Vision: invalid command parameter",
    CAMERA_AIA_INVALID_ADDRESS => "GigE Vision: address not accessible",
    CAMERA_AIA_WRITE_PROTECT => "GigE Vision: target is write protected",
    CAMERA_AIA_BAD_ALIGNMENT => "GigE Vision: address alignment error",
    CAMERA_AIA_ACCESS_DENIED => "GigE Vision: access denied",
    CAMERA_AIA_BUSY => "GigE Vision: command is being processed",
    CAMERA_AIA_DEPRECATED => "GigE Vision: deprecated command",
    CAMERA_AIA_PACKET_UNAVAILABLE => "GigE Vision: packet unavailable",
    CAMERA_AIA_DATA_OVERRUN => "GigE Vision: data overrun",
    CAMERA_AIA_INVALID_HEADER => "GigE Vision: invalid packet header",
    CAMERA_AIA_PACKET_NOT_YET_AVAILABLE => "GigE Vision: packet not yet available",
    CAMERA_AIA_PACKET_AND_PREV_REMOVED_FROM_MEMORY => "GigE Vision: packet and earlier packets no longer in memory",
    CAMERA_AIA_PACKET_REMOVED_FROM_MEMORY => "GigE Vision: packet no longer in memory",
    CAMERA_AIA_NO_REF_TIME => "GigE Vision: no reference clock",
    CAMERA_AIA_PACKET_TEMPORARILY_UNAVAILABLE => "GigE Vision: packet temporarily unavailable (bandwidth)",
    CAMERA_AIA_OVERFLOW => "GigE Vision: device queue overflow",
    CAMERA_AIA_ACTION_LATE => "GigE Vision: action command arrived too late",
    CAMERA_AIA_ERROR => "GigE Vision: generic device error",
}

/// Looks up a status code in [`STATUS_TABLE`].
pub fn status_info(code: CameraSdkStatus) -> Option<&'static StatusInfo> {
    STATUS_TABLE.iter().find(|s| s.code == code)
}

// ---------------------------------------------------------------------------
// Image formats (media types)
// ---------------------------------------------------------------------------

pub const CAMERA_MEDIA_TYPE_MONO: u32 = 0x0100_0000;
pub const CAMERA_MEDIA_TYPE_RGB: u32 = 0x0200_0000;
pub const CAMERA_MEDIA_TYPE_COLOR: u32 = 0x0200_0000;
pub const CAMERA_MEDIA_TYPE_CUSTOM: u32 = 0x8000_0000;
pub const CAMERA_MEDIA_TYPE_COLOR_MASK: u32 = 0xFF00_0000;
pub const CAMERA_MEDIA_TYPE_OCCUPY1BIT: u32 = 0x0001_0000;
pub const CAMERA_MEDIA_TYPE_OCCUPY2BIT: u32 = 0x0002_0000;
pub const CAMERA_MEDIA_TYPE_OCCUPY4BIT: u32 = 0x0004_0000;
pub const CAMERA_MEDIA_TYPE_OCCUPY8BIT: u32 = 0x0008_0000;
pub const CAMERA_MEDIA_TYPE_OCCUPY10BIT: u32 = 0x000A_0000;
pub const CAMERA_MEDIA_TYPE_OCCUPY12BIT: u32 = 0x000C_0000;
pub const CAMERA_MEDIA_TYPE_OCCUPY16BIT: u32 = 0x0010_0000;
pub const CAMERA_MEDIA_TYPE_OCCUPY24BIT: u32 = 0x0018_0000;
pub const CAMERA_MEDIA_TYPE_OCCUPY32BIT: u32 = 0x0020_0000;
pub const CAMERA_MEDIA_TYPE_OCCUPY36BIT: u32 = 0x0024_0000;
pub const CAMERA_MEDIA_TYPE_OCCUPY48BIT: u32 = 0x0030_0000;
pub const CAMERA_MEDIA_TYPE_OCCUPY64BIT: u32 = 0x0040_0000;
pub const CAMERA_MEDIA_TYPE_EFFECTIVE_PIXEL_SIZE_MASK: u32 = 0x00FF_0000;
pub const CAMERA_MEDIA_TYPE_EFFECTIVE_PIXEL_SIZE_SHIFT: u32 = 16;
pub const CAMERA_MEDIA_TYPE_ID_MASK: u32 = 0x0000_FFFF;

pub const CAMERA_MEDIA_TYPE_MONO8: u32 = CAMERA_MEDIA_TYPE_MONO | CAMERA_MEDIA_TYPE_OCCUPY8BIT | 0x0001;
pub const CAMERA_MEDIA_TYPE_MONO10: u32 = CAMERA_MEDIA_TYPE_MONO | CAMERA_MEDIA_TYPE_OCCUPY16BIT | 0x0003;
pub const CAMERA_MEDIA_TYPE_MONO12: u32 = CAMERA_MEDIA_TYPE_MONO | CAMERA_MEDIA_TYPE_OCCUPY16BIT | 0x0005;
pub const CAMERA_MEDIA_TYPE_MONO16: u32 = CAMERA_MEDIA_TYPE_MONO | CAMERA_MEDIA_TYPE_OCCUPY16BIT | 0x0007;
pub const CAMERA_MEDIA_TYPE_BAYGR8: u32 = CAMERA_MEDIA_TYPE_MONO | CAMERA_MEDIA_TYPE_OCCUPY8BIT | 0x0008;
pub const CAMERA_MEDIA_TYPE_BAYRG8: u32 = CAMERA_MEDIA_TYPE_MONO | CAMERA_MEDIA_TYPE_OCCUPY8BIT | 0x0009;
pub const CAMERA_MEDIA_TYPE_BAYGB8: u32 = CAMERA_MEDIA_TYPE_MONO | CAMERA_MEDIA_TYPE_OCCUPY8BIT | 0x000A;
pub const CAMERA_MEDIA_TYPE_BAYBG8: u32 = CAMERA_MEDIA_TYPE_MONO | CAMERA_MEDIA_TYPE_OCCUPY8BIT | 0x000B;
pub const CAMERA_MEDIA_TYPE_RGB8: u32 = CAMERA_MEDIA_TYPE_COLOR | CAMERA_MEDIA_TYPE_OCCUPY24BIT | 0x0014;
pub const CAMERA_MEDIA_TYPE_BGR8: u32 = CAMERA_MEDIA_TYPE_COLOR | CAMERA_MEDIA_TYPE_OCCUPY24BIT | 0x0015;
pub const CAMERA_MEDIA_TYPE_RGBA8: u32 = CAMERA_MEDIA_TYPE_COLOR | CAMERA_MEDIA_TYPE_OCCUPY32BIT | 0x0016;
pub const CAMERA_MEDIA_TYPE_BGRA8: u32 = CAMERA_MEDIA_TYPE_COLOR | CAMERA_MEDIA_TYPE_OCCUPY32BIT | 0x0017;

/// Number of bits one pixel occupies for a media type (0 when unknown).
pub const fn media_type_bits_per_pixel(media_type: u32) -> u32 {
    (media_type & CAMERA_MEDIA_TYPE_EFFECTIVE_PIXEL_SIZE_MASK) >> CAMERA_MEDIA_TYPE_EFFECTIVE_PIXEL_SIZE_SHIFT
}

/// Whether a media type is a single-channel (mono or raw Bayer) format.
pub const fn media_type_is_mono(media_type: u32) -> bool {
    media_type & CAMERA_MEDIA_TYPE_COLOR_MASK == CAMERA_MEDIA_TYPE_MONO
}

// ---------------------------------------------------------------------------
// Parameter-loading constants used by CameraInit / CameraSaveParameter
// ---------------------------------------------------------------------------

/// Parameter team selector: the factory default set.
pub const PARAMETER_TEAM_DEFAULT: c_int = 0xff;
pub const PARAMETER_TEAM_A: c_int = 0;
pub const PARAMETER_TEAM_B: c_int = 1;
pub const PARAMETER_TEAM_C: c_int = 2;
pub const PARAMETER_TEAM_D: c_int = 3;

/// Parameter loading mode for `CameraInit`: by model name.
pub const PARAM_MODE_BY_MODEL: c_int = 0;
/// Parameter loading mode for `CameraInit`: by device nickname.
pub const PARAM_MODE_BY_NAME: c_int = 1;
/// Parameter loading mode for `CameraInit`: by serial number.
pub const PARAM_MODE_BY_SN: c_int = 2;
/// Parameter loading mode for `CameraInit`: from the camera's own storage.
pub const PARAM_MODE_IN_DEVICE: c_int = 3;

// ---------------------------------------------------------------------------
// Structures
// ---------------------------------------------------------------------------

/// Enumeration record describing one device (260 bytes on every target).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct tSdkCameraDevInfo {
    pub acProductSeries: [c_char; 32],
    pub acProductName: [c_char; 32],
    pub acFriendlyName: [c_char; 32],
    pub acLinkName: [c_char; 32],
    pub acDriverVersion: [c_char; 32],
    pub acSensorType: [c_char; 32],
    pub acPortType: [c_char; 32],
    pub acSn: [c_char; 32],
    pub uInstance: c_uint,
}

/// Header accompanying every image buffer (64 bytes on every target).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct tSdkFrameHead {
    pub uiMediaType: c_uint,
    pub uBytes: c_uint,
    pub iWidth: c_int,
    pub iHeight: c_int,
    pub iWidthZoomSw: c_int,
    pub iHeightZoomSw: c_int,
    pub bIsTrigger: c_int,
    /// Acquisition time stamp in units of 0.1 ms.
    pub uiTimeStamp: c_uint,
    /// Exposure of this frame in microseconds.
    pub uiExpTime: c_uint,
    pub fAnalogGain: f32,
    pub iGamma: c_int,
    pub iContrast: c_int,
    pub iSaturation: c_int,
    pub fRgain: f32,
    pub fGgain: f32,
    pub fBgain: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct tSdkRect {
    pub iLeft: c_int,
    pub iTop: c_int,
    pub iWidth: c_int,
    pub iHeight: c_int,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct tSdkResolutionRange {
    pub iHeightMax: c_int,
    pub iHeightMin: c_int,
    pub iWidthMax: c_int,
    pub iWidthMin: c_int,
    pub uSkipModeMask: c_uint,
    pub uBinSumModeMask: c_uint,
    pub uBinAverageModeMask: c_uint,
    pub uResampleMask: c_uint,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct tSdkImageResolution {
    pub iIndex: c_int,
    pub acDescription: [c_char; 32],
    pub uBinSumMode: c_uint,
    pub uBinAverageMode: c_uint,
    pub uSkipMode: c_uint,
    pub uResampleMask: c_uint,
    pub iHOffsetFOV: c_int,
    pub iVOffsetFOV: c_int,
    pub iWidthFOV: c_int,
    pub iHeightFOV: c_int,
    pub iWidth: c_int,
    pub iHeight: c_int,
    pub iWidthZoomHd: c_int,
    pub iHeightZoomHd: c_int,
    pub iWidthZoomSw: c_int,
    pub iHeightZoomSw: c_int,
}

/// Index plus 32-byte description; shared by several descriptor tables.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct tSdkIndexedDescription {
    pub iIndex: c_int,
    pub acDescription: [c_char; 32],
}

pub type tSdkColorTemperatureDes = tSdkIndexedDescription;
pub type tSdkFrameSpeed = tSdkIndexedDescription;
pub type tSdkTrigger = tSdkIndexedDescription;
pub type tSdkPresetLut = tSdkIndexedDescription;
pub type tSdkAeAlgorithm = tSdkIndexedDescription;
pub type tSdkBayerDecodeAlgorithm = tSdkIndexedDescription;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct tSdkExpose {
    pub uiTargetMin: c_uint,
    pub uiTargetMax: c_uint,
    pub uiAnalogGainMin: c_uint,
    pub uiAnalogGainMax: c_uint,
    pub fAnalogGainStep: f32,
    pub uiExposeTimeMin: c_uint,
    pub uiExposeTimeMax: c_uint,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct tSdkPackLength {
    pub iIndex: c_int,
    pub acDescription: [c_char; 32],
    /// Packet size in bytes.
    pub iPackSize: c_uint,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct tSdkFrameStatistic {
    pub iTotal: c_int,
    pub iCapture: c_int,
    pub iLost: c_int,
}

/// Frame event passed to a `CameraSetFrameEventCallback` callback (32 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct tSdkFrameEvent {
    /// 1: frame start, 2: frame end.
    pub uType: c_uint,
    /// 0: success, otherwise an error.
    pub uStatus: c_uint,
    pub uFrameID: c_uint,
    pub uWidth: c_uint,
    pub uHeight: c_uint,
    pub uPixelFormat: c_uint,
    /// Low 32 bits of the time stamp.
    pub TimeStampL: c_uint,
    /// High 32 bits of the time stamp.
    pub TimeStampH: c_uint,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct tSdkMediaType {
    pub iIndex: c_int,
    pub acDescription: [c_char; 32],
    pub iMediaType: c_uint,
}

/// Minimum and maximum of an integer setting.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct tSdkIntRange {
    pub iMin: c_int,
    pub iMax: c_int,
}

pub type tGammaRange = tSdkIntRange;
pub type tContrastRange = tSdkIntRange;
pub type tSaturationRange = tSdkIntRange;
pub type tSharpnessRange = tSdkIntRange;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct tRgbGainRange {
    pub iRGainMin: c_int,
    pub iRGainMax: c_int,
    pub iGGainMin: c_int,
    pub iGGainMax: c_int,
    pub iBGainMin: c_int,
    pub iBGainMax: c_int,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct tSdkIspCapacity {
    pub bMonoSensor: c_int,
    pub bWbOnce: c_int,
    pub bAutoWb: c_int,
    pub bAutoExposure: c_int,
    pub bManualExposure: c_int,
    pub bAntiFlick: c_int,
    pub bDeviceIsp: c_int,
    pub bForceUseDeviceIsp: c_int,
    pub bZoomHD: c_int,
}

/// Capability description filled by `CameraGetCapability`.
///
/// The descriptor pointers reference memory owned by the SDK; they stay valid
/// while the camera handle is open. Size: 344 bytes on 64-bit targets, 256 on
/// 32-bit targets.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct tSdkCameraCapbility {
    pub pTriggerDesc: *mut tSdkTrigger,
    pub iTriggerDesc: c_int,
    pub pImageSizeDesc: *mut tSdkImageResolution,
    pub iImageSizeDesc: c_int,
    pub pClrTempDesc: *mut tSdkColorTemperatureDes,
    pub iClrTempDesc: c_int,
    pub pMediaTypeDesc: *mut tSdkMediaType,
    pub iMediaTypdeDesc: c_int,
    pub pFrameSpeedDesc: *mut tSdkFrameSpeed,
    pub iFrameSpeedDesc: c_int,
    pub pPackLenDesc: *mut tSdkPackLength,
    pub iPackLenDesc: c_int,
    pub iOutputIoCounts: c_int,
    pub iInputIoCounts: c_int,
    pub pPresetLutDesc: *mut tSdkPresetLut,
    pub iPresetLut: c_int,
    pub iUserDataMaxLen: c_int,
    pub bParamInDevice: c_int,
    pub pAeAlmSwDesc: *mut tSdkAeAlgorithm,
    pub iAeAlmSwDesc: c_int,
    pub pAeAlmHdDesc: *mut tSdkAeAlgorithm,
    pub iAeAlmHdDesc: c_int,
    pub pBayerDecAlmSwDesc: *mut tSdkBayerDecodeAlgorithm,
    pub iBayerDecAlmSwDesc: c_int,
    pub pBayerDecAlmHdDesc: *mut tSdkBayerDecodeAlgorithm,
    pub iBayerDecAlmHdDesc: c_int,
    pub sExposeDesc: tSdkExpose,
    pub sResolutionRange: tSdkResolutionRange,
    pub sRgbGainRange: tRgbGainRange,
    pub sSaturationRange: tSaturationRange,
    pub sGammaRange: tGammaRange,
    pub sContrastRange: tContrastRange,
    pub sSharpnessRange: tSharpnessRange,
    pub sIspCapacity: tSdkIspCapacity,
}

impl Default for tSdkCameraCapbility {
    fn default() -> Self {
        // SAFETY: every field is an integer, a float, a raw pointer, or a struct
        // of those; the all-zero bit pattern is a valid value for each of them.
        unsafe { std::mem::zeroed() }
    }
}

// ---------------------------------------------------------------------------
// Text helpers
// ---------------------------------------------------------------------------

/// Copies a NUL-terminated `c_char` array into bytes, stopping at the first NUL.
pub fn c_chars_to_bytes(chars: &[c_char]) -> Vec<u8> {
    chars.iter().take_while(|&&c| c != 0).map(|&c| c as u8).collect()
}

/// Decodes bytes produced by the SDK.
///
/// Pure ASCII is returned as is. Anything else is decoded as GBK, the code
/// page used by the vendor for Chinese text. The bytes are never assumed to be
/// UTF-8.
pub fn decode_sdk_bytes(bytes: &[u8]) -> String {
    let bytes = match bytes.iter().position(|&b| b == 0) {
        Some(n) => &bytes[..n],
        None => bytes,
    };
    if bytes.is_ascii() {
        // ASCII is valid UTF-8.
        return String::from_utf8_lossy(bytes).into_owned();
    }
    let (text, _had_errors) = encoding_rs::GBK.decode_without_bom_handling(bytes);
    text.into_owned()
}

/// Decodes a fixed-size `c_char` array from an SDK structure.
pub fn decode_c_chars(chars: &[c_char]) -> String {
    decode_sdk_bytes(&c_chars_to_bytes(chars))
}

/// Decodes a NUL-terminated string returned by pointer from the SDK.
///
/// # Safety
/// `ptr` must be null or point to a NUL-terminated string that stays valid
/// for the duration of the call.
pub unsafe fn decode_c_str(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let bytes = std::ffi::CStr::from_ptr(ptr).to_bytes();
    Some(decode_sdk_bytes(bytes))
}

/// Encodes text for the SDK: ASCII as is, anything else as GBK.
///
/// Returns `None` when the text contains a NUL byte or a character that GBK
/// cannot represent.
pub fn encode_sdk_text(text: &str) -> Option<Vec<u8>> {
    if text.contains('\0') {
        return None;
    }
    if text.is_ascii() {
        return Some(text.as_bytes().to_vec());
    }
    let (bytes, _, had_errors) = encoding_rs::GBK.encode(text);
    if had_errors {
        None
    } else {
        Some(bytes.into_owned())
    }
}

// ---------------------------------------------------------------------------
// Function table
// ---------------------------------------------------------------------------

macro_rules! define_api {
    (
        required {
            $( $(#[doc = $rdoc:literal])* fn $rname:ident ( $($rarg:ident : $rty:ty),* $(,)? ) $(-> $rret:ty)? ; )*
        }
        optional {
            $( $(#[doc = $odoc:literal])* fn $oname:ident ( $($oarg:ident : $oty:ty),* $(,)? ) $(-> $oret:ty)? ; )*
        }
    ) => {
        /// Function table resolved from the vendor library at runtime.
        ///
        /// Field names match the C function names. Required functions are plain
        /// function pointers; [`Api::load`] fails if any of them is missing.
        /// Optional functions are `Option`s, `None` when the loaded build of the
        /// SDK does not export them. Functions not listed here can be resolved
        /// with [`Api::symbol`].
        ///
        /// Calling any of these pointers is `unsafe`: the caller must uphold the
        /// SDK's own contract (valid handles, buffers of the documented size).
        pub struct Api {
            path: PathBuf,
            $( $(#[doc = $rdoc])* pub $rname: unsafe extern "system" fn($($rarg: $rty),*) $(-> $rret)?, )*
            $( $(#[doc = $odoc])* pub $oname: Option<unsafe extern "system" fn($($oarg: $oty),*) $(-> $oret)?>, )*
            // Keeps the library mapped for as long as the pointers above exist.
            lib: libloading::Library,
        }

        impl Api {
            /// Names of the functions [`Api::load`] requires.
            pub const REQUIRED_SYMBOLS: &'static [&'static str] = &[$(stringify!($rname)),*];
            /// Names of the functions [`Api::load`] resolves when present.
            pub const OPTIONAL_SYMBOLS: &'static [&'static str] = &[$(stringify!($oname)),*];

            /// Opens the vendor library at `path` (or a bare file name, which is
            /// resolved by the system loader) and resolves the function table.
            ///
            /// This does not call `CameraSdkInit`; [`crate::Sdk::load`] does.
            ///
            /// # Safety
            /// Loading a shared library runs its initialisation code. The file
            /// must be a genuine MVSDK library whose ABI matches these
            /// declarations.
            pub unsafe fn load(path: impl AsRef<Path>) -> Result<Api, LoadError> {
                let path = path.as_ref().to_path_buf();
                let lib = libloading::Library::new(&path).map_err(|source| LoadError::Open {
                    path: path.clone(),
                    source,
                })?;
                $(
                    let $rname = *lib
                        .get::<unsafe extern "system" fn($($rty),*) $(-> $rret)?>(
                            concat!(stringify!($rname), "\0").as_bytes(),
                        )
                        .map_err(|source| LoadError::MissingSymbol {
                            path: path.clone(),
                            symbol: stringify!($rname),
                            source,
                        })?;
                )*
                $(
                    let $oname = lib
                        .get::<unsafe extern "system" fn($($oty),*) $(-> $oret)?>(
                            concat!(stringify!($oname), "\0").as_bytes(),
                        )
                        .ok()
                        .map(|s| *s);
                )*
                Ok(Api { path, $($rname,)* $($oname,)* lib })
            }

            /// Names of optional functions the loaded library does not export.
            pub fn missing_optional_symbols(&self) -> Vec<&'static str> {
                let mut missing = Vec::new();
                $( if self.$oname.is_none() { missing.push(stringify!($oname)); } )*
                missing
            }
        }
    };
}

define_api! {
    required {
        /// Initialises the SDK; `iLanguageSel` selects the message language.
        fn CameraSdkInit(iLanguageSel: c_int) -> CameraSdkStatus;
        /// Enumerates devices. `piNums` is in/out: array capacity on entry,
        /// number of devices found on return.
        fn CameraEnumerateDevice(pCameraList: *mut tSdkCameraDevInfo, piNums: *mut c_int) -> CameraSdkStatus;
        /// Opens a device. `-1, -1` reuses the last parameter mode and team.
        fn CameraInit(pCameraInfo: *mut tSdkCameraDevInfo, emParamLoadMode: c_int, emTeam: c_int, pCameraHandle: *mut CameraHandle) -> CameraSdkStatus;
        /// Closes a device handle.
        fn CameraUnInit(hCamera: CameraHandle) -> CameraSdkStatus;
        /// Fills the capability description of an open camera.
        fn CameraGetCapability(hCamera: CameraHandle, pCameraInfo: *mut tSdkCameraCapbility) -> CameraSdkStatus;
        /// Reads the user-defined name into a buffer of at least 32 bytes.
        fn CameraGetFriendlyName(hCamera: CameraHandle, pName: *mut c_char) -> CameraSdkStatus;
        /// Writes the user-defined name (at most 32 bytes including NUL) to camera storage.
        fn CameraSetFriendlyName(hCamera: CameraHandle, pName: *mut c_char) -> CameraSdkStatus;
        /// Reads camera and host-NIC IP, mask, gateway; each buffer at least 16 bytes.
        fn CameraGigeGetIp(pCameraInfo: *mut tSdkCameraDevInfo, CamIp: *mut c_char, CamMask: *mut c_char, CamGateWay: *mut c_char, EtIp: *mut c_char, EtMask: *mut c_char, EtGateWay: *mut c_char) -> CameraSdkStatus;
        /// Assigns the camera IP configuration; `bPersistent = 0` selects automatic addressing.
        fn CameraGigeSetIp(pCameraInfo: *mut tSdkCameraDevInfo, Ip: *const c_char, SubMask: *const c_char, GateWay: *const c_char, bPersistent: c_int) -> CameraSdkStatus;
        /// Reads camera and host-NIC MAC addresses; each buffer at least 18 bytes.
        fn CameraGigeGetMac(pCameraInfo: *mut tSdkCameraDevInfo, CamMac: *mut c_char, EtMac: *mut c_char) -> CameraSdkStatus;
        /// Enables (1) or disables (0) automatic exposure.
        fn CameraSetAeState(hCamera: CameraHandle, bAeState: c_int) -> CameraSdkStatus;
        /// Sets the exposure time in microseconds.
        fn CameraSetExposureTime(hCamera: CameraHandle, fExposureTime: f64) -> CameraSdkStatus;
        /// Reads the exposure time in microseconds.
        fn CameraGetExposureTime(hCamera: CameraHandle, pfExposureTime: *mut f64) -> CameraSdkStatus;
        /// Caps the frame rate in Hz; values `<= 0` remove the cap.
        fn CameraSetFrameRate(hCamera: CameraHandle, RateHZ: c_int) -> CameraSdkStatus;
        /// Selects the ISP output format (`CAMERA_MEDIA_TYPE_*`).
        fn CameraSetIspOutFormat(hCamera: CameraHandle, uFormat: c_uint) -> CameraSdkStatus;
        /// Selects continuous (0), software (1) or hardware (2) trigger mode.
        fn CameraSetTriggerMode(hCamera: CameraHandle, iModeSel: c_int) -> CameraSdkStatus;
        /// Starts streaming.
        fn CameraPlay(hCamera: CameraHandle) -> CameraSdkStatus;
        /// Pauses streaming.
        fn CameraPause(hCamera: CameraHandle) -> CameraSdkStatus;
        /// Waits up to `wTimes` ms for a raw frame; the buffer must be released.
        fn CameraGetImageBuffer(hCamera: CameraHandle, pFrameInfo: *mut tSdkFrameHead, pbyBuffer: *mut *mut u8, wTimes: c_uint) -> CameraSdkStatus;
        /// Returns a buffer obtained from `CameraGetImageBuffer`.
        fn CameraReleaseImageBuffer(hCamera: CameraHandle, pbyBuffer: *mut u8) -> CameraSdkStatus;
        /// Runs the ISP on a raw frame into `pbyOut` using the ISP output format.
        fn CameraImageProcess(hCamera: CameraHandle, pbyIn: *mut u8, pbyOut: *mut u8, pFrInfo: *mut tSdkFrameHead) -> CameraSdkStatus;
        /// Selects the GigE transport packet size by index into the capability table.
        fn CameraSetTransPackLen(hCamera: CameraHandle, iPackSel: c_int) -> CameraSdkStatus;
        /// Returns a static description for a status code.
        fn CameraGetErrorString(iStatusCode: CameraSdkStatus) -> *const c_char;
        /// Saves the current parameters to a team (A to D).
        fn CameraSaveParameter(hCamera: CameraHandle, iTeam: c_int) -> CameraSdkStatus;
        /// Loads parameters from a team (A to D, or default).
        fn CameraLoadParameter(hCamera: CameraHandle, iTeam: c_int) -> CameraSdkStatus;
    }
    optional {
        /// Writes the SDK version into a buffer of more than 32 bytes.
        fn CameraSdkGetVersionString(pVersionString: *mut c_char) -> CameraSdkStatus;
        /// Sets the directory where the SDK stores per-camera data files.
        fn CameraSetDataDirectory(dirname: *const c_char) -> CameraSdkStatus;
        /// Reports whether a device is already opened by some process.
        fn CameraIsOpened(pCameraInfo: *mut tSdkCameraDevInfo, pOpened: *mut c_int) -> CameraSdkStatus;
        /// Enumerates GigE devices at the given unicast addresses.
        fn CameraGigeEnumerateDevice(ppIpList: *const *const c_char, numIp: c_int, pCameraList: *mut tSdkCameraDevInfo, piNums: *mut c_int) -> CameraSdkStatus;
        /// Sets a process-wide GigE option such as `NumBuffers`.
        fn CameraGigeSetOption(optionName: *const c_char, value: *const c_char) -> CameraSdkStatus;
        /// Reads the automatic exposure state.
        fn CameraGetAeState(hCamera: CameraHandle, pAeState: *mut c_int) -> CameraSdkStatus;
        /// Reads the exposure range and step in microseconds.
        fn CameraGetExposureTimeRange(hCamera: CameraHandle, pfMin: *mut f64, pfMax: *mut f64, pfStep: *mut f64) -> CameraSdkStatus;
        /// Reads the duration of one sensor line in microseconds.
        fn CameraGetExposureLineTime(hCamera: CameraHandle, pfLineTime: *mut f64) -> CameraSdkStatus;
        /// Reads the frame-rate cap in Hz.
        fn CameraGetFrameRate(hCamera: CameraHandle, RateHZ: *mut c_int) -> CameraSdkStatus;
        /// Selects a frame-speed tier by index.
        fn CameraSetFrameSpeed(hCamera: CameraHandle, iFrameSpeed: c_int) -> CameraSdkStatus;
        /// Reads the frame-speed tier index.
        fn CameraGetFrameSpeed(hCamera: CameraHandle, piFrameSpeed: *mut c_int) -> CameraSdkStatus;
        /// Reads the ISP output format.
        fn CameraGetIspOutFormat(hCamera: CameraHandle, puFormat: *mut c_uint) -> CameraSdkStatus;
        /// Reads the trigger mode.
        fn CameraGetTriggerMode(hCamera: CameraHandle, piModeSel: *mut c_int) -> CameraSdkStatus;
        /// Reads the trigger delay in microseconds.
        fn CameraGetTriggerDelayTime(hCamera: CameraHandle, puDelayTimeUs: *mut c_uint) -> CameraSdkStatus;
        /// Reads the number of frames captured per trigger.
        fn CameraGetTriggerCount(hCamera: CameraHandle, piCount: *mut c_int) -> CameraSdkStatus;
        /// Issues a software trigger.
        fn CameraSoftTrigger(hCamera: CameraHandle) -> CameraSdkStatus;
        /// Stops streaming.
        fn CameraStop(hCamera: CameraHandle) -> CameraSdkStatus;
        /// Reads the selected packet-size index.
        fn CameraGetTransPackLen(hCamera: CameraHandle, piPackSel: *mut c_int) -> CameraSdkStatus;
        /// Reads the current output resolution.
        fn CameraGetImageResolution(hCamera: CameraHandle, psCurVideoSize: *mut tSdkImageResolution) -> CameraSdkStatus;
        /// Reads the analog gain in SDK units.
        fn CameraGetAnalogGain(hCamera: CameraHandle, piAnalogGain: *mut c_int) -> CameraSdkStatus;
        /// Reads the digital R, G, B gains (100 = 1.0x).
        fn CameraGetGain(hCamera: CameraHandle, piRGain: *mut c_int, piGGain: *mut c_int, piBGain: *mut c_int) -> CameraSdkStatus;
        /// Reads the gamma setting.
        fn CameraGetGamma(hCamera: CameraHandle, piGamma: *mut c_int) -> CameraSdkStatus;
        /// Reads the contrast setting.
        fn CameraGetContrast(hCamera: CameraHandle, piContrast: *mut c_int) -> CameraSdkStatus;
        /// Reads the saturation setting.
        fn CameraGetSaturation(hCamera: CameraHandle, piSaturation: *mut c_int) -> CameraSdkStatus;
        /// Reads the sharpness setting.
        fn CameraGetSharpness(hCamera: CameraHandle, piSharpness: *mut c_int) -> CameraSdkStatus;
        /// Reads the firmware version into a buffer of at least 32 bytes.
        fn CameraGetFirmwareVersion(hCamera: CameraHandle, pVersion: *mut c_char) -> CameraSdkStatus;
        /// Reads acquisition statistics.
        fn CameraGetFrameStatistic(hCamera: CameraHandle, psFrameStatistic: *mut tSdkFrameStatistic) -> CameraSdkStatus;
        /// Discards frames buffered by the SDK.
        fn CameraClearBuffer(hCamera: CameraHandle) -> CameraSdkStatus;
        /// Installs a frame callback (replaces polling with `CameraGetImageBuffer`).
        fn CameraSetCallbackFunction(hCamera: CameraHandle, pCallBack: CameraSnapProc, pContext: *mut c_void, pCallbackOld: *mut CameraSnapProc) -> CameraSdkStatus;
        /// Allocates an aligned buffer owned by the SDK allocator.
        fn CameraAlignMalloc(size: c_int, align: c_int) -> *mut u8;
        /// Frees a buffer from `CameraAlignMalloc`.
        fn CameraAlignFree(membuffer: *mut u8);
        /// Sets the analog gain in SDK units (factor = value x `sExposeDesc.fAnalogGainStep`).
        fn CameraSetAnalogGain(hCamera: CameraHandle, iAnalogGain: c_int) -> CameraSdkStatus;
        /// Sets the auto-exposure brightness target.
        fn CameraSetAeTarget(hCamera: CameraHandle, iAeTarget: c_int) -> CameraSdkStatus;
        /// Reads the auto-exposure brightness target.
        fn CameraGetAeTarget(hCamera: CameraHandle, piAeTarget: *mut c_int) -> CameraSdkStatus;
        /// Sets the digital R, G, B gains (factor = value / 100).
        fn CameraSetGain(hCamera: CameraHandle, iRGain: c_int, iGGain: c_int, iBGain: c_int) -> CameraSdkStatus;
        /// Sets the number of frames captured per trigger.
        fn CameraSetTriggerCount(hCamera: CameraHandle, iCount: c_int) -> CameraSdkStatus;
        /// Sets the hardware-trigger delay in microseconds.
        fn CameraSetTriggerDelayTime(hCamera: CameraHandle, uDelayTimeUs: c_uint) -> CameraSdkStatus;
        /// Selects a resolution (preset table entry or custom).
        fn CameraSetImageResolution(hCamera: CameraHandle, pImageResolution: *mut tSdkImageResolution) -> CameraSdkStatus;
        /// Selects a preset (`iIndex`) or a custom ROI (`iIndex` = 0xFF) with binning, skipping or resampling.
        fn CameraSetImageResolutionEx(hCamera: CameraHandle, iIndex: c_int, Mode: c_int, ModeSize: c_uint, x: c_int, y: c_int, width: c_int, height: c_int, ZoomWidth: c_int, ZoomHeight: c_int) -> CameraSdkStatus;
        /// Enables or disables the ISP mirror in one direction (0 horizontal, 1 vertical).
        fn CameraSetMirror(hCamera: CameraHandle, iDir: c_int, bEnable: c_int) -> CameraSdkStatus;
        /// Reads the ISP mirror state for one direction.
        fn CameraGetMirror(hCamera: CameraHandle, iDir: c_int, pbEnable: *mut c_int) -> CameraSdkStatus;
        /// Sets the counterclockwise rotation (0 none, 1: 90, 2: 180, 3: 270 degrees).
        fn CameraSetRotate(hCamera: CameraHandle, iRot: c_int) -> CameraSdkStatus;
        /// Reads the rotation.
        fn CameraGetRotate(hCamera: CameraHandle, iRot: *mut c_int) -> CameraSdkStatus;
        /// Sets gamma (effective in the parameter-generated LUT mode).
        fn CameraSetGamma(hCamera: CameraHandle, iGamma: c_int) -> CameraSdkStatus;
        /// Sets contrast (effective in the parameter-generated LUT mode).
        fn CameraSetContrast(hCamera: CameraHandle, iContrast: c_int) -> CameraSdkStatus;
        /// Sets saturation (100 = unchanged; no effect on mono cameras).
        fn CameraSetSaturation(hCamera: CameraHandle, iSaturation: c_int) -> CameraSdkStatus;
        /// Sets sharpening (0 disables).
        fn CameraSetSharpness(hCamera: CameraHandle, iSharpness: c_int) -> CameraSdkStatus;
        /// Sets the black level (0 to 255, default 0).
        fn CameraSetBlackLevel(hCamera: CameraHandle, iBlackLevel: c_int) -> CameraSdkStatus;
        /// Reads the black level.
        fn CameraGetBlackLevel(hCamera: CameraHandle, piBlackLevel: *mut c_int) -> CameraSdkStatus;
        /// Selects automatic (1) or manual (0) white balance.
        fn CameraSetWbMode(hCamera: CameraHandle, bAuto: c_int) -> CameraSdkStatus;
        /// Reads the white-balance mode.
        fn CameraGetWbMode(hCamera: CameraHandle, pbAuto: *mut c_int) -> CameraSdkStatus;
        /// Runs one manual white balance on the next frame.
        fn CameraSetOnceWB(hCamera: CameraHandle) -> CameraSdkStatus;
        /// Enables anti-flicker for automatic exposure.
        fn CameraSetAntiFlick(hCamera: CameraHandle, bEnable: c_int) -> CameraSdkStatus;
        /// Reads the anti-flicker state.
        fn CameraGetAntiFlick(hCamera: CameraHandle, pbEnable: *mut c_int) -> CameraSdkStatus;
        /// Selects the mains frequency for anti-flicker (0: 50 Hz, 1: 60 Hz).
        fn CameraSetLightFrequency(hCamera: CameraHandle, iFrequencySel: c_int) -> CameraSdkStatus;
        /// Reads the mains frequency selection.
        fn CameraGetLightFrequency(hCamera: CameraHandle, piFrequencySel: *mut c_int) -> CameraSdkStatus;
        /// Sets how often a GigE camera resends an unacknowledged frame in trigger mode (<= 0 disables).
        fn CameraSetFrameResendCount(hCamera: CameraHandle, count: c_int) -> CameraSdkStatus;
        /// Selects where parameter groups are stored (`emSdkParameterMode`).
        fn CameraSetParameterMode(hCamera: CameraHandle, iMode: c_int) -> CameraSdkStatus;
        /// Reads where parameter groups are stored.
        fn CameraGetParameterMode(hCamera: CameraHandle, piTarget: *mut c_int) -> CameraSdkStatus;
        /// Reads the currently selected parameter group.
        fn CameraGetCurrentParameterGroup(hCamera: CameraHandle, piTeam: *mut c_int) -> CameraSdkStatus;
        /// Sets the STROBE output mode (`emStrobeControl`).
        fn CameraSetStrobeMode(hCamera: CameraHandle, iMode: c_int) -> CameraSdkStatus;
        /// Reads the STROBE output mode.
        fn CameraGetStrobeMode(hCamera: CameraHandle, piMode: *mut c_int) -> CameraSdkStatus;
        /// Sets the STROBE active level (0 low, 1 high).
        fn CameraSetStrobePolarity(hCamera: CameraHandle, uPolarity: c_int) -> CameraSdkStatus;
        /// Sets the STROBE delay after the trigger in microseconds.
        fn CameraSetStrobeDelayTime(hCamera: CameraHandle, uDelayTimeUs: c_uint) -> CameraSdkStatus;
        /// Sets the STROBE pulse width in microseconds.
        fn CameraSetStrobePulseWidth(hCamera: CameraHandle, uTimeUs: c_uint) -> CameraSdkStatus;
        /// Sets an output line (1 high, 0 low, the same on every model).
        fn CameraSetIOStateEx(hCamera: CameraHandle, iOutputIOIndex: c_int, uState: c_uint) -> CameraSdkStatus;
        /// Reads an input line (1 high, 0 low).
        fn CameraGetIOStateEx(hCamera: CameraHandle, iInputIOIndex: c_int, puState: *mut c_uint) -> CameraSdkStatus;
        /// Reads the level of an output line (1 high, 0 low).
        fn CameraGetOutPutIOStateEx(hCamera: CameraHandle, iOutputIOIndex: c_int, puState: *mut c_uint) -> CameraSdkStatus;
        /// Installs a frame start/end event callback.
        fn CameraSetFrameEventCallback(hCamera: CameraHandle, pCallBack: CameraFrameEventCallback, pContext: *mut c_void) -> CameraSdkStatus;
        /// Installs a connection lost/restored callback.
        fn CameraSetConnectionStatusCallback(hCamera: CameraHandle, pCallBack: CameraConnectionStatusCallback, pContext: *mut c_void) -> CameraSdkStatus;
    }
}

impl Api {
    /// Path (or bare file name) the library was loaded from.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Resolves any other exported function by name.
    ///
    /// # Safety
    /// `T` must be the exact function pointer type of the symbol (use
    /// `unsafe extern "system" fn(...)`), otherwise calling it is undefined
    /// behaviour. The returned pointer must not outlive this `Api`.
    pub unsafe fn symbol<T: Copy>(&self, name: &str) -> Result<T, libloading::Error> {
        let mut bytes = name.as_bytes().to_vec();
        bytes.push(0);
        self.lib.get::<T>(&bytes).map(|s| *s)
    }
}

impl std::fmt::Debug for Api {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Api")
            .field("path", &self.path)
            .field("missing_optional", &self.missing_optional_symbols())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_c_chars(s: &[u8]) -> [c_char; 32] {
        let mut out = [0 as c_char; 32];
        for (dst, &b) in out.iter_mut().zip(s) {
            *dst = b as c_char;
        }
        out
    }

    #[test]
    fn decode_ascii_stops_at_nul() {
        let arr = to_c_chars(b"sb03\0garbage");
        assert_eq!(decode_c_chars(&arr), "sb03");
    }

    #[test]
    fn decode_gbk_fallback() {
        // "中文" encoded as GBK.
        let arr = to_c_chars(&[0xD6, 0xD0, 0xCE, 0xC4]);
        assert_eq!(decode_c_chars(&arr), "中文");
    }

    #[test]
    fn encode_roundtrip() {
        assert_eq!(encode_sdk_text("sb00").unwrap(), b"sb00");
        assert_eq!(encode_sdk_text("中文").unwrap(), vec![0xD6, 0xD0, 0xCE, 0xC4]);
        assert!(encode_sdk_text("a\0b").is_none());
    }

    #[test]
    fn media_type_helpers() {
        assert_eq!(media_type_bits_per_pixel(CAMERA_MEDIA_TYPE_MONO8), 8);
        assert_eq!(media_type_bits_per_pixel(CAMERA_MEDIA_TYPE_BGR8), 24);
        assert_eq!(media_type_bits_per_pixel(CAMERA_MEDIA_TYPE_RGBA8), 32);
        assert_eq!(media_type_bits_per_pixel(CAMERA_MEDIA_TYPE_MONO16), 16);
        assert!(media_type_is_mono(CAMERA_MEDIA_TYPE_MONO8));
        assert!(!media_type_is_mono(CAMERA_MEDIA_TYPE_RGB8));
        assert_eq!(CAMERA_MEDIA_TYPE_MONO8, 0x0108_0001);
    }

    #[test]
    fn api_symbol_lists() {
        assert_eq!(Api::REQUIRED_SYMBOLS.len(), 25);
        assert!(Api::REQUIRED_SYMBOLS.contains(&"CameraGetImageBuffer"));
        assert!(Api::OPTIONAL_SYMBOLS.contains(&"CameraSdkGetVersionString"));
    }

    #[test]
    fn load_missing_library_fails_cleanly() {
        let err = unsafe { Api::load("/nonexistent/dir/libMVSDK.so") }.unwrap_err();
        assert!(matches!(err, LoadError::Open { .. }));
    }
}
