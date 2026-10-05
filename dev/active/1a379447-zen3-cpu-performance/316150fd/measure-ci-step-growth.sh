#!/usr/bin/env bash
# Measures how much the `default-features-core-*` steps of `scripts/cargo-ci.sh`
# grow `target/ci-test` (jit:316150fd), and writes `ci-step-growth.txt` beside
# this script: the bytes before and after, the difference, and the step
# commands as `scripts/cargo-ci.sh` states them.
#
# Run it from the worktree root on a tree where the workspace `test-build` step has run, for
# example after
#   CARGO_CI_STEPS='^test-build$' ./scripts/cargo-ci.sh
# `du -sb` counts the bytes of every file under the directory, so the growth is
# the sum of the files the steps add.
#
# Usage: measure-ci-step-growth.sh
set -euo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel)
cd "$root"
export RUSTUP_TOOLCHAIN=1.95.0
pattern='^default-features-core-'
target=target/ci-test

before=$(du -sb "$target" | cut -f1)
CARGO_CI_STEPS=$pattern nice -n 19 ./scripts/cargo-ci.sh
after=$(du -sb "$target" | cut -f1)

{
    echo "# toolchain: $(rustc --version)"
    echo "# directory: $target"
    echo "bytes before: $before"
    echo "bytes after: $after"
    echo "growth in bytes: $((after - before))"
    echo "# baseline: target/ci-test after the test-build step of scripts/cargo-ci.sh"
    echo "# steps (scripts/cargo-ci.sh):"
    grep -E "^run_step ${pattern#^}" scripts/cargo-ci.sh
} > "$here/ci-step-growth.txt"
cat "$here/ci-step-growth.txt"
