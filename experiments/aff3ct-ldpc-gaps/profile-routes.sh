#!/usr/bin/env bash
# User-space perf profiles of gf2's QC and layered routes in gf2-fast-bench on
# the recorded bundles, at (workers, batch) = (1, 1), (1, 16) and (12, 16).
#
# Environment: GF2_LDPC_BUNDLES names the directory holding the bundle
# directories named in arms.json; QUICK_COMPARE_CALLS sets the timed calls per
# run (default 30). The script builds gf2-fast-bench with target-cpu=native and
# debug line tables, then re-runs itself under
# `dev/scripts/ccx1-bench-flock.sh --full-host`, which refuses outside a
# benchmark window. Each profile writes perf.data, the fast-bench JSON line and
# a `perf report` of the top self-time symbols under
# target/aff3ct-ldpc-gaps/profile-routes/<UTC>/.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo=$(git -C "$here" rev-parse --show-toplevel)
cd "$repo"
: "${GF2_LDPC_BUNDLES:?GF2_LDPC_BUNDLES must name the directory of bundle directories}"
export GF2_LDPC_BUNDLES

target=$(cargo metadata --format-version 1 --no-deps --offline |
    python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
gf2_bin="$target/release/gf2-fast-bench"

if [[ "${1:-}" != --measure ]]; then
    export RUSTFLAGS="-C target-cpu=native -C force-frame-pointers=yes" CARGO_INCREMENTAL=0
    export CARGO_PROFILE_RELEASE_DEBUG=line-tables-only
    ./scripts/cargo-budget.sh cargo build --locked --release -p aff3ct-ldpc-gaps --bin gf2-fast-bench
    CARGO_CI_NO_LOCK=1 exec ./dev/scripts/ccx1-bench-flock.sh --full-host \
        bash "$here/profile-routes.sh" --measure
fi

calls="${QUICK_COMPARE_CALLS:-30}"
out="$repo/target/aff3ct-ldpc-gaps/profile-routes/$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$out"

bundle() {
    python3 -c 'import json, sys
print(json.load(open(sys.argv[1]))["codes"][sys.argv[2]]["bundle"])' "$here/arms.json" "$1"
}

for code in dvb-t2-r12 nr-bg1-r12; do
    dir="$GF2_LDPC_BUNDLES/$(bundle "$code")"
    for cell in "1 1" "1 16" "12 16"; do
        read -r workers batch <<< "$cell"
        for kind in qc layered; do
            name="$code-$kind-w$workers-b$batch"
            perf record -q --call-graph fp -o "$out/$name.data" -- \
                "$gf2_bin" "$dir" "$code" "$kind" avx2 f32 "$workers" "$batch" "$calls" \
                > "$out/$name.json"
            perf report -q --stdio --no-children --percent-limit 1 \
                -i "$out/$name.data" --sort symbol > "$out/$name.txt" 2> /dev/null
            printf '%s\n' "$name" "$(head -n 12 "$out/$name.txt")"
        done
    done
done | tee "$out/summary.txt"
