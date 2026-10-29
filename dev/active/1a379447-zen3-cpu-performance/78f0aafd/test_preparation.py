"""Untimed behavioral checks of profile preparation and evidence joins."""
import importlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

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
            if cell['domain'] != 'LDPC decode' or cell['family'] == '1362381c-ldpc-aff3ct-integration':
                continue
            joined = accounting.available_profile(receipt, planned)
            self.assertIsNotNone(joined)
            build = accounting.read(ROOT/joined['build_record']['path'])
            actual = build.get('executables', build.get('generations', {}).get('after', {}).get('executables', {}))
            actual = actual or build.get('kernel', {}).get('executables', {})
            self.assertTrue(accounting.has_digest(actual, joined['measured_arm_sha256']))

    def test_integrated_profile_does_not_claim_an_unproved_code_join(self):
        rows = [row for row in accounting.inventory()
                if row['family'] == '1362381c-ldpc-aff3ct-integration']
        self.assertTrue(rows)
        profile = accounting.cited(HERE/'decoder-profile/profile-summary.json')
        for row in rows:
            self.assertEqual(row['coverage']['code'], 'integrated')
            self.assertEqual(row['coverage']['profile'], profile)
            self.assertIn(row['coverage']['status'], ('profile-cited', 'identity-unresolved'))
            if row['coverage']['status'] == 'identity-unresolved':
                self.assertIn('unpinned', row['coverage']['basis'])
                self.assertTrue(row['coverage']['launcher'].endswith('/fi3-arm-window.sh'))

    def test_fi3_cases_use_the_receipts_workloads_and_cpus(self):
        cases = list(accounting.arms.cases('fi3-arm'))
        gaps = [row for row in accounting.arms.gaps()
                if row[0]['family'] == '1362381c-ldpc-aff3ct-integration']
        self.assertEqual({row['cell'] for row in cases}, {row[0]['cell'] for row in gaps})
        for cell, receipt, plan, planned, declared, _ in gaps:
            row = next(row for row in cases if row['cell'] == cell['cell'])
            measured = next(row for row in receipt['cells'] if row['cell_id'] == cell['cell'])
            self.assertEqual(row['sha256'], receipt['arms'][planned['baseline_arm']]['executable_sha256'])
            self.assertEqual(row['request']['case'], planned['case'])
            self.assertEqual(row['request']['cpus'], measured['resolved_cpus'])
            self.assertEqual(row['request']['workers_declared'], declared['workers']['declared'])
            self.assertEqual(row['environment'], plan['arms'][planned['baseline_arm']]['environment'])
            request = accounting.arms.wire_request(row)
            self.assertEqual(request['cpus'], measured['resolved_cpus'])

    def test_fi3_coverage_requires_completed_matching_summary(self):
        cases = list(accounting.arms.cases('fi3-arm'))
        gaps = [row for row in accounting.arms.gaps()
                if row[0]['family'] == '1362381c-ldpc-aff3ct-integration']
        with tempfile.TemporaryDirectory(dir=HERE) as temporary, patch.object(accounting, 'HERE', Path(temporary)):
            directory = Path(temporary)/'fi3-arm-profile'
            directory.mkdir()
            expanded = directory/'cases.expanded.tsv'
            expanded.write_text(''.join(json.dumps(row)+'\n' for row in cases))
            (directory/'repetitions.log').write_text('series done fixture sessions=9\n')
            def coverage(gap):
                cell, receipt, _, planned, _, _ = gap
                return accounting.prepared_profile('fi3-arm', cell, receipt, planned)
            self.assertTrue(all(coverage(gap) is None for gap in gaps))
            summary = {'cases': accounting.cited(expanded), 'profiles': [
                {'cell': row['label'], 'status': 'profiled'} for row in cases]}
            path = directory/'profile-summary.json'
            path.write_text(json.dumps(summary))
            accounting.read.cache_clear()
            for gap in gaps:
                joined = coverage(gap)
                self.assertEqual(joined['status'], 'profile-cited')
                self.assertEqual(joined['profile'], accounting.cited(path))
                self.assertEqual(joined['measured_arm_sha256'], cases[0]['sha256'])
            for mutate in ('digest', 'batch', 'workers', 'cpus'):
                altered = json.loads(json.dumps(cases))
                row = altered[0]
                if mutate == 'digest':
                    row['sha256'] = '0'*64
                elif mutate == 'batch':
                    row['request']['case']['batch_size'] += 1
                elif mutate == 'workers':
                    row['request']['workers_declared'] += 1
                else:
                    row['request']['cpus'] = []
                expanded.write_text(''.join(json.dumps(row)+'\n' for row in altered))
                summary['cases'] = accounting.cited(expanded)
                path.write_text(json.dumps(summary))
                accounting.read.cache_clear()
                gap = next(gap for gap in gaps if gap[0]['cell'] == altered[0]['cell'])
                self.assertIsNone(coverage(gap), mutate)

    def test_fi3_identity_accepts_expected_bytes_and_refuses_a_different_file(self):
        cases = list(accounting.arms.cases('fi3-arm'))
        with tempfile.TemporaryDirectory(dir=HERE) as temporary:
            binary = Path(temporary)/'measured'
            binary.write_bytes(b'measured executable fixture')
            for row in cases:
                row.update(executable=str(binary), sha256=accounting.arms.digest(binary))
            path = Path(temporary)/'cases.jsonl'
            path.write_text(''.join(json.dumps(row)+'\n' for row in cases))
            accounting.arms.identity(path)
            directory = Path(temporary)/'dry-session'
            accounting.arms.session(directory, path, dry_run=True)
            outputs = list(directory.glob('*.dry-run.json'))
            self.assertEqual(len(outputs), len(cases))
            for output in outputs:
                prepared = json.loads(output.read_text())
                request = prepared['request']
                self.assertEqual(prepared['placement'], ['taskset', '-c', ','.join(map(str, request['cpus']))])
                self.assertEqual(len(request['cpus']), request['workers_declared'])
            different = Path(temporary)/'different'
            different.write_bytes(b'different executable')
            cases[0]['executable'] = str(different)
            path.write_text(''.join(json.dumps(row)+'\n' for row in cases))
            with self.assertRaisesRegex(SystemExit, 'executable differs:'):
                accounting.arms.identity(path)

    def test_fi3_completed_profile_supplies_inventory_coverage(self):
        joined = {'status': 'profile-cited', 'code': 'historical',
                  'basis': 'same measured executable digest and case'}
        with patch.object(accounting, 'prepared_profile',
                          side_effect=lambda subject, *args: joined if subject == 'fi3-arm' else None):
            rows = [row for row in accounting.inventory()
                    if row['family'] == '1362381c-ldpc-aff3ct-integration']
        self.assertTrue(rows)
        self.assertTrue(all(row['coverage'] == joined for row in rows))

    def test_restored_series_profile_the_measured_bytes(self):
        attempts = accounting.read(HERE/'restoration.json')['attempts']
        restored = {row['expected_sha256'] for row in attempts if row['verdict'] == 'restored'}
        self.assertTrue(all(row['obtained_sha256'] == row['expected_sha256'] for row in attempts
                            if row['verdict'] == 'restored'))
        for cell, receipt, _, planned, _, _ in accounting.arms.gaps():
            if cell['family'] == '1362381c-ldpc-aff3ct-integration':
                continue
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
