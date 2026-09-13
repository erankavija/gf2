#!/usr/bin/env bash
# Freeze the comparator confirmation addendum (jit:1d4fd63d).
#
# Usage: ./freeze-external-confirmation.sh <comparator pilot receipt dir> <frozen-utc>
#
# The freezer is the canonical one,
# `dev/active/c7113c5a/survey/freeze-confirmation.py`. It pins the named pilot
# receipt by path and SHA-256, derives the measurement resolution from that
# receipt's own intervals, refuses a margin the resolution does not admit, and
# writes the derivation record beside the addendum. This script holds the
# arguments so the derivation reproduces from committed bytes.
#
# The selection keeps the two cells of the lane the sibling family selects.
# P-20 admits at most two comparisons here at a rate that carries twenty
# expected draws per bootstrap tail, and those two are the ones a statement
# about the lane gf2 would publish rests on; the other six cells' pilot
# outcomes stand as the exploratory evidence about the lanes it would not.

set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
PILOT=${1:?usage: freeze-external-confirmation.sh <pilot receipt dir> <frozen-utc>}
FROZEN_UTC=${2:?usage: freeze-external-confirmation.sh <pilot receipt dir> <frozen-utc>}
ACTIVE=dev/active/1d4fd63d

python3 dev/active/c7113c5a/survey/freeze-confirmation.py \
  --pilot-addendum "$ACTIVE/addendum-v4-external-pilot.json" \
  --pilot "$PILOT" \
  --frozen-utc "$FROZEN_UTC" \
  --output "$ACTIVE/addendum-v4-external-confirmation.json" \
  --record "$ACTIVE/external-confirmation-derivation.txt" \
  --cell lane-avx2-ymm6-block-64-vs-m4ri \
  --cell lane-avx2-ymm6-block-64-vs-bitshuffle \
  --selection-rationale "P-20 carries twenty expected draws per bootstrap tail on this family's first attempt only up to two comparisons, so at most two cells can be confirmatory. The two retained are the lane the sibling transpose-lane-selection family measures as its candidate, against each comparator: a confirmatory comparator-gap decision about the lane gf2 would publish is the one this family exists to supply. The six dropped cells keep their pilot outcomes as exploratory evidence about the lanes that are not candidates for publication, and the resolution this addendum freezes is derived from all eight." \
  --equivalence-margin 1.12 \
  --equivalence-rationale "The family's measured resolution does not admit the ten percent margin the pilot declared, so this is the smallest two-decimal margin that lies strictly outside it. The resolution is set by one cell alone, the production lane against Bitshuffle, whose gf2 per-execution values are a tight mode with occasional excursions well above it, so the median of a 24-pair sample moves and that cell's relative bootstrap half-width is several times every other cell's; the derivation record beside this addendum lists all eight. The decisions this margin governs are unaffected in direction: every cell's interval lies far from both margins." \
  --family-description "Whether a material gap separates the avx2-ymm6 64x64 bit-block transpose lane from the two external comparators at the kernel geometry 6fb89a3c froze: one 64x64 block per call into a preallocated output, with no tail and no geometry adapter, because one block of 64 eight-byte elements is the canonical gf2 layout for both comparators. The baseline arm of each cell is the avx2-ymm6 lane and the candidate is the external arm, so the speedup of medians is median(gf2) / median(external) and a value above one means the external arm is ahead; improved means a material gap in the external arm's favour that needs attribution and regressed means gf2 is materially ahead. The gf2 arm selects its lane from GF2_TRANSPOSE_LANE through the public transpose::lane. The external arms are 6fb89a3c's pinned executables, rebuilt by its committed fetch-build.sh and checked byte for byte against its committed build-evidence.json before any timed run: M4RI 20260122 mzd_transpose into a preallocated output, and Bitshuffle 0.5.2 bshuf_bitshuffle over 64 eight-byte elements. Every arm reads the same fixture, 64 words of SplitMix64 seeded at the cell's seed, and this issue's verify-bit-mapping.py checks every one of them against naive bit arithmetic at five seeds before any timing. Adapter costs are zero at this geometry and are not hidden: the geometries that do pay an adapter, 63 and 65 rows, are 6fb89a3c's own consumer cells and that survey carries them. Two confirmatory cells decide the question on fresh samples; this family adopts no implementation and selects nothing."

echo "comparator confirmation addendum and derivation record written" >&2
