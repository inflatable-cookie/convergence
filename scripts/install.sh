#!/bin/sh
# Install Convergence from a published release (g02.022 batch 22.5).
#
#   curl -fsSL https://raw.githubusercontent.com/<owner>/convergence/main/scripts/install.sh | sh
#
# Deliberately POSIX sh and curl-or-wget only: the point of this script
# is the machine that has no Rust toolchain, and such a machine cannot be
# assumed to have bash either.
#
# It verifies the checksum before it installs anything. An installer that
# downloads over TLS and skips the hash is trusting exactly one thing;
# checking SHA256SUMS means a tampered artifact fails loudly rather than
# landing on the PATH.
set -eu

OWNER="${CONVERGE_REPO:-inflatable-cookie/convergence}"
VERSION="${CONVERGE_VERSION:-latest}"
PREFIX="${CONVERGE_PREFIX:-$HOME/.local/bin}"

say() { printf '%s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

fetch() {
    if command -v curl > /dev/null 2>&1; then
        curl -fsSL "$1" -o "$2"
    elif command -v wget > /dev/null 2>&1; then
        wget -qO "$2" "$1"
    else
        die "need curl or wget"
    fi
}

# Rust target triples, which is what the release archives are named for.
case "$(uname -s)" in
    Darwin) os="apple-darwin" ;;
    Linux)  os="unknown-linux-gnu" ;;
    *)      die "unsupported OS $(uname -s); build from source with cargo install --path crates/converge-cli" ;;
esac
case "$(uname -m)" in
    arm64|aarch64) arch="aarch64" ;;
    x86_64|amd64)  arch="x86_64" ;;
    *)             die "unsupported architecture $(uname -m)" ;;
esac
target="${arch}-${os}"

# CONVERGE_BASE_URL points this at a mirror, an internal artifact store,
# or a local directory served over HTTP -- which is also how this script
# gets tested without publishing a release to find out whether it works.
if [ -n "${CONVERGE_BASE_URL:-}" ]; then
    base="${CONVERGE_BASE_URL}"
elif [ "${VERSION}" = "latest" ]; then
    base="https://github.com/${OWNER}/releases/latest/download"
else
    base="https://github.com/${OWNER}/releases/download/${VERSION}"
fi

tmp="$(mktemp -d)"
# Leaving a half-downloaded archive in /tmp on failure helps nobody.
trap 'rm -rf "${tmp}"' EXIT INT TERM

say "Convergence: ${VERSION} for ${target}"

# The archive name embeds the version, which "latest" does not know. Ask
# SHA256SUMS, which lists exactly what this release shipped -- and which
# has to be downloaded anyway to verify against.
fetch "${base}/SHA256SUMS" "${tmp}/SHA256SUMS" \
    || die "no release found at ${base} (is ${OWNER} right? has a release been published?)"
archive="$(awk -v t="${target}" '$2 ~ t {print $2}' "${tmp}/SHA256SUMS" | head -n1)"
[ -n "${archive}" ] || die "this release has no build for ${target}"

say "downloading ${archive}"
fetch "${base}/${archive}" "${tmp}/${archive}" || die "download failed"

say "verifying checksum"
(
    cd "${tmp}"
    # Verify only our line: SHA256SUMS covers every platform and the
    # others are not present, which -c reports as a failure.
    grep " ${archive}\$" SHA256SUMS > expected.txt
    if command -v sha256sum > /dev/null 2>&1; then
        sha256sum -c expected.txt > /dev/null
    elif command -v shasum > /dev/null 2>&1; then
        shasum -a 256 -c expected.txt > /dev/null
    else
        die "need sha256sum or shasum to verify the download"
    fi
) || die "checksum mismatch -- do not use this download"

tar -xzf "${tmp}/${archive}" -C "${tmp}"
extracted="${tmp}/$(basename "${archive}" .tar.gz)"

mkdir -p "${PREFIX}"
for bin in converge converge-server converge-tui; do
    install -m 0755 "${extracted}/${bin}" "${PREFIX}/${bin}"
done

say "installed to ${PREFIX}:"
say "  converge  converge-server  converge-tui"

case ":${PATH}:" in
    *":${PREFIX}:"*) ;;
    *)
        say ""
        say "${PREFIX} is not on your PATH. Add it:"
        say "  export PATH=\"${PREFIX}:\$PATH\""
        ;;
esac

say ""
say "next: converge doctor"
