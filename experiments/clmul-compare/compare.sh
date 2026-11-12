#!/usr/bin/env bash
# Long carry-less product throughput of gf2 against gf2x, one table per run.
#
# Environment: GF2_GF2X_ROOT names a directory of gf2x install prefixes, each
# a `prefix-<configuration>` subdirectory (for example prefix-tuned and
# prefix-native); the table reports gf2x's fastest configuration per size.
# CLMUL_COMPARE_CPU names the one core the measurements run on (default 8).
#
# The script builds the comparison binary once per gf2x prefix with
# target-cpu=native, then re-runs itself under `ccx1-bench-flock.sh`,
# which refuses outside a benchmark window. The table goes to stdout and to
# target/clmul-compare/<UTC>.md, below the host and the SHA-256 of each
# measured binary and of the gf2x library it loads.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo=$(git -C "$here" rev-parse --show-toplevel)
cd "$repo"
flock="$repo/$(python3 "$repo/$(git -C "$repo" ls-files -- ':(glob)**/repository_files.py')" file ccx1-bench-flock.sh)"
: "${GF2_GF2X_ROOT:?GF2_GF2X_ROOT must name the directory of gf2x prefixes}"
export GF2_GF2X_ROOT
cpu="${CLMUL_COMPARE_CPU:-8}"
manifest="$here/external/Cargo.toml"
build="$repo/target/clmul-compare/build"

prefixes() {
    find "$GF2_GF2X_ROOT" -mindepth 1 -maxdepth 1 -type d -name 'prefix-*' | sort
}

if [[ "${1:-}" != --measure ]]; then
    prefixes | while read -r prefix; do
        GF2_GF2X_PREFIX="$prefix" RUSTFLAGS="-C target-cpu=native" CARGO_INCREMENTAL=0 \
            CARGO_TARGET_DIR="$build/$(basename "$prefix")" \
            ./scripts/cargo-budget.sh cargo build --locked --release --manifest-path "$manifest"
    done
    CARGO_CI_NO_LOCK=1 exec "$flock" bash "$here/compare.sh" --measure
fi

out="$repo/target/clmul-compare"
mkdir -p "$out"
raw="$out/$(date -u +%Y%m%dT%H%M%SZ).tsv"
prefixes | while read -r prefix; do
    configuration=$(basename "$prefix")
    taskset -c "$cpu" "$build/$configuration/release/clmul-compare" |
        sed "s/^/$configuration\t/"
done > "$raw"
digests=$(prefixes | while read -r prefix; do
    "$build/$(basename "$prefix")/release/clmul-compare" --digests "$prefix/lib/libgf2x.so"
done)
python3 - "$raw" "$(hostname)" "$(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2- | sed 's/^ *//')" "$cpu" "$digests" <<'PY' | tee "${raw%.tsv}.md"
import sys
from collections import defaultdict
raw, host, model, cpu, digests = sys.argv[1:6]
gf2, gf2x = defaultdict(list), defaultdict(list)
for line in open(raw):
    configuration, arm, words, rate = line.split("\t")
    (gf2 if arm == "gf2" else gf2x)[int(words)].append((float(rate), configuration))
print(f"Host {host} ({model}), one core ({cpu}); products per second, median of the samples")
print("each binary takes; gf2 is the median over the per-prefix builds, gf2x its fastest prefix.")
print(digests)
print()
print("| words | gf2 | gf2x (configuration) | gf2 / gf2x |")
print("|---|---|---|---|")
for words in sorted(gf2):
    ours = sorted(rate for rate, _ in gf2[words])[len(gf2[words]) // 2]
    theirs, configuration = max(gf2x[words])
    print(f"| {words} | {ours:.4g} | {theirs:.4g} ({configuration.removeprefix('prefix-')}) | {ours / theirs:.3f} |")
PY
