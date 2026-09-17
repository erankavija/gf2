#!/usr/bin/env bash
# Carry every arm and case operation through the real runner (jit:ad2a6a58).
#
# A campaign that reaches the benchmark window and dies on its first arm spends
# the window and measures nothing. Reading the runner and the arm side by side
# does not establish the wire: the arm is a separate workspace whose request
# mirror and environment contract are checked only when the two processes
# actually speak. This script is the same `benchmark-ab-runner` binary, the
# same plan projector, the same arm executable and the same wire the campaigns
# use, over a throwaway family taken from the frozen addendum's own cells.
#
# The four cells it keeps name all four arms, the one case operation this
# family declares and all three cache states, so every path the arm takes for a
# declared cell is exercised.
#
# Untimed by policy: the host is never locked during a working session, so the
# script takes a private throwaway lock under `target/` rather than the
# canonical CCX1 mutex, and never calls `dev/scripts/ccx1-bench-flock.sh`. The
# runner requires an inherited descriptor for a held lock, which the private
# lock supplies. Nothing it writes is evidence: the addendum, the ledger, the
# stage and the receipt live under `target/` and are rebuilt on every
# invocation, it reserves nothing in the family ledger, it finalizes no receipt
# under the family's result tree, and its numbers decide nothing.
#
# Usage (from the worker worktree root):
#   dev/active/ad2a6a58/survey/smoke-arms.sh
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../../.." && pwd)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worker worktree root' >&2
    exit 2
}

ISSUE=ad2a6a58
SURVEY=dev/active/${ISSUE}/survey
ADDENDUM=dev/active/${ISSUE}/addendum-v4-axpy-pilot.json
PRODUCING=${SURVEY}/producing-inputs.json
PLAN_TOOL=${SURVEY}/make-plan.py
RECORD=${SURVEY}/runner-smoke.txt
ARM="${REPO}/target/${ISSUE}-arm/release/gf256-axpy-arm"
RUNNER="${REPO}/target/release/benchmark-ab-runner"
# The receipt pins the addendum and the ledger by a repository-relative path,
# so the smoke keeps every campaign input relative and runs from the root.
SMOKE=target/${ISSUE}-campaigns/arms-smoke
ARMS=(scalar-element table-element scalar-wide table-wide)
# One cell per (representation, cache state) the frozen addendum declares, at
# the smallest size of each state.
KEEP=(axpy-1k-cold-element axpy-4k-element axpy-1k-cold-wide axpy-2m-stream-wide)

export PATH="${HOME}/.cargo/bin:${PATH}"
export RUSTUP_TOOLCHAIN=1.95

[[ -x "${ARM}" ]] || {
    echo "${ARM} is absent; run dev/bench_results/${ISSUE}/run-axpy-confirmation.sh build" >&2
    exit 2
}
./scripts/cargo-budget.sh cargo build --release --locked -p tuning-campaign-support \
    --bin benchmark-ab-runner >/dev/null

rm -rf "${SMOKE}"
mkdir -p "${SMOKE}"
: >"${SMOKE}/ledger.jsonl"
touch "${SMOKE}/lock"

python3 - "${ADDENDUM}" "${SMOKE}" "${KEEP[@]}" <<'PY'
import json, sys

addendum, smoke, keep = sys.argv[1], sys.argv[2], set(sys.argv[3:])
family = json.load(open(addendum))
family["family"] = dict(
    family["family"],
    id="gf256-shipped-axpy-lane-arms-smoke",
    description=(
        "Throwaway wire smoke of issue ad2a6a58: every arm, the one case operation and every "
        "cache state the campaigns name, at the smallest size of each state, on a throwaway "
        "ledger under target/. It decides nothing, adopts nothing and no receipt cites it."
    ),
)
family["family_wise"] = dict(family["family_wise"], ledger_path=f"{smoke}/ledger.jsonl")
family["cells"] = [cell for cell in family["cells"] if cell["cell_id"] in keep]
if len(family["cells"]) != len(keep):
    raise SystemExit("the frozen addendum no longer declares every smoke cell")
json.dump(family, open(f"{smoke}/addendum.json", "w"), indent=2)
PY

python3 -B "${PLAN_TOOL}" \
    --addendum "${SMOKE}/addendum.json" \
    --label smoke \
    --campaign-id "${ISSUE}-axpy-arms-smoke" \
    --campaign-seed 20260918 \
    --lock "$(realpath "${SMOKE}/lock")" \
    --executable "${ARM}" \
    --producing-manifest "${PRODUCING}" \
    --max-cells-per-session 4 \
    --pilot-pairs 6 \
    --output "${SMOKE}/plan.json"
"${RUNNER}" check "${SMOKE}/plan.json"

# The runner refuses to measure without an inherited descriptor for a held
# lock. A private lock satisfies that without taking the canonical host mutex.
exec {LOCK_FD}<"${SMOKE}/lock"
flock -x "${LOCK_FD}"
"${RUNNER}" run "${SMOKE}/stage" "${SMOKE}/plan.json"
flock -u "${LOCK_FD}"
exec {LOCK_FD}<&-
"${RUNNER}" finalize "${SMOKE}/stage" "${SMOKE}/receipt"

# The smoke is judged by what every arm wrote, not by the exit codes above: one
# parsed result line per arm per role, in cells that carry paired executions,
# and a lane witness that names the lane the arm was asked to hold.
python3 - "${SMOKE}/receipt/receipt.json" "${SMOKE}/stage/execution.log" "${ARM}" "${RECORD}" \
    "${ARMS[@]}" <<'PY'
import hashlib, json, sys

receipt_path, log_path, arm_path, record_path = sys.argv[1:5]
expected = set(sys.argv[5:])
receipt = json.load(open(receipt_path))
events = [json.loads(line) for line in open(log_path)]

spawns = sum(1 for event in events if event["event"] == "child-spawn")
exits = [event for event in events if event["event"] == "child-exit"]
nonzero = [event for event in exits if event["details"]["outcome"].get("exit_code") != 0]
diagnostics = [event for event in events if event["event"] == "child-diagnostic"]
terminal = [event["event"] for event in events
            if event["event"] in ("complete", "failed", "paused", "budget-exhausted")]
if terminal != ["complete"]:
    raise SystemExit(f"the stage ended {terminal} rather than complete")

# The lane an arm name promises, and the witness token that proves it ran.
LANES = {"scalar": "lane=gf256-table-declined", "table": "lane=gf256-product-table"}

lines, cells, lanes = {}, [], {}
for cell in receipt["cells"]:
    pairs = cell.get("pairs") or []
    if not pairs:
        raise SystemExit(f"cell {cell['cell_id']} carries no paired execution")
    cells.append(cell["cell_id"])
    for pair in pairs:
        for role in ("baseline", "candidate"):
            execution = pair[role]
            arm = cell[f"{role}_arm"]
            if not execution.get("windows"):
                raise SystemExit(f"{arm} in {cell['cell_id']} reported no window")
            witness = LANES[arm.split("-", 1)[0]]
            if witness not in execution["selected_path"]:
                raise SystemExit(f"{arm} in {cell['cell_id']} reported "
                                 f"{execution['selected_path']!r}, which is not {witness}")
            lines[arm] = lines.get(arm, 0) + 1
            lanes.setdefault(arm, set()).add(execution["selected_path"])

missing = sorted(expected - set(lines))
if missing:
    raise SystemExit(f"no result line from {', '.join(missing)}")
if nonzero or diagnostics:
    raise SystemExit(f"{len(nonzero)} arm children failed and "
                     f"{len(diagnostics)} wrote diagnostics")
if spawns != len(exits) or spawns != sum(lines.values()):
    raise SystemExit(f"{spawns} spawns, {len(exits)} exits, {sum(lines.values())} result lines")

digest = hashlib.sha256(open(arm_path, "rb").read()).hexdigest()
with open(record_path, "w") as record:
    print(
        "# GF(2^8) axpy runner-wire smoke (jit:ad2a6a58)\n"
        "# command: dev/active/ad2a6a58/survey/smoke-arms.sh\n"
        "# every line below is observed at run time from the stage execution log and the\n"
        "# finalized throwaway receipt; the record carries no clock reading, so a rerun on\n"
        "# the same executable reproduces it byte for byte",
        file=record,
    )
    print(f"# gf256-axpy-arm sha256: {digest}", file=record)
    for cell in sorted(cells):
        print(f"PASS {cell}: paired executions complete", file=record)
    for arm in sorted(lines):
        for path in sorted(lanes[arm]):
            print(f"PASS {arm}: {path}", file=record)
        print(f"PASS {arm}: {lines[arm]} handshakes, {lines[arm]} result lines parsed", file=record)
    print(f"PASS wire: {spawns} child spawns, {len(exits)} clean exits, "
          f"{len(diagnostics)} child diagnostics", file=record)
print(open(record_path).read(), end="")
PY
echo "smoke record: ${RECORD}" >&2
