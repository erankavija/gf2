#!/usr/bin/env bash
# Runs the fast-tier suites that reach the kernel crate under Rust 1.95, with
# and without the `simd` feature, and writes `simd-build.txt` and
# `scalar-build.txt` into LOG_DIR. Each log opens with the toolchain and
# command and closes with the exit status.
#
# Usage: run-kernel-suites.sh LOG_DIR
set -uo pipefail

if [[ $# -ne 1 ]]; then
    echo "usage: run-kernel-suites.sh LOG_DIR" >&2
    exit 2
fi
mkdir -p -- "$1"
logs=$(cd -- "$1" && pwd)
root=$(git rev-parse --show-toplevel)
cd "$root"
export RUSTUP_TOOLCHAIN=1.95.0

quiet=(--cargo-profile ci-test --profile ci --status-level fail --final-status-level fail)

run() {
    local log="$logs/$1.txt"
    shift
    local command=(nice -n 19 ./scripts/cargo-budget.sh --test cargo nextest run "$@" "${quiet[@]}")
    {
        echo "# toolchain: $(rustc --version)"
        echo "# command: ${command[*]}"
        "${command[@]}" 2>&1 | sed "s|$root/||g"
        echo "# exit: ${PIPESTATUS[0]}"
    } > "$log"
    tail -n 3 "$log"
}

run simd-build -p gf2-kernels-simd -p gf2-core -p gf2-algebra -p gf2-coding --all-features

# `gf2-core` is the consumer whose suites build without `simd`: the test builds
# of `gf2-algebra` and `gf2-coding` enable their default features, `simd`
# included, through their own `test-support` dev-dependency. The list is every
# `gf2-core` feature except `simd`.
run scalar-build -p gf2-core --no-default-features \
    --features rand,io,parallel,visualization,tuning-profile,test-support
