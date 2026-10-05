"""Shared locators for this task's generators (jit:fa1d8733).

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
import crate_functions  # noqa: E402
import repository_files  # noqa: E402
import rust_code_text  # noqa: E402

ISSUE = "fa1d8733"
PACKAGE_NAME = "gf2-kernels-simd"
PACKAGE = repository_files.package_directory(ROOT, PACKAGE_NAME)

# The two trees this task compares, each by content: per-path digests of the
# kernel package, and byte snapshots of the paths the working tree changes.
#
#   before  the sources holding the unroll bodies and their listing
#   after   the sources without them and the listing regenerated from those
BASELINES = {
    stage: content_anchor.Anchor(ROOT, HERE / f"{stage}-baseline.json", HERE / "inputs" / stage)
    for stage in ("before", "after")
}
STEP = "before to after"


def tracked(suffix):
    """Sorted root-relative package paths ending in `suffix`."""
    listing = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "--", PACKAGE],
        capture_output=True, check=True, text=True,
    ).stdout.split()
    return sorted(path for path in listing if path.endswith(suffix))
