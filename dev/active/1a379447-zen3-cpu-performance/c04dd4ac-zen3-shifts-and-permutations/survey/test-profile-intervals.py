#!/usr/bin/env python3
"""Behavioral checks for nine-session counter and hot-location attribution."""

import contextlib
import importlib.util
import io
import json
import pathlib
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).with_name("make-dvb-tables.py")
SPEC = importlib.util.spec_from_file_location("make_dvb_tables", SCRIPT)
TABLES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(TABLES)
CASE = "qam16-r12-normal-direct"


class ProfileIntervals(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = pathlib.Path(self.temporary.name)
        lines = []
        for index in range(1, 10):
            rep = self.root / f"rep-{index:02d}"
            (rep / "counters").mkdir(parents=True)
            (rep / "hot").mkdir()
            (rep / "cases.json").write_text("[]")
            lines.append(f"{rep.name} done test")
            stem = rep / "counters" / CASE
            stem.with_suffix(".status").write_text("0")
            stem.with_suffix(".json").write_text(json.dumps({"case": CASE, "calls": 1}))
            events = {
                "cycles:u": 10 * index,
                "instructions:u": 20 * index,
                "branches:u": 3 * index,
                "branch-misses:u": index,
                "cache-references:u": 2 * index,
                "cache-misses:u": index,
            }
            stem.with_suffix(".csv").write_text(
                "".join(f"{value},,{event}\n" for event, value in events.items())
            )
            hot = rep / "hot" / CASE
            hot.with_suffix(".status").write_text("0")
            (rep / "hot" / f"{CASE}.report.txt").write_text(
                f" {10 + index:.2f}%  {100 + index}  dvb-profile  [.] selected_symbol  -  -\n"
                " 1.00%  10  dvb-profile  [.] other_symbol  -  -\n"
            )
            (rep / "hot" / f"{CASE}.instructions.txt").write_text(
                f"Disassembly (100 samples, percent: local period)\n"
                f" {1 + index:.2f} :   37093: add    %rcx,%rax\n"
                " 0.50 :   37094: nop\n"
            )
        lines.append("series done test sessions=9")
        (self.root / "repetitions.log").write_text("\n".join(lines) + "\n")

    def render(self, function, *args):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            function(*args)
        return output.getvalue()

    def test_nine_repetitions_have_across_session_intervals(self):
        reps = TABLES.complete_perf_repetitions(self.root, [CASE])
        counters = self.render(
            TABLES.repeated_counters, reps, [CASE], {"qam16-r12-normal": 10},
            {CASE: "qam16-r12-normal"}
        )
        self.assertIn("| 5.000 | [2.000, 8.000] | 10.000", counters)
        self.assertIn("| 2.000 | [2.000, 2.000] |", counters)
        symbols = self.render(TABLES.repeated_symbols, reps, [CASE])
        self.assertIn("selected_symbol` | 15.00% | [12.00%, 18.00%]", symbols)
        instructions = self.render(TABLES.repeated_instructions, reps)
        self.assertIn("37093` | `add %rcx,%rax` | 9 | 6.00% | [3.00%, 9.00%]", instructions)
        self.assertNotIn("37094", instructions)

    def test_missing_perf_outcome_cannot_be_summarized(self):
        (self.root / "rep-09" / "hot" / f"{CASE}.status").unlink()
        with self.assertRaisesRegex(ValueError, "perf collection unavailable"):
            TABLES.complete_perf_repetitions(self.root, [CASE])

    def test_incomplete_counter_values_cannot_be_summarized(self):
        path = self.root / "rep-09" / "counters" / f"{CASE}.csv"
        path.write_text("1,,cycles:u\n")
        reps = TABLES.complete_perf_repetitions(self.root, [CASE])
        with self.assertRaisesRegex(ValueError, "incomplete counter values"):
            self.render(
                TABLES.repeated_counters, reps, [CASE], {"qam16-r12-normal": 10},
                {CASE: "qam16-r12-normal"}
            )


if __name__ == "__main__":
    unittest.main()
