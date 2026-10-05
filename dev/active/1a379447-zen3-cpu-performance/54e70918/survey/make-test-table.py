#!/usr/bin/env python3
"""Tabulates the implementations each list-form test ran (jit:54e70918).

Reads `test-logs/<configuration>-implementations.txt`, the logs
`run-suites.sh` writes with the output of each passing test, and writes
`test-implementations.json` and `test-implementations.md` beside itself: one
row per test that printed an `implementation:` line, with the distinct labels
it printed in each feature configuration, in print order.

Exits nonzero after writing when a log holds a failed test, when a listed test
printed no label in some configuration, or when a test of the two targets the
issue names is not listed.

Usage: make-test-table.py
"""

import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ISSUE = "54e70918"
CONFIGURATIONS = ("default", "simd", "all-features", "no-simd")
NAMED_TARGETS = ("gf2-core::simd_equiv_demo", "gf2-core::simd_equiv_dispatch_hoist")
STATUS = re.compile(r"^\s+([A-Z]+) \[[^\]]*\] \(\s*\d+/\d+\) (\S+) (\S+)$")
LABEL = re.compile(r"^\s+implementation: (.*)$")
COMMAND = re.compile(r"^# command: (.*)$")


def read(configuration):
    """`(command, {(binary, test): [labels]}, [failed tests])` of one log."""
    command, tests, failed, current = None, {}, [], None
    log = HERE / "test-logs" / f"{configuration}-implementations.txt"
    for line in log.read_text().splitlines():
        if match := COMMAND.match(line):
            command = match[1]
        elif match := STATUS.match(line):
            current = (match[2], match[3])
            tests.setdefault(current, [])
            if match[1] != "PASS":
                failed.append(" ".join(current))
        elif (match := LABEL.match(line)) and current:
            if match[1] not in tests[current]:
                tests[current].append(match[1])
    return command, tests, failed


def main():
    commands, runs, problems = {}, {}, []
    for configuration in CONFIGURATIONS:
        commands[configuration], runs[configuration], failed = read(configuration)
        problems += [f"{configuration}: {test} did not pass" for test in failed]
    listed = sorted({test for run in runs.values() for test, labels in run.items() if labels})
    for configuration, run in runs.items():
        problems += [
            f"{configuration}: {' '.join(test)} printed no implementation"
            for test in listed if not run.get(test)
        ]
        problems += [
            f"{configuration}: {' '.join(test)} is not a list-form test"
            for test in run if test[0] in NAMED_TARGETS and test not in listed
        ]
    rows = [
        {
            "binary": binary,
            "test": test,
            "implementations": {c: runs[c].get((binary, test), []) for c in CONFIGURATIONS},
        }
        for binary, test in listed
    ]
    record = {
        "issue": ISSUE,
        "commands": commands,
        "named_targets": list(NAMED_TARGETS),
        "tests": rows,
        "problems": problems,
    }
    (HERE / "test-implementations.json").write_text(json.dumps(record, indent=1) + "\n")

    lines = [
        f"# Implementations each list-form test ran (jit:{ISSUE})",
        "",
        "> **Diátaxis Type:** Research",
        "",
        "Written by `make-test-table.py` from the `*-implementations.txt` logs of",
        "`run-suites.sh`. A cell holds the distinct labels the test printed in that",
        "feature configuration, in print order.",
        "",
        "| Test | " + " | ".join(f"`{c}`" for c in CONFIGURATIONS) + " |",
        "|---|" + "---|" * len(CONFIGURATIONS),
    ]
    for row in rows:
        cells = ["; ".join(row["implementations"][c]) or "none" for c in CONFIGURATIONS]
        lines.append(f"| `{row['binary']} {row['test']}` | " + " | ".join(cells) + " |")
    lines += ["", f"Problems: {len(problems)}."] + [f"- {problem}" for problem in problems]
    (HERE / "test-implementations.md").write_text("\n".join(lines) + "\n")

    for problem in problems:
        print(problem, file=sys.stderr)
    print(f"{len(rows)} tests listed; {len(problems)} problems")
    return 1 if problems else 0


if __name__ == "__main__":
    raise SystemExit(main())
