#!/usr/bin/env bash
# Checks that the candidate launcher beside this script refuses every action
# that would build a candidate, and that its window actions stay closed
# outside the benchmark window. Runs no build and no timed work.
#
# Usage: run-candidate.test.sh, from any directory of the checkout.
set -uo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
cd "$(git -C "$here" rev-parse --show-toplevel)"
launcher="$here/run-candidate.sh"
unset GF2_BENCH_WINDOW
failed=0

expect() {
    local line="$1" output status
    shift
    output=$("$launcher" "$@" 2>&1)
    status=$?
    if [[ $status == 2 && "$output" == "$line" ]]; then
        echo "ok: $*"
    else
        echo "FAILED: $* exited $status with: $output"
        failed=1
    fi
}

snapshots_only="the unroll variants exist in the receipts' input snapshots only"
for factor in 2 4; do
    for family in isolated row; do
        expect "refusing build: $snapshots_only" build "$factor" "$family"
        expect "refusing smoke: $snapshots_only" smoke "$factor" "$family"
        expect 'timed candidates run only in the scheduled benchmark window' \
            window "$factor" "$family"
    done
    expect 'timed candidate profiles run only in the scheduled benchmark window' \
        profile-window "$factor" profile
done
exit "$failed"
