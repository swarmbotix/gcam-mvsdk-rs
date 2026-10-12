---
name: add-sdk-function
description: Expose another MVSDK (CameraApi.h) function in gcam-mvsdk-rs end to end, from the raw ffi::Api entry through a safe Camera/Sdk method to tests, without copying vendor header text. Use when a caller needs an SDK function that has no wrapper yet.
---

# add-sdk-function

Example used below: `CameraSetAnalogGain(CameraHandle hCamera, INT iAnalogGain)`.

## 1. Find the exact signature (read only; never copy into the repo)

```bash
INC="${GCAM_MVSDK_HOME:-$HOME/.gcam-mvsdk}/2.1.0.49/include"   # or vendor/2.1.0.49/include in a checkout
grep -n -A6 "CameraSetAnalogGain(" "$INC/CameraApi.h"
grep -n -A6 "def CameraSetAnalogGain" path/to/linuxSDK_*/demo/python_demo/mvsdk.py   # buffer sizes, call pattern
nm -D --defined-only vendor/2.1.0.49/lib/x64/libMVSDK.so | grep -w CameraSetAnalogGain
```

Note every pointer argument's direction and the documented buffer size (char buffers are
usually "at least 32 bytes"; this crate allocates 64 or more).

## 2. Raw entry in `src/ffi.rs`

Add one line to the `optional { ... }` block of `define_api!` (use `required` only for functions
every supported SDK build exports; a missing required symbol makes `Api::load` fail):

```rust
/// Sets the analog gain in SDK units.
fn CameraSetAnalogGain(hCamera: CameraHandle, iAnalogGain: c_int) -> CameraSdkStatus;
```

Type mapping: `int`, `INT`, `BOOL`, `LONG`, `CameraHandle`, `CameraSdkStatus` to `c_int`; `UINT`,
`DWORD`, `ULONG` to `c_uint`; `USHORT` to `u16`; `float` to `f32`; `double` to `f64`;
`char*`/`LPCTSTR` to `*mut c_char` (`*const c_char` when the header says `char const*`);
`BYTE*` to `*mut u8`; `PVOID` to `*mut c_void`; `T*` to `*mut T`; function pointers to
`Option<unsafe extern "system" fn(...)>`. Write the doc line in your own words.

If the function uses a struct not yet declared, add a `#[repr(C)]` struct with fields in header
order, then measure it:

```bash
bash .claude/skills/add-sdk-function/sizeof.sh "$INC" tSdkFrameStatistic iTotal iCapture iLost
```

and add the size and offsets to `tests/struct_sizes.rs` (use `#[cfg(target_pointer_width)]`
when the struct contains pointers).

## 3. Safe method

Per-camera functions go in `impl Camera` (`src/camera.rs`), process-wide ones in `impl Sdk`
(`src/sdk.rs`). Pattern:

```rust
/// Sets the analog gain in SDK units.
pub fn set_analog_gain(&self, gain: i32) -> Result<()> {
    let f = self.sdk.api().CameraSetAnalogGain.ok_or(Error::Unsupported("CameraSetAnalogGain"))?;
    // SAFETY: open handle, plain integer argument.
    let status = unsafe { f(self.handle, gain) };
    self.check("CameraSetAnalogGain", status)
}
```

Rules: return `Result<T>`; validate arguments before the call (`Error::InvalidArgument`); decode
text with `ffi::decode_c_chars`; encode text with `ffi::encode_sdk_text`; zero-initialise output
structs with `Default`; never keep SDK pointers beyond the call (copy tables out, as
`Capability::from_raw` does). Getters taking `*mut c_int` can reuse `Camera::get_i32`.

## 4. Tests and checks

- Pure logic (conversions, validation): unit test in the same module.
- Hardware behaviour: a test in `tests/hw/main.rs` behind `require_hw!()`, restoring any
  camera setting it changes.
- Update `ffi::tests::api_symbol_lists` only if a required symbol was added.

```bash
cargo test && cargo clippy --all-targets -- -D warnings
GCAM_MVSDK_HW_TEST=1 cargo test --test hw -- --nocapture --test-threads=1   # on the bench
```

## 5. Optional: CLI

Expose it in `src/main.rs` only if users need it (for example a new line in `info`). Update the
README command table when a command or flag changes.
