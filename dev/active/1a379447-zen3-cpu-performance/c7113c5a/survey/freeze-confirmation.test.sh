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
#      addendum and record bytes (`rederive-confirmation.py`), except a record
#      frozen under a retained-cells floor, whose resolution lies below the
#      whole-pilot half-width acceptance verifies under P-03 and which the
#      freezer therefore refuses to freeze again;
#   5. a supplied resolution that covers the selected cells and lies below
#      the whole pilot's half-width is refused, so no addendum is written
#      that acceptance rejects on its declared resolution;
#   6. `rederive-confirmation.py` takes its inputs by content identity: in a
#      scratch repository it re-derives a record whose files all lie at other
#      paths than the record names, ignores same-named files with other bytes
#      at the recorded paths, and refuses, naming the digest, once no file
#      holds the recorded receipt.
#
# Usage (from the repository root): freeze-confirmation.test.sh
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
[[ "$PWD" == "$repo" ]] || { echo 'invoke from the repository root' >&2; exit 2; }
ISSUE=c7113c5a
FREEZER=dev/active/1a379447-zen3-cpu-performance/$ISSUE/survey/freeze-confirmation.py
PILOT=dev/bench_results/1c602857/2026-09-13-1c602857-public-clmul-pilot
PILOT_ADDENDUM=dev/active/1c602857/addendum-v4-public-clmul-pilot.json
CONFIRMATION_ADDENDUM=dev/active/1c602857/addendum-v4-public-clmul-confirmation.json
RECORD=dev/active/1c602857/confirmation-derivation.txt
FROZEN=2026-09-13T06:35:49Z
# The three paths above are the ones the committed bytes record. The fixture
# keeps that layout; the files lie beside the one record that names the
# confirmation addendum.
live=$(dirname "$(git grep -l -F -e "confirmation addendum $CONFIRMATION_ADDENDUM" -- '*.txt' ':(exclude,glob)**/inputs/**')")

fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
mkdir -p "$fixture/$(dirname "$FREEZER")" "$fixture/$PILOT" \
  "$fixture/$(dirname "$PILOT_ADDENDUM")" "$fixture/$(dirname "$CONFIRMATION_ADDENDUM")"
cp "$FREEZER" "$fixture/$FREEZER"
cp "$PILOT/receipt.json" "$PILOT/acceptance-summary.json" "$fixture/$PILOT/"
cp "$live/${PILOT_ADDENDUM##*/}" "$fixture/$PILOT_ADDENDUM"

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
cmp "$live/${CONFIRMATION_ADDENDUM##*/}" "$fixture/$CONFIRMATION_ADDENDUM" \
  || { echo "case v1 pilot: confirmation addendum differs from the committed bytes" >&2; exit 1; }
cmp "$live/${RECORD##*/}" "$fixture/$RECORD" \
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

# Case 4: every committed confirmation re-derives byte for byte. A record
# frozen under a retained-cells floor is a preserved falsified freeze: the
# freezer must refuse it at the whole-pilot floor.
REDERIVE="$(dirname "$FREEZER")/rederive-confirmation.py"
RETAINED='^floor scope  *retained cells$'
mapfile -t all_records < <(git grep -l '^frozen measurement resolution' -- '*.txt' ':(exclude,glob)**/inputs/**')
records=() falsified=()
for record in "${all_records[@]}"; do
  if grep -q "$RETAINED" "$record"; then falsified+=("$record"); else records+=("$record"); fi
done
python3 -B "$REDERIVE" "$fixture/rederived" "${records[@]}" >"$fixture/rederived.txt" \
  || { echo "case committed records: a committed confirmation no longer re-derives" >&2
       grep -v '^same ' "$fixture/rederived.txt" >&2; exit 1; }
for record in "${falsified[@]}"; do
  if python3 -B "$REDERIVE" "$fixture/falsified" "$record" >"$fixture/falsified.txt"; then
    echo "case committed records: $record freezes below the whole-pilot floor" >&2; exit 1
  fi
  grep -q 'freezer refused: supplied resolution .* is below the pilot-alpha floor' "$fixture/falsified.txt" \
    || { echo "case committed records: unexpected refusal" >&2; cat "$fixture/falsified.txt" >&2; exit 1; }
done
echo "case committed records: ${#records[@]} committed confirmations re-derive byte for byte;" \
  "${#falsified[@]} frozen below the whole-pilot floor refused"

# Case 5 selects the five narrow cells of the same pilot; its sixth cell is the
# widest, so a resolution that covers the selection lies below the whole pilot's.
SELECTED=(clmul-wide-9w-owned clmul-wide-4w-accumulate clmul-wide-1w-dispatch-overhead
          clmul-wide-2w-dispatch-overhead clmul-wide-16w-dispatch-overhead)
echo 'resolution over the selected cells' >"$fixture/derivation.md"
# freeze_selected <resolution> <prefix>
freeze_selected() {
  local cells=() cell
  for cell in "${SELECTED[@]}"; do cells+=(--cell "$cell"); done
  (cd "$fixture" && python3 "$FREEZER" \
    --pilot-addendum "$PILOT_ADDENDUM" --pilot "$PILOT" --frozen-utc "$FROZEN" \
    --output "$2.json" --record "$2.txt" --resolution-decimals 3 \
    --resolution "$1" --resolution-derivation derivation.md \
    --selection-rationale 'the narrow cells' "${cells[@]}" >/dev/null 2>"$2.err")
}
if freeze_selected 0.010 below; then
  echo "case whole-pilot floor: a resolution below the whole pilot's half-width was frozen" >&2; exit 1
fi
grep -q 'below the pilot-alpha floor 0.018' "$fixture/below.err" && [[ ! -e "$fixture/below.json" ]] \
  || { echo "case whole-pilot floor: unexpected refusal" >&2; cat "$fixture/below.err" >&2; exit 1; }
freeze_selected 0.018 whole \
  || { echo "case whole-pilot floor: the whole-pilot width was refused" >&2
       cat "$fixture/whole.err" >&2; exit 1; }
echo "case whole-pilot floor: a selection's resolution below the whole pilot's half-width is refused"

# Case 6: a scratch repository holds the 1c602857 confirmation's files under
# `moved/`, so no recorded path exists in it.
scratch="$fixture/scratch"
mkdir -p "$scratch/tools" "$scratch/moved/pilot"
git -C "$scratch" init -q
cp "$FREEZER" "$REDERIVE" "$scratch/tools/"
cp "$PILOT/receipt.json" "$PILOT/acceptance-summary.json" "$scratch/moved/pilot/"
cp "$live/${PILOT_ADDENDUM##*/}" "$live/${CONFIRMATION_ADDENDUM##*/}" "$live/${RECORD##*/}" "$scratch/moved/"
git -C "$scratch" add -A
# rederive_scratch <label>
rederive_scratch() {
  (cd "$scratch" && python3 -B tools/rederive-confirmation.py "$fixture/scratch-$1" \
    "moved/${RECORD##*/}" >"$fixture/scratch-$1.txt")
}
rederive_scratch relocated && grep -q '^same ' "$fixture/scratch-relocated.txt" \
  || { echo "case content identity: relocated inputs were not found by digest" >&2
       cat "$fixture/scratch-relocated.txt" >&2; exit 1; }
mkdir -p "$scratch/$PILOT" "$scratch/$(dirname "$PILOT_ADDENDUM")"
for decoy in "$PILOT/receipt.json" "$PILOT/acceptance-summary.json" "$PILOT_ADDENDUM" "$CONFIRMATION_ADDENDUM"; do
  echo '{"decoy": true}' >"$scratch/$decoy"
done
git -C "$scratch" add -A
rederive_scratch decoy && grep -q '^same ' "$fixture/scratch-decoy.txt" \
  || { echo "case content identity: a same-named file with other bytes was read" >&2
       cat "$fixture/scratch-decoy.txt" >&2; exit 1; }
git -C "$scratch" rm -q --cached moved/pilot/receipt.json
if rederive_scratch missing; then
  echo "case content identity: re-derived without the recorded receipt" >&2; exit 1
fi
grep -q "no committed file has sha256 $(sha256sum "$PILOT/receipt.json" | cut -d' ' -f1) (pilot receipt)" \
  "$fixture/scratch-missing.txt" \
  || { echo "case content identity: unexpected refusal" >&2; cat "$fixture/scratch-missing.txt" >&2; exit 1; }
echo "case content identity: relocated inputs found by digest, decoys ignored, a missing receipt refused"

echo 'freeze-confirmation.test.sh: every case passed'

