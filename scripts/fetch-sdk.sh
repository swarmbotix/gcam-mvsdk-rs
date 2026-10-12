#!/usr/bin/env bash
# Developer bootstrap: download, verify (SHA-256) and extract the vendor SDK into ./vendor/<version>/.
# Extra arguments are passed to `gcam-mvsdk-rs setup`, for example: --from ~/Downloads/linuxSDK_V2.1.0.49.tar.gz
set -euo pipefail
cd "$(dirname "$0")/.."
exec cargo run --quiet -- setup --dest vendor/ "$@"
