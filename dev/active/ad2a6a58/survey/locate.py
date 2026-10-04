"""Runtime locations of this family's generators (jit:ad2a6a58).

Every location is resolved under the repository root git reports: this family's
own files from this file's directory, the launcher by file name through the
shared `repository_files`, which also names harness and tool crates by package.
Importing this module puts the shared campaign scripts on `sys.path`.
"""

import os
import subprocess
import sys
from pathlib import Path

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = subprocess.run(
    ["git", "-C", HERE, "rev-parse", "--show-toplevel"], check=True, capture_output=True,
    text=True,
).stdout.strip()


def _shared_scripts():
    """Root-relative directory of the shared campaign scripts.

    Receipt input snapshots hold byte copies of it under an `inputs` directory;
    the live one is the path outside them.
    """
    listing = subprocess.run(
        ["git", "-C", ROOT, "ls-files", "--", ":(glob)**/repository_files.py"], check=True,
        capture_output=True, text=True,
    ).stdout.split()
    live = [path for path in listing if "inputs" not in path.split("/")[:-1]]
    if len(live) != 1:
        raise SystemExit(f"{len(live)} live repository_files.py files; exactly one must exist")
    return os.path.dirname(live[0])


SHARED = _shared_scripts()
sys.path.insert(0, os.path.join(ROOT, SHARED))
import repository_files  # noqa: E402

SURVEY = os.path.relpath(HERE, ROOT)
ISSUE = os.path.dirname(SURVEY)
LAUNCHER = repository_files.live_file(Path(ROOT), "run-axpy-confirmation.sh")
RESULTS = os.path.dirname(LAUNCHER)
