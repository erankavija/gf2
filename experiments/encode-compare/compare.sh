#!/usr/bin/env bash
# Encoder throughput against the reference implementations, one table per run:
#   compare.sh nr    gf2 / srsRAN / AFF3CT NR rate-matched LDPC encoding
#
# Environment: GF2_SRSRAN_ROOT and GF2_AFF3CT_ROOT name the staged source
# trees (AFF3CT with its built static library). ENCODE_COMPARE_WORKERS lists
# the worker counts (default 1,12).
#
# The script builds the comparison binary with target-cpu=native, then re-runs
# itself under `dev/scripts/ccx1-bench-flock.sh --full-host`, which refuses
# outside a benchmark window. The table goes to stdout and to
# target/encode-compare/<suite>-<UTC>.md.
set -euo pipefail

suite="${1:?usage: compare.sh nr}"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo=$(git -C "$here" rev-parse --show-toplevel)
cd "$repo"
: "${GF2_SRSRAN_ROOT:?GF2_SRSRAN_ROOT must name the staged srsRAN tree}"
: "${GF2_AFF3CT_ROOT:?GF2_AFF3CT_ROOT must name the staged AFF3CT tree}"
export GF2_SRSRAN_ROOT GF2_AFF3CT_ROOT

manifest="$here/external/Cargo.toml"
bin="$here/external/target/release/encode-compare"

if [[ "${2:-}" != --measure ]]; then
    RUSTFLAGS="-C target-cpu=native" CARGO_INCREMENTAL=0 \
        ./scripts/cargo-budget.sh cargo build --locked --release --manifest-path "$manifest"
    CARGO_CI_NO_LOCK=1 exec ./dev/scripts/ccx1-bench-flock.sh --full-host \
        bash "$here/compare.sh" "$suite" --measure
fi

out="$repo/target/encode-compare"
mkdir -p "$out"
"$bin" "$suite" | tee "$out/$suite-$(date -u +%Y%m%dT%H%M%SZ).md"
