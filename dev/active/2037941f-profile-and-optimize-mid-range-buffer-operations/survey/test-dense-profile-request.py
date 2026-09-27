#!/usr/bin/env python3
"""Check the profiler request against the arm's exact JSON wire contract."""

import json
from pathlib import Path
import subprocess
import unittest


CAMPAIGN = Path("target/e1f9a78f-arms/release/dense-campaign")
ADDENDUM = Path(
    "dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/"
    "campaigns/dense-isolated-fused-parity.json"
)


class ProfileRequestTest(unittest.TestCase):
    def test_emits_exact_compact_request(self):
        result = subprocess.run(
            [
                str(CAMPAIGN), "profile-request", "--family",
                "2037941f-dense-isolated-fused-parity", "--addendum",
                str(ADDENDUM), "--cell", "and-popcnt-8w-warm", "--arm",
                "and-popcnt-a", "--cpu", "0",
            ],
            check=True,
            capture_output=True,
        )
        request = json.loads(result.stdout)
        self.assertEqual(
            result.stdout,
            json.dumps(request, separators=(",", ":"), ensure_ascii=False).encode(),
        )


if __name__ == "__main__":
    unittest.main()
