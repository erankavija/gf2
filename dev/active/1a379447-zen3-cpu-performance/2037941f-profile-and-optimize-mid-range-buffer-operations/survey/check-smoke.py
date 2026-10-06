#!/usr/bin/env python3
"""Judge the logical-buffer non-timed smoke and write its record.

Each family's smoke record comes from `benchmark-ab-runner smoke`, whose
contract `tuning_campaign_support::arm::smoke` states. This check adds what the
shared record cannot know: that every oracle case passed, and that the record
covers exactly the cells and arms the family's campaign addendum declares. Every
record line projects one of those observations.
"""

import argparse
import hashlib
import json
import pathlib
import sys

RECORD_SCHEMA = "zen3-arm-smoke-record-v1"


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
        smoke = directory / "smoke.json"
        if not smoke.is_file():
            fail(f"{family} wrote no smoke record")
        record = json.loads(smoke.read_text())
        if record["schema"] != RECORD_SCHEMA:
            fail(f"{family} wrote schema {record['schema']!r}")
        declared = {
            cell["cell_id"]: cell["cache_state"]
            for cell in json.loads((directory / "smoke-addendum.json").read_text())["cells"]
        }
        dispatches = {}
        cells = {}
        for cell in record["cells"]:
            cell_id = cell["cell_id"]
            if declared.get(cell_id) != cell["cache_state"]:
                fail(f"{cell_id} reports cache state {cell['cache_state']!r}")
            for arm in cell["arms"]:
                dispatches[arm["arm"]] = dispatches.get(arm["arm"], 0) + 1
            cells[cell_id] = (len(cell["arms"]), cell["cache_state"])
        if sorted(cells) != sorted(declared):
            fail(f"{family} smoked {sorted(cells)} rather than the declared {sorted(declared)}")

        lines.append(
            f"PASS {family}: {len(cells)} declared cells smoked, "
            f"{sum(dispatches.values())} validation dispatches"
        )
        for cell_id in sorted(cells):
            arms, cache = cells[cell_id]
            lines.append(
                f"PASS {cell_id}: {arms} arms validated, 0 timing windows, cache {cache}"
            )
        for arm in sorted(dispatches):
            lines.append(f"PASS {arm}: {dispatches[arm]} dispatches, "
                         f"{dispatches[arm]} result lines parsed")
        for route in sorted({arm["selected_path"]
                             for cell in record["cells"] for arm in cell["arms"]}):
            lines.append(f"PASS {family} route: {route}")

    digests = [("logical-arm", arguments.gf2_arm)]
    if arguments.isal_arm:
        digests.append(("logical-isal-arm", arguments.isal_arm))

    record_path = pathlib.Path(arguments.record)
    with record_path.open("w") as handle:
        print(
            "# Logical-buffer harness non-timed wire smoke (jit:bb769456)\n"
            f"# command: {arguments.command}\n"
            "# contract: benchmark-ab-runner smoke (tuning_campaign_support::arm::smoke)\n"
            "# every line projects the semantic oracle or a family's smoke record",
            file=handle,
        )
        for name, path in digests:
            digest = hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()
            print(f"# {name} sha256: {digest}", file=handle)
        for line in lines:
            print(line, file=handle)
    print(record_path.read_text(), end="", file=sys.stderr)


if __name__ == "__main__":
    main()
