#!/usr/bin/env python3
"""Build the throwaway smoke addenda of the dense-parity harness.

The smoke drives arms, not a measurement: each family is restricted to a few
cells and its ledger path is moved under `target/`, so the smoke names no
committed ledger and nothing it writes is evidence. Every other field is the
harness transcription of the frozen addendum, unchanged.
"""

import argparse
import json
import pathlib

# The smallest anchor plus one cell in a second cache state or arm pairing, so
# warm, streaming, the frozen cold call count, the scalar-reference arm and the
# retained-state comparator all reach an arm.
SMOKE_CELLS = {
    "2037941f-dense-isolated-fused-parity": [
        "and-popcnt-8w-warm",
        "and-popcnt-8w-streaming",
    ],
    "2037941f-dense-allocated-matvec": [
        "matvec-r1024-8w-warm",
        "matvec-r1024-8w-cold",
        "matvec-r1024-8w-scalar-reference-warm",
    ],
    "2037941f-dense-matvec-vs-m4ri": [
        "m4ri-gap-65x512-warm",
        "m4ri-gap-65x512-retained-warm",
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
            id=f"{family}-arm-smoke",
            description=(
                "Throwaway arm smoke of issue e1f9a78f: "
                f"{len(keep)} cells of {family} on a throwaway ledger under target/. "
                "It decides nothing, adopts nothing and no receipt cites it."
            ),
        )
        ledger = directory / "ledger.jsonl"
        addendum["family_wise"] = dict(addendum["family_wise"], ledger_path=str(ledger))
        addendum["cells"] = [cell for cell in addendum["cells"] if cell["cell_id"] in keep]
        if len(addendum["cells"]) != len(keep):
            raise SystemExit(f"{family} no longer declares every smoke cell {keep}")
        ledger.write_text("")
        (directory / "smoke-addendum.json").write_text(json.dumps(addendum, indent=2) + "\n")
        print(f"{directory / 'smoke-addendum.json'}: {len(keep)} cells")


if __name__ == "__main__":
    main()
