# Plan: Rust wrapper for the HuaTeng MVSDK (`gcam-mvsdk-rs`)

Goal: a user types `cargo install gcam-mvsdk-rs`, then `gcam-mvsdk-rs setup`, then `gcam-mvsdk-rs list`, and sees every
HuaTeng GigE / USB camera on the LAN. A library consumer types `cargo add gcam-mvsdk-rs` and gets the same
functionality (enumerate, rename, set IP, set exposure, grab frames) from Rust. Everything this
repository does today in `camconfig/*.py` and `src/gige_ht_capture.cpp` should be reachable from the
Rust crate, on Linux and Windows, on every CPU architecture the vendor supports.

This document records the facts established while inspecting the SDK, the design decisions that follow
from them, and the work items. A companion document, `PLAN_PYTHON_WRAPPER.md` in the `gcam-mvsdk-py` repository, covers the Python
package that shares the same downloader and on-disk layout.

---

## 1. Facts about the vendor SDK

Source: `linux/linuxSDK_V2.1.0.49(202511141513)/` in this directory, the English
`LinuxSDK Document.pdf` (19 pages), the Chinese `readme-中文.pdf` (6 pages), `install.sh`,
`demo/python_demo/mvsdk.py`, and the real `lib/` tree found at
`~/Dropbox/Projects/huateng_gige_camera/linux/linuxSDK_V2.1.0.49(202511141513)/lib/`.

### 1.1 API surface

- Pure C API. `include/CameraApi.h` declares 304 `MVSDK_API` functions. Plain C structs, integers,
  fixed-size `char` arrays, and C function pointers for callbacks. No C++ classes cross the boundary.
- The same header serves Linux and Windows. On Windows the library is `MVCAMSDK.dll` (32-bit) or
  `MVCAMSDK_X64.dll` (64-bit); on Linux it is `libMVSDK.so`.
- `demo/python_demo/mvsdk.py` is a complete ctypes wrapper and demonstrates that runtime loading of the
  library with hand-declared signatures is sufficient. The Rust wrapper mirrors that approach.

### 1.2 Platforms shipped by the vendor

From the vendor download center (section 1.7), checked 2026-10-12:

| OS | Architectures | Package | Notes |
|---|---|---|---|
| Linux (glibc) | x86, x64, arm (hard float), arm_softfp, arm64 | `linuxSDK_V2.1.0.49(202511141513).tar.gz`, 57,978,497 bytes | `lib/<arch>/libMVSDK.so`, headers, demos, `GeConfigCmd` |
| Windows | x86, x64 | `HuaTengVision Camera Platform Setup(2.1.10.195_20260616).rar`, 128 MB; contains an `.exe` installer | Installs `SDK\` (DLLs: `MVCAMSDK.dll`, `MVCAMSDK_X64.dll`), `Drivers\` (USB and GigE filter driver), `DEMO\`, `Document\`, `Tools\` |
| macOS, Apple Silicon | arm64 | `Mac_sdk_m3(250120).zip`, 5.1 MB | Labelled "Mac OS_M3_V1.1" |
| macOS, Intel | x64 | `macsdk(240831).zip`, 5.7 MB | Labelled "Mac OS_Intel_V1.1" |
| Android | (not inspected) | `Android_SDK_Demo_20220820.rar`, 22.7 MB | V2.0, 2022 |
| GenTL producer, Windows | x64 | `GenTL Producer.rar`, 375 KB | `.cti` for Halcon, Aravis-style GenTL consumers |
| GenTL producer, Linux | x64 | `GenTL_linux.tar.gz`, 353 KB | `.cti` |
| V4L2 demo, Linux | | `mv_v4l2.tar.gz`, 33 MB | Exposes camera as a V4L2 device |

All Linux and Windows targets are Rust tier 1 or tier 2 targets. The macOS packages make a later
`aarch64-apple-darwin` and `x86_64-apple-darwin` port plausible with the same runtime-loading design, but
they have not been inspected and are out of scope for the first release.

Example sources also offered on the same page: Windows C++ demo (`Demo.rar`), `python_demo.zip`,
`java_demo.zip`. Documentation category (`download/2/`): `SDK_API_ENG.chm`, `LinuxSDK Document.pdf`,
`Industrial Camera Development Manual.pdf`, Windows platform user manual.

### 1.3 Runtime dependencies

`ldd lib/x64/libMVSDK.so`:

```
librt.so.1  libpthread.so.0  libdl.so.2  libm.so.6  libc.so.6
```

Pure glibc; `libstdc++` is folded in statically. Highest versioned symbol is `GLIBC_2.15`, so the
library runs on Ubuntu 12.04 and newer. No apt packages are required. `install.sh` only copies
files: two udev rules (USB cameras only), three headers to `/usr/include`, and one `.so` to `/lib`.

### 1.4 Binary sizes

| Arch | `libMVSDK.so` |
|---|---|
| x64 | 22.9 MB |
| arm64 | 17.3 MB |
| all five Linux | 74 MB total |

### 1.5 License

The only license statement is the 声明 (Statement) paragraph on page 1 of `readme-中文.pdf`:

> 任何单位和个人未经我公司授权不能使用、修改、再发布本软件的任何部分

"No unit or individual may use, modify, or redistribute any part of this software without our
company's authorization." The English PDF and the headers contain no license text at all.

### 1.6 GigE discovery and privileges

Discovery is GigE Vision GVCP: a UDP broadcast on the host NIC, answered by every camera on the same
L2 segment. No IP sweep is involved. The vendor document requires the firewall to be off (or UDP 3956
open), a static IP on the host NIC in the camera subnet, `net.ipv4.conf.all.rp_filter=0`, and root
(or `CAP_NET_RAW`) for the process. This is why `docker-compose.yml` uses `network_mode: host` and
`privileged: true`.

---

### 1.7 Where the SDK comes from

Vendor download center: `http://huatengvision.com/download/1/` (site 华腾威视, HuaTeng Vision). This is
where the copy under `~/Dropbox/Projects/huateng_gige_camera/` was obtained.

Mechanics, verified 2026-10-12 from this machine:

- The HTML page answers the first request with `302 Location: /download/1/` while setting `PHPSESSID` and
  `WebLang` cookies; browsers follow this transparently, `curl` needs `-L` with a cookie jar
  (`-c jar -b jar`). Without cookies the page loops. This affects only the HTML page, not the files.
- File links on the page are `javascript:down_link(<id>, '<path>')`. The files themselves are served
  directly at `http://huatengvision.com/<path>` with no cookie, no login, no referer check, plain HTTP,
  `GET` and `HEAD` both return `200` with `Content-Length`. HTTPS is not available (connection times out).
  Parentheses in file names must be percent-encoded (`%28`, `%29`).
- Direct URL of the Linux SDK:

  ```
  http://huatengvision.com/attachment/down/13/linuxSDK_V2.1.0.49%28202511141513%29.tar.gz
  ```

  Downloaded size 57,978,497 bytes, SHA-256
  `9624d193a908d94a9850400346cf1ab8095437a040e381b2fd39029b0183dd8a`, identical to the Dropbox copy.
- Direct URL of the Windows SDK:

  ```
  http://huatengvision.com/attachment/down/24/HuaTengVision%20Camera%20Platform%20Setup%282.1.10.195_20260616%29.rar
  ```

  128,081,184 bytes. A `.rar` wrapping an `.exe` installer; extraction needs `unrar` or `7z`.
- Other direct paths, all under `http://huatengvision.com/`: `attachment/down/28/Mac_sdk_m3(250120).zip`,
  `attachment/down/0/macsdk(240831).zip`, `attachment/down/0/Android_SDK_Demo_20220820.rar`,
  `attachment/down/37/GenTL Producer.rar`, `attachment/down/0/GenTL_linux.tar.gz`,
  `attachment/down/0/mv_v4l2.tar.gz`, `attachment/down/0/python_demo.zip`, `attachment/down/0/java_demo.zip`.
- The numeric segment (`down/13/`, `down/24/`) is an attachment id that changes when the vendor uploads
  a new version. The `setup` subcommand therefore carries a small table of (version, URL, size, SHA-256)
  rather than deriving URLs, and reports when the recorded URL stops answering `200`.

Consequences for `gcam-mvsdk-rs setup`:

1. Plain HTTP means the SHA-256 check is mandatory, not optional; a mismatch aborts.
2. Linux: download the tarball, verify, extract `include/`, `lib/`, `tools/GeConfigCmd/` into
   `~/.gcam-mvsdk/2.1.0.49/`. One command, no user interaction.
3. Windows: download the `.rar`, verify, extract the `.exe`, then either run the installer (requires
   administrator, installs the GigE filter driver and USB driver as well) or tell the user to run it.
   Afterwards locate `SDK\MVCAMSDK_X64.dll` under the installation directory (default path to be
   confirmed on a Windows machine) and record it in `~/.gcam-mvsdk/`. The installer is the only way to
   get the drivers, so on Windows `setup` cannot be fully silent.
4. `setup --from <file>` remains for air-gapped machines and for the Dropbox mirror.

Local mirror for the team: `~/Dropbox/Projects/huateng_gige_camera/` with `linux/`, `win/`, `mac/`,
`android/`, `Document/`. The Windows installer there is an older build, `2.1.10.193 (251108)`, 165 MB.

Support: `support@huatengvision.com` (English PDF); `mdwvision@163.com` appears in a mirrored copy of the
same PDF.

---

## 2. Design decisions

### 2.1 Never ship vendor binaries or headers in the crate

Two independent blockers:

1. crates.io caps a crate at 10 MB. The x64 library alone is 22.9 MB.
2. The vendor notice forbids redistribution without authorization.

Therefore the published crate contains Rust source only, under MIT or Apache-2.0. The vendor library
is obtained by the end user from HuaTeng, either manually or through the downloader described in 2.3.
Public GitHub plus crates.io is acceptable under this rule; a private repository would additionally
allow vendoring the blobs for internal use, but that is not the chosen path.

### 2.2 Load the SDK at runtime with `libloading`, do not link at build time

`cargo install` compiles and links immediately. If `build.rs` required `libMVSDK.so` and the headers,
installation would fail on a machine that has not yet downloaded the SDK, before any downloader could
run. Runtime loading removes that ordering problem:

- The `ffi` module declares `extern "C"` function signatures by hand for the subset of the API that is
  wrapped. No bindgen, no vendor header at build time. This is the same technique `mvsdk.py` uses
  with ctypes and the same technique used by Rust bindings for other proprietary SDKs.
- At runtime, `Sdk::load()` searches, in order: `$GCAM_MVSDK_DIR/lib/<arch>` and `$GCAM_MVSDK_DIR/lib`,
  `./vendor/<version>/lib/<arch>` (developer checkout), `~/.gcam-mvsdk/<version>/lib/<arch>`, then the
  system loader path (`/lib`, `/usr/lib`, or `PATH` on
  Windows). It opens the library with `libloading` and resolves symbols once.
- The crate therefore compiles on any machine with no SDK present and no network access during the
  build. If the library is missing at runtime, the error message says exactly where it looked and how
  to fix it.

Trade-off: a wrong hand-written signature is undefined behaviour at runtime instead of a compile
error. Mitigation: a hardware integration test (section 5) that runs against a real camera on the
bench machine.

### 2.3 A separate downloader, as a subcommand of the CLI

`gcam-mvsdk-rs setup` downloads the vendor tarball to `~/.gcam-mvsdk/<version>/`, verifies a SHA-256 recorded in
the crate, and extracts it. The user obtains the binary directly from HuaTeng; the crate redistributes
nothing.

Fallback when the vendor URL cannot be fetched unattended (login or form required, URL changed):
`gcam-mvsdk-rs setup --from ./linuxSDK_V2.1.0.49.tar.gz` installs from a tarball the user downloaded by hand.
The checksum table makes a wrong SDK version fail early.

On Windows the same subcommand extracts `MVCAMSDK_X64.dll`, the import library, and headers from the
vendor zip or installer payload. The GigE filter driver still has to be installed once by the vendor
installer; the CLI prints that instruction.

The `~/.gcam-mvsdk` layout is shared with the Python package so that one download serves both ecosystems.

### 2.4 Hand-written signatures are not a copy of the header

The `extern "C"` declarations describe a public interface in Rust syntax, the same way `mvsdk.py`
describes it in ctypes. They do not reproduce the header text or its comments. Neither the headers
nor a pregenerated `bindings.rs` produced from them is committed to the public repository.

---

### 2.5 Naming convention

One name per language, used for every artifact of that language without variation:

| Artifact | Rust | Python |
|---|---|---|
| GitHub repository | `gcam-mvsdk-rs` | `gcam-mvsdk-py` |
| Registry package | crates.io `gcam-mvsdk-rs` | PyPI `gcam-mvsdk-py` |
| Install | `cargo install gcam-mvsdk-rs` | `pip install gcam-mvsdk-py` |
| CLI binary | `gcam-mvsdk-rs <verb>` | `gcam-mvsdk-py <verb>` |
| Library use | `gcam_mvsdk_rs::Sdk::load()` | `import gcam_mvsdk_py` |

The only forced deviation is inside code, where the hyphen becomes an underscore: Cargo does this
automatically for `use` paths, and Python requires `gcam_mvsdk_py` as the import directory.

Language-neutral pieces that both packages read share one name: the SDK directory
`~/.gcam-mvsdk/<version>/lib/<arch>/` and the environment variables `GCAM_MVSDK_DIR`,
`GCAM_MVSDK_ARCH`, `GCAM_MVSDK_HW_TEST`.

There is no separate `-sys` crate. Nothing links at build time (section 2.2), so the raw FFI lives in
the module `gcam_mvsdk_rs::ffi` inside the single crate. Library and CLI binary ship in that same crate;
Cargo names the binary after the package, so `cargo install gcam-mvsdk-rs` yields the command
`gcam-mvsdk-rs`.

Availability verified on 2026-10-12: `gcam-mvsdk-rs` and `gcam-mvsdk-py` are unregistered on crates.io
and PyPI, and no GitHub repository name contains `gcam-mvsdk`.

---

## 3. Crate layout

```
gcam-mvsdk-rs/                    public GitHub, MIT OR Apache-2.0; one crate, library + binary
├── Cargo.toml                    [package] name = "gcam-mvsdk-rs"
├── sdk-manifest.toml             known vendor releases: version, os, url, size, sha256 (section 3.5)
├── scripts/fetch-sdk.sh          cargo run -- setup --dest vendor/   (developer bootstrap)
├── .gitignore                    vendor/, archives, *.so, *.dll, *.dylib, vendor headers
├── vendor/                       NOT committed; populated by fetch-sdk.sh
├── src/
│   ├── lib.rs                    pub mod ffi; pub use sdk::*, camera::*, error::*
│   ├── ffi.rs                    hand-written extern "C" signatures, structs, status codes,
│   │                             Api::load() via libloading; no build.rs
│   ├── sdk.rs                    Sdk: load(), search path, enumerate(), gige_set_ip()
│   ├── camera.rs                 Camera, Frame (RAII, Drop releases SDK handles and buffers)
│   ├── error.rs                  MvError(status) with CameraGetErrorString text
│   ├── setup.rs                  downloader: fetch, sha256, extract, --from
│   └── main.rs                   CLI binary `gcam-mvsdk-rs`: setup | list | rename | setip | info | grab | doctor
└── tests/                        struct sizes, status table, search path; hw/ gated by GCAM_MVSDK_HW_TEST

cam-gige-pub/                     iceoryx2 publisher in Rust, replaces src/gige_ht_capture.cpp
                                  (lives in this workspace, depends on gcam-mvsdk-rs; not published;
                                  pins iceoryx2 = 0.9.3)
```

### 3.1 `gcam_mvsdk_rs::ffi` (raw FFI module)

- `struct tSdkCameraDevInfo`, `tSdkFrameHead`, `tSdkCameraCapbility` (only the fields the safe
  layer reads; the rest as opaque padding of the correct size, with `size_of` asserted in a test).
- Status codes from `CameraStatus.h` as an `i32` constant table.
- `struct Api { CameraSdkInit: Symbol<...>, CameraEnumerateDevice: Symbol<...>, ... }` populated by
  `Api::load(path) -> Result<Api, LoadError>`.
- Calling convention: `extern "C"` on all targets except 32-bit Windows, where the DLL uses `stdcall`
  (`extern "system"` selects the right one automatically).

### 3.2 `gcam-mvsdk-rs` (safe layer)

- `Sdk::load() -> Result<Sdk>`: runs the search path, calls `CameraSdkInit(1)` once.
- `sdk.enumerate() -> Result<Vec<DeviceInfo>>`: wraps `CameraEnumerateDevice`. `DeviceInfo` exposes
  `friendly_name`, `product_name`, `serial`, `port_type`, and, for GigE, `gige_ip()`, `gige_mac()`
  via `CameraGigeGetIp` / `CameraGigeGetMac`. `char` arrays decoded as ASCII, falling back to GBK
  through `encoding_rs`; never assumed to be UTF-8.
- `sdk.gige_set_ip(&dev, ip, mask, gw, persistent)`: wraps `CameraGigeSetIp`.
- `Camera::open(&sdk, &dev) -> Result<Camera>`: `CameraInit`; `Drop` calls `CameraUnInit`.
- `camera.user_name()`, `camera.set_user_name(&str)`: `CameraGetFriendlyName` /
  `CameraSetFriendlyName` (the "user-set name" written by `nameit.py`, persisted in camera NVM).
- `camera.set_exposure_us(f64)`, `camera.set_frame_rate(..)`, `camera.set_gige_packet_size(..)`,
  `camera.set_output_format(Mono8 | ..)`, `camera.play()`, `camera.pause()`.
- `camera.grab(timeout) -> Result<Frame<'_>>`: `CameraGetImageBuffer`; `Frame` borrows the camera,
  exposes `width`, `height`, `timestamp`, `as_slice()`, and releases the SDK buffer in `Drop` via
  `CameraReleaseImageBuffer`. Zero copy into an iceoryx2 sample is possible from the slice.
- Callback mode (`CameraSetCallbackFunction`): an `extern "C"` trampoline that forwards to a boxed
  `FnMut` stored on the `Camera`. Second iteration; polling `grab()` covers the publisher today.
- `Camera: Send`, not `Sync`. The SDK is not documented as thread safe per handle.

Initial function subset (about 25), chosen to cover `list_cameras.py`, `nameit.py`,
`camera_info.py`, `live_stream.py`, and `gige_ht_capture.cpp`:

```
CameraSdkInit                 CameraEnumerateDevice          CameraInit
CameraUnInit                  CameraGetCapability            CameraGetFriendlyName
CameraSetFriendlyName         CameraGigeGetIp                CameraGigeSetIp
CameraGigeGetMac              CameraSetAeState               CameraSetExposureTime
CameraGetExposureTime         CameraSetFrameRate             CameraSetIspOutFormat
CameraSetTriggerMode          CameraPlay                     CameraPause
CameraGetImageBuffer          CameraReleaseImageBuffer       CameraImageProcess
CameraSetGigePacketSize       CameraGetErrorString           CameraSaveParameter
CameraLoadParameter
```

The raw `gcam_mvsdk_rs::ffi::Api` struct is public so that callers can reach any of the remaining functions
without waiting for a safe wrapper.

### 3.3 `gcam-mvsdk-rs` CLI

| Command | Replaces | Notes |
|---|---|---|
| `gcam-mvsdk-rs setup [--from FILE] [--version V]` | `install.sh`, Dockerfile `COPY` lines | Downloads or extracts SDK into `~/.gcam-mvsdk/<ver>/` |
| `gcam-mvsdk-rs list [--json]` | `camconfig/list_cameras.py`, `GeConfigCmd` listing | User name, model, SN, camera IP/MAC, host NIC IP/MAC |
| `gcam-mvsdk-rs rename <SN> <NAME>` | `camconfig/nameit.py` | Writes DeviceUserID to camera NVM |
| `gcam-mvsdk-rs setip <MAC> <IP> <MASK> <GW>` | `GeConfigCmd set ...` | Works even when camera is on the wrong subnet |
| `gcam-mvsdk-rs info <NAME or SN>` | `camconfig/camera_info.py` | Capability dump |
| `gcam-mvsdk-rs grab <NAME or SN> [-n N] [-o DIR]` | `demo/Sampli_Save_Demo` | Saves PNG frames, sanity check |
| `gcam-mvsdk-rs doctor` | troubleshooting section of README | Checks firewall, `rp_filter`, NIC subnet, `CAP_NET_RAW`, SDK presence |

`gcam-mvsdk-rs list --json` is the hook for a plug-and-play `runscript.bash`: enumerate first, then start
one publisher per discovered user name instead of the hardcoded `NUM_CAMS=4`.

### 3.4 `cam-gige-pub`

Rust port of `src/gige_ht_capture.cpp`. Uses `gcam-mvsdk-rs` for the camera and the iceoryx2 Rust API
(pinned `=0.9.3` to match `camera_ips/Cargo.toml` and the host installation) to publish on
`camera/<CAM_NAME>/image` with the same `ImageFrame` layout defined under `swarmbotix_io/`. Keeps
the environment variables `CAM_NAME`, `EXPOSURE_US`, `FRAME_RATE_HZ`, `GIGE_MTU`, `SHOW` so the
existing compose files and `runscript.bash` work unchanged.

---

### 3.5 How the repository manages the vendor SDK

The repository is public and never contains the SDK. Rules:

- `.gitignore` excludes `vendor/`, `*.tar.gz`, `*.rar`, `*.zip`, `*.so`, `*.dll`, `*.dylib`, and the three
  vendor headers. A CI check (`git ls-files | grep -E '\.(so|dll|dylib|tar\.gz|rar|zip)$'`) fails the
  build if a binary ever gets committed.
- The only vendor-derived content committed is `sdk-manifest.toml`, a table of known SDK releases:

  ```toml
  [[release]]
  version = "2.1.0.49"
  os      = "linux"
  url     = "http://huatengvision.com/attachment/down/13/linuxSDK_V2.1.0.49%28202511141513%29.tar.gz"
  size    = 57978497
  sha256  = "9624d193a908d94a9850400346cf1ab8095437a040e381b2fd39029b0183dd8a"
  layout  = "tar"        # top-level dir linuxSDK_V2.1.0.49(202511141513)/ with include/, lib/<arch>/, tools/

  [[release]]
  version = "2.1.10.195"
  os      = "windows"
  url     = "http://huatengvision.com/attachment/down/24/HuaTengVision%20Camera%20Platform%20Setup%282.1.10.195_20260616%29.rar"
  size    = 128081184
  sha256  = ""           # fill in after first download on a Windows machine
  layout  = "rar-installer"
  ```

  The same file is copied verbatim into `gcam-mvsdk-py` so both `setup` commands fetch identical bytes.
  Adding a new vendor release is a pull request that appends one entry.
- `src/setup.rs` implements the downloader once. It is reached two ways:
  - `gcam-mvsdk-rs setup` for end users, destination `~/.gcam-mvsdk/<version>/`.
  - `scripts/fetch-sdk.sh`, a two-line wrapper around `cargo run -- setup --dest vendor/`, for people
    working in the repository. Destination `vendor/<version>/`, which the loader searches when
    `GCAM_MVSDK_DIR` is unset and the current directory is the repository (see search order in 2.2).
- Steps performed by `setup`: HTTP `HEAD` to confirm `200` and the recorded `Content-Length`; `GET` with a
  progress bar; SHA-256 compare, abort on mismatch; extract `include/`, `lib/`, `tools/GeConfigCmd/`;
  write `manifest.json` (`version`, `source_url`, `sha256`, `installed_at`) next to them for `doctor`.
  `--from <file>` skips the network and runs the same verify-and-extract on a local archive, which is
  how the Dropbox mirror and air-gapped machines are served.
- Windows: `setup` downloads and verifies the `.rar`, extracts `Setup.exe` with `7z` if present, prints
  the path, and stops. The installer must be run by the user with administrator rights because it also
  installs the GigE filter driver and USB driver. On the next `setup` or `doctor` run, the DLL is located
  under the installation directory and recorded in `~/.gcam-mvsdk/manifest.json`.
- Build never touches `vendor/`. `cargo build`, `cargo test` (unit), `cargo clippy`, `cargo publish
  --dry-run` all succeed on a machine that has never downloaded the SDK. Only the hardware tests
  (`GCAM_MVSDK_HW_TEST=1`) and the CLI at runtime need it.

Repository bootstrap for a developer:

```bash
git clone https://github.com/swarmbotix/gcam-mvsdk-rs && cd gcam-mvsdk-rs
cargo build                       # works immediately, no SDK needed
./scripts/fetch-sdk.sh            # download + sha256 + extract into vendor/  (Linux: unattended)
cargo run -- list                 # loader finds vendor/<ver>/lib/x64/libMVSDK.so
GCAM_MVSDK_HW_TEST=1 cargo test   # hardware tests against the cameras on the bench
```

### 3.6 Building and using the crate from this machine

Because the crate contains no vendor bytes, building it locally and publishing it to crates.io are legally
identical; the choice is convenience. Until the API is stable, consumers on this machine and on the team's
machines use it without any registry.

Build the library and the CLI, install the CLI on `PATH`:

```bash
cd /mnt/A4CC6455CC6423B0/Github/vision_workspace/drivers/gcam-mvsdk-rs
cargo build --release                      # target/release/libgcam_mvsdk_rs.rlib + target/release/gcam-mvsdk-rs
cargo install --path .                     # copies the binary to ~/.cargo/bin/gcam-mvsdk-rs
gcam-mvsdk-rs setup                        # one-time per machine: SDK into ~/.gcam-mvsdk/
gcam-mvsdk-rs list
```

Consume the library from another project on this computer. Cargo compiles the dependency from source as
part of the consumer's build; there is no separate library install step in Rust.

```toml
# camera_ips/Cargo.toml, cam-gige-pub/Cargo.toml, any other local crate
[dependencies]
gcam-mvsdk-rs = { path = "../../drivers/gcam-mvsdk-rs" }   # from camera_publishers/cam_gige_ht; "../drivers/gcam-mvsdk-rs" from camera_ips
```

Consume it from another machine without a registry:

```toml
gcam-mvsdk-rs = { git = "https://github.com/swarmbotix/gcam-mvsdk-rs", tag = "v0.1.0" }
```

```bash
cargo install --git https://github.com/swarmbotix/gcam-mvsdk-rs --tag v0.1.0
```

Later, once published, the consumer's line becomes `gcam-mvsdk-rs = "0.1"` and the install becomes
`cargo install gcam-mvsdk-rs`. The crate source is identical in all three cases.

The checkout lives at `vision_workspace/drivers/gcam-mvsdk-rs` (remote
`https://github.com/swarmbotix/gcam-mvsdk-rs`), so the `path` dependency above resolves from
`camera_publishers/cam_gige_ht/` and, with one fewer `../`, from `camera_ips/`. A `[patch.crates-io]` entry in the workspace
root can later redirect the registry name to the local checkout for development without editing each
consumer.

Every consumer, whichever way it obtains the crate, still needs the SDK on its machine: run
`gcam-mvsdk-rs setup` once, or set `GCAM_MVSDK_DIR` to an existing copy. The wrapper does not carry it.

---

## 4. Privileges and host configuration

The crate cannot remove these; it can only detect and explain them (`gcam-mvsdk-rs doctor`).

- Linux GigE: `sudo setcap cap_net_raw,cap_net_admin+ep $(which gcam-mvsdk-rs)` once, or run as root.
  Firewall off or UDP 3956 allowed. Host NIC static IP in the camera subnet. `rp_filter=0`.
- Linux USB: udev rules `88-mvusb.rules`, `99-mvusb.rules` (vendor ID `f622`); `gcam-mvsdk-rs setup` can
  offer to install them with `sudo`.
- Windows: the vendor installer must be run once for the GigE filter driver. `gcam-mvsdk-rs setup` prints
  this and checks for the DLL.
- Discovery only reaches cameras on the same L2 segment as the host NIC. Cameras behind a router
  require `CameraGigeEnumerateDevice` with a unicast target, or a static IP set first.

---

## 5. Testing

- Unit: struct sizes against values recorded from the C header (`size_of::<tSdkCameraDevInfo>()`),
  status code table, search path resolution with a fake `~/.gcam-mvsdk`.
- Compile matrix (no hardware): `cargo check` for `x86_64-unknown-linux-gnu`,
  `aarch64-unknown-linux-gnu`, `armv7-unknown-linux-gnueabihf`, `i686-unknown-linux-gnu`,
  `x86_64-pc-windows-msvc`, `i686-pc-windows-msvc`. Because nothing links against the SDK, this is
  a plain cross-check.
- Hardware integration (bench machine with cameras, run manually or as a self-hosted CI job):
  `gcam-mvsdk-rs list` finds the four `sb0x` cameras; rename round trip on one camera; `grab` produces a
  non-empty Mono8 frame with the expected dimensions; `cam-gige-pub` publishes and `camera_ips`
  subscribes.
- Windows: compile only until a Windows machine with the vendor driver is available.

---

## 6. Work items and estimate

| # | Item | Estimate |
|---|---|---|
| 1 | `ffi` module: structs, status codes, `Api::load`, search path | 0.5 day |
| 2 | safe layer: `Sdk`, `DeviceInfo`, `Camera`, `Frame`, GigE IP/MAC/name | 0.5 day |
| 3 | CLI: `list`, `rename`, `setip`, `info`, `grab`, `doctor` | 0.5 day |
| 4 | `gcam-mvsdk-rs setup`: downloader, checksum table, `--from`, Windows extraction | 0.5 day |
| 5 | `cam-gige-pub`: iceoryx2 publisher, parity with the C++ publisher | 1 day |
| 6 | `runscript.bash`: discovery-driven launch using `gcam-mvsdk-rs list --json` | 0.25 day |
| 7 | `sdk-manifest.toml`, `.gitignore`, CI binary check, `scripts/fetch-sdk.sh` | 0.25 day |
| 8 | Switch `camera_ips` and `cam-gige-pub` to `path` dependencies, `cargo install --path .` | 0.25 day |
| 9 | Callback mode, broader API coverage | incremental |

Total for a usable release: about 3.5 days.

---

## 7. Open questions

1. Resolved 2026-10-12: the vendor serves direct, unauthenticated download URLs (section 1.7).
   `gcam-mvsdk-rs setup` is one command on Linux. Remaining risk is URL churn when the vendor uploads a
   new version, handled by the recorded (version, URL, SHA-256) table.
2. Is there a Windows SDK build for arm64? The download center offers one Windows installer; whether it
   contains arm64 DLLs is unknown until it is inspected on a Windows machine. macOS arm64 and Intel
   packages exist (section 1.2) and are a possible later port.
3. Should an email to support@huatengvision.com request redistribution permission? If granted, a
   `vendored` feature or a GitHub Release with the blobs becomes possible; the Rust code is identical
   either way, so this does not block starting.
4. Where does the Dockerfile currently obtain `lib/x64/libMVSDK.so`? The repository copy of the SDK
   folder has no `lib/` directory, so the `COPY` line depends on something outside the repo. The
   Dropbox mirror has the full tree. Unrelated to the wrapper, but worth fixing while the SDK path is
   being rationalized; the Dockerfile could run the same download-and-verify step as `setup`.

---

## 8. Local development without installing the SDK

```bash
export GCAM_MVSDK_DIR="$HOME/Dropbox/Projects/huateng_gige_camera/linux/linuxSDK_V2.1.0.49(202511141513)"
cargo run -- list
```

`Sdk::load()` finds `lib/x64/libMVSDK.so` under `GCAM_MVSDK_DIR` without `sudo ./install.sh`.

---

## 9. Status (2026-10-12)

Implemented in this repository (uncommitted; see `IMPLEMENTATION_NOTES.md` for every decision):

- Work item 1: `src/ffi.rs` with hand-written `#[repr(C)]` structs (sizes and offsets asserted
  against gcc measurements: `tSdkCameraDevInfo` 260, `tSdkFrameHead` 64, `tSdkCameraCapbility`
  344 on 64-bit / 256 on 32-bit), the status table, and `Api::load` via `libloading` with 25
  required and about 30 optional functions (`extern "system"`). Search path as in section 2.2,
  with the per-user root taken from `GCAM_MVSDK_HOME` (default `~/.gcam-mvsdk`), as in the
  Python package; `setup` installs there by default and `doctor` reports the source.
- Work item 2: `Sdk` (load once, enumerate, `gige_set_ip`, `find_device`), `DeviceInfo`
  (ASCII/GBK decoding, `gige_ip()`, `gige_mac()`), `Camera` (`Send`, not `Sync`), `Frame`
  (borrows the camera, releases the buffer on drop), `MvError`.
- Work item 3: CLI `setup | list [--json] | rename | setip | info | grab | doctor`.
- Work item 4: `setup` with HEAD check, streamed SHA-256, staged extraction, `manifest.json`,
  `--from`, `--dest`, `--version`, and the Windows `.rar` branch (not run on Windows).
- Work item 7: `sdk-manifest.toml` (byte-identical to `gcam-mvsdk-py`), `.gitignore`,
  `scripts/fetch-sdk.sh`, `scripts/check-no-vendor-files.sh`, minimal CI workflow.
- Additionally: `install.bash` (end-user installer), `CLAUDE.md`, `.claude/skills/`, README for
  production users, MIT/Apache-2.0 licence files.

Deviation: `CameraSetGigePacketSize` does not exist in SDK 2.1.0.49; packet size is set with
`CameraSetTransPackLen` (index into the capability's packet-size table), as the C++ publisher does.

Verified on this machine: `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`,
`cargo publish --dry-run`; the loader against the Dropbox SDK copy and against `./vendor`;
`setup --from` and one real download from the vendor URL (SHA-256 matched); `install.bash`
end to end with `--from` and `--no-sudo`.

Not verified: any camera operation. No camera answered discovery during implementation (the
process had no `CAP_NET_RAW`, `sudo` needed a password, and an unprivileged GigE Vision discovery
broadcast received no reply). The hardware tests (`GCAM_MVSDK_HW_TEST=1`) remain to be run on the
bench.

Not done: work items 5 (`cam-gige-pub`), 6 (`runscript.bash` using `list --json`), 8 (switching
`camera_ips` and `cam-gige-pub` to path dependencies), 9 (callback mode); the cross-compilation
matrix is not in CI; Windows is untested.
