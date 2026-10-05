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

# The baselines this task compares, each by content: per-path digests, and
# byte snapshots of the paths the working tree changes. An `inputs` directory
# is outside every live lookup.
#
#   anchor            the unedited sources with listings regenerated from them
#   before-1b034786   the tree holding this task's contracts, before the code
#                     change of jit:1b034786
#   after-1b034786    the tree that change leaves
#   contracts-complete  the tree holding the contracts of the two calls that
#                     change makes dischargeable
#
# The task's own change is `anchor` to `before-1b034786` and `after-1b034786`
# to `contracts-complete`; the step between belongs to jit:1b034786.
FIX = "1b034786"


def _baseline(stage):
    return content_anchor.Anchor(ROOT, HERE / f"{stage}-baseline.json", HERE / "inputs" / stage)


ANCHOR = _baseline("anchor")
BEFORE_FIX = _baseline(f"before-{FIX}")
AFTER_FIX = _baseline(f"after-{FIX}")
COMPLETE = _baseline("contracts-complete")
BASELINES = {
    "anchor": ANCHOR,
    f"before-{FIX}": BEFORE_FIX,
    f"after-{FIX}": AFTER_FIX,
    "contracts-complete": COMPLETE,
}
# The task's two steps, as (name, first tree, last tree).
STEPS = [
    (f"anchor to before-{FIX}", "anchor", f"before-{FIX}"),
    (f"after-{FIX} to contracts-complete", f"after-{FIX}", "contracts-complete"),
]


def digest_changes(before, after):
    """Sorted paths whose digests differ between two baselines, or exist in one only."""
    old, new = before.digests(), after.digests()
    return sorted(path for path in set(old) | set(new) if old.get(path) != new.get(path))


def tracked(suffix):
    """Sorted root-relative package paths ending in `suffix`."""
    listing = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "--", PACKAGE],
        capture_output=True, check=True, text=True,
    ).stdout.split()
    return sorted(path for path in listing if path.endswith(suffix))
