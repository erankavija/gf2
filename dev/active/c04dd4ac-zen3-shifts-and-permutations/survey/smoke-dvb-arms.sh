#!/usr/bin/env bash
# Carry every DVB profile arm through the real runner before a campaign is
# queued (jit:9fb40c83).
#
# A campaign that reaches the benchmark window and dies on its first arm spends
# the window and measures nothing. Reading the runner and the arm side by side
# does not establish the wire: the arm is a separate workspace whose request
# mirror and environment contract are checked only when the two processes
# actually speak. This script is the same `benchmark-ab-runner` binary, the
# same plan projector, the same arm executable and the same wire the campaign
# uses, over a throwaway family that names all four arms.
#
# Untimed by policy: the host is never locked during a working session, so the
# script takes a private throwaway lock under `target/` rather than the
# canonical CCX1 mutex, and never calls `dev/scripts/ccx1-bench-flock.sh`. The
# runner requires an inherited descriptor for a held lock, which the private
# lock supplies. Nothing it writes is evidence: the addendum, ledger, stage and
# receipt live under `target/` and are rebuilt on every invocation, and its
# numbers decide nothing.
#
# Usage (from the worker worktree root):
#   dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/smoke-dvb-arms.sh
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../../.." && pwd)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worker worktree root' >&2
    exit 2
}

ADDENDUM=dev/active/c04dd4ac-zen3-shifts-and-permutations/dvb-profile-addendum.json
PRODUCING=dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/dvb-producing-inputs.json
PLAN_TOOL=dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/make-dvb-plan.py
RECORD=dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/runner-smoke.txt
ARM="${REPO}/target/9fb40c83-arms-native/release/dvb-profile-arm"
# The receipt pins the addendum and the ledger by a repository-relative path,
# so the smoke keeps every campaign input relative and runs from the root.
SMOKE=target/9fb40c83-campaigns/arms-smoke
RUNNER="${REPO}/target/release/benchmark-ab-runner"
ARMS=(gf2-direct-a gf2-direct-b gf2-stage xdsopl-stage)

export PATH="${HOME}/.cargo/bin:${PATH}"
export RUSTUP_TOOLCHAIN=1.95
export CARGO_CI_NO_SCCACHE=1

[[ -x "${ARM}" ]] || {
    echo "${ARM} is absent; run ${HERE#"${REPO}/"}/build-dvb-harness.sh first" >&2
    exit 2
}
./scripts/cargo-budget.sh cargo build --release --locked -p tuning-campaign-support \
    --bin benchmark-ab-runner >/dev/null

rm -rf "${SMOKE}"
mkdir -p "${SMOKE}"
: >"${SMOKE}/ledger.jsonl"
touch "${SMOKE}/lock"

# The throwaway family is the frozen addendum restricted to the two smallest
# cells, which between them name all four arms at the same declared boundaries.
python3 - "${ADDENDUM}" "${SMOKE}" <<'PY'
import json, sys

addendum, smoke = sys.argv[1], sys.argv[2]
family = json.load(open(addendum))
keep = {
    "dvb-t2-qam16-r12-short-warm-isolated-null",
    "dvb-t2-qam16-r12-short-warm-sim-stage-gap",
}
family["family"] = dict(
    family["family"],
    id="dvb-t2-interleave-arms-smoke",
    description=(
        "Throwaway wire smoke of issue 9fb40c83: every arm of the profile at the smallest "
        "declared frame, on a throwaway ledger under target/. It decides nothing, adopts "
        "nothing and no receipt cites it."
    ),
)
family["family_wise"] = dict(family["family_wise"], ledger_path=f"{smoke}/ledger.jsonl")
family["cells"] = [cell for cell in family["cells"] if cell["cell_id"] in keep]
if len(family["cells"]) != len(keep):
    raise SystemExit("the frozen addendum no longer declares both smoke cells")
json.dump(family, open(f"{smoke}/addendum.json", "w"), indent=2)
PY

python3 -B "${PLAN_TOOL}" \
    --addendum "${SMOKE}/addendum.json" \
    --campaign-id 9fb40c83-dvb-interleave-arms-smoke \
    --campaign-seed 20260916 \
    --lock "${SMOKE}/lock" \
    --executable "${ARM}" \
    --producing-manifest "${PRODUCING}" \
    --output "${SMOKE}/plan.json" \
    --max-cells-per-session 2 \
    --pilot-pairs 6

# The runner refuses to measure without an inherited descriptor for a held
# lock. A private lock satisfies that without taking the canonical host mutex.
exec {LOCK_FD}<"${SMOKE}/lock"
flock -x "${LOCK_FD}"
"${RUNNER}" run "${SMOKE}/stage" "${SMOKE}/plan.json"
flock -u "${LOCK_FD}"
exec {LOCK_FD}<&-
"${RUNNER}" finalize "${SMOKE}/stage" "${SMOKE}/receipt"

# The smoke is judged by what every arm wrote, not by the exit codes above: one
# parsed result line per arm per role, in cells that carry paired executions.
python3 - "${SMOKE}/receipt/receipt.json" "${SMOKE}/stage/execution.log" "${ARM}" "${RECORD}" \
    "${ARMS[@]}" <<'PY'
import hashlib, json, sys

receipt_path, log_path, arm_path, record_path = sys.argv[1:5]
expected = set(sys.argv[5:])
receipt = json.load(open(receipt_path))
events = [json.loads(line) for line in open(log_path)]

spawns = sum(1 for event in events if event["event"] == "child-spawn")
diagnostics = [event for event in events if event["event"] == "child-diagnostic"]
exits = [event for event in events if event["event"] == "child-exit"]
nonzero = [
    event
    for event in exits
    if event["details"]["outcome"].get("exit_code") != 0
]
terminal = [
    event["event"]
    for event in events
    if event["event"] in ("complete", "failed", "paused", "budget-exhausted")
]
if terminal != ["complete"]:
    raise SystemExit(f"the stage ended {terminal} rather than complete")

lines, cells = {}, []
for cell in receipt["cells"]:
    pairs = cell.get("pairs") or []
    if not pairs:
        raise SystemExit(f"cell {cell['cell_id']} carries no paired execution")
    cells.append(cell["cell_id"])
    for pair in pairs:
        for role in ("baseline", "candidate"):
            arm = cell[f"{role}_arm"]
            if not pair[role].get("windows"):
                raise SystemExit(f"{arm} in {cell['cell_id']} reported no window")
            lines[arm] = lines.get(arm, 0) + 1

missing = sorted(expected - set(lines))
if missing:
    raise SystemExit(f"no result line from {', '.join(missing)}")
if nonzero or diagnostics:
    raise SystemExit(f"{len(nonzero)} arm children failed and {len(diagnostics)} wrote diagnostics")
if spawns != len(exits) or spawns != sum(lines.values()):
    raise SystemExit(f"{spawns} spawns, {len(exits)} exits, {sum(lines.values())} result lines")

digest = hashlib.sha256(open(arm_path, "rb").read()).hexdigest()
with open(record_path, "w") as record:
    print(
        "# DVB-T2 interleaver profile runner-wire smoke (jit:9fb40c83)\n"
        "# command: dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/smoke-dvb-arms.sh\n"
        "# every line below is observed at run time from the stage execution log and the\n"
        "# finalized throwaway receipt; the record carries no clock reading, so a rerun on\n"
        "# the same executable reproduces it byte for byte",
        file=record,
    )
    print(f"# dvb-profile-arm sha256: {digest}", file=record)
    for cell in sorted(cells):
        print(f"PASS {cell}: paired executions complete", file=record)
    for arm in sorted(lines):
        print(f"PASS {arm}: {lines[arm]} handshakes, {lines[arm]} result lines parsed", file=record)
    print(
        f"PASS wire: {spawns} child spawns, {len(exits)} clean exits, "
        f"{len(diagnostics)} child diagnostics",
        file=record,
    )
print(open(record_path).read(), end="")
PY
echo "smoke record: ${RECORD}" >&2
