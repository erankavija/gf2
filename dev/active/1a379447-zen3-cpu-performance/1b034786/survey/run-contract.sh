#!/usr/bin/env bash
# Runs the shared `M4rmGrayBuildFn` argument-contract test under Rust 1.95 and
# writes `test-logs/<LABEL>.txt` beside this script (jit:1b034786). The log
# opens with the toolchain and command and closes with the exit status.
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

command=(nice -n 19 ./scripts/cargo-budget.sh --test cargo nextest run -p gf2-kernels-simd
    --test m4rm_gray_build_contract --cargo-profile ci-test --profile ci)
{
    echo "# toolchain: $(rustc --version)"
    echo "# command: ${command[*]}"
    "${command[@]}" 2>&1 | sed "s|$root/||g"
    echo "# exit: ${PIPESTATUS[0]}"
} > "$here/test-logs/$1.txt"
tail -n 4 "$here/test-logs/$1.txt"
