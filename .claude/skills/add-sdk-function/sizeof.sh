#!/usr/bin/env bash
# Prints sizeof/alignof of an MVSDK struct and the offsets of the named fields, compiled with gcc
# against the vendor headers in a temporary directory (nothing is written to the repository).
# Usage: sizeof.sh <vendor include dir> <struct type> [field ...] [-- extra gcc flags, e.g. -m32]
set -euo pipefail
[[ $# -ge 2 ]] || { echo "usage: $0 <include dir> <struct> [field ...] [-- gcc flags]" >&2; exit 2; }
INC="$1"; TYPE="$2"; shift 2
FIELDS=(); CFLAGS=()
while [[ $# -gt 0 ]]; do
    if [[ "$1" == "--" ]]; then shift; CFLAGS=("$@"); break; fi
    FIELDS+=("$1"); shift
done
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
{
    echo '#include <stdio.h>'
    echo '#include <stddef.h>'
    echo '#include "CameraApi.h"'
    echo 'int main(void) {'
    printf '  printf("%s size=%%zu align=%%zu\\n", sizeof(%s), _Alignof(%s));\n' "$TYPE" "$TYPE" "$TYPE"
    for f in "${FIELDS[@]}"; do
        printf '  printf("  %s @ %%zu\\n", offsetof(%s, %s));\n' "$f" "$TYPE" "$f"
    done
    echo '  return 0; }'
} > "$TMP/sizes.c"
gcc "${CFLAGS[@]}" -I"$INC" -o "$TMP/sizes" "$TMP/sizes.c"
"$TMP/sizes"
