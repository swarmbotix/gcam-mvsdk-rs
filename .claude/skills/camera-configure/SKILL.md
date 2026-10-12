---
name: camera-configure
description: Change how a HuaTeng camera captures (exposure, auto exposure, gain, frame rate, trigger mode, ROI and binning, mirror and rotation, white balance) and optionally store the settings in a parameter group, using gcam-mvsdk-rs info to read ranges first, gcam-mvsdk-rs set for frame rate and exposure, and the library for the rest. Use when the user wants to tune or persist camera settings. Reads before it writes; saving needs confirmation.
---

# camera-configure

## When to use

- The user wants different exposure, gain, frame rate, trigger behaviour, image size, orientation
  or white balance, or wants settings to stay after the program exits.
- Before relying on a setting: check that this camera model supports it and in what range.

Reference for every setting, with vendor function names and use cases: `docs/CAPABILITIES.md`.
The safe methods are in `src/controls.rs` and `src/camera.rs`.

## Prerequisites

- camera-list shows the camera; no other program streams from it.
- For library use: a Rust toolchain and this repository (or `gcam-mvsdk-rs` as a dependency).
- GigE: `CAP_NET_RAW` on the binary that opens the camera, or `sudo -E`.

## Steps

1. **Read the current values and the model's ranges** (read only):
   ```bash
   gcam-mvsdk-rs info sb00
   ```
   `info` prints exposure (time, auto on or off, range, line time), gain, current resolution and
   sensor maximum, frame-rate cap and frame-speed tier, trigger mode, gamma, contrast, saturation,
   sharpness, mono or color, and the camera's preset resolutions, frame-speed tiers, packet sizes,
   output formats and trigger modes.
2. **Frame rate, exposure and auto exposure: use the CLI first.** `set` applies only the options
   given, reads each value back and prints it with the old value:
   ```bash
   gcam-mvsdk-rs set sb00                                  # print frame rate, exposure, auto exposure
   gcam-mvsdk-rs set sb00 --frame-rate 30                  # cap at 30 Hz; 0 removes the cap (some GigE models only)
   gcam-mvsdk-rs set sb00 --exposure-us 5000               # manual exposure; turns auto exposure off
   gcam-mvsdk-rs set sb00 --auto-exposure on
   gcam-mvsdk-rs set sb00 --frame-rate 30 --exposure-us 5000 --save B   # also store in group B
   ```
   Without `--save` the change stays in RAM. For a single capture, `grab --exposure-us` works too
   (see camera-grab). Everything else goes through the library (step 3).
3. **Library: use the template** `.claude/skills/camera-configure/configure.rs`. It prints the
   current values and ranges, applies the values in its `configure()` function with `--apply`,
   and stores them with `--save A|B|C|D`:
   ```bash
   cargo new --bin cam-config && cd cam-config
   cargo add gcam-mvsdk-rs --path /path/to/gcam-mvsdk-rs --no-default-features
   cp /path/to/gcam-mvsdk-rs/.claude/skills/camera-configure/configure.rs src/main.rs
   # edit configure() in src/main.rs, then:
   cargo build --release
   sudo setcap cap_net_raw,cap_net_admin+ep target/release/cam-config   # GigE, after each build
   ./target/release/cam-config sb00                    # read only
   ./target/release/cam-config sb00 --apply            # apply for this session
   ./target/release/cam-config sb00 --apply --save B   # apply and store in group B
   ```
4. **Methods for each setting** (all on `gcam_mvsdk_rs::Camera`):

   | Setting | Read | Write |
   |---|---|---|
   | Exposure (microseconds) | `exposure_us`, `exposure_range_us` | `set_auto_exposure(false)`, then `set_exposure_us` |
   | Automatic exposure | `auto_exposure`, `ae_target` | `set_auto_exposure(true)`, `set_ae_target` |
   | Analog gain | `analog_gain` (range in `capability().expose`) | `set_analog_gain` |
   | Digital RGB gain | `rgb_gain` | `set_rgb_gain` |
   | Frame rate | `frame_rate`, `frame_speed` | `set_frame_rate` (some GigE models; 0 or less removes the cap), `set_frame_speed` |
   | Trigger | `trigger_mode`, `trigger_count`, `trigger_delay_us` | `set_trigger_mode`, `set_trigger_count`, `set_trigger_delay_us`, `soft_trigger` |
   | Resolution, ROI, binning | `image_resolution`, `capability().image_sizes` | `set_image_resolution(&preset)`, `set_roi(Roi { .. binning: Binning::Sum(2) })` |
   | Mirror, rotation | `mirror`, `rotation` | `set_mirror`, `set_rotation` |
   | White balance (color) | `auto_white_balance` | `set_auto_white_balance`, `white_balance_once` |
   | Store / load | `current_parameter_team`, `parameter_mode` | `save_parameter`, `load_parameter`, `set_parameter_mode` |

5. **Verify** with `gcam-mvsdk-rs info sb00` and a test capture (camera-grab).

## Reading the output

- **Where settings live.** A setting changed through the SDK applies to the open session. To keep
  it, store it with `save_parameter(ParameterTeam::A..D)`. When a program opens the camera
  (`Camera::open`, which calls `CameraInit` with "last mode, last group"), the SDK loads the
  parameter group and storage mode used last time, according to the header. The header does not
  document what the camera applies by itself at power-on without a program.
- **Where groups are stored** depends on `parameter_mode()`: per model, per nickname or per serial
  number in the SDK data directory of the program that opened the camera (the CLI uses
  `<SDK home>/data`; other programs use their working directory unless they call
  `Sdk::set_data_directory`), or inside the camera with `ParameterMode::InDevice` when
  `capability().param_in_device` is true. Two programs with different data directories do not
  see each other's file-based groups; in-camera storage avoids that.
- **Model dependence.** Ranges and support come from the capability block (`info`, or
  `Camera::capability()`): `expose` (AE target and gain ranges), `exposure_range_us()`,
  `resolution_range` and `image_sizes`, `frame_speeds`, `triggers`, `isp` flags (mono sensor,
  auto white balance). An unsupported setting returns an SDK error rather than being ignored.

## If it fails

| Output | Cause and fix |
|---|---|
| `... failed with status -4 (CAMERA_STATUS_NOT_SUPPORTED)` or another status | The model does not support the setting, or the value is out of range; check `info`. |
| `invalid argument: ...` | Rejected before reaching the camera (for example black level outside 0 to 255, ROI with negative offset). |
| `... is not exported by the loaded MVSDK library` | Older SDK build; update with `gcam-mvsdk-rs setup`. |
| Status `-13`, `32774` or `-45` | Another program holds the camera. |
| Settings gone after restart | They were not saved, or another program loaded a different group or data directory; see "Where settings live". |

## Safety

- Read before you write: run `info` (or the template without `--apply`) and show the user the
  current values first.
- Confirm with the user before `set --save` or `save_parameter` on a production camera; it
  overwrites the stored group that programs load next time.
- Do not change trigger mode or exposure on a camera a running publisher uses; stop it first.
