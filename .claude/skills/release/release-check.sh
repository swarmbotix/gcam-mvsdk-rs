#!/usr/bin/env bash
# Runs every pre-release check for gcam-mvsdk-rs. Stops at the first failure.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../../.."
run() { printf '==> %s\n' "$*"; "$@"; }

run sh scripts/check-no-vendor-files.sh
run cargo build --locked --quiet
run cargo build --locked --quiet --no-default-features
run cargo test --locked --quiet
run cargo clippy --locked --quiet --all-targets -- -D warnings

if [[ -f ../gcam-mvsdk-py/sdk-manifest.toml ]]; then
    run cmp sdk-manifest.toml ../gcam-mvsdk-py/sdk-manifest.toml
    echo "    sdk-manifest.toml is byte-identical to ../gcam-mvsdk-py"
else
    echo "warning: ../gcam-mvsdk-py/sdk-manifest.toml not found; compare the manifests by hand" >&2
fi

echo "==> cargo package --list --allow-dirty"
LIST="$(cargo package --list --allow-dirty 2>/dev/null)"
BAD="$(grep -E '\.(so|so\.[0-9.]+|dll|dylib|lib|exe|chm|tar\.gz|tgz|rar|zip|7z)$|(^|/)(CameraApi|CameraDefine|CameraStatus)\.h$|^vendor/' <<<"$LIST" || true)"
if [[ -n "$BAD" ]]; then
    echo "The package would contain vendor files:" >&2
    echo "$BAD" >&2
    exit 1
fi
echo "    $(wc -l <<<"$LIST") files, no vendor files"

# A separate target directory: the dry run's verification build would otherwise overwrite the
# dev-profile fingerprints in target/debug, leaving target/debug/gcam-mvsdk-rs stale afterwards.
run cargo publish --dry-run --locked --allow-dirty --target-dir target/package-verify
echo "All release checks passed."
