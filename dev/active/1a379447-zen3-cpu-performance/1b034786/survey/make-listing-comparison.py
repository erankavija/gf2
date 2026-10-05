#!/usr/bin/env python3
"""Compare gf2-core's M4RM listing across the argument checks (jit:1b034786).

Reads `inputs/before-checks/m4rm.asm.txt`, the listing `regen-asm.sh` writes
for the tree without the checks, and the committed `m4rm.asm.txt` beside
`gf2-core`'s M4RM source, both for the symbols the committed listing names.
Writes `listing-comparison.json` beside itself with each symbol's instruction
text digests under `asm_listing.RULE`. A differing symbol is the expected
outcome and does not fail the script.

Usage: make-listing-comparison.py
"""

import hashlib
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = Path(
    subprocess.run(
        ["git", "-C", str(HERE), "rev-parse", "--show-toplevel"],
        capture_output=True, check=True, text=True,
    ).stdout.strip()
)
_shared = subprocess.run(
    ["git", "-C", str(ROOT), "ls-files", "--", ":(glob)**/repository_files.py"],
    capture_output=True, check=True, text=True,
).stdout.split()
_live = [path for path in _shared if "inputs" not in Path(path).parts[:-1]]
if len(_live) != 1:
    raise SystemExit(f"{len(_live)} live repository_files.py files; exactly one must exist")
sys.path.insert(0, str(ROOT / Path(_live[0]).parent))
import asm_listing  # noqa: E402
import repository_files  # noqa: E402

LISTING = "m4rm.asm.txt"


def main():
    before = HERE / "inputs" / "before-checks" / LISTING
    current = ROOT / repository_files.live_file(ROOT, LISTING)
    rows = asm_listing.compare(before.read_text(), current.read_text())
    output = HERE / "listing-comparison.json"
    output.write_text(
        json.dumps(
            {
                "schema": "m4rm-listing-comparison-v1",
                "issue": "1b034786",
                "before": {
                    "listing": str(before.relative_to(ROOT)),
                    "sha256": hashlib.sha256(before.read_bytes()).hexdigest(),
                },
                "current": {
                    "listing": str(current.relative_to(ROOT)),
                    "sha256": hashlib.sha256(current.read_bytes()).hexdigest(),
                },
                "comparison_rule": asm_listing.RULE,
                "symbols": rows,
            },
            indent=2,
        )
        + "\n"
    )
    for row in rows:
        print(f"{row['symbol']}: {row['instruction_text']}")


if __name__ == "__main__":
    main()
