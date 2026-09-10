#!/usr/bin/env bash
# Verify the pinned AFF3CT build and build the NR de-rate-matching arms
# (jit:eda07788).
#
# Usage:
#   dev/active/eda07788/survey/nr-derate-build.sh <aff3ct-root> [staging-dir]
#
# <aff3ct-root> is the AFF3CT v4.7.0 tree that
# `dev/active/c077a88b/survey/fetch-build.sh` clones and builds. This script
# only reads it, and refuses a tree whose commit or static-library digest
# differs from the pins below, which are the ones `c077a88b`'s build identity
# records. Build output goes to the staging directory, by default
# `.agents/ext/eda07788` under this checkout.
#
# Two build flavours produce the three arm executables the family addenda
# declare. Each compiles the Rust side at one architecture level; the AFF3CT
# library and its shim are `-march=native` builds:
#
#   target-nr-native/    RUSTFLAGS=-C target-cpu=native, --features aff3ct
#                        -> gf2-nr-derate-arm     (build identity `native`)
#                        -> aff3ct-nr-derate-arm  (build identity `external`)
#                        -> validate-nr-derate-equivalence
#   target-nr-portable/  RUSTFLAGS=-C target-cpu=x86-64
#                        -> gf2-nr-derate-arm     (build identity
#                           `conservative-portable`, the scalar/compiler control)
#
# The equivalence gate then runs from the native flavour, because a
# configuration whose adapter does not reproduce gf2's output must not reach a
# timed cell. Nothing here may run under the CCX1 exclusive mutex.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
AFF3CT_ROOT="$(cd "${1:?usage: nr-derate-build.sh <aff3ct-root> [staging-dir]}" && pwd)"
EXT="${2:-${REPO}/.agents/ext/eda07788}"

AFF3CT_COMMIT="e8a65c5047262d97a15563b9edc961f69b2792cc"
AFF3CT_STATIC_SHA256="b9605974b1d1413524e3a393a79c0842594683fdc9e240eee4f72a287cb92529"
MANIFEST="${REPO}/dev/active/eda07788/survey/nr-derate/Cargo.toml"

got="$(git -C "${AFF3CT_ROOT}" rev-parse HEAD)"
if [[ "${got}" != "${AFF3CT_COMMIT}" ]]; then
    echo "aff3ct: expected ${AFF3CT_COMMIT}, got ${got}" >&2
    exit 1
fi
echo "aff3ct: pinned commit ${got} verified"
static="$(sha256sum "${AFF3CT_ROOT}/build/lib/libaff3ct-4.7.0.a" | cut -d' ' -f1)"
if [[ "${static}" != "${AFF3CT_STATIC_SHA256}" ]]; then
    echo "aff3ct: static library ${static} differs from ${AFF3CT_STATIC_SHA256}" >&2
    exit 1
fi
echo "aff3ct: static library sha256 ${static} verified"

mkdir -p "${EXT}"
EXT="$(cd "${EXT}" && pwd)"

build_flavour() {
    local flavour="$1" rustflags="$2"
    shift 2
    echo "building ${flavour}: RUSTFLAGS=${rustflags} $*"
    (
        cd "${REPO}"
        CARGO_TARGET_DIR="${EXT}/target-nr-${flavour}" \
        RUSTFLAGS="${rustflags}" \
        GF2_AFF3CT_ROOT="${AFF3CT_ROOT}" \
            ./scripts/cargo-budget.sh cargo build --release --locked \
            --manifest-path "${MANIFEST}" "$@"
    )
}

build_flavour native "-C target-cpu=native" --features aff3ct
build_flavour portable "-C target-cpu=x86-64" --bin gf2-nr-derate-arm

"${EXT}/target-nr-native/release/validate-nr-derate-equivalence"

echo
echo "arm executables and digests:"
for arm in \
    "target-nr-native/release/gf2-nr-derate-arm" \
    "target-nr-native/release/aff3ct-nr-derate-arm" \
    "target-nr-portable/release/gf2-nr-derate-arm"; do
    printf '%s  %s\n' "$(sha256sum "${EXT}/${arm}" | cut -d' ' -f1)" "${arm}"
done
echo
echo "NR de-rate-matching arms ready under ${EXT}"
