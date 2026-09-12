#!/usr/bin/env bash
# Fetch and build the pinned C/C++ external comparison arms for jit:6fb89a3c.
#
# All generated trees live below .agents/ext/6fb89a3c, which is git-excluded.
# M4RI is the GPL-2.0-or-later release tarball (COPYING is the GPLv2 text;
# the source headers say "version 2 or higher") pinned by version and SHA-256. Bitshuffle
# is the MIT tag pinned by commit and is reduced to its two core C files. ISA-L
# is the BSD-3-Clause tag pinned by commit. Every build action holds the shared
# benchmark lock, allowing sibling builds while excluding exclusive timed runs.
#
# ISA-L is reduced to its portable C reference `raid/raid_base.c` because the
# NASM-built multi-binary dispatcher cannot be assembled on this host; see the
# note beside that build step. Every external object uses -O3 -march=native.
#
# The staged trees and prebuilt libraries are reused when present; every
# reuse re-checks the pinned commits, and build-evidence.json records the
# library digests the harness binaries were linked against.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Resolve the active checkout, so a linked-worktree run stages only inside
# that worktree and a merged run stages below the normal repository root.
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
EXT="${1:-${REPO}/.agents/ext/6fb89a3c}"
PREFIX="${EXT}/prefix"

M4RI_VERSION=20260122
M4RI_SHA256=7e033ca1fd36be8861e2f67d9d124c398fc0d830209bb0226462485876346404
BITSHUFFLE_TAG=0.5.2
BITSHUFFLE_COMMIT=52aec3b80d05606c090956aecfe868489d96b95c
ISAL_TAG=v2.32.1
ISAL_COMMIT=7c3479e0a9dac17f448603ec1ad64c7c625f530c

mkdir -p "${EXT}" "${PREFIX}/lib"
cd "${EXT}"

require_commit() {
    local dir="$1" want="$2" name="$3" got
    got="$(git -C "${dir}" rev-parse HEAD)"
    if [[ "${got}" != "${want}" ]]; then
        echo "${name}: expected ${want}, got ${got}" >&2
        exit 1
    fi
}

if [[ ! -f "${PREFIX}/lib/libm4ri.so" ]]; then
    tarball="m4ri-${M4RI_VERSION}.tar.gz"
    [[ -f "${tarball}" ]] || curl -fsSL -o "${tarball}" \
        "https://github.com/malb/m4ri/releases/download/${M4RI_VERSION}/${tarball}"
    echo "${M4RI_SHA256}  ${tarball}" | sha256sum -c -
    mkdir -p m4ri-src "${PREFIX}"
    tar -C m4ri-src --strip-components=1 -xf "${tarball}"
    (
        cd m4ri-src
        CARGO_CI_NO_SCCACHE=1 "${REPO}/scripts/cargo-budget.sh" \
            bash -c 'CFLAGS="-O3 -march=native -fPIC" ./configure --prefix="$1" --disable-static && make -j"${CARGO_BUILD_JOBS:-1}" && make install' _ "${PREFIX}"
    )
fi

if [[ ! -f "${PREFIX}/lib/libbitshuffle_min.a" ]]; then
    if [[ ! -d bitshuffle ]]; then
        git clone --depth 1 --branch "${BITSHUFFLE_TAG}" \
            https://github.com/kiyo-masui/bitshuffle.git bitshuffle
    fi
    require_commit bitshuffle "${BITSHUFFLE_COMMIT}" bitshuffle
    mkdir -p bitshuffle-build
    # bitshuffle.c (the compression-filter wrapper) needs lz4.h, which this
    # fetch does not pin since no compression filter is used here; the public
    # bshuf_bitshuffle/bshuf_bitunshuffle entry points this harness calls live
    # entirely in bitshuffle_core.c, which needs only iochain.c (its ioc_*
    # chain-iterator helpers) alongside it -- neither has an lz4 dependency.
    CARGO_CI_NO_SCCACHE=1 "${REPO}/scripts/cargo-budget.sh" bash -c '
        set -euo pipefail
        gcc -O3 -march=native -fPIC -Wall -Wextra -c -I bitshuffle/src bitshuffle/src/bitshuffle_core.c -o bitshuffle-build/bitshuffle_core.o
        gcc -O3 -march=native -fPIC -Wall -Wextra -c -I bitshuffle/src bitshuffle/src/iochain.c -o bitshuffle-build/iochain.o
        ar rcs "$1" bitshuffle-build/bitshuffle_core.o bitshuffle-build/iochain.o
    ' _ "${PREFIX}/lib/libbitshuffle_min.a"
else
    [[ ! -d bitshuffle ]] || require_commit bitshuffle "${BITSHUFFLE_COMMIT}" bitshuffle
fi

if [[ ! -f "${PREFIX}/lib/libisal_base.a" ]]; then
    if [[ ! -d isa-l ]]; then
        git clone --depth 1 --branch "${ISAL_TAG}" https://github.com/intel/isa-l.git isa-l
    fi
    require_commit isa-l "${ISAL_COMMIT}" isa-l
    # The full ISA-L build (`./autogen.sh && ./configure && make`) compiles
    # raid_multibinary.asm and the AVX/AVX512 xor_gen kernels with NASM
    # (>= 2.14.01), which is not installed on this host and cannot be
    # installed without root. `xor_gen`'s public multi-binary dispatcher is
    # therefore unavailable here. `raid/raid_base.c` carries ISA-L's own
    # portable C reference implementation, `xor_gen_base`, with the exact
    # same (vects, len, array) contract `include/raid.h` documents for
    # `xor_gen` (array[vects-1] is the fresh XOR of array[0..vects-2];
    # source and destination pointers aligned to 32 bytes). This harness
    # measures that portable reference build, compiled with the same
    # -O3 -march=native flags, not the NASM-dispatched one -- recorded as a
    # build-environment limitation in findings.md and build-evidence.json,
    # not silently substituted for a different operation.
    mkdir -p "${PREFIX}/lib"
    CARGO_CI_NO_SCCACHE=1 "${REPO}/scripts/cargo-budget.sh" bash -c '
        set -euo pipefail
        gcc -O3 -march=native -fPIC -Wall -Wextra -c isa-l/raid/raid_base.c -o isa-l/raid_base.o
        ar rcs "$1" isa-l/raid_base.o
    ' _ "${PREFIX}/lib/libisal_base.a"
else
    [[ ! -d isa-l ]] || require_commit isa-l "${ISAL_COMMIT}" isa-l
fi

require_commit bitshuffle "${BITSHUFFLE_COMMIT}" bitshuffle
require_commit isa-l "${ISAL_COMMIT}" isa-l
CARGO_CI_NO_SCCACHE=1 "${REPO}/scripts/cargo-budget.sh" \
    make -C "${HERE}" EXT="${EXT}"
# The gf2 arms are the conservative-portable build identity: the x86-64
# baseline target with the production runtime dispatch deciding the kernel.
# RUSTFLAGS is set explicitly so the recorded arm identity is a build fact.
CARGO_CI_NO_SCCACHE=1 CARGO_TARGET_DIR="${HERE}/gf2-side/target" \
    RUSTFLAGS="${GF2_SURVEY_RUSTFLAGS:--C target-cpu=x86-64}" \
    "${REPO}/scripts/cargo-budget.sh" cargo +1.95.0 build --release \
    --manifest-path "${HERE}/gf2-side/Cargo.toml"
echo "baselines and harnesses ready under ${EXT}"
