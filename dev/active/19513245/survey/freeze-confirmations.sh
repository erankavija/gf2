#!/usr/bin/env bash
# Freeze the three confirmation addenda from the committed pilot receipts
# (jit:19513245).
#
# Usage: dev/active/19513245/survey/freeze-confirmations.sh <frozen-utc>
#
# Every argument this script passes is the frozen design; the arithmetic is the
# canonical freezer's, `dev/active/c7113c5a/survey/freeze-confirmation.py`,
# which pins the pilot receipt by path and SHA-256, derives the measurement
# resolution from that pilot's widest relative bootstrap half-width, refuses an
# addendum whose margins do not strictly exceed one plus that resolution, and
# writes the derivation record beside the addendum. This script writes no
# resolution and no margin of its own.
#
# The vector and matrix families select six of their pilot cells, the most
# P-20's tail-support bound admits as Bonferroni comparisons in a family's
# first confirmatory attempt; the control family has three cells and confirms
# all of them. `--family-description` restates each family's prose for the
# confirmatory stage, because a pilot's "every cell is exploratory" sentence
# inside a frozen confirmation would describe the wrong stage.
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
[[ "$PWD" == "$repo" ]] || { echo 'invoke from the repository root' >&2; exit 2; }
FROZEN=${1:?usage: freeze-confirmations.sh <YYYY-MM-DDTHH:MM:SSZ>}
ISSUE=19513245
ACTIVE=dev/active/$ISSUE
RECEIPTS=dev/bench_results/$ISSUE
FREEZER=dev/active/c7113c5a/survey/freeze-confirmation.py

# The confirmatory-stage preamble every family description opens with.
STAGE="Confirmatory stage of issue $ISSUE, protocol version 4: every cell of this addendum is \
confirmatory and spends one comparison of its family's budget. The addendum is frozen from the \
family's own committed exploratory pilot receipt, which the effect rule pins by content and which \
supplies the measurement resolution; the margins are the ones declared with that pilot, unchanged. \
The cells are the pilot's own cells with the role changed, so each measures the workload, size, \
seed, cache state, metric kind and arms its pilot measured, on fresh samples at the protocol's \
confirmatory pair count."

# Everything after the stage preamble is the pilot's own description from its
# second sentence on, so the confirmation describes the same question in the
# same words. `jq` is not a dependency of this repository, so python reads it.
question() {
  python3 - "$ACTIVE/addendum-v4-$1-pilot.json" <<'PY'
import json, sys
text = json.load(open(sys.argv[1]))["family"]["description"]
marker = "Question: "
print(text[text.index(marker):], end="")
PY
}

freeze() {
  local short=$1; shift
  local rationale=$1; shift
  local cells=("$@")
  local arguments=()
  for cell in "${cells[@]}"; do arguments+=(--cell "$cell"); done
  if [[ ${#cells[@]} -gt 0 ]]; then
    arguments+=(--selection-rationale "$rationale")
  fi
  python3 "$FREEZER" \
    --pilot-addendum "$ACTIVE/addendum-v4-$short-pilot.json" \
    --pilot "$RECEIPTS/r1-$short-pilot" \
    --frozen-utc "$FROZEN" \
    --output "$ACTIVE/addendum-v4-$short-confirmation.json" \
    --record "$ACTIVE/confirmation-derivation-$short.txt" \
    --family-description "$STAGE $(question "$short")" \
    "${arguments[@]}"
}

freeze vector \
  "The confirmatory budget of a family's first attempt admits six cells, so the confirmation \
spends them on the sweep that answers the question and leaves the rest of the pilot's breadth as \
exploratory evidence. The six are the whole warm cache sweep of the element representation, the \
128 KiB vector cell of the wide representation, and both 128 KiB region cells: between them they \
cover both shapes, both element representations and the three warm cache regimes. The streaming \
regime, the 4 KiB and 8 MiB wide cells and the 4 KiB region cells keep the pilot's exploratory \
strength." \
  axpy-4k-element axpy-128k-element axpy-8m-element axpy-128k-wide \
  region-128k-element region-128k-wide

freeze matrix \
  "The confirmatory budget of a family's first attempt admits six cells. The six are the two \
larger square dimensions for both element representations, which is where a dense product's own \
cost dominates its per-call overhead, and both FieldMatrix-boundary cells, which are the ones that \
decide whether the prototype's conversion eats its gain for a library consumer. The n = 64 cells \
keep the pilot's exploratory strength, because at that dimension the per-call allocation and \
transpose of the current route are a large part of its cost and the cell answers a different \
question." \
  matmul-n256-element matmul-n512-element matmul-n256-wide matmul-n512-wide \
  matmul-n256-whole-element matmul-n256-whole-wide

freeze control ""

for short in vector matrix control; do
  echo "-- $short"
  tail -n 3 "$ACTIVE/confirmation-derivation-$short.txt"
done
