"""Runtime locations of this family's generators (jit:4c1e441f).

Every location is resolved under the repository root git reports: this family's
own files from this file's directory, shared scripts and the launcher by file
name, harness and tool crates by package name. Importing this module puts the
shared campaign scripts on `sys.path`.
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


def package(name):
    """Root-relative directory of the one live Cargo package called `name`."""
    return repository_files.package_directory(Path(ROOT), name)


def live_file(name):
    """Root-relative path of the one live file called `name`."""
    found = repository_files.tracked_files(Path(ROOT), name)
    if len(found) != 1:
        raise SystemExit(f"{len(found)} live files are called {name}; exactly one must be")
    return found[0]


SURVEY = os.path.relpath(HERE, ROOT)
ISSUE = os.path.dirname(SURVEY)
LAUNCHER = live_file("run-dense-product-confirmation.sh")
RESULTS = os.path.dirname(LAUNCHER)
