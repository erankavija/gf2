#!/usr/bin/env bash
# BCH cutover comparison (@/issue/fd9d5416): the pre-cutover gf2 revision, the
# current one (HEAD) and the external baselines on the epic's two workloads,
# W1 batch encoding and W2 generator materialization, one table per run.
#
# Environment: GF2_SRSRAN_ROOT, GF2_AFF3CT_ROOT, GF2_BCHLIB_ROOT and
# GF2_M4RI_ROOT (an M4RI install prefix) name the staged comparator trees.
# CUTOVER_PRE_REVISION (default 74871c6b, the pinned pre-cutover revision of
# @/issue/88ca7d2f), CUTOVER_ROUNDS (default 5), CUTOVER_CELLS (cell-ID
# prefixes such as W1/B2,W2/T2S) and CUTOVER_CALL_BUDGET_S (default 10)
# configure the run; CUTOVER_CORES (default 2-5) pins an indication.
#
# The script extracts both revisions with `git archive` under
# target/encode-compare/cutover/trees, builds each one's `bch_cutover` example
# of gf2-coding from `cutover/main.rs`, `src/cutover/harness.rs` and that
# revision's adapter, and builds the external comparison binary, all with
# target-cpu=native. Inside a benchmark window (GF2_BENCH_WINDOW=1) it re-runs
# itself under `dev/scripts/ccx1-bench-flock.sh`, which pins to the CCX1 cores
# and refuses outside a window, and labels the table a window measurement;
# outside one it measures on CUTOVER_CORES and labels the table an indication.
# Each round runs every arm once, in an order that rotates between rounds. The
# table goes to stdout and, with the raw rounds, to
# target/encode-compare/cutover/<UTC>/. It exits 1 when an arm's output
# differs from the current revision's on a cell.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo=$(git -C "$here" rev-parse --show-toplevel)
cd "$repo"
: "${GF2_SRSRAN_ROOT:?GF2_SRSRAN_ROOT must name the staged srsRAN tree}"
: "${GF2_AFF3CT_ROOT:?GF2_AFF3CT_ROOT must name the staged AFF3CT tree}"
: "${GF2_BCHLIB_ROOT:?GF2_BCHLIB_ROOT must name the staged bchlib tree}"
: "${GF2_M4RI_ROOT:?GF2_M4RI_ROOT must name the M4RI install prefix}"
export GF2_SRSRAN_ROOT GF2_AFF3CT_ROOT GF2_BCHLIB_ROOT GF2_M4RI_ROOT

pre=$(git rev-parse --verify "${CUTOVER_PRE_REVISION:-74871c6b73b722f35d5acb5ef68750cecd93e2ef}^{commit}")
current=$(git rev-parse --verify "HEAD^{commit}")
work="$repo/target/encode-compare/cutover"
external="$here/external/target/release/encode-compare"
mode=indication
[[ "${GF2_BENCH_WINDOW:-}" == 1 ]] && mode=window

arm_binary() {
    echo "$work/trees/$1/target/release/examples/bch_cutover"
}

# Extracts revision $1 once and builds its example over adapter $2.
build_revision() {
    local revision=$1 adapter=$2 tree="$work/trees/$1"
    if [[ ! -e "$tree/.extracted" ]]; then
        rm -rf "$tree"
        mkdir -p "$tree"
        git archive "$revision" | tar -x -C "$tree"
        touch "$tree/.extracted"
    fi
    local example="$tree/crates/gf2-coding/examples/bch_cutover"
    mkdir -p "$example"
    cp "$here/cutover/main.rs" "$example/main.rs"
    cp "$here/src/cutover/harness.rs" "$example/harness.rs"
    cp "$adapter" "$example/api.rs"
    (cd "$tree" && RUSTFLAGS="-C target-cpu=native" CARGO_INCREMENTAL=0 \
        "$repo/scripts/cargo-budget.sh" cargo build --locked --release --quiet \
        -p gf2-coding --example bch_cutover)
}

if [[ "${1:-}" != --measure ]]; then
    build_revision "$pre" "$here/cutover/pre.rs"
    build_revision "$current" "$here/src/cutover/current.rs"
    RUSTFLAGS="-C target-cpu=native" CARGO_INCREMENTAL=0 \
        ./scripts/cargo-budget.sh cargo build --locked --release --quiet \
        --manifest-path "$here/external/Cargo.toml"
    if [[ $mode == window ]]; then
        CARGO_CI_NO_LOCK=1 exec ./dev/scripts/ccx1-bench-flock.sh \
            bash "$here/cutover.sh" --measure
    fi
fi

pin=()
[[ $mode == indication ]] && pin=(taskset -c "${CUTOVER_CORES:-2-5}")
run="$work/$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$run"
raw="$run/rounds.tsv"
: >"$raw"
arms=(pre current external)
rounds=${CUTOVER_ROUNDS:-5}
for ((round = 1; round <= rounds; round++)); do
    for ((slot = 0; slot < ${#arms[@]}; slot++)); do
        arm=${arms[$(((slot + round - 1) % ${#arms[@]}))]}
        case $arm in
            pre) CUTOVER_REVISION=${pre:0:9} "${pin[@]}" "$(arm_binary "$pre")" round "$round" ;;
            current) CUTOVER_REVISION=${current:0:9} "${pin[@]}" "$(arm_binary "$current")" round "$round" ;;
            external) "${pin[@]}" "$external" cutover round "$round" ;;
        esac >>"$raw"
    done
done

{
    echo "# host: $(hostname) ($(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2- | sed 's/^ //'))"
    if [[ $mode == window ]]; then
        echo "# benchmark window, cores ${GF2_CCX1_CPUS:-6-11}"
    else
        echo "# indication, cores ${CUTOVER_CORES:-2-5}"
    fi
    echo "# pre-cutover ${pre:0:9}, current ${current:0:9}; $rounds interleaved rounds;" \
        "call budget ${CUTOVER_CALL_BUDGET_S:-10} s; harness experiments/encode-compare/cutover.sh"
    python3 -B "$here/cutover/summarize.py" "$raw"
} | tee "$run/table.md"
