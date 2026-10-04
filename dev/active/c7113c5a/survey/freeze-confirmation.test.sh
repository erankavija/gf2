#!/usr/bin/env bash
# Guards of the canonical confirmation freeze's acceptance-schema handling
# (jit:c7113c5a, jit:c5e01de3).
#
# `zen3-benchmark-acceptance-v1` reports the sequential-attempt allocation
# under the single field name `family_alpha`; `zen3-benchmark-acceptance-v2`
# separates that allocation (`attempt_alpha`) from the frozen total
# (`family_alpha`) and the per-comparison corrected level (`corrected_alpha`).
# The freezer keys its legacy handling on the declared schema identity, not on
# which fields happen to be present. These cases run the real freezer over a
# fixture tree that holds only the files it reads:
#
#   1. a v1 pilot summary reproduces the committed 1c602857 confirmation's
#      addendum and derivation record byte for byte, through the legacy
#      branch, using the single-line family accounting the freezer always
#      wrote for a v1 summary;
#   2. a v2 pilot summary derived from the same pilot data labels the frozen
#      total, the attempt allocation and the corrected level distinctly, and
#      never calls the attempt allocation "family-wise alpha";
#   3. `--resolution-decimals 3` rounds the same pilot's widest half-width up
#      to the next 0.001 in the addendum and in the record.
#
# Usage (from the repository root): freeze-confirmation.test.sh
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
[[ "$PWD" == "$repo" ]] || { echo 'invoke from the repository root' >&2; exit 2; }
ISSUE=c7113c5a
FREEZER=dev/active/$ISSUE/survey/freeze-confirmation.py
PILOT=dev/bench_results/1c602857/2026-09-13-1c602857-public-clmul-pilot
PILOT_ADDENDUM=dev/active/1c602857/addendum-v4-public-clmul-pilot.json
CONFIRMATION_ADDENDUM=dev/active/1c602857/addendum-v4-public-clmul-confirmation.json
RECORD=dev/active/1c602857/confirmation-derivation.txt
FROZEN=2026-09-13T06:35:49Z

fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
mkdir -p "$fixture/$(dirname "$FREEZER")" "$fixture/$PILOT" \
  "$fixture/$(dirname "$PILOT_ADDENDUM")" "$fixture/$(dirname "$CONFIRMATION_ADDENDUM")"
cp "$FREEZER" "$fixture/$FREEZER"
cp "$PILOT/receipt.json" "$PILOT/acceptance-summary.json" "$fixture/$PILOT/"
cp "$PILOT_ADDENDUM" "$fixture/$PILOT_ADDENDUM"

freeze() {
  (cd "$fixture" && python3 "$FREEZER" \
    --pilot-addendum "$PILOT_ADDENDUM" --pilot "$PILOT" --frozen-utc "$FROZEN" \
    --output "$1" --record "$2" >/dev/null)
}

# Case 1: the committed pilot summary carries schema v1 as committed.
schema=$(python3 -c "import json; print(json.load(open('$fixture/$PILOT/acceptance-summary.json'))['schema'])")
[[ "$schema" == zen3-benchmark-acceptance-v1 ]] \
  || { echo "case v1 pilot: fixture pilot summary is schema $schema, expected zen3-benchmark-acceptance-v1 (test fixture is stale)" >&2; exit 2; }
freeze "$CONFIRMATION_ADDENDUM" "$RECORD"
cmp "$CONFIRMATION_ADDENDUM" "$fixture/$CONFIRMATION_ADDENDUM" \
  || { echo "case v1 pilot: confirmation addendum differs from the committed bytes" >&2; exit 1; }
cmp "$RECORD" "$fixture/$RECORD" \
  || { echo "case v1 pilot: derivation record differs from the committed bytes" >&2; exit 1; }
grep -q '^family.*attempt alpha 0.025, per-comparison confidence' "$fixture/$RECORD" \
  || { echo "case v1 pilot: legacy single-line family accounting missing" >&2
       grep '^family' "$fixture/$RECORD" >&2; exit 1; }
echo "case v1 pilot: reproduces the committed 1c602857 confirmation byte for byte through the legacy branch"

# Case 2: a v2 pilot summary derived from the same pilot data.
python3 - "$fixture/$PILOT/acceptance-summary.json" <<'PY'
import json, sys
path = sys.argv[1]
document = json.load(open(path))
document["schema"] = "zen3-benchmark-acceptance-v2"
family = document["family"]
family["attempt_alpha"] = family["family_alpha"]
family["family_alpha"] = 0.05
family["corrected_alpha"] = family["attempt_alpha"] / family["comparisons"]
json.dump(document, open(path, "w"), indent=2)
PY
freeze "$fixture/current-confirmation.json" "$fixture/current-record.txt"
grep -q 'family-wise alpha 0.05, attempt alpha 0.025, corrected alpha 0.025, per-comparison confidence 0.975' \
  "$fixture/current-record.txt" \
  || { echo "case v2 pilot: family line does not label all three alphas distinctly" >&2
       grep '^family' "$fixture/current-record.txt" >&2; exit 1; }
if grep -q 'family-wise alpha 0.025' "$fixture/current-record.txt"; then
  echo "case v2 pilot: attempt allocation mislabelled as family-wise alpha" >&2; exit 1
fi
echo "case v2 pilot: labels family-wise, attempt, and corrected alpha distinctly"

# Case 3: a family whose frozen rule rounds to three decimals.
(cd "$fixture" && python3 "$FREEZER" \
  --pilot-addendum "$PILOT_ADDENDUM" --pilot "$PILOT" --frozen-utc "$FROZEN" \
  --output fine-confirmation.json --record fine-record.txt --resolution-decimals 3 >/dev/null)
python3 - "$fixture/fine-confirmation.json" "$fixture/fine-record.txt" <<'PY'
import json, math, re, sys
frozen = json.load(open(sys.argv[1]))["effect"]["measurement_resolution"]
record = open(sys.argv[2]).read()
widest = float(re.search(r"^widest relative half-width\s+(\S+)$", record, re.M).group(1))
if frozen != math.ceil(widest * 1000) / 1000:
    raise SystemExit(f"case three decimals: froze {frozen} from widest {widest}")
if f"frozen measurement resolution {frozen:.3f}\n" not in record:
    raise SystemExit("case three decimals: the record does not state the frozen value")
PY
echo "case three decimals: rounds the widest half-width up to the next 0.001"

echo 'freeze-confirmation.test.sh: every case passed'
