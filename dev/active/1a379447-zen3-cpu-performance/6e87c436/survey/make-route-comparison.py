#!/usr/bin/env python3
"""Compare the paths the dense receipts measured with the paths this tree selects (jit:6e87c436).

Every pair of a receipt records the `selected_path` each arm reported. An
untimed arm smoke of the same campaign addendum on the working tree records the
path each arm reports there. This script joins the two by cell and arm and
writes `route-comparison.json` beside itself. A cell arm is `same` when the
smoke path is the one path every measured pair of that arm reports, and
`differs` otherwise; any `differs` fails the script after the record is written.

Usage: make-route-comparison.py CAMPAIGN=SMOKE_RECORD...
"""

import hashlib
import json
import sys

from locate import CAMPAIGNS, HERE, ROOT, repo_artifacts


def measured_paths(receipt):
    """Per (cell, arm), the distinct paths the receipt's pairs report."""
    paths = {}
    for cell in receipt["cells"]:
        for pair in cell["pairs"]:
            for side in ("baseline", "candidate"):
                execution = pair[side]
                paths.setdefault((cell["cell_id"], execution["arm"]), set()).add(
                    execution["selected_path"]
                )
    return paths


def main():
    smokes = dict(argument.split("=", 1) for argument in sys.argv[1:])
    if sorted(smokes) != sorted(CAMPAIGNS):
        raise SystemExit(__doc__)
    campaigns, differing = [], 0
    for campaign in CAMPAIGNS:
        directory = repo_artifacts.receipt(campaign)
        receipt = json.loads((ROOT / directory / "receipt.json").read_bytes())
        smoke_bytes = (ROOT / smokes[campaign]).read_bytes()
        smoke = json.loads(smoke_bytes)
        if smoke["addendum_sha256"] != receipt["addendum"]["sha256"]:
            raise SystemExit(f"{smokes[campaign]} smokes another addendum than {campaign} measured")
        current = {
            (cell["cell_id"], arm["arm"]): arm
            for cell in smoke["cells"]
            for arm in cell["arms"]
        }
        measured = measured_paths(receipt)
        if set(current) != set(measured):
            raise SystemExit(f"{campaign}: the smoke and the receipt name different cell arms")
        rows = []
        for (cell, arm), paths in sorted(measured.items()):
            now = current[(cell, arm)]["selected_path"]
            same = paths == {now}
            differing += not same
            rows.append(
                {
                    "cell_id": cell,
                    "arm": arm,
                    "measured_paths": sorted(paths),
                    "current_path": now,
                    "measured_executable_sha256": receipt["arms"][arm]["executable_sha256"],
                    "current_executable_sha256": current[(cell, arm)]["executable_sha256"],
                    "selection": "same" if same else "differs",
                }
            )
        campaigns.append(
            {
                "campaign_id": campaign,
                "receipt": str(directory / "receipt.json"),
                "addendum_sha256": receipt["addendum"]["sha256"],
                "smoke_record": smokes[campaign],
                "smoke_record_sha256": hashlib.sha256(smoke_bytes).hexdigest(),
                "cell_arms": len(rows),
                "differing_cell_arms": sum(row["selection"] == "differs" for row in rows),
                "rows": rows,
            }
        )
    output = HERE / "route-comparison.json"
    output.write_text(
        json.dumps(
            {"schema": "dense-route-comparison-v1", "issue": "6e87c436", "campaigns": campaigns},
            indent=2,
        )
        + "\n"
    )
    for campaign in campaigns:
        print(campaign["campaign_id"], campaign["cell_arms"], campaign["differing_cell_arms"])
    print(f"{output.relative_to(ROOT)}: {differing} differing cell arms")
    if differing:
        raise SystemExit("the current selection differs from a measured one")


if __name__ == "__main__":
    main()
