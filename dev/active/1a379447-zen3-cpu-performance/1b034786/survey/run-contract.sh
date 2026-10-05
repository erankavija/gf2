#!/usr/bin/env bash
# Runs the shared Gray-table builder contract under Rust 1.95 and writes
# `test-logs/<LABEL>-simd.txt` and `test-logs/<LABEL>-scalar.txt` beside this
# script (jit:1b034786): the kernel crate's own run and `gf2-core`'s run over
# every builder with every feature, then `gf2-core`'s run without `simd`. Each
# log opens with the toolchain and command, holds one line per builder and
# case, and closes with the exit status.
#
# Usage: run-contract.sh LABEL
set -uo pipefail

if [[ $# -ne 1 ]]; then
    echo "usage: run-contract.sh LABEL" >&2
    exit 2
fi
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel)
cd "$root"
export RUSTUP_TOOLCHAIN=1.95.0
mkdir -p "$here/test-logs"

run() {
    local log="$here/test-logs/$1.txt"
    shift
    local command=(nice -n 19 ./scripts/cargo-budget.sh --test cargo nextest run "$@"
        --cargo-profile ci-test --profile ci --no-capture)
    {
        echo "# toolchain: $(rustc --version)"
        echo "# command: ${command[*]}"
        "${command[@]}" 2>&1 | sed "s|$root/||g"
        echo "# exit: ${PIPESTATUS[0]}"
    } > "$log"
    tail -n 2 "$log"
}

run "$1-simd" -p gf2-kernels-simd -p gf2-core --all-features \
    -E 'binary(m4rm_gray_build_contract) | test(/^m4rm::tests::/)'
# Every `gf2-core` feature except `simd`.
run "$1-scalar" -p gf2-core --no-default-features \
    --features rand,io,parallel,visualization,tuning-profile,test-support \
    --test m4rm_gray_build_contract
