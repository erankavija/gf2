#!/usr/bin/env bash
# Correctness evidence for the public wide carry-less product receipt
# (jit:1c602857).
#
# Runs the shared carry-less product conformance suite under every feature
# configuration this issue's findings cite -- the crate's default features,
# `--features simd,test-support`, and `--all-features` -- and records each as
# its own entry in validation.json, which the launcher asserts before it
# takes the benchmark mutex: timing never precedes correctness.
#
# Each entry captures the exact command, its own raw nextest output file, the
# parsed pass/fail count, the `dispatch-lane-witness` lines the suite prints
# for the widths it exercises (nextest `--success-output=final` surfaces a
# passing test's captured stdout), and the toolchain that built it.
# validation.json's top-level `passed` is true only when every configuration
# passed, which is what run-public-clmul.sh gates on.
#
# Usage: ./run-validation.sh

set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
ISSUE=1c602857
DIR="dev/active/$ISSUE"
OUT="$DIR/validation.json"

TOOLCHAIN=$(rustc --version)

# label:feature-flag (empty flag = crate default features; test-support is
# always present regardless, because gf2-core's own Cargo.toml declares it as
# a dev-dependency of itself with `features = ["test-support"]`)
CONFIGS=(
  "default-features:"
  "simd-test-support:--features simd,test-support"
  "all-features:--all-features"
)

RUN_ARGS=()
for entry in "${CONFIGS[@]}"; do
  label=${entry%%:*}
  flag=${entry#*:}
  log="$DIR/validation-raw-$label.txt"

  suite=(cargo nextest run -p gf2-core --cargo-profile ci-test --profile ci
         --test clmul_wide_conformance --success-output=final)
  if [[ -n "$flag" ]]; then
    # shellcheck disable=SC2206
    suite+=($flag)
  fi

  set +e
  ./scripts/cargo-budget.sh --test "${suite[@]}" >"$log" 2>&1
  status=$?
  set -e
  cat "$log"

  command="./scripts/cargo-budget.sh --test ${suite[*]}"
  RUN_ARGS+=("$label" "$flag" "$command" "$log" "$status")
done

python3 - "$OUT" "$TOOLCHAIN" "${RUN_ARGS[@]}" <<'PY'
import json
import re
import sys
import time

out, toolchain, *rest = sys.argv[1:]
assert len(rest) % 5 == 0
runs = []
overall_passed = True
lane_pattern = re.compile(r"dispatch-lane-witness N=(\d+) lane=(\S+)")
summary_pattern = re.compile(r"(\d+) tests run: (\d+) passed(?:, (\d+) failed)?")

for i in range(0, len(rest), 5):
    label, flag, command, log_path, status = rest[i : i + 5]
    status = int(status)
    passed = status == 0
    overall_passed = overall_passed and passed
    with open(log_path) as handle:
        text = handle.read()
    lanes = {int(n): lane for n, lane in lane_pattern.findall(text)}
    summary = summary_pattern.search(text)
    if summary:
        total, ok, failed = summary.group(1), summary.group(2), summary.group(3)
        counts = {
            "total": int(total),
            "passed": int(ok),
            "failed": int(failed) if failed else 0,
        }
    else:
        counts = None
    runs.append(
        {
            "label": label,
            "cargo_features": flag if flag else "(default)",
            "command": command,
            "raw_output": log_path,
            "exit_status": status,
            "passed": passed,
            "counts": counts,
            "observed_dispatch_lanes": lanes,
        }
    )

document = {
    "schema": "public-clmul-validation-v2",
    "issue": "1c602857",
    "recorded_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    "suite": "crates/gf2-core/tests/clmul_wide_conformance.rs",
    "toolchain": toolchain,
    "passed": overall_passed,
    "runs": runs,
}
with open(out, "w") as handle:
    json.dump(document, handle, indent=2)
    handle.write("\n")
print(f"validation passed={overall_passed} -> {out}", file=sys.stderr)
if not overall_passed:
    sys.exit(1)
PY
