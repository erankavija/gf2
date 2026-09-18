#!/usr/bin/env python3
"""Judge the logical-buffer non-timed smoke and write its record.

The smoke is judged by what the arms and the smoke driver wrote, not by exit
codes: one parsed result line per arm per cell, an append-only execution log
whose first session is a byte prefix of the final log, one `cell-complete` per
declared cell, a terminal `complete` record, an immutable checkpoint per cell,
a stage holding no finalized receipt, and a handshake record whose every arm
reports zero timing windows. Every record line projects one of those
observations; the smoke's own contract is stated at
`tuning_campaign_support::arm::smoke`.
"""

import argparse
import hashlib
import json
import pathlib
import sys

SMOKE_SCHEMA = "logical-buffer-nontimed-smoke-v1"
# Journal events that exist only because a timing interval completed.
TIMING_EVENTS = ("execution-progress", "window-progress")


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
    parser.add_argument("--isal-arm")
    parser.add_argument("--families", required=True, nargs="+")
    arguments = parser.parse_args()
    stage = pathlib.Path(arguments.stage)

    # Semantics: every oracle case passes, and the record carries the case and
    # check counts that run reported.
    oracle = pathlib.Path(arguments.oracle).read_text().splitlines()
    if not oracle or any(not case.startswith("PASS ") for case in oracle):
        fail(f"the semantic oracle reported {[c for c in oracle if not c.startswith('PASS ')]}")
    checks = sum(int(case.rsplit(": ", 1)[1].split()[0]) for case in oracle)

    lines = [f"PASS semantic oracle: {len(oracle)} cases, {checks} checks"]
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
        for cell in handshake["cells"]:
            cell_id = cell["cell_id"]
            if declared.get(cell_id) != cell["cache_state"]:
                fail(f"{cell_id} reports cache state {cell['cache_state']!r}")
            for arm in cell["arms"]:
                if arm["windows"] != 0:
                    fail(f"{arm['arm']} in {cell_id} reported {arm['windows']} timing windows")
                if arm["cache_state_applied"] != cell["cache_state"]:
                    fail(f"{arm['arm']} in {cell_id} applied another cache state")
                if not arm["selected_path"]:
                    fail(f"{arm['arm']} in {cell_id} reported no route provenance")
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
            f"PASS {family} schema: handshake record decodes as {handshake['schema']}, "
            f"{len(cells)} cells"
        )

    digests = [("logical-arm", arguments.gf2_arm)]
    if arguments.isal_arm:
        digests.append(("logical-isal-arm", arguments.isal_arm))

    record = pathlib.Path(arguments.record)
    with record.open("w") as handle:
        print(
            "# Logical-buffer harness non-timed wire smoke (jit:bb769456)\n"
            f"# command: {arguments.command}\n"
            "# contract: benchmark-ab-runner smoke (tuning_campaign_support::arm::smoke)\n"
            "# every line projects the semantic oracle, the stage execution logs, the\n"
            "# checkpoint stores or the handshake records under target/",
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
