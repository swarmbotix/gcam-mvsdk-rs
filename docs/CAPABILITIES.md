# What the SDK can change

This document lists what the HuaTeng MVSDK can read and change on a camera, grouped by topic, and shows how to do each from Rust with `gcam-mvsdk-rs`. It is meant for deciding whether the SDK covers a requirement and how to call it. Every function name below is exported by `libMVSDK.so` 2.1.0.49; ranges and defaults are taken from the vendor header comments. The same outline is used in the Python package `gcam-mvsdk-py`.

## How to read it

- **wrapped**: a safe method exists on `Camera`, `Sdk` or `DeviceInfo`.
- **partly wrapped**: some of the functions have safe methods, the rest are raw.
- **raw**: no safe method yet; the snippet calls the C function through `Sdk::api()` or the `raw` helpers below. A wrong signature is undefined behaviour, so each raw call states the C declaration it relies on.
- **Model dependence**: every capability depends on the camera model. The capability struct (`Camera::capability()`, printed by `gcam-mvsdk-rs info`) says whether the camera supports a feature and in what range; each section names the fields that apply. An unsupported call returns an SDK error, which the crate reports as `Error::Sdk` with the status code and text.

Every snippet is a function in [`examples/capabilities.rs`](../examples/capabilities.rs), which CI compiles, and a test checks that this document shows the same code. To run snippets against a camera:

```bash
cargo run --example capabilities -- sb00                    # list the snippets
cargo run --example capabilities -- sb00 gige_ip read_sn    # read-only snippets
cargo run --example capabilities -- sb00 --write exposure   # snippets that change settings
```

The listing groups the snippets by the section numbers used below. Snippets that store their
demonstration values persistently (`user_name`, `set_ip`, `parameter_groups`, `parameter_file`,
`parameter_mode`) are refused unless `--persistent` is given as well.

Each snippet receives this context:

```rust
/// Result type of every snippet.
type R<T = ()> = Result<T, Box<dyn Error>>;

/// What every snippet receives: the SDK, the enumerated device, and the open camera.
struct Ctx {
    sdk: Sdk,
    dev: DeviceInfo,
    cam: Camera,
}
```

Raw calls with common shapes go through these helpers:

```rust
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
```

## Summary

| Section | Items | Wrapped | Partly wrapped | Raw |
|---|---|---|---|---|
| [1. Identity and network](#1-identity-and-network) | 4 | 2 | 0 | 2 |
| [2. Exposure and gain](#2-exposure-and-gain) | 13 | 7 | 0 | 6 |
| [3. Frame rate, trigger and timing](#3-frame-rate-trigger-and-timing) | 10 | 6 | 1 | 3 |
| [4. Image geometry](#4-image-geometry) | 9 | 3 | 0 | 6 |
| [5. Image processing](#5-image-processing) | 22 | 7 | 0 | 15 |
| [6. Digital I/O, strobe and lighting](#6-digital-io-strobe-and-lighting) | 8 | 2 | 0 | 6 |
| [7. GigE transport](#7-gige-transport) | 2 | 2 | 0 | 0 |
| [8. Parameter persistence](#8-parameter-persistence) | 6 | 3 | 0 | 3 |
| [9. Callbacks](#9-callbacks) | 3 | 0 | 0 | 3 |
| **Total** | **77** | **32** | **1** | **44** |

Section 10 lists 7 groups of vendor GUI and recording functions that are not wrapped and not needed for headless use.

## 1. Identity and network

**Model dependence.** `Capability::user_data_max_len` (`iUserDataMaxLen`) is the size of the user data area; 0 means the model has none. The user-set name exists on every model; the GigE functions apply to GigE cameras only.

### 1.1 User-set name (wrapped)

- **What it does:** Reads and writes the name (DeviceUserID) stored in the camera's non-volatile memory.
- **Use case:** Name cameras by mounting position (`sb00` to `sb03`) so publishers select them by name, not by serial number.
- **Vendor functions:** `CameraGetFriendlyName`, `CameraSetFriendlyName`
- **Note:** At most 32 bytes including the terminating NUL; the crate allows 31 bytes and rejects `\ / : * ? " < > |`. Enumeration may show a new name only after the camera restarts.
- **Rust:** `Camera::user_name`, `Camera::set_user_name`; CLI `rename`

```rust
fn user_name(c: &Ctx) -> R {
    println!("user-set name: {:?}", c.cam.user_name()?);
    c.cam.set_user_name("sb03")?; // at most 31 bytes; stored in the camera, survives power cycles
    Ok(())
}
```

### 1.2 GigE address (wrapped)

- **What it does:** Reads the camera's and the host NIC's address, mask, gateway and MAC; assigns the camera a static address (`bPersistent` TRUE) or automatic addressing (FALSE).
- **Use case:** Move a camera that answers from 169.254.x.x into the subnet of the network card it is cabled to.
- **Vendor functions:** `CameraGigeGetIp`, `CameraGigeGetMac`, `CameraGigeSetIp`
- **Note:** `CameraGigeSetIp` works even when the camera is outside the host subnet.
- **Rust:** `DeviceInfo::gige_ip`, `DeviceInfo::gige_mac`, `Sdk::gige_set_ip`; CLI `list`, `setip`

```rust
fn gige_ip(c: &Ctx) -> R {
    let ip = c.dev.gige_ip()?; // camera and host NIC: address, mask, gateway
    let mac = c.dev.gige_mac()?;
    println!("camera {} MAC {}; host {}; same subnet: {}", ip.camera, mac.camera, ip.host, ip.same_subnet());
    Ok(())
}
```

```rust
fn set_ip(c: &Ctx) -> R {
    let ip: Ipv4Addr = "192.168.1.50".parse()?;
    let mask: Ipv4Addr = "255.255.255.0".parse()?;
    c.sdk.gige_set_ip(&c.dev, ip, mask, Ipv4Addr::UNSPECIFIED, true)?; // persistent; works across subnets
    Ok(())
}
```

### 1.3 Serial numbers (raw)

- **What it does:** The camera holds three 32-byte serial numbers. Level 0 is the factory serial number; levels 1 and 2 are reserved for integrators.
- **Use case:** Store your own asset number in level 1 and read it back during commissioning.
- **Vendor functions:** `CameraReadSN`, `CameraWriteSN`
- **Note:** `CameraWriteSN` writes permanently and only accepts levels 1 and 2. It is deliberately not wrapped and not shown: a wrong call overwrites an identifier other software may rely on. The factory serial number used everywhere else is `DeviceInfo::serial`.
- **Rust:** raw

```rust
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
```

### 1.4 User data (raw)

- **What it does:** Reads and writes bytes in the camera's non-volatile user area.
- **Use case:** Keep a calibration ID or the lens focal length with the camera, so it travels when the camera is swapped between machines.
- **Vendor functions:** `CameraLoadUserData`, `CameraSaveUserData`
- **Note:** Start address plus length must stay below `Capability::user_data_max_len` (model dependent).
- **Rust:** raw

```rust
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
```

## 2. Exposure and gain

**Model dependence.** `Capability::expose` (`sExposeDesc`) holds the auto-exposure target range (`uiTargetMin`, `uiTargetMax`), the analog gain range (`uiAnalogGainMin`, `uiAnalogGainMax`) and its step (`fAnalogGainStep`); `Camera::exposure_range_us` gives the exposure range in microseconds. `Capability::isp` says whether the model supports automatic exposure (`bAutoExposure`), manual exposure (`bManualExposure`) and anti-flicker (`bAntiFlick`). `Capability::rgb_gain_range` bounds the digital gains.

### 2.1 Exposure time (wrapped)

- **What it does:** Sets and reads the exposure in microseconds while automatic exposure is off.
- **Use case:** Fixed exposure for marker detection under flicker-free LED light, so brightness does not drift between frames.
- **Vendor functions:** `CameraSetExposureTime`, `CameraGetExposureTime`, `CameraGetExposureTimeRange`, `CameraGetExposureLineTime`
- **Note:** CMOS sensors expose in whole lines (`Camera::exposure_line_time_us`), so the SDK rounds the value; read it back. Range model dependent.
- **Rust:** `Camera::exposure_us`, `set_exposure_us`, `exposure_range_us`, `exposure_line_time_us`

```rust
fn exposure(c: &Ctx) -> R {
    let (min, max, step) = c.cam.exposure_range_us()?;
    println!("exposure {:.1} us (range {min:.1} to {max:.1}, step {step:.2})", c.cam.exposure_us()?);
    c.cam.set_auto_exposure(false)?; // manual exposure
    c.cam.set_exposure_us(2000.0)?; // the sensor rounds to whole lines; read it back
    Ok(())
}
```

### 2.2 Automatic exposure (wrapped)

- **What it does:** Turns automatic exposure on or off.
- **Use case:** Outdoor or window-lit scenes whose brightness changes during the day.
- **Vendor functions:** `CameraSetAeState`, `CameraGetAeState`
- **Rust:** `Camera::auto_exposure`, `set_auto_exposure`

```rust
fn auto_exposure(c: &Ctx) -> R {
    c.cam.set_auto_exposure(true)?;
    println!("auto exposure: {}", c.cam.auto_exposure()?);
    Ok(())
}
```

### 2.3 Auto-exposure target (wrapped)

- **What it does:** Sets the image brightness automatic exposure aims for.
- **Use case:** Keep fiducials slightly darker than saturation so their edges stay sharp.
- **Vendor functions:** `CameraSetAeTarget`, `CameraGetAeTarget`
- **Note:** Range `expose.uiTargetMin..=uiTargetMax` (model dependent).
- **Rust:** `Camera::ae_target`, `set_ae_target`

```rust
fn ae_target(c: &Ctx) -> R {
    let e = c.cam.capability()?.expose; // target range uiTargetMin..=uiTargetMax
    println!("target {} (range {} to {})", c.cam.ae_target()?, e.uiTargetMin, e.uiTargetMax);
    c.cam.set_ae_target(((e.uiTargetMin + e.uiTargetMax) / 2) as i32)?;
    Ok(())
}
```

### 2.4 Auto-exposure threshold (raw)

- **What it does:** Automatic exposure stops adjusting while |target - image brightness| is below the threshold.
- **Use case:** Prevent exposure from hunting when small bright objects move through the image.
- **Vendor functions:** `CameraSetAeThreshold`, `CameraGetAeThreshold`
- **Rust:** raw

```rust
fn ae_threshold(c: &Ctx) -> R {
    raw::set_i32(&c.cam, "CameraSetAeThreshold", 5)?; // stop adjusting within 5 of the target
    println!("threshold {}", raw::get_i32(&c.cam, "CameraGetAeThreshold")?);
    Ok(())
}
```

### 2.5 Auto-exposure time range (raw)

- **What it does:** Limits the exposure times automatic exposure may choose, in microseconds.
- **Use case:** Cap exposure to keep motion blur below one pixel at the robot's maximum speed.
- **Vendor functions:** `CameraSetAeExposureRange`, `CameraGetAeExposureRange`
- **Rust:** raw

```rust
fn ae_exposure_range(c: &Ctx) -> R {
    // SAFETY: CameraSetAeExposureRange(CameraHandle, double fMin, double fMax), microseconds.
    let set: unsafe extern "system" fn(CameraHandle, f64, f64) -> CameraSdkStatus =
        unsafe { c.sdk.api().symbol("CameraSetAeExposureRange")? };
    // SAFETY: open handle, plain values.
    c.sdk.check("CameraSetAeExposureRange", unsafe { set(c.cam.handle(), 100.0, 10_000.0) })?;
    Ok(())
}
```

### 2.6 Auto-exposure gain range (raw)

- **What it does:** Limits the analog gain automatic exposure may choose, in SDK gain units.
- **Use case:** Lock gain at its minimum so automatic exposure trades only exposure time, never noise.
- **Vendor functions:** `CameraSetAeAnalogGainRange`, `CameraGetAeAnalogGainRange`
- **Rust:** raw

```rust
fn ae_gain_range(c: &Ctx) -> R {
    let e = c.cam.capability()?.expose; // analog gain range in SDK units
    raw::set_i32x2(&c.cam, "CameraSetAeAnalogGainRange", e.uiAnalogGainMin as i32, e.uiAnalogGainMin as i32)?;
    Ok(()) // auto exposure may now change only the exposure time, never the gain
}
```

### 2.7 Auto-exposure window (raw)

- **What it does:** Selects the image area automatic exposure measures.
- **Use case:** Expose for a conveyor belt in the lower half and ignore a bright ceiling light.
- **Vendor functions:** `CameraSetAeWindow`, `CameraGetAeWindow`
- **Note:** All four values 0 select a centered window of half the image size; a window outside the current resolution falls back to the same.
- **Rust:** raw

```rust
fn ae_window(c: &Ctx) -> R {
    raw::set_i32x4(&c.cam, "CameraSetAeWindow", 0, 0, 0, 0)?; // all zero: centered, half the image
    Ok(())
}
```

### 2.8 Analog gain (wrapped)

- **What it does:** Sets the sensor's analog gain in SDK units; the amplification is the value times `expose.fAnalogGainStep`.
- **Use case:** Raise gain only after exposure has reached the motion-blur limit.
- **Vendor functions:** `CameraSetAnalogGain`, `CameraGetAnalogGain`
- **Note:** Range `expose.uiAnalogGainMin..=uiAnalogGainMax` (model dependent).
- **Rust:** `Camera::analog_gain`, `set_analog_gain`

```rust
fn analog_gain(c: &Ctx) -> R {
    let e = c.cam.capability()?.expose; // range uiAnalogGainMin..=uiAnalogGainMax
    let gain = c.cam.analog_gain()?;
    println!("analog gain {gain} = x{:.2}", gain as f32 * e.fAnalogGainStep);
    c.cam.set_analog_gain(e.uiAnalogGainMin as i32)?; // lowest gain, least noise
    Ok(())
}
```

### 2.9 Analog gain as a factor (raw)

- **What it does:** The same gain expressed directly as an amplification factor (float).
- **Use case:** Configure gain from a stored calibration value without knowing the model's gain step.
- **Vendor functions:** `CameraSetAnalogGainX`, `CameraGetAnalogGainX`, `CameraGetAnalogGainXRange`
- **Rust:** raw

```rust
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
```

### 2.10 Digital gain per channel (wrapped)

- **What it does:** Sets the ISP's digital R, G, B gains; the factor is value / 100.
- **Use case:** Manual color balance under a fixed light source; on mono cameras use equal values.
- **Vendor functions:** `CameraSetGain`, `CameraGetGain`
- **Note:** Range `Capability::rgb_gain_range` (model dependent).
- **Rust:** `Camera::rgb_gain`, `set_rgb_gain`

```rust
fn rgb_gain(c: &Ctx) -> R {
    println!("digital gain R, G, B: {:?} (100 = x1.0)", c.cam.rgb_gain()?);
    c.cam.set_rgb_gain(100, 100, 100)?; // range Capability::rgb_gain_range
    Ok(())
}
```

### 2.11 Multiple exposures (raw)

- **What it does:** Several exposures per line with their own time and gamma.
- **Use case:** High-dynamic-range line-scan inspection of shiny and dark parts in one pass.
- **Vendor functions:** `CameraSetMultiExposureCount`, `CameraGetMultiExposureCount`, `CameraGetMultiExposureMaxCount`, `CameraSetMultiExposureTime`, `CameraGetMultiExposureTime`, `CameraSetMultiExposureGamma`, `CameraGetMultiExposureGamma`
- **Note:** The header states that only line-scan cameras support this; area-scan cameras return an error.
- **Rust:** raw

```rust
fn multi_exposure(c: &Ctx) -> R {
    // line-scan cameras only; area-scan cameras return an error status
    println!("max exposures per line: {}", raw::get_i32(&c.cam, "CameraGetMultiExposureMaxCount")?);
    Ok(())
}
```

### 2.12 Anti-flicker (wrapped)

- **What it does:** Makes automatic exposure choose times that cancel mains-frequency flicker.
- **Use case:** Stable brightness under fluorescent or cheap LED lighting with automatic exposure.
- **Vendor functions:** `CameraSetAntiFlick`, `CameraGetAntiFlick`
- **Note:** Acts only in automatic exposure mode; `isp.bAntiFlick` says whether the model has it.
- **Rust:** `Camera::anti_flick`, `set_anti_flick`

```rust
fn anti_flick(c: &Ctx) -> R {
    c.cam.set_auto_exposure(true)?; // anti-flicker acts only in automatic exposure
    c.cam.set_anti_flick(true)?;
    println!("anti-flicker: {}", c.cam.anti_flick()?);
    Ok(())
}
```

### 2.13 Mains frequency (wrapped)

- **What it does:** Selects the mains frequency anti-flicker compensates: 0 for 50 Hz, 1 for 60 Hz.
- **Use case:** Match the local grid when deploying the same configuration in Europe and North America.
- **Vendor functions:** `CameraSetLightFrequency`, `CameraGetLightFrequency`
- **Rust:** `Camera::light_frequency`, `set_light_frequency`

```rust
fn light_frequency(c: &Ctx) -> R {
    c.cam.set_light_frequency(LightFrequency::Hz50)?; // mains frequency of the room lighting
    println!("{:?}", c.cam.light_frequency()?);
    Ok(())
}
```

## 3. Frame rate, trigger and timing

**Model dependence.** `Capability::frame_speeds` (`pFrameSpeedDesc`) lists the frame-speed tiers and `Capability::triggers` (`pTriggerDesc`) the trigger modes. `CameraGetExtTrigCapability` returns a mask of external-trigger features (`EXT_TRIG_MASK_GRR_SHUTTER` 1, `EXT_TRIG_MASK_LEVEL_MODE` 2, `EXT_TRIG_MASK_DOUBLE_EDGE` 4, `EXT_TRIG_MASK_BUFFERED_DELAY` 8). Several functions below are marked in the header as available on some models only.

### 3.1 Frame rate cap (wrapped)

- **What it does:** Limits the frame rate (area-scan) or line rate (line-scan) in Hz; 0 or less means the maximum.
- **Use case:** Run four cameras at 30 fps to share one gigabit link.
- **Vendor functions:** `CameraSetFrameRate`, `CameraGetFrameRate`
- **Note:** The header states that only some GigE cameras support it.
- **Rust:** `Camera::frame_rate`, `set_frame_rate`

```rust
fn frame_rate(c: &Ctx) -> R {
    c.cam.set_frame_rate(30)?; // only some GigE models; <= 0 removes the cap
    println!("frame-rate cap {} Hz", c.cam.frame_rate()?);
    Ok(())
}
```

### 3.2 Frame-speed tier (wrapped)

- **What it does:** Selects a readout speed tier (usually low, normal, high, super).
- **Use case:** Select the fastest tier for the full rated frame rate; the default tier is often slower.
- **Vendor functions:** `CameraSetFrameSpeed`, `CameraGetFrameSpeed`
- **Note:** Index range 0 to `frame_speeds.len() - 1` (model dependent).
- **Rust:** `Camera::frame_speed`, `set_frame_speed`

```rust
fn frame_speed(c: &Ctx) -> R {
    let speeds = c.cam.capability()?.frame_speeds; // indices 0 to len - 1
    if let Some(last) = speeds.last() {
        c.cam.set_frame_speed(last.index)?;
    }
    println!("frame speed {} of {speeds:?}", c.cam.frame_speed()?);
    Ok(())
}
```

### 3.3 Trigger mode (wrapped)

- **What it does:** Free-running acquisition (0), software trigger (1) or hardware trigger (2).
- **Use case:** Hardware trigger from an encoder or a PLC for synchronized multi-camera capture.
- **Vendor functions:** `CameraSetTriggerMode`, `CameraGetTriggerMode`
- **Note:** Values 3 and 4 (encoder modes) exist on line-scan cameras.
- **Rust:** `Camera::trigger_mode`, `set_trigger_mode`, `TriggerMode`

```rust
fn trigger_mode(c: &Ctx) -> R {
    println!("modes: {:?}", c.cam.capability()?.triggers);
    c.cam.set_trigger_mode(TriggerMode::Hardware)?; // Continuous, Software or Hardware
    Ok(())
}
```

### 3.4 Software trigger (wrapped)

- **What it does:** Captures `trigger_count()` frames on demand in software-trigger mode.
- **Use case:** Capture exactly when a robot has stopped at a waypoint.
- **Vendor functions:** `CameraSoftTrigger`, `CameraSoftTriggerEx`
- **Note:** `CameraSoftTriggerEx` with flag `CAMERA_ST_CLEAR_BUFFER_BEFORE` (1) discards buffered frames first (raw).
- **Rust:** `Camera::soft_trigger`

```rust
fn soft_trigger(c: &Ctx) -> R {
    c.cam.set_trigger_mode(TriggerMode::Software)?;
    c.cam.play()?;
    c.cam.soft_trigger()?; // captures trigger_count() frames
    let frame = c.cam.grab(Duration::from_secs(1))?;
    println!("{}x{}", frame.width(), frame.height());
    Ok(())
}
```

### 3.5 Frames per trigger (wrapped)

- **What it does:** Number of frames captured per software or hardware trigger; default 1.
- **Use case:** Take a burst of three frames per part and keep the sharpest.
- **Vendor functions:** `CameraSetTriggerCount`, `CameraGetTriggerCount`
- **Rust:** `Camera::trigger_count`, `set_trigger_count`

```rust
fn trigger_count(c: &Ctx) -> R {
    c.cam.set_trigger_count(3)?; // frames per trigger (default 1)
    println!("{} frames per trigger", c.cam.trigger_count()?);
    Ok(())
}
```

### 3.6 Trigger delay (wrapped)

- **What it does:** Delay between a hardware trigger and the start of capture, in microseconds.
- **Use case:** Compensate the distance between a light barrier and the camera's field of view.
- **Vendor functions:** `CameraSetTriggerDelayTime`, `CameraGetTriggerDelayTime`
- **Note:** The header states that only some models support it.
- **Rust:** `Camera::trigger_delay_us`, `set_trigger_delay_us`

```rust
fn trigger_delay(c: &Ctx) -> R {
    c.cam.set_trigger_delay_us(500)?; // hardware trigger; only some models
    println!("delay {} us", c.cam.trigger_delay_us()?);
    Ok(())
}
```

### 3.7 Interval between frames of one trigger (raw)

- **What it does:** Time between the frames of a multi-frame trigger, in microseconds.
- **Use case:** Space a burst evenly to observe a moving part at known intervals.
- **Vendor functions:** `CameraSetTriggerPeriodTime`, `CameraGetTriggerPeriodTime`
- **Note:** The header states that only some models support it.
- **Rust:** raw

```rust
fn trigger_period(c: &Ctx) -> R {
    raw::set_u32(&c.cam, "CameraSetTriggerPeriodTime", 10_000)?; // 10 ms between frames of one trigger
    println!("period {} us", raw::get_u32(&c.cam, "CameraGetTriggerPeriodTime")?);
    Ok(())
}
```

### 3.8 External trigger signal (raw)

- **What it does:** Edge or level (`emExtTrigSignal`: 0 rising edge, the default; 1 falling edge; 2 high level; 3 low level), shutter type (0 standard, the default; 1 GRR on some rolling-shutter models), delay, buffered delay (triggers arriving during the delay are queued, at most 128), minimum interval and debounce time, all in microseconds with default 0.
- **Use case:** Debounce a mechanical switch, or trigger several cameras from one encoder pulse.
- **Vendor functions:** `CameraGetExtTrigCapability`, `CameraSetExtTrigSignalType`, `CameraGetExtTrigSignalType`, `CameraSetExtTrigShutterType`, `CameraGetExtTrigShutterType`, `CameraSetExtTrigDelayTime`, `CameraGetExtTrigDelayTime`, `CameraSetExtTrigBufferedDelayTime`, `CameraGetExtTrigBufferedDelayTime`, `CameraSetExtTrigIntervalTime`, `CameraGetExtTrigIntervalTime`, `CameraSetExtTrigJitterTime`, `CameraGetExtTrigJitterTime`
- **Rust:** raw

```rust
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
```

### 3.9 Play, pause, stop (partly wrapped)

- **What it does:** Starts and pauses streaming; `CameraStop` puts the SDK in its stopped state before closing.
- **Use case:** Pause streaming while reconfiguring resolution, then resume.
- **Vendor functions:** `CameraPlay`, `CameraPause`, `CameraStop`
- **Note:** After `CameraStop` the camera accepts no more settings. `Camera` closes the handle when dropped; calling `CameraStop` is not required.
- **Rust:** `Camera::play`, `pause`; `CameraStop` raw

```rust
fn play_pause_stop(c: &Ctx) -> R {
    c.cam.play()?;
    c.cam.pause()?; // keeps the settings; play() resumes
    let stop = c.sdk.api().CameraStop.ok_or("CameraStop is not exported")?;
    // SAFETY: open handle. After CameraStop the camera accepts no more settings; close it.
    c.sdk.check("CameraStop", unsafe { stop(c.cam.handle()) })?;
    Ok(())
}
```

### 3.10 Clear buffered frames (raw)

- **What it does:** Discards all frames the SDK has buffered.
- **Use case:** Make sure the next frame was exposed with new settings.
- **Vendor functions:** `CameraClearBuffer`
- **Rust:** raw

```rust
fn clear_buffer(c: &Ctx) -> R {
    raw::call(&c.cam, "CameraClearBuffer")?; // discard frames captured before a settings change
    Ok(())
}
```

## 4. Image geometry

**Model dependence.** `Capability::image_sizes` (`pImageSizeDesc`) is the table of preset resolutions, including binned and skipped modes. `Capability::resolution_range` (`sResolutionRange`) gives the minimum and maximum size and the masks of supported skip, sum-binning, average-binning and resampling factors (bit 0 is 2x2, bit 1 is 3x3, and so on). `isp.bZoomHD` says whether the camera can scale down in hardware.

### 4.1 Resolution, ROI and binning (wrapped)

- **What it does:** Selects a preset from the camera's table, or a custom region of interest (index 0xFF) with normal readout, sum or average binning, skipping, or resampling.
- **Use case:** Crop to the working area to raise the frame rate, or bin 2x2 for more sensitivity in low light.
- **Vendor functions:** `CameraSetImageResolution`, `CameraGetImageResolution`, `CameraSetImageResolutionEx`
- **Note:** `CameraSetImageResolutionEx` mode: 0 normal, 1 sum, 2 average, 3 skip, 4 resample; the header does not say whether width and height are counted before or after binning, so read `image_resolution()` afterwards.
- **Rust:** `Camera::image_resolution`, `set_image_resolution`, `set_roi`, `Roi`, `Binning`

```rust
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
```

```rust
fn roi(c: &Ctx) -> R {
    let range = c.cam.capability()?.resolution_range; // size limits and bin/skip masks
    println!("max {}x{}, sum-binning mask {:#x}", range.iWidthMax, range.iHeightMax, range.uBinSumModeMask);
    c.cam.set_roi(Roi { x: 0, y: 0, width: 640, height: 480, binning: Binning::None })?;
    let full = Roi { x: 0, y: 0, width: range.iWidthMax, height: range.iHeightMax, binning: Binning::Sum(2) };
    c.cam.set_roi(full)?; // 2x2 summing binning over the full sensor
    println!("{:?}", c.cam.image_resolution()?);
    Ok(())
}
```

### 4.2 Snapshot resolution (raw)

- **What it does:** Resolution used in snapshot mode; width and height 0 follow the preview resolution.
- **Use case:** Preview at low resolution and capture stills at full resolution.
- **Vendor functions:** `CameraSetResolutionForSnap`, `CameraGetResolutionForSnap`
- **Rust:** raw

```rust
fn resolution_for_snap(c: &Ctx) -> R {
    // SAFETY: CameraSetResolutionForSnap(CameraHandle, tSdkImageResolution*).
    let set: unsafe extern "system" fn(CameraHandle, *mut ffi::tSdkImageResolution) -> CameraSdkStatus =
        unsafe { c.sdk.api().symbol("CameraSetResolutionForSnap")? };
    let mut same_as_preview = ffi::tSdkImageResolution::default(); // width = height = 0
    // SAFETY: open handle and a struct of the correct layout.
    c.sdk.check("CameraSetResolutionForSnap", unsafe { set(c.cam.handle(), &mut same_as_preview) })?;
    Ok(())
}
```

### 4.3 Multi-region transfer (raw)

- **What it does:** The camera crops up to four regions (index 0 to 3) and transmits only those; the SDK reassembles a full frame with the rest filled black.
- **Use case:** Watch two separate inspection zones at a higher frame rate than the full sensor allows.
- **Vendor functions:** `CameraSetTransferRoi`, `CameraGetTransferRoi`, `CameraEnableTransferRoi`
- **Note:** Models without the feature return an error.
- **Rust:** raw

```rust
fn transfer_roi(c: &Ctx) -> R {
    // SAFETY: CameraSetTransferRoi(CameraHandle, int index, UINT x1, UINT y1, UINT x2, UINT y2).
    let set: unsafe extern "system" fn(CameraHandle, c_int, c_uint, c_uint, c_uint, c_uint) -> CameraSdkStatus =
        unsafe { c.sdk.api().symbol("CameraSetTransferRoi")? };
    // SAFETY: open handle; region 0, upper-left (0, 0), lower-right (639, 479).
    c.sdk.check("CameraSetTransferRoi", unsafe { set(c.cam.handle(), 0, 0, 0, 639, 479) })?;
    raw::set_u32(&c.cam, "CameraEnableTransferRoi", 0b0001)?; // regions 0 to 3, one bit each
    Ok(())
}
```

### 4.4 Mirror (wrapped)

- **What it does:** Mirrors the image horizontally or vertically in the SDK's image pipeline.
- **Use case:** Compensate a camera mounted upside down or viewing through a mirror.
- **Vendor functions:** `CameraSetMirror`, `CameraGetMirror`
- **Rust:** `Camera::mirror`, `set_mirror`, `MirrorDirection`

```rust
fn mirror(c: &Ctx) -> R {
    c.cam.set_mirror(MirrorDirection::Horizontal, true)?; // done by the SDK image pipeline
    println!("vertical mirror: {}", c.cam.mirror(MirrorDirection::Vertical)?);
    Ok(())
}
```

### 4.5 Hardware mirror (raw)

- **What it does:** The same mirroring done by the sensor, at no host CPU cost.
- **Use case:** Mirror at full frame rate on a weak embedded host.
- **Vendor functions:** `CameraSetHardwareMirror`, `CameraGetHardwareMirror`
- **Note:** The header states that only some GigE and USB3 cameras support it.
- **Rust:** raw

```rust
fn hardware_mirror(c: &Ctx) -> R {
    raw::set_i32x2(&c.cam, "CameraSetHardwareMirror", 0, 1)?; // direction 0 horizontal, 1 vertical; on
    Ok(())
}
```

### 4.6 Rotation (wrapped)

- **What it does:** Rotates the processed image counterclockwise by 0, 90, 180 or 270 degrees.
- **Use case:** Deliver upright images from a camera mounted at 90 degrees.
- **Vendor functions:** `CameraSetRotate`, `CameraGetRotate`
- **Rust:** `Camera::rotation`, `set_rotation`, `Rotation`

```rust
fn rotate(c: &Ctx) -> R {
    c.cam.set_rotation(Rotation::Ccw180)?; // counterclockwise
    println!("{:?}", c.cam.rotation()?);
    Ok(())
}
```

### 4.7 Inversion (raw)

- **What it does:** Inverts the image (a film-negative effect).
- **Use case:** Turn dark-on-light markings into light-on-dark for a detector trained that way.
- **Vendor functions:** `CameraSetInverse`, `CameraGetInverse`
- **Rust:** raw

```rust
fn inverse(c: &Ctx) -> R {
    raw::set_bool(&c.cam, "CameraSetInverse", true)?; // negative image
    println!("inverted: {}", raw::get_bool(&c.cam, "CameraGetInverse")?);
    Ok(())
}
```

### 4.8 Flip a buffer (raw)

- **What it does:** Flips an image buffer in place: 1 top to bottom, 2 left to right, 3 both (180 degrees). Works on a buffer, not on the camera.
- **Use case:** Flip only the frames you store, without changing the live stream.
- **Vendor functions:** `CameraFlipFrameBuffer`
- **Rust:** raw

```rust
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
```

### 4.9 Line splicing (raw)

- **What it does:** Combines lines from several frames into one frame; 0 (the default) disables it.
- **Use case:** Build tall images from a line-scan camera.
- **Vendor functions:** `CameraSetSpliceLines`, `CameraGetSpliceLines`
- **Note:** The header states that only line-scan cameras support it.
- **Rust:** raw

```rust
fn splice_lines(c: &Ctx) -> R {
    raw::set_i32(&c.cam, "CameraSetSpliceLines", 0)?; // line-scan only; 0 = no splicing (default)
    println!("splice lines {}", raw::get_i32(&c.cam, "CameraGetSpliceLines")?);
    Ok(())
}
```

## 5. Image processing

**Model dependence.** Ranges come from `Capability::gamma_range`, `contrast_range`, `saturation_range`, `sharpness_range` and `rgb_gain_range`. `Capability::isp` says whether the sensor is monochrome (`bMonoSensor`), whether the model supports one-shot and automatic white balance (`bWbOnce`, `bAutoWb`) and an on-camera ISP (`bDeviceIsp`, `bForceUseDeviceIsp`). `color_temperatures`, `preset_luts` and `bayer_algorithms_sw` / `bayer_algorithms_hw` list the choices for the functions that take an index. Color functions have no effect on mono cameras.

### 5.1 Output format (wrapped)

- **What it does:** Pixel format `Frame::process` produces: Mono8, Mono16, RGB8, BGR8, RGBA8, BGRA8.
- **Use case:** Mono8 for detection pipelines that expect one byte per pixel.
- **Vendor functions:** `CameraSetIspOutFormat`, `CameraGetIspOutFormat`
- **Rust:** `Camera::output_format`, `set_output_format`, `OutputFormat`

```rust
fn isp_out_format(c: &Ctx) -> R {
    c.cam.set_output_format(OutputFormat::Mono8)?; // what Frame::process and process_into produce
    println!("{:?}", c.cam.output_format());
    Ok(())
}
```

### 5.2 Where the ISP runs (raw)

- **What it does:** Runs image processing on the host (0) or in the camera (1).
- **Use case:** Offload processing to the camera to free host CPU on an embedded computer.
- **Vendor functions:** `CameraSetIspProcessor`, `CameraGetIspProcessor`
- **Rust:** raw

```rust
fn isp_processor(c: &Ctx) -> R {
    if c.cam.capability()?.isp.bDeviceIsp != 0 {
        raw::set_i32(&c.cam, "CameraSetIspProcessor", 1)?; // 0 host PC, 1 camera
    }
    println!("ISP runs on {}", raw::get_i32(&c.cam, "CameraGetIspProcessor")?);
    Ok(())
}
```

### 5.3 Color to gray (raw)

- **What it does:** Converts a color camera's output to gray in the ISP.
- **Use case:** Use a color camera with a detector trained on gray images.
- **Vendor functions:** `CameraSetMonochrome`, `CameraGetMonochrome`
- **Rust:** raw

```rust
fn monochrome(c: &Ctx) -> R {
    raw::set_bool(&c.cam, "CameraSetMonochrome", true)?; // a color camera delivers gray images
    println!("monochrome: {}", raw::get_bool(&c.cam, "CameraGetMonochrome")?);
    Ok(())
}
```

### 5.4 Gamma (wrapped)

- **What it does:** Gamma of the lookup table the ISP generates.
- **Use case:** Lift shadows for a human viewer; keep linear for measurement.
- **Vendor functions:** `CameraSetGamma`, `CameraGetGamma`
- **Note:** Takes effect only in LUT mode 0 (generated from gamma and contrast), see Lookup table.
- **Rust:** `Camera::gamma`, `set_gamma`

```rust
fn gamma(c: &Ctx) -> R {
    let range = c.cam.capability()?.gamma_range;
    c.cam.set_gamma((range.iMin + range.iMax) / 2)?; // effective in the generated-LUT mode
    println!("gamma {} (range {} to {})", c.cam.gamma()?, range.iMin, range.iMax);
    Ok(())
}
```

### 5.5 Contrast (wrapped)

- **What it does:** Contrast of the generated lookup table.
- **Use case:** Separate low-contrast markings from their background.
- **Vendor functions:** `CameraSetContrast`, `CameraGetContrast`
- **Note:** Takes effect only in LUT mode 0.
- **Rust:** `Camera::contrast`, `set_contrast`

```rust
fn contrast(c: &Ctx) -> R {
    let range = c.cam.capability()?.contrast_range;
    c.cam.set_contrast((range.iMin + range.iMax) / 2)?; // effective in the generated-LUT mode
    println!("contrast {} (range {} to {})", c.cam.contrast()?, range.iMin, range.iMax);
    Ok(())
}
```

### 5.6 Saturation (wrapped)

- **What it does:** Color saturation; 100 leaves colors unchanged.
- **Use case:** Strengthen colors for color-based sorting.
- **Vendor functions:** `CameraSetSaturation`, `CameraGetSaturation`
- **Note:** No effect on mono cameras.
- **Rust:** `Camera::saturation`, `set_saturation`

```rust
fn saturation(c: &Ctx) -> R {
    c.cam.set_saturation(100)?; // 100 = colors unchanged; no effect on mono cameras
    println!("saturation {}", c.cam.saturation()?);
    Ok(())
}
```

### 5.7 Sharpness (wrapped)

- **What it does:** Sharpening strength; 0 disables it. The header gives the typical range as 0 to 100.
- **Use case:** Turn sharpening off before sub-pixel edge measurement.
- **Vendor functions:** `CameraSetSharpness`, `CameraGetSharpness`
- **Rust:** `Camera::sharpness`, `set_sharpness`

```rust
fn sharpness(c: &Ctx) -> R {
    c.cam.set_sharpness(0)?; // 0 = no sharpening
    println!("sharpness {} (range {:?})", c.cam.sharpness()?, c.cam.capability()?.sharpness_range);
    Ok(())
}
```

### 5.8 Black level (wrapped)

- **What it does:** Black reference level, 0 to 255, default 0.
- **Use case:** Remove a dark offset so that unlit background reads 0.
- **Vendor functions:** `CameraSetBlackLevel`, `CameraGetBlackLevel`
- **Rust:** `Camera::black_level`, `set_black_level`

```rust
fn black_level(c: &Ctx) -> R {
    c.cam.set_black_level(0)?; // 0 to 255, default 0
    println!("black level {}", c.cam.black_level()?);
    Ok(())
}
```

### 5.9 White level (raw)

- **What it does:** White reference level, 0 to 255, default 255.
- **Use case:** Stretch a dim scene to the full 8-bit range.
- **Vendor functions:** `CameraSetWhiteLevel`, `CameraGetWhiteLevel`
- **Rust:** raw

```rust
fn white_level(c: &Ctx) -> R {
    raw::set_i32(&c.cam, "CameraSetWhiteLevel", 255)?; // 0 to 255, default 255
    println!("white level {}", raw::get_i32(&c.cam, "CameraGetWhiteLevel")?);
    Ok(())
}
```

### 5.10 Logarithmic curve (raw)

- **What it does:** Strength of a logarithmic tone curve, 0 to 255.
- **Use case:** Compress highlights in scenes with very bright and very dark areas.
- **Vendor functions:** `CameraSetLogarithmicCurveValue`, `CameraGetLogarithmicCurveValue`
- **Rust:** raw

```rust
fn log_curve(c: &Ctx) -> R {
    raw::set_i32(&c.cam, "CameraSetLogarithmicCurveValue", 0)?; // 0 to 255
    println!("log curve {}", raw::get_i32(&c.cam, "CameraGetLogarithmicCurveValue")?);
    Ok(())
}
```

### 5.11 Noise filter (raw)

- **What it does:** Turns the ISP's spatial noise filter on or off.
- **Use case:** Reduce noise at high gain when fine detail does not matter.
- **Vendor functions:** `CameraSetNoiseFilter`, `CameraGetNoiseFilterState`
- **Rust:** raw

```rust
fn noise_filter(c: &Ctx) -> R {
    raw::set_bool(&c.cam, "CameraSetNoiseFilter", true)?;
    println!("noise filter: {}", raw::get_bool(&c.cam, "CameraGetNoiseFilterState")?);
    Ok(())
}
```

### 5.12 Noise reduction strength (raw)

- **What it does:** Noise-reduction coefficient, 0 to 7.
- **Use case:** Tune noise reduction against detail loss.
- **Vendor functions:** `CameraSetNoiseReductionValue`, `CameraGetNoiseReductionValue`
- **Rust:** raw

```rust
fn noise_reduction(c: &Ctx) -> R {
    raw::set_i32(&c.cam, "CameraSetNoiseReductionValue", 3)?; // 0 to 7
    println!("noise reduction {}", raw::get_i32(&c.cam, "CameraGetNoiseReductionValue")?);
    Ok(())
}
```

### 5.13 Temporal (3D) noise reduction (raw)

- **What it does:** Averages 2 to 8 consecutive frames with optional weights.
- **Use case:** Low-noise images of a static scene at high gain.
- **Vendor functions:** `CameraSetDenoise3DParams`, `CameraGetDenoise3DParams`, `CameraManualDenoise3D`
- **Note:** Moving objects blur; `CameraManualDenoise3D` applies the same to frames you collected.
- **Rust:** raw

```rust
fn denoise_3d(c: &Ctx) -> R {
    // SAFETY: CameraSetDenoise3DParams(CameraHandle, BOOL bEnable, int nCount, float* Weights).
    let set: unsafe extern "system" fn(CameraHandle, c_int, c_int, *mut f32) -> CameraSdkStatus =
        unsafe { c.sdk.api().symbol("CameraSetDenoise3DParams")? };
    let mut weights = [0.3f32, 0.3, 0.4]; // one weight per frame; the newest weighs most
    // SAFETY: open handle; 3 frames (2 to 8 allowed) and 3 weights.
    c.sdk.check("CameraSetDenoise3DParams", unsafe { set(c.cam.handle(), 1, 3, weights.as_mut_ptr()) })?;
    Ok(())
}
```

### 5.14 Dead-pixel correction (raw)

- **What it does:** Replaces known defective pixels.
- **Use case:** Avoid false detections from a stuck pixel.
- **Vendor functions:** `CameraSetCorrectDeadPixel`, `CameraGetCorrectDeadPixel`
- **Rust:** raw

```rust
fn dead_pixel(c: &Ctx) -> R {
    raw::set_bool(&c.cam, "CameraSetCorrectDeadPixel", true)?;
    println!("dead-pixel correction: {}", raw::get_bool(&c.cam, "CameraGetCorrectDeadPixel")?);
    Ok(())
}
```

### 5.15 Bayer interpolation (raw)

- **What it does:** Selects the demosaicing algorithm for host (0) or camera (1) processing.
- **Use case:** Trade quality against CPU time on color cameras.
- **Vendor functions:** `CameraSetBayerDecAlgorithm`, `CameraGetBayerDecAlgorithm`
- **Note:** Index from `bayer_algorithms_sw` or `bayer_algorithms_hw` (model dependent).
- **Rust:** raw

```rust
fn bayer_algorithm(c: &Ctx) -> R {
    let algorithms = c.cam.capability()?.bayer_algorithms_sw; // empty on mono cameras
    if let Some(a) = algorithms.first() {
        raw::set_i32x2(&c.cam, "CameraSetBayerDecAlgorithm", 0, a.index)?; // 0 = host ISP
    }
    Ok(())
}
```

### 5.16 White balance (wrapped)

- **What it does:** Automatic white balance, or manual mode with a one-shot balance applied from the next frame.
- **Use case:** Balance once on a gray card at installation, then keep colors fixed.
- **Vendor functions:** `CameraSetWbMode`, `CameraGetWbMode`, `CameraSetOnceWB`
- **Note:** `isp.bAutoWb` and `isp.bWbOnce` say what the model supports.
- **Rust:** `Camera::auto_white_balance`, `set_auto_white_balance`, `white_balance_once`

```rust
fn white_balance(c: &Ctx) -> R {
    c.cam.set_auto_white_balance(false)?; // manual: balance once on a gray target
    c.cam.white_balance_once()?; // applied with the next frame
    println!("automatic white balance: {}", c.cam.auto_white_balance()?);
    Ok(())
}
```

### 5.17 White-balance window (raw)

- **What it does:** Image area the white balance measures.
- **Use case:** Balance on a gray reference patch in a known place.
- **Vendor functions:** `CameraSetWbWindow`, `CameraGetWbWindow`
- **Rust:** raw

```rust
fn wb_window(c: &Ctx) -> R {
    raw::set_i32x4(&c.cam, "CameraSetWbWindow", 0, 0, 200, 200)?; // x, y, width, height of the reference area
    Ok(())
}
```

### 5.18 Color temperature (raw)

- **What it does:** Color-temperature mode (`emSdkClrTmpMode`: 0 automatic, 1 preset, 2 user) and preset selection.
- **Use case:** Fix the color temperature of a known light source.
- **Vendor functions:** `CameraSetClrTempMode`, `CameraGetClrTempMode`, `CameraSetPresetClrTemp`, `CameraGetPresetClrTemp`
- **Note:** Preset indices from `Capability::color_temperatures`.
- **Rust:** raw

```rust
fn color_temperature(c: &Ctx) -> R {
    println!("presets: {:?}", c.cam.capability()?.color_temperatures);
    raw::set_i32(&c.cam, "CameraSetClrTempMode", 1)?; // 0 automatic, 1 preset, 2 user gains and matrix
    raw::set_i32(&c.cam, "CameraSetPresetClrTemp", 0)?; // preset index from the list above
    Ok(())
}
```

### 5.19 User color gains and matrix (raw)

- **What it does:** In user mode: digital gains (0 to 400 meaning 0x to 4x) and a 3x3 color matrix.
- **Use case:** Apply a color calibration measured with a chart.
- **Vendor functions:** `CameraSetUserClrTempGain`, `CameraGetUserClrTempGain`, `CameraSetUserClrTempMatrix`, `CameraGetUserClrTempMatrix`
- **Rust:** raw

```rust
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
```

### 5.20 Black balance (raw)

- **What it does:** Performs one black balance.
- **Use case:** Equalize channel offsets after warm-up, with the lens covered.
- **Vendor functions:** `CameraSetOnceBB`
- **Rust:** raw

```rust
fn once_black_balance(c: &Ctx) -> R {
    raw::call(&c.cam, "CameraSetOnceBB")?; // one black balance; cover the lens first
    Ok(())
}
```

### 5.21 Lookup table (raw)

- **What it does:** LUT mode (`emSdkLutMode`: 0 generated from gamma and contrast, 1 preset, 2 user) and the tables; a table has 4096 entries, one per 12-bit input value.
- **Use case:** Load a custom tone curve, for example one that linearizes a sensor response.
- **Vendor functions:** `CameraSetLutMode`, `CameraGetLutMode`, `CameraSelectLutPreset`, `CameraGetLutPresetSel`, `CameraSetCustomLut`, `CameraGetCustomLut`, `CameraGetCurrentLut`
- **Note:** Preset indices from `Capability::preset_luts`.
- **Rust:** raw

```rust
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
```

### 5.22 Lens undistortion (raw)

- **What it does:** Removes lens distortion in the SDK from intrinsics (fx, fy, cx, cy) and distortion coefficients (k1, k2, p1, p2, k3).
- **Use case:** Straight lines before ArUco detection, without an OpenCV remap in your own code.
- **Vendor functions:** `CameraSetUndistortParams`, `CameraGetUndistortParams`, `CameraSetUndistortEnable`, `CameraGetUndistortEnable`
- **Note:** Use values from your own calibration; the snippet's numbers are placeholders.
- **Rust:** raw

```rust
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
```

## 6. Digital I/O, strobe and lighting

**Model dependence.** `Capability::input_io_count` and `output_io_count` (`iInputIoCounts`, `iOutputIoCounts`) give the number of programmable lines, numbered from 0. Per line, `CameraGetInPutIOModeCapbility` / `CameraGetOutPutIOModeCapbility` and the matching format functions return masks of supported modes (`emCameraGPIOMode`) and electrical formats (`emCameraGPIOFormat`). LEDs and light controllers exist only on the models the header names.

### 6.1 Line state (wrapped)

- **What it does:** Reads an input line and drives or reads an output line; 1 is high, 0 low on every model.
- **Use case:** Read a part-present sensor, or switch a reject valve from the vision result.
- **Vendor functions:** `CameraSetIOStateEx`, `CameraGetIOStateEx`, `CameraGetOutPutIOStateEx`, `CameraSetIOState`, `CameraGetIOState`, `CameraGetOutPutIOState`
- **Note:** The older functions without `Ex` invert the level on GE and SUA models and are marked obsolete in the header; the wrapper uses the `Ex` functions.
- **Rust:** `Camera::input_io_state`, `output_io_state`, `set_output_io_state`

```rust
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
```

### 6.2 Input line configuration (raw)

- **What it does:** Assigns an input line its function (trigger input, general input, encoder input) and format (single-ended, RS422, RS422 terminated, optocoupler).
- **Use case:** Wire an encoder to line 0 and use it as the trigger source.
- **Vendor functions:** `CameraSetInPutIOMode`, `CameraGetInPutIOMode`, `CameraGetInPutIOModeCapbility`, `CameraSetInPutIOFormat`, `CameraGetInPutIOFormat`, `CameraGetInPutIOFormatCapbility`
- **Rust:** raw

```rust
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
```

### 6.3 Output line configuration (raw)

- **What it does:** Assigns an output line its function (strobe, general output, PWM, line strobe) and format.
- **Use case:** Turn a line into a general-purpose output before driving it with `set_output_io_state`.
- **Vendor functions:** `CameraSetOutPutIOMode`, `CameraGetOutPutIOMode`, `CameraGetOutPutIOModeCapbility`, `CameraSetOutPutIOFormat`, `CameraGetOutPutIOFormat`, `CameraGetOutPutIOFormatCapbility`
- **Rust:** raw

```rust
fn output_io(c: &Ctx) -> R {
    raw::set_i32x2(&c.cam, "CameraSetOutPutIOMode", 0, 3)?; // line 0 as general-purpose output (IOMODE_GP_OUTPUT)
    raw::set_i32x2(&c.cam, "CameraSetOutPutIOFormat", 0, 0)?; // single-ended (IOFORMAT_SINGLE)
    Ok(())
}
```

### 6.4 Output pulse and PWM (raw)

- **What it does:** Per output line: polarity (0 normal, 1 inverted), delay and pulse width in microseconds, PWM period in microseconds and duty cycle 1 to 99 %.
- **Use case:** Dim a ring light with PWM, or time a pulse to an external actuator.
- **Vendor functions:** `CameraSetOutPutPolarity`, `CameraSetOutPutDelayTime`, `CameraSetOutPutPulseWidth`, `CameraSetOutPutPWM`
- **Rust:** raw

```rust
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
```

### 6.5 Strobe (wrapped)

- **What it does:** The STROBE signal: automatic during exposure, delayed pulse of a set width after the trigger, or held high or low; active level selectable (default high).
- **Use case:** Fire an LED flash only during exposure to freeze motion and save power.
- **Vendor functions:** `CameraSetStrobeMode`, `CameraGetStrobeMode`, `CameraSetStrobePolarity`, `CameraGetStrobePolarity`, `CameraSetStrobeDelayTime`, `CameraGetStrobeDelayTime`, `CameraSetStrobePulseWidth`, `CameraGetStrobePulseWidth`
- **Rust:** `Camera::strobe_mode`, `set_strobe_mode`, `set_strobe_polarity`, `set_strobe_delay_us`, `set_strobe_pulse_width_us`, `StrobeMode`

```rust
fn strobe(c: &Ctx) -> R {
    c.cam.set_strobe_mode(StrobeMode::TriggerManual)?; // pulse after each trigger
    c.cam.set_strobe_polarity(true)?; // active high (default)
    c.cam.set_strobe_delay_us(0)?;
    c.cam.set_strobe_pulse_width_us(2000)?; // light on for 2 ms
    println!("{:?}", c.cam.strobe_mode()?);
    Ok(())
}
```

### 6.6 Camera LEDs (raw)

- **What it does:** Enable, switch, brightness (0 off to 255) and duration (milliseconds) of built-in LEDs.
- **Use case:** Use a camera's built-in illumination without an external controller.
- **Vendor functions:** `CameraSetLedEnable`, `CameraGetLedEnable`, `CameraSetLedOnOff`, `CameraGetLedOnOff`, `CameraSetLedBrightness`, `CameraGetLedBrightness`, `CameraSetLedDuration`, `CameraGetLedDuration`
- **Note:** Models without LEDs return an error.
- **Rust:** raw

```rust
fn led(c: &Ctx) -> R {
    raw::set_i32x2(&c.cam, "CameraSetLedEnable", 0, 1)?; // LED 0; models with LEDs only
    raw::set_index_u32(&c.cam, "CameraSetLedBrightness", 0, 128)?; // 0 off to 255 brightest
    raw::set_index_u32(&c.cam, "CameraSetLedDuration", 0, 10)?; // ms
    raw::set_i32x2(&c.cam, "CameraSetLedOnOff", 0, 1)?;
    Ok(())
}
```

### 6.7 Light controller (raw)

- **What it does:** Mode (0 follow strobe, 1 manual) and manual state (0 off, 1 on) of an attached light controller.
- **Use case:** Switch the inspection light from software between runs.
- **Vendor functions:** `CameraSetLightingControllerMode`, `CameraSetLightingControllerState`
- **Note:** The header states that the smart-camera series with suitable hardware is required.
- **Rust:** raw

```rust
fn lighting_controller(c: &Ctx) -> R {
    raw::set_i32x2(&c.cam, "CameraSetLightingControllerMode", 0, 1)?; // controller 0: 0 follow strobe, 1 manual
    raw::set_i32x2(&c.cam, "CameraSetLightingControllerState", 0, 1)?; // manual: 0 off, 1 on
    Ok(())
}
```

### 6.8 Rotary encoder (raw)

- **What it does:** Valid direction (0 both, 1 clockwise with phase A leading B, 2 counterclockwise) and frequency multiplier and divider of an encoder input.
- **Use case:** Trigger one line per millimeter of conveyor travel on a line-scan camera.
- **Vendor functions:** `CameraSetRotaryEncDir`, `CameraGetRotaryEncDir`, `CameraSetRotaryEncFreq`, `CameraGetRotaryEncFreq`
- **Rust:** raw

```rust
fn rotary_encoder(c: &Ctx) -> R {
    raw::set_i32(&c.cam, "CameraSetRotaryEncDir", 1)?; // 0 both, 1 clockwise (A leads B), 2 counterclockwise
    raw::set_i32x2(&c.cam, "CameraSetRotaryEncFreq", 1, 1)?; // multiplier, divider
    Ok(())
}
```

## 7. GigE transport

**Model dependence.** `Capability::pack_lengths` (`pPackLenDesc`) lists the packet sizes the camera offers. Both functions apply to GigE cameras.

### 7.1 Packet size (wrapped)

- **What it does:** Selects the GigE packet size by index into the camera's table.
- **Use case:** Use jumbo packets on a NIC with MTU 9000 for full frame rate; stay at or below 1500 otherwise.
- **Vendor functions:** `CameraSetTransPackLen`, `CameraGetTransPackLen`
- **Note:** `set_gige_packet_size(mtu)` picks the largest offered size that fits.
- **Rust:** `Camera::set_gige_packet_size`, `set_trans_pack_len`; CLI `grab --mtu`

```rust
fn packet_size(c: &Ctx) -> R {
    for p in c.cam.capability()?.pack_lengths {
        println!("[{}] {} bytes", p.index, p.size);
    }
    println!("using {} bytes", c.cam.set_gige_packet_size(1500)?); // largest size <= the NIC MTU
    Ok(())
}
```

### 7.2 Frame resend (wrapped)

- **What it does:** How often the camera resends a frame the host did not confirm, in trigger mode; 0 or less disables resends.
- **Use case:** Recover triggered frames on a lossy link without re-triggering.
- **Vendor functions:** `CameraSetFrameResendCount`
- **Note:** The header states GigE cameras only; the SDK has no getter for this value.
- **Rust:** `Camera::set_frame_resend_count`

```rust
fn frame_resend(c: &Ctx) -> R {
    c.cam.set_frame_resend_count(2)?; // GigE, trigger mode; <= 0 disables resends
    Ok(())
}
```

## 8. Parameter persistence

**Model dependence.** `Capability::param_in_device` (`bParamInDevice`) says whether the camera can store parameter groups itself (`ParameterMode::InDevice`). File-based groups are written to the SDK data directory.

### 8.1 Parameter groups A to D (wrapped)

- **What it does:** Saves the current settings to group A, B, C or D and loads a group, or the factory defaults.
- **Use case:** Keep a day and a night configuration and switch between them.
- **Vendor functions:** `CameraSaveParameter`, `CameraLoadParameter`, `CameraGetCurrentParameterGroup`
- **Rust:** `Camera::save_parameter`, `load_parameter`, `current_parameter_team`, `ParameterTeam`

```rust
fn parameter_groups(c: &Ctx) -> R {
    println!("current group {:?}", c.cam.current_parameter_team()?);
    c.cam.save_parameter(ParameterTeam::B)?; // groups A to D
    c.cam.load_parameter(ParameterTeam::B)?; // ParameterTeam::Default loads the factory settings
    Ok(())
}
```

### 8.2 Parameter files (raw)

- **What it does:** Saves the settings to a file and loads them back, also on another computer.
- **Use case:** Version-control a camera configuration and roll it out to identical cameras.
- **Vendor functions:** `CameraSaveParameterToFile`, `CameraReadParameterFromFile`
- **Rust:** raw

```rust
fn parameter_file(c: &Ctx) -> R {
    // SAFETY: CameraSaveParameterToFile(CameraHandle, char* sFileName).
    let save: unsafe extern "system" fn(CameraHandle, *mut c_char) -> CameraSdkStatus =
        unsafe { c.sdk.api().symbol("CameraSaveParameterToFile")? };
    let path = CString::new("/tmp/camera.config")?;
    // SAFETY: open handle and a NUL-terminated path.
    c.sdk.check("CameraSaveParameterToFile", unsafe { save(c.cam.handle(), path.as_ptr().cast_mut()) })?;
    Ok(()) // CameraReadParameterFromFile takes the same arguments and loads the file
}
```

### 8.3 Parameter storage mode (wrapped)

- **What it does:** Where groups are stored: per model, per nickname, per serial number, or in the camera.
- **Use case:** Give each camera its own settings (by serial) even when several cameras share a model.
- **Vendor functions:** `CameraSetParameterMode`, `CameraGetParameterMode`
- **Rust:** `Camera::parameter_mode`, `set_parameter_mode`, `ParameterMode`

```rust
fn parameter_mode(c: &Ctx) -> R {
    println!("{:?}", c.cam.parameter_mode()?);
    if c.cam.capability()?.param_in_device {
        c.cam.set_parameter_mode(ParameterMode::InDevice)?; // keep groups A to D in the camera
    }
    Ok(())
}
```

### 8.4 Parameter mask (raw)

- **What it does:** Selects which settings pages are loaded and saved (`emSdkPropSheetMask`, values 0 to 16 for exposure, color, LUT, resolution, I/O, trigger and so on).
- **Use case:** Load only exposure settings from a group, keeping the current resolution.
- **Vendor functions:** `CameraSetParameterMask`
- **Note:** The header names the type a mask but does not document the bit layout; the snippet selects everything.
- **Rust:** raw

```rust
fn parameter_mask(c: &Ctx) -> R {
    raw::set_u32(&c.cam, "CameraSetParameterMask", u32::MAX)?; // which settings pages load and save; all
    Ok(())
}
```

### 8.5 System options (raw)

- **What it does:** Process-wide SDK options such as the number of frame buffers, usually set before a camera is opened.
- **Use case:** More buffers smooth out packet jitter at high frame rates.
- **Vendor functions:** `CameraSetSysOption`, `CameraGigeSetOption`
- **Note:** `Sdk::gige_set_option` wraps the GigE variant.
- **Rust:** raw

```rust
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
```

### 8.6 Data directory (wrapped)

- **What it does:** Directory for the files the SDK writes when a camera is opened (`.config`, `.mvdat`); the default is the current directory. Set it before opening cameras.
- **Use case:** Keep camera data files out of the working directory of your service.
- **Vendor functions:** `CameraSetDataDirectory`
- **Note:** The CLI uses `<SDK home>/data`.
- **Rust:** `Sdk::set_data_directory`; CLI `--data-dir`

```rust
fn data_directory(c: &Ctx) -> R {
    c.sdk.set_data_directory(std::path::Path::new("/tmp/gcam-mvsdk-data"))?; // before opening cameras
    Ok(())
}
```

## 9. Callbacks

**Model dependence.** Callbacks are SDK features; the capability struct has no flags for them. A callback runs on an SDK thread and must return quickly. The header does not document how to remove a callback; closing the camera ends it.

### 9.1 Frame callback (raw)

- **What it does:** Calls a function for every frame instead of polling with `grab`.
- **Use case:** Push frames into a queue with the lowest latency the SDK offers.
- **Vendor functions:** `CameraSetCallbackFunction`
- **Note:** A safe callback wrapper is not implemented yet; polling `grab` covers the current use cases.
- **Rust:** raw (`Sdk::api().CameraSetCallbackFunction`)

```rust
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
```

### 9.2 Frame events (raw)

- **What it does:** Calls a function at frame start and frame end; on global-shutter sensors frame start means the end of exposure.
- **Use case:** Move a robot as soon as exposure has finished, before the image has been transferred.
- **Vendor functions:** `CameraSetFrameEventCallback`
- **Rust:** raw (`Sdk::api().CameraSetFrameEventCallback`, `ffi::tSdkFrameEvent`)

```rust
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
```

### 9.3 Connection status (raw)

- **What it does:** Calls a function when the connection is lost (0) and when it is restored (1).
- **Use case:** Alert an operator or restart a pipeline when a GigE cable is unplugged.
- **Vendor functions:** `CameraSetConnectionStatusCallback`
- **Rust:** raw (`Sdk::api().CameraSetConnectionStatusCallback`)

```rust
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
```

## 10. Vendor GUI and recording (not wrapped)

| Functions | What they do | Why not wrapped |
|---|---|---|
| `CameraDisplayInit`, `CameraDisplayRGB24`, `CameraSetDisplayMode`, `CameraSetDisplayOffset`, `CameraSetDisplaySize` | The SDK's own display into a native window handle. | Needs a platform window handle; show frames with your own toolkit (see the live view example). |
| `CameraSetCrossLine`, `CameraGetCrossLine`, `CameraImageOverlay` | Up to nine crosshair lines drawn into frames by the SDK. | Drawing belongs in the application. |
| `CameraCreateSettingPage`, `CameraShowSettingPage`, `CameraSetActiveSettingSubPage` | The vendor's settings dialog and its pages. | Vendor GUI. |
| `CameraSetMediaType`, `CameraGetMediaType` | Selects the camera's raw output format by index into `Capability::media_types`. | Changes the raw frame layout; `info` lists the formats. Not wrapped yet. |
| `CameraSetMediaBitRate`, `CameraGetMediaBitRate` | Bit rate of H264 and H265 output formats. | Only for compressed output formats. |
| `CameraInitRecord`, `CameraStopRecord` | SDK video recording (uncompressed or MSCV). | Record with your own encoder. |
| `CameraSaveImage` | Saves a frame as BMP, JPG, PNG or RAW. | `gcam-mvsdk-rs grab` writes PNG; use an image crate in your code. |
