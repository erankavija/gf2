#!/usr/bin/env bash
# Rust 1.95 build-and-suite validation record for jit f8dd4dde (REQ-06).
#
# Usage: ./validate-msrv.sh [output.json]   (default dev/active/f8dd4dde/validate-msrv-record.json)
#
# Builds gf2-core and gf2-kernels-simd under the pinned MSRV toolchain, runs
# the shared residual-shift suite (gf2-core's residual_shift unit tests and
# residual_shift_routes integration suite) and gf2-kernels-simd's shift_funnel
# unit tests through the repository's ci-test/ci nextest profiles, then runs
# the standalone witness probe in msrv-validation/, which reports the host's
# own bmi2 detection and the route gf2_core::residual_shift's lane witness
# recorded for each arm of its force switch.
#
# The record states only what this run observed: no duration, no nextest run
# id, and test results are sorted by name rather than kept in completion
# order, so re-running on an unchanged tree reproduces it byte for byte.

set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
export PATH="$HOME/.cargo/bin:$PATH"
export RUSTUP_TOOLCHAIN=1.95
OUTPUT=${1:-dev/active/f8dd4dde/validate-msrv-record.json}
BUDGET=./scripts/cargo-budget.sh
WITNESS_MANIFEST=dev/active/f8dd4dde/msrv-validation/Cargo.toml
LOG=$(mktemp)
trap 'rm -f "$LOG"' EXIT

run() {
  local name="$1"
  shift
  echo "== $name: $*" >>"$LOG"
  if nice -n 19 "$@" >>"$LOG" 2>&1; then
    echo "== $name: exit 0" >>"$LOG"
  else
    local rc=$?
    echo "== $name: exit $rc" >>"$LOG"
    return "$rc"
  fi
}

status=0
run build-all-features "$BUDGET" cargo build -p gf2-core -p gf2-kernels-simd --all-features || status=$?
run build-dispatched-feature-set "$BUDGET" cargo build -p gf2-core -p gf2-kernels-simd --features simd || status=$?
run test-residual-shift-unit "$BUDGET" --test cargo nextest run --cargo-profile ci-test --profile ci -p gf2-core --features simd --lib -E 'test(residual_shift)' || status=$?
run test-residual-shift-routes "$BUDGET" --test cargo nextest run --cargo-profile ci-test --profile ci -p gf2-core --features simd --test residual_shift_routes || status=$?
run test-shift-funnel-unit "$BUDGET" --test cargo nextest run --cargo-profile ci-test --profile ci -p gf2-kernels-simd --lib -E 'test(shift_funnel)' || status=$?
run witness-probe cargo run --quiet --manifest-path "$WITNESS_MANIFEST" || status=$?

rustc -Vv >"${LOG}.rustc"
cargo -V >"${LOG}.cargo"

python3 - "$OUTPUT" "$status" "$LOG" "${LOG}.rustc" "${LOG}.cargo" <<'PY'
import json
import re
import sys

output, status, log_path, rustc_path, cargo_path = sys.argv[1:6]
status = int(status)
log = open(log_path).read()

HEADER = re.compile(r"^== (\S+): (.*)$")
RESULT = re.compile(
    r"^\s*(PASS|FAIL|ABORT|TIMEOUT)\s+\[[^\]]*\]\s+(?:\(\d+/\d+\)\s+)?(\S+)\s+(.+?)\s*$"
)
SUMMARY = re.compile(r"^\s*Summary\s+\[[^\]]*\]\s+(.*)$")

steps = {}
current = None
for line in log.splitlines():
    header = HEADER.match(line)
    if header and not header.group(2).startswith("exit "):
        current = {"command": header.group(2), "results": [], "witness": {}}
        steps[header.group(1)] = current
        continue
    if header:
        current["exit"] = int(header.group(2).split()[1])
        continue
    if current is None:
        continue
    m = RESULT.match(line)
    if m:
        status_word, binary_id, test_name = m.groups()
        current["results"].append(f"{status_word} {binary_id} {test_name}")
        continue
    m = SUMMARY.match(line)
    if m:
        current["summary"] = m.group(1)
        continue
    if line.startswith("host_bmi2="):
        current["witness"]["host_bmi2"] = line.split("=", 1)[1] == "true"
    elif line.startswith("arm="):
        arm, route = re.match(r"arm=(\S+) route=(\S+)", line).groups()
        current["witness"][f"arm_{arm.replace('-', '_')}_route"] = route

for step in steps.values():
    step["results"].sort()

record = {
    "schema": "f8dd4dde-msrv-1.95-validation-v1",
    "passed": status == 0,
    "toolchain": {
        "rustc_verbose": open(rustc_path).read().strip(),
        "cargo_version": open(cargo_path).read().strip(),
    },
    "builds": {
        name: {"command": s["command"], "exit": s["exit"]}
        for name, s in steps.items()
        if name.startswith("build-")
    },
    "tests": {
        name: {
            "command": s["command"],
            "exit": s["exit"],
            "summary": s.get("summary"),
            "results": s["results"],
        }
        for name, s in steps.items()
        if name.startswith("test-")
    },
    "witness": {
        "command": steps["witness-probe"]["command"],
        "exit": steps["witness-probe"]["exit"],
        **steps["witness-probe"]["witness"],
    },
}
with open(output, "w") as handle:
    json.dump(record, handle, indent=2, sort_keys=False)
    handle.write("\n")
print(f"validate-msrv {'passed' if record['passed'] else 'FAILED'} -> {output}")
PY
rm -f "${LOG}.rustc" "${LOG}.cargo"
exit "$status"
