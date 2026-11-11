#!/usr/bin/env bash
# gf2 decoder routes against AFF3CT NMS configurations on identical recorded
# bundles: flooding NMS 0.75 with cap 50, and gf2's layered i8 route under its
# corpus configuration with cap 25, all with syndrome stopping. FER counts information-window frame errors.
#
# Environment: GF2_AFF3CT_ROOT names the staged AFF3CT tree (as for the AFF3CT
# arm) and GF2_LDPC_BUNDLES the directory holding the bundle directories named
# in arms.json. QUICK_COMPARE_CALLS sets the timed calls per run (default 30).
#
# The script builds both fast-bench executables with target-cpu=native, then
# re-runs itself under `ccx1-bench-flock.sh --full-host`, which
# refuses outside a benchmark window. Each run prints one JSON line; a run
# whose batch is not whole native waves of its decoder is skipped. Each code's
# table goes to stdout and target/aff3ct-ldpc-gaps/quick-compare/<UTC>.txt as
# soon as that code's runs finish, beside the raw lines. The ratio is best gf2
# frames/s over best AFF3CT frames/s, so a value above 1 means gf2 is faster;
# the flooding ratio restricts both sides to flooding decoders (every gf2 route
# but layered, AFF3CT's flooding configurations at either precision).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo=$(git -C "$here" rev-parse --show-toplevel)
cd "$repo"
flock="$repo/$(python3 "$repo/$(git -C "$repo" ls-files -- ':(glob)**/repository_files.py')" file ccx1-bench-flock.sh)"
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
    CARGO_CI_NO_LOCK=1 exec "$flock" --full-host \
        bash "$here/quick-compare.sh" --measure
fi

calls="${QUICK_COMPARE_CALLS:-30}"
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
out="$repo/target/aff3ct-ldpc-gaps/quick-compare"
mkdir -p "$out"
raw="$out/$stamp.jsonl"
: > "$raw"

# One "<bundle directory> <code name>" line per arms.json code entry.
arm_codes() {
    python3 -c 'import json, sys
for arm in json.load(open(sys.argv[1]))["codes"].values():
    print(arm["bundle"], arm["code"])' "$here/arms.json"
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
# "workers batch pinning"; pinned workers take one CPU of every physical core
# before any SMT sibling, and the unpinned cells show what pinning costs or gains.
cells=(
    "1 1 pinned" "1 8 pinned" "1 16 pinned" "12 8 pinned" "12 16 pinned" "24 16 pinned"
    "12 16 unpinned" "24 16 unpinned"
)

run() {
    local side=$1 executable=$2
    shift 2
    local line
    if line=$("$executable" "$@" < /dev/null 2> "$out/$stamp.stderr"); then
        printf '{"side":"%s","run":%s}\n' "$side" "$line" >> "$raw"
    elif grep -q 'native waves\|whole' "$out/$stamp.stderr"; then
        :
    else
        printf '%s %s: %s\n' "$side" "$*" "$(tail -n 1 "$out/$stamp.stderr")" >&2
        printf '{"side":"%s","code":"%s","failed":"%s"}\n' "$side" "$2" "$*" >> "$raw"
    fi
}

table() {
    python3 - "$raw" "$1" << 'EOF'
import json, sys

code = sys.argv[2]
rows = [json.loads(line) for line in open(sys.argv[1])]
cells = {}
for row in rows:
    run = row.get("run")
    if run and run["code"] == code:
        key = (run["workers"], run["batch"], run["pinning"])
        cells.setdefault(key, []).append((row["side"], run))
for (workers, batch, pinning), runs in cells.items():
    print(f"\n{code}  workers {workers}  batch {batch}  {pinning}")
    print(f"  {'side':7} {'decoder':44} {'prec':4} {'cap':>4} {'FER':>9} {'frames/s':>10} {'info Mb/s':>10}")
    best, flooding = {}, {}
    for side, run in runs:
        fps = run["frames_per_second"]
        best[side] = max(best.get(side, 0.0), fps)
        if run["kind"] not in ("layered", "layered-onms"):
            flooding[side] = max(flooding.get(side, 0.0), fps)
        errors = f"{run['frame_errors']}/{run['quality_frames']}"
        print(f"  {side:7} {run['decoder'][:44]:44} {run['precision']:4} {run['iteration_cap']:>4} {errors:>9} {fps:10.1f} {run['info_mbps']:10.2f}")
    if "gf2" in best and "aff3ct" in best:
        print(f"  ratio {best['gf2'] / best['aff3ct']:.3f}")
    if "gf2" in flooding and "aff3ct" in flooding:
        print(f"  flooding ratio {flooding['gf2'] / flooding['aff3ct']:.3f}")
for row in rows:
    if "failed" in row and row["code"] == code:
        print(f"\nfailed: {row['side']} {row['failed']}")
EOF
}

{
    echo "host $(hostname); FER = frames with an information-bit error over the decoded frames;"
    echo "ratio = best gf2 frames/s / best AFF3CT frames/s;"
    echo "flooding ratio = the same over flooding decoders only"
    arm_codes | while read -r bundle code; do
        dir="$GF2_LDPC_BUNDLES/$bundle"
        for cell in "${cells[@]}"; do
            read -r workers batch pinning <<< "$cell"
            for route in "${gf2_routes[@]}"; do
                read -r kind simd <<< "$route"
                run gf2 "$gf2_bin" "$dir" "$code" "$kind" "$simd" f32 \
                    "$workers" "$batch" "$calls" "$pinning"
            done
            for config in "${aff3ct_configs[@]}"; do
                read -r kind simd precision <<< "$config"
                run aff3ct "$aff3ct_bin" "$dir" "$code" "$kind" "$simd" "$precision" \
                    "$workers" "$batch" "$calls" "$pinning"
            done
        done
        table "$code"
    done
} | tee "$out/$stamp.txt"
rm -f "${out:?}/${stamp:?}.stderr"
