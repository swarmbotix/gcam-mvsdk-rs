---
name: run-live-view
description: Show a live image from one HuaTeng camera in an OpenCV window with `gcam-mvsdk-rs view` (feature view), and record PNG images from it (r starts, c stops; rate and duration from a YAML file given with --config, output under the SDK data directory). Use when the user wants to see the camera image live, check focus or framing interactively, confirm streaming works with a display attached, or save a short image sequence from the live view.
---

# run-live-view

## When to use

- The user wants to watch the camera live (focus, framing, lighting) on a machine with a display.
- Interactive check that streaming works end to end.
- Record a short image sequence (default 5 images per second for 30 s) while watching.

For still images on a headless machine use **camera-grab**; for the delivered frame rate without
a window use `gcam-mvsdk-rs measure`.

## Prerequisites

- The command built with the `view` feature. `install.bash` adds it when OpenCV 4 and libclang
  are installed (the `opencv` crate binds the system OpenCV). On Ubuntu:
  ```bash
  sudo apt-get install -y libopencv-dev clang libclang-dev
  bash install.bash                                   # or: cargo install --path . --locked --features view
  gcam-mvsdk-rs view --help                           # "unrecognized subcommand" means built without the feature
  ```
  Other distributions: https://github.com/twistedfall/opencv-rust#getting-opencv
- A running display: a desktop session with `DISPLAY` set, or in Docker the X socket shared into
  the container and `xhost +local:` run on the host.
- camera-list shows the camera; no other program streams from it.

## Steps

```bash
gcam-mvsdk-rs view sb00
gcam-mvsdk-rs view sb00 --mtu 1500                    # if frames time out
gcam-mvsdk-rs view sb00 --config examples/record.yaml # recording options from a file
```

Arguments: camera name or serial (may be omitted when exactly one camera is present),
`--exposure-us US` (default 5000, automatic exposure off), `--frame-speed IDX` (sensor readout
tier from `info`; default the fastest the camera lists, so the window shows the full rate
whatever the camera's stored defaults), `--mtu BYTES` (packet size, as for `grab`),
`--config FILE` (recording options; defaults without it). Nothing is persisted in the camera.

Keys in the window: `r` (or `R`) starts a recording, `c` stops it before the duration is over,
`q` or Esc quits (a running recording is finished first); closing the window also quits. The
bottom line of the overlay lists the keys that act in the current state.

## Recording

`examples/record.yaml` is a sample of the three accepted keys; pass your own file with `--config`:

```yaml
duration_s: 30   # a recording stops after this many seconds
rate_hz: 5       # images saved per second; the camera keeps running at its own frame rate
# data_dir: ./data   # default: the SDK data directory ($GCAM_MVSDK_DATA_DIR, else <sdk home>/data)
```

Each recording creates `<data_dir>/<YYYYMMDD_HHMMSS>_<camera>/` (local time at the key press;
`<camera>` is the user-set name, else the serial) and writes `0.png`, `1.png`, ... there: the
Mono8 camera image without the overlay. One image is saved per `1/rate_hz` seconds, anchored to
the start, so a 90 fps camera at `rate_hz: 5` yields about 150 files in 30 s. Unknown keys or a
non-positive number in the file stop the program at startup; a missing `--config` file is an error.

Unit tests for the keys, the config file and the sampling (no camera or display, OpenCV needed to
build): `cargo test --features view`.

## Reading the output

- The window title is `gcam-mvsdk-rs view`; the image is the camera's Mono8 output, scaled to the
  window.
- The overlay in the top-left corner shows the frames delivered in the last second
  (`measured NN.N fps, cap ...`). While recording, a second line reads `REC 23 saved  4.6 s`.
- `frame-speed tier 2 (High) for this run` at startup names the tier selected.
- `recording to <dir>` is printed when `r` is pressed and `recording stopped: N images in <dir>`
  when it ends (duration reached, `c`, or quit).
- `recording: 5 images/s for 30 s into <data_dir>` at startup echoes the options in use.
- `GigE packet size: N bytes` is printed when `--mtu` is given (or `$GIGE_MTU` is set).
- `no frame within 1 s (check --mtu, the firewall and rp_filter)` is printed for each time-out;
  the viewer keeps waiting.

## If it fails

| Output | Cause and fix |
|---|---|
| `error: unrecognized subcommand 'view'` | The command was built without the feature; install OpenCV and libclang, then `cargo install --path . --locked --features view`. |
| Build: `couldn't find any valid shared libraries matching: ['libclang.so', 'libclang-*.so']` | libclang development files missing: install `clang libclang-dev`, or export `LIBCLANG_PATH=/usr/lib/llvm-18/lib`. |
| Build: `Failed to find installed OpenCV package using probes: environment, pkg_config, cmake, vcpkg_cmake, vcpkg` | OpenCV development files missing: install `libopencv-dev`. |
| `error: OpenCV: ...` when the window opens (wording depends on the OpenCV build) | No display: run in a desktop session or set `DISPLAY`; in Docker share the X socket and run `xhost +local:`. |
| `error: 0 cameras found; pass a name or serial (see gcam-mvsdk-rs list)` | No camera seen; see camera-discovery. With several cameras the message shows their count; pass a name or serial. |
| `error: DeviceNotFound { query: "sb00", available: [...] }` | Wrong name; check camera-list. |
| Status `-13`, `32774` or `-45` | Another program holds the camera; stop it. |
| Repeated `no frame within 1 s` | Add `--mtu 1500`; then check firewall and `rp_filter` (`gcam-mvsdk-rs doctor`). |
| A steady 60 to 63 fps on a 90 fps camera | A slower tier was forced with `--frame-speed`; drop the option or pass the fastest index from `info`. |
| `error: <file>: unknown field ...` or `rate_hz must be a positive number` | Fix the config file; only `duration_s`, `rate_hz` and `data_dir` are accepted. |
| `error: creating <dir>: Permission denied` | `data_dir` is not writable by this user; set another `data_dir` in the config or `--data-dir`. |

## Safety

The viewer sets the camera to Mono8 output, continuous trigger, the fastest frame-speed tier,
automatic exposure off and the given exposure for the session, and does not save them. Stop any
production publisher on that camera first; it restores its own settings when it restarts.
Recordings only add files under `data_dir`; nothing there is deleted or overwritten (every
recording gets a new folder).
