"""Shared locators for this issue's generators (jit:bfc2ceab).

Importing this module puts the shared scripts on `sys.path`; they resolve
files under the root git reports.
"""

import functools
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
import asm_listing  # noqa: E402
import content_anchor  # noqa: E402
import crate_functions  # noqa: E402
import repository_files  # noqa: E402
import rust_code_text  # noqa: E402

ISSUE = "bfc2ceab"
KERNEL_PACKAGE = "gf2-kernels-simd"


@functools.cache
def package(name):
    """Root-relative directory of the live Cargo package `name`."""
    return repository_files.package_directory(ROOT, name)


@functools.cache
def _tracked(pattern):
    listing = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "--", f":(glob){pattern}"],
        capture_output=True, check=True, text=True,
    ).stdout.split()
    return tuple(sorted(path for path in listing if not repository_files.is_snapshot_copy(path)))


def tracked(pattern):
    """Sorted root-relative tracked paths matching the glob `pattern`, outside receipt snapshots."""
    return list(_tracked(pattern))
