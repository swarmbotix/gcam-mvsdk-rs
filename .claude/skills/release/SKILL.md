---
name: release
description: Prepare a gcam-mvsdk-rs release - bump the version, run tests, clippy and publish --dry-run, prove the packaged crate contains no vendor bytes, tag, and give the final cargo publish command. Use when the user asks to release, tag, version-bump or publish the crate.
---

# release

Commit, tag, push and `cargo publish` only when the user explicitly asks for each of them.

## 1. Bump the version

- Edit `version` in `Cargo.toml` (semver: breaking API change before 1.0 bumps the minor version).
- Run `cargo build` so `Cargo.lock` records the new version.
- Update version references that users copy: README (`tag = "v0.1.0"` in the git dependency line).

## 2. Run every check

```bash
bash .claude/skills/release/release-check.sh
```

It runs, and stops at the first failure:

1. `sh scripts/check-no-vendor-files.sh` (git-tracked files)
2. `cargo build --locked` and `cargo build --locked --no-default-features`
3. `cargo test --locked`
4. `cargo clippy --locked --all-targets -- -D warnings`
5. `cmp sdk-manifest.toml ../gcam-mvsdk-py/sdk-manifest.toml` (skipped with a warning if the
   Python checkout is absent)
6. `cargo package --list --allow-dirty` checked for `.so`, `.dll`, `.dylib`, archives,
   `vendor/`, and the vendor header names
7. `cargo publish --dry-run --locked --allow-dirty --target-dir target/package-verify` (crate size
   must stay far below 10 MB; the separate target directory keeps `target/debug` valid)

Optionally run the hardware tests on the bench machine (see the hardware-test skill).

## 3. Tag (when asked)

```bash
git add -A && git status --short          # review: no vendor files, no target/
git commit -m "Release vX.Y.Z"
git tag -a vX.Y.Z -m "gcam-mvsdk-rs vX.Y.Z"
git push origin HEAD --follow-tags
```

`scripts/fetch-sdk.sh`, `scripts/check-no-vendor-files.sh` and `install.bash` need the executable
bit; on file systems with `core.fileMode=false` add them once with `git add --chmod=+x <file>`.

## 4. Publish (later, when asked)

```bash
cargo publish --locked
```

Consumers then use `gcam-mvsdk-rs = "X.Y"` and `cargo install gcam-mvsdk-rs`. The published
crate never contains the SDK; users still run `gcam-mvsdk-rs setup` or `install.bash`.
