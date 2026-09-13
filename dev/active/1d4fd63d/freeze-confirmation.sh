#!/usr/bin/env bash
# Freeze the transpose-lane confirmation addendum (jit:1d4fd63d).
#
# Usage: ./freeze-confirmation.sh <selected-stage receipt dir> <frozen-utc>
#
# The freezer is the canonical one,
# `dev/active/c7113c5a/survey/freeze-confirmation.py`. It pins the named pilot
# receipt by path and SHA-256, derives the measurement resolution from that
# receipt's own intervals, refuses a margin the resolution does not admit, and
# writes the derivation record beside the addendum. This script holds the
# arguments so the derivation is reproducible from committed bytes rather than
# from a shell history.
#
# The selection drops the identity control, which is the one cell of the
# selected stage that measures no lane difference. P-20 admits at most six
# confirmatory comparisons on this family's first attempt, and the six retained
# cells are the ones a lane decision rests on.

set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
PILOT=${1:?usage: freeze-confirmation.sh <selected-stage receipt dir> <frozen-utc>}
FROZEN_UTC=${2:?usage: freeze-confirmation.sh <selected-stage receipt dir> <frozen-utc>}
ACTIVE=dev/active/1d4fd63d

python3 dev/active/c7113c5a/survey/freeze-confirmation.py \
  --pilot-addendum "$ACTIVE/addendum-v4-transpose-lane-selected.json" \
  --pilot "$PILOT" \
  --frozen-utc "$FROZEN_UTC" \
  --output "$ACTIVE/addendum-v4-transpose-lane-confirmation.json" \
  --record "$ACTIVE/confirmation-derivation.txt" \
  --cell lane-avx2-ymm6-block-256-1core \
  --cell lane-avx2-ymm6-bulk-4096-6core \
  --cell lane-avx2-ymm6-bitslice-absorb-m14-1core \
  --cell lane-avx2-ymm6-bitslice-unpack-m14-1core \
  --cell lane-avx2-ymm6-matrix-transpose-4096-1core \
  --cell lane-avx2-ymm6-matrix-transpose-65-1core \
  --selection-rationale "The identity control measures no lane difference: both of its arms run the production family entry point, which resolves its own block kernel. It pins the consumer latency the conversion cells sit inside, which is a pilot question, and a confirmatory reservation for it would spend one of the six comparisons P-20 admits on this family's first attempt on a cell whose interval is expected to contain one. The six retained cells are the ones a lane decision rests on: the block kernel in isolation and under a streaming run, both halves of the BCH bit-slice conversion, and the whole BitMatrix::transpose consumer at a dense and a partial-tile geometry." \
  --equivalence-margin 1.08 \
  --equivalence-rationale "The family's measured resolution does not admit the five percent margin the selected stage declared, so this is the smallest two-decimal margin that lies strictly outside it. The resolution is set by one cell alone, the bit-slice unpack, whose per-execution values are bimodal inside each arm, so a 24-pair median moves between the modes and that cell's relative bootstrap half-width is several times every other cell's; the derivation record beside this addendum lists all seven. A non-regression cell therefore certifies that the candidate lane is at most eight percent slower at the family confidence, which is the strongest non-inferiority statement this family can resolve." \
  --family-description "Whether gf2 should publish the avx2-ymm6 64x64 bit-block transpose lane in place of the avx2-bit-twiddle lane its dispatch publishes now. The baseline arm is the kernel gf2_kernels_simd::transpose::detect publishes on this host, the pinned pre-change implementation that BitMatrix::transpose and gf2-coding's bit-sliced BCH encoding reach; the candidate arm is the avx2-ymm6 lane, which runs all six mask-shift-XOR stages in YMM registers and declares no stack scratch, named through the public transpose::lane and reaching the consumers through the same abstraction the production kernel does. One executable serves both arms and selects its lane from GF2_TRANSPOSE_LANE, so the arms share a build, a fixture generator, a warm pass and a timing loop. Six confirmatory cells decide it on fresh samples: the block kernel alone over a 256-block L2-resident run and over a 4096-block streaming run on a six-core arm; the two halves of the BCH bit-slice conversion, which are one lane group's whole message absorbed through the transpose and the bit-sliced recurrence and the reduced register read back as packed per-frame parity; and the whole BitMatrix::transpose consumer at 4096 squared and at 65 squared, whose output allocation, tile assembly, partial boundary tiles and tail mask are inside the timed call. The 65-squared cell is a non-regression cell and the other five are improvement cells, so the complete conversion cost with its tile assembly and packing is what the worthwhile threshold applies to. The two lanes this family's ranking stage measured and did not select, avx2-pshufb and avx2-movemask, carry no confirmatory reservation; their ranking outcomes stand as the exploratory evidence that rejected them."

echo "confirmation addendum and derivation record written" >&2
