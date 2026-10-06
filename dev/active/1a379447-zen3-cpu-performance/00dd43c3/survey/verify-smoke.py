#!/usr/bin/env python3
"""Check the shared runner's untimed record for both residual routes."""

import hashlib
import json
import sys
from pathlib import Path

record = json.load(open(sys.argv[1]))
addendum = json.load(open(sys.argv[2]))
plan_path = Path(sys.argv[3])
plan = json.loads(plan_path.read_text())
if record["plan_sha256"] != hashlib.sha256(plan_path.read_bytes()).hexdigest():
    raise SystemExit("smoke record belongs to a different plan")
declared = {cell["cell_id"] for cell in addendum["cells"]}
observed = {cell["cell_id"] for cell in record["cells"]}
if observed != declared:
    raise SystemExit(f"smoke cells {observed} differ from frozen cells {declared}")
for cell in record["cells"]:
    direction = cell["cell_id"].split("-", 1)[0]
    expected = {
        "residual-scalar": f"bitvec-residual-scalar-funnel-{direction}",
        "residual-gated": f"bitvec-residual-bmi2-funnel-{direction}",
    }
    arms = {arm["arm"]: arm for arm in cell["arms"]}
    if set(arms) != set(expected):
        raise SystemExit(f"{cell['cell_id']}: arms {set(arms)} differ from {set(expected)}")
    for name, path in expected.items():
        arm = arms[name]
        executable = Path(plan["arms"][name]["executable"])
        if Path(arm["executable"]).resolve() != executable.resolve():
            raise SystemExit(f"{cell['cell_id']} {name}: smoke used another executable")
        if arm["executable_sha256"] != hashlib.sha256(executable.read_bytes()).hexdigest():
            raise SystemExit(f"{cell['cell_id']} {name}: smoked executable bytes differ")
        if arm["role"] != "validation" or arm["windows"] != 0:
            raise SystemExit(f"{cell['cell_id']} {name}: smoke measured a timing window")
        if arm["selected_path"] != path:
            raise SystemExit(f"{cell['cell_id']} {name}: executed {arm['selected_path']!r}, expected {path!r}")
print("all frozen cases used both residual routes with zero timing windows")
