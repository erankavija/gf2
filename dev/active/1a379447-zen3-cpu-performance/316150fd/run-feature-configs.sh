#!/usr/bin/env bash
# Builds and runs the `gf2-core` test targets under Rust 1.95 in the default,
# `simd`-only and all-features configurations (jit:316150fd).
#
# Per configuration it writes two logs into `feature-logs/<label>/` beside this
# script, each opening with the toolchain and command and closing with the exit
# status:
#
#   <config>-build   `cargo build --tests --keep-going`: every target that fails
#                    to compile is reported, not only the first
#   <config>-run     the focused nextest command of AGENTS.md
#
# Usage: run-feature-configs.sh <label>
set -uo pipefail

label=${1:?usage: run-feature-configs.sh <label>}
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel)
cd "$root"
export RUSTUP_TOOLCHAIN=1.95.0
logs="$here/feature-logs/$label"
mkdir -p "$logs"

run() {
    local log="$logs/$1.txt"
    shift
    {
        echo "# toolchain: $(rustc --version)"
        echo "# command: $*"
        "$@" 2>&1 | sed "s|$root/||g; s|$HOME/|~/|g"
        echo "# exit: ${PIPESTATUS[0]}"
    } > "$log"
    echo "$(basename "$log"): $(tail -n 1 "$log")"
}

configuration() {
    local name=$1
    shift
    run "$name-build" nice -n 19 ./scripts/cargo-budget.sh cargo build -p gf2-core --tests "$@" \
        --profile ci-test --keep-going
    run "$name-run" nice -n 19 ./scripts/cargo-budget.sh --test cargo nextest run -p gf2-core "$@" \
        --cargo-profile ci-test --profile ci
}

configuration default
configuration simd --features simd
configuration all-features --all-features
