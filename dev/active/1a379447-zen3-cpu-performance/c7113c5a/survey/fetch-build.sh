#!/usr/bin/env bash
# Fetch and build the pinned gf2x external baseline and the survey arms
# (jit:c7113c5a).
#
# Usage:
#   ./fetch-build.sh [staging-directory]
#
# Staging defaults to <repo>/target/c7113c5a-ext, inside the checkout's ignored
# build area; nothing this script downloads or builds is committed. The pins
# below are the contract: the upstream tag plus the commit it resolves to.
#
# Source: when GF2X_MIRROR names a local gf2x clone, the pinned commit is
# exported from it with `git archive`; otherwise the upstream is cloned into the
# staging area first. Either way the build tree is a fresh export of exactly
# the pinned commit, and `autoreconf -i` generates its configure script with the
# host autotools that `record-build-evidence.sh` records.
#
# gf2x is GPL-3.0-or-later in this configuration: `configure.ac` defines
# GPL_CODE_PRESENT exactly when `toom-gpl.c` contains the phrase "released under
# the GPL", and that condition also compiles the Toom-Cook routines the larger
# cells execute. The library is measured as an out-of-tree comparator only.
# Nothing here becomes a gf2 production dependency and no gf2 multiplication
# algorithm changes.
#
# Three build variants give gf2x the same host-targeting ladder the gf2 arms
# get, so neither side is measured under a handicap:
#
#   conservative  CFLAGS="-O2"                    RUSTFLAGS unset
#   tuned         CFLAGS="-O3 -march=x86-64-v3"   RUSTFLAGS="-C target-cpu=x86-64-v3"
#   native        CFLAGS="-O3 -march=native"      RUSTFLAGS="-C target-cpu=native"
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
EXT="${1:-${GF2_SURVEY_EXT:-${REPO}/target/${ISSUE}-ext}}"
JOBS=12

# ---------------------------------------------------------------------- pins
GF2X_URL="https://gitlab.inria.fr/gf2x/gf2x.git"
GF2X_TAG="gf2x-1.3.0"
GF2X_COMMIT="27ba588f03bf6e1e74763903bab25e6e8bb6d0f0"
GF2X_LICENSE="GPL-3.0-or-later"
# The stock CC_FOR_BUILD candidate `c89` rejects the C++-style comments of
# lowlevel/gen_bb_mul_code.c under this host's GCC, and configure's own
# build-compiler probe calls `exit` undeclared. That generator emits only the
# bit-by-bit fallback basecases used without PCLMUL.
CC_FOR_BUILD_OVERRIDE="gcc -std=gnu99 -Wno-implicit-function-declaration"
# The survey measures with the repository's minimum supported Rust toolchain.
export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-1.95}"

# Every compile takes the shared side of the CCX1 mutex through
# `scripts/cargo-budget.sh`, which is the only supported shared acquirer: it
# passes through the turnstile in `dev/scripts/ccx1-bench-flock.sh`, so a queued
# `--full-host` measurement run is not overtaken by a stream of builds.
shared() { "${REPO}/scripts/cargo-budget.sh" "$@"; }

mkdir -p "${EXT}"
EXT="$(cd "${EXT}" && pwd)"

# -------------------------------------------------------------------- source
if [[ ! -f "${EXT}/gf2x/.source-commit" ]]; then
    mirror="${GF2X_MIRROR:-}"
    if [[ -z "${mirror}" ]]; then
        mirror="${EXT}/gf2x-clone"
        [[ -d "${mirror}" ]] || git clone --quiet "${GF2X_URL}" "${mirror}"
    fi
    git -C "${mirror}" cat-file -e "${GF2X_COMMIT}^{commit}"
    tag_commit="$(git -C "${mirror}" rev-parse "${GF2X_TAG}^{commit}")"
    if [[ "${tag_commit}" != "${GF2X_COMMIT}" ]]; then
        echo "gf2x: tag ${GF2X_TAG} resolves to ${tag_commit}, not ${GF2X_COMMIT}" >&2
        exit 1
    fi
    rm -rf "${EXT}/gf2x"
    mkdir -p "${EXT}/gf2x"
    git -C "${mirror}" archive --format=tar "${GF2X_COMMIT}" | tar -x -C "${EXT}/gf2x"
    (cd "${EXT}/gf2x" && shared autoreconf -i >"${EXT}/autoreconf.log" 2>&1)
    echo "${GF2X_COMMIT}" >"${EXT}/gf2x/.source-commit"
fi
if [[ "$(cat "${EXT}/gf2x/.source-commit")" != "${GF2X_COMMIT}" ]]; then
    echo "gf2x: staged source is not ${GF2X_COMMIT}" >&2
    exit 1
fi
# The license condition configure.ac itself tests, asserted rather than assumed.
if ! grep -q "released under the GPL" "${EXT}/gf2x/toom-gpl.c"; then
    echo "gf2x: toom-gpl.c is the placeholder; the license is not ${GF2X_LICENSE}" >&2
    exit 1
fi

# -------------------------------------------------------------------- builds
build_variant() {
    local name="$1" cflags="$2"
    local dir="${EXT}/build-${name}"
    local prefix="${EXT}/prefix-${name}"
    if [[ -f "${prefix}/lib/libgf2x.so" ]]; then
        return 0
    fi
    rm -rf "${dir}"
    mkdir -p "${dir}"
    (
        cd "${dir}"
        CFLAGS="${cflags}" shared "${EXT}/gf2x/configure" \
            --prefix="${prefix}" --disable-static --enable-shared \
            "CC_FOR_BUILD=${CC_FOR_BUILD_OVERRIDE}" \
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
# distinct in the receipt. `--locked` builds exactly the committed lock file.
build_arms() {
    local name="$1" rustflags="$2" cflags="$3"
    CARGO_TARGET_DIR="${EXT}/arms-${name}" \
    RUSTFLAGS="${rustflags}" \
    GF2X_PREFIX="${EXT}/prefix-${name}" \
    GF2X_CFLAGS="${cflags}" \
        shared cargo build --release --locked \
            --manifest-path "${HERE}/gf2-side/Cargo.toml" \
            >"${EXT}/arms-${name}.build.log" 2>&1
}

build_arms conservative ""                        "-O2"
build_arms tuned        "-C target-cpu=x86-64-v3" "-O3 -march=x86-64-v3"
build_arms native       "-C target-cpu=native"    "-O3 -march=native"

cat <<SUMMARY
gf2x pin      : ${GF2X_TAG} ${GF2X_COMMIT} (${GF2X_LICENSE})
gf2x prefixes : ${EXT}/prefix-{conservative,tuned,native}
arm binaries  : ${EXT}/arms-{conservative,tuned,native}/release/{gf2-poly-arm,gf2x-poly-arm,poly-validate}
toolchain     : $(rustc --version)
SUMMARY
