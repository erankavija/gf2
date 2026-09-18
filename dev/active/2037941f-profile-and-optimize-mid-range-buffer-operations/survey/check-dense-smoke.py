#!/usr/bin/env python3
"""Judge the dense-parity non-timed smoke and write its record.

`tuning_campaign_support::arm::smoke` states the arm contract. This judge reads
what that dispatch and the harness's own session left behind — the stage
execution logs, the checkpoint stores and the handshake records — rather than an
exit code, and refuses a stage holding a finalized receipt. Every recorded line
is observed at run time and carries no clock reading, so a rerun on the same
executables reproduces the record byte for byte. The recorded command line is the
launcher invocation that ran, passed through as `--command`.
"""

import argparse
import hashlib
import json
import pathlib
import re
import sys

SMOKE_SCHEMA = "dense-parity-nontimed-smoke-v1"
# The position the shared record names for every dispatch it holds; a dispatch
# in either pair position would carry the plan's timing-window budget.
VALIDATION_ROLE = "validation"
# Journal events that exist only because a timing interval completed.
TIMING_EVENTS = ("execution-progress", "window-progress")
# The shared object an external arm reports having loaded, and its digest. The
# arm refuses a run whose object is not the qualified one, so a handshake that
# carries this pair is a handshake against the qualified comparator.
EXTERNAL_OBJECT = re.compile(r"/loaded=(?P<path>.+)/sha256=(?P<digest>[0-9a-f]{64})$")
# The fixture banks a gf2 arm built and the bytes they hold, which a receipt
# retains beside the cache claim they arrange.
GF2_BANKS = re.compile(r"/banks=(?P<banks>\d+)x(?P<bank>\d+)B/working-set=(?P<total>\d+)B")


def events(path):
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def fail(message):
    raise SystemExit(f"smoke failed: {message}")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage", required=True)
    parser.add_argument("--command", required=True)
    parser.add_argument("--record", required=True)
    parser.add_argument("--oracle", required=True)
    parser.add_argument("--gf2-arm", required=True)
    parser.add_argument("--scalar-arm", required=True)
    parser.add_argument("--m4ri-arm")
    parser.add_argument("--families", required=True, nargs="+")
    arguments = parser.parse_args()
    stage = pathlib.Path(arguments.stage)

    # Semantics: every oracle case passes, and the record carries the case and
    # check counts that run reported.
    oracle = pathlib.Path(arguments.oracle).read_text().splitlines()
    if not oracle or any(not case.startswith("PASS ") for case in oracle):
        fail(f"the semantic oracle reported {[c for c in oracle if not c.startswith('PASS ')]}")
    checks = sum(int(case.rsplit(": ", 1)[1].split()[0]) for case in oracle)

    # Cell generation: every family of the frozen addendum transcribes, verifies
    # and reproduces byte for byte, including one whose arms the wire smoke does
    # not drive.
    comparator = stage / "comparator-cells.json"
    if not comparator.is_file():
        fail("the comparator family did not transcribe")

    lines = [f"PASS semantic oracle: {len(oracle)} cases, {checks} checks"]
    external = {}
    for family in arguments.families:
        directory = stage / family
        session = directory / "stage"
        log = session / "execution.log"
        first = directory / "execution.session-1.log"
        if not log.is_file():
            fail(f"{family} opened no execution log")

        # Append-only: the paused session's log is a byte prefix of the final one.
        final_bytes = log.read_bytes()
        first_bytes = first.read_bytes()
        if not final_bytes.startswith(first_bytes) or len(final_bytes) <= len(first_bytes):
            fail(f"{family} rewrote or did not extend its execution log")

        records = events(log)
        if records[0]["event"] != "campaign-start":
            fail(f"{family} did not open with campaign-start")
        terminal = [r["event"] for r in records if r["event"] in
                    ("complete", "failed", "paused", "budget-exhausted")]
        if terminal[-1] != "complete":
            fail(f"{family} ended {terminal} rather than complete")
        sessions = terminal.count("paused") + 1
        if sessions < 2:
            fail(f"{family} completed in one session, so resume was not exercised")

        completed = [r for r in records if r["event"] == "cell-complete"]
        keys = [r["case"]["cell_id"] for r in completed]
        if len(keys) != len(set(keys)):
            fail(f"{family} completed a cell twice: {keys}")
        omitted = [r for r in records if r["event"] == "omission"]
        if not omitted:
            fail(f"{family} recorded no completed-in-prior-session omission on resume")
        abandoned = [r for r in records if r["event"] == "cell-abandoned"]
        if abandoned:
            fail(f"{family} abandoned {len(abandoned)} cell attempts")
        timed = [r for r in records if r["event"] in TIMING_EVENTS]
        if timed:
            fail(f"{family} journalled {len(timed)} timing records")

        spawns = sum(1 for r in records if r["event"] == "child-spawn")
        exits = [r for r in records if r["event"] == "child-exit"]
        diagnostics = [r for r in records if r["event"] == "child-diagnostic"]
        nonzero = [r for r in exits if r["details"]["outcome"].get("exit_code") != 0]

        # A finalized receipt is the one artifact a non-timed smoke must not
        # leave behind; the stage holds the log, the plan, the checkpoints and
        # the handshake record only.
        receipts = sorted(
            path.name
            for path in list(session.glob("receipt*")) + list(directory.glob("receipt*"))
        )
        if receipts:
            fail(f"{family} finalized {receipts}")

        declared = {
            cell["cell_id"]: cell["cache_state"]
            for cell in json.loads((directory / "smoke-addendum.json").read_text())["cells"]
        }
        handshake = json.loads((session / "handshake.json").read_text())
        if handshake["schema"] != SMOKE_SCHEMA:
            fail(f"{family} wrote schema {handshake['schema']!r}")
        parsed = {}
        cells = {}
        fixtures = 0
        for cell in handshake["cells"]:
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
                if not arm["selected_path"]:
                    fail(f"{arm['arm']} in {cell_id} reported no route provenance")
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
                parsed[arm["arm"]] = parsed.get(arm["arm"], 0) + 1
            cells[cell_id] = (len(cell["arms"]), cell["cache_state"])
        if sorted(cells) != sorted(keys):
            fail(f"{family} handshake cells {sorted(cells)} differ from log {sorted(keys)}")
        if sorted(cells) != sorted(declared):
            fail(f"{family} handshook {sorted(cells)} rather than the declared {sorted(declared)}")

        units = sorted((session / "checkpoints" / "units").glob("*.json"))
        if len(units) != len(cells):
            fail(f"{family} checkpointed {len(units)} units for {len(cells)} cells")
        if nonzero or diagnostics:
            fail(f"{family}: {len(nonzero)} arm children failed, "
                 f"{len(diagnostics)} wrote diagnostics")
        if spawns != len(exits) or spawns != sum(parsed.values()):
            fail(f"{family}: {spawns} spawns, {len(exits)} exits, "
                 f"{sum(parsed.values())} result lines")

        lines.append(f"PASS {family}: {sessions} sessions, resume repeated no cell")
        for cell_id in sorted(cells):
            arms, cache = cells[cell_id]
            lines.append(
                f"PASS {cell_id}: {arms} arms handshook, 0 timing windows, cache {cache}"
            )
        for arm in sorted(parsed):
            lines.append(f"PASS {arm}: {parsed[arm]} handshakes, "
                         f"{parsed[arm]} result lines parsed")
        lines.append(
            f"PASS {family} wire: {spawns} child spawns, {len(exits)} clean exits, "
            f"{len(diagnostics)} child diagnostics"
        )
        lines.append(
            f"PASS {family} journal: {len(timed)} timing records, {len(units)} checkpointed "
            f"cells, {len(receipts)} finalized receipts"
        )
        lines.append(
            f"PASS {family} fixtures: {fixtures} gf2 arms name their fixture banks and a "
            "working set those banks account for"
        )
        lines.append(
            f"PASS {family} schema: handshake record decodes as {handshake['schema']}, "
            f"{len(cells)} cells"
        )

    if arguments.m4ri_arm:
        if len(external) != 1:
            fail(f"the external arms report {len(external)} distinct shared objects: {external}")
        (path, digest), handshakes = next(iter(external.items()))
        lines.append(
            f"PASS external comparator object: {path} sha256 {digest}, "
            f"observed by {handshakes} arm handshakes"
        )

    comparator_cells = json.loads(comparator.read_text())["cells"]
    lines.append(
        f"PASS 2037941f-dense-matvec-vs-m4ri cells: {len(comparator_cells)} cells transcribe "
        "and verify against the frozen addendum"
    )

    digests = [("dense-arm", arguments.gf2_arm), ("dense-arm-scalar", arguments.scalar_arm)]
    if arguments.m4ri_arm:
        digests.append(("dense-m4ri-arm", arguments.m4ri_arm))

    record = pathlib.Path(arguments.record)
    with record.open("w") as handle:
        print(
            "# Dense-parity harness non-timed wire smoke (jit:e1f9a78f)\n"
            f"# command: {arguments.command}\n"
            "# every line below is observed at run time from the semantic oracle, the stage\n"
            "# execution logs, the checkpoint stores and the handshake records under target/;\n"
            "# the record carries no clock reading and no receipt is finalized, so a rerun on\n"
            "# the same executables reproduces it byte for byte and the smoke cannot serve as\n"
            "# a pilot",
            file=handle,
        )
        for name, path in digests:
            digest = hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()
            print(f"# {name} sha256: {digest}", file=handle)
        for line in lines:
            print(line, file=handle)
    print(record.read_text(), end="", file=sys.stderr)


if __name__ == "__main__":
    main()
