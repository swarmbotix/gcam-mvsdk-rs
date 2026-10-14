#!/usr/bin/env bash
# Builds the hardware test binary and runs it as the current user with GCAM_MVSDK_HW_TEST=1.
# GigE discovery needs no privileges, so nothing here uses sudo.
# Extra arguments are passed to the test binary (for example a test name filter).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../../.."
BIN="$(cargo test --locked --test hw --no-run --message-format=json 2>/dev/null \
    | grep '"kind":\["test"\]' | grep '"name":"hw"' \
    | sed -n 's/.*"executable":"\([^"]*\)".*/\1/p' | tail -n1 || true)"
[[ -n "$BIN" && -x "$BIN" ]] || { echo "could not build the hw test binary" >&2; exit 1; }
export GCAM_MVSDK_HW_TEST=1
exec "$BIN" --test-threads=1 --nocapture "$@"
