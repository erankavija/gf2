"""Re-validates committed tuning-campaign records with the current validator.

Each published campaign commits its session record; a declared campaign that
has not run commits only its declaration. Both must stay valid under the
validator of the checkout, whatever fields later campaigns add to its table.

A calibrating issue also commits `selector-outcomes.json`, which cites each
measurement outcome that bears on a selector by path, SHA-256 and outcome. A
citation must name the cited record's bytes and outcome, and the committed
tuning profiles must hold what the outcome says: after `no-change`, every
profile stating the selector holds the value of its conservative definition.

Run from any directory:
    python3 validate_tuning_extent_campaign_test.py
"""

from __future__ import annotations

import hashlib
import json
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.dont_write_bytecode = True

from repository_files import repository_root, tracked_files  # noqa: E402

VALIDATOR = Path(__file__).with_name("validate-tuning-extent-campaign.py")
ROOT = repository_root(Path(__file__).resolve())
OUTPUT = "GF2_TUNING_VALIDATION="
SELECTOR_OUTCOMES = "selector-outcomes.json"
CORE_SECTION = "gf2-core/selectors"


def validate(*arguments: str) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, str(VALIDATOR), *arguments],
                          capture_output=True, text=True)


def session_record(issue: str) -> Path:
    """The one committed session record of a campaign of `issue`."""
    found = {(ROOT / path).parent for path in tracked_files(ROOT, "core-list-grid.json")
             if (ROOT / path).parent.name.startswith(f"gf2-{issue}-")}
    if len(found) != 1:
        raise LookupError(f"{len(found)} committed session records belong to {issue}")
    return found.pop()


def stated(profile: dict, selector: str):
    """The value `profile` states for the core `selector`, or `None`."""
    value = profile.get("sections", {}).get(CORE_SECTION, {}).get("selectors", {})
    for part in selector.split("."):
        value = value.get(part) if isinstance(value, dict) else None
    return value


def committed_profiles() -> dict[str, dict]:
    """Every tracked profile envelope lying beside a conservative envelope."""
    profiles = {}
    for conservative in tracked_files(ROOT, "conservative.json"):
        listing = subprocess.run(
            ["git", "-C", str(ROOT), "ls-files", "-z", "--", str(Path(conservative).parent)],
            capture_output=True, check=True, text=True).stdout
        for path in filter(None, listing.split("\0")):
            if path.endswith(".json"):
                envelope = json.loads((ROOT / path).read_bytes())
                if isinstance(envelope, dict) and "profile_format_version" in envelope:
                    profiles[path] = envelope
    return profiles


def conservative_definition(ledger: dict, claim: str, source: str) -> int:
    """The integer the ledger's `claim` line defines, read from `source`."""
    entry = next(item for item in ledger["claims"] if item["id"] == claim)
    if entry["verbatim"] not in source.splitlines():
        raise LookupError(f"{entry['path']} no longer holds the line of claim {claim}")
    return int(re.search(r"= ([0-9_]+);", entry["verbatim"]).group(1).replace("_", ""))


def outcome_problems(citation: dict, record: bytes, definition: int,
                     profiles: dict[str, dict]) -> list[str]:
    """Why `citation` disagrees with the cited `record` or with `profiles`."""
    selector, problems = citation["selector"], []
    if hashlib.sha256(record).hexdigest() != citation["sha256"]:
        return [f"{citation['record']} does not have sha256 {citation['sha256']}"]
    outcome = json.loads(record)
    for field in ["selector", "issue", "outcome"]:
        if outcome[field] != citation[field]:
            problems.append(f"cited {field} {citation[field]!r} is {outcome[field]!r} in the record")
    values = {path: stated(profile, selector) for path, profile in profiles.items()}
    values = {path: value for path, value in values.items() if value is not None}
    conservative = {path: value for path, value in values.items()
                    if profiles[path]["profile_id"] == "conservative"}
    if not conservative:
        problems.append(f"no conservative profile states {selector}")
    problems += [f"{path} states {value}, the conservative definition is {definition}"
                 for path, value in conservative.items() if value != definition]
    measured = {path: value for path, value in values.items() if path not in conservative}
    if outcome["outcome"] == "no-change":
        problems += [f"{path} states {value} after a no-change outcome at {definition}"
                     for path, value in measured.items() if value != definition]
    elif outcome["threshold_words"] not in measured.values():
        problems.append(f"no measured profile states the outcome's {outcome['threshold_words']}")
    return problems


class SelectorOutcomes(unittest.TestCase):
    def test_committed_citations_agree_with_the_profiles(self):
        profiles = committed_profiles()
        records = tracked_files(ROOT, SELECTOR_OUTCOMES)
        self.assertTrue(records, "no selector outcome record is committed")
        for path in records:
            for citation in json.loads((ROOT / path).read_bytes())["outcomes"]:
                cited = citation["conservative_definition"]
                ledger = json.loads((ROOT / cited["ledger"]).read_bytes())
                claim = next(item for item in ledger["claims"] if item["id"] == cited["claim"])
                definition = conservative_definition(
                    ledger, cited["claim"], (ROOT / claim["path"]).read_text())
                self.assertEqual(outcome_problems(
                    citation, (ROOT / citation["record"]).read_bytes(), definition, profiles), [])

    def test_a_disagreement_is_reported(self):
        selector = "bit_matrix.matvec_simd_min_words"
        def profile(identity, value):
            return {"profile_id": identity, "sections": {CORE_SECTION: {"selectors": {
                "bit_matrix": {"matvec_simd_min_words": value}}}}}
        def record(outcome, threshold):
            return json.dumps({"selector": selector, "issue": "0000aaaa", "outcome": outcome,
                               "threshold_words": threshold}).encode()
        def citation(cited, outcome="no-change"):
            return {"selector": selector, "issue": "0000aaaa", "record": "outcome.json",
                    "sha256": hashlib.sha256(cited).hexdigest(), "outcome": outcome}
        unchanged, moved = record("no-change", None), record("threshold", 10)
        agreeing = {"conservative.json": profile("conservative", 8), "owner.json": profile("m", 8)}
        tuned = agreeing | {"owner.json": profile("m", 10)}
        self.assertEqual(outcome_problems(citation(unchanged), unchanged, 8, agreeing), [])
        self.assertEqual(outcome_problems(citation(moved, "threshold"), moved, 8, tuned), [])
        for cited, bytes_, definition, profiles in [
            (citation(unchanged), unchanged + b" ", 8, agreeing),       # other bytes
            (citation(moved), moved, 8, tuned),                         # other outcome
            (citation(unchanged), unchanged, 8, tuned),                 # a profile moved
            (citation(unchanged), unchanged, 9, agreeing),              # the definition moved
            (citation(moved, "threshold"), moved, 8, agreeing),         # the value not installed
            (citation(unchanged), unchanged, 8, {"owner.json": profile("m", 8)}),
        ]:
            self.assertNotEqual(outcome_problems(cited, bytes_, definition, profiles), [])
        self.assertRaises(LookupError, conservative_definition,
                          {"claims": [{"id": "c", "path": "x.rs", "verbatim": "const A: usize = 8;"}]},
                          "c", "const A: usize = 9;\n")


class CommittedRecords(unittest.TestCase):
    def accepted(self, *arguments: str) -> dict:
        result = validate(*arguments)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(result.stdout.startswith(OUTPUT), result.stdout)
        return json.loads(result.stdout.removeprefix(OUTPUT))

    def rejected(self, *arguments: str) -> None:
        result = validate(*arguments)
        self.assertEqual(result.returncode, 2, result.stdout)

    def test_a83583e0_session_record_validates(self):
        record = session_record("a83583e0")
        self.assertEqual(self.accepted("--session-record", str(record))["campaign_id"],
                         record.name.removesuffix("-session"))

    def test_dbd8787d_session_record_validates(self):
        record = session_record("dbd8787d")
        self.assertEqual(self.accepted("--session-record", str(record))["campaign_id"],
                         record.name.removesuffix("-session"))

    def test_each_record_selects_its_own_field_table(self):
        earlier, later = (self.accepted("--session-record", str(session_record(issue)))
                          for issue in ["a83583e0", "dbd8787d"])
        self.assertLess(earlier["retained_fields"], later["retained_fields"])
        self.assertLess(earlier["cells"], later["cells"])

    def test_63bad95d_declaration_covers_the_complete_table(self):
        declared = self.accepted("--declaration-check", "63bad95d")
        published = self.accepted("--session-record", str(session_record("dbd8787d")))
        self.assertGreater(declared["retained_fields"], published["retained_fields"])

    def test_a_narrower_declaration_needs_its_inventory(self):
        self.rejected("--declaration-check", "dbd8787d")

    def test_a_record_is_rejected_under_another_campaigns_declaration(self):
        source, other = session_record("a83583e0"), session_record("dbd8787d")
        with tempfile.TemporaryDirectory() as directory:
            forged = Path(directory) / other.name
            shutil.copytree(source, forged)
            self.rejected("--session-record", str(forged))


if __name__ == "__main__":
    unittest.main()
