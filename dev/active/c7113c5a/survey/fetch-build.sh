#!/usr/bin/env bash
# Fetch and build the pinned gf2x external baseline (jit:c7113c5a).
#
# Usage:
#   ./fetch-build.sh [staging-directory]
#
# Staging defaults to <repo>/.agents/ext/c7113c5a, which the agent-local git
# excludes keep out of the working tree; nothing this script downloads or
# builds is committed. The pins below are the contract: the upstream tag plus
# the commit it resolves to.
#
# gf2x is GPL-3.0-or-later in this configuration: the archive carries a real
# `toom-gpl.c` (61146 bytes, not the 1384-byte placeholder), which its README
# names as the condition selecting GPL-3.0-or-later over LGPL-2.1-or-later.
# The library is measured as an out-of-tree comparator only. Nothing here
# becomes a gf2 production dependency and no gf2 multiplication algorithm
# changes.
#
# Three build variants give gf2x the same host-targeting ladder the gf2 arms
# get, so neither side is measured under a handicap:
#
#   conservative  CFLAGS="-O2"                    portable baseline
#   tuned         CFLAGS="-O3 -march=x86-64-v3"   portable ISA level
#   native        CFLAGS="-O3 -march=native"      host-specific
#
# gf2x's own configure appends -msse2 -msse3 -mssse3 -msse4.1 -mpclmul to
# CFLAGS whenever the compiler accepts them (config/acinclude.m4), so every
# variant compiles the PCLMUL basecases. `record-build-evidence.sh` extracts
# the selected hardware directory and the instructions actually emitted.
#
# Every compile runs under the shared side of the canonical CCX1 benchmark
# mutex, taken through `scripts/cargo-budget.sh`, so it can never overlap a
# timed run on this host and never overtakes one that is queued.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
ISSUE=c7113c5a
EXT="${1:-${GF2_SURVEY_EXT:-${REPO}/.agents/ext/${ISSUE}}}"
JOBS=12

# ---------------------------------------------------------------------- pins
GF2X_URL="https://gitlab.inria.fr/gf2x/gf2x.git"
GF2X_TAG="gf2x-1.3.0"
GF2X_COMMIT="27ba588f03bf6e1e74763903bab25e6e8bb6d0f0"
GF2X_LICENSE="GPL-3.0-or-later"

# Every compile takes the shared side of the CCX1 mutex through
# `scripts/cargo-budget.sh`, which is the only supported shared acquirer: it
# passes through the turnstile in `dev/scripts/ccx1-bench-flock.sh`, so a queued
# `--full-host` measurement run is not overtaken by a stream of builds. A bare
# `flock -s` here would skip that turnstile, and wrapping cargo-budget.sh in one
# would take the shared side twice for the same work.
shared() { "${REPO}/scripts/cargo-budget.sh" "$@"; }

mkdir -p "${EXT}"
cd "${EXT}"

# -------------------------------------------------------------------- source
if [[ ! -d gf2x ]]; then
    git clone --quiet "${GF2X_URL}" gf2x
fi
git -C gf2x checkout --quiet "${GF2X_COMMIT}"
got="$(git -C gf2x rev-parse HEAD)"
if [[ "${got}" != "${GF2X_COMMIT}" ]]; then
    echo "gf2x: expected ${GF2X_COMMIT}, got ${got}" >&2
    exit 1
fi
# The GPL condition the README states, asserted rather than assumed.
if [[ "$(stat -c %s gf2x/toom-gpl.c)" -lt 10000 ]]; then
    echo "gf2x: toom-gpl.c is a placeholder; the license is not ${GF2X_LICENSE}" >&2
    exit 1
fi

if [[ ! -x gf2x/configure ]]; then
    (cd gf2x && shared autoreconf -i >/dev/null 2>&1)
fi

# -------------------------------------------------------------------- builds
build_variant() {
    local name="$1" cflags="$2"
    local dir="${EXT}/build-${name}"
    local prefix="${EXT}/prefix-${name}"
    if [[ -f "${prefix}/lib/libgf2x.so" ]]; then
        return 0
    fi
    mkdir -p "${dir}"
    (
        cd "${dir}"
        CFLAGS="${cflags}" shared "${EXT}/gf2x/configure" \
            --prefix="${prefix}" --disable-static --enable-shared \
            >configure.stdout.log 2>&1
        shared make -j"${JOBS}" >make.stdout.log 2>&1
        shared make install >install.stdout.log 2>&1
    )
}

build_variant conservative "-O2"
build_variant tuned        "-O3 -march=x86-64-v3"
build_variant native       "-O3 -march=native"

# ------------------------------------------------------------------- gf2 side
# The arm binaries are one standalone cargo project outside the gf2 workspace,
# consuming the production crates by path exactly as an external user would and
# reusing `tuning-campaign-support` for the canonical child-v2 framing. Each
# build variant gets its own target directory so its executable digest is
# distinct in the receipt.
build_arms() {
    local name="$1" rustflags="$2" cflags="$3"
    CARGO_TARGET_DIR="${EXT}/arms-${name}" \
    RUSTFLAGS="${rustflags}" \
    GF2X_PREFIX="${EXT}/prefix-${name}" \
    GF2X_CFLAGS="${cflags}" \
        shared cargo build --release \
            --manifest-path "${HERE}/gf2-side/Cargo.toml" \
            >"${EXT}/arms-${name}.build.log" 2>&1
}

build_arms conservative ""                      "-O2"
build_arms tuned        "-C target-cpu=x86-64-v3" "-O3 -march=x86-64-v3"
build_arms native       "-C target-cpu=native"    "-O3 -march=native"

cat <<SUMMARY
gf2x pin      : ${GF2X_TAG} ${GF2X_COMMIT} (${GF2X_LICENSE})
gf2x prefixes : ${EXT}/prefix-{conservative,tuned,native}
arm binaries  : ${EXT}/arms-{conservative,tuned,native}/release/{gf2-poly-arm,gf2x-poly-arm,poly-validate}
SUMMARY
