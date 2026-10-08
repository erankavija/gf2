#!/usr/bin/env bash
# Record the runner's untimed check of the pilot plan (jit:7dbe1393).
#
# Usage: record-plan-check.sh <benchmark-ab-runner> <plan.json>, from the
# worktree root. `check` decodes the plan and the campaign addendum it names
# and validates the addendum against the live version-4 schema; it measures
# nothing. The plan is the projection the launcher's `window --smoke` stores.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worktree root' >&2
    exit 2
}
RUNNER="${1:?usage: record-plan-check.sh <benchmark-ab-runner> <plan.json>}"
PLAN="${2:?usage: record-plan-check.sh <benchmark-ab-runner> <plan.json>}"
RECORD="${HERE}/pilot-plan-check.txt"
{
    echo '# Runner check of the ragged-tail pilot plan (jit:7dbe1393)'
    echo "# plan sha256: $(sha256sum "${PLAN}" | cut -d' ' -f1)"
    "${RUNNER}" check "${PLAN}"
} >"${RECORD}.tmp"
mv "${RECORD}.tmp" "${RECORD}"
cat "${RECORD}"
