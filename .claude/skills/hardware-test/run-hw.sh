#!/usr/bin/env bash
# Builds the hardware test binary as the current user and runs it with GCAM_MVSDK_HW_TEST=1,
# using sudo -E only for the test binary when this user lacks CAP_NET_RAW.
# Extra arguments are passed to the test binary (for example a test name filter).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../../.."
BIN="$(cargo test --locked --test hw --no-run --message-format=json 2>/dev/null \
    | grep '"kind":\["test"\]' | grep '"name":"hw"' \
    | sed -n 's/.*"executable":"\([^"]*\)".*/\1/p' | tail -n1 || true)"
[[ -n "$BIN" && -x "$BIN" ]] || { echo "could not build the hw test binary" >&2; exit 1; }
CAPS="$(awk '/^CapEff:/ {print $2}' /proc/self/status)"
PRIV=0
if [[ $EUID -eq 0 ]] || (( (16#${CAPS:-0} >> 13) & 1 )); then PRIV=1; fi
export GCAM_MVSDK_HW_TEST=1
ARGS=(--test-threads=1 --nocapture "$@")
if [[ $PRIV -eq 1 ]]; then
    exec "$BIN" "${ARGS[@]}"
fi
echo "No CAP_NET_RAW: running the test binary with sudo -E (GigE discovery needs it)."
exec sudo -E "$BIN" "${ARGS[@]}"
