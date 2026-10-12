"""Shared locators for this issue's generators (jit:7dbe1393).

Importing this module puts the shared `repository_files` on `sys.path`; it
resolves files under the root git reports.
"""

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


def _shared_scripts():
    """Root-relative directory of the one `repository_files.py` outside receipt snapshots."""
    listing = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "--", ":(glob)**/repository_files.py"],
        capture_output=True, check=True, text=True,
    ).stdout.split()
    live = [path for path in listing if "inputs" not in Path(path).parts[:-1]]
    if len(live) != 1:
        raise SystemExit(f"{len(live)} live repository_files.py files; exactly one must exist")
    return Path(live[0]).parent


sys.path.insert(0, str(ROOT / _shared_scripts()))
import repository_files  # noqa: E402

FAMILY = "7dbe1393-matvec-ragged-tail"
REPEAT_FAMILY = "7dbe1393-matvec-ragged-tail-v2"
PREDECESSOR_FAMILY = "4337c02e-matvec-simd-threshold"

# The three lanes of the allocated product and the frozen cell that reaches
# each: (lane, family, cell stem, arm). The scalar lane is reached through the
# predecessor family's pinned-lane arm at the same strides.
LANES = [
    ("scalar", PREDECESSOR_FAMILY, "lane-r1024-{words}w-warm", "matvec-lane-scalar"),
    ("simd", FAMILY, "ragged-r1024-{words}w{control}-warm", "matvec-a"),
    ("parity-kernel", FAMILY, "ragged-r1024-{words}w{control}-warm", "matvec-lane-parity"),
]


def campaign(family):
    """Root-relative pilot campaign addendum of `family`."""
    name = {
        FAMILY: "matvec-ragged-tail.json",
        PREDECESSOR_FAMILY: "matvec-simd-threshold.json",
    }[family]
    return repository_files.live_file(ROOT, name)
