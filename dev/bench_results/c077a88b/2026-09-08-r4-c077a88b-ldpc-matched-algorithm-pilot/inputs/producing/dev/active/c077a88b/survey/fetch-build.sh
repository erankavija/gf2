#!/usr/bin/env bash
# Fetch and build the pinned external LDPC decoder baselines (jit:c077a88b).
#
# Usage:
#   ./fetch-build.sh [destination]
#
# Destination defaults to <repo>/.agents/ext/c077a88b, which the agent-local
# git excludes keep out of the working tree; nothing this script downloads or
# builds is committed. The pins below are the contract: a tag plus the commit
# it resolves to for every git source, together with its license.
#
# Every compilation step runs under the shared side of the canonical CCX1
# benchmark mutex (flock -s /tmp/gf2-ccx1.lock) with at most 12 jobs, so a
# build can never overlap a timed run holding the exclusive side.
#
# Build flags mirror the survey convention of benchmarks/Containerfile
# (-O3 -march=native); the run's host record captures the compiler actually
# used.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../../.." && pwd)"
EXT="${1:-${REPO}/.agents/ext/c077a88b}"
LOCK="${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}"
JOBS=12
CMAKE="${GF2_CMAKE:-/usr/bin/cmake}"

# ---------------------------------------------------------------------- pins
# AFF3CT: MIT. v4.7.0 is both the newest tag and the current develop head as
# resolved at pin time, so the pin is the maintained upstream, not a stale tag.
AFF3CT_TAG="v4.7.0"
AFF3CT_COMMIT="e8a65c5047262d97a15563b9edc961f69b2792cc"
# srsRAN Project: AGPL-3.0. The GitHub repository is archived; release_25_10 is
# its final release tag and the pin therefore names a frozen upstream.
SRSRAN_TAG="release_25_10"
SRSRAN_COMMIT="d2f4b70dda8e2c557d5b05a0ac5f92dbddda19bc"
# xdsopl/LDPC: 0BSD. Header-only, no tags; the pin is the master head commit.
XDSOPL_COMMIT="32357d8ad55a6a302c34e093759f0454e45cca56"
# OpenAirInterface 5G: Collaborative Standards Software License v1.0. Weekly tags; 2026.w36 is the
# newest at pin time.
OAI_TAG="2026.w36"
OAI_COMMIT="b3930e39f53e626b821c6bb84c22c841811c4536"

mkdir -p "${EXT}"
cd "${EXT}"

require_commit() {
    local dir="$1" want="$2" name="$3" got
    got="$(git -C "${dir}" rev-parse HEAD)"
    if [[ "${got}" != "${want}" ]]; then
        echo "${name}: expected ${want}, got ${got}" >&2
        exit 1
    fi
}

# ------------------------------------------------------------------- aff3ct
if [[ ! -d aff3ct ]]; then
    git clone --recurse-submodules --depth 1 --branch "${AFF3CT_TAG}" \
        https://github.com/aff3ct/aff3ct.git aff3ct
fi
require_commit aff3ct "${AFF3CT_COMMIT}" aff3ct

if [[ ! -f aff3ct/build/lib/libaff3ct-4.7.0.a ]]; then
    mkdir -p aff3ct/build
    (
        cd aff3ct/build
        flock -s "${LOCK}" "${CMAKE}" .. \
            -DCMAKE_BUILD_TYPE=Release \
            -DAFF3CT_COMPILE_EXE=ON \
            -DAFF3CT_COMPILE_STATIC_LIB=ON \
            -DAFF3CT_COMPILE_SHARED_LIB=OFF \
            -DAFF3CT_INCLUDE_SPU_LIB=ON \
            -DCMAKE_CXX_FLAGS="-O3 -march=native -funroll-loops"
        flock -s "${LOCK}" make -j"${JOBS}"
    )
fi

# ------------------------------------------------------------------- srsran
# Only the LDPC channel-coding static library and its srsvec/srslog
# dependencies are built; the radio stack is not needed and is not compiled.
if [[ ! -d srsran ]]; then
    git clone --depth 1 --branch "${SRSRAN_TAG}" \
        https://github.com/srsran/srsRAN_Project.git srsran
fi
require_commit srsran "${SRSRAN_COMMIT}" srsran

if [[ ! -f srsran/build/lib/phy/upper/channel_coding/ldpc/libsrsran_ldpc.a ]]; then
    mkdir -p srsran/build
    (
        cd srsran/build
        flock -s "${LOCK}" "${CMAKE}" .. \
            -DCMAKE_BUILD_TYPE=Release \
            -DENABLE_EXPORT=OFF \
            -DENABLE_UHD=OFF \
            -DENABLE_ZEROMQ=OFF \
            -DENABLE_DPDK=OFF \
            -DENABLE_TRX_DRIVER=OFF \
            -DBUILD_TESTS=OFF \
            -DMARCH=native \
            -DAUTO_DETECT_ISA=ON
        flock -s "${LOCK}" make -j"${JOBS}" srsran_ldpc srsvec srslog srsran_support
    )
fi

# ------------------------------------------------------------------- xdsopl
# Header-only C++; the survey harness compiles the headers directly, so the
# checkout is the whole build step.
if [[ ! -d xdsopl-ldpc ]]; then
    git clone https://github.com/xdsopl/LDPC.git xdsopl-ldpc
fi
git -C xdsopl-ldpc checkout --quiet "${XDSOPL_COMMIT}"
require_commit xdsopl-ldpc "${XDSOPL_COMMIT}" xdsopl-ldpc

# ---------------------------------------------------------------------- oai
# Fetched for source, license, buildability and backend inspection. The
# LDPC-only build target is attempted separately by inspect-oai.sh, which
# records what the attempt produced.
if [[ ! -d oai ]]; then
    git clone --depth 1 --branch "${OAI_TAG}" \
        https://gitlab.eurecom.fr/oai/openairinterface5g.git oai
fi
require_commit oai "${OAI_COMMIT}" oai

# ------------------------------------------------------------------ harness
GF2_AFF3CT_ROOT="${EXT}/aff3ct" CARGO_TARGET_DIR="${EXT}/survey-target" \
    RUSTFLAGS="-C target-cpu=native" \
    "${REPO}/scripts/cargo-budget.sh" cargo build --release --features aff3ct \
    --manifest-path "${HERE}/harness/Cargo.toml"

echo "LDPC decoder baselines ready under ${EXT}"
