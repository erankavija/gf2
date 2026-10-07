#!/usr/bin/env python3
"""Print the unique shifts benchmark executable from Cargo JSON output."""

import json
import os
import sys

paths = set()
for line in open(sys.argv[1]):
    row = json.loads(line)
    if (
        row.get("reason") == "compiler-artifact"
        and row.get("target", {}).get("name") == "shifts"
        and row.get("executable")
    ):
        paths.add(os.path.realpath(row["executable"]))
if len(paths) != 1:
    raise SystemExit(f"expected one shifts executable, found {sorted(paths)}")
path = paths.pop()
if not os.access(path, os.X_OK):
    raise SystemExit(f"{path} is not executable")
print(path)
