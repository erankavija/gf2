#!/usr/bin/env bash
# Tests dev/scripts/verify-campaign-log.py against committed campaign evidence.
#
# Usage (from the repository root): dev/scripts/verify-campaign-log.test.sh
#
# The positive case is a committed accepted campaign, so the checker is tested
# against a journal a real session wrote rather than a fixture; each negative
# case perturbs exactly one record of that journal.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CHECKER="${ROOT}/dev/scripts/verify-campaign-log.py"
# A committed accepted campaign that paused at its session cell budget and then
# completed, so the positive case covers a resumed campaign.
CAMPAIGN="${ROOT}/dev/bench_results/19513245/r1-matrix-confirmation"
WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT

run() {
    python3 -B "${CHECKER}" --log "$1" \
        --receipt "${CAMPAIGN}/receipt.json" --plan "${CAMPAIGN}/plan.json"
}

expect_refusal() {
    local log=$1 pattern=$2 output
    if output=$(run "${log}" 2>&1); then
        echo "the checker accepted ${log}: ${output}" >&2
        exit 1
    fi
    grep -q -- "${pattern}" <<<"${output}" || {
        echo "refusal of ${log} does not mention ${pattern}: ${output}" >&2
        exit 1
    }
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
