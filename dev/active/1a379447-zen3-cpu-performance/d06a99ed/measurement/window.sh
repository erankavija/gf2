#!/usr/bin/env bash
# Existing pilot/freezer/confirmation commands for the overnight window.
set -euo pipefail

if [[ $# -ne 2 ]]; then
    echo "usage: bash window.sh SURVEY_DIRECTORY CANONICAL_FREEZER" >&2
    exit 2
fi
repo=$(git rev-parse --show-toplevel)
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
survey=$(realpath "$1")
freezer=$(realpath "$2")
cd "$repo"
before="$here/before-build.json"
after="$here/after-build.json"
pilot="dev/bench_results/d06a99ed/worker-routing/d06a99ed-worker-routing-pilot"

python3 "$survey/run-family-campaign.py" "$here/pilot/plan.json" "$before" "$after"
mkdir -p "$here/confirmation"
if [[ ! -f "$here/confirmation/addendum.json" || ! -f "$here/confirmation/derivation.txt" ]]; then
    frozen_utc=$(python3 - "$here/confirmation/addendum.json" <<'PYTIME'
import datetime, json, pathlib, sys
path = pathlib.Path(sys.argv[1])
print(json.loads(path.read_text())["frozen"]["frozen_utc"] if path.exists() else
      datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"))
PYTIME
)
    freeze_metadata=$(python3 - "$here/declaration.json" <<'PYDATA'
import json, sys
with open(sys.argv[1]) as handle:
    declaration = json.load(handle)
effect = declaration["effect"]
for key in ("worthwhile_speedup", "rationale", "equivalence_margin", "equivalence_rationale"):
    print(effect[key])
print(declaration["family"]["description"].replace("Pilot outcomes are exploratory. ", ""))
PYDATA
)
    mapfile -t freeze_values <<< "$freeze_metadata"
    python3 "$freezer" --pilot-addendum "$here/pilot/addendum.json" --pilot "$pilot" \
        --frozen-utc "$frozen_utc" \
        --worthwhile-speedup "${freeze_values[0]}" --worthwhile-rationale "${freeze_values[1]}" \
        --equivalence-margin "${freeze_values[2]}" --equivalence-rationale "${freeze_values[3]}" \
        --family-description "${freeze_values[4]}" \
        --output "$here/confirmation/addendum.json" --record "$here/confirmation/derivation.txt"
fi
if [[ ! -f "$here/confirmation/plan.json" ]]; then
    python3 "$here/prepare.py" "$survey/make-family.py" "$before" "$after" \
        "$here/confirmation" --confirmation
fi

# The runner requires a committed addendum. Commit only this freeze's outputs;
# unrelated staged files and the pilot's mutable run records stay untouched.
freeze_files=(
    "$here/confirmation/addendum.json"
    "$here/confirmation/plan.json"
    "$here/confirmation/derivation.txt"
)
git add -- "${freeze_files[@]}"
if ! git diff --quiet HEAD -- "${freeze_files[@]}"; then
    git commit --only -m 'docs(jit:d06a99ed): freeze worker-routing confirmation' \
        -- "${freeze_files[@]}"
fi

python3 "$survey/run-family-campaign.py" "$here/confirmation/plan.json" "$before" "$after"
