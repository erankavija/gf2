#!/usr/bin/env bash
# Fetch and build the pinned byte-field external baselines (jit:6c6b09b1).
#
# Usage:
#   ./fetch-build.sh [destination]
#   ./fetch-build.sh --print-pins             # report the pins below and stop
#   ./fetch-build.sh --verify-sources [dir]   # check the staged sources and stop
#
# Destination defaults to the primary checkout's .agents/ext/6c6b09b1, which
# the agent-local git excludes keep out of the working tree; nothing this
# script downloads or builds is committed. The pins below are the contract: a
# version plus sha256 for the tarball sources, and a branch or tag plus the
# commit it resolves to for the git sources. Every pin is re-verified on each
# run, so a mutated upstream fails the build instead of silently changing the
# measured baseline. `--verify-sources` performs only that verification, for
# `stage-externals.sh`, which copies an already built prefix into a worktree.
#
# Host discipline: every compile step runs under the shared side of the CCX1
# benchmark mutex so it can never overlap a timed run, with at most 12 jobs.
#
# Build flags mirror benchmarks/Containerfile (-O3 -march=native) so the
# reference binaries and the gf2 release build see the same host ISA.

set -euo pipefail

PRINT_PINS=0
VERIFY_ONLY=0
if [[ "${1:-}" == "--print-pins" ]]; then
    PRINT_PINS=1
    shift
elif [[ "${1:-}" == "--verify-sources" ]]; then
    VERIFY_ONLY=1
    shift
fi

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../../.." && pwd)"
# Externals stage beside the primary checkout, never inside a worktree that
# gets committed. `--git-common-dir` resolves to the primary `.git` directory
# from a linked worktree as well as from the primary checkout itself.
PRIMARY="$(cd "$(dirname "$(cd "${REPO}" && git rev-parse --git-common-dir)")" && pwd)"
EXT="${1:-${PRIMARY}/.agents/ext/6c6b09b1}"
PREFIX="${EXT}/prefix"
JOBS="${GF2_SURVEY_JOBS:-12}"
LOCK="${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}"
OPT="${GF2_SURVEY_OPT:--O3 -march=native}"

# ---------------------------------------------------------------------- pins
# NASM is a build-time dependency of ISA-L: the ISA-L autotools build assembles
# its x86_64 kernels with nasm and has no pure-C build path on this target.
# Its sha256 was observed when this survey first fetched it and is asserted on
# every later run.  License: BSD-2-Clause (nasm-2.16.03/LICENSE).
NASM_VERSION="2.16.03"
NASM_SHA256="1412a1c760bbd05db026b6c0d1657affd6631cd0a63cddb6f73cc6d4aa616148"

# M4RI is the GF(2) bit-matrix layer M4RIE builds on. The version and digest
# are the ones benchmarks/image.lock already pins for the reference image.
# License: GPL-2.0-or-later (m4ri-20260122/COPYING).
M4RI_VERSION="20260122"
M4RI_SHA256="7e033ca1fd36be8861e2f67d9d124c398fc0d830209bb0226462485876346404"

# M4RIE supplies the dense GF(2^e) matrix comparator (mzed_mul).  Version and
# digest match benchmarks/image.lock.  License: GPL-2.0-or-later
# (m4rie-20250128/COPYING).
M4RIE_VERSION="20250128"
M4RIE_SHA256="96f1adafd50e6a0b51dc3aa1cb56cb6c1361ae7c10d97dc35c3fa70822a55bd7"

# GF-Complete supplies region multiply-XOR over GF(2^8) with a selectable
# arithmetic backend.  The Ceph mirror publishes no tags, so the pin is the
# branch plus the commit it resolved to.  License: BSD-3-Clause
# (gf-complete/License.txt).
GFCOMPLETE_BRANCH="master"
GFCOMPLETE_COMMIT="a6862d10c9db467148f20eef2c6445ac9afd94d8"

# ISA-L supplies gf_vect_mul / gf_vect_mad / gf_vect_dot_prod / ec_encode_data
# over its compiled-in GF(2^8) polynomial 0x11D.  License: BSD-3-Clause
# (isa-l/LICENSE).
ISAL_TAG="v2.32.1"
ISAL_COMMIT="7c3479e0a9dac17f448603ec1ad64c7c625f530c"

# `--print-pins` reports the pins above and stops, so the survey's provenance
# record projects them from this script instead of restating them.
if [[ "${PRINT_PINS}" == "1" ]]; then
    echo "nasm version=${NASM_VERSION} sha256=${NASM_SHA256} license=nasm-src/LICENSE"
    echo "m4ri version=${M4RI_VERSION} sha256=${M4RI_SHA256} license=m4ri-src/COPYING"
    echo "m4rie version=${M4RIE_VERSION} sha256=${M4RIE_SHA256} license=m4rie-src/COPYING"
    echo "gf-complete branch=${GFCOMPLETE_BRANCH} commit=${GFCOMPLETE_COMMIT} license=gf-complete/License.txt"
    echo "isa-l tag=${ISAL_TAG} commit=${ISAL_COMMIT} license=isa-l/LICENSE"
    echo "opt ${OPT}"
    echo "staging ${EXT}"
    exit 0
fi

require_commit() {
    local dir="$1" want="$2" name="$3" got
    got="$(git -C "${dir}" rev-parse HEAD)"
    if [[ "${got}" != "${want}" ]]; then
        echo "${name}: expected ${want}, got ${got}" >&2
        exit 1
    fi
}

if [[ "${VERIFY_ONLY}" == "1" ]]; then
    for entry in "nasm-${NASM_VERSION}.tar.xz ${NASM_SHA256}" \
                 "m4ri-${M4RI_VERSION}.tar.gz ${M4RI_SHA256}" \
                 "m4rie-${M4RIE_VERSION}.tar.gz ${M4RIE_SHA256}"; do
        set -- ${entry}
        echo "$2  ${EXT}/$1" | sha256sum -c - >/dev/null
        echo "verified $1 sha256=$2"
    done
    require_commit "${EXT}/gf-complete" "${GFCOMPLETE_COMMIT}" gf-complete
    echo "verified gf-complete commit=${GFCOMPLETE_COMMIT}"
    require_commit "${EXT}/isa-l" "${ISAL_COMMIT}" isa-l
    echo "verified isa-l tag=${ISAL_TAG} commit=${ISAL_COMMIT}"
    exit 0
fi

mkdir -p "${EXT}" "${PREFIX}"
cd "${EXT}"

# Every compile step is serialized against timed runs by the shared side of the
# CCX1 mutex, taken through cargo-budget.sh. That wrapper is the only supported
# shared acquirer: it passes the turnstile that gives a queued measurement run
# writer preference, which a direct `flock -s` skips.
touch "${LOCK}"
shared() {
    "${REPO}/scripts/cargo-budget.sh" "$@"
}

fetch_tarball() {
    local file="$1" url="$2" digest="$3"
    [[ -f "${file}" ]] || curl -fsSL -o "${file}" "${url}"
    echo "${digest}  ${file}" | sha256sum -c - >/dev/null
}

# --------------------------------------------------------------------- nasm
if [[ ! -x "${PREFIX}/bin/nasm" ]]; then
    fetch_tarball "nasm-${NASM_VERSION}.tar.xz" \
        "https://www.nasm.us/pub/nasm/releasebuilds/${NASM_VERSION}/nasm-${NASM_VERSION}.tar.xz" \
        "${NASM_SHA256}"
    rm -rf nasm-src && mkdir -p nasm-src
    tar -C nasm-src --strip-components=1 -xf "nasm-${NASM_VERSION}.tar.xz"
    (
        cd nasm-src
        shared ./configure --prefix="${PREFIX}"
        shared make -j"${JOBS}"
        shared make install
    )
fi
export PATH="${PREFIX}/bin:${PATH}"

# --------------------------------------------------------------------- m4ri
if [[ ! -f "${PREFIX}/lib/libm4ri.so" ]]; then
    fetch_tarball "m4ri-${M4RI_VERSION}.tar.gz" \
        "https://github.com/malb/m4ri/releases/download/${M4RI_VERSION}/m4ri-${M4RI_VERSION}.tar.gz" \
        "${M4RI_SHA256}"
    rm -rf m4ri-src && mkdir -p m4ri-src
    tar -C m4ri-src --strip-components=1 -xf "m4ri-${M4RI_VERSION}.tar.gz"
    (
        cd m4ri-src
        shared env CFLAGS="${OPT} -fPIC" ./configure --prefix="${PREFIX}" --disable-static
        shared make -j"${JOBS}"
        shared make install
    )
fi

# -------------------------------------------------------------------- m4rie
if [[ ! -f "${PREFIX}/lib/libm4rie.so" ]]; then
    fetch_tarball "m4rie-${M4RIE_VERSION}.tar.gz" \
        "https://github.com/malb/m4rie/releases/download/${M4RIE_VERSION}/m4rie-${M4RIE_VERSION}.tar.gz" \
        "${M4RIE_SHA256}"
    rm -rf m4rie-src && mkdir -p m4rie-src
    tar -C m4rie-src --strip-components=1 -xf "m4rie-${M4RIE_VERSION}.tar.gz"
    (
        cd m4rie-src
        shared env CFLAGS="${OPT} -fPIC" CXXFLAGS="${OPT} -fPIC" \
            ./configure --prefix="${PREFIX}" --with-m4ri="${PREFIX}" --disable-static
        shared make -j"${JOBS}"
        shared make install
    )
fi

# -------------------------------------------------------------- gf-complete
if [[ ! -d gf-complete ]]; then
    git clone --branch "${GFCOMPLETE_BRANCH}" \
        https://github.com/ceph/gf-complete.git gf-complete
    git -C gf-complete checkout --detach "${GFCOMPLETE_COMMIT}"
fi
require_commit gf-complete "${GFCOMPLETE_COMMIT}" gf-complete
if [[ ! -f "${PREFIX}/lib/libgf_complete.so" ]]; then
    (
        cd gf-complete
        [[ -f configure ]] || shared ./autogen.sh
        shared env CFLAGS="${OPT} -fPIC" ./configure --prefix="${PREFIX}" --disable-static
        shared make -j"${JOBS}"
        shared make install
    )
fi

# -------------------------------------------------------------------- isa-l
if [[ ! -d isa-l ]]; then
    git clone --branch "${ISAL_TAG}" https://github.com/intel/isa-l.git isa-l
    git -C isa-l checkout --detach "${ISAL_COMMIT}"
fi
require_commit isa-l "${ISAL_COMMIT}" isa-l
if [[ ! -f "${PREFIX}/lib/libisal.so" ]]; then
    (
        cd isa-l
        [[ -f configure ]] || shared ./autogen.sh
        shared env CFLAGS="${OPT} -fPIC" ./configure --prefix="${PREFIX}" --disable-static
        shared make -j"${JOBS}"
        shared make install
    )
fi

# The survey harness (shim, conformance binaries and arms) is built by the
# campaign launcher's build step from a worktree copy of this prefix, which
# `stage-externals.sh` verifies against the committed digests.
echo "pinned libraries ready under ${PREFIX}"
