#!/usr/bin/env bash
# Guards of the confirmation freeze (jit:6c6b09b1).
#
# The freeze must write the same bytes whenever it reads the same committed
# pilot receipts, including after the confirmation it froze has run and its
# receipt sits beside the pilot. These cases run the real generator over a
# fixture tree that holds only the files it reads:
#
#   1. the freeze reproduces the committed addenda byte for byte;
#   2. a committed confirmation receipt of the same family changes nothing;
#   3. a pilot-labelled campaign started from the confirmation addendum's own
#      path changes nothing, which is the case the receipt label alone misses;
#   4. a second genuine pilot of one family fails the freeze instead of
#      choosing one silently.
#
# Usage (from the repository root): make-confirmation-addenda.test.sh
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
[[ "$PWD" == "$repo" ]] || { echo 'invoke from the repository root' >&2; exit 2; }
ISSUE=6c6b09b1
SURVEY=dev/active/$ISSUE/survey
RESULTS=dev/bench_results/$ISSUE
ANALYSIS=$repo/target/$ISSUE-analysis/release/survey-analysis
FAMILIES=(region-axpy matrix-product pairwise-control)
FROZEN=2026-09-12T13:20:00Z
[[ -x "$ANALYSIS" ]] || { echo "build survey-analysis first: $ANALYSIS" >&2; exit 2; }

fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
mkdir -p "$fixture/$SURVEY" "$fixture/$RESULTS"
cp "$SURVEY"/make-addenda.py "$SURVEY"/make-confirmation-addenda.py "$fixture/$SURVEY/"
for family in "${FAMILIES[@]}"; do
  stage=$fixture/$RESULTS/v4-r1-$family-pilot
  mkdir -p "$stage/inputs"
  cp "$RESULTS/v4-r1-$family-pilot"/{receipt.json,acceptance-summary.json} "$stage/"
  cp "$RESULTS/v4-r1-$family-pilot/inputs/family-addendum.json" "$stage/inputs/"
done

freeze() { (cd "$fixture" && python3 "$SURVEY/make-confirmation-addenda.py" "$FROZEN" \
            --survey-analysis "$ANALYSIS" >/dev/null); }

compare() {
  for family in "${FAMILIES[@]}"; do
    cmp "dev/active/$ISSUE/addendum-v4-$family-confirmation.json" \
        "$fixture/dev/active/$ISSUE/addendum-v4-$family-confirmation.json" \
      || { echo "case $1: $family differs from the committed addendum" >&2; exit 1; }
  done
  echo "case $1: the three frozen addenda match the committed bytes"
}

# A receipt of the given label and addendum path, carrying nothing else the
# freeze reads: it must be rejected before any field beyond these is needed.
decoy() {
  mkdir -p "$fixture/$RESULTS/$1"
  python3 - "$fixture/$RESULTS/$1/receipt.json" "$2" "$3" "$4" <<'PY'
import json, sys
path, label, family, addendum = sys.argv[1:]
json.dump({"label": label, "family_id": family, "addendum": {"path": addendum}}, open(path, "w"))
PY
}

freeze; compare "committed inputs"

decoy v4-r1-region-axpy-confirmation confirmation byte-field-region-axpy \
  "dev/active/$ISSUE/addendum-v4-region-axpy-confirmation.json"
freeze; compare "a committed confirmation receipt"

decoy v4-r2-region-axpy-relaunch pilot byte-field-region-axpy \
  "dev/active/$ISSUE/addendum-v4-region-axpy-confirmation.json"
freeze; compare "a pilot relaunched from the confirmation addendum"

decoy v4-r2-region-axpy-pilot pilot byte-field-region-axpy \
  "dev/active/$ISSUE/addendum-v4-region-axpy-pilot.json"
if freeze 2>"$fixture/error"; then
  echo 'case a second pilot: the freeze chose one silently' >&2
  exit 1
fi
grep -q 'expected one committed pilot receipt' "$fixture/error" \
  || { echo 'case a second pilot: the freeze failed for another reason' >&2
       cat "$fixture/error" >&2; exit 1; }
echo "case a second pilot: the freeze refuses to choose"

echo 'make-confirmation-addenda.test.sh: every case passed'
