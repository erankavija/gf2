#!/usr/bin/env bash
# Fetch and build the pinned external baselines of the survey (jit:4e732b56).
#
# Usage:
#   ./fetch-build.sh [destination]
#
# Destination defaults to <repo>/.agents/ext, which the agent-local git
# excludes keep out of the working tree; nothing this script downloads or
# builds is committed. The pins below are the contract: a tag plus the commit
# it resolves to for the git sources, and a version plus sha256 for the M4RI
# tarball, matching the pin `benchmarks/image.lock` already carries.
#
# Build flags mirror benchmarks/Containerfile (-O3 -march=native). The host
# compiler is not the container's gcc-12, so the run's host record captures the
# compiler actually used.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../../.." && pwd)"
EXT="${1:-${REPO}/.agents/ext}"

# ---------------------------------------------------------------------- pins
AFF3CT_TAG="v4.7.0"
AFF3CT_COMMIT="e8a65c5047262d97a15563b9edc961f69b2792cc"
BCHLIB_TAG="v2.1.3"
BCHLIB_COMMIT="8d0656ab8f37e734428635501738d360ad80eebd"
M4RI_VERSION="20260122"
M4RI_SHA256="7e033ca1fd36be8861e2f67d9d124c398fc0d830209bb0226462485876346404"
# Recorded so a candidate that this survey rejected can be re-checked at the
# same version rather than at whatever upstream has become.
LIQUID_TAG="v1.8.2"
LIQUID_COMMIT="03d052b89b543e6d5f9a882139ba19f7683bcd29"

mkdir -p "${EXT}"
cd "${EXT}"

require_commit() {
    local dir="$1" want="$2" name="$3"
    local got
    got="$(git -C "${dir}" rev-parse HEAD)"
    if [[ "${got}" != "${want}" ]]; then
        echo "${name}: expected ${want}, got ${got}" >&2
        exit 1
    fi
}

# ------------------------------------------------------------------- aff3ct
if [[ ! -d aff3ct ]]; then
    git clone --recurse-submodules --depth 1 --branch "${AFF3CT_TAG}" https://github.com/aff3ct/aff3ct.git aff3ct
fi
require_commit aff3ct "${AFF3CT_COMMIT}" aff3ct

if [[ ! -f aff3ct/build/lib/libaff3ct-4.7.0.a ]]; then
    mkdir -p aff3ct/build
    (
        cd aff3ct/build
        cmake .. \
            -DCMAKE_BUILD_TYPE=Release \
            -DAFF3CT_COMPILE_EXE=OFF \
            -DAFF3CT_COMPILE_STATIC_LIB=ON \
            -DAFF3CT_COMPILE_SHARED_LIB=OFF \
            -DAFF3CT_INCLUDE_SPU_LIB=ON \
            -DCMAKE_CXX_FLAGS="-O3 -march=native -funroll-loops"
        make -j"$(nproc)"
    )
fi

# ------------------------------------------------------------------- bchlib
# The userspace packaging of the Linux kernel's lib/bch.c (Ivan Djelic,
# Parrot S.A., GPL-2.0). Only src/bch.c is used; the Python extension is not
# built. The harness compiles that source directly, so there is no build step
# here beyond the checkout.
if [[ ! -d bchlib ]]; then
    git clone --depth 1 --branch "${BCHLIB_TAG}" https://github.com/jkent/python-bchlib.git bchlib
fi
require_commit bchlib "${BCHLIB_COMMIT}" bchlib

# --------------------------------------------------------------------- m4ri
if [[ ! -f "prefix/lib/libm4ri.so" ]]; then
    tarball="m4ri-${M4RI_VERSION}.tar.gz"
    [[ -f "${tarball}" ]] || curl -fsSL -o "${tarball}" \
        "https://github.com/malb/m4ri/releases/download/${M4RI_VERSION}/${tarball}"
    echo "${M4RI_SHA256}  ${tarball}" | sha256sum -c -
    mkdir -p m4ri-src prefix
    tar -C m4ri-src --strip-components=1 -xf "${tarball}"
    (
        cd m4ri-src
        CFLAGS="-O3 -march=native -fPIC" ./configure --prefix="${EXT}/prefix" --disable-static
        make -j"$(nproc)"
        make install
    )
fi

# ------------------------------------------------------------------- liquid
# Fetched only so the survey's "no BCH encoder" finding is checkable against
# the exact version it was made at. Not built: nothing in it is measured.
if [[ "${GF2_SURVEY_FETCH_LIQUID:-0}" == "1" ]]; then
    if [[ ! -d liquid-dsp ]]; then
        git clone --depth 1 --branch "${LIQUID_TAG}" https://github.com/jgaeddert/liquid-dsp.git liquid-dsp
    fi
    require_commit liquid-dsp "${LIQUID_COMMIT}" liquid-dsp
fi

# ------------------------------------------------------------------ harness
make -C "${HERE}" EXT="${EXT}"
CARGO_TARGET_DIR="${EXT}/survey-target" cargo build --release \
    --manifest-path "${HERE}/gf2-side/Cargo.toml"

echo "baselines ready under ${EXT}"
