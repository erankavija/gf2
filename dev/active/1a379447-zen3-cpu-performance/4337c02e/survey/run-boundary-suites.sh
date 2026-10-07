#!/usr/bin/env bash
# Run the shared matvec suite on both lanes and record the outcome (jit:4337c02e).
#
# Usage: run-boundary-suites.sh, from the worktree root. Untimed.
#
# The suite runs once with every feature, where the selector takes the SIMD
# lane at and above its threshold, and once with the default features, where
# every stride takes the scalar lane. The record holds test names and
# verdicts only, so a rerun on the same sources reproduces it byte for byte.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worktree root' >&2
    exit 2
}
export PATH="${HOME}/.cargo/bin:${PATH}"
export RUSTUP_TOOLCHAIN=1.95
RECORD="${HERE}/boundary-suites.txt"
SUITE=(--test simd_equiv_matvec --test matrix_vector --cargo-profile ci-test --profile ci)

run_build() {
    local label="$1"
    shift
    local command=(./scripts/cargo-budget.sh --test cargo nextest run -p gf2-core "$@" "${SUITE[@]}")
    local log
    log="$(mktemp)"
    "${command[@]}" >"${log}" 2>&1 || { cat "${log}" >&2; exit 1; }
    echo "## ${label}"
    echo "# command: ${command[*]}"
    sed -n 's/^ *\(PASS\|FAIL\|SKIP\) \[[^]]*\] *([^)]*) \(.*\)$/\1 \2/p' "${log}" | LC_ALL=C sort
    sed -n 's/^ *Summary \[[^]]*\] /# summary: /p' "${log}"
    rm -f "${log}"
}

{
    echo '# Shared matvec suite on both lanes (jit:4337c02e)'
    echo "# toolchain: $(rustc --version)"
    run_build 'simd build' --all-features
    run_build 'build without simd'
} >"${RECORD}.tmp"
mv "${RECORD}.tmp" "${RECORD}"
cat "${RECORD}"
