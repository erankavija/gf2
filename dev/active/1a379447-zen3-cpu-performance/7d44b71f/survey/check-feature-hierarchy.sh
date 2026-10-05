#!/usr/bin/env bash
# Compiles `feature-hierarchy.rs` under Rust 1.95 and writes
# `feature-hierarchy.txt` beside itself: the toolchain, the command and its
# exit status (jit:7d44b71f).
#
# Usage: check-feature-hierarchy.sh
set -uo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
export RUSTUP_TOOLCHAIN=1.95.0
scratch=$(mktemp -d)
trap 'rm -rf -- "$scratch"' EXIT
cd "$here"
command=(rustc --edition 2021 --emit=metadata --out-dir "$scratch" feature-hierarchy.rs)
{
    echo "# toolchain: $(rustc --version)"
    echo "# command: rustc --edition 2021 --emit=metadata feature-hierarchy.rs"
    "${command[@]}" 2>&1
    echo "# exit: $?"
} > feature-hierarchy.txt
tail -n 1 feature-hierarchy.txt
