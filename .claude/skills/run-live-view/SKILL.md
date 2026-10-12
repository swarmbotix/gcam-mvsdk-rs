---
name: run-live-view
description: Show a live image from one HuaTeng camera in an OpenCV window with the live_view example of gcam-mvsdk-rs (feature view). Use when the user wants to see the camera image live, check focus or framing interactively, or confirm streaming works with a display attached.
---

# run-live-view

## When to use

- The user wants to watch the camera live (focus, framing, lighting) on a machine with a display.
- Interactive check that streaming works end to end.

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
sudo setcap cap_net_raw,cap_net_admin+ep target/release/examples/live_view   # GigE, after each build
./target/release/examples/live_view sb00
```

Or in one step (without the capability, run it with `sudo -E`):

```bash
cargo run --release --features view --example live_view -- sb00
sudo -E target/release/examples/live_view sb00
```

Arguments: camera name or serial (may be omitted when exactly one camera is present),
`--exposure-us US` (default 5000, automatic exposure off), `--mtu BYTES` (packet size, as for
`grab`). Quit with `q` or Esc, or close the window.

## Reading the output

- The window title is `gcam-mvsdk-rs live view`; the image is the camera's Mono8 output, scaled
  to the window.
- The overlay in the top-left corner shows the number of frames displayed in the last second
  (`NN fps`).
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

## Safety

The viewer sets the camera to Mono8 output, continuous trigger, automatic exposure off and the
given exposure for the session, and does not save them. Stop any production publisher on that
camera first; it restores its own settings when it restarts.
