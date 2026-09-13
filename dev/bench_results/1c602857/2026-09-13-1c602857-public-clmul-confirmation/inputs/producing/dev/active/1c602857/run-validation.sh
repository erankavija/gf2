#!/usr/bin/env bash
# Correctness evidence for the public wide carry-less product receipt
# (jit:1c602857).
#
# Runs the shared carry-less product conformance suite and records the outcome
# in validation.json, which the launcher asserts before it takes the benchmark
# mutex: timing never precedes correctness. The suite compares both public
# entry points, the capability dispatch and the portable fallback against an
# independent shift-and-XOR oracle at dispatched and non-dispatched widths.
#
# Usage: ./run-validation.sh

set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
ISSUE=1c602857
OUT="dev/active/$ISSUE/validation.json"
LOG="dev/active/$ISSUE/validation-raw.txt"

SUITE=(cargo nextest run -p gf2-core --cargo-profile ci-test --profile ci
       --features simd,test-support --test clmul_wide_conformance)

set +e
./scripts/cargo-budget.sh --test "${SUITE[@]}" >"$LOG" 2>&1
status=$?
set -e
cat "$LOG"

python3 - "$OUT" "$LOG" "$status" "${SUITE[*]}" <<'PY'
import json, sys, time

out, log, status, command = sys.argv[1:]
passed = status == "0"
document = {
    "schema": "public-clmul-validation-v1",
    "issue": "1c602857",
    "recorded_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    "command": f"./scripts/cargo-budget.sh --test {command}",
    "raw_output": log,
    "suite": "crates/gf2-core/tests/clmul_wide_conformance.rs",
    "exit_status": int(status),
    "passed": passed,
}
with open(out, "w") as handle:
    json.dump(document, handle, indent=2)
    handle.write("\n")
print(f"validation passed={passed} -> {out}", file=sys.stderr)
PY
exit "$status"
