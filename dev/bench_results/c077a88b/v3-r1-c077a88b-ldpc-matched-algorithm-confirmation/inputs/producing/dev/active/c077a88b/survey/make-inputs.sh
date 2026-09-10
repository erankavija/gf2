#!/usr/bin/env bash
# Builds the recorded LDPC decoder input bundles of the survey (jit:c077a88b).
#
# Each bundle fixes one parity-check matrix, one set of transmitted codewords
# and one set of channel LLRs. Every arm decodes the identical bytes, which is
# what makes a matched-algorithm comparison matched. The parity-check matrix
# comes from the workspace `export_alist` binary, the single construction site
# of the comparison AList, so this script never re-implements it.
#
# Bundles are large and exactly reproducible, so they stage outside the
# repository beside the external builds; their content digests are what the
# family addenda pin.
#
# Usage: make-inputs.sh [destination]
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../../.." && pwd)"
EXT="${1:-/tmp/c077a88b-ext}"
INPUTS="${EXT}/inputs"
mkdir -p "${INPUTS}"

cd "${REPO}"
./scripts/cargo-budget.sh cargo +1.95 build -p gf2-sim --release \
    --features test-support --bin export_alist
GF2_AFF3CT_ROOT="${EXT}/aff3ct" CARGO_TARGET_DIR="${REPO}/target/ldpc-survey" \
    RUSTFLAGS="-C target-cpu=native" \
    ./scripts/cargo-budget.sh cargo +1.95 build --release \
    --manifest-path "${HERE}/harness/Cargo.toml"

EXPORT="${REPO}/target/release/export_alist"
MAKE="${REPO}/target/ldpc-survey/release/ldpc-make-inputs"

# Experimental DVB-T2 noise setting frozen in the family addenda.
./scripts/cargo-budget.sh "${EXPORT}" --code dvb-t2-r12 --output "${INPUTS}/dvb_t2_r12.alist"
./scripts/cargo-budget.sh "${MAKE}" --code dvb-t2-r12 --alist "${INPUTS}/dvb_t2_r12.alist" \
    --esn0-db -1.2 --frames 128 --seed 42 --codeword-source both \
    --output "${INPUTS}/dvb-t2-r12-waterfall"

# Experimental NR mother-code noise setting frozen in the family addenda.
./scripts/cargo-budget.sh "${EXPORT}" --code nr-bg1-r12 --output "${INPUTS}/nr_bg1_r12.alist"
./scripts/cargo-budget.sh "${MAKE}" --code nr-bg1-r12 --alist "${INPUTS}/nr_bg1_r12.alist" \
    --esn0-db -4.3 --frames 128 --seed 42 --codeword-source both \
    --output "${INPUTS}/nr-bg1-z384-mother"

echo "input bundles ready under ${INPUTS}"
