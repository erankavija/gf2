#!/usr/bin/env bash
# Tests verify-campaign-log.py, which lies beside this file, against committed
# campaign evidence.
#
# Usage: verify-campaign-log.test.sh
#
# Positive cases are committed campaigns, so the checker sees journals real
# sessions wrote. Negative cases perturb those journals.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT=$(git -C "${HERE}" rev-parse --show-toplevel)
CHECKER="${HERE}/verify-campaign-log.py"

# The committed receipt directory whose path ends in $1: receipt input snapshots
# hold byte copies under an `inputs` directory, and the live one lies outside.
campaign() {
    local found
    mapfile -t found < <(git -C "${ROOT}" ls-files -- ":(glob)**/$1/receipt.json" |
        grep -v '/inputs/')
    [[ ${#found[@]} -eq 1 ]] || {
        echo "${#found[@]} committed campaigns end in $1; exactly one must" >&2
        exit 2
    }
    dirname "${ROOT}/${found[0]}"
}

# A committed accepted campaign that paused at its session cell budget and then
# completed, so the positive case covers a resumed campaign.
CAMPAIGN=$(campaign r1-matrix-confirmation)
WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT

run() {
    local campaign=${2:-${CAMPAIGN}}
    python3 -B "${CHECKER}" --log "$1" \
        --receipt "${campaign}/receipt.json" --plan "${campaign}/plan.json"
}

expect_refusal() {
    local log=$1 pattern=$2 campaign=${3:-${CAMPAIGN}} output
    if output=$(run "${log}" "${campaign}" 2>&1); then
        echo "the checker accepted ${log}: ${output}" >&2
        exit 1
    fi
    grep -q -- "${pattern}" <<<"${output}" || {
        echo "refusal of ${log} does not mention ${pattern}: ${output}" >&2
        exit 1
    }
}

# Change every completed pair count to the wrong count for an exploratory log.
retell_all_completion_pairs() {
    python3 - "$1" "$2" "$3" <<'PY'
import json, sys
source, pairs, output = sys.argv[1:4]
with open(output, "w") as dest:
    for line in open(source):
        record = json.loads(line)
        if record["event"] == "cell-complete":
            record["details"]["pairs"] = int(pairs)
            line = json.dumps(record) + "\n"
        dest.write(line)
PY
}

# A journal with one record dropped, selected by its event name and occurrence.
drop_event() {
    python3 - "$1" "$2" "$3" <<'PY'
import json, sys
source, event, output = sys.argv[1:4]
kept, dropped = [], False
for line in open(source):
    if not dropped and json.loads(line)["event"] == event:
        dropped = True
        continue
    kept.append(line)
if not dropped:
    raise SystemExit(f"the journal carries no {event} record")
open(output, "w").writelines(kept)
PY
}

# A journal whose first cell-complete claims a different pair count or status.
retell_completion() {
    python3 - "$1" "$2" "$3" "$4" <<'PY'
import json, sys
source, field, value, output = sys.argv[1:5]
lines, retold = [], False
for line in open(source):
    record = json.loads(line)
    if not retold and record["event"] == "cell-complete":
        record["details"][field] = json.loads(value)
        retold = True
        line = json.dumps(record) + "\n"
    lines.append(line)
if not retold:
    raise SystemExit("the journal carries no cell-complete record")
open(output, "w").writelines(lines)
PY
}

run "${CAMPAIGN}/execution.log" | grep -q 'terminal record' || {
    echo 'the checker refused a committed accepted campaign' >&2
    exit 1
}

# The plans omit pilot_pairs; their frozen addenda declare exploratory cells.
for family in 2037941f-logical-isolated-xor 2037941f-logical-nr-construction \
    2037941f-logical-public-row-xor; do
    pilot=$(campaign "${family}/v4-r1-pilot")
    run "${pilot}/execution.log" "${pilot}" | grep -q 'terminal record'
done
retell_all_completion_pairs "${pilot}/execution.log" 24 "${WORK}/wrong-pilot-pairs.log"
expect_refusal "${WORK}/wrong-pilot-pairs.log" "24 pairs, 6 declared" "${pilot}"

drop_event "${CAMPAIGN}/execution.log" complete "${WORK}/no-complete.log"
expect_refusal "${WORK}/no-complete.log" "session-terminal records"

drop_event "${CAMPAIGN}/execution.log" checkpoint-accepted "${WORK}/no-checkpoint.log"
expect_refusal "${WORK}/no-checkpoint.log" "0 checkpoints accepted"

drop_event "${CAMPAIGN}/execution.log" cell-complete "${WORK}/no-cell-complete.log"
expect_refusal "${WORK}/no-cell-complete.log" "never completed"

retell_completion "${CAMPAIGN}/execution.log" pairs 1 "${WORK}/short-cell.log"
expect_refusal "${WORK}/short-cell.log" "1 pairs"

retell_completion "${CAMPAIGN}/execution.log" status '"unavailable"' "${WORK}/unmeasured.log"
expect_refusal "${WORK}/unmeasured.log" "completed with status"

# The session loop's own question: complete for a finished stage, not complete
# for a stage whose terminal record is missing and for a log that does not exist.
python3 -B "${CHECKER}" --log "${CAMPAIGN}/execution.log" --stage-complete
! python3 -B "${CHECKER}" --log "${WORK}/no-complete.log" --stage-complete
! python3 -B "${CHECKER}" --log "${WORK}/absent.log" --stage-complete

echo 'PASS verify-campaign-log: a committed resumed campaign verifies; five perturbations refuse;'
echo 'PASS verify-campaign-log: the stage-complete question answers a finished, an unfinished and'
echo 'PASS verify-campaign-log: an absent log'
echo 'PASS verify-campaign-log: null exploratory pairs use the frozen pilot minimum;'
echo 'PASS verify-campaign-log: confirmatory pairs verify and wrong pilot counts refuse'
