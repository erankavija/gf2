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
#      to the next 0.001 in the addendum and in the record;
#   4. every committed derivation record freezes again to its committed
#      addendum and record bytes (`rederive-confirmation.py`);
#   5. a frozen rule the pilot receipt pins, declaring the confirmatory cells
#      and the resolution rule over them, floors a supplied resolution at the
#      retained cells' width, and the record names the excluded cells;
#   6. a supplied resolution below that retained-cells floor is refused;
#   7. a rule the pilot receipt does not pin, and a selection other than the
#      one the rule declares, keep the whole-pilot floor.
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

# Case 4: every committed confirmation re-derives byte for byte.
mapfile -t records < <(git grep -l '^frozen measurement resolution' -- 'dev/active/*.txt')
python3 -B "$(dirname "$FREEZER")/rederive-confirmation.py" "$fixture/rederived" "${records[@]}" \
  >"$fixture/rederived.txt" \
  || { echo "case committed records: a committed confirmation no longer re-derives" >&2
       grep -v '^same ' "$fixture/rederived.txt" >&2; exit 1; }
echo "case committed records: ${#records[@]} committed confirmations re-derive byte for byte"

# Cases 5 to 7 freeze the five narrow cells of the same pilot; its sixth cell
# is the widest. The rule document exists only in the fixture, and the pinned
# variant of the pilot records its digest the way a receipt pins a producing
# input.
RULE=rule.md
SELECTED=(clmul-wide-9w-owned clmul-wide-4w-accumulate clmul-wide-1w-dispatch-overhead
          clmul-wide-2w-dispatch-overhead clmul-wide-16w-dispatch-overhead)
{
  echo 'The pilot-derived resolution is the largest relative bootstrap half-width'
  echo 'over the five confirmatory cells at the corrected alpha.'
  echo
  echo '| Cell | Role in the confirmation |'
  echo '|---|---|'
  for cell in "${SELECTED[@]}"; do echo "| \`$cell\` | confirmatory |"; done
} >"$fixture/$RULE"
PINNED=pinned-pilot
mkdir "$fixture/$PINNED"
python3 - "$fixture" "$PILOT" "$PINNED" "$RULE" <<'PY'
import hashlib, json, sys
fixture, pilot, pinned, rule = sys.argv[1:]
receipt = json.load(open(f"{fixture}/{pilot}/receipt.json"))
digest = hashlib.sha256(open(f"{fixture}/{rule}", "rb").read()).hexdigest()
receipt["source"]["producing"]["behavior_sha256"][rule] = digest
encoded = json.dumps(receipt).encode()
open(f"{fixture}/{pinned}/receipt.json", "wb").write(encoded)
summary = json.load(open(f"{fixture}/{pilot}/acceptance-summary.json"))
summary["receipt_sha256"] = hashlib.sha256(encoded).hexdigest()
json.dump(summary, open(f"{fixture}/{pinned}/acceptance-summary.json", "w"))
PY
# freeze_rule <pilot> <resolution> <prefix> <cell>...
freeze_rule() {
  local pilot=$1 resolution=$2 prefix=$3 cells=() cell
  shift 3
  for cell in "$@"; do cells+=(--cell "$cell"); done
  (cd "$fixture" && python3 "$FREEZER" \
    --pilot-addendum "$PILOT_ADDENDUM" --pilot "$pilot" --frozen-utc "$FROZEN" \
    --output "$prefix.json" --record "$prefix.txt" --resolution-decimals 3 \
    --resolution "$resolution" --resolution-derivation "$RULE" \
    --selection-rationale 'the frozen rule names these cells' "${cells[@]}" \
    --frozen-rule "$RULE" >/dev/null 2>"$prefix.err")
}

freeze_rule "$PINNED" 0.010 retained "${SELECTED[@]}" \
  || { echo "case retained floor: the pinned rule's resolution was refused" >&2
       cat "$fixture/retained.err" >&2; exit 1; }
grep -q '^floor scope                  retained cells$' "$fixture/retained.txt" \
  && grep -q '^excluded from floor          clmul-wide-4w-owned ' "$fixture/retained.txt" \
  && [[ $(grep -c '^floor cell ' "$fixture/retained.txt") == "${#SELECTED[@]}" ]] \
  || { echo "case retained floor: the record does not state the floor's scope and exclusions" >&2
       cat "$fixture/retained.txt" >&2; exit 1; }
echo "case retained floor: a pinned frozen rule floors the resolution at its confirmatory cells"

if freeze_rule "$PINNED" 0.005 below "${SELECTED[@]}"; then
  echo "case below retained floor: a resolution below the retained cells' width was frozen" >&2; exit 1
fi
grep -q 'below the pilot-alpha floor 0.007' "$fixture/below.err" \
  || { echo "case below retained floor: unexpected refusal" >&2; cat "$fixture/below.err" >&2; exit 1; }
echo "case below retained floor: refused"

if freeze_rule "$PILOT" 0.010 unpinned "${SELECTED[@]}"; then
  echo "case unpinned rule: a rule the pilot does not pin narrowed the floor" >&2; exit 1
fi
grep -q 'below the pilot-alpha floor 0.018' "$fixture/unpinned.err" \
  || { echo "case unpinned rule: unexpected refusal" >&2; cat "$fixture/unpinned.err" >&2; exit 1; }
if freeze_rule "$PINNED" 0.010 other "${SELECTED[@]:1}"; then
  echo "case undeclared selection: a selection the rule does not declare narrowed the floor" >&2; exit 1
fi
grep -q 'below the pilot-alpha floor 0.018' "$fixture/other.err" \
  || { echo "case undeclared selection: unexpected refusal" >&2; cat "$fixture/other.err" >&2; exit 1; }
freeze_rule "$PILOT" 0.018 whole "${SELECTED[@]}" \
  && grep -q '^floor scope                  whole pilot: ' "$fixture/whole.txt" \
  || { echo "case unpinned rule: the record does not state the whole-pilot floor" >&2; exit 1; }
echo "case unpinned rule: an unpinned rule and an undeclared selection keep the whole-pilot floor"

echo 'freeze-confirmation.test.sh: every case passed'
