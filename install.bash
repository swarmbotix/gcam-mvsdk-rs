#!/usr/bin/env bash
# install.bash: install the gcam-mvsdk-rs command and the HuaTeng Vision MVSDK for this user.
#
#   bash install.bash                 # build, install, download + verify the SDK
#   bash install.bash --from FILE     # use an SDK archive downloaded by hand (air-gapped machines)
#   bash install.bash --no-sudo       # never call sudo; print the privileged commands instead
#   bash install.bash --dev           # developer checkout: cargo build, SDK into ./vendor
#   bash install.bash --uninstall     # remove the command (and, after confirmation, the SDK)
#
# Safe to run again: every step checks what is already in place.
set -euo pipefail

if [[ -z "${BASH_VERSINFO:-}" || "${BASH_VERSINFO[0]}" -lt 4 ]]; then
    echo "install.bash needs bash 4 or newer." >&2
    exit 1
fi

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PKG="gcam-mvsdk-rs"
MIN_RUST="1.88"
# SDK home: $GCAM_MVSDK_HOME (made absolute and exported, so the installed tool uses it too),
# else ~/.gcam-mvsdk. Same rule as the gcam-mvsdk-rs loader and the Python package.
HOME_FROM_ENV=0
GCAM_MVSDK_HOME="${GCAM_MVSDK_HOME:-}"
GCAM_MVSDK_HOME="${GCAM_MVSDK_HOME#"${GCAM_MVSDK_HOME%%[![:space:]]*}"}"   # trim leading blanks
GCAM_MVSDK_HOME="${GCAM_MVSDK_HOME%"${GCAM_MVSDK_HOME##*[![:space:]]}"}"   # trim trailing blanks
if [[ -n "$GCAM_MVSDK_HOME" ]]; then
    HOME_FROM_ENV=1
    # A literal "~" can arrive unexpanded (for example GCAM_MVSDK_HOME="~/sdk"); expand it here.
    # shellcheck disable=SC2088
    case "$GCAM_MVSDK_HOME" in
        "~") GCAM_MVSDK_HOME="$HOME" ;;
        "~/"*) GCAM_MVSDK_HOME="$HOME/${GCAM_MVSDK_HOME#"~/"}" ;;
        /*) ;;
        *) GCAM_MVSDK_HOME="$PWD/$GCAM_MVSDK_HOME" ;;
    esac
    export GCAM_MVSDK_HOME
    SDK_HOME="$GCAM_MVSDK_HOME"
else
    unset GCAM_MVSDK_HOME
    SDK_HOME="$HOME/.gcam-mvsdk"
fi
CARGO_BIN="${CARGO_INSTALL_ROOT:-${CARGO_HOME:-$HOME/.cargo}}/bin"

FROM_FILE=""
NO_SUDO=0
DEV=0
UNINSTALL=0
UDEV=0
ASSUME_YES=0

usage() {
    cat <<EOF
Usage: bash install.bash [options]

Installs the ${PKG} command into ${CARGO_BIN} and the vendor SDK into
\${GCAM_MVSDK_HOME:-\$HOME/.gcam-mvsdk} (currently ${SDK_HOME}).

Options:
  --from FILE   Install the SDK from a local archive instead of downloading it.
  --no-sudo     Never run sudo. Privileged steps are printed for you to run.
  --udev        Also install the vendor's USB udev rules (USB cameras only).
  --dev         Developer mode: cargo build in the checkout, SDK into ./vendor/.
  --uninstall   Remove the ${PKG} command; offer to remove the SDK directory.
  -y, --yes     Answer yes to every question (for unattended use).
  -h, --help    Show this help.

Environment:
  GCAM_MVSDK_HOME   SDK root directory (default \$HOME/.gcam-mvsdk).

See README.md for details.
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --from)
            [[ $# -ge 2 ]] || { echo "--from needs a file argument" >&2; exit 2; }
            FROM_FILE="$2"
            shift 2
            ;;
        --from=*) FROM_FILE="${1#--from=}"; shift ;;
        --no-sudo) NO_SUDO=1; shift ;;
        --udev) UDEV=1; shift ;;
        --dev) DEV=1; shift ;;
        --uninstall) UNINSTALL=1; shift ;;
        -y | --yes) ASSUME_YES=1; shift ;;
        -h | --help) usage; exit 0 ;;
        *) echo "Unknown option: $1 (see --help)" >&2; exit 2 ;;
    esac
done

if [[ -n "$FROM_FILE" ]]; then
    [[ -f "$FROM_FILE" ]] || { echo "--from: no such file: $FROM_FILE" >&2; exit 2; }
    FROM_FILE="$(cd "$(dirname "$FROM_FILE")" && pwd)/$(basename "$FROM_FILE")"
fi

STEP=0
step() { STEP=$((STEP + 1)); printf '==> [%d] %s\n' "$STEP" "$*"; }
info() { printf '    %s\n' "$*"; }
warn() { printf '    warning: %s\n' "$*" >&2; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

# Asks a yes/no question. Without a terminal, the answer is "no" unless --yes was given.
confirm() {
    local question="$1" default="${2:-n}" reply=""
    if [[ $ASSUME_YES -eq 1 ]]; then return 0; fi
    if [[ ! -t 0 ]]; then return 1; fi
    if [[ "$default" == "y" ]]; then read -r -p "    $question [Y/n] " reply || true; reply="${reply:-y}"
    else read -r -p "    $question [y/N] " reply || true; reply="${reply:-n}"; fi
    [[ "$reply" =~ ^[Yy] ]]
}

version_ge() { [[ "$(printf '%s\n%s\n' "$2" "$1" | sort -V | head -n1)" == "$2" ]]; }

# ---------------------------------------------------------------------------
# Platform
# ---------------------------------------------------------------------------

case "$(uname -s)" in
    Linux) ;;
    Darwin) die "macOS is not supported by this installer; see README.md (the vendor's macOS SDK has not been integrated)." ;;
    MINGW* | MSYS* | CYGWIN* | Windows_NT) die "Windows is not supported by this installer; see README.md (run the vendor installer, then 'cargo install --path .')." ;;
    *) die "unsupported operating system $(uname -s); see README.md." ;;
esac

OS_ID="unknown"; OS_LIKE=""; OS_NAME="Linux"
if [[ -r /etc/os-release ]]; then
    # shellcheck disable=SC1091
    OS_ID="$(. /etc/os-release && echo "${ID:-unknown}")"
    OS_LIKE="$(. /etc/os-release && echo "${ID_LIKE:-}")"
    OS_NAME="$(. /etc/os-release && echo "${PRETTY_NAME:-Linux}")"
fi
DEBIAN_LIKE=0
if [[ "$OS_ID" == "debian" || "$OS_ID" == "ubuntu" || " $OS_LIKE " == *" debian "* || " $OS_LIKE " == *" ubuntu "* ]]; then
    DEBIAN_LIKE=1
fi

MACHINE="$(uname -m)"
case "$MACHINE" in
    x86_64 | amd64) ARCH_DIR="x64" ;;
    aarch64 | arm64) ARCH_DIR="arm64" ;;
    i686 | i586 | i386) ARCH_DIR="x86" ;;
    armv7l | armv7*)
        ARCH_DIR="arm"
        if command -v dpkg >/dev/null 2>&1 && [[ "$(dpkg --print-architecture 2>/dev/null)" == "armel" ]]; then
            ARCH_DIR="arm_softfp"
        fi
        ;;
    *) die "unsupported CPU architecture '$MACHINE'. The vendor ships Linux SDKs for x86_64, aarch64, armv7l and i686 only." ;;
esac

SUDO=""
if [[ $EUID -ne 0 ]]; then SUDO="sudo"; fi

# Runs a privileged command, reusing the credentials cached by the single prompt.
run_privileged() {
    if [[ -z "$SUDO" ]]; then "$@"; else sudo -n "$@"; fi
}

# ---------------------------------------------------------------------------
# Uninstall
# ---------------------------------------------------------------------------

if [[ $UNINSTALL -eq 1 ]]; then
    step "Uninstalling ${PKG}"
    if command -v cargo >/dev/null 2>&1 || [[ -x "${CARGO_HOME:-$HOME/.cargo}/bin/cargo" ]]; then
        CARGO="$(command -v cargo || echo "${CARGO_HOME:-$HOME/.cargo}/bin/cargo")"
        INSTALLED="$("$CARGO" install --list 2>/dev/null || true)"
        if grep -q "^${PKG} " <<<"$INSTALLED"; then
            "$CARGO" uninstall "$PKG"
            info "removed ${CARGO_BIN}/${PKG}"
        else
            info "${PKG} is not installed with cargo; nothing to remove"
        fi
    else
        info "cargo not found; nothing to remove"
    fi
    step "SDK directory ${SDK_HOME}"
    if [[ -d "$SDK_HOME" ]]; then
        if confirm "Remove ${SDK_HOME} (downloaded vendor SDK)?" n; then
            rm -rf -- "$SDK_HOME"
            info "removed ${SDK_HOME}"
        else
            info "kept; remove it later with: rm -rf '${SDK_HOME}'"
        fi
    else
        info "not present"
    fi
    info "USB udev rules, if you installed them, stay in /etc/udev/rules.d/ (88-mvusb.rules, 99-mvusb.rules)."
    exit 0
fi

[[ -f "$REPO_DIR/Cargo.toml" ]] || die "Cargo.toml not found next to install.bash ($REPO_DIR)."

step "Platform: ${OS_NAME}, ${MACHINE} (SDK library directory lib/${ARCH_DIR})"

# ---------------------------------------------------------------------------
# Plan privileged actions up front so that sudo asks for a password once.
# ---------------------------------------------------------------------------

APT_PACKAGES=()
if ! command -v cc >/dev/null 2>&1 && ! command -v gcc >/dev/null 2>&1; then APT_PACKAGES+=(build-essential); fi
if ! command -v pkg-config >/dev/null 2>&1; then APT_PACKAGES+=(pkg-config); fi

SUDO_PLAN=()
if [[ ${#APT_PACKAGES[@]} -gt 0 && $DEBIAN_LIKE -eq 1 ]]; then
    SUDO_PLAN+=("apt-get install -y ${APT_PACKAGES[*]}")
fi
if [[ $UDEV -eq 1 ]]; then
    SUDO_PLAN+=("cp ${SDK_HOME}/<version>/udev/*.rules /etc/udev/rules.d/ && udevadm control --reload-rules && udevadm trigger")
fi

SUDO_OK=0
step "Privileged steps"
if [[ ${#SUDO_PLAN[@]} -eq 0 ]]; then
    info "none needed"
elif [[ $NO_SUDO -eq 1 || $DEV -eq 1 ]]; then
    info "skipped (--no-sudo or --dev); the commands are printed where they apply"
else
    for c in "${SUDO_PLAN[@]}"; do info "${SUDO:+sudo }$c"; done
    if confirm "Run these with ${SUDO:-root privileges} (one password prompt)?" y; then
        if [[ -z "$SUDO" ]]; then
            SUDO_OK=1
        elif ! command -v sudo >/dev/null 2>&1; then
            warn "sudo is not installed; the commands are printed where they apply"
        elif sudo -v; then
            SUDO_OK=1
            # Keep the cached credentials fresh during the build so there is no second prompt.
            ( while sleep 50; do sudo -n -v 2>/dev/null || exit 0; done ) &
            SUDO_KEEPALIVE=$!
            trap 'kill "$SUDO_KEEPALIVE" 2>/dev/null || true' EXIT
        else
            warn "sudo authentication failed; the commands are printed where they apply"
        fi
    else
        info "not run (declined, or no terminal; use --yes for unattended runs); the commands are printed where they apply"
    fi
fi

# ---------------------------------------------------------------------------
# Rust toolchain and build tools
# ---------------------------------------------------------------------------

step "Rust toolchain"
if ! command -v cargo >/dev/null 2>&1 && [[ -f "${CARGO_HOME:-$HOME/.cargo}/env" ]]; then
    # shellcheck disable=SC1091
    . "${CARGO_HOME:-$HOME/.cargo}/env"
fi
if ! command -v cargo >/dev/null 2>&1; then
    command -v curl >/dev/null 2>&1 || die "cargo is missing and curl is not installed; install curl or Rust (https://rustup.rs) first."
    info "cargo not found; installing Rust with rustup (minimal profile) into ${CARGO_HOME:-$HOME/.cargo}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    # shellcheck disable=SC1091
    . "${CARGO_HOME:-$HOME/.cargo}/env"
fi
RUSTC_VERSION="$(rustc --version | awk '{print $2}')"
if ! version_ge "$RUSTC_VERSION" "$MIN_RUST"; then
    if command -v rustup >/dev/null 2>&1; then
        info "rustc ${RUSTC_VERSION} is older than ${MIN_RUST}; running rustup update stable"
        rustup update stable
        RUSTC_VERSION="$(rustc --version | awk '{print $2}')"
    fi
    version_ge "$RUSTC_VERSION" "$MIN_RUST" || die "rustc ${RUSTC_VERSION} is too old; ${PKG} needs ${MIN_RUST} or newer."
fi
info "rustc ${RUSTC_VERSION}, $(cargo --version)"

step "Build tools (C linker, pkg-config)"
if [[ ${#APT_PACKAGES[@]} -eq 0 ]]; then
    info "present"
else
    if [[ $DEBIAN_LIKE -eq 1 && $SUDO_OK -eq 1 ]]; then
        info "${SUDO:+sudo }apt-get install -y ${APT_PACKAGES[*]}"
        run_privileged apt-get install -y "${APT_PACKAGES[@]}"
    elif ! command -v cc >/dev/null 2>&1 && ! command -v gcc >/dev/null 2>&1; then
        if [[ $DEBIAN_LIKE -eq 1 ]]; then
            die "no C compiler/linker found. Run: sudo apt-get install -y build-essential pkg-config"
        fi
        die "no C compiler/linker found. Install gcc with your distribution's package manager."
    else
        warn "missing ${APT_PACKAGES[*]} (not required to build ${PKG}); install with: sudo apt-get install -y ${APT_PACKAGES[*]}"
    fi
fi

# ---------------------------------------------------------------------------
# Build and install the command
# ---------------------------------------------------------------------------

# The `view` subcommand (OpenCV window) is compiled in only when OpenCV and libclang are present,
# because the opencv crate needs both at build time; everything else never needs them.
VIEW_FEATURE=()
if command -v pkg-config >/dev/null 2>&1 && pkg-config --exists opencv4 2>/dev/null; then
    if [[ -z "${LIBCLANG_PATH:-}" ]]; then
        for d in /usr/lib/llvm-*/lib; do
            if compgen -G "$d/libclang.so*" >/dev/null; then export LIBCLANG_PATH="$d"; fi
        done
    fi
    if [[ -n "${LIBCLANG_PATH:-}" ]] || compgen -G "/usr/lib/*/libclang.so*" >/dev/null; then
        VIEW_FEATURE=(--features view)
        info "OpenCV and libclang found: the view subcommand is included (--features view${LIBCLANG_PATH:+, LIBCLANG_PATH=$LIBCLANG_PATH})"
    else
        info "OpenCV found but no libclang: the view subcommand is left out (install clang libclang-dev and rerun)"
    fi
else
    info "OpenCV not found: the view subcommand is left out (install libopencv-dev clang libclang-dev and rerun to add it)"
fi

if [[ $DEV -eq 1 ]]; then
    step "Building ${PKG} in ${REPO_DIR} (cargo build --locked ${VIEW_FEATURE[*]})"
    (cd "$REPO_DIR" && cargo build --locked --quiet "${VIEW_FEATURE[@]}") || die "cargo build failed. If this machine is offline, the crates in Cargo.lock must already be in the cargo cache (run 'cargo fetch --locked' while online)."
    BIN="$REPO_DIR/target/debug/$PKG"
    SDK_DEST="$REPO_DIR/vendor"
else
    step "Installing ${PKG} into ${CARGO_BIN} (cargo install --locked ${VIEW_FEATURE[*]}; takes a minute)"
    cargo install --path "$REPO_DIR" --locked --quiet "${VIEW_FEATURE[@]}" || die "cargo install failed. If this machine is offline, the crates in Cargo.lock must already be in the cargo cache (run 'cargo fetch --locked' while online)."
    BIN="$CARGO_BIN/$PKG"
    SDK_DEST="$SDK_HOME"
fi
[[ -x "$BIN" ]] || die "expected binary not found: $BIN"
info "installed $BIN ($("$BIN" --version))"
if [[ $DEV -eq 0 && ":$PATH:" != *":$CARGO_BIN:"* ]]; then
    warn "${CARGO_BIN} is not on PATH. Add it, for example to ~/.bashrc:"
    info "export PATH=\"${CARGO_BIN}:\$PATH\""
fi

# ---------------------------------------------------------------------------
# Vendor SDK
# ---------------------------------------------------------------------------

manifest_field() {
    # Prints a field of the Linux release in sdk-manifest.toml.
    awk -v want="$1" '
        /^\[\[release\]\]/ { os = ""; val = "" }
        $1 == "os"  { gsub(/"/, "", $3); os = $3 }
        $1 == want  { v = $3; gsub(/"/, "", v); val = v }
        os == "linux" && val != "" { print val; exit }
    ' "$REPO_DIR/sdk-manifest.toml"
}

if [[ -n "$FROM_FILE" ]]; then
    step "Installing the vendor SDK from ${FROM_FILE} into ${SDK_DEST}"
    SETUP_ARGS=(setup --dest "$SDK_DEST" --from "$FROM_FILE")
else
    step "Downloading and verifying the vendor SDK into ${SDK_DEST}"
    SETUP_ARGS=(setup --dest "$SDK_DEST")
fi
if ! "$BIN" "${SETUP_ARGS[@]}"; then
    echo >&2
    echo "The SDK could not be installed. To install it by hand:" >&2
    echo "  1. Download $(manifest_field url)" >&2
    echo "     (or the Linux SDK from http://huatengvision.com/download/1/)" >&2
    echo "     expected size $(manifest_field size) bytes, SHA-256 $(manifest_field sha256)" >&2
    echo "  2. Run: bash install.bash --from /path/to/the/downloaded.tar.gz" >&2
    exit 1
fi

SDK_VERSION_DIR="$(find "$SDK_DEST" -mindepth 1 -maxdepth 1 -type d -name '[0-9]*' 2>/dev/null | sort -V | tail -n1)"
[[ -n "$SDK_VERSION_DIR" && -f "$SDK_VERSION_DIR/lib/$ARCH_DIR/libMVSDK.so" ]] \
    || die "no lib/${ARCH_DIR}/libMVSDK.so under ${SDK_DEST}; the archive does not support this machine."

# The tool finds the SDK through GCAM_MVSDK_HOME (exported above when set); GCAM_MVSDK_DIR
# would override it.
if [[ -n "${GCAM_MVSDK_DIR:-}" ]]; then
    warn "GCAM_MVSDK_DIR=${GCAM_MVSDK_DIR} is set and takes precedence over ${SDK_DEST}."
fi

# ---------------------------------------------------------------------------
# USB udev rules (GigE discovery needs no privileges)
# ---------------------------------------------------------------------------

step "USB udev rules"
if [[ $UDEV -eq 0 ]]; then
    info "skipped (USB cameras only; rerun with --udev, or: sudo cp ${SDK_VERSION_DIR}/udev/*.rules /etc/udev/rules.d/)"
elif [[ $SUDO_OK -eq 1 ]]; then
    info "${SUDO:+sudo }cp ${SDK_VERSION_DIR}/udev/*.rules /etc/udev/rules.d/ && udevadm control --reload-rules && udevadm trigger"
    run_privileged cp "$SDK_VERSION_DIR"/udev/*.rules /etc/udev/rules.d/
    run_privileged udevadm control --reload-rules
    run_privileged udevadm trigger || true
    info "done"
else
    info "run: sudo cp ${SDK_VERSION_DIR}/udev/*.rules /etc/udev/rules.d/ && sudo udevadm control --reload-rules"
fi

# ---------------------------------------------------------------------------
# Checks
# ---------------------------------------------------------------------------

CHECK_DIR="$PWD"
if [[ $DEV -eq 1 ]]; then CHECK_DIR="$REPO_DIR"; fi

step "Host check: ${PKG} doctor"
DOCTOR_STATUS=0
(cd "$CHECK_DIR" && "$BIN" doctor) || DOCTOR_STATUS=$?

step "Camera list: ${PKG} list"
LIST_STATUS=0
# The SDK writes per-camera files when a camera is opened; keep them under the SDK root.
(cd "$CHECK_DIR" && "$BIN" --data-dir "$SDK_DEST/data" list) || LIST_STATUS=$?

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

step "Summary"
info "command : $BIN"
info "SDK     : $SDK_VERSION_DIR ($(basename "$SDK_VERSION_DIR"), lib/${ARCH_DIR})"
if [[ $HOME_FROM_ENV -eq 1 && $DEV -eq 0 ]]; then
    info "keep GCAM_MVSDK_HOME set so the tool finds the SDK; add to ~/.bashrc:"
    info "  export GCAM_MVSDK_HOME=\"$SDK_HOME\""
fi
if [[ $DOCTOR_STATUS -ne 0 ]]; then info "doctor reported a failure (exit $DOCTOR_STATUS); see its output above"; fi
if [[ $LIST_STATUS -ne 0 ]]; then info "list failed (exit $LIST_STATUS); see its output above"; fi
if [[ $DEV -eq 1 ]]; then
    info "next    : cargo run -- list"
    info "          GCAM_MVSDK_HW_TEST=1 cargo test --test hw -- --nocapture"
else
    info "next    : ${PKG} list --json"
    info "          ${PKG} grab <NAME or SN> -n 3 -o frames/"
fi
