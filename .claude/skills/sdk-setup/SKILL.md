---
name: sdk-setup
description: Install, re-install, verify or locate the HuaTeng MVSDK used by gcam-mvsdk-rs, and diagnose "could not find the HuaTeng MVSDK library". Use when the SDK is missing, when GCAM_MVSDK_HOME or GCAM_MVSDK_DIR is involved, when a checksum or download fails, or before running anything that loads the SDK.
---

# sdk-setup

The SDK is never stored in the repository. It is installed per machine into the SDK home,
`$GCAM_MVSDK_HOME/<version>/` (default `~/.gcam-mvsdk/<version>/`, for users), or into
`./vendor/<version>/` (developers), or pointed to with `GCAM_MVSDK_DIR`. The Python package
`gcam-mvsdk-py` uses the same SDK home, so one installation serves both.

## 1. See what the loader finds

```bash
cargo run -q -- doctor 2>&1 | head -12        # or: gcam-mvsdk-rs doctor
bash .claude/skills/sdk-setup/check-sdk.sh    # optional: checks one SDK directory in detail
```

`doctor` prints `SDK home: <path> (from $GCAM_MVSDK_HOME)` or `(default)`, then
`SDK library found: <path>` and the version, or the error listing every location checked. `check-sdk.sh [DIR]` verifies `manifest.json` against `sdk-manifest.toml`, the library
for this architecture, and that the required symbols are exported.

## 2. Install or re-install

| Situation | Command |
|---|---|
| End user, network available | `bash install.bash` (or `gcam-mvsdk-rs setup`) |
| Developer checkout | `./scripts/fetch-sdk.sh` (SDK into `./vendor/`) |
| Archive already downloaded / air-gapped | `gcam-mvsdk-rs setup --from <file>` or `bash install.bash --from <file>` |
| Replace a damaged installation | add `--force` |
| Other SDK home | `export GCAM_MVSDK_HOME=<DIR>` (also in `~/.bashrc`), then `gcam-mvsdk-rs setup` or `bash install.bash` |
| One-off destination | `gcam-mvsdk-rs setup --dest <DIR>` then `export GCAM_MVSDK_DIR=<DIR>/<version>` |
| Existing extracted SDK elsewhere | `export GCAM_MVSDK_DIR=<dir containing lib/>` |

`setup` refuses an archive whose size or SHA-256 does not match `sdk-manifest.toml`. Do not work
around this: download again, or if the vendor published a new version, add a manifest entry
(see CLAUDE.md, both repositories) after verifying the hash of a real download.

A verified copy of 2.1.0.49 for local testing: `GCAM_MVSDK_DIR="$HOME/Dropbox/Projects/huateng_gige_camera/linux/linuxSDK_V2.1.0.49(202511141513)"`.

## 3. Diagnose "could not find the HuaTeng MVSDK library"

1. Read the "Looked in" list in the error.
2. Compare the `SDK home` line of `doctor` with where the SDK was installed. An installation in
   a non-default home is found only while `GCAM_MVSDK_HOME` is set in the environment of the
   command (check `~/.bashrc`, systemd units, and `sudo`, which drops variables unless `-E`).
3. If `GCAM_MVSDK_DIR` is set, it is the only place searched. Check it points to the directory
   that contains `lib/` (not `lib/` itself, although that also works) and that
   `lib/<arch>/libMVSDK.so` exists. `unset GCAM_MVSDK_DIR` to fall back to the default locations.
4. `<version> (no SDK version installed)` means the root exists but has no version directory:
   run `setup`.
5. Wrong architecture: `lib/x64` on x86_64, `lib/arm64` on aarch64, `lib/arm` on armv7 hard
   float, `lib/x86` on i686. `GCAM_MVSDK_ARCH` overrides.
6. `could not open the MVSDK library ... : <dlopen error>`: the file exists but does not load
   (truncated download, wrong architecture, missing glibc). Re-run `setup --force` and compare
   `sha256sum` of the archive with `sdk-manifest.toml`.
7. `does not export the required function`: the library is not a compatible MVSDK build.

## 4. Never

- Copy `libMVSDK.so` or the headers into tracked paths, or commit `vendor/`.
- Edit `sdk-manifest.toml` without updating `../gcam-mvsdk-py/sdk-manifest.toml` identically.
