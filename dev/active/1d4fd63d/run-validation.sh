#!/usr/bin/env bash
# Correctness evidence for the transpose-lane receipts (jit:1d4fd63d).
#
# Usage: ./run-validation.sh [output.json]   (default dev/active/1d4fd63d/validation.json)
#
# Runs the shared contracts every lane of the family answers and records what
# it observed. The launcher refuses to start a timed campaign unless the
# recorded run passed, so correctness precedes timing.
#
# The suites are the repository's own, not a copy of them:
#
#   * `gf2-kernels-simd`'s `transpose` and `bch_encode` unit tests, which run
#     the block contract and the bit-slice round trip over every available
#     lane;
#   * `gf2-core`'s `transpose_lane_contract`, which drives the production
#     tiling through each lane for zero-sized, non-square, partial-word and
#     partial-tile shapes, with and without the `simd` feature that decides
#     what `BitMatrix::transpose` itself resolves.
#
# The record states the command, its exit status and the test counts the
# harness reported; it asserts nothing the run did not print.

set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
export PATH="$HOME/.cargo/bin:$PATH"
# The repository MSRV, which the measurement contract requires shipped code to
# be verified at and which the arm executables are built with.
export RUSTUP_TOOLCHAIN=1.95
OUTPUT=${1:-dev/active/1d4fd63d/validation.json}
BUDGET=./scripts/cargo-budget.sh
LOG=$(mktemp)
trap 'rm -f "$LOG"' EXIT

run() {
  local name="$1"
  shift
  echo "== $name: $*" >>"$LOG"
  if "$@" >>"$LOG" 2>&1; then
    echo "== $name: exit 0" >>"$LOG"
  else
    local status=$?
    echo "== $name: exit $status" >>"$LOG"
    return "$status"
  fi
}

status=0
run kernels-lib "$BUDGET" --test cargo test -p gf2-kernels-simd --lib transpose || status=$?
run kernels-bitslice "$BUDGET" --test cargo test -p gf2-kernels-simd --lib bch_encode || status=$?
run matrix-contract-portable "$BUDGET" --test cargo test -p gf2-core --test transpose_lane_contract || status=$?
run matrix-contract-simd "$BUDGET" --test cargo test -p gf2-core --features simd --test transpose_lane_contract || status=$?

python3 - "$OUTPUT" "$status" "$LOG" <<'PY'
import json
import re
import subprocess
import sys

output, status, log_path = sys.argv[1], int(sys.argv[2]), sys.argv[3]
log = open(log_path).read()
steps = []
current = None
for line in log.splitlines():
    header = re.match(r"^== (\S+): (.*)$", line)
    if header and not header.group(2).startswith("exit "):
        current = {"step": header.group(1), "command": header.group(2), "results": []}
        steps.append(current)
    elif header:
        current["exit"] = int(header.group(2).split()[1])
    elif current is not None and line.startswith("test result:"):
        current["results"].append(line.strip())

record = {
    "schema": "1d4fd63d-validation-v1",
    "passed": status == 0,
    "toolchain": subprocess.run(
        ["rustc", "--version"], capture_output=True, text=True, check=True
    ).stdout.strip(),
    "steps": steps,
    "transcript": log,
}
with open(output, "w") as handle:
    json.dump(record, handle, indent=2)
    handle.write("\n")
print(f"validation {'passed' if record['passed'] else 'FAILED'} -> {output}")
PY
exit "$status"
