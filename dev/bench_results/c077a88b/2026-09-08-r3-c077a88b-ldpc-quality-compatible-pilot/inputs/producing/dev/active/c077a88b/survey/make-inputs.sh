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
EXT="${1:-${REPO}/.agents/ext/c077a88b}"
LOCK="${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}"
INPUTS="${EXT}/inputs"
mkdir -p "${INPUTS}"

cd "${REPO}"
flock -s "${LOCK}" ./scripts/cargo-budget.sh cargo build -p gf2-sim --release \
    --features test-support --bin export_alist
GF2_AFF3CT_ROOT="${EXT}/aff3ct" CARGO_TARGET_DIR="${EXT}/survey-target" \
    RUSTFLAGS="-C target-cpu=native" \
    flock -s "${LOCK}" ./scripts/cargo-budget.sh cargo build --release \
    --manifest-path "${HERE}/harness/Cargo.toml"

EXPORT="${REPO}/target/release/export_alist"
MAKE="${EXT}/survey-target/release/ldpc-make-inputs"

# DVB-T2 rate-1/2 normal LDPC at the Es/N0 where the committed gf2-vs-AFF3CT
# BLER comparison crosses FER 1e-2, so the timed cells sit in the waterfall
# where the iteration distribution is informative.
"${EXPORT}" --code dvb-t2-r12 --output "${INPUTS}/dvb_t2_r12.alist"
"${MAKE}" --code dvb-t2-r12 --alist "${INPUTS}/dvb_t2_r12.alist" \
    --esn0-db -1.2 --frames 128 --seed 42 --codeword-source both \
    --output "${INPUTS}/dvb-t2-r12-waterfall"

# 5G NR BG1 lifting 384 mother code at its own waterfall point.
"${EXPORT}" --code nr-bg1-r12 --output "${INPUTS}/nr_bg1_r12.alist"
"${MAKE}" --code nr-bg1-r12 --alist "${INPUTS}/nr_bg1_r12.alist" \
    --esn0-db -4.3 --frames 128 --seed 42 --codeword-source both \
    --output "${INPUTS}/nr-bg1-z384-mother"

echo "input bundles ready under ${INPUTS}"
