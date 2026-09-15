#!/usr/bin/env bash
# Correctness evidence for the count-optimization receipts (jit:5cbb6545).
#
# Runs the shared population-count suite over every resolved route, the survey's
# own arm verifier, a production-selection audit, and a source-evidence
# reproducibility check, then records all outcomes in validation.json, which the
# launcher asserts before it takes the benchmark mutex: timing never precedes
# correctness. The shared suite covers the library routes and the bit semantics;
# the verifier additionally covers the external arms, their byte-length tails,
# Mula's alignment precondition and the consumer arms, all against an independent
# byte-table count.
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
PRODUCTION_LOG="$ACTIVE/validation-production.txt"
SOURCE_EVIDENCE_LOG="$ACTIVE/validation-source-evidence.txt"
SOURCE_EVIDENCE="$ACTIVE/survey/source-evidence.json"

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

set +e
python3 "$ACTIVE/survey/make-source-evidence.py" --check "$SOURCE_EVIDENCE" \
  >"$SOURCE_EVIDENCE_LOG" 2>&1
source_evidence_status=$?
set -e
cat "$SOURCE_EVIDENCE_LOG"

set +e
(
  if rg -n 'popcount_csa_min_words|POPCOUNT_CSA_MIN_WORDS' \
      crates/gf2-core/src/kernels/backend.rs \
      crates/gf2-core/src/tuning/baked.rs \
      crates/gf2-core/src/tuning/mod.rs \
      crates/gf2-core/data/tuning-profiles/conservative.json; then
    echo "unsupported CSA selector remains"
    selector_status=1
  else
    echo "unsupported CSA selector absent"
    selector_status=0
  fi
  if rg -n 'SimdCarrySave|popcnt_csa_fn|and_popcnt_csa_fn' \
      crates/gf2-core/src/kernels/ops.rs crates/gf2-core/src/matrix.rs; then
    echo "automatic CSA dispatch remains"
    dispatch_status=1
  else
    echo "automatic CSA dispatch absent"
    dispatch_status=0
  fi
  audit_status=$((selector_status || dispatch_status))
  echo "production audit status=$audit_status"
  exit "$audit_status"
) >"$PRODUCTION_LOG" 2>&1
audit_status=$?
set -e
cat "$PRODUCTION_LOG"

python3 - "$OUT" "$LOG" "$suite_status" "$VERIFY_LOG" "$verify_status" \
  "$PRODUCTION_LOG" "$audit_status" "$SOURCE_EVIDENCE" \
  "$SOURCE_EVIDENCE_LOG" "$source_evidence_status" "${SUITE[*]}" <<'PY'
import json, sys, time

(
    out,
    log,
    suite_status,
    verify_log,
    verify_status,
    production_log,
    audit_status,
    source_evidence,
    source_evidence_log,
    source_evidence_status,
    command,
) = sys.argv[1:]
passed = (
    suite_status == "0"
    and verify_status == "0"
    and audit_status == "0"
    and source_evidence_status == "0"
)
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
    "production_audit": {
        "command": "rg for unsupported CSA selector and automatic-dispatch references",
        "raw_output": production_log,
        "exit_status": int(audit_status),
    },
    "source_evidence": {
        "command": f"python3 dev/active/5cbb6545/survey/make-source-evidence.py --check {source_evidence}",
        "artifact": source_evidence,
        "raw_output": source_evidence_log,
        "exit_status": int(source_evidence_status),
    },
    "passed": passed,
}
with open(out, "w") as handle:
    json.dump(document, handle, indent=2)
    handle.write("\n")
print(f"validation passed={passed} -> {out}", file=sys.stderr)
PY
if [[ "$suite_status" -ne 0 ]]; then exit "$suite_status"; fi
if [[ "$verify_status" -ne 0 ]]; then exit "$verify_status"; fi
if [[ "$source_evidence_status" -ne 0 ]]; then exit "$source_evidence_status"; fi
exit "$audit_status"
