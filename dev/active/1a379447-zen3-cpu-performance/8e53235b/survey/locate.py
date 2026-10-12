"""Shared locators for this issue's generators (jit:8e53235b).

Importing this module puts the shared scripts on `sys.path`; they resolve
files under the root git reports.
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


SHARED = _shared_scripts().as_posix()
sys.path.insert(0, str(ROOT / SHARED))
import repository_files  # noqa: E402

ISSUE = HERE.parent.name
SURVEY = HERE.relative_to(ROOT).as_posix()
FAMILY_DIRECTORY = HERE.parent.relative_to(ROOT).as_posix()
LAUNCHER = repository_files.live_file(ROOT, "run-codeword-assembly.sh")
RESULTS = Path(LAUNCHER).parent.as_posix()


def package(name):
    """Root-relative directory of the live Cargo package `name`."""
    return repository_files.package_directory(ROOT, name)
