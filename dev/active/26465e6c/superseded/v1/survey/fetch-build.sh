#!/usr/bin/env bash
# Fetch and build pinned popcount baseline arms (jit:26465e6c).
# LIBPOPCNT_TAG=v4.2 LIBPOPCNT_COMMIT=923c43377278edc97de155ea70ac3ab20f397f3e
# MULA_COMMIT=138c91e21c3e6dab7875521b5d33b995e0e4c85e
# License names quoted from the pinned LICENSE files: BSD 2-Clause License
# (libpopcnt) and BSD 2-Clause License (mula-sse-popcount).

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# The worktree root (holds the tracked scripts/ directory this script calls).
WORKTREE="$(cd "${HERE}/../../../.." && pwd)"
# The true repository root, which may differ from the worktree root when this
# script runs inside a `git worktree` checkout: `.agents/ext` is untracked and
# lives only beside the main checkout, not replicated into each worktree.
# `--git-common-dir` resolves to the shared `.git` directory in both cases.
REPO="$(dirname "$(git -C "${HERE}" rev-parse --path-format=absolute --git-common-dir)")"
EXT="${1:-${REPO}/.agents/ext/26465e6c}"
LIBPOPCNT_TAG="v4.2"
LIBPOPCNT_COMMIT="923c43377278edc97de155ea70ac3ab20f397f3e"
MULA_COMMIT="138c91e21c3e6dab7875521b5d33b995e0e4c85e"

mkdir -p "${EXT}"

require_commit() {
    local dir="$1" want="$2" name="$3" got
    got="$(git -C "${dir}" rev-parse HEAD)"
    if [[ "${got}" != "${want}" ]]; then
        echo "${name}: expected ${want}, got ${got}" >&2
        exit 1
    fi
}

if [[ ! -d "${EXT}/libpopcnt/.git" ]]; then
    git clone --depth 1 --branch "${LIBPOPCNT_TAG}" https://github.com/kimwalisch/libpopcnt.git "${EXT}/libpopcnt"
fi
require_commit "${EXT}/libpopcnt" "${LIBPOPCNT_COMMIT}" libpopcnt

if [[ ! -d "${EXT}/mula-sse-popcount/.git" ]]; then
    git clone https://github.com/WojciechMula/sse-popcount.git "${EXT}/mula-sse-popcount"
fi
require_commit "${EXT}/mula-sse-popcount" "${MULA_COMMIT}" mula-sse-popcount

mkdir -p "${HERE}/vendor/mula"
cp "${EXT}/mula-sse-popcount/popcnt-lookup.cpp" "${HERE}/vendor/mula/popcnt-lookup.cpp"
cp "${EXT}/mula-sse-popcount/popcnt-avx2-harley-seal.cpp" "${HERE}/vendor/mula/popcnt-avx2-harley-seal.cpp"
cp "${EXT}/mula-sse-popcount/LICENSE" "${HERE}/vendor/mula/LICENSE"

# Both builds go through scripts/cargo-budget.sh, the only supported shared
# acquirer of the CCX1 mutex. Taking `flock -s` on that mutex directly, as this
# script used to for the make, skips the turnstile in
# dev/scripts/ccx1-bench-flock.sh and lets a harness build enter ahead of a
# queued measurement run. cargo-budget.sh wraps an arbitrary command, so the
# make routes through it as the cargo build does.
(cd "${WORKTREE}" && ./scripts/cargo-budget.sh make -C "${HERE}" EXT="${EXT}")
(cd "${WORKTREE}" && ./scripts/cargo-budget.sh cargo build --release --manifest-path "${HERE}/gf2-side/Cargo.toml")

# The two evidence artifacts every receipt in this issue cites. Regenerating
# them here keeps them the state of the binaries that were just built, so a
# stale artifact cannot outlive the arm it describes.
"${HERE}/observe-instructions.sh" >"${HERE}/objdump-evidence.txt"
GF2_SURVEY_EXT="${EXT}" "${HERE}/record-provenance.sh"
