"""Shared locators of the BCH integration family's generators (jit:1358d6ab).

Importing this module puts the shared scripts on `sys.path`; they resolve files
under the root git reports.
"""

import subprocess
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # a generator's `--check` writes nothing

HERE = Path(__file__).resolve().parent
ROOT = Path(
    subprocess.run(
        ["git", "-C", str(HERE), "rev-parse", "--show-toplevel"],
        capture_output=True, check=True, text=True,
    ).stdout.strip()
)
_listing = subprocess.run(
    ["git", "-C", str(ROOT), "ls-files", "--", ":(glob)**/repository_files.py"],
    capture_output=True, check=True, text=True,
).stdout.split()
_live = [path for path in _listing if "inputs" not in Path(path).parts[:-1]]
if len(_live) != 1:
    raise SystemExit(f"{len(_live)} live repository_files.py files; exactly one must exist")
SHARED = Path(_live[0]).parent.as_posix()
sys.path.insert(0, str(ROOT / SHARED))
import repository_files  # noqa: E402

ISSUE = HERE.parent.name
FAMILY_DIRECTORY = HERE.relative_to(ROOT).as_posix()


def package(name):
    """Root-relative directory of the live Cargo package `name`."""
    return repository_files.package_directory(ROOT, name)
