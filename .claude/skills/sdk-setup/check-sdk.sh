#!/usr/bin/env bash
# Checks one installed SDK directory: manifest.json against sdk-manifest.toml, the library for
# this architecture, and the exported symbols that gcam-mvsdk-rs requires.
# Usage: check-sdk.sh [SDK_VERSION_DIR]
#   default: newest version under ./vendor, then under ${GCAM_MVSDK_HOME:-~/.gcam-mvsdk}
set -euo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
case "$(uname -m)" in
    x86_64) ARCH=x64 ;; aarch64) ARCH=arm64 ;; armv7l) ARCH=arm ;; i686 | i386) ARCH=x86 ;;
    *) echo "unsupported architecture $(uname -m)"; exit 1 ;;
esac
DIR="${1:-}"
if [[ -z "$DIR" ]]; then
    for root in "$REPO/vendor" "${GCAM_MVSDK_HOME:-$HOME/.gcam-mvsdk}"; do
        cand="$(find "$root" -mindepth 1 -maxdepth 1 -type d -name '[0-9]*' 2>/dev/null | sort -V | tail -n1)"
        if [[ -n "$cand" ]]; then DIR="$cand"; break; fi
    done
fi
[[ -n "$DIR" && -d "$DIR" ]] || { echo "no SDK directory found; run setup first"; exit 1; }
echo "SDK directory: $DIR"
LIB="$DIR/lib/$ARCH/libMVSDK.so"
if [[ -f "$LIB" ]]; then echo "library: $LIB ($(stat -c %s "$LIB") bytes)"; else echo "MISSING: $LIB"; exit 1; fi
if [[ -f "$DIR/manifest.json" ]]; then
    sha="$(sed -n 's/.*"sha256": *"\([0-9a-f]*\)".*/\1/p' "$DIR/manifest.json")"
    ver="$(sed -n 's/.*"version": *"\([^"]*\)".*/\1/p' "$DIR/manifest.json")"
    if grep -q "$sha" "$REPO/sdk-manifest.toml" && [[ -n "$sha" ]]; then
        echo "manifest.json: version $ver, sha256 matches sdk-manifest.toml"
    else
        echo "WARNING: manifest.json sha256 '$sha' is not in sdk-manifest.toml"
    fi
else
    echo "no manifest.json (not installed by setup)"
fi
if command -v nm >/dev/null 2>&1; then
    missing=0
    syms="$(nm -D --defined-only "$LIB")"
    for sym in CameraSdkInit CameraEnumerateDevice CameraInit CameraUnInit CameraGetCapability \
        CameraGetFriendlyName CameraSetFriendlyName CameraGigeGetIp CameraGigeSetIp CameraGigeGetMac \
        CameraSetAeState CameraSetExposureTime CameraGetExposureTime CameraSetFrameRate \
        CameraSetIspOutFormat CameraSetTriggerMode CameraPlay CameraPause CameraGetImageBuffer \
        CameraReleaseImageBuffer CameraImageProcess CameraSetTransPackLen CameraGetErrorString \
        CameraSaveParameter CameraLoadParameter; do
        grep -qw "$sym" <<<"$syms" || { echo "MISSING symbol: $sym"; missing=1; }
    done
    if [[ $missing -eq 0 ]]; then echo "all 25 required symbols exported"; else exit 1; fi
fi
