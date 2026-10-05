"""Shared locators for this task's generators (jit:6e87c436).

Importing this module puts the shared `repository_files` and the dense story's
`repo_artifacts` on `sys.path`; both resolve files under the root git reports.
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

sys.path.insert(0, str(ROOT / Path(repository_files.live_file(ROOT, "repo_artifacts.py")).parent))
import repo_artifacts  # noqa: E402

# The dense families' receipts, by the campaign each records.
CAMPAIGNS = [
    "v4-r1-2037941f-dense-isolated-fused-parity",
    "v4-r1-2037941f-dense-allocated-matvec",
    "v4-r1-2037941f-dense-matvec-vs-m4ri",
    "v4-r1-confirmation-2037941f-dense-matvec-vs-m4ri",
]

# The production packages the dense campaigns measure.
PACKAGES = ["gf2-core", "gf2-kernels-simd"]

# The task anchor by content: per-path digests, and byte snapshots of the
# paths this task changes. An `inputs` directory is outside every live lookup.
ANCHOR_BASELINE = HERE / "anchor-baseline.json"
ANCHOR_SNAPSHOT = HERE / "inputs" / "anchor"


def anchor_digests():
    """Root-relative path to SHA-256 for every package file at the task anchor."""
    return json.loads(ANCHOR_BASELINE.read_bytes())["sha256"]


def anchor_identity():
    """The baseline record's root-relative path and digest."""
    return {
        "baseline": str(ANCHOR_BASELINE.relative_to(ROOT)),
        "baseline_sha256": hashlib.sha256(ANCHOR_BASELINE.read_bytes()).hexdigest(),
    }


def anchor_bytes(path):
    """The snapshotted anchor bytes of `path`, checked against the baseline digest."""
    held = (ANCHOR_SNAPSHOT / path).read_bytes()
    if hashlib.sha256(held).hexdigest() != anchor_digests()[path]:
        raise SystemExit(f"the anchor snapshot of {path} does not hold its baseline digest")
    return held


def changed_since_anchor():
    """Sorted package paths whose working-tree bytes differ from the anchor's, or exist on one side only."""
    digests = anchor_digests()
    packages = json.loads(ANCHOR_BASELINE.read_bytes())["packages"]
    tracked = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "--", *packages],
        capture_output=True, check=True, text=True,
    ).stdout.split()
    changed = set(tracked) ^ set(digests)
    for path in set(tracked) & set(digests):
        if hashlib.sha256((ROOT / path).read_bytes()).hexdigest() != digests[path]:
            changed.add(path)
    return sorted(changed)
