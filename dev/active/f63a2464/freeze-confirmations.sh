#!/usr/bin/env bash
# Freeze the three QC-family confirmation addenda (jit:f63a2464).
#
# Usage: ./freeze-confirmations.sh <frozen-utc>
#
# The freezer is the canonical one,
# `freeze-confirmation.py` of `c7113c5a`. For each family it pins
# that family's committed pilot receipt by path and SHA-256, derives the
# measurement resolution from that receipt's own intervals, refuses a margin
# the resolution does not admit, and writes the derivation record beside the
# addendum. This script holds the arguments so each derivation is reproducible
# from committed bytes rather than from a shell history.
#
# Every pilot cell becomes confirmatory: the three families declare two, three
# and two cells, and P-20 admits six comparisons on each family's first
# attempt, so no selection is needed and none is made. The margins below are
# fixed from each family's own pilot-measured resolution and from the consumer
# benefit and maintenance cost the decision record states, never from the
# pilots' measured ratios.

set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
repo=$(git -C "$here" rev-parse --show-toplevel)
cd "$repo"
FROZEN_UTC=${1:?usage: freeze-confirmations.sh <frozen-utc>}
ACTIVE=$(realpath --relative-to="$repo" "$here")
# The results directory is where the pilot addenda place their family ledgers.
RESULTS=$(python3 -c 'import json,os,sys; print(os.path.dirname(json.load(open(sys.argv[1]))["family_wise"]["ledger_path"]))' \
  "$ACTIVE/addendum-ldpc-qc-intra-frame-single-worker-pilot.json")
files=$(git ls-files --cached --others --exclude-standard -- ':(glob)**/repository_files.py')
FREEZER=$(python3 -B "$files" document freeze-confirmation.py \
  $'#!/usr/bin/env python3\n"""Freeze a confirmation addendum from its accepted pilot.')

SHARED="The steady-state operation of \`3be770d5\`, unchanged: every worker is a thread pinned to one resolved CPU and owns one decoder built before timing. AList parsing and construction are reported as setup, outside the timed windows. A timed call makes every worker decode its declared batch of recorded frames of fixture bank zero, including conversion of the recorded f32 LLRs, the dispatch to the workers and extraction of the information-window decisions. The candidate is the QC-aware intra-frame prototype of \`f63a2464\`, which declares the canonical numerical contract unchanged: f32 flooding normalized min-sum at factor 0.75, iteration cap 50, syndrome stopping, one frame per decoder invocation, vectorized across the lifted positions of one frame's circulant blocks rather than across frames, so it fills no batch. The \`c077a88b\` prepared 128-frame quality evidence is reused without resampling and every timed execution checks each worker's per-frame bit errors against it, so a candidate that parted from the contract fails the run. The DVB-T2 workload declares no cell in this family: its check rows carry no equal-degree circulant block partition, which \`f63a2464\`'s numerical-contract review records with its evidence. Arms report observed per-worker affinity, CPUs and process thread counts; no nested pool is declared."

python3 "$FREEZER" \
  --pilot-addendum "$ACTIVE/addendum-ldpc-qc-intra-frame-single-worker-pilot.json" \
  --pilot "$RESULTS/v4-r1-f63a2464-ldpc-qc-intra-frame-single-worker-pilot" \
  --frozen-utc "$FROZEN_UTC" \
  --output "$ACTIVE/addendum-ldpc-qc-intra-frame-single-worker.json" \
  --record "$ACTIVE/resolution-ldpc-qc-intra-frame-single-worker.txt" \
  --worthwhile-speedup 1.10 \
  --worthwhile-rationale "The candidate carries a circulant-block layout, a QC-aware flooding decoder over it and one unsafe AVX2 check kernel with its scalar reference, which is the whole complexity budget this family declares. A tenth of the decode time is the smallest gain that repays maintaining that second decoder and its kernel beside the canonical one: below it a consumer of the decoder sees the same sustained frame rate and the same single-frame latency, and the canonical decoder keeps the workload. The pilot resolves this family to one percent, so a tenth lies far outside what the measurement can confuse with no change." \
  --equivalence-margin 1.02 \
  --equivalence-rationale "A cell that gains nothing must not cost anything. The pilot resolves this family's whole-consumer cells to one percent, so one percent cannot separate a candidate that costs nothing from one that costs something; two percent is the smallest two-decimal margin that lies strictly outside that resolution, and it stays far below the gain the worthwhile threshold requires." \
  --family-description "Confirmatory stage: every cell is confirmatory and decides this family on fresh samples under the margins this addendum freezes from the pilot receipt it pins. The canonical gf2 decoder against the QC-aware intra-frame candidate on the NR BG1 lifting-384 mother code, on a bit-identical parity-check matrix and bit-identical recorded LLRs under one numerical contract. This family asks the per-core question: one worker, no sharing, at two granularities, sustained throughput over a batch of frames and the latency of a single frame, which is the axis inter-frame batching cannot improve. $SHARED Two confirmatory cells decide it, both improvement cells against the canonical decoder: sustained throughput and single-frame latency."

python3 "$FREEZER" \
  --pilot-addendum "$ACTIVE/addendum-ldpc-qc-intra-frame-multicore-pilot.json" \
  --pilot "$RESULTS/v4-r1-f63a2464-ldpc-qc-intra-frame-multicore-pilot" \
  --frozen-utc "$FROZEN_UTC" \
  --output "$ACTIVE/addendum-ldpc-qc-intra-frame-multicore.json" \
  --record "$ACTIVE/resolution-ldpc-qc-intra-frame-multicore.txt" \
  --worthwhile-speedup 1.10 \
  --worthwhile-rationale "The consumer rule is this issue's single-worker rule applied to the shared-memory arms: a tenth of the decode time is the smallest gain that repays maintaining a second decoder and its unsafe AVX2 kernel beside the canonical one, because below it a consumer decoding on every core sees the same sustained frame rate. The pilot resolves this family to five percent, set by the twenty-four-logical-CPU cell where two threads share one core's memory pipeline, and a tenth lies strictly outside that resolution." \
  --equivalence-margin 1.06 \
  --equivalence-rationale "A core arm that gains nothing must not cost anything. The pilot resolves this family to five percent, so five percent cannot separate a candidate that costs nothing from one that costs something; six percent is the smallest two-decimal margin that lies strictly outside that resolution, and it stays below the gain the worthwhile threshold requires." \
  --family-description "Confirmatory stage: every cell is confirmatory and decides this family on fresh samples under the margins this addendum freezes from the pilot receipt it pins. The same two arms decoding independent frames on several workers, which decides how the candidate shares the memory system rather than deciding adoption on its own. $SHARED Three confirmatory cells decide it, all improvement cells against the canonical decoder: six physical cores, twelve physical cores and twenty-four logical CPUs."

python3 "$FREEZER" \
  --pilot-addendum "$ACTIVE/addendum-ldpc-qc-comparator-single-worker-pilot.json" \
  --pilot "$RESULTS/v4-r1-f63a2464-ldpc-qc-comparator-single-worker-pilot" \
  --frozen-utc "$FROZEN_UTC" \
  --output "$ACTIVE/addendum-ldpc-qc-comparator-single-worker.json" \
  --record "$ACTIVE/resolution-ldpc-qc-comparator-single-worker.txt" \
  --material-gap-threshold 1.20 \
  --material-gap-rationale "A fifth of the runtime is the smallest residual gap against the pinned external arm that would justify further kernel work on this candidate rather than a reported measurement; a smaller gap is reported and left, because it changes no decision a consumer of the gf2 decoder makes. The pilot resolves this family to five percent, so a fifth lies far outside what the measurement can confuse with parity." \
  --equivalence-margin 1.06 \
  --equivalence-rationale "Parity with the matched external arm means the candidate is at most this factor slower at the family confidence. The pilot resolves this family to five percent, set by the sustained-throughput cell, so six percent is the smallest two-decimal margin that lies strictly outside that resolution." \
  --family-description "Confirmatory stage: every cell is confirmatory and decides this family on fresh samples under the material-gap threshold this addendum freezes from the pilot receipt it pins. The QC-aware intra-frame candidate against the matched external arm, AFF3CT v4.7.0's scalar f32 flooding normalized-min-sum decoder [Cassagne2019], under the same contract on the same recorded frames. It decides whether the residual gap against that arm is material and selects nothing for production. The fastest quality-compatible external arm is a separate question whose shortlist \`c077a88b\` closed empty: no surveyed candidate satisfied the paired quality-admission rule on this corpus, and that outcome is preserved rather than replaced by a matched arm. $SHARED Two confirmatory cells decide it, both comparator-gap cells: sustained throughput and single-frame latency."

echo 'three confirmation addenda and derivation records written' >&2
