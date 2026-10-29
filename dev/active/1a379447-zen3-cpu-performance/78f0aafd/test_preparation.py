"""Untimed behavioral checks of profile preparation and evidence joins."""
import importlib
import json
import os
from pathlib import Path
import subprocess
import sys
import unittest

sys.dont_write_bytecode = True
from locate import HERE, ROOT, live, load
accounting = importlib.import_module('make-accounting')


class Preparation(unittest.TestCase):
    def test_production_join_requires_the_profiled_content(self):
        build = {'production_inputs': {'kernel.rs': 'same', 'envelope.json': 'frozen'}}
        receipt = {'source': {'producing': {'behavior_sha256': build['production_inputs']}}}
        self.assertTrue(accounting.same_production_inputs(receipt, build))
        self.assertFalse(accounting.same_production_inputs(receipt, {'production_inputs': {}}))
        receipt['source']['producing']['behavior_sha256'] = {'kernel.rs': 'different', 'envelope.json': 'frozen'}
        self.assertFalse(accounting.same_production_inputs(receipt, build))

    def test_historical_profiles_join_their_own_builds(self):
        for cell, receipt, _, planned, _, _ in accounting.arms.gaps():
            if cell['domain'] != 'LDPC decode':
                continue
            joined = accounting.available_profile(receipt, planned)
            self.assertIsNotNone(joined)
            build = accounting.read(ROOT/joined['build_record']['path'])
            actual = build.get('executables', build.get('generations', {}).get('after', {}).get('executables', {}))
            actual = actual or build.get('kernel', {}).get('executables', {})
            self.assertTrue(accounting.has_digest(actual, joined['measured_arm_sha256']))

    def test_restored_series_profile_the_measured_bytes(self):
        attempts = accounting.read(HERE/'restoration.json')['attempts']
        restored = {row['expected_sha256'] for row in attempts if row['verdict'] == 'restored'}
        self.assertTrue(all(row['obtained_sha256'] == row['expected_sha256'] for row in attempts
                            if row['verdict'] == 'restored'))
        for cell, receipt, _, planned, _, _ in accounting.arms.gaps():
            if not any(cell['family'] in families for families in accounting.arms.SUBJECTS.values()):
                continue
            sha = receipt['arms'][planned['baseline_arm']]['executable_sha256']
            expected = 'historical' if sha in restored else 'unrestored'
            self.assertEqual(accounting.restoration(sha)['code'], expected)
        self.assertEqual(accounting.restoration('0'*64)['code'], 'unrestored')

    def test_new_executable_has_source_span_rules(self):
        summary = load('3be770d5/survey/summarize-profile.py', 'summary_under_test')
        record = accounting.read(HERE/'preparation/build-identity.json')
        sha = record['executables']['ldpc-batch-profile']['sha256']
        rules = summary.RULES_BY_EXECUTABLE[sha]
        self.assertEqual(summary.categorize([('', summary.BATCH, 410)], rules), 'lane-transpose-in')
        self.assertEqual(summary.categorize([('', summary.BATCH, 330)], rules), 'batch-wave-dispatch')

    def test_window_launchers_refuse_without_the_window(self):
        environment = {k: v for k, v in os.environ.items() if k != 'GF2_BENCH_WINDOW'}
        for name in ('decoder', 'bch', *accounting.arms.SUBJECTS):
            result = subprocess.run(['bash', str(HERE/f'{name}-window.sh')],
                                    env=environment, capture_output=True, text=True, cwd=ROOT)
            self.assertEqual(result.returncode, 2)
            self.assertIn('GF2_BENCH_WINDOW=1', result.stderr)


if __name__ == '__main__':
    unittest.main()
