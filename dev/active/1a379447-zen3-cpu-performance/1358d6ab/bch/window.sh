#!/usr/bin/env bash
# Campaigns of family 1362381c-bch-integration (jit:1358d6ab).
#
# Usage (from the worktree root):
#   bash window.sh plan pilot|confirmation ADDENDUM OUTPUT
#   bash window.sh window SURVEY_DIRECTORY CANONICAL_FREEZER
#
# `plan` projects a stage's runner plan over the arms `build-identity.json`
# pins; it times nothing. `window` runs in the benchmark window: the pilot, the
# canonical freeze, a commit of the freeze outputs alone, and the confirmation,
# each through the story launcher `run-family-campaign.py` in SURVEY_DIRECTORY,
# which resumes an interrupted campaign under its own identity. A frozen
# confirmation is reused, never re-derived.
set -euo pipefail

repo=$(git rev-parse --show-toplevel)
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
cd "$repo"
export PATH="$HOME/.cargo/bin:$PATH"
rel=$(realpath --relative-to="$repo" "$here")
build="$here/build-identity.json"

project() {
    local label=$1 addendum=$2 output=$3 seed pairs=()
    case "$label" in
        pilot) seed=1358101 pairs=(--pilot-pairs 24) ;;
        confirmation) seed=1358102 ;;
        *) echo "stage must be pilot or confirmation" >&2; exit 2 ;;
    esac
    executable=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["arms"]["consumer-arm"]["path"])' "$build")
    python3 -B "$here/make-plan.py" --addendum "$addendum" --label "$label" \
        --campaign-id "1358d6ab-bch-integration-$label" --campaign-seed "$seed" \
        --lock "${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}" --executable "$repo/$executable" \
        --producing-manifest "$rel/producing-inputs.json" --max-cells-per-session 1 \
        "${pairs[@]}" --output "$output"
}

case "${1:-}" in
    plan)
        [[ $# -eq 4 ]] || { echo "usage: bash window.sh plan STAGE ADDENDUM OUTPUT" >&2; exit 2; }
        project "$2" "$3" "$4"
        exit 0 ;;
    window)
        [[ $# -eq 3 ]] || { echo "usage: bash window.sh window SURVEY_DIRECTORY CANONICAL_FREEZER" >&2; exit 2; } ;;
    *) echo "action must be plan or window" >&2; exit 2 ;;
esac
survey=$(realpath "$2")
freezer=$(realpath "$3")

python3 "$survey/run-family-campaign.py" "$here/pilot/plan.json" "$build"

# Repository-relative receipt directory of the pilot, ledger and family, from the addendum.
mapfile -t stage < <(python3 - "$here/pilot/addendum.json" "$here/pilot/plan.json" <<'PYSTAGE'
import json, pathlib, sys
addendum = json.loads(pathlib.Path(sys.argv[1]).read_text())
plan = json.loads(pathlib.Path(sys.argv[2]).read_text())
ledger = pathlib.PurePosixPath(addendum["family_wise"]["ledger_path"])
print(ledger.parent / plan["campaign_id"])
print(ledger)
print(addendum["family"]["id"])
PYSTAGE
)
target/release/benchmark-acceptance --next-attempt "${stage[1]}" "${stage[2]}"

mkdir -p "$here/confirmation"
if [[ ! -f "$here/confirmation/addendum.json" || ! -f "$here/confirmation/derivation.txt" ]]; then
    description=$(cd "$here" && python3 -B -c 'import importlib; print(importlib.import_module("make-pilot-addendum").CONFIRMATION_DESCRIPTION)')
    python3 "$freezer" --pilot-addendum "$here/pilot/addendum.json" --pilot "${stage[0]}" \
        --frozen-utc "$(date -u +%Y-%m-%dT%H:%M:%SZ)" --family-description "$description" \
        --output "$here/confirmation/addendum.json" --record "$here/confirmation/derivation.txt"
fi
project confirmation "$rel/confirmation/addendum.json" "$here/confirmation/plan.json"

# The runner requires a committed addendum. Commit only this freeze's outputs; unrelated staged
# files and the pilot's run records stay untouched.
freeze_files=(
    "$here/confirmation/addendum.json"
    "$here/confirmation/plan.json"
    "$here/confirmation/derivation.txt"
)
git add -- "${freeze_files[@]}"
if ! git diff --quiet HEAD -- "${freeze_files[@]}"; then
    git commit --only -m 'docs(jit:1358d6ab): freeze BCH integration confirmation' \
        -- "${freeze_files[@]}"
fi

python3 "$survey/run-family-campaign.py" "$here/confirmation/plan.json" "$build"
