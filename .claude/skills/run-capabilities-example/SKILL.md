---
name: run-capabilities-example
description: Run the capabilities example of gcam-mvsdk-rs to test, item by item, which settings in docs/CAPABILITIES.md a specific HuaTeng camera model supports. Use before relying on a feature (trigger, ROI, strobe, I/O, white balance, ...) on a new camera model, or when the user asks what a camera can do. Read-only unless --write is given; persistent items also need --persistent.
---

# run-capabilities-example

## When to use

- A project needs a feature (hardware trigger, ROI, strobe, I/O line, LUT, ...) and the camera
  model is new: check that the SDK call works on that model before building on it.
- The user asks what a particular camera supports.

`docs/CAPABILITIES.md` describes every item; this example runs the same code against a camera.

## Prerequisites

- The repository checkout and a Rust toolchain (no OpenCV needed).
- The SDK (sdk-setup) and a camera visible in camera-list, not streaming elsewhere.
- GigE: `CAP_NET_RAW` on `target/debug/examples/capabilities` or `sudo -E`.

## Steps

```bash
cd /path/to/gcam-mvsdk-rs
cargo build --example capabilities
cargo run -q --example capabilities -- sb00                               # list items by section
cargo run -q --example capabilities -- sb00 gige_ip read_sn user_data     # read-only items
cargo run -q --example capabilities -- sb00 --write ae_target trigger_count   # items that change settings
```

- The first argument is the camera (name or serial). Listing items does not open the camera.
- Several items can run in one call, in the order given. `--write` and `--persistent` may appear
  anywhere after the camera.
- Without `--write`, items that change settings are refused (`<item>: changes camera settings; add
  --write to run it`). Items that store values persistently with demonstration values are also
  refused without `--persistent`: `user_name` (renames the camera to sb03), `set_ip`
  (192.168.1.50/24), `parameter_groups` (overwrites group B), `parameter_file`
  (`/tmp/camera.config`), `parameter_mode`.
- With `sudo -E`: `cargo build --example capabilities` first, then
  `sudo -E target/debug/examples/capabilities sb00 gige_ip`.

## Reading the output

- The listing is grouped as `1. Identity and network` ... `9. Callbacks`, the section numbers of
  `docs/CAPABILITIES.md`. An item's name is the function name of the snippet shown under that
  section in the document; it is marked `(writes; --write)` or `(persistent; --write --persistent)`
  where it changes something.
- Each run prints `<item>: ok` or `<item>: <error>`, after whatever the snippet prints. An SDK
  status such as `failed with status -4 (CAMERA_STATUS_NOT_SUPPORTED)` means this model does not
  support the item; `... is not exported by the loaded MVSDK library` means the SDK build lacks it.
- `play_pause_stop` calls `CameraStop`, after which the header says the camera accepts no more
  settings; items listed after it in the same run may fail. Run it last or alone.

## If it fails

| Output | Cause and fix |
|---|---|
| `Error: DeviceNotFound { query: "sb00", available: [] }` | No camera, or wrong name; camera-list, then camera-discovery. |
| `<item>: unknown snippet` | Typo; run without items to list the names. |
| Status `-13`, `32774` or `-45` | Another program holds the camera. |
| `Error: Load(...)` / SDK not found | sdk-setup. |

## Safety

- Default to read-only items. Show the user what a `--write` item does (its snippet in
  `docs/CAPABILITIES.md`) before running it; the values are demonstrations, not the user's
  settings.
- Never pass `--persistent` unless the user explicitly asks for that specific item on that
  specific camera; these items rename the camera, change its IP address or overwrite stored
  settings.
- Settings changed by `--write` items last for the session only; restart the camera's normal
  program afterwards so it applies its own settings.
