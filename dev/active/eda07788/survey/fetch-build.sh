#!/usr/bin/env bash
# Fetch and build the pinned DVB-T2 bit-interleaver arms (jit:eda07788).
#
# Usage:
#   dev/active/eda07788/survey/fetch-build.sh [staging-dir]
#
# The default staging directory is `.agents/ext/eda07788` under the repository
# root of the checkout this script lives in, which `.git/info/exclude` keeps
# out of every commit. Passing a directory overrides it.
#
# Two build flavours produce the three arm executables the family addenda
# declare. Each flavour compiles both the Rust side and the C++ shim at one
# architecture level, so an arm never mixes levels:
#
#   target-native/    RUSTFLAGS=-C target-cpu=native   g++ -march=native
#                     -> gf2-dvb-t2-candidate      (build identity `native`)
#                     -> xdsopl-dvb-t2-baseline    (build identity `external`)
#   target-portable/  RUSTFLAGS=-C target-cpu=x86-64   g++ -march=x86-64
#                     -> gf2-dvb-t2-candidate      (build identity
#                        `conservative-portable`, the scalar/compiler control)
#
# It also runs the correctness gate, because a build whose adapter does not
# reproduce gf2's permutation must not reach a timed cell. Builds and the gate
# finish before any timing run; nothing here may execute under the CCX1
# exclusive mutex.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
EXT="${1:-${REPO}/.agents/ext/eda07788}"

XDSOPL_COMMIT="32357d8ad55a6a302c34e093759f0454e45cca56"
XDSOPL_URL="https://github.com/xdsopl/LDPC.git"
MANIFEST="${REPO}/dev/active/eda07788/survey/gf2-side/Cargo.toml"

mkdir -p "${EXT}"
EXT="$(cd "${EXT}" && pwd)"

require_commit() {
    local dir="$1" want="$2" name="$3"
    local got
    got="$(git -C "${dir}" rev-parse HEAD)"
    if [[ "${got}" != "${want}" ]]; then
        echo "${name}: expected ${want}, got ${got}" >&2
        exit 1
    fi
    echo "${name}: pinned commit ${got} verified"
}

if [[ ! -d "${EXT}/xdsopl-ldpc" ]]; then
    git clone "${XDSOPL_URL}" "${EXT}/xdsopl-ldpc"
    git -C "${EXT}/xdsopl-ldpc" checkout --detach "${XDSOPL_COMMIT}"
fi
require_commit "${EXT}/xdsopl-ldpc" "${XDSOPL_COMMIT}" xdsopl-ldpc

export GF2_XDSOPL_INCLUDE="${EXT}/xdsopl-ldpc"

build_flavour() {
    local flavour="$1" rustflags="$2" cxx_arch="$3"
    echo "building ${flavour}: RUSTFLAGS=${rustflags} -march=${cxx_arch}"
    (
        cd "${REPO}"
        CARGO_TARGET_DIR="${EXT}/target-${flavour}" \
        RUSTFLAGS="${rustflags}" \
        GF2_XDSOPL_CXX_ARCH="${cxx_arch}" \
            ./scripts/cargo-budget.sh cargo build --release --manifest-path "${MANIFEST}"
    )
}

build_flavour native "-C target-cpu=native" native
build_flavour portable "-C target-cpu=x86-64" x86-64

# The correctness gate runs from the tuned flavour: the adapter it links is the
# one the external arm times.
"${EXT}/target-native/release/validate-permutation-equivalence"

echo
echo "arm executables and digests:"
for arm in \
    "target-native/release/gf2-dvb-t2-candidate" \
    "target-native/release/xdsopl-dvb-t2-baseline" \
    "target-portable/release/gf2-dvb-t2-candidate"; do
    printf '%s  %s\n' "$(sha256sum "${EXT}/${arm}" | cut -d' ' -f1)" "${arm}"
done
echo
echo "baselines ready under ${EXT}"
