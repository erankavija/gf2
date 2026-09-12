#!/usr/bin/env bash
# Verify the pinned comparator sources and build the NR encoder arms
# (jit:12fdeb5b).
#
# Usage:
#   dev/active/12fdeb5b/survey/nr-encode-build.sh <aff3ct-root> <srsran-root> [staging-dir]
#
# <aff3ct-root> is the AFF3CT v4.7.0 tree and <srsran-root> the srsRAN Project
# tree that `dev/active/c077a88b/survey/fetch-build.sh` clones. This script
# only reads them, and refuses a tree whose commit, submodule commit or
# static-library digest differs from the pins below, which are the ones
# `c077a88b`'s build identity records. Build output goes to the staging
# directory, by default `.agents/ext/12fdeb5b` inside the invoking checkout,
# which is git-ignored and rebuildable; a linked worktree therefore keeps its
# own build and never writes a sibling's.
#
# Two build flavours produce the four arm executables the family addenda
# declare. Each compiles the Rust side at one architecture level; the AFF3CT
# static library, the srsRAN translation units and both shims are
# `-march=native` builds:
#
#   target-nr-encode-native/    RUSTFLAGS=-C target-cpu=native,
#                               --features aff3ct,srsran
#                               -> gf2-nr-encode-arm     (identity `native`)
#                               -> aff3ct-nr-encode-arm  (identity `external`)
#                               -> srsran-nr-encode-arm  (identity `external`)
#                               -> validate-nr-encode-equivalence
#                               -> derive-nr-encode-parameters
#   target-nr-encode-portable/  RUSTFLAGS=-C target-cpu=x86-64
#                               -> gf2-nr-encode-arm (identity
#                                  `conservative-portable`, the scalar control)
#
# The equivalence gate then runs from the native flavour and writes the
# validation record, because a configuration whose adapter does not reproduce
# gf2's codeword must not reach a timed cell. Nothing here may run under the
# CCX1 exclusive mutex.

set -euo pipefail

# The arms are release executables the campaign digests; pin the MSRV
# toolchain here and in the launcher so both build the same bytes.
export PATH="${HOME}/.cargo/bin:${PATH}" RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-1.95}"

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
AFF3CT_ROOT="$(cd "${1:?usage: nr-encode-build.sh <aff3ct-root> <srsran-root> [staging-dir]}" && pwd)"
SRSRAN_ROOT="$(cd "${2:?usage: nr-encode-build.sh <aff3ct-root> <srsran-root> [staging-dir]}" && pwd)"
EXT="${3:-${REPO}/.agents/ext/12fdeb5b}"

AFF3CT_COMMIT="e8a65c5047262d97a15563b9edc961f69b2792cc"
AFF3CT_CONF_COMMIT="ecae10cd0375f12febe2f5513f8ca39f2540c640"
AFF3CT_STATIC_SHA256="b9605974b1d1413524e3a393a79c0842594683fdc9e240eee4f72a287cb92529"
SRSRAN_COMMIT="d2f4b70dda8e2c557d5b05a0ac5f92dbddda19bc"
SURVEY="${REPO}/dev/active/12fdeb5b/survey"
MANIFEST="${SURVEY}/nr-encode/Cargo.toml"

# The srsRAN translation units the shim compiles; `nr-encode/build.rs` names
# the same list.
SRSRAN_SOURCES=(
    "lib/phy/upper/channel_coding/ldpc/ldpc_encoder_impl.cpp"
    "lib/phy/upper/channel_coding/ldpc/ldpc_encoder_generic.cpp"
    "lib/phy/upper/channel_coding/ldpc/ldpc_encoder_avx2.cpp"
    "lib/phy/upper/channel_coding/ldpc/ldpc_graph_impl.cpp"
    "lib/phy/upper/channel_coding/ldpc/ldpc_luts_impl.cpp"
    "lib/phy/upper/channel_coding/ldpc/ldpc_rate_matcher_impl.cpp"
    "lib/srsvec/bit.cpp"
)

check_commit() {
    local label="$1" root="$2" expected="$3" got
    got="$(git -C "${root}" rev-parse HEAD)"
    if [[ "${got}" != "${expected}" ]]; then
        echo "${label}: expected ${expected}, got ${got}" >&2
        exit 1
    fi
    echo "${label}: pinned commit ${got} verified"
}

check_commit aff3ct "${AFF3CT_ROOT}" "${AFF3CT_COMMIT}"
check_commit aff3ct-conf "${AFF3CT_ROOT}/conf" "${AFF3CT_CONF_COMMIT}"
check_commit srsran "${SRSRAN_ROOT}" "${SRSRAN_COMMIT}"

static="$(sha256sum "${AFF3CT_ROOT}/build/lib/libaff3ct-4.7.0.a" | cut -d' ' -f1)"
if [[ "${static}" != "${AFF3CT_STATIC_SHA256}" ]]; then
    echo "aff3ct: static library ${static} differs from ${AFF3CT_STATIC_SHA256}" >&2
    exit 1
fi
echo "aff3ct: static library sha256 ${static} verified"

# srsRAN's own CMake configuration stops on a host without MbedTLS, so the
# shim compiles the LDPC translation units directly. Record whether this host
# provides the package, and digest every compiled unit.
if pkg-config --exists mbedtls; then
    echo "srsran: pkg-config module mbedtls present; the full CMake configuration is possible"
else
    echo "srsran: pkg-config module mbedtls missing; the shim compiles the LDPC units directly"
fi
echo "srsran: compiled translation units:"
for source in "${SRSRAN_SOURCES[@]}"; do
    printf '  %s  %s\n' "$(sha256sum "${SRSRAN_ROOT}/${source}" | cut -d' ' -f1)" "${source}"
done

mkdir -p "${EXT}"
EXT="$(cd "${EXT}" && pwd)"

build_flavour() {
    local flavour="$1" rustflags="$2"
    shift 2
    echo "building ${flavour}: RUSTFLAGS=${rustflags} $*"
    (
        cd "${REPO}"
        CARGO_TARGET_DIR="${EXT}/target-nr-encode-${flavour}" \
        RUSTFLAGS="${rustflags}" \
        GF2_AFF3CT_ROOT="${AFF3CT_ROOT}" \
        GF2_SRSRAN_ROOT="${SRSRAN_ROOT}" \
            ./scripts/cargo-budget.sh cargo build --release --locked \
            --manifest-path "${MANIFEST}" "$@"
    )
}

build_flavour native "-C target-cpu=native" --features aff3ct,srsran
build_flavour portable "-C target-cpu=x86-64" --bin gf2-nr-encode-arm

"${EXT}/target-nr-encode-native/release/derive-nr-encode-parameters" \
    >"${SURVEY}/nr-encode-parameters.json"
"${EXT}/target-nr-encode-native/release/validate-nr-encode-equivalence" \
    "${AFF3CT_ROOT}/conf" "${SURVEY}/nr-encode-validation.json" \
    | tee "${SURVEY}/nr-encode-validation.txt"

python3 "${SURVEY}/inspect-sources.py" "${AFF3CT_ROOT}" "${SRSRAN_ROOT}" \
    "${SURVEY}/source-evidence.json"
python3 "${SURVEY}/record-build-evidence.py" "${AFF3CT_ROOT}" "${SRSRAN_ROOT}" "${EXT}" \
    "${SURVEY}/build-evidence.json"

echo
echo "arm executables and digests:"
for arm in \
    "target-nr-encode-native/release/gf2-nr-encode-arm" \
    "target-nr-encode-native/release/aff3ct-nr-encode-arm" \
    "target-nr-encode-native/release/srsran-nr-encode-arm" \
    "target-nr-encode-portable/release/gf2-nr-encode-arm"; do
    printf '%s  %s\n' "$(sha256sum "${EXT}/${arm}" | cut -d' ' -f1)" "${arm}"
done
echo
echo "NR rate-matched encoder arms ready under ${EXT}"
