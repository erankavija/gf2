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


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def family_ledger(pilot_addendum):
    """The ledger of the family `pilot_addendum` freezes: its path and its entries."""
    path = ROOT / json.loads(pilot_addendum.read_text())["family_wise"]["ledger_path"]
    return path, [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def family_stages(pilot_addendum):
    """The family's stages in ledger order, as `(label, receipt directory, receipt)`.

    A stage is the committed receipt of a ledger entry's campaign. A label that
    repeats takes its ordinal, as the launcher's campaign names do. Each
    acceptance summary must evaluate its receipt's bytes.
    """
    path, entries = family_ledger(pilot_addendum)
    family = json.loads(pilot_addendum.read_text())["family"]["id"]
    receipts = {}
    for file in sorted(path.parent.glob("*/receipt.json")):
        receipt = json.loads(file.read_text())
        if receipt["family_id"] == family:
            receipts[receipt["campaign_id"]] = (file.parent, receipt)
    found, seen = [], {}
    for entry in entries:
        if entry["campaign"] not in receipts:
            raise SystemExit(f"no committed receipt of ledger campaign {entry['campaign']}")
        directory, receipt = receipts[entry["campaign"]]
        summary = json.loads((directory / "acceptance-summary.json").read_text())
        if summary["receipt_sha256"] != sha256(directory / "receipt.json"):
            raise SystemExit(f"the acceptance summary of {directory.name} evaluates another receipt")
        seen[receipt["label"]] = ordinal = seen.get(receipt["label"], 0) + 1
        found.append((receipt["label"] + (str(ordinal) if ordinal > 1 else ""), directory, receipt))
    return found
