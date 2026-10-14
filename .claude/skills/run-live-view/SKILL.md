---
name: run-live-view
description: Show a live image from one HuaTeng camera in an OpenCV window with the live_view example of gcam-mvsdk-rs (feature view), and record PNG images from it (r starts, c stops; rate and duration in examples/record.yaml; output under ~/.gcam-mvsdk/data). Use when the user wants to see the camera image live, check focus or framing interactively, confirm streaming works with a display attached, or save a short image sequence from the live view.
---

# run-live-view

## When to use

- The user wants to watch the camera live (focus, framing, lighting) on a machine with a display.
- Interactive check that streaming works end to end.
- Record a short image sequence (default 5 images per second for 30 s) while watching.

For still images on a headless machine use **camera-grab** instead.

## Prerequisites

- OpenCV 4 and libclang on the build machine (the `opencv` crate binds the system OpenCV). On
  Ubuntu:
  ```bash
  sudo apt-get install -y libopencv-dev clang libclang-dev
  ```
  Other distributions: https://github.com/twistedfall/opencv-rust#getting-opencv
- A running display: a desktop session with `DISPLAY` set, or in Docker the X socket shared into
  the container and `xhost +local:` run on the host.
- The repository checkout (the example is not part of the installed command).
- camera-list shows the camera; no other program streams from it.

## Steps

```bash
cd /path/to/gcam-mvsdk-rs
cargo build --release --features view --example live_view
./target/release/examples/live_view sb00
```

Or in one step:

```bash
cargo run --release --features view --example live_view -- sb00
```

Arguments: camera name or serial (may be omitted when exactly one camera is present),
`--exposure-us US` (default 5000, automatic exposure off), `--mtu BYTES` (packet size, as for
`grab`), `--config FILE` (recording options, default `examples/record.yaml`).

Keys in the window: `r` (or `R`) starts a recording, `c` stops it before the duration is over,
`q` or Esc quits (a running recording is finished first); closing the window also quits.

## Recording

`examples/record.yaml` holds the options; edit it or pass another file with `--config`:

```yaml
duration_s: 30   # a recording stops after this many seconds
rate_hz: 5       # images saved per second; the camera keeps running at its own frame rate
# data_dir: ~/.gcam-mvsdk/data   # default: <sdk home>/data
```

Each recording creates `<data_dir>/<YYYYMMDD_HHMMSS>_<camera>/` (local time at the key press;
`<camera>` is the user-set name, else the serial) and writes `0.png`, `1.png`, ... there: the
Mono8 camera image without the overlay. One image is saved per `1/rate_hz` seconds, anchored to
the start, so a 90 fps camera at `rate_hz: 5` yields about 150 files in 30 s. Unknown keys or a
non-positive number in the file stop the program at startup. A missing file is reported and the
defaults above are used.

Unit tests for the keys, the config file and the sampling (no camera or display, OpenCV needed to
build): `cargo test --features view --example live_view`.

## Reading the output

- The window title is `gcam-mvsdk-rs live view`; the image is the camera's Mono8 output, scaled
  to the window.
- The overlay in the top-left corner shows the number of frames displayed in the last second
  (`NN fps`). While recording, a second line reads `REC 23 saved  4.6 s`.
- `recording to <dir>` is printed when `r` is pressed and `recording stopped: N images in <dir>`
  when it ends (duration reached, `c`, or quit).
- `recording: 5 images/s for 30 s into <data_dir>` at startup echoes the config file.
- `GigE packet size: N bytes` is printed when `--mtu` is given.
- `no frame within 1 s (check --mtu, the firewall and rp_filter)` is printed for each time-out;
  the viewer keeps waiting.

## If it fails

| Output | Cause and fix |
|---|---|
| Build: `couldn't find any valid shared libraries matching: ['libclang.so', 'libclang-*.so']` | libclang development files missing: install `clang libclang-dev`. |
| Build: `Failed to find installed OpenCV package using probes: environment, pkg_config, cmake, vcpkg_cmake, vcpkg` | OpenCV development files missing: install `libopencv-dev`. |
| OpenCV error when the window opens (wording depends on the OpenCV build) | No display: run in a desktop session or set `DISPLAY`; in Docker share the X socket and run `xhost +local:`. |
| `0 cameras found; pass a name or serial (see gcam-mvsdk-rs list)` (exit 2) | No camera seen; see camera-discovery. With several cameras the message shows their count; pass a name or serial. |
| `Error: DeviceNotFound { query: "sb00", available: [...] }` | Wrong name; check camera-list. |
| Status `-13`, `32774` or `-45` | Another program holds the camera; stop it. |
| Repeated `no frame within 1 s` | Add `--mtu 1500`; then check firewall and `rp_filter` (`gcam-mvsdk-rs doctor`). |
| `Error: examples/record.yaml: unknown field ...` or `rate_hz must be a positive number` | Fix the config file; only `duration_s`, `rate_hz` and `data_dir` are accepted. |
| `Error: creating <dir>: Permission denied` | `data_dir` (default `~/.gcam-mvsdk/data`) is not writable by this user; set another `data_dir`. |

## Safety

The viewer sets the camera to Mono8 output, continuous trigger, automatic exposure off and the
given exposure for the session, and does not save them. Stop any production publisher on that
camera first; it restores its own settings when it restarts. Recordings only add files under
`data_dir`; nothing there is deleted or overwritten (every recording gets a new folder).
