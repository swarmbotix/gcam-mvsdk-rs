# Implementation notes

Decisions taken where `PLAN_RUST_WRAPPER.md` is silent, and the places where the implementation
differs from it. Facts about the SDK were checked against SDK 2.1.0.49 for Linux x64.

## Deviations from the plan

1. **`CameraSetGigePacketSize` does not exist.** Neither the 2.1.0.49 headers nor the exported
   symbols of `libMVSDK.so` contain it. The packet size is selected with `CameraSetTransPackLen`,
   which takes an index into the `pPackLenDesc` table of `tSdkCameraCapbility`.
   `Camera::set_gige_packet_size(max_bytes)` picks the largest offered size that does not exceed
   `max_bytes` (normally the NIC MTU), as `gige_ht_camera.cpp` does, and returns the chosen size.
   The required function table therefore has 25 entries with `CameraSetTransPackLen` in place of
   `CameraSetGigePacketSize`.
2. **`tSdkCameraCapbility` is declared field by field** instead of "read fields plus opaque padding".
   The full declaration has the correct layout on both 32-bit and 64-bit targets without
   per-target padding constants. Tests assert 344 bytes (64-bit) and 256 bytes (32-bit) plus the
   offset of every block.
3. **Extra module `src/doctor.rs`.** The host checks (privileges, `rp_filter`, firewall, NIC
   subnets) live in the library so that other programs, such as the planned `cam-gige-pub`, can
   run them on failure. The CLI adds the SDK and camera checks.
4. **`sdk-manifest.toml` carries no comments.** It is written exactly as specified for the shared
   file, without the inline comments shown in section 3.5 of the plan. A test asserts the exact
   bytes so that it stays identical to the copy in `gcam-mvsdk-py`.
5. **Callback mode is not implemented** (the plan schedules it for a second iteration). The raw
   `CameraSetCallbackFunction` pointer and the `CameraSnapProc` type are available in `ffi`.

## Loading

- **Calling convention:** every function pointer and the callback type use `extern "system"`
  (C on all targets except 32-bit Windows, where it is `stdcall`).
- **Required and optional symbols.** The 25 functions of the plan's list are required;
  `Api::load` fails with `LoadError::MissingSymbol` if one is absent. About 30 further functions
  (version string, data directory, getters used by `info`, frame speed, software trigger,
  statistics, callbacks, aligned allocation) are `Option`s, `None` when a build of the SDK does
  not export them. `Api::symbol::<T>(name)` resolves anything else.
- **One load per process.** `Sdk` is cached in a process-wide static. The first successful
  `load`, `load_with` or `load_from` wins and calls `CameraSdkInit` once, as the vendor requires;
  later calls return the same instance. `load_from` with a different path returns
  `LoadError::AlreadyLoaded`. The library is never unloaded.
- **`CameraSdkInit(1)`.** The header comment says 0 selects English and 1 Chinese, but every vendor
  demo and the production C++ publisher pass 1. On Linux 2.1.0.49 both values produce English
  error strings (checked with a small C program), so 1 is kept as the plan specifies.
- **`GCAM_MVSDK_DIR` is exclusive.** When it is set, only `$DIR/lib/<arch>`, `$DIR/lib` and `$DIR`
  are tried (the variable may also name the library file). A wrong value produces an error
  listing those paths instead of silently falling back to another copy.
- **SDK home.** `$GCAM_MVSDK_HOME` replaces `~/.gcam-mvsdk` as the per-user root, for the loader,
  for the recorded Windows library, for the default `setup --dest`, and for the CLI's default
  `--data-dir` (`<home>/data`). The rule matches the Python package's `sdk_home()`: the value is
  trimmed, an empty value counts as unset, and a leading `~` is expanded; relative paths are kept
  relative. (The Python package additionally reads a marker that its installer writes into its
  virtual environment; there is no Rust equivalent.) `doctor` prints
  `SDK home: <path> (from $GCAM_MVSDK_HOME)` or `(default)`. `SearchConfig::from_lookup` takes the
  environment as a function so tests never modify process variables.
- **Version directories.** Under `./vendor/` and the SDK home, every subdirectory whose name
  starts with a digit is treated as a version and tried newest first (numeric comparison), so an
  SDK version not yet in the manifest still loads.
- **Recorded library.** The Windows DLL location is stored under the key `library` in
  `<SDK home>/manifest.json` and tried before the system loader. The key is read on every OS.
- **Architecture mapping.** `x86_64` to `x64`, `x86` to `x86`, `aarch64` to `arm64`, 32-bit ARM
  with `target_abi = "eabihf"` to `arm`, other 32-bit ARM to `arm_softfp`. `GCAM_MVSDK_ARCH`
  overrides it.

## Safe layer

- **Text decoding.** Fixed `char` arrays are cut at the first NUL; pure ASCII is used as is,
  anything else is decoded as GBK with `encoding_rs`. `set_user_name` encodes the same way
  (GBK) and rejects names longer than 31 bytes or containing `\ / : * ? " < > |` or control
  characters before calling the SDK.
- **Enumeration** requests up to 64 devices. `CAMERA_STATUS_NO_DEVICE_FOUND` is returned as an
  empty list, not an error.
- **`DeviceInfo`** exposes the decoded fields as public `String`s. `gige_ip()` returns typed
  `Ipv4Addr` values (an empty string from the SDK becomes `0.0.0.0`); `gige_mac()` returns the
  MAC strings as the SDK formats them. `is_gige()` checks whether the port type contains `gige`
  or starts with `net` (case-insensitive); this heuristic follows the vendor's Python tools and
  has not yet been checked against a live camera's port-type string.
- **`Sdk::find_device(query)`** matches the serial number, then a unique enumerated name, then
  opens each camera to compare its user-set name (as `camera_info.py` does). The C++ publisher
  matches `acFriendlyName` from enumeration; both paths are covered.
- **`Camera`** holds the ISP output format in a `Cell`, which makes it `!Sync` while keeping it
  `Send`; a compile-time test asserts both. The format is queried with `CameraGetIspOutFormat`
  at open time and updated by `set_output_format` and `load_parameter`.
- **`Frame`** exposes the raw buffer (`as_slice`, length `uBytes`) and runs the vendor ISP with
  `process_into(&mut [u8])`, which first checks the buffer against
  `max(width, sw_zoom_width) * max(height, sw_zoom_height) * bytes_per_pixel(output format)`.
  `process()` allocates. `grab` returns `Err` on time-out; `Error::is_timeout()` identifies it.
- **Errors.** `MvError { status, function, message }` takes its message from
  `CameraGetErrorString`, falling back to the crate's own English description in
  `ffi::STATUS_TABLE` (codes 0 to -57 and the GigE Vision AIA codes). All other failures are
  variants of `gcam_mvsdk_rs::Error`.

## Setup (downloader)

- **HTTP client:** `ureq` 3 without TLS, since the vendor serves plain HTTP only. Timeouts: 20 s
  connect, 60 s to the response head, one hour for the whole body.
- **Download location:** `<dest>/downloads/<file name from the URL>`, written as `*.part` and
  renamed only after size and SHA-256 match. A verified archive already present there is reused.
  After a successful Linux installation the archive is deleted unless `--keep-archive` is given.
- **Installation is staged:** extraction goes to `<dest>/.<version>.staging-<pid>/`, then
  `manifest.json` is written and the directory is renamed to `<dest>/<version>/`. If that
  directory already exists, the command does nothing when its `manifest.json` records the same
  release, and otherwise refuses unless `--force` is given.
- **Extraction** keeps `include/`, `lib/`, `tools/GeConfigCmd/` and the two top-level udev rule
  files (placed under `udev/`; they are not installed into `/etc` automatically, the CLI prints
  the command). The vendor's top-level directory is stripped. Entries with `..`, absolute paths
  or drive prefixes abort the extraction; symbolic links are skipped. The vendor archive stores
  files as mode 0666 without execute bits, so modes are set explicitly: 0755 under `lib/` and
  `tools/`, 0644 elsewhere.
- **`manifest.json`** holds the plan's four fields (`version`, `source_url`, `sha256`,
  `installed_at`, RFC 3339 UTC) plus optional `os`, `source_file` (for `--from`) and `installer`.
- **`--from FILE`** identifies the release by SHA-256. With `--version`, the named release is used
  but its size and checksum must still match.
- **Releases without a recorded SHA-256** (currently the Windows entry) are installed only with
  `--accept-unverified`; without it the command prints the computed hash and stops.
- **Windows branch:** the `.rar` is kept in `<dest>/downloads/`; `7z`, `7z.exe`, `7za` or `7zz` on
  `PATH` (or `Program Files\7-Zip\7z.exe`) extracts it to `<dest>/<version>/installer/`; the first
  `.exe` found is reported. Instructions to run the installer as Administrator are printed. An
  installed DLL is searched for under `Program Files` in directories whose names contain
  `huateng`, `mindvision`, `mvsdk` or `camera` (depth 5) and recorded. The command exits with
  status 2 while the installer still has to be run. This branch has not been run on Windows.

## CLI

- **SDK data directory.** On `CameraInit` the SDK writes `Camera/Data/<serial>.mvdat` into the
  working directory (seen in `camconfig/Camera/Data/`). Commands that open cameras (`list`
  without `--no-open`, `rename`, `info`, `grab`) call `CameraSetDataDirectory` with
  `<SDK home>/data`, or the global `--data-dir`. The library leaves the SDK default unless
  `Sdk::set_data_directory` is called.
- **`list`** opens each camera briefly for the user-set name, as `list_cameras.py` does;
  `--no-open` skips this. It flags GigE cameras outside their host NIC's subnet. With no
  cameras and no privileges it says so.
- **`rename`** selects the camera by serial number only, as in the plan and `nameit.py`.
- **`setip`** always sets a persistent static address (`bPersistent = TRUE`), validates the mask
  and address, and warns when the new address is outside the subnet of the NIC that found the
  camera.
- **`grab`** selects Mono8 for mono sensors and RGB8 for colour sensors, continuous trigger, then
  writes `<user name or serial>_NNNN.png`. `--mtu`, or `$GIGE_MTU` as in the C++ publisher,
  selects the packet size. Three consecutive time-outs abort with a hint.
- **`doctor`** reads `CapEff` from `/proc/self/status` (`CAP_NET_RAW` is bit 13), takes the
  effective `rp_filter` per interface as the maximum of `all` and the interface value, detects
  `ufw` (from `/etc/ufw/ufw.conf`) and `firewalld` (running process), lists IPv4 interfaces with
  `getifaddrs` and their MTU from `/sys/class/net`. It exits with status 1 if any check fails.

## Packaging

- **Features:** `cli` (default; enables `setup`, `clap`, `png`) and `setup` (`ureq`, `flate2`,
  `tar`, `sha2`, `toml`, `humantime`). The binary has `required-features = ["cli"]`.
  `serde` and `serde_json` are unconditional because the loader reads the recorded library path.
  `libc` is a Unix-only dependency (used by `doctor`).
- **`rust-version = "1.88"`**, the minimum required by `encoding_rs` 0.8.42 in the lock file.
  Edition 2021.
- **Licence files:** `LICENSE` (dual-licence notice), `LICENSE-MIT`, `LICENSE-APACHE` (official
  text from apache.org).
- **CI:** `.github/workflows/ci.yml` is deliberately minimal, matching the Python repository:
  `scripts/check-no-vendor-files.sh` (same pattern as the Python copy) plus build, test and
  clippy on `ubuntu-latest`. The plan's cross-compilation matrix (section 5) and
  `cargo publish --dry-run` are not in CI; `.claude/skills/release/release-check.sh` runs the
  dry run locally. The workflow has not been run on GitHub yet.
- **Executable bit:** this checkout is on a file system where `core.fileMode` is `false`, so the
  executable bit must be recorded explicitly when the scripts are first added:
  `git add --chmod=+x install.bash scripts/*.sh .claude/skills/*/*.sh`. CI calls the check
  script through `sh`, so it does not depend on the bit.
- **`Cargo.lock` is committed** (it is not ignored), so `cargo install --locked` and
  `cargo build --locked` reproduce the tested dependency versions.

## Capabilities document and further controls

- `docs/CAPABILITIES.md` follows the ten-section outline shared with `gcam-mvsdk-py`. Each item
  states what it does, a use case, the vendor functions, ranges and defaults from the header
  comments (or "model dependent"), the Rust API, and a snippet. Status counts: 32 wrapped, 1 partly
  wrapped, 44 raw, plus the vendor GUI table.
- Every snippet lives in `examples/capabilities.rs` between `// [doc:name]` and `// [/doc]`
  markers. CI builds the example, `tests/capabilities_doc.rs` requires the document's Rust blocks
  and the marked regions to be identical in both directions, and `tests/symbols.rs` resolves every
  quoted vendor function name in a local SDK (it skips in CI, where no SDK is installed). The
  example runs snippets by name; those that change settings need `--write`.
- New safe methods live in `src/controls.rs` (a second `impl Camera` using only the public
  `handle()` and `sdk()` plus the crate-private `get_i32`). 43 functions were added to the optional
  part of `ffi::Api`, with `tSdkFrameEvent` (32 bytes, asserted) and the frame-event and
  connection-status callback types.
- `Resolution` gained a public `raw` field holding the SDK record, so `set_image_resolution` passes
  a preset back exactly as the camera described it. This changes the struct's public fields
  (acceptable before 1.0).
- I/O uses `CameraSetIOStateEx` / `CameraGetIOStateEx` / `CameraGetOutPutIOStateEx` (1 high, 0 low
  on every model); the header marks the functions without `Ex` obsolete because they invert the
  level on GE and SUA models.
- `set_roi` calls `CameraSetImageResolutionEx` with index 0xFF. The header does not say whether
  width and height are counted before or after binning; the method documents this and the snippet
  reads the resolution back.
- `CameraWriteSN` is documented as dangerous and neither wrapped nor shown. `CameraSetParameterMask`
  is shown with all bits set because the header names the type a mask without documenting the bit
  layout. Callbacks stay raw until the planned safe callback wrapper.
- Not verified on hardware: every new method and snippet. Only signatures (against the header) and
  symbol presence (against `libMVSDK.so` 2.1.0.49) were checked; `tests/hw` has a read-only smoke
  test for the new getters.

## OpenCV live view example

- `examples/live_view.rs` uses `opencv` 0.101 (`default-features = false`, `highgui` and
  `imgproc`), an optional dependency enabled by the feature `view`. The example is declared with
  `required-features = ["view"]`, so `cargo build --all-targets`, `cargo test`, clippy and
  `cargo publish --dry-run` skip it and never need OpenCV or libclang.
- The safe API exposes the raw SDK buffer read-only (`Frame::as_slice`), but the FPS overlay needs
  a writable image. The example therefore runs the vendor ISP (`Frame::process_into`) into one
  reusable buffer, which `Mat::new_rows_cols_with_data_mut` wraps as `CV_8UC1` without copying.
  This matches the production C++ publisher (`CameraImageProcess`, then a `cv::Mat` over the
  output buffer). The raw buffer is returned to the SDK before drawing.
- FPS is the number of frames displayed in the last second. A time-out prints one line and the
  loop continues; `q`, Esc, or a closed window (`WND_PROP_VISIBLE < 1`) ends it.
- The CI job `example-view` (Ubuntu, `libopencv-dev clang libclang-dev` from apt) runs clippy on
  the example with `-D warnings`; it is the only build verification of the example.
- OpenCV cannot be installed through Cargo. The `opencv` crate is a binding to a system OpenCV
  (found with pkg-config, CMake or vcpkg) and generates its bindings with libclang at build time;
  no crate ships an OpenCV build that Cargo could install. The documented prerequisite is
  therefore `sudo apt-get install -y libopencv-dev clang libclang-dev`. A pure-Rust window crate
  was considered as a Cargo-only alternative and not adopted, because the owner chose to keep
  OpenCV. `opencv` is at 0.101.1, the newest release on crates.io as of 2026-10-12.

## Installer and developer tooling

- **`install.bash`** (bash 4+, `set -euo pipefail`) runs: platform and architecture detection
  (`/etc/os-release`, `uname -m`; macOS and Windows refused); a single up-front list of
  privileged commands with one confirmation and one `sudo -v` prompt, kept alive by a background
  `sudo -n -v` loop during the build; Rust via rustup when `cargo` is missing (and
  `rustup update stable` when `rustc` is older than 1.88); build tools (`build-essential`,
  `pkg-config`, and `libcap2-bin` when `setcap` is missing, via `apt-get` on Debian-like
  systems); `cargo install --path <repo> --locked`; `gcam-mvsdk-rs setup --dest
  ${GCAM_MVSDK_HOME:-~/.gcam-mvsdk} [--from FILE]`; `setcap` on the installed binary (skipped
  when `getcap` shows it is already set); optional udev rules (`--udev`); then `doctor`, `list`
  and a summary. Extra flag `-y/--yes` answers every question; without a terminal and without
  `--yes`, no privileged command and no deletion is performed and the commands are printed.
- **`GCAM_MVSDK_HOME` in the installer.** When set, the installer trims it, expands `~`, makes a
  relative value absolute (as the Python installer does), exports it so the final `doctor` and
  `list` find the SDK, and prints `export GCAM_MVSDK_HOME=...` for the user's shell profile. It no
  longer sets `GCAM_MVSDK_DIR`. The final `list` passes `--data-dir <SDK destination>/data`, so
  `--dev` keeps those files under `./vendor/`.
- **`--dev`** builds in the checkout, installs the SDK into `./vendor/`, skips `setcap` (it would
  be lost on the next build) and runs the checks from the repository directory.
- **`--uninstall`** runs `cargo uninstall gcam-mvsdk-rs` and removes the SDK directory only after
  confirmation (or with `--yes`). Installed udev rules are left in place and mentioned.
- **On a download failure** the installer prints the recorded URL, size and SHA-256 from
  `sdk-manifest.toml` and the `--from` command, and exits with status 1.
- **`.claude/skills/`**: `sdk-setup` (with `check-sdk.sh`, which verifies `manifest.json` and the
  25 required symbols), `camera-discovery` (with `gvcp_probe.py`, an unprivileged GigE Vision
  discovery broadcast that shows whether any camera answers at all), `add-sdk-function` (with
  `sizeof.sh`, which measures struct sizes and offsets from the vendor headers in a temporary
  directory), `release` (with `release-check.sh`), and `hardware-test` (with `run-hw.sh`, which
  builds the test binary as the user and runs only that binary with `sudo -E` when
  `CAP_NET_RAW` is missing, so `target/` never becomes root-owned).
- **Error convention.** The coordinator's brief describes `Result<T, MvError>`; the crate uses
  `gcam_mvsdk_rs::Result<T>` = `Result<T, Error>`, where every SDK status failure is
  `Error::Sdk(MvError)`. A single error type is needed because loading, I/O, argument validation
  and setup also fail. `CLAUDE.md` documents the actual convention.
