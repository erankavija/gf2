#!/usr/bin/env python3
"""Compare plan projections across the generalisation of `campaign_plan.py` (jit:bfc2ceab).

Usage: make-plan-projection-comparison.py, with no arguments

Projects every frozen addendum of each live `make-plan.py` that calls
`campaign_plan.main` with the anchor projector under `inputs/plan-anchor` and
with the working tree's, under fixed arguments, and writes
`plan-projection-comparison.json` beside itself. Exits nonzero after writing
when a pair of plans differs.
"""

import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path

from locate import HERE, ISSUE, ROOT, repository_files

ANCHOR = HERE / "inputs" / "plan-anchor" / "campaign_plan.py"
CURRENT = ROOT / repository_files.live_file(ROOT, "campaign_plan.py")
LANE_ENTRY = "campaign_plan.main("
ARGUMENTS = [
    "--label", "pilot", "--campaign-id", "plan-projection-comparison", "--campaign-seed", "1",
    "--lock", "/nonexistent/host.lock", "--executable", "/nonexistent/target/release/arm",
    "--producing-manifest", "producing-inputs.json", "--max-cells-per-session", "3",
    "--pilot-pairs", "12",
]
# Runs a caller with one projector's source installed as the `campaign_plan`
# module at the live projector's location, which the module resolves the lock
# wrapper from.
RUNNER = (
    "import runpy, sys, types\n"
    "source, location, caller = sys.argv[1:4]\n"
    "module = types.ModuleType('campaign_plan')\n"
    "module.__file__ = location\n"
    "exec(compile(open(source).read(), location, 'exec'), module.__dict__)\n"
    "sys.modules['campaign_plan'] = module\n"
    "sys.path.insert(0, caller.rsplit('/', 1)[0])\n"
    "sys.argv = sys.argv[3:]\n"
    "runpy.run_path(caller, run_name='__main__')\n"
)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def projected(projector, caller, addendum, scratch):
    output = Path(scratch) / "plan.json"
    subprocess.run(
        [sys.executable, "-B", "-c", RUNNER, str(projector), str(CURRENT), str(ROOT / caller),
         "--addendum", addendum, *ARGUMENTS, "--output", str(output)],
        cwd=ROOT, check=True, capture_output=True,
    )
    return digest(output)


def addenda(caller):
    """The frozen addenda of the caller's family: those of the nearest directory holding any."""
    for directory in (ROOT / caller).parents:
        found = sorted(directory.glob("addendum-*.json"))
        if found or directory == ROOT:
            return found


def main():
    if sys.argv[1:]:
        raise SystemExit(__doc__)
    callers = [path for path in repository_files.tracked_files(ROOT, "make-plan.py")
               if LANE_ENTRY in (ROOT / path).read_text()]
    rows = []
    with tempfile.TemporaryDirectory() as scratch:
        for caller in callers:
            for addendum in addenda(caller):
                relative = str(addendum.relative_to(ROOT))
                before = projected(ANCHOR, caller, relative, scratch)
                after = projected(CURRENT, caller, relative, scratch)
                rows.append({"caller": caller, "addendum": relative, "anchor_plan_sha256": before,
                             "current_plan_sha256": after, "plan_bytes": "same" if before == after
                             else "differ"})
    record = {
        "schema": "plan-projection-comparison-v1",
        "issue": ISSUE,
        "anchor_projector": {"path": str(ANCHOR.relative_to(ROOT)), "sha256": digest(ANCHOR)},
        "current_projector": {"path": str(CURRENT.relative_to(ROOT)), "sha256": digest(CURRENT)},
        "arguments": ARGUMENTS,
        "plan_count": len(rows),
        "differing_plan_count": sum(row["plan_bytes"] != "same" for row in rows),
        "plans": rows,
    }
    output = HERE / "plan-projection-comparison.json"
    output.write_text(json.dumps(record, indent=2) + "\n")
    print(f"{output.relative_to(ROOT)}: {record['plan_count']} plans, "
          f"{record['differing_plan_count']} differing")
    if not rows or record["differing_plan_count"]:
        raise SystemExit("a plan differs across the generalisation, or no caller was found")


if __name__ == "__main__":
    main()
