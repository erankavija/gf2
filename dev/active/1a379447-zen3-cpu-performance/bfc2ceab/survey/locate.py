"""Shared locators for this issue's generators (jit:bfc2ceab): importing this
module puts the shared scripts on `sys.path`, and they resolve files under the
root git reports.
"""

import functools
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


def package_files(name, glob):
    """The live files of package `name` whose file name matches `glob`, root-relative."""
    prefix = package(name) + "/"
    return [path for path in repository_files.tracked_files(ROOT, glob)
            if path.startswith(prefix)]


def family_stages(confirmation_addendum):
    """Receipt directory of each committed stage of the family, by label.

    The pilot is the receipt the confirmation addendum pins as its resolution
    evidence, and the confirmation the receipt beside it that pins that
    addendum; each acceptance summary must evaluate its receipt's bytes.
    """
    pinned = hashlib.sha256(confirmation_addendum.read_bytes()).hexdigest()
    evidence = json.loads(confirmation_addendum.read_text())["effect"]["resolution_evidence"]
    pilot = ROOT / evidence["receipt"]
    if hashlib.sha256(pilot.read_bytes()).hexdigest() != evidence["sha256"]:
        raise SystemExit(f"{evidence['receipt']} is not the receipt the confirmation pins")
    stages = {"pilot": pilot.parent}
    confirmations = [
        receipt.parent for receipt in sorted(pilot.parent.parent.glob("*/receipt.json"))
        if json.loads(receipt.read_text())["addendum"]["sha256"] == pinned
    ]
    if len(confirmations) > 1:
        raise SystemExit(f"{len(confirmations)} receipts pin {confirmation_addendum.name}")
    if confirmations:
        stages["confirmation"] = confirmations[0]
    for label, directory in stages.items():
        receipt = hashlib.sha256((directory / "receipt.json").read_bytes()).hexdigest()
        summary = json.loads((directory / "acceptance-summary.json").read_text())
        if summary["receipt_sha256"] != receipt:
            raise SystemExit(f"the {label}'s acceptance summary evaluates another receipt")
    return stages
