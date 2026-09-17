#!/usr/bin/env python3
"""Build the throwaway smoke addenda of the logical-buffer harness.

The smoke exercises the wire, the cache policies and checkpoint/resume, not a
measurement: each family is restricted to two cells and its ledger path is
moved under `target/`, so the smoke names no committed ledger and nothing it
writes is evidence. Every other field is the harness transcription of the
frozen addendum, unchanged.
"""

import argparse
import json
import pathlib

# Two cells per family: the smallest anchor plus one cell in a second cache
# state, so warm, streaming and the frozen cold call count all reach the wire.
SMOKE_CELLS = {
    "2037941f-logical-isolated-xor": ["xor-8w-a64-warm", "xor-8w-a64-streaming"],
    "2037941f-logical-public-row-xor": ["row-xor-8w-full-warm", "row-xor-8w-full-streaming"],
    "2037941f-logical-nr-construction": [
        "nr-construct-bg2-256-49-z9-8w-warm",
        "nr-construct-bg2-256-49-z9-8w-cold",
    ],
    "2037941f-logical-isal-base-gap": [
        "isal-base-gap-8w-a64-warm",
        "isal-base-gap-8w-a64-streaming",
    ],
}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage", required=True)
    parser.add_argument("--families", required=True, nargs="+")
    arguments = parser.parse_args()

    stage = pathlib.Path(arguments.stage)
    for family in arguments.families:
        keep = SMOKE_CELLS[family]
        directory = stage / family
        addendum = json.loads((directory / "addendum.a.json").read_text())
        addendum["family"] = dict(
            addendum["family"],
            id=f"{family}-wire-smoke",
            description=(
                "Throwaway wire smoke of issue bb769456: two cells of "
                f"{family} on a throwaway ledger under target/. It decides nothing, "
                "adopts nothing and no receipt cites it."
            ),
        )
        ledger = directory / "ledger.jsonl"
        addendum["family_wise"] = dict(
            addendum["family_wise"], ledger_path=str(ledger)
        )
        addendum["cells"] = [cell for cell in addendum["cells"] if cell["cell_id"] in keep]
        if len(addendum["cells"]) != len(keep):
            raise SystemExit(f"{family} no longer declares every smoke cell {keep}")
        ledger.write_text("")
        (directory / "smoke-addendum.json").write_text(json.dumps(addendum, indent=2) + "\n")
        print(f"{directory / 'smoke-addendum.json'}: {len(keep)} cells")


if __name__ == "__main__":
    main()
