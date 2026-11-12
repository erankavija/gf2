#!/usr/bin/env bash
# Encoder throughput against the reference implementations, one table per run:
#   compare.sh nr    gf2 / srsRAN / AFF3CT NR rate-matched LDPC encoding
#   compare.sh bch   gf2 / bchlib DVB-T2 BCH encoding at batch 1 and 256
#
# Environment: GF2_SRSRAN_ROOT, GF2_AFF3CT_ROOT and GF2_BCHLIB_ROOT name the
# staged source trees (AFF3CT with its built static library); the one binary
# links all three. ENCODE_COMPARE_WORKERS lists the NR worker counts (default
# 1,12).
#
# The script builds the comparison binary with target-cpu=native, then re-runs
# itself under `ccx1-bench-flock.sh --full-host`, which refuses
# outside a benchmark window. The table goes to stdout and to
# target/encode-compare/<suite>-<UTC>.md, below the SHA-256 of the binary,
# which links the comparators statically, and of AFF3CT's static library.
set -euo pipefail

suite="${1:?usage: compare.sh nr|bch}"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo=$(git -C "$here" rev-parse --show-toplevel)
flock="$repo/$(python3 "$repo/$(git -C "$repo" ls-files -- ':(glob)**/repository_files.py')" file ccx1-bench-flock.sh)"
cd "$repo"
: "${GF2_SRSRAN_ROOT:?GF2_SRSRAN_ROOT must name the staged srsRAN tree}"
: "${GF2_AFF3CT_ROOT:?GF2_AFF3CT_ROOT must name the staged AFF3CT tree}"
: "${GF2_BCHLIB_ROOT:?GF2_BCHLIB_ROOT must name the staged bchlib tree}"
export GF2_SRSRAN_ROOT GF2_AFF3CT_ROOT GF2_BCHLIB_ROOT

manifest="$here/external/Cargo.toml"
bin="$here/external/target/release/encode-compare"

if [[ "${2:-}" != --measure ]]; then
    RUSTFLAGS="-C target-cpu=native" CARGO_INCREMENTAL=0 \
        ./scripts/cargo-budget.sh cargo build --locked --release --manifest-path "$manifest"
    CARGO_CI_NO_LOCK=1 exec "$flock" --full-host \
        bash "$here/compare.sh" "$suite" --measure
fi

out="$repo/target/encode-compare"
mkdir -p "$out"
{
    "$bin" --digests "$GF2_AFF3CT_ROOT/build/lib/libaff3ct-4.7.0.a"
    "$bin" "$suite"
} | tee "$out/$suite-$(date -u +%Y%m%dT%H%M%SZ).md"
