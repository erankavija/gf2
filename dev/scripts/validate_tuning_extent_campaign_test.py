"""Re-validates committed tuning-campaign records with the current validator.

Each published campaign commits its session record; a declared campaign that
has not run commits only its declaration. Both must stay valid under the
validator of the checkout, whatever fields later campaigns add to its table.

Run from any directory:
    python3 validate_tuning_extent_campaign_test.py
"""

from __future__ import annotations

import json
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
