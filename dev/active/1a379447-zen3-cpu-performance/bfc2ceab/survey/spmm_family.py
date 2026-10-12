"""Committed stages of the sparse product check-cost family (jit:bfc2ceab)."""

import hashlib
import json

import review
from locate import HERE, ROOT

FAMILY = HERE.parent


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def pilot_addendum():
    return json.loads((FAMILY / review.GRAIN_FAMILY_ADDENDUM).read_text())


def ledger():
    """The family ledger's path and its entries."""
    path = ROOT / pilot_addendum()["family_wise"]["ledger_path"]
    return path, [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def stages():
    """The family's stages in ledger order, as `(label, receipt directory, receipt)`.

    A stage is the committed receipt of a ledger entry's campaign. A label that
    repeats takes its ordinal, as the launcher's campaign names do. Each
    acceptance summary must evaluate its receipt's bytes.
    """
    path, entries = ledger()
    family = pilot_addendum()["family"]["id"]
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
