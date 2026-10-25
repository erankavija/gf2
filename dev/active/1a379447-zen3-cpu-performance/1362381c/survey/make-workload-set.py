#!/usr/bin/env python3
"""Project the integrated workload declaration from accepted receipts (jit:eb07aa88).

Usage: make-workload-set.py [--root REPOSITORY] [--registry JSON] [--output JSON]
       python3 test_make_workload_set.py
"""

import argparse
import collections
import fnmatch
import hashlib
import importlib.util
import json
import pathlib
import subprocess
import sys

sys.dont_write_bytecode = True

SELECTION_RULE = {
    "confirmation": "For each registry family, prefer accepted protocol-v4 confirmations; "
                    "if none exists, use accepted confirmations of any protocol version. "
                    "Accepted means acceptance-summary verdict accepted, regardless of qualifies.",
    "ordering": "Within that pool choose the greatest sequence of the campaign's reservation "
                "in its receipt-pinned family ledger prefix. A receipt without a ledger sorts "
                "before ledger-backed receipts. Ties use protocol version, campaign identity, "
                "then repository-relative receipt path, lexicographically greatest; no Git "
                "date, file timestamp or live ledger is used.",
    "cells": "Emit every confirmatory and holdout cell in the selected pinned addendum. "
             "If no accepted confirmation exists, select the latest accepted pilot by the "
             "same ordering and emit all its cells as exploratory. Smoke receipts are excluded. "
             "Registry patterns, minus their explicit exclusions, expand against the pinned "
             "addenda's declared family identities.",
    "primary": "A confirmatory or holdout cell is primary exactly when either of its two "
               "plan arms has the registry's external build identity. Exploratory cells are "
               "never primary; external_arms still records their external comparisons.",
    "pending": "A registry entry with no accepted confirmation or pilot emits no cells and "
               "is recorded under pending. Distinct declared family identities remain distinct.",
    "domains": "A registry entry supplies the domain, with matching workload-identity "
               "overrides partitioning families shared by several domains.",
}


def load(path):
    spec = importlib.util.spec_from_file_location(path.stem.replace("-", "_"), path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def read(path):
    return json.loads(path.read_bytes())


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def attempt_order(directory, receipt, addendum):
    """Use the campaign's own reservation, including zero-comparison pilot entries."""
    pin = receipt.get("trial_ledger")
    sequence = -1
    if pin:
        records = [json.loads(line) for line in
                   (directory / pin["snapshot"]).read_text().splitlines() if line.strip()]
        own = [record for record in records
               if record["family"] == addendum["family"]["id"]
               and record["campaign"] == receipt["campaign_id"]]
        if len(own) != 1:
            raise ValueError(f"{directory}: expected one campaign reservation, got {len(own)}")
        sequence = own[0]["sequence"]
    return sequence, addendum["protocol"]["version"], receipt["campaign_id"]


def choose(campaigns):
    confirmations = [item for item in campaigns if item["summary"]["label"] == "confirmation"]
    v4 = [item for item in confirmations if item["addendum"]["protocol"]["version"] == 4]
    pilots = [item for item in campaigns if item["summary"]["label"] == "pilot"]
    pool = v4 or confirmations or pilots
    return max(pool, key=lambda item: (*item["order"], item["path"])) if pool else None


def declaration(root, registry_path, selector):
    registry = read(registry_path)
    families = collections.defaultdict(list)
    rejected = collections.defaultdict(list)
    selected_paths, _ = selector.select(root)
    for path in selected_paths:
        directory = (root / path).parent
        receipt = read(root / path)
        addendum = read(directory / receipt["addendum"]["snapshot"])
        family = addendum["family"]["id"]
        entries = [entry for entry in registry["families"]
                   if fnmatch.fnmatchcase(family, entry["pattern"])
                   and not any(fnmatch.fnmatchcase(family, pattern)
                               for pattern in entry.get("exclude", []))]
        if not entries:
            continue
        if len(entries) != 1:
            raise ValueError(f"{family}: matches several registry entries")
        summary = read(directory / "acceptance-summary.json")
        if summary["verdict"] != "accepted":
            if summary["label"] == "confirmation":
                rejected[family].append(path)
            continue
        if summary["label"] not in ("confirmation", "pilot"):
            continue
        receipt_sha = digest(root / path)
        if summary["receipt_sha256"] != receipt_sha:
            raise ValueError(f"{path}: acceptance summary names a different receipt")
        families[family].append({"path": path, "receipt": receipt,
                                 "receipt_sha256": receipt_sha,
                                 "summary": summary, "addendum": addendum,
                                 "entry": entries[0],
                                 "order": attempt_order(directory, receipt, addendum)})

    rows, selections = [], []
    for family, campaigns in sorted(families.items()):
        chosen = choose(campaigns)
        entry = chosen["entry"]
        directory = (root / chosen["path"]).parent
        plan = read(directory / "plan.json")
        planned = {cell["cell_id"]: cell for cell in plan["cells"]}
        pilot = chosen["summary"]["label"] == "pilot"
        external = registry["external_arm_identity"]
        selections.append({
            "family": family, "registry_pattern": entry["pattern"],
            "issues": entry["issues"], "label": chosen["summary"]["label"],
            "protocol_version": chosen["addendum"]["protocol"]["version"],
            "ledger_sequence": chosen["order"][0] if chosen["order"][0] >= 0 else None,
            "receipt": chosen["path"], "receipt_sha256": chosen["receipt_sha256"],
            "addendum": str((directory / chosen["receipt"]["addendum"]["snapshot"])
                            .relative_to(root)),
            "rejected_confirmations": sorted(rejected[family]),
        })
        for cell in sorted(chosen["addendum"]["cells"], key=lambda cell: cell["cell_id"]):
            role = "exploratory" if pilot else cell["role"]
            if not pilot and role not in ("confirmatory", "holdout"):
                continue
            arms = planned[cell["cell_id"]]
            external_arms = sorted({arms[side] for side in ("baseline_arm", "candidate_arm")
                                    if plan["arms"][arms[side]][external["field"]]
                                    == external["equals"]})
            domain = entry["domain"]
            for override in entry.get("domain_by_workload", []):
                if fnmatch.fnmatchcase(cell["workload"]["identity"], override["pattern"]):
                    domain = override["domain"]
            rows.append({"domain": domain, "family": family, "cell": cell["cell_id"],
                         "role": role, "primary": not pilot and bool(external_arms),
                         "external_arms": external_arms,
                         "receipt": chosen["path"], "receipt_sha256": chosen["receipt_sha256"]})

    counts = {}
    for domain in sorted({row["domain"] for row in rows}):
        domain_rows = [row for row in rows if row["domain"] == domain]
        counts[domain] = {"cells": len(domain_rows),
                          "primary": sum(row["primary"] for row in domain_rows),
                          **{role: sum(row["role"] == role for row in domain_rows)
                             for role in ("confirmatory", "holdout", "exploratory")}}
    pending = [entry for entry in registry["families"] if not any(
        selection["registry_pattern"] == entry["pattern"] for selection in selections)]
    return {
        "registry": str(registry_path.relative_to(root)),
        "registry_sha256": digest(registry_path), "registry_source": registry["source"],
        "selection_rule": SELECTION_RULE, "counts_by_domain": counts,
        "families": selections, "pending": pending,
        "cells": sorted(rows, key=lambda row: (row["domain"], row["family"], row["cell"])),
    }


def render(value):
    return json.dumps(value, indent=2, ensure_ascii=False) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=pathlib.Path)
    parser.add_argument("--registry", type=pathlib.Path,
                        default=pathlib.Path(__file__).resolve().with_name("family-registry.json"))
    parser.add_argument("--output", type=pathlib.Path)
    args = parser.parse_args()
    root = (args.root or pathlib.Path(subprocess.check_output(
        ["git", "-C", str(pathlib.Path(__file__).resolve().parent),
         "rev-parse", "--show-toplevel"], text=True).strip())).resolve()
    registry_path = args.registry.resolve()
    registry = read(registry_path)
    selector = load(root / registry["receipt_selector"])
    output = args.output or root / registry["output"]
    output.write_text(render(declaration(root, registry_path, selector)), encoding="utf-8")
    print(output)


if __name__ == "__main__":
    main()
