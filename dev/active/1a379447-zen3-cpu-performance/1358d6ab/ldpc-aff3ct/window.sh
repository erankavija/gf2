#!/usr/bin/env bash
# Pilot, canonical freeze and confirmation of family 1362381c-ldpc-aff3ct-integration in the
# benchmark window. Rerunning resumes the stage that was interrupted; a frozen confirmation is
# reused, never re-derived.
set -euo pipefail

if [[ $# -ne 3 ]]; then
    echo "usage: bash window.sh SURVEY_DIRECTORY CANONICAL_FREEZER BUILD_IDENTITY" >&2
    exit 2
fi
repo=$(git rev-parse --show-toplevel)
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
survey=$(realpath "$1")
freezer=$(realpath "$2")
build=$(realpath "$3")
cd "$repo"
export PATH="$HOME/.cargo/bin:$PATH"
declaration="$survey/families/1362381c-ldpc-aff3ct-integration.json"

# The generator writes the same addendum bytes from the same declaration; it restores the
# git-ignored bundles the plans name.
python3 "$survey/make-family.py" "$declaration" "$here/pilot" >/dev/null
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
pilot=${stage[0]}
target/release/benchmark-acceptance --next-attempt "${stage[1]}" "${stage[2]}"

mkdir -p "$here/confirmation"
if [[ ! -f "$here/confirmation/addendum.json" || ! -f "$here/confirmation/derivation.txt" ]]; then
    description=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["confirmation"]["family_description"])' "$declaration")
    python3 "$freezer" --pilot-addendum "$here/pilot/addendum.json" --pilot "$pilot" \
        --frozen-utc "$(date -u +%Y-%m-%dT%H:%M:%SZ)" --family-description "$description" \
        --output "$here/confirmation/addendum.json" --record "$here/confirmation/derivation.txt"
fi
python3 "$survey/make-family.py" "$declaration" "$here/confirmation" --confirmation >/dev/null

# The runner requires a committed addendum. Commit only this freeze's outputs; unrelated staged
# files and the pilot's run records stay untouched.
freeze_files=(
    "$here/confirmation/addendum.json"
    "$here/confirmation/plan.json"
    "$here/confirmation/derivation.txt"
)
git add -- "${freeze_files[@]}"
if ! git diff --quiet HEAD -- "${freeze_files[@]}"; then
    git commit --only -m 'docs(jit:1358d6ab): freeze LDPC AFF3CT integration confirmation' \
        -- "${freeze_files[@]}"
fi

python3 "$survey/run-family-campaign.py" "$here/confirmation/plan.json" "$build"
