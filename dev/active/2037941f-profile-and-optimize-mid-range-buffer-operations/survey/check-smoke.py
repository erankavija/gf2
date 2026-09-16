#!/usr/bin/env python3
"""Judge the logical-buffer runner smoke and write its record.

The smoke is judged by what the arms and the runner wrote, not by exit codes:
one parsed result line per arm per role, an append-only execution log whose
first session is a byte prefix of the final log, one `cell-complete` per cell,
a terminal `complete` record, and a decoding acceptance summary. Every line of
the record is observed at run time and carries no clock reading, so a rerun on
the same executables reproduces it byte for byte.
"""

import argparse
import hashlib
import json
import pathlib
import sys


def events(path):
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def fail(message):
    raise SystemExit(f"smoke failed: {message}")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage", required=True)
    parser.add_argument("--record", required=True)
    parser.add_argument("--gf2-arm", required=True)
    parser.add_argument("--isal-arm")
    parser.add_argument("--families", required=True, nargs="+")
    arguments = parser.parse_args()
    stage = pathlib.Path(arguments.stage)

    lines = []
    for family in arguments.families:
        directory = stage / family
        log = directory / "stage" / "execution.log"
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

        spawns = sum(1 for r in records if r["event"] == "child-spawn")
        exits = [r for r in records if r["event"] == "child-exit"]
        diagnostics = [r for r in records if r["event"] == "child-diagnostic"]
        nonzero = [r for r in exits if r["details"]["outcome"].get("exit_code") != 0]

        receipt = json.loads((directory / "receipt" / "receipt.json").read_text())
        parsed = {}
        cells = {}
        for cell in receipt["cells"]:
            pairs = cell.get("pairs") or []
            if not pairs:
                fail(f"{cell['cell_id']} carries no paired execution")
            states = set()
            for pair in pairs:
                for role in ("baseline", "candidate"):
                    arm = cell[f"{role}_arm"]
                    execution = pair[role]
                    if not execution.get("windows"):
                        fail(f"{arm} in {cell['cell_id']} reported no window")
                    states.add(execution["cache_state_applied"])
                    parsed[arm] = parsed.get(arm, 0) + 1
            cells[cell["cell_id"]] = (len(pairs), sorted(states))
        if sorted(cells) != sorted(keys):
            fail(f"{family} receipt cells {sorted(cells)} differ from log {sorted(keys)}")
        if nonzero or diagnostics:
            fail(f"{family}: {len(nonzero)} arm children failed, "
                 f"{len(diagnostics)} wrote diagnostics")
        if spawns != len(exits) or spawns != sum(parsed.values()):
            fail(f"{family}: {spawns} spawns, {len(exits)} exits, "
                 f"{sum(parsed.values())} result lines")

        summary = json.loads((directory / "receipt" / "acceptance-summary.json").read_text())
        codes = sorted({finding["code"] for finding in summary["findings"]})

        lines.append(f"PASS {family}: {sessions} sessions, resume repeated no cell")
        for cell_id in sorted(cells):
            pairs, states = cells[cell_id]
            lines.append(
                f"PASS {cell_id}: {pairs} paired executions, cache {'+'.join(states)}"
            )
        for arm in sorted(parsed):
            lines.append(f"PASS {arm}: {parsed[arm]} handshakes, "
                         f"{parsed[arm]} result lines parsed")
        lines.append(
            f"PASS {family} wire: {spawns} child spawns, {len(exits)} clean exits, "
            f"{len(diagnostics)} child diagnostics"
        )
        lines.append(
            f"PASS {family} schema: acceptance summary decodes, verdict "
            f"{summary['verdict']}, finding codes {','.join(codes) or 'none'}"
        )

    digests = [("logical-arm", arguments.gf2_arm)]
    if arguments.isal_arm:
        digests.append(("logical-isal-arm", arguments.isal_arm))

    record = pathlib.Path(arguments.record)
    with record.open("w") as handle:
        print(
            "# Logical-buffer harness runner-wire smoke (jit:bb769456)\n"
            "# command: dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/"
            "survey/run-logical-harness.sh smoke\n"
            "# every line below is observed at run time from the stage execution logs and the\n"
            "# finalized throwaway receipts under target/; the record carries no clock reading\n"
            "# and no timing sample, so a rerun on the same executables reproduces it byte for\n"
            "# byte and the smoke cannot serve as a pilot",
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
