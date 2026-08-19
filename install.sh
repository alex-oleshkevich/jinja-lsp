#!/usr/bin/env bash
# jinja-lsp installer.
#
#   curl -fsSL https://raw.githubusercontent.com/alex-oleshkevich/jinja-lsp/master/install.sh | bash
#
# Downloads the release binary for this machine, verifies its checksum, and puts
# it in your user binary directory. No sudo, nothing written outside that
# directory, no system packages touched.
#
# Environment:
#   JINJA_LSP_VERSION       tag to install (default: the latest release)
#   JINJA_LSP_INSTALL_DIR   where to put the binary (default: ~/.local/bin)

set -euo pipefail

REPO="alex-oleshkevich/jinja-lsp"
BINARY="jinja-lsp"
# Global, not a `local` in main(): the EXIT trap runs after main's scope is gone,
# and under `set -u` an unbound name there aborts the trap with a confusing error
# *after* a successful install.
TMP_DIR=""

cleanup() { [ -n "$TMP_DIR" ] && rm -rf "$TMP_DIR"; }
trap cleanup EXIT

die() {
    printf '\033[31merror:\033[0m %s\n' "$1" >&2
    exit 1
}

info() { printf '\033[36m==>\033[0m %s\n' "$1" >&2; }
warn() { printf '\033[33mwarning:\033[0m %s\n' "$1" >&2; }

need() {
    command -v "$1" >/dev/null 2>&1 || die "'$1' is required but not installed."
}

# ── Which build do we need? ───────────────────────────────────────────────────

detect_target() {
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$arch" in
        x86_64 | amd64) arch=x86_64 ;;
        aarch64 | arm64) arch=aarch64 ;;
        *) die "unsupported architecture '$arch'. Build from source: https://github.com/$REPO" ;;
    esac

    case "$os" in
        Linux)
            # The Linux builds link glibc. On musl (Alpine and friends) they will
            # not run, and the failure mode is a confusing loader error — say so
            # here instead.
            if command -v ldd >/dev/null 2>&1 && ldd --version 2>&1 | grep -qi musl; then
                warn "this looks like a musl system; the release binaries link glibc and may not run."
                warn "if it fails, build from source: cargo install --git https://github.com/$REPO"
            fi
            # Spelled out rather than composed from $arch: these must match the
            # release build matrix exactly, and a parity test greps for them.
            case "$arch" in
                x86_64) echo "x86_64-unknown-linux-gnu.tar.gz" ;;
                aarch64) echo "aarch64-unknown-linux-gnu.tar.gz" ;;
            esac
            ;;
        Darwin)
            # Only Apple Silicon is built. Rosetta runs x86_64 binaries on arm64,
            # not the reverse, so an Intel Mac has nothing to fall back to.
            [ "$arch" = "aarch64" ] || die \
                "Intel macOS has no published build. Build from source: cargo install --git https://github.com/$REPO"
            echo "aarch64-apple-darwin.tar.gz"
            ;;
        MINGW* | MSYS* | CYGWIN*)
            [ "$arch" = "x86_64" ] || die "only x86_64 Windows is published."
            echo "x86_64-pc-windows-msvc.zip"
            ;;
        *)
            die "unsupported OS '$os'. Build from source: https://github.com/$REPO"
            ;;
    esac
}

# Resolve the newest tag by following the /releases/latest redirect rather than
# calling the API, which rate-limits unauthenticated callers to 60/hour — plenty
# to break a CI pipeline that installs on every run.
latest_version() {
    local url
    url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest")"
    printf '%s' "${url##*/}"
}

install_dir() {
    if [ -n "${JINJA_LSP_INSTALL_DIR:-}" ]; then
        printf '%s' "$JINJA_LSP_INSTALL_DIR"
    elif [ -n "${XDG_BIN_HOME:-}" ]; then
        printf '%s' "$XDG_BIN_HOME"
    else
        printf '%s' "$HOME/.local/bin"
    fi
}

verify_checksum() {
    local file="$1" sums="$2" expected actual
    # The published file is `<hash>  <bare archive name>`; compare the hash alone
    # so verification does not depend on our temp directory layout.
    expected="$(awk '{print $1}' "$sums")"
    if command -v sha256sum >/dev/null 2>&1; then
        actual="$(sha256sum "$file" | awk '{print $1}')"
    elif command -v shasum >/dev/null 2>&1; then
        actual="$(shasum -a 256 "$file" | awk '{print $1}')"
    else
        die "neither sha256sum nor shasum found; cannot verify the download."
    fi
    [ "$expected" = "$actual" ] || die "checksum mismatch — expected $expected, got $actual. Refusing to install."
}

main() {
    need curl
    need tar

    local target version asset base dest
    target="$(detect_target)"
    version="${JINJA_LSP_VERSION:-$(latest_version)}"
    [ -n "$version" ] || die "could not determine the latest version; set JINJA_LSP_VERSION."
    # Accept either `0.2.0` or `v0.2.0`.
    case "$version" in v*) ;; *) version="v$version" ;; esac

    asset="$BINARY-$version-$target"
    base="https://github.com/$REPO/releases/download/$version"
    dest="$(install_dir)"

    # Strip the archive extension so the message names the platform, not the file.
    info "installing $BINARY $version (${target%%.*})"

    TMP_DIR="$(mktemp -d)"
    local tmp="$TMP_DIR"

    curl -fsSL "$base/$asset" -o "$tmp/$asset" \
        || die "download failed: $base/$asset (is $version a published release?)"
    curl -fsSL "$base/$asset.sha256" -o "$tmp/$asset.sha256" \
        || die "could not fetch the checksum for $asset."
    verify_checksum "$tmp/$asset" "$tmp/$asset.sha256"
    info "checksum verified"

    case "$asset" in
        *.zip)
            need unzip
            unzip -qo "$tmp/$asset" -d "$tmp"
            ;;
        *)
            tar -xzf "$tmp/$asset" -C "$tmp"
            ;;
    esac

    local built="$tmp/$BINARY"
    [ -f "$built" ] || built="$tmp/$BINARY.exe"
    [ -f "$built" ] || die "the archive did not contain $BINARY."

    mkdir -p "$dest"
    # Install via a temp name and rename, so a running jinja-lsp is replaced
    # atomically rather than being truncated under its own feet.
    cp "$built" "$dest/.$BINARY.new"
    chmod +x "$dest/.$BINARY.new"
    mv -f "$dest/.$BINARY.new" "$dest/$(basename "$built")"

    info "installed to $dest/$(basename "$built")"
    "$dest/$(basename "$built")" --version >&2 || die "the installed binary did not run."

    case ":$PATH:" in
        *":$dest:"*) ;;
        *)
            warn "$dest is not on your PATH. Add this to your shell profile:"
            printf '\n    export PATH="%s:$PATH"\n\n' "$dest" >&2
            ;;
    esac
}

# Run unless being sourced for testing. Deliberately *not* the usual
# `[ "${BASH_SOURCE[0]}" = "$0" ]` guard: under `curl | bash` there is no source
# file, so that comparison is false and the installer would silently do nothing —
# exactly the invocation this script exists for.
[ -n "${JINJA_LSP_INSTALL_SH_LIB:-}" ] || main "$@"
