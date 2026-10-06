#!/usr/bin/env python3
"""Judge the dense-parity staged arm smoke and write its committed record.

The smoke's contract is stated once, at `tuning_campaign_support::arm::smoke`.
This judge reads each family's runner stage and smoke record plus the semantic
oracle's output, refuses what the frozen addendum forbids, and projects every
recorded line. It carries no clock reading, so a rerun on the same executables
reproduces the record byte for byte. The recorded command line is the launcher
invocation that ran, passed through as `--command`.
"""

import argparse
import json
import os
import pathlib
import re
import subprocess
import sys

# The position every dispatch of a record takes; either pair position would
# carry the plan's timing-window budget.
VALIDATION_ROLE = "validation"
# The shared object an external arm reports having loaded, and its digest. The
# arm refuses a run whose object is not the qualified one, so a dispatch that
# carries this pair ran against the qualified comparator.
EXTERNAL_OBJECT = re.compile(r"/loaded=(?P<path>.+)/sha256=(?P<digest>[0-9a-f]{64})$")
# The fixture banks a gf2 arm built and the bytes they hold, which a receipt
# retains beside the cache claim they arrange.
GF2_BANKS = re.compile(r"/banks=(?P<banks>\d+)x(?P<bank>\d+)B/working-set=(?P<total>\d+)B")


def fail(message):
    raise SystemExit(f"smoke failed: {message}")


def check_stage(directory, plan, runner):
    stage = directory / "stage"
    log_path = stage / "execution.log"
    if not log_path.is_file():
        fail(f"{directory.name} has no staged execution log")
    cells = [cell["cell_id"] for cell in plan["cells"]]
    if len(cells) < 2 or plan["max_cells_per_session"] != 1:
        fail(f"{directory.name} cannot demonstrate a cell-boundary pause")
    log = log_path.read_bytes()
    records = [json.loads(line) for line in log.splitlines()]
    events = [entry["event"] for entry in records]
    starts = [entry["case"]["cell_id"] for entry in records if entry["event"] == "cell-start"]
    completed = [entry["case"]["cell_id"] for entry in records if entry["event"] == "cell-complete"]
    if starts != cells or completed != cells:
        fail(f"{directory.name} repeated or missed a cell: {starts}, {completed}")
    if events.count("checkpoint-accepted") != len(cells):
        fail(f"{directory.name} did not checkpoint every completed cell")
    terminals = [event for event in events if event in ("paused", "complete", "failed")]
    if terminals != ["paused"] * (len(cells) - 1) + ["complete"]:
        fail(f"{directory.name} has terminal sequence {terminals}")
    if len({entry["session_id"] for entry in records if entry["event"] == "cell-start"}) != len(cells):
        fail(f"{directory.name} did not resume in a fresh session for each cell")
    previous = b""
    for index in range(1, len(cells) + 1):
        prefix = (directory / f"session-{index:02d}.log").read_bytes()
        if not prefix.startswith(previous) or not log.startswith(prefix) or len(prefix) <= len(previous):
            fail(f"{directory.name} session {index} did not append to its log")
        stdout = (directory / f"session-{index:02d}.stdout").read_text().splitlines()
        if not stdout or stdout[0] != f"GF2_CAMPAIGN_EXECUTION_LOG={log_path.resolve()}":
            fail(f"{directory.name} did not announce its log before work")
        previous = prefix
    if previous != log:
        fail(f"{directory.name} changed its log after its last session")
    if not (stage / "checkpoints" / "manifest.json").is_file():
        fail(f"{directory.name} has no checkpoint manifest")
    if any(event in ("execution-progress", "window-progress", "lock-hold") for event in events):
        fail(f"{directory.name} logged timing or a lock hold")
    if any("ns_per_call" in entry["details"] or entry["details"].get("windows", 0) != 0 for entry in records):
        fail(f"{directory.name} logged a timing sample")
    if (stage / "receipt.json").exists():
        fail(f"{directory.name} wrote a receipt")
    out = directory / "finalize-refusal"
    refused = subprocess.run([runner, "finalize", str(stage), str(out)], capture_output=True, text=True)
    if refused.returncode == 0 or "is a non-timed smoke stage" not in refused.stderr or out.exists():
        fail(f"{directory.name} finalize did not refuse its smoke stage: {refused.stderr}")
    return len(cells)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage", required=True)
    parser.add_argument("--command", required=True)
    parser.add_argument("--record", required=True)
    parser.add_argument("--oracle", required=True)
    parser.add_argument("--families", required=True, nargs="+")
    parser.add_argument("--runner", required=True)
    arguments = parser.parse_args()
    stage = pathlib.Path(arguments.stage)

    # Semantics: every oracle case passes, and the record carries the case and
    # check counts that run reported.
    oracle = pathlib.Path(arguments.oracle).read_text().splitlines()
    if not oracle or any(not case.startswith("PASS ") for case in oracle):
        fail(f"the semantic oracle reported {[c for c in oracle if not c.startswith('PASS ')]}")
    checks = sum(int(case.rsplit(": ", 1)[1].split()[0]) for case in oracle)

    # Cell generation: every family of the frozen addendum transcribes and
    # verifies, including one whose arms this smoke does not drive.
    comparator = stage / "comparator-cells.json"
    if not comparator.is_file():
        fail("the comparator family did not transcribe")

    lines = [f"PASS semantic oracle: {len(oracle)} cases, {checks} checks"]
    executables = {}
    external = {}
    for family in arguments.families:
        directory = stage / family
        plan = json.loads((directory / "plan.json").read_text())
        staged_cells = check_stage(directory, plan, arguments.runner)
        record = json.loads((directory / "smoke.json").read_text())
        if record["family"] != f"{family}-arm-smoke":
            fail(f"{family} recorded family {record['family']!r}")
        declared = {
            cell["cell_id"]: cell["cache_state"]
            for cell in json.loads((directory / "smoke-addendum.json").read_text())["cells"]
        }
        dispatches = [arm for cell in record["cells"] for arm in cell["arms"]]
        fixtures = 0
        for cell in record["cells"]:
            cell_id = cell["cell_id"]
            if declared.get(cell_id) != cell["cache_state"]:
                fail(f"{cell_id} reports cache state {cell['cache_state']!r}")
            for arm in cell["arms"]:
                if arm["role"] != VALIDATION_ROLE:
                    fail(f"{arm['arm']} in {cell_id} took the {arm['role']!r} position")
                if arm["windows"] != 0:
                    fail(f"{arm['arm']} in {cell_id} reported {arm['windows']} timing windows")
                if arm["cache_state_declared"] != cell["cache_state"]:
                    fail(f"{arm['arm']} in {cell_id} was sent another cache state")
                if arm["cache_state_applied"] != cell["cache_state"]:
                    fail(f"{arm['arm']} in {cell_id} applied another cache state")
                executables[arm["executable"]] = arm["executable_sha256"]
                if arm["selected_path"].startswith("m4ri/"):
                    observed = EXTERNAL_OBJECT.search(arm["selected_path"])
                    if not observed:
                        fail(f"{arm['arm']} in {cell_id} named no loaded shared object")
                    key = (observed["path"], observed["digest"])
                    external[key] = external.get(key, 0) + 1
                else:
                    banks = GF2_BANKS.search(arm["selected_path"])
                    if not banks:
                        fail(f"{arm['arm']} in {cell_id} named no fixture banks")
                    if int(banks["banks"]) * int(banks["bank"]) != int(banks["total"]):
                        fail(f"{arm['arm']} in {cell_id} reports banks the working set contradicts")
                    fixtures += 1
        recorded = sorted(cell["cell_id"] for cell in record["cells"])
        if recorded != sorted(declared):
            fail(f"{family} validated {recorded} rather than the declared {sorted(declared)}")

        lines.append(f"PASS campaign {record['campaign_id']}: {len(record['cells'])} cells")
        pauses = staged_cells - 1
        lines.append(
            f"PASS staged campaign {record['campaign_id']}: {staged_cells} sessions, "
            f"{pauses} pause{'s' if pauses != 1 else ''}, append-only log and checkpoints, "
            "0 timing samples, finalize refused"
        )
        for cell in record["cells"]:
            names = " and ".join(arm["arm"] for arm in cell["arms"])
            windows = sum(arm["windows"] for arm in cell["arms"])
            lines.append(
                f"PASS cell {cell['cell_id']}: {names} validated, "
                f"cache {cell['cache_state']}, {windows} timing windows"
            )
        for name in sorted({arm["arm"] for arm in dispatches}):
            own = [arm for arm in dispatches if arm["arm"] == name]
            lines.append(
                f"PASS arm {name}: {len(own)} dispatches, "
                f"{sum(arm['windows'] for arm in own)} timing windows"
            )
            for route in sorted({arm["selected_path"] for arm in own}):
                lines.append(f"PASS route {name}: {route}")
        lines.append(
            f"PASS {family}: {len(record['cells'])} cells, {len(dispatches)} dispatches, "
            f"{sum(arm['windows'] for arm in dispatches)} timing windows, "
            f"{fixtures} gf2 dispatches naming a working set their banks account for"
        )

    if external:
        if len(external) != 1:
            fail(f"the external arms report {len(external)} distinct shared objects: {external}")
        (path, digest), dispatched = next(iter(external.items()))
        lines.append(
            f"PASS external comparator object: {path} sha256 {digest}, "
            f"observed by {dispatched} dispatches"
        )

    comparator_cells = json.loads(comparator.read_text())["cells"]
    lines.append(
        f"PASS 2037941f-dense-matvec-vs-m4ri cells: {len(comparator_cells)} cells transcribe "
        "and verify against the frozen addendum"
    )

    record = pathlib.Path(arguments.record)
    with record.open("w") as handle:
        print(
            "# Dense-parity harness non-timed arm smoke (jit:e1f9a78f)\n"
            f"# command: {arguments.command}\n"
            "# contract: benchmark-ab-runner smoke (tuning_campaign_support::arm::smoke)\n"
            "# every line below is projected from the semantic oracle's output and the\n"
            "# families' smoke records under target/; a rerun on the same executables\n"
            "# reproduces it byte for byte",
            file=handle,
        )
        for path, digest in sorted(executables.items()):
            print(f"# {os.path.basename(path)} ({path}) sha256: {digest}", file=handle)
        for line in lines:
            print(line, file=handle)
    print(record.read_text(), end="", file=sys.stderr)


if __name__ == "__main__":
    main()
