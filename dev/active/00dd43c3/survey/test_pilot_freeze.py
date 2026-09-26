#!/usr/bin/env python3
"""Check the frozen candidate pilot against its committed profile evidence."""

import hashlib
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
PROFILE = ROOT / "dev/bench_results/c04dd4ac/residual-shift-profile"
ACTIVE = ROOT / "dev/active/00dd43c3"


class PilotFreezeTest(unittest.TestCase):
    def test_pilot_covers_the_profiles_material_residual_cells(self):
        summary = json.loads((PROFILE / "acceptance-summary.json").read_text())
        profile = json.loads((PROFILE / "inputs/family-addendum.json").read_text())
        pilot = json.loads((ACTIVE / "pilot-addendum.json").read_text())
        context = json.loads((ACTIVE / "profile-context.json").read_text())

        material = {cell["cell_id"] for cell in summary["cells"] if cell["decision"] == "improved"}
        self.assertEqual({cell["cell_id"] for cell in pilot["cells"]}, material)
        self.assertEqual({cell["role"] for cell in pilot["cells"]}, {"exploratory"})
        self.assertEqual({cell["objective"] for cell in pilot["cells"]}, {"improvement"})
        self.assertEqual({cell["cell_id"] for cell in profile["cells"]} & material, material)
        self.assertEqual(
            {cell["cell_id"].split("-", 1)[0] for cell in pilot["cells"]},
            {"left", "right"},
        )
        self.assertEqual(
            context["profile_receipt"]["sha256"],
            hashlib.sha256((PROFILE / "receipt.json").read_bytes()).hexdigest(),
        )

    def test_plan_runs_both_lanes_on_the_same_residual_case(self):
        pilot = json.loads((ACTIVE / "pilot-addendum.json").read_text())
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "plan.json"
            subprocess.run(
                ["python3", "-B", "dev/active/00dd43c3/survey/make-plan.py", str(path),
                 "/tmp/shifts-arm", "/tmp/unused-shift-lock", "--label", "pilot",
                 "--campaign-id", "test-residual-pilot"],
                cwd=ROOT, check=True, capture_output=True, text=True,
            )
            plan = json.loads(path.read_text())
        self.assertEqual({arm["executable"] for arm in plan["arms"].values()}, {"/tmp/shifts-arm"})
        self.assertEqual(
            {name: arm["environment"]["GF2_SHIFT_ARM"] for name, arm in plan["arms"].items()},
            {"residual-scalar": "residual-scalar", "residual-gated": "residual-gated"},
        )
        declared = {cell["cell_id"]: cell for cell in pilot["cells"]}
        self.assertEqual({cell["cell_id"] for cell in plan["cells"]}, set(declared))
        for cell in plan["cells"]:
            source = declared[cell["cell_id"]]
            self.assertEqual(cell["case"]["residual_offset"], source["workload"]["size"]["residual_offset"])
            self.assertEqual(cell["baseline_arm"], "residual-scalar")
            self.assertEqual(cell["candidate_arm"], "residual-gated")
            self.assertIsNone(cell["pilot_pairs"])


if __name__ == "__main__":
    unittest.main()
