#!/usr/bin/env bash
# Runs the `gf2-algebra` test suite under Rust 1.95 on the scalar lane
# (`scripts/scalar-lane-tests.sh`, which also runs `gf2-coding`) and in the
# default configuration with the `simd` feature (jit:dcfe8386), and writes `toolchain-runs.txt` beside this script. Per
# configuration the record holds the toolchain, the command, the feature lists
# Cargo resolves for the workspace packages of the build, the nextest summary
# line and the exit status, each as observed in this run.
#
# Usage: record-toolchain-runs.sh
set -uo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
cd "$(git -C "$here" rev-parse --show-toplevel)"
export RUSTUP_TOOLCHAIN=1.95
packages=(--package=gf2-algebra)
nextest=(--cargo-profile ci-test --profile ci)

plain() {
    sed -E 's/\x1b\[[0-9;]*[mK]//g'
}

# The scalar-lane script prints its own `scalar lane:` feature lines; the
# pattern keeps those and the nextest summary.
run() {
    echo "# toolchain: $(rustc --version)"
    echo "# command: $*"
    "$@" 2>&1 | plain | grep -E '^scalar lane: |^[[:space:]]*Summary '
    echo "# exit: ${PIPESTATUS[0]}"
}

{
    run ./scripts/cargo-budget.sh --test ./scripts/scalar-lane-tests.sh "${nextest[@]}"
    echo
    cargo tree "${packages[@]}" -e normal,dev --prefix none -f '{p} [{f}]' |
        sed -nE 's|^([^ ]+ v[^ ]+) \(/[^)]*\) (\[[^]]*\]).*|default: \1 \2|p' | sort -u
    run ./scripts/cargo-budget.sh --test cargo nextest run "${packages[@]}" "${nextest[@]}"
} > "$here/toolchain-runs.txt"
cat "$here/toolchain-runs.txt"
