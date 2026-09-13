#!/usr/bin/env bash
# Correctness evidence for the count-optimization receipts (jit:5cbb6545).
#
# Runs the shared population-count suite over every resolved route and the
# survey's own arm verifier, then records both outcomes in validation.json,
# which the launcher asserts before it takes the benchmark mutex: timing never
# precedes correctness. The shared suite covers the library routes and the bit
# semantics; the verifier additionally covers the external arms, their
# byte-length tails, Mula's alignment precondition and the consumer arms, all
# against an independent byte-table count.
#
# Usage: ./run-validation.sh

set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
ISSUE=5cbb6545
ACTIVE="dev/active/$ISSUE"
OUT="$ACTIVE/validation.json"
LOG="$ACTIVE/validation-raw.txt"
VERIFY_LOG="$ACTIVE/validation-arms.txt"

SUITE=(cargo nextest run -p gf2-core --cargo-profile ci-test --profile ci
       --features simd,test-support --test popcount_routes)

set +e
./scripts/cargo-budget.sh --test "${SUITE[@]}" >"$LOG" 2>&1
suite_status=$?
set -e
cat "$LOG"

./scripts/cargo-budget.sh cargo build --release \
  --manifest-path "$ACTIVE/survey/gf2-side/Cargo.toml"
set +e
"$ACTIVE/survey/gf2-side/target/release/count-verify" >"$VERIFY_LOG" 2>&1
verify_status=$?
set -e
cat "$VERIFY_LOG"

python3 - "$OUT" "$LOG" "$suite_status" "$VERIFY_LOG" "$verify_status" "${SUITE[*]}" <<'PY'
import json, sys, time

out, log, suite_status, verify_log, verify_status, command = sys.argv[1:]
passed = suite_status == "0" and verify_status == "0"
document = {
    "schema": "count-optimization-validation-v1",
    "issue": "5cbb6545",
    "recorded_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    "shared_suite": {
        "command": f"./scripts/cargo-budget.sh --test {command}",
        "suite": "crates/gf2-core/tests/popcount_routes.rs",
        "raw_output": log,
        "exit_status": int(suite_status),
    },
    "arm_verifier": {
        "command": "dev/active/5cbb6545/survey/gf2-side/target/release/count-verify",
        "source": "dev/active/5cbb6545/survey/gf2-side/src/bin/count-verify.rs",
        "raw_output": verify_log,
        "exit_status": int(verify_status),
    },
    "passed": passed,
}
with open(out, "w") as handle:
    json.dump(document, handle, indent=2)
    handle.write("\n")
print(f"validation passed={passed} -> {out}", file=sys.stderr)
PY
if [[ "$suite_status" -ne 0 ]]; then exit "$suite_status"; fi
exit "$verify_status"
