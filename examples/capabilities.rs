//! Compilable snippets for `docs/CAPABILITIES.md`, one function per documented capability.
//! Every code block in that document is copied from a `// [doc:...]` region of this file, and
//! `tests/capabilities_doc.rs` checks that the two stay identical.
//!
//! List the snippets:          cargo run --example capabilities -- <camera>
//! Run read-only snippets:     cargo run --example capabilities -- <camera> gige_ip read_sn
//! Run snippets that write:    cargo run --example capabilities -- <camera> --write exposure
//!
//! `<camera>` is a user-set name or serial number. Snippets that change settings for the session
//! run only with `--write`. Snippets that store demonstration values persistently (`user_name`
//! renames the camera to sb03, `set_ip` assigns 192.168.1.50/24, `parameter_groups` overwrites
//! group B, `parameter_file`, `parameter_mode`) also need `--persistent`.

use std::error::Error;
use std::ffi::{c_char, c_int, c_uint, c_void, CString};
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use gcam_mvsdk_rs::ffi::{self, CameraHandle, CameraSdkStatus};
use gcam_mvsdk_rs::{
    Binning, Camera, DeviceInfo, LightFrequency, MirrorDirection, OutputFormat, ParameterMode, ParameterTeam, Roi,
    Rotation, Sdk, StrobeMode, TriggerMode,
};

// [doc:ctx]
/// Result type of every snippet.
type R<T = ()> = Result<T, Box<dyn Error>>;

/// What every snippet receives: the SDK, the enumerated device, and the open camera.
struct Ctx {
    sdk: Sdk,
    dev: DeviceInfo,
    cam: Camera,
}
// [/doc]

// [doc:raw]
/// Calls to SDK functions that have no safe wrapper. Each helper resolves the C function by
/// name and calls it with the shape in its own name; pass only names whose declaration in
/// CameraApi.h has exactly that shape (`BOOL` and `INT` are C `int`, `UINT` is `unsigned int`).
mod raw {
    use super::*;

    type Fn0 = unsafe extern "system" fn(CameraHandle) -> CameraSdkStatus;
    type FnI = unsafe extern "system" fn(CameraHandle, c_int) -> CameraSdkStatus;
    type FnU = unsafe extern "system" fn(CameraHandle, c_uint) -> CameraSdkStatus;
    type FnPI = unsafe extern "system" fn(CameraHandle, *mut c_int) -> CameraSdkStatus;
    type FnPU = unsafe extern "system" fn(CameraHandle, *mut c_uint) -> CameraSdkStatus;
    type FnII = unsafe extern "system" fn(CameraHandle, c_int, c_int) -> CameraSdkStatus;
    type FnIU = unsafe extern "system" fn(CameraHandle, c_int, c_uint) -> CameraSdkStatus;
    type FnIIII = unsafe extern "system" fn(CameraHandle, c_int, c_int, c_int, c_int) -> CameraSdkStatus;

    /// `name(CameraHandle)`
    pub fn call(cam: &Camera, name: &'static str) -> R {
        // SAFETY: the caller names a function with exactly this signature.
        let f: Fn0 = unsafe { cam.sdk().api().symbol(name)? };
        // SAFETY: open handle.
        Ok(cam.sdk().check(name, unsafe { f(cam.handle()) })?)
    }

    /// `name(CameraHandle, int)`
    pub fn set_i32(cam: &Camera, name: &'static str, v: i32) -> R {
        // SAFETY: the caller names a function with exactly this signature.
        let f: FnI = unsafe { cam.sdk().api().symbol(name)? };
        // SAFETY: open handle, plain value.
        Ok(cam.sdk().check(name, unsafe { f(cam.handle(), v) })?)
    }

    /// `name(CameraHandle, unsigned int)`
    pub fn set_u32(cam: &Camera, name: &'static str, v: u32) -> R {
        // SAFETY: the caller names a function with exactly this signature.
        let f: FnU = unsafe { cam.sdk().api().symbol(name)? };
        // SAFETY: open handle, plain value.
        Ok(cam.sdk().check(name, unsafe { f(cam.handle(), v) })?)
    }

    /// `name(CameraHandle, BOOL)`
    pub fn set_bool(cam: &Camera, name: &'static str, v: bool) -> R {
        set_i32(cam, name, i32::from(v))
    }

    /// `name(CameraHandle, int*)`
    pub fn get_i32(cam: &Camera, name: &'static str) -> R<i32> {
        // SAFETY: the caller names a function with exactly this signature.
        let f: FnPI = unsafe { cam.sdk().api().symbol(name)? };
        let mut v: c_int = 0;
        // SAFETY: open handle and a valid out pointer.
        cam.sdk().check(name, unsafe { f(cam.handle(), &mut v) })?;
        Ok(v)
    }

    /// `name(CameraHandle, unsigned int*)`
    pub fn get_u32(cam: &Camera, name: &'static str) -> R<u32> {
        // SAFETY: the caller names a function with exactly this signature.
        let f: FnPU = unsafe { cam.sdk().api().symbol(name)? };
        let mut v: c_uint = 0;
        // SAFETY: open handle and a valid out pointer.
        cam.sdk().check(name, unsafe { f(cam.handle(), &mut v) })?;
        Ok(v)
    }

    /// `name(CameraHandle, BOOL*)`
    pub fn get_bool(cam: &Camera, name: &'static str) -> R<bool> {
        Ok(get_i32(cam, name)? != 0)
    }

    /// `name(CameraHandle, int, int)`
    pub fn set_i32x2(cam: &Camera, name: &'static str, a: i32, b: i32) -> R {
        // SAFETY: the caller names a function with exactly this signature.
        let f: FnII = unsafe { cam.sdk().api().symbol(name)? };
        // SAFETY: open handle, plain values.
        Ok(cam.sdk().check(name, unsafe { f(cam.handle(), a, b) })?)
    }

    /// `name(CameraHandle, int index, unsigned int value)`
    pub fn set_index_u32(cam: &Camera, name: &'static str, index: i32, v: u32) -> R {
        // SAFETY: the caller names a function with exactly this signature.
        let f: FnIU = unsafe { cam.sdk().api().symbol(name)? };
        // SAFETY: open handle, plain values.
        Ok(cam.sdk().check(name, unsafe { f(cam.handle(), index, v) })?)
    }

    /// `name(CameraHandle, int, int, int, int)`
    pub fn set_i32x4(cam: &Camera, name: &'static str, a: i32, b: i32, c: i32, d: i32) -> R {
        // SAFETY: the caller names a function with exactly this signature.
        let f: FnIIII = unsafe { cam.sdk().api().symbol(name)? };
        // SAFETY: open handle, plain values.
        Ok(cam.sdk().check(name, unsafe { f(cam.handle(), a, b, c, d) })?)
    }
}
// [/doc]

// ---------------------------------------------------------------------------
// 1. Identity and network
// ---------------------------------------------------------------------------

// [doc:user_name]
fn user_name(c: &Ctx) -> R {
    println!("user-set name: {:?}", c.cam.user_name()?);
    c.cam.set_user_name("sb03")?; // at most 31 bytes; stored in the camera, survives power cycles
    Ok(())
}
// [/doc]

// [doc:gige_ip]
fn gige_ip(c: &Ctx) -> R {
    let ip = c.dev.gige_ip()?; // camera and host NIC: address, mask, gateway
    let mac = c.dev.gige_mac()?;
    println!("camera {} MAC {}; host {}; same subnet: {}", ip.camera, mac.camera, ip.host, ip.same_subnet());
    Ok(())
}
// [/doc]

// [doc:set_ip]
fn set_ip(c: &Ctx) -> R {
    let ip: Ipv4Addr = "192.168.1.50".parse()?;
    let mask: Ipv4Addr = "255.255.255.0".parse()?;
    c.sdk.gige_set_ip(&c.dev, ip, mask, Ipv4Addr::UNSPECIFIED, true)?; // persistent; works across subnets
    Ok(())
}
// [/doc]

// [doc:read_sn]
fn read_sn(c: &Ctx) -> R {
    // SAFETY: CameraReadSN(CameraHandle, BYTE* pbySN, INT iLevel).
    let read: unsafe extern "system" fn(CameraHandle, *mut u8, c_int) -> CameraSdkStatus =
        unsafe { c.sdk.api().symbol("CameraReadSN")? };
    let mut sn = [0u8; 64]; // each level holds 32 bytes; the vendor wrapper passes 64
    // SAFETY: open handle and a 64-byte buffer; levels 1 and 2 are reserved for integrators.
    c.sdk.check("CameraReadSN", unsafe { read(c.cam.handle(), sn.as_mut_ptr(), 1) })?;
    println!("level-1 serial: {:?}", ffi::decode_sdk_bytes(&sn));
    Ok(())
}
// [/doc]

// [doc:user_data]
fn user_data(c: &Ctx) -> R {
    let max = c.cam.capability()?.user_data_max_len; // bytes in the camera's user area; 0 = none
    let len = (max - 1).clamp(0, 16); // start + length must stay below the maximum
    if len == 0 {
        return Ok(());
    }
    // SAFETY: CameraLoadUserData(CameraHandle, UINT uStartAddr, BYTE* pbData, int ilen).
    let load: unsafe extern "system" fn(CameraHandle, c_uint, *mut u8, c_int) -> CameraSdkStatus =
        unsafe { c.sdk.api().symbol("CameraLoadUserData")? };
    let mut buf = vec![0u8; len as usize];
    // SAFETY: open handle; reads `len` bytes from address 0 into a buffer of that size.
    c.sdk.check("CameraLoadUserData", unsafe { load(c.cam.handle(), 0, buf.as_mut_ptr(), len) })?;
    println!("user area: {buf:?}"); // CameraSaveUserData takes the same arguments and writes
    Ok(())
}
// [/doc]

// ---------------------------------------------------------------------------
// 2. Exposure and gain
// ---------------------------------------------------------------------------

// [doc:exposure]
fn exposure(c: &Ctx) -> R {
    let (min, max, step) = c.cam.exposure_range_us()?;
    println!("exposure {:.1} us (range {min:.1} to {max:.1}, step {step:.2})", c.cam.exposure_us()?);
    c.cam.set_auto_exposure(false)?; // manual exposure
    c.cam.set_exposure_us(2000.0)?; // the sensor rounds to whole lines; read it back
    Ok(())
}
// [/doc]

// [doc:auto_exposure]
fn auto_exposure(c: &Ctx) -> R {
    c.cam.set_auto_exposure(true)?;
    println!("auto exposure: {}", c.cam.auto_exposure()?);
    Ok(())
}
// [/doc]

// [doc:ae_target]
fn ae_target(c: &Ctx) -> R {
    let e = c.cam.capability()?.expose; // target range uiTargetMin..=uiTargetMax
    println!("target {} (range {} to {})", c.cam.ae_target()?, e.uiTargetMin, e.uiTargetMax);
    c.cam.set_ae_target(((e.uiTargetMin + e.uiTargetMax) / 2) as i32)?;
    Ok(())
}
// [/doc]

// [doc:ae_threshold]
fn ae_threshold(c: &Ctx) -> R {
    raw::set_i32(&c.cam, "CameraSetAeThreshold", 5)?; // stop adjusting within 5 of the target
    println!("threshold {}", raw::get_i32(&c.cam, "CameraGetAeThreshold")?);
    Ok(())
}
// [/doc]

// [doc:ae_exposure_range]
fn ae_exposure_range(c: &Ctx) -> R {
    // SAFETY: CameraSetAeExposureRange(CameraHandle, double fMin, double fMax), microseconds.
    let set: unsafe extern "system" fn(CameraHandle, f64, f64) -> CameraSdkStatus =
        unsafe { c.sdk.api().symbol("CameraSetAeExposureRange")? };
    // SAFETY: open handle, plain values.
    c.sdk.check("CameraSetAeExposureRange", unsafe { set(c.cam.handle(), 100.0, 10_000.0) })?;
    Ok(())
}
// [/doc]

// [doc:ae_gain_range]
fn ae_gain_range(c: &Ctx) -> R {
    let e = c.cam.capability()?.expose; // analog gain range in SDK units
    raw::set_i32x2(&c.cam, "CameraSetAeAnalogGainRange", e.uiAnalogGainMin as i32, e.uiAnalogGainMin as i32)?;
    Ok(()) // auto exposure may now change only the exposure time, never the gain
}
// [/doc]

// [doc:ae_window]
fn ae_window(c: &Ctx) -> R {
    raw::set_i32x4(&c.cam, "CameraSetAeWindow", 0, 0, 0, 0)?; // all zero: centered, half the image
    Ok(())
}
// [/doc]

// [doc:analog_gain]
fn analog_gain(c: &Ctx) -> R {
    let e = c.cam.capability()?.expose; // range uiAnalogGainMin..=uiAnalogGainMax
    let gain = c.cam.analog_gain()?;
    println!("analog gain {gain} = x{:.2}", gain as f32 * e.fAnalogGainStep);
    c.cam.set_analog_gain(e.uiAnalogGainMin as i32)?; // lowest gain, least noise
    Ok(())
}
// [/doc]

// [doc:analog_gain_x]
fn analog_gain_x(c: &Ctx) -> R {
    // SAFETY: CameraGetAnalogGainXRange(CameraHandle, float* min, float* max, float* step).
    let range: unsafe extern "system" fn(CameraHandle, *mut f32, *mut f32, *mut f32) -> CameraSdkStatus =
        unsafe { c.sdk.api().symbol("CameraGetAnalogGainXRange")? };
    let (mut min, mut max, mut step) = (0f32, 0f32, 0f32);
    // SAFETY: open handle and three valid out pointers.
    c.sdk.check("CameraGetAnalogGainXRange", unsafe { range(c.cam.handle(), &mut min, &mut max, &mut step) })?;
    println!("gain factor {min} to {max}, step {step}"); // CameraSetAnalogGainX(CameraHandle, float)
    Ok(())
}
// [/doc]

// [doc:rgb_gain]
fn rgb_gain(c: &Ctx) -> R {
    println!("digital gain R, G, B: {:?} (100 = x1.0)", c.cam.rgb_gain()?);
    c.cam.set_rgb_gain(100, 100, 100)?; // range Capability::rgb_gain_range
    Ok(())
}
// [/doc]

// [doc:multi_exposure]
fn multi_exposure(c: &Ctx) -> R {
    // line-scan cameras only; area-scan cameras return an error status
    println!("max exposures per line: {}", raw::get_i32(&c.cam, "CameraGetMultiExposureMaxCount")?);
    Ok(())
}
// [/doc]

// [doc:anti_flick]
fn anti_flick(c: &Ctx) -> R {
    c.cam.set_auto_exposure(true)?; // anti-flicker acts only in automatic exposure
    c.cam.set_anti_flick(true)?;
    println!("anti-flicker: {}", c.cam.anti_flick()?);
    Ok(())
}
// [/doc]

// [doc:light_frequency]
fn light_frequency(c: &Ctx) -> R {
    c.cam.set_light_frequency(LightFrequency::Hz50)?; // mains frequency of the room lighting
    println!("{:?}", c.cam.light_frequency()?);
    Ok(())
}
// [/doc]

// ---------------------------------------------------------------------------
// 3. Frame rate, trigger and timing
// ---------------------------------------------------------------------------

// [doc:frame_rate]
fn frame_rate(c: &Ctx) -> R {
    c.cam.set_frame_rate(30)?; // only some GigE models; <= 0 removes the cap
    println!("frame-rate cap {} Hz", c.cam.frame_rate()?);
    Ok(())
}
// [/doc]

// [doc:frame_speed]
fn frame_speed(c: &Ctx) -> R {
    let speeds = c.cam.capability()?.frame_speeds; // indices 0 to len - 1
    if let Some(last) = speeds.last() {
        c.cam.set_frame_speed(last.index)?;
    }
    println!("frame speed {} of {speeds:?}", c.cam.frame_speed()?);
    Ok(())
}
// [/doc]

// [doc:trigger_mode]
fn trigger_mode(c: &Ctx) -> R {
    println!("modes: {:?}", c.cam.capability()?.triggers);
    c.cam.set_trigger_mode(TriggerMode::Hardware)?; // Continuous, Software or Hardware
    Ok(())
}
// [/doc]

// [doc:soft_trigger]
fn soft_trigger(c: &Ctx) -> R {
    c.cam.set_trigger_mode(TriggerMode::Software)?;
    c.cam.play()?;
    c.cam.soft_trigger()?; // captures trigger_count() frames
    let frame = c.cam.grab(Duration::from_secs(1))?;
    println!("{}x{}", frame.width(), frame.height());
    Ok(())
}
// [/doc]

// [doc:trigger_count]
fn trigger_count(c: &Ctx) -> R {
    c.cam.set_trigger_count(3)?; // frames per trigger (default 1)
    println!("{} frames per trigger", c.cam.trigger_count()?);
    Ok(())
}
// [/doc]

// [doc:trigger_delay]
fn trigger_delay(c: &Ctx) -> R {
    c.cam.set_trigger_delay_us(500)?; // hardware trigger; only some models
    println!("delay {} us", c.cam.trigger_delay_us()?);
    Ok(())
}
// [/doc]

// [doc:trigger_period]
fn trigger_period(c: &Ctx) -> R {
    raw::set_u32(&c.cam, "CameraSetTriggerPeriodTime", 10_000)?; // 10 ms between frames of one trigger
    println!("period {} us", raw::get_u32(&c.cam, "CameraGetTriggerPeriodTime")?);
    Ok(())
}
// [/doc]

// [doc:external_trigger]
fn external_trigger(c: &Ctx) -> R {
    let caps = raw::get_u32(&c.cam, "CameraGetExtTrigCapability")?; // EXT_TRIG_MASK_* bits
    raw::set_i32(&c.cam, "CameraSetExtTrigSignalType", 0)?; // 0 rising edge (default), 1 falling, 2 high, 3 low level
    raw::set_i32(&c.cam, "CameraSetExtTrigShutterType", 0)?; // 0 standard (default), 1 GRR
    raw::set_u32(&c.cam, "CameraSetExtTrigJitterTime", 100)?; // debounce, us (default 0)
    raw::set_u32(&c.cam, "CameraSetExtTrigDelayTime", 0)?; // us (default 0)
    raw::set_u32(&c.cam, "CameraSetExtTrigBufferedDelayTime", 0)?; // us; buffers up to 128 triggers
    raw::set_u32(&c.cam, "CameraSetExtTrigIntervalTime", 0)?; // us (default 0)
    println!("external trigger capability mask {caps:#x}");
    Ok(())
}
// [/doc]

// [doc:play_pause_stop]
fn play_pause_stop(c: &Ctx) -> R {
    c.cam.play()?;
    c.cam.pause()?; // keeps the settings; play() resumes
    let stop = c.sdk.api().CameraStop.ok_or("CameraStop is not exported")?;
    // SAFETY: open handle. After CameraStop the camera accepts no more settings; close it.
    c.sdk.check("CameraStop", unsafe { stop(c.cam.handle()) })?;
    Ok(())
}
// [/doc]

// [doc:clear_buffer]
fn clear_buffer(c: &Ctx) -> R {
    raw::call(&c.cam, "CameraClearBuffer")?; // discard frames captured before a settings change
    Ok(())
}
// [/doc]

// ---------------------------------------------------------------------------
// 4. Image geometry
// ---------------------------------------------------------------------------

// [doc:resolution]
fn resolution(c: &Ctx) -> R {
    let presets = c.cam.capability()?.image_sizes; // the camera's preset table
    for r in &presets {
        println!("[{}] {}x{} {}", r.index, r.width, r.height, r.description);
    }
    if let Some(first) = presets.first() {
        c.cam.set_image_resolution(first)?;
    }
    println!("now {:?}", c.cam.image_resolution()?.description);
    Ok(())
}
// [/doc]

// [doc:roi]
fn roi(c: &Ctx) -> R {
    let range = c.cam.capability()?.resolution_range; // size limits and bin/skip masks
    println!("max {}x{}, sum-binning mask {:#x}", range.iWidthMax, range.iHeightMax, range.uBinSumModeMask);
    c.cam.set_roi(Roi { x: 0, y: 0, width: 640, height: 480, binning: Binning::None })?;
    let full = Roi { x: 0, y: 0, width: range.iWidthMax, height: range.iHeightMax, binning: Binning::Sum(2) };
    c.cam.set_roi(full)?; // 2x2 summing binning over the full sensor
    println!("{:?}", c.cam.image_resolution()?);
    Ok(())
}
// [/doc]

// [doc:resolution_for_snap]
fn resolution_for_snap(c: &Ctx) -> R {
    // SAFETY: CameraSetResolutionForSnap(CameraHandle, tSdkImageResolution*).
    let set: unsafe extern "system" fn(CameraHandle, *mut ffi::tSdkImageResolution) -> CameraSdkStatus =
        unsafe { c.sdk.api().symbol("CameraSetResolutionForSnap")? };
    let mut same_as_preview = ffi::tSdkImageResolution::default(); // width = height = 0
    // SAFETY: open handle and a struct of the correct layout.
    c.sdk.check("CameraSetResolutionForSnap", unsafe { set(c.cam.handle(), &mut same_as_preview) })?;
    Ok(())
}
// [/doc]

// [doc:transfer_roi]
fn transfer_roi(c: &Ctx) -> R {
    // SAFETY: CameraSetTransferRoi(CameraHandle, int index, UINT x1, UINT y1, UINT x2, UINT y2).
    let set: unsafe extern "system" fn(CameraHandle, c_int, c_uint, c_uint, c_uint, c_uint) -> CameraSdkStatus =
        unsafe { c.sdk.api().symbol("CameraSetTransferRoi")? };
    // SAFETY: open handle; region 0, upper-left (0, 0), lower-right (639, 479).
    c.sdk.check("CameraSetTransferRoi", unsafe { set(c.cam.handle(), 0, 0, 0, 639, 479) })?;
    raw::set_u32(&c.cam, "CameraEnableTransferRoi", 0b0001)?; // regions 0 to 3, one bit each
    Ok(())
}
// [/doc]

// [doc:mirror]
fn mirror(c: &Ctx) -> R {
    c.cam.set_mirror(MirrorDirection::Horizontal, true)?; // done by the SDK image pipeline
    println!("vertical mirror: {}", c.cam.mirror(MirrorDirection::Vertical)?);
    Ok(())
}
// [/doc]

// [doc:hardware_mirror]
fn hardware_mirror(c: &Ctx) -> R {
    raw::set_i32x2(&c.cam, "CameraSetHardwareMirror", 0, 1)?; // direction 0 horizontal, 1 vertical; on
    Ok(())
}
// [/doc]

// [doc:rotate]
fn rotate(c: &Ctx) -> R {
    c.cam.set_rotation(Rotation::Ccw180)?; // counterclockwise
    println!("{:?}", c.cam.rotation()?);
    Ok(())
}
// [/doc]

// [doc:inverse]
fn inverse(c: &Ctx) -> R {
    raw::set_bool(&c.cam, "CameraSetInverse", true)?; // negative image
    println!("inverted: {}", raw::get_bool(&c.cam, "CameraGetInverse")?);
    Ok(())
}
// [/doc]

// [doc:flip_frame_buffer]
fn flip_frame_buffer(c: &Ctx) -> R {
    // SAFETY: CameraFlipFrameBuffer(BYTE* pFrameBuffer, tSdkFrameHead* pFrameHead, int Flags).
    let flip: unsafe extern "system" fn(*mut u8, *mut ffi::tSdkFrameHead, c_int) -> CameraSdkStatus =
        unsafe { c.sdk.api().symbol("CameraFlipFrameBuffer")? };
    c.cam.set_output_format(OutputFormat::Mono8)?;
    c.cam.play()?;
    let frame = c.cam.grab(Duration::from_secs(1))?;
    let mut image = frame.process()?;
    let mut head = *frame.head(); // describe the processed buffer, not the raw one
    head.uiMediaType = image.info.media_type;
    head.uBytes = image.data.len() as c_uint;
    head.iWidth = image.info.width as c_int;
    head.iHeight = image.info.height as c_int;
    // SAFETY: buffer and header describe the same image. Flags: 1 top-bottom, 2 left-right, 3 both.
    c.sdk.check("CameraFlipFrameBuffer", unsafe { flip(image.data.as_mut_ptr(), &mut head, 1) })?;
    Ok(())
}
// [/doc]

// [doc:splice_lines]
fn splice_lines(c: &Ctx) -> R {
    raw::set_i32(&c.cam, "CameraSetSpliceLines", 0)?; // line-scan only; 0 = no splicing (default)
    println!("splice lines {}", raw::get_i32(&c.cam, "CameraGetSpliceLines")?);
    Ok(())
}
// [/doc]

// ---------------------------------------------------------------------------
// 5. Image processing
// ---------------------------------------------------------------------------

// [doc:isp_out_format]
fn isp_out_format(c: &Ctx) -> R {
    c.cam.set_output_format(OutputFormat::Mono8)?; // what Frame::process and process_into produce
    println!("{:?}", c.cam.output_format());
    Ok(())
}
// [/doc]

// [doc:isp_processor]
fn isp_processor(c: &Ctx) -> R {
    if c.cam.capability()?.isp.bDeviceIsp != 0 {
        raw::set_i32(&c.cam, "CameraSetIspProcessor", 1)?; // 0 host PC, 1 camera
    }
    println!("ISP runs on {}", raw::get_i32(&c.cam, "CameraGetIspProcessor")?);
    Ok(())
}
// [/doc]

// [doc:monochrome]
fn monochrome(c: &Ctx) -> R {
    raw::set_bool(&c.cam, "CameraSetMonochrome", true)?; // a color camera delivers gray images
    println!("monochrome: {}", raw::get_bool(&c.cam, "CameraGetMonochrome")?);
    Ok(())
}
// [/doc]

// [doc:gamma]
fn gamma(c: &Ctx) -> R {
    let range = c.cam.capability()?.gamma_range;
    c.cam.set_gamma((range.iMin + range.iMax) / 2)?; // effective in the generated-LUT mode
    println!("gamma {} (range {} to {})", c.cam.gamma()?, range.iMin, range.iMax);
    Ok(())
}
// [/doc]

// [doc:contrast]
fn contrast(c: &Ctx) -> R {
    let range = c.cam.capability()?.contrast_range;
    c.cam.set_contrast((range.iMin + range.iMax) / 2)?; // effective in the generated-LUT mode
    println!("contrast {} (range {} to {})", c.cam.contrast()?, range.iMin, range.iMax);
    Ok(())
}
// [/doc]

// [doc:saturation]
fn saturation(c: &Ctx) -> R {
    c.cam.set_saturation(100)?; // 100 = colors unchanged; no effect on mono cameras
    println!("saturation {}", c.cam.saturation()?);
    Ok(())
}
// [/doc]

// [doc:sharpness]
fn sharpness(c: &Ctx) -> R {
    c.cam.set_sharpness(0)?; // 0 = no sharpening
    println!("sharpness {} (range {:?})", c.cam.sharpness()?, c.cam.capability()?.sharpness_range);
    Ok(())
}
// [/doc]

// [doc:black_level]
fn black_level(c: &Ctx) -> R {
    c.cam.set_black_level(0)?; // 0 to 255, default 0
    println!("black level {}", c.cam.black_level()?);
    Ok(())
}
// [/doc]

// [doc:white_level]
fn white_level(c: &Ctx) -> R {
    raw::set_i32(&c.cam, "CameraSetWhiteLevel", 255)?; // 0 to 255, default 255
    println!("white level {}", raw::get_i32(&c.cam, "CameraGetWhiteLevel")?);
    Ok(())
}
// [/doc]

// [doc:log_curve]
fn log_curve(c: &Ctx) -> R {
    raw::set_i32(&c.cam, "CameraSetLogarithmicCurveValue", 0)?; // 0 to 255
    println!("log curve {}", raw::get_i32(&c.cam, "CameraGetLogarithmicCurveValue")?);
    Ok(())
}
// [/doc]

// [doc:noise_filter]
fn noise_filter(c: &Ctx) -> R {
    raw::set_bool(&c.cam, "CameraSetNoiseFilter", true)?;
    println!("noise filter: {}", raw::get_bool(&c.cam, "CameraGetNoiseFilterState")?);
    Ok(())
}
// [/doc]

// [doc:noise_reduction]
fn noise_reduction(c: &Ctx) -> R {
    raw::set_i32(&c.cam, "CameraSetNoiseReductionValue", 3)?; // 0 to 7
    println!("noise reduction {}", raw::get_i32(&c.cam, "CameraGetNoiseReductionValue")?);
    Ok(())
}
// [/doc]

// [doc:denoise_3d]
fn denoise_3d(c: &Ctx) -> R {
    // SAFETY: CameraSetDenoise3DParams(CameraHandle, BOOL bEnable, int nCount, float* Weights).
    let set: unsafe extern "system" fn(CameraHandle, c_int, c_int, *mut f32) -> CameraSdkStatus =
        unsafe { c.sdk.api().symbol("CameraSetDenoise3DParams")? };
    let mut weights = [0.3f32, 0.3, 0.4]; // one weight per frame; the newest weighs most
    // SAFETY: open handle; 3 frames (2 to 8 allowed) and 3 weights.
    c.sdk.check("CameraSetDenoise3DParams", unsafe { set(c.cam.handle(), 1, 3, weights.as_mut_ptr()) })?;
    Ok(())
}
// [/doc]

// [doc:dead_pixel]
fn dead_pixel(c: &Ctx) -> R {
    raw::set_bool(&c.cam, "CameraSetCorrectDeadPixel", true)?;
    println!("dead-pixel correction: {}", raw::get_bool(&c.cam, "CameraGetCorrectDeadPixel")?);
    Ok(())
}
// [/doc]

// [doc:bayer_algorithm]
fn bayer_algorithm(c: &Ctx) -> R {
    let algorithms = c.cam.capability()?.bayer_algorithms_sw; // empty on mono cameras
    if let Some(a) = algorithms.first() {
        raw::set_i32x2(&c.cam, "CameraSetBayerDecAlgorithm", 0, a.index)?; // 0 = host ISP
    }
    Ok(())
}
// [/doc]

// [doc:white_balance]
fn white_balance(c: &Ctx) -> R {
    c.cam.set_auto_white_balance(false)?; // manual: balance once on a gray target
    c.cam.white_balance_once()?; // applied with the next frame
    println!("automatic white balance: {}", c.cam.auto_white_balance()?);
    Ok(())
}
// [/doc]

// [doc:wb_window]
fn wb_window(c: &Ctx) -> R {
    raw::set_i32x4(&c.cam, "CameraSetWbWindow", 0, 0, 200, 200)?; // x, y, width, height of the reference area
    Ok(())
}
// [/doc]

// [doc:color_temperature]
fn color_temperature(c: &Ctx) -> R {
    println!("presets: {:?}", c.cam.capability()?.color_temperatures);
    raw::set_i32(&c.cam, "CameraSetClrTempMode", 1)?; // 0 automatic, 1 preset, 2 user gains and matrix
    raw::set_i32(&c.cam, "CameraSetPresetClrTemp", 0)?; // preset index from the list above
    Ok(())
}
// [/doc]

// [doc:user_color_temperature]
fn user_color_temperature(c: &Ctx) -> R {
    // SAFETY: CameraSetUserClrTempGain(CameraHandle, int r, int g, int b).
    let gains: unsafe extern "system" fn(CameraHandle, c_int, c_int, c_int) -> CameraSdkStatus =
        unsafe { c.sdk.api().symbol("CameraSetUserClrTempGain")? };
    // SAFETY: CameraSetUserClrTempMatrix(CameraHandle, float* pMatrix), pMatrix = float[3][3].
    let matrix: unsafe extern "system" fn(CameraHandle, *mut f32) -> CameraSdkStatus =
        unsafe { c.sdk.api().symbol("CameraSetUserClrTempMatrix")? };
    raw::set_i32(&c.cam, "CameraSetClrTempMode", 2)?;
    // SAFETY: open handle; gains 0 to 400 mean x0 to x4.
    c.sdk.check("CameraSetUserClrTempGain", unsafe { gains(c.cam.handle(), 100, 100, 100) })?;
    let mut identity = [1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    // SAFETY: open handle and 9 floats.
    c.sdk.check("CameraSetUserClrTempMatrix", unsafe { matrix(c.cam.handle(), identity.as_mut_ptr()) })?;
    Ok(())
}
// [/doc]

// [doc:once_black_balance]
fn once_black_balance(c: &Ctx) -> R {
    raw::call(&c.cam, "CameraSetOnceBB")?; // one black balance; cover the lens first
    Ok(())
}
// [/doc]

// [doc:lut]
fn lut(c: &Ctx) -> R {
    // SAFETY: CameraGetCurrentLut(CameraHandle, int iChannel, USHORT* pLut), 4096 entries.
    let get: unsafe extern "system" fn(CameraHandle, c_int, *mut u16) -> CameraSdkStatus =
        unsafe { c.sdk.api().symbol("CameraGetCurrentLut")? };
    let mut table = vec![0u16; 4096]; // one entry per 12-bit input value
    // SAFETY: open handle; channel 0 = all (returns the red table); 4096 entries.
    c.sdk.check("CameraGetCurrentLut", unsafe { get(c.cam.handle(), 0, table.as_mut_ptr()) })?;
    if !c.cam.capability()?.preset_luts.is_empty() {
        raw::set_i32(&c.cam, "CameraSetLutMode", 1)?; // 0 generated, 1 preset, 2 user (CameraSetCustomLut)
        raw::set_i32(&c.cam, "CameraSelectLutPreset", 0)?;
    }
    println!("LUT[0] = {}, LUT[4095] = {}", table[0], table[4095]);
    Ok(())
}
// [/doc]

// [doc:undistort]
fn undistort(c: &Ctx) -> R {
    // SAFETY: CameraSetUndistortParams(CameraHandle, int w, int h, double[4], double[5]).
    let set: unsafe extern "system" fn(CameraHandle, c_int, c_int, *mut f64, *mut f64) -> CameraSdkStatus =
        unsafe { c.sdk.api().symbol("CameraSetUndistortParams")? };
    let r = c.cam.image_resolution()?;
    let mut k = [1000.0, 1000.0, r.width as f64 / 2.0, r.height as f64 / 2.0]; // fx, fy, cx, cy
    let mut d = [0.0f64; 5]; // k1, k2, p1, p2, k3 from your calibration
    // SAFETY: open handle, 4 and 5 doubles.
    c.sdk.check("CameraSetUndistortParams", unsafe {
        set(c.cam.handle(), r.width, r.height, k.as_mut_ptr(), d.as_mut_ptr())
    })?;
    raw::set_bool(&c.cam, "CameraSetUndistortEnable", true)?;
    Ok(())
}
// [/doc]

// ---------------------------------------------------------------------------
// 6. Digital I/O, strobe and lighting
// ---------------------------------------------------------------------------

// [doc:io_state]
fn io_state(c: &Ctx) -> R {
    let cap = c.cam.capability()?; // input_io_count and output_io_count lines, numbered from 0
    if cap.input_io_count > 0 {
        println!("input 0 high: {}", c.cam.input_io_state(0)?);
    }
    if cap.output_io_count > 0 {
        c.cam.set_output_io_state(0, true)?; // the line must be in general-purpose output mode
        println!("output 0 high: {}", c.cam.output_io_state(0)?);
    }
    Ok(())
}
// [/doc]

// [doc:input_io]
fn input_io(c: &Ctx) -> R {
    // SAFETY: CameraGetInPutIOModeCapbility(CameraHandle, INT index, UINT* mask).
    let modes: unsafe extern "system" fn(CameraHandle, c_int, *mut c_uint) -> CameraSdkStatus =
        unsafe { c.sdk.api().symbol("CameraGetInPutIOModeCapbility")? };
    let mut mask: c_uint = 0;
    // SAFETY: open handle, line 0, valid out pointer.
    c.sdk.check("CameraGetInPutIOModeCapbility", unsafe { modes(c.cam.handle(), 0, &mut mask) })?;
    println!("input 0 supported-mode mask {mask:#b}");
    raw::set_i32x2(&c.cam, "CameraSetInPutIOMode", 0, 0)?; // line 0 as trigger input (IOMODE_TRIG_INPUT)
    raw::set_i32x2(&c.cam, "CameraSetInPutIOFormat", 0, 0)?; // single-ended (IOFORMAT_SINGLE)
    Ok(())
}
// [/doc]

// [doc:output_io]
fn output_io(c: &Ctx) -> R {
    raw::set_i32x2(&c.cam, "CameraSetOutPutIOMode", 0, 3)?; // line 0 as general-purpose output (IOMODE_GP_OUTPUT)
    raw::set_i32x2(&c.cam, "CameraSetOutPutIOFormat", 0, 0)?; // single-ended (IOFORMAT_SINGLE)
    Ok(())
}
// [/doc]

// [doc:output_pulse]
fn output_pulse(c: &Ctx) -> R {
    raw::set_index_u32(&c.cam, "CameraSetOutPutPolarity", 0, 0)?; // line 0: 0 normal, 1 inverted
    raw::set_index_u32(&c.cam, "CameraSetOutPutDelayTime", 0, 100)?; // us
    raw::set_index_u32(&c.cam, "CameraSetOutPutPulseWidth", 0, 1000)?; // us
    // SAFETY: CameraSetOutPutPWM(CameraHandle, INT index, UINT iCycle, UINT uDuty).
    let pwm: unsafe extern "system" fn(CameraHandle, c_int, c_uint, c_uint) -> CameraSdkStatus =
        unsafe { c.sdk.api().symbol("CameraSetOutPutPWM")? };
    // SAFETY: open handle; line 0 in PWM mode, period 1000 us, duty 50 % (1 to 99).
    c.sdk.check("CameraSetOutPutPWM", unsafe { pwm(c.cam.handle(), 0, 1000, 50) })?;
    Ok(())
}
// [/doc]

// [doc:strobe]
fn strobe(c: &Ctx) -> R {
    c.cam.set_strobe_mode(StrobeMode::TriggerManual)?; // pulse after each trigger
    c.cam.set_strobe_polarity(true)?; // active high (default)
    c.cam.set_strobe_delay_us(0)?;
    c.cam.set_strobe_pulse_width_us(2000)?; // light on for 2 ms
    println!("{:?}", c.cam.strobe_mode()?);
    Ok(())
}
// [/doc]

// [doc:led]
fn led(c: &Ctx) -> R {
    raw::set_i32x2(&c.cam, "CameraSetLedEnable", 0, 1)?; // LED 0; models with LEDs only
    raw::set_index_u32(&c.cam, "CameraSetLedBrightness", 0, 128)?; // 0 off to 255 brightest
    raw::set_index_u32(&c.cam, "CameraSetLedDuration", 0, 10)?; // ms
    raw::set_i32x2(&c.cam, "CameraSetLedOnOff", 0, 1)?;
    Ok(())
}
// [/doc]

// [doc:lighting_controller]
fn lighting_controller(c: &Ctx) -> R {
    raw::set_i32x2(&c.cam, "CameraSetLightingControllerMode", 0, 1)?; // controller 0: 0 follow strobe, 1 manual
    raw::set_i32x2(&c.cam, "CameraSetLightingControllerState", 0, 1)?; // manual: 0 off, 1 on
    Ok(())
}
// [/doc]

// [doc:rotary_encoder]
fn rotary_encoder(c: &Ctx) -> R {
    raw::set_i32(&c.cam, "CameraSetRotaryEncDir", 1)?; // 0 both, 1 clockwise (A leads B), 2 counterclockwise
    raw::set_i32x2(&c.cam, "CameraSetRotaryEncFreq", 1, 1)?; // multiplier, divider
    Ok(())
}
// [/doc]

// ---------------------------------------------------------------------------
// 7. GigE transport
// ---------------------------------------------------------------------------

// [doc:packet_size]
fn packet_size(c: &Ctx) -> R {
    for p in c.cam.capability()?.pack_lengths {
        println!("[{}] {} bytes", p.index, p.size);
    }
    println!("using {} bytes", c.cam.set_gige_packet_size(1500)?); // largest size <= the NIC MTU
    Ok(())
}
// [/doc]

// [doc:frame_resend]
fn frame_resend(c: &Ctx) -> R {
    c.cam.set_frame_resend_count(2)?; // GigE, trigger mode; <= 0 disables resends
    Ok(())
}
// [/doc]

// ---------------------------------------------------------------------------
// 8. Parameter persistence
// ---------------------------------------------------------------------------

// [doc:parameter_groups]
fn parameter_groups(c: &Ctx) -> R {
    println!("current group {:?}", c.cam.current_parameter_team()?);
    c.cam.save_parameter(ParameterTeam::B)?; // groups A to D
    c.cam.load_parameter(ParameterTeam::B)?; // ParameterTeam::Default loads the factory settings
    Ok(())
}
// [/doc]

// [doc:parameter_file]
fn parameter_file(c: &Ctx) -> R {
    // SAFETY: CameraSaveParameterToFile(CameraHandle, char* sFileName).
    let save: unsafe extern "system" fn(CameraHandle, *mut c_char) -> CameraSdkStatus =
        unsafe { c.sdk.api().symbol("CameraSaveParameterToFile")? };
    let path = CString::new("/tmp/camera.config")?;
    // SAFETY: open handle and a NUL-terminated path.
    c.sdk.check("CameraSaveParameterToFile", unsafe { save(c.cam.handle(), path.as_ptr().cast_mut()) })?;
    Ok(()) // CameraReadParameterFromFile takes the same arguments and loads the file
}
// [/doc]

// [doc:parameter_mode]
fn parameter_mode(c: &Ctx) -> R {
    println!("{:?}", c.cam.parameter_mode()?);
    if c.cam.capability()?.param_in_device {
        c.cam.set_parameter_mode(ParameterMode::InDevice)?; // keep groups A to D in the camera
    }
    Ok(())
}
// [/doc]

// [doc:parameter_mask]
fn parameter_mask(c: &Ctx) -> R {
    raw::set_u32(&c.cam, "CameraSetParameterMask", u32::MAX)?; // which settings pages load and save; all
    Ok(())
}
// [/doc]

// [doc:sys_option]
fn sys_option(c: &Ctx) -> R {
    // SAFETY: CameraSetSysOption(char const* optionName, char const* value).
    let set: unsafe extern "system" fn(*const c_char, *const c_char) -> CameraSdkStatus =
        unsafe { c.sdk.api().symbol("CameraSetSysOption")? };
    let (name, value) = (CString::new("NumBuffers")?, CString::new("8")?);
    // SAFETY: two NUL-terminated strings; normally set before a camera is opened.
    c.sdk.check("CameraSetSysOption", unsafe { set(name.as_ptr(), value.as_ptr()) })?;
    c.sdk.gige_set_option("NumBuffers", "8")?; // wrapped GigE transport option
    Ok(())
}
// [/doc]

// [doc:data_directory]
fn data_directory(c: &Ctx) -> R {
    c.sdk.set_data_directory(std::path::Path::new("/tmp/gcam-mvsdk-data"))?; // before opening cameras
    Ok(())
}
// [/doc]

// ---------------------------------------------------------------------------
// 9. Callbacks
// ---------------------------------------------------------------------------

// [doc:frame_callback]
static FRAMES: AtomicU64 = AtomicU64::new(0);

/// Runs on an SDK thread for every frame; keep it short and never block.
unsafe extern "system" fn on_frame(_cam: CameraHandle, _data: *mut u8, _head: *mut ffi::tSdkFrameHead, _ctx: *mut c_void) {
    FRAMES.fetch_add(1, Ordering::Relaxed);
}

fn frame_callback(c: &Ctx) -> R {
    let set = c.sdk.api().CameraSetCallbackFunction.ok_or("CameraSetCallbackFunction is not exported")?;
    let null = std::ptr::null_mut();
    // SAFETY: `on_frame` matches CAMERA_SNAP_PROC and touches only an atomic counter.
    c.sdk.check("CameraSetCallbackFunction", unsafe { set(c.cam.handle(), Some(on_frame), null, null.cast()) })?;
    c.cam.play()?;
    std::thread::sleep(Duration::from_secs(1));
    println!("{} frames in 1 s", FRAMES.load(Ordering::Relaxed));
    Ok(())
}
// [/doc]

// [doc:frame_event_callback]
static FRAME_ENDS: AtomicU64 = AtomicU64::new(0);

unsafe extern "system" fn on_event(_cam: CameraHandle, event: *mut ffi::tSdkFrameEvent, _ctx: *mut c_void) {
    // uType 1: frame start (end of exposure on global-shutter sensors), 2: frame end
    if event.as_ref().is_some_and(|e| e.uType == 2) {
        FRAME_ENDS.fetch_add(1, Ordering::Relaxed);
    }
}

fn frame_event_callback(c: &Ctx) -> R {
    let set = c.sdk.api().CameraSetFrameEventCallback.ok_or("CameraSetFrameEventCallback is not exported")?;
    // SAFETY: `on_event` matches CAMERA_FRAME_EVENT_CALLBACK and touches only an atomic counter.
    c.sdk.check("CameraSetFrameEventCallback", unsafe { set(c.cam.handle(), Some(on_event), std::ptr::null_mut()) })?;
    c.cam.play()?;
    std::thread::sleep(Duration::from_secs(1));
    println!("{} frame-end events in 1 s", FRAME_ENDS.load(Ordering::Relaxed));
    Ok(())
}
// [/doc]

// [doc:connection_callback]
unsafe extern "system" fn on_connection(_cam: CameraHandle, msg: c_uint, _param: c_uint, _ctx: *mut c_void) {
    eprintln!("camera connection {}", if msg == 0 { "lost" } else { "restored" }); // 0 lost, 1 restored
}

fn connection_callback(c: &Ctx) -> R {
    let set = c.sdk.api().CameraSetConnectionStatusCallback.ok_or("CameraSetConnectionStatusCallback is not exported")?;
    // SAFETY: `on_connection` matches CAMERA_CONNECTION_STATUS_CALLBACK.
    c.sdk.check("CameraSetConnectionStatusCallback", unsafe {
        set(c.cam.handle(), Some(on_connection), std::ptr::null_mut())
    })?;
    Ok(())
}
// [/doc]

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

type Snippet = fn(&Ctx) -> R;

/// What running a snippet changes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Effect {
    /// Reads only.
    ReadOnly,
    /// Changes settings for this session; nothing is saved (needs `--write`).
    Writes,
    /// Stores something in the camera or on disk with demonstration values (needs `--write --persistent`).
    Persistent,
}

/// Section titles of docs/CAPABILITIES.md.
const SECTIONS: &[&str] = &[
    "Identity and network",
    "Exposure and gain",
    "Frame rate, trigger and timing",
    "Image geometry",
    "Image processing",
    "Digital I/O, strobe and lighting",
    "GigE transport",
    "Parameter persistence",
    "Callbacks",
];

/// Section of docs/CAPABILITIES.md, name, what running it changes, function.
const SNIPPETS: &[(u8, &str, Effect, Snippet)] = &[
    (1, "user_name", Effect::Persistent, user_name),
    (1, "gige_ip", Effect::ReadOnly, gige_ip),
    (1, "set_ip", Effect::Persistent, set_ip),
    (1, "read_sn", Effect::ReadOnly, read_sn),
    (1, "user_data", Effect::ReadOnly, user_data),
    (2, "exposure", Effect::Writes, exposure),
    (2, "auto_exposure", Effect::Writes, auto_exposure),
    (2, "ae_target", Effect::Writes, ae_target),
    (2, "ae_threshold", Effect::Writes, ae_threshold),
    (2, "ae_exposure_range", Effect::Writes, ae_exposure_range),
    (2, "ae_gain_range", Effect::Writes, ae_gain_range),
    (2, "ae_window", Effect::Writes, ae_window),
    (2, "analog_gain", Effect::Writes, analog_gain),
    (2, "analog_gain_x", Effect::ReadOnly, analog_gain_x),
    (2, "rgb_gain", Effect::Writes, rgb_gain),
    (2, "multi_exposure", Effect::ReadOnly, multi_exposure),
    (2, "anti_flick", Effect::Writes, anti_flick),
    (2, "light_frequency", Effect::Writes, light_frequency),
    (3, "frame_rate", Effect::Writes, frame_rate),
    (3, "frame_speed", Effect::Writes, frame_speed),
    (3, "trigger_mode", Effect::Writes, trigger_mode),
    (3, "soft_trigger", Effect::Writes, soft_trigger),
    (3, "trigger_count", Effect::Writes, trigger_count),
    (3, "trigger_delay", Effect::Writes, trigger_delay),
    (3, "trigger_period", Effect::Writes, trigger_period),
    (3, "external_trigger", Effect::Writes, external_trigger),
    (3, "play_pause_stop", Effect::Writes, play_pause_stop),
    (3, "clear_buffer", Effect::Writes, clear_buffer),
    (4, "resolution", Effect::Writes, resolution),
    (4, "roi", Effect::Writes, roi),
    (4, "resolution_for_snap", Effect::Writes, resolution_for_snap),
    (4, "transfer_roi", Effect::Writes, transfer_roi),
    (4, "mirror", Effect::Writes, mirror),
    (4, "hardware_mirror", Effect::Writes, hardware_mirror),
    (4, "rotate", Effect::Writes, rotate),
    (4, "inverse", Effect::Writes, inverse),
    (4, "flip_frame_buffer", Effect::Writes, flip_frame_buffer),
    (4, "splice_lines", Effect::Writes, splice_lines),
    (5, "isp_out_format", Effect::Writes, isp_out_format),
    (5, "isp_processor", Effect::Writes, isp_processor),
    (5, "monochrome", Effect::Writes, monochrome),
    (5, "gamma", Effect::Writes, gamma),
    (5, "contrast", Effect::Writes, contrast),
    (5, "saturation", Effect::Writes, saturation),
    (5, "sharpness", Effect::Writes, sharpness),
    (5, "black_level", Effect::Writes, black_level),
    (5, "white_level", Effect::Writes, white_level),
    (5, "log_curve", Effect::Writes, log_curve),
    (5, "noise_filter", Effect::Writes, noise_filter),
    (5, "noise_reduction", Effect::Writes, noise_reduction),
    (5, "denoise_3d", Effect::Writes, denoise_3d),
    (5, "dead_pixel", Effect::Writes, dead_pixel),
    (5, "bayer_algorithm", Effect::Writes, bayer_algorithm),
    (5, "white_balance", Effect::Writes, white_balance),
    (5, "wb_window", Effect::Writes, wb_window),
    (5, "color_temperature", Effect::Writes, color_temperature),
    (5, "user_color_temperature", Effect::Writes, user_color_temperature),
    (5, "once_black_balance", Effect::Writes, once_black_balance),
    (5, "lut", Effect::Writes, lut),
    (5, "undistort", Effect::Writes, undistort),
    (6, "io_state", Effect::Writes, io_state),
    (6, "input_io", Effect::Writes, input_io),
    (6, "output_io", Effect::Writes, output_io),
    (6, "output_pulse", Effect::Writes, output_pulse),
    (6, "strobe", Effect::Writes, strobe),
    (6, "led", Effect::Writes, led),
    (6, "lighting_controller", Effect::Writes, lighting_controller),
    (6, "rotary_encoder", Effect::Writes, rotary_encoder),
    (7, "packet_size", Effect::Writes, packet_size),
    (7, "frame_resend", Effect::Writes, frame_resend),
    (8, "parameter_groups", Effect::Persistent, parameter_groups),
    (8, "parameter_file", Effect::Persistent, parameter_file),
    (8, "parameter_mode", Effect::Persistent, parameter_mode),
    (8, "parameter_mask", Effect::Writes, parameter_mask),
    (8, "sys_option", Effect::Writes, sys_option),
    (8, "data_directory", Effect::Writes, data_directory),
    (9, "frame_callback", Effect::Writes, frame_callback),
    (9, "frame_event_callback", Effect::Writes, frame_event_callback),
    (9, "connection_callback", Effect::Writes, connection_callback),
];

fn main() -> R {
    let mut args = std::env::args().skip(1);
    let Some(camera) = args.next() else {
        println!("usage: capabilities <camera name or serial> [--write [--persistent]] [snippet ...]");
        return Ok(());
    };
    let (mut write, mut persistent, mut names) = (false, false, Vec::new());
    for arg in args {
        match arg.as_str() {
            "--write" => write = true,
            "--persistent" => persistent = true,
            _ => names.push(arg),
        }
    }
    if names.is_empty() {
        for (i, title) in SECTIONS.iter().enumerate() {
            println!("{}. {title}", i + 1);
            for (_, name, effect, _) in SNIPPETS.iter().filter(|s| usize::from(s.0) == i + 1) {
                let note = match effect {
                    Effect::ReadOnly => "",
                    Effect::Writes => "  (writes; --write)",
                    Effect::Persistent => "  (persistent; --write --persistent)",
                };
                println!("   {name}{note}");
            }
        }
        return Ok(());
    }
    // Check every name and flag before touching the camera.
    let mut selected: Vec<(&str, Snippet)> = Vec::new();
    for name in &names {
        let Some(&(_, _, effect, run)) = SNIPPETS.iter().find(|s| s.1 == name) else {
            eprintln!("{name}: unknown snippet");
            continue;
        };
        if effect != Effect::ReadOnly && !write {
            eprintln!("{name}: changes camera settings; add --write to run it");
        } else if effect == Effect::Persistent && !persistent {
            eprintln!("{name}: stores demonstration values persistently; add --persistent as well to run it");
        } else {
            selected.push((name, run));
        }
    }
    if selected.is_empty() {
        return Ok(());
    }
    let sdk = Sdk::load()?;
    let dev = sdk.find_device(&camera)?;
    let cam = sdk.open(&dev)?;
    let ctx = Ctx { sdk, dev, cam };
    for (name, run) in selected {
        match run(&ctx) {
            Ok(()) => println!("{name}: ok"),
            Err(e) => println!("{name}: {e}"),
        }
    }
    Ok(())
}
