#!/usr/bin/env bash
# Counts, per package, the `test-support` feature nodes of `cargo tree -e
# normal,features` (jit:316150fd) and writes them to `consumer-features.txt`
# beside this script. The edge selection follows normal dependencies only, so a
# dev-dependency feature appears only if resolver 2 unified it into a
# non-test build.
#
# Usage: record-consumer-features.sh
set -euo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
cd "$(git -C "$here" rev-parse --show-toplevel)"
export RUSTUP_TOOLCHAIN=1.95.0
{
    echo "# toolchain: $(rustc --version)"
    for package in gf2-kernels-simd gf2-core gf2-algebra gf2-coding gf2-sim; do
        nodes=$(cargo tree --offline -p "$package" -e normal,features | grep -c 'feature "test-support"' || true)
        echo "$package: $nodes test-support feature nodes"
    done
    nodes=$(cargo tree --offline -p gf2-core -e dev,features | grep -c 'feature "test-support"' || true)
    echo "control, gf2-core with dev edges: $nodes test-support feature nodes"
} > "$here/consumer-features.txt"
cat "$here/consumer-features.txt"
