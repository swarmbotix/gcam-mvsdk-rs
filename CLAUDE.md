# CLAUDE.md

Guidance for Claude Code sessions in this repository.

## Purpose

Rust library and CLI for HuaTeng Vision GigE/USB cameras. The vendor MVSDK (`libMVSDK.so`,
`MVCAMSDK_X64.dll`) is opened at runtime with `libloading`; nothing links against it at build
time, so `cargo build` and `cargo test` work on machines that have never seen the SDK. Users run
`bash install.bash`; the SDK itself is downloaded from the vendor and SHA-256 verified.

## Naming (exact, no variants)

- Repository, package, binary: `gcam-mvsdk-rs`. In Rust code: `gcam_mvsdk_rs`.
- One crate only (library + binary). No `-sys` crate; the raw FFI is the module `gcam_mvsdk_rs::ffi`.
- Never shorten to `gcam`, `mvcam` or similar in names, paths, env vars or docs.
- Shared with the Python package `gcam-mvsdk-py`: the SDK home `$GCAM_MVSDK_HOME` (default
  `~/.gcam-mvsdk`) with `<version>/lib/<arch>/`, `GCAM_MVSDK_DIR`, `GCAM_MVSDK_ARCH`,
  `GCAM_MVSDK_HW_TEST`, and `sdk-manifest.toml`. Keep the home rule identical to the Python
  `sdk_home()`: trim, empty means unset, expand a leading `~`.

## Never commit vendor files

The vendor forbids redistribution. Never add `.so`, `.dll`, `.dylib`, archives, `vendor/`,
`CameraApi.h`, `CameraDefine.h`, `CameraStatus.h`, or text copied from those headers.
`.gitignore` covers them; `sh scripts/check-no-vendor-files.sh` fails if any is tracked (CI runs
it). Before a release also check `cargo package --list --allow-dirty`. Struct layouts and
signatures in `src/ffi.rs` are hand-written in Rust syntax, not pasted header text.

## Layout

- `src/lib.rs`: crate docs, module declarations, re-exports of `sdk`, `camera`, `error`.
- `src/ffi.rs`: `#[repr(C)]` structs, status codes and `STATUS_TABLE`, media types, text decoding (ASCII, else GBK), `Api` function table built by `define_api!`.
- `src/sdk.rs`: platform/arch mapping, search path (`SearchConfig`, `resolve_library`), `Sdk` (load once, enumerate, GigE IP), `DeviceInfo`.
- `src/camera.rs`: `Camera` (RAII, `CameraUnInit` on drop), `Frame` (releases its buffer on drop), `Capability`, enums.
- `src/error.rs`: `MvError` (status + `CameraGetErrorString` text), `LoadError`, crate `Error`, `Result`.
- `src/setup.rs` (feature `setup`): manifest parsing, HEAD/GET download, SHA-256, tar extraction, Windows installer branch.
- `src/doctor.rs`: host checks (CAP_NET_RAW, rp_filter, firewall, NIC subnets).
- `src/main.rs` (feature `cli`): clap CLI `setup | list | rename | setip | info | grab | doctor`.
- `tests/`: struct sizes, status table, search path, manifest, setup/extraction; `tests/hw/` hardware tests.
- `examples/live_view.rs` (feature `view`, `required-features`): OpenCV window with FPS overlay; the
  only code that uses OpenCV.
- `install.bash`: end-user installer. `scripts/fetch-sdk.sh`: SDK into `./vendor/`.
- `sdk-manifest.toml`: known vendor releases. `.claude/skills/`: task procedures.

## Commands

```bash
cargo build
cargo test                                        # no SDK or hardware needed
cargo clippy --all-targets -- -D warnings
cargo build --no-default-features                 # library without CLI/downloader
cargo publish --dry-run --allow-dirty --target-dir target/package-verify
sh scripts/check-no-vendor-files.sh
./scripts/fetch-sdk.sh --from <tarball>           # SDK into ./vendor/ for cargo run
GCAM_MVSDK_HW_TEST=1 cargo test --test hw -- --nocapture   # bench machine only
cargo clippy --features view --example live_view -- -D warnings   # needs libopencv-dev clang libclang-dev
```

Without `--features view` every command above skips the example, so none of them needs OpenCV;
the CI job `example-view` installs OpenCV and is the build check for the example.

All of these must pass before work is considered done. Run the publish dry run with its own
`--target-dir`: with the shared target directory, its verification build overwrites the
fingerprints of `target/debug/gcam-mvsdk-rs`, which then stays stale after later edits (fix with
`cargo clean -p gcam-mvsdk-rs`).

## Loader search order

1. `$GCAM_MVSDK_DIR/lib/<arch>`, `$GCAM_MVSDK_DIR/lib`, `$GCAM_MVSDK_DIR` (exclusive when set).
2. `./vendor/<version>/lib/<arch>` (newest version first).
3. `<sdk home>/<version>/lib/<arch>`; sdk home = `$GCAM_MVSDK_HOME`, else `~/.gcam-mvsdk`
   (`sdk::resolve_sdk_home`; `setup --dest` defaults to the same directory).
4. Library path recorded under `library` in `<sdk home>/manifest.json` (Windows).
5. System loader with the bare file name.

`<arch>`: `x64`, `x86`, `arm64`, `arm` (hard float), `arm_softfp`; override with `GCAM_MVSDK_ARCH`.
`Sdk` is loaded and `CameraSdkInit(1)` called once per process; later loads return the same instance.
Tests build `SearchConfig` by hand or with `SearchConfig::from_lookup`; never modify the process
environment in tests.

## SDK source and sdk-manifest.toml

The SDK comes from http://huatengvision.com/download/1/ (plain HTTP; the checksum is mandatory).
To add a release, append one `[[release]]` entry (`version`, `os`, `url` percent-encoded, `size`,
`sha256` lowercase hex, `layout` = `tar` or `rar-installer`). The file must stay byte-identical
to `../gcam-mvsdk-py/sdk-manifest.toml`: edit both, then
`cmp sdk-manifest.toml ../gcam-mvsdk-py/sdk-manifest.toml` and update `EXPECTED_MANIFEST` in
`tests/manifest.rs`. Verify size and hash from a real download (`sha256sum`), never from memory.

## Wrapping another MVSDK function

1. Read the declaration in the vendor `CameraApi.h` (outside this repo, for example
   `~/.gcam-mvsdk/2.1.0.49/include/`) and the matching call in the vendor `mvsdk.py` for buffer sizes.
2. In `src/ffi.rs`, add one line to `define_api!`: under `optional` unless every supported SDK
   build exports it (`nm -D --defined-only libMVSDK.so | grep <Name>`). Map `int`/`BOOL`/`INT`/`CameraHandle`
   to `c_int`, `UINT` to `c_uint`, `char*` to `*mut c_char`, `BYTE*` to `*mut u8`, `double*` to `*mut f64`.
   New structs get `#[repr(C)]` and a size/offset assertion in `tests/struct_sizes.rs`, measured
   with gcc (`sizeof`, `offsetof`) in a scratch directory.
3. Add a safe method in `src/camera.rs` (per-handle) or `src/sdk.rs` (global): resolve optional
   pointers with `.ok_or(Error::Unsupported("CameraXxx"))?`, call inside `unsafe` with a
   `// SAFETY:` comment, convert the status with `self.check("CameraXxx", status)?`.
4. Add a unit test for any pure logic, a hardware test in `tests/hw/main.rs` if it touches a
   camera, and expose it in the CLI only if users need it. See `.claude/skills/add-sdk-function/`.

## Conventions

- Public fallible APIs return `gcam_mvsdk_rs::Result<T>` (`Result<T, Error>`); every SDK status
  failure is `Error::Sdk(MvError)` produced by `Sdk::check`. Do not return bare status codes.
- No `unwrap()` outside tests. `expect` only for invariants that cannot fail, with a message.
- Every `unsafe` block has a `// SAFETY:` comment stating why the call is sound.
- `Camera` is `Send` but not `Sync` (a `Cell` enforces it; a test checks it). `Frame<'_>` borrows
  its camera. Do not add `unsafe impl Sync`.
- Function pointers use `extern "system"` (stdcall on 32-bit Windows, C elsewhere).
- Text from the SDK is decoded with `ffi::decode_c_chars` (ASCII, else GBK), never as UTF-8.
- Professional wording in docs and messages; no em-dash characters.

## Do not

- Commit vendor binaries, headers, archives, or `vendor/`; or add a `build.rs` that touches the SDK.
- Link the vendor SDK at build time, add bindgen for it, or commit generated bindings. (The
  `opencv` crate builds with libclang, which is why it stays an optional dependency behind `view`;
  never add `view` to the default features or use OpenCV from `src/`.)
- Add a second crate or rename anything.
- Call `CameraSdkInit` outside `Sdk`, or close a handle outside `Camera::drop`.
- Write to the SDK home from tests (use `tempfile`), or run `rename`/`setip` against production
  cameras without the user's explicit instruction.
- Edit `sdk-manifest.toml` in only one of the two repositories.
- Commit or push unless asked.
