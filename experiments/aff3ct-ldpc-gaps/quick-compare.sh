#!/usr/bin/env bash
# gf2 decoder routes against AFF3CT NMS configurations on identical recorded
# bundles, worker i pinned to CPU i: flooding NMS 0.75 with cap 50, and gf2's
# layered i8 route under its corpus configuration with cap 25, all with
# syndrome stopping. FER counts information-window frame errors.
#
# Environment: GF2_AFF3CT_ROOT names the staged AFF3CT tree (as for the AFF3CT
# arm) and GF2_LDPC_BUNDLES the directory holding the bundle directories named
# in arms.json. QUICK_COMPARE_CALLS sets the timed calls per run (default 30).
#
# The script builds both fast-bench executables with target-cpu=native, then
# re-runs itself under `dev/scripts/ccx1-bench-flock.sh --full-host`, which
# refuses outside a benchmark window. Each run prints one JSON line; a run
# whose batch is not whole native waves of its decoder is skipped. The table
# goes to stdout and target/aff3ct-ldpc-gaps/quick-compare/<UTC>.txt beside
# the raw lines. The ratio is best gf2 frames/s over best AFF3CT frames/s, so
# a value above 1 means gf2 is faster.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo=$(git -C "$here" rev-parse --show-toplevel)
cd "$repo"
: "${GF2_AFF3CT_ROOT:?GF2_AFF3CT_ROOT must name the staged AFF3CT tree}"
: "${GF2_LDPC_BUNDLES:?GF2_LDPC_BUNDLES must name the directory of bundle directories}"
export GF2_AFF3CT_ROOT GF2_LDPC_BUNDLES

target_dir() {
    cargo metadata --format-version 1 --no-deps --offline --manifest-path "$1" |
        python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])'
}
gf2_bin="$(target_dir Cargo.toml)/release/gf2-fast-bench"
aff3ct_manifest="$here/aff3ct/Cargo.toml"
aff3ct_bin="$(target_dir "$aff3ct_manifest")/release/aff3ct-fast-bench"

if [[ "${1:-}" != --measure ]]; then
    export RUSTFLAGS="-C target-cpu=native" CARGO_INCREMENTAL=0
    ./scripts/cargo-budget.sh cargo build --locked --release -p aff3ct-ldpc-gaps --bin gf2-fast-bench
    # The adapter's build script needs the compiler's dependency file, which an
    # sccache hit does not write.
    SCCACHE_RECACHE=1 ./scripts/cargo-budget.sh cargo build --locked --release \
        --manifest-path "$aff3ct_manifest" --bin aff3ct-fast-bench
    CARGO_CI_NO_LOCK=1 exec ./dev/scripts/ccx1-bench-flock.sh --full-host \
        bash "$here/quick-compare.sh" --measure
fi

calls="${QUICK_COMPARE_CALLS:-30}"
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
out="$repo/target/aff3ct-ldpc-gaps/quick-compare"
mkdir -p "$out"
raw="$out/$stamp.jsonl"
: > "$raw"

bundle() {
    python3 -c 'import json, sys
codes = json.load(open(sys.argv[1]))["codes"]
print(codes[sys.argv[2]]["bundle"])' "$here/arms.json" "$1"
}

gf2_routes=(
    "single none" "lanes none" "lanes avx2" "qc none" "qc avx2" "default auto"
    "layered none" "layered avx2"
)
aff3ct_configs=(
    "flooding none f32" "layered none f32"
    "flooding INTER f32" "layered INTER f32" "layered-onms INTER f32"
    "flooding INTER i16" "layered INTER i16" "layered-onms INTER i16"
)
cells=("1 1" "1 8" "1 16" "12 8" "12 16" "24 16")

run() {
    local side=$1 executable=$2
    shift 2
    local line
    if line=$("$executable" "$@" 2> "$out/$stamp.stderr"); then
        printf '{"side":"%s","run":%s}\n' "$side" "$line" >> "$raw"
    elif grep -q 'native waves\|whole' "$out/$stamp.stderr"; then
        :
    else
        printf '%s %s: %s\n' "$side" "$*" "$(tail -n 1 "$out/$stamp.stderr")" >&2
        printf '{"side":"%s","failed":"%s"}\n' "$side" "$*" >> "$raw"
    fi
}

for code in dvb-t2-r12 nr-bg1-r12; do
    dir="$GF2_LDPC_BUNDLES/$(bundle "$code")"
    for cell in "${cells[@]}"; do
        read -r workers batch <<< "$cell"
        for route in "${gf2_routes[@]}"; do
            read -r kind simd <<< "$route"
            run gf2 "$gf2_bin" "$dir" "$code" "$kind" "$simd" f32 "$workers" "$batch" "$calls"
        done
        for config in "${aff3ct_configs[@]}"; do
            read -r kind simd precision <<< "$config"
            run aff3ct "$aff3ct_bin" "$dir" "$code" "$kind" "$simd" "$precision" \
                "$workers" "$batch" "$calls"
        done
    done
done
rm -f "$out/$stamp.stderr"

python3 - "$raw" "$(hostname)" << 'EOF' | tee "$out/$stamp.txt"
import json, sys
from collections import defaultdict

rows = [json.loads(line) for line in open(sys.argv[1])]
cells = defaultdict(list)
failed = [row for row in rows if "failed" in row]
for row in rows:
    if "run" in row:
        run = row["run"]
        cells[(run["code"], run["workers"], run["batch"])].append((row["side"], run))
print(f"host {sys.argv[2]}; FER = frames with an information-bit error over the decoded frames;")
print("ratio = best gf2 frames/s / best AFF3CT frames/s")
for (code, workers, batch), runs in cells.items():
    print(f"\n{code}  workers {workers}  batch {batch}")
    print(f"  {'side':7} {'decoder':44} {'cap':>4} {'FER':>9} {'frames/s':>10} {'info Mb/s':>10}")
    best = {}
    for side, run in runs:
        fps = run["frames_per_second"]
        best[side] = max(best.get(side, 0.0), fps)
        errors = f"{run['frame_errors']}/{run['quality_frames']}"
        print(f"  {side:7} {run['decoder'][:44]:44} {run['iteration_cap']:>4} {errors:>9} {fps:10.1f} {run['info_mbps']:10.2f}")
    if "gf2" in best and "aff3ct" in best:
        print(f"  ratio {best['gf2'] / best['aff3ct']:.3f}")
for row in failed:
    print(f"\nfailed: {row['side']} {row['failed']}")
EOF
