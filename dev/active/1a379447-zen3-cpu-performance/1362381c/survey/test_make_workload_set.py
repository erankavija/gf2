#!/usr/bin/env python3
"""Behavioral fixtures for make-workload-set.py; run this file with python3."""

import hashlib
import importlib.util
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True
HERE = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("workload_set", HERE / "make-workload-set.py")
generator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(generator)
ROOT = pathlib.Path(subprocess.check_output(
    ["git", "-C", str(HERE), "rev-parse", "--show-toplevel"], text=True).strip())
LIVE_REGISTRY = json.loads((HERE / "family-registry.json").read_text())
selector = generator.load(ROOT / LIVE_REGISTRY["receipt_selector"])


class WorkloadSetTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = pathlib.Path(self.temporary.name)
        self.registry = self.root / "family-registry.json"
        self.registry.write_text(json.dumps({
            "source": "design.md#family-registry",
            "external_arm_identity": {"field": "build", "equals": "external"},
            "families": [{"pattern": "family-*", "domain": "test", "issues": ["owner"]}],
        }))

    def campaign(self, campaign, family="family-test", label="confirmation", version=4,
                 sequence=1, verdict="accepted", roles=None, external=True):
        roles = roles or {"confirmed": "confirmatory"}
        directory = self.root / "dev" / "bench_results" / "owner" / campaign
        directory.mkdir(parents=True)
        cells = [{"cell_id": cell, "role": role,
                  "workload": {"identity": cell}} for cell, role in roles.items()]
        addendum = {"family": {"id": family, "issue": "owner"},
                    "protocol": {"version": version}, "cells": cells}
        (directory / "addendum.json").write_text(json.dumps(addendum))
        ledger = {"sequence": sequence, "family": family, "campaign": campaign}
        (directory / "ledger.jsonl").write_text(json.dumps(ledger) + "\n")
        plan = {"arms": {"local": {"build": "native"},
                         "peer": {"build": "external" if external else "native"}},
                "cells": [{"cell_id": cell, "baseline_arm": "local",
                           "candidate_arm": "peer" if cell != "local-only" else "local"}
                          for cell in roles]}
        (directory / "plan.json").write_text(json.dumps(plan))
        receipt = {"schema": selector.BENCHMARK_SCHEMA, "campaign_id": campaign,
                   "label": label, "family_id": family,
                   "addendum": {"path": str((directory / "addendum.json").relative_to(self.root))},
                   "trial_ledger": {"path": str((directory / "ledger.jsonl").relative_to(self.root))}}
        (directory / "receipt.json").write_text(json.dumps(receipt))
        digest = hashlib.sha256((directory / "receipt.json").read_bytes()).hexdigest()
        summary = {"verdict": verdict, "label": label, "receipt_sha256": digest,
                   "qualifies": False, "cells": cells}
        (directory / "acceptance-summary.json").write_text(json.dumps(summary))
        return directory

    def declaration(self):
        return generator.declaration(self.root, self.registry, selector)

    def test_roles_and_primary_follow_each_cells_arms(self):
        self.campaign("confirmation", roles={"confirmed": "confirmatory",
                      "held-out": "holdout", "local-only": "confirmatory",
                      "exploration": "exploratory"})
        rows = {row["cell"]: row for row in self.declaration()["cells"]}
        self.assertEqual(set(rows), {"confirmed", "held-out", "local-only"})
        self.assertEqual(rows["held-out"]["role"], "holdout")
        self.assertTrue(rows["confirmed"]["primary"])
        self.assertTrue(rows["held-out"]["primary"])
        self.assertFalse(rows["local-only"]["primary"])

    def test_roles_come_from_the_pinned_addendum(self):
        directory = self.campaign("confirmation", roles={"held-out": "holdout"})
        path = directory / "acceptance-summary.json"
        summary = json.loads(path.read_text())
        summary["cells"][0]["role"] = "confirmatory"
        path.write_text(json.dumps(summary))
        self.assertEqual(self.declaration()["cells"][0]["role"], "holdout")

    def test_external_pilot_is_exploratory_and_never_primary(self):
        self.campaign("pilot", label="pilot", roles={"pilot-cell": "exploratory"})
        self.campaign("rejected-confirmation", sequence=2, verdict="rejected")
        result = self.declaration()
        self.assertEqual([row["role"] for row in result["cells"]], ["exploratory"])
        self.assertFalse(result["cells"][0]["primary"])
        self.assertEqual(result["cells"][0]["external_arms"], ["peer"])
        self.assertTrue(result["families"][0]["rejected_confirmations"])

    def test_v4_preference_then_ledger_order_not_campaign_name(self):
        self.campaign("z-older-v4", sequence=1)
        self.campaign("a-latest-v4", sequence=2)
        self.campaign("zz-v3", sequence=3, version=3)
        self.campaign("zzz-rejected-v4", sequence=4, verdict="rejected")
        self.assertIn("a-latest-v4/receipt.json", self.declaration()["cells"][0]["receipt"])

    def test_fallback_confirmation_beats_newer_pilot(self):
        self.campaign("v3-confirmation", version=3)
        self.campaign("v4-pilot", label="pilot", sequence=2,
                      roles={"pilot-cell": "exploratory"})
        self.assertIn("v3-confirmation/receipt.json", self.declaration()["cells"][0]["receipt"])

    def test_latest_pilot_and_deterministic_ties(self):
        self.campaign("z-old-pilot", label="pilot", sequence=0)
        self.campaign("a-new-pilot", label="pilot", sequence=1)
        self.campaign("b-new-pilot", label="pilot", sequence=1)
        first = generator.render(self.declaration())
        self.assertIn("b-new-pilot/receipt.json", self.declaration()["cells"][0]["receipt"])
        self.assertEqual(first, generator.render(self.declaration()))

    def test_no_receipts_are_pending_without_cell_rows(self):
        self.campaign("smoke", label="smoke")
        result = self.declaration()
        self.assertEqual(result["cells"], [])
        self.assertEqual(result["pending"][0]["pattern"], "family-*")

    def test_legacy_receipt_uses_pinned_family_without_live_addendum(self):
        directory = self.campaign("legacy-pilot", label="pilot", version=1,
                                  roles={"legacy": "exploratory"})
        receipt = json.loads((directory / "receipt.json").read_text())
        del receipt["family_id"]
        del receipt["trial_ledger"]
        (directory / "receipt.json").write_text(json.dumps(receipt))
        summary = json.loads((directory / "acceptance-summary.json").read_text())
        summary["receipt_sha256"] = hashlib.sha256(
            (directory / "receipt.json").read_bytes()).hexdigest()
        (directory / "acceptance-summary.json").write_text(json.dumps(summary))
        self.assertEqual(self.declaration()["cells"][0]["family"], "family-test")

    def test_domain_projection_partitions_shared_family(self):
        registry = json.loads(self.registry.read_text())
        registry["families"][0]["domain_by_workload"] = [
            {"pattern": "bch-*", "domain": "BCH"}]
        self.registry.write_text(json.dumps(registry))
        self.campaign("mixed", roles={"bch-encode": "confirmatory", "transpose": "holdout"})
        rows = {row["cell"]: row for row in self.declaration()["cells"]}
        self.assertEqual(rows["bch-encode"]["domain"], "BCH")
        self.assertEqual(rows["transpose"]["domain"], "test")

    def test_integrated_pattern_excludes_named_families(self):
        registry = json.loads(self.registry.read_text())
        registry["families"] = [
            {"pattern": "family-core-integration", "domain": "core", "issues": ["owner"]},
            {"pattern": "family-*-integration", "domain": "other", "issues": ["owner"],
             "exclude": ["family-core-integration"]},
        ]
        self.registry.write_text(json.dumps(registry))
        self.campaign("core", family="family-core-integration")
        self.campaign("other", family="family-other-integration")
        rows = self.declaration()["cells"]
        self.assertEqual({row["domain"] for row in rows}, {"core", "other"})
        self.assertEqual(len(rows), 2)

    def test_pinned_ledger_is_used_instead_of_live_family_path(self):
        self.campaign("first", sequence=1)
        self.campaign("second", sequence=2)
        (self.root / "ledger.jsonl").write_text("not the pinned reservation chain\n")
        self.assertIn("second/receipt.json", self.declaration()["cells"][0]["receipt"])


if __name__ == "__main__":
    unittest.main()
