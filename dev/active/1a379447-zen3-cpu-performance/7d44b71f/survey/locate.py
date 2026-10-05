"""Shared locators for this task's generators (jit:7d44b71f).

Importing this module puts the shared scripts on `sys.path`; they resolve
files under the root git reports.
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
import asm_listing  # noqa: E402
import content_anchor  # noqa: E402
import repository_files  # noqa: E402
import rust_code_text  # noqa: E402

ISSUE = "7d44b71f"
PACKAGE_NAME = "gf2-kernels-simd"
PACKAGE = repository_files.package_directory(ROOT, PACKAGE_NAME)

# The task anchor by content: per-path digests, and byte snapshots of the
# paths this task changes. An `inputs` directory is outside every live lookup.
ANCHOR = content_anchor.Anchor(ROOT, HERE / "anchor-baseline.json", HERE / "inputs" / "anchor")


def tracked(suffix):
    """Sorted root-relative package paths ending in `suffix`."""
    listing = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "--", PACKAGE],
        capture_output=True, check=True, text=True,
    ).stdout.split()
    return sorted(path for path in listing if path.endswith(suffix))
