#!/usr/bin/env python3
"""Project material gaps and per-profile mechanism accounting from committed evidence.

--check compares the projection without writing. A related profile whose build
cannot be joined by content identity remains context, never coverage.
"""
from functools import cache
import argparse
import collections
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

sys.dont_write_bytecode = True
from locate import HERE, ROOT, live, load, repository_files
from importlib import import_module
arms = import_module('arm-profiles')
intervals = load('3be770d5/survey/intervals.py', 'profile_intervals')
outcomes = load('survey/make-collection-outcome.py', 'collection_outcomes')

# Profiles of measured decoder builds. The build records pin both the profiled
# executable and the campaign arm; Git revisions play no role in this join.
DECODER_PROFILES = [
    ('ed3d490e/preparation/build-identity.json', 'f434ddd3/profile/profile-summary.json'),
    ('3be770d5/preparation/build-identity.json', '3be770d5/v4-r1-steady-profile/profile-summary.json'),
    ('07ca8585/preparation/build-identity.json', '07ca8585/v4-r1-resampled-profile/profile-summary.json'),
    ('07ca8585/preparation/kernel-build-identity.json', '07ca8585/v4-r1-resampled-profile/profile-summary.json'),
]
MECHANISMS = ('allocation', 'instructions', 'dependencies', 'compiler_spills',
              'packing', 'cache_traffic', 'bandwidth', 'orchestration', 'hardware_limit')
CONTEXT = {
    'byte-field': '19513245/r1-consumer-profile/profile-summary.md',
    'polynomial': '53c5a8c0/profile/profile.md',
    'logical': '2037941f/logical-profile/profile-summary.md',
    'dense': '2037941f/dense-baseline-profile-r2/profile-summary.md',
    'shift': 'c04dd4ac-zen3-shifts-and-permutations/shift-profile.md',
    'interleave': 'c04dd4ac/dvb-interleave-profile/v4-r3-dynamic-profile/profile-summary.md',
}


@cache
def read(path):
    return json.loads(Path(path).read_text())


def relative(path):
    return Path(path).relative_to(ROOT).as_posix()


def cited(path):
    path = Path(path)
    return {'path': relative(path), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}


def has_digest(node, digest):
    if isinstance(node, dict):
        return any(has_digest(value, digest) for value in node.values())
    if isinstance(node, list):
        return any(has_digest(value, digest) for value in node)
    return node == digest


def available_profile(receipt, planned):
    sha = receipt['arms'][planned['baseline_arm']]['executable_sha256']
    for record, profile in DECODER_PROFILES:
        build_path, profile_path = live(record), live(profile)
        build, summary = read(build_path), read(profile_path)
        built = build.get('executables', build.get('generations', {}).get('after', {}).get('executables', {}))
        built = built or build.get('kernel', {}).get('executables', {})
        if not has_digest(built, sha):
            continue
        profiled_sha = summary['resolution'].get('executable_sha256')
        if profiled_sha is None:
            log = profile_path.parent / 'repetitions.log'
            found = re.search(r'ldpc-profile=([0-9a-f]{64})', log.read_text())
            profiled_sha = found[1] if found else None
        # The update generation's separate kernel record joins the same profile
        # to its whole-decoding build by its pinned whole_decoding_identity.
        if profiled_sha and not has_digest(build.get('executables', build.get('profile_resample', {}).get('executables', {})), profiled_sha):
            kernel = read(live('07ca8585/preparation/kernel-build-identity.json'))
            pointer = kernel['whole_decoding_identity']
            if not (has_digest(kernel, profiled_sha) and
                    pointer['sha256'] == cited(build_path)['sha256']):
                continue
        return {'status': 'profile-cited', 'profile': cited(profile_path), 'code': 'historical',
                'basis': 'the build record pins the measured arm and profile executable',
                'build_record': cited(build_path), 'measured_arm_sha256': sha,
                'profile_executable_sha256': profiled_sha,
                'scope': 'code coverage; profile batch/core/iteration settings are its own rows'}
    return None


def drift_classes():
    record = read(live('1362381c/production-drift.json'))
    return {receipt['receipt']: row['class'] for row in record['rows']
            for receipt in row['receipts']}


def same_production_inputs(receipt, build):
    producing = receipt.get('source', {}).get('producing', {})
    measured = {**producing.get('behavior_sha256', {}), **producing.get('build_inputs_sha256', {})}
    profiled = build.get('production_inputs', {})
    return bool(profiled) and all(measured.get(path) == sha for path, sha in profiled.items())


def prepared_profile(subject, cell, receipt, planned):
    directory = HERE/f'{subject}-profile'
    expanded = directory/'cases.expanded.tsv'
    log = directory/'repetitions.log'
    if not expanded.is_file() or not log.is_file() or 'series done ' not in log.read_text():
        return None
    summary_path = directory/'profile-summary.json'
    summary = None
    if subject == 'fi3-arm':
        if not summary_path.is_file():
            return None
        summary = read(summary_path)
        if summary['cases'] != cited(expanded):
            return None
    sha = receipt['arms'][planned['baseline_arm']]['executable_sha256']
    for line in expanded.read_text().splitlines():
        row = json.loads(line)
        if row['cell'] == cell['cell'] and row['sha256'] == sha and row['request']['case'] == planned['case']:
            if summary is not None:
                measured = next(row for row in receipt['cells'] if row['cell_id'] == cell['cell'])
                if (row['request']['cpus'] != measured['resolved_cpus'] or
                        row['request']['workers_declared'] != len(measured['resolved_cpus']) or
                        not any(profile['cell'] == row['label'] and profile['status'] == 'profiled'
                                for profile in summary['profiles'])):
                    continue
            return {'status': 'profile-cited', 'profile': cited(summary_path if summary else expanded), 'code': 'historical',
                    'basis': 'same measured executable digest and case'+('; recorded worker CPUs agree' if summary else ''),
                    'measured_arm_sha256': sha}
    return None


def restoration(sha):
    """The rebuild attempt that restored the measured bytes the series profiles."""
    record = HERE/'restoration.json'
    attempts = read(record)['attempts'] if record.is_file() else []
    if any(row['expected_sha256'] == sha and row['verdict'] == 'restored' for row in attempts):
        return {'code': 'historical', 'restoration': cited(record)}
    return {'code': 'unrestored'}


def inventory():
    drift = drift_classes()
    rows = []
    for cell, receipt, plan, planned, declared, result in arms.gaps():
        interpretation = outcomes.INTERPRETATION[(declared['objective'], result['outcome'])]
        profile = available_profile(receipt, planned) if cell['domain'] == 'LDPC decode' else None
        if profile is None and cell['family'] == '1362381c-ldpc-aff3ct-integration':
            exact = prepared_profile('fi3-arm', cell, receipt, planned)
            integrated = HERE/'decoder-profile/profile-summary.json'
            profile = {'status': 'profile-cited' if integrated.is_file() else 'awaiting-window',
                       'code': 'integrated',
                       'launcher': relative(HERE/'fi3-arm-window.sh'),
                       'basis': 'integrated production route; awaiting measured producing-input comparison'}
            if integrated.is_file():
                build = read(HERE/'preparation/build-identity.json')
                same_inputs = same_production_inputs(receipt, build)
                measured_flags = receipt['arms'][planned['baseline_arm']].get('rustflags')
                profiled_flags = build['build_commands'][0]['environment'].get('RUSTFLAGS')
                same_inputs = same_inputs and measured_flags == profiled_flags
                profile['profile'] = cited(integrated)
                profile['status'] = 'profile-cited' if same_inputs else 'identity-unresolved'
                profile['basis'] = 'compiled production inputs equal by digest and target flags agree' if same_inputs else 'producing inputs or target flags differ or are unpinned'
            if exact:
                profile = exact
        if profile is None:
            subject = next((key for key, families in arms.SUBJECTS.items()
                            if cell['family'] in families), None)
            if cell['family'] == '1362381c-bch-integration':
                subject = 'bch'
            profile = {'status': 'awaiting-window' if subject else 'unresolved',
                       'launcher': relative(HERE/'subject-window.sh')+' '+subject if subject else None}
            existing = prepared_profile(subject, cell, receipt, planned) if subject else None
            if existing:
                profile = existing
            if subject in arms.SUBJECTS:
                profile.update(restoration(receipt['arms'][planned['baseline_arm']]['executable_sha256']))
            if subject in CONTEXT and not existing:
                profile['related_context'] = cited(live(CONTEXT[subject]))
                profile['basis'] = 'related profile; equality with the producing inputs is not established'
        rows.append({**cell, 'outcome': result['outcome'], 'interpretation': interpretation,
                     'drift_class': drift.get(cell['receipt'], 'unresolvable'),
                     'coverage': profile})
    return rows


def interval(values):
    if len(values) < 6:
        return {'status': 'unmeasured', 'completed_sessions': len(values)}
    median, lower, upper, coverage = intervals.median_interval(values)
    return dict(median=median, lower=lower, upper=upper, coverage=coverage, n=len(values))


def empty_mechanisms():
    return {key: {'status': 'unmeasured'} for key in MECHANISMS}


def decoder_rows(path):
    summary = read(path)
    rows = []
    for label, cell in summary['stats'].items():
        mechanisms = empty_mechanisms()
        metrics = cell['metrics']
        mechanisms['instructions'] = {'status': 'measured', 'source': cited(path),
                                      'metrics': {k: v for k, v in metrics.items()
                                                  if k in ('ipc', 'instructions_per_frame', 'cycles_per_frame')}}
        mechanisms['dependencies']['reason'] = 'IPC does not identify dependency latency'
        mechanisms['cache_traffic'] = {'status': 'measured', 'source': cited(path),
                                       'metrics': {k: v for k, v in metrics.items()
                                                   if k in ('l1d_miss_fraction', 'cache_miss_fraction')}}
        ratios = [row for row in summary['ratios'] if row['numerator'] == label or
                  row['denominator'] == label]
        if ratios:
            mechanisms['orchestration'] = {'status': 'measured per-worker ratio; route and attribution unmeasured',
                                           'source': cited(path), 'ratios': ratios,
                                           'sessions': summary['sessions']}
        record = summary['records'].get(label)
        if record:
            packing = [row for row in record['categories'] if 'transpose' in row['category'] or
                       'conversion' in row['category']]
            if packing:
                mechanisms['packing'] = {'status': 'sampled', 'source': cited(path),
                                          'samples': record['total_samples'], 'categories': packing}
        census = summary.get('census')
        if census and cell['arm'] == 'gf2':
            mechanisms['allocation'] = {'status': 'measured census; see census workload scope',
                                        'source': cited(path), 'census': census.get(cell['code'])}
        rows.append({'profile': relative(path), 'cell': label, 'status': 'profiled',
                     'core_arm': cell['core_arm'], 'batch': cell['batch'],
                     'mechanisms': mechanisms})
    return rows


def csv_counters(path):
    counters = {}
    for line in path.read_text().splitlines():
        fields = line.split(',')
        if len(fields) >= 3:
            try:
                counters[fields[2].removesuffix(':u')] = float(fields[0])
            except ValueError:
                pass
    return counters


def subject_rows(subject):
    directory = HERE/f'{subject}-profile'
    log = directory/'repetitions.log'
    expanded = directory/'cases.expanded.tsv'
    cases = [json.loads(line) for line in expanded.read_text().splitlines()] if expanded.is_file() else list(arms.cases(subject))
    done = re.findall(r'^(rep-\d+) done ', log.read_text(), re.M) if log.is_file() else []
    rows = []
    for case in cases:
        mechanisms = empty_mechanisms()
        samples = collections.defaultdict(list)
        for rep in done:
            for group in ('core', 'memory'):
                path = directory/rep/(case['label']+'.'+group+'.csv')
                if not path.is_file():
                    continue
                counters = csv_counters(path)
                for numerator, denominator, metric in [('instructions','cycles','ipc'),
                    ('L1-dcache-load-misses','L1-dcache-loads','l1d_miss_fraction'),
                    ('cache-misses','cache-references','cache_miss_fraction')]:
                    if numerator in counters and counters.get(denominator, 0):
                        samples[metric].append(counters[numerator]/counters[denominator])
        for mechanism, names in [('instructions', ['ipc']),
                                  ('cache_traffic', ['l1d_miss_fraction', 'cache_miss_fraction'])]:
            if samples:
                mechanisms[mechanism] = {'status': 'whole-process counters',
                    'metrics': {name: interval(samples[name]) for name in names},
                    'scope': case['scope'], 'source': relative(directory)}
        rows.append({'profile': relative(directory), 'cell': case['label'],
                     'status': 'profiled' if len(done) == 9 else 'awaiting-window',
                     'mechanisms': mechanisms})
    return rows


def stages(inventory_rows):
    rows = []
    for row in inventory_rows:
        receipt = read(ROOT/row['receipt'])
        cell = next(cell for cell in receipt['cells'] if cell['cell_id'] == row['cell'])
        for role in ('baseline', 'candidate'):
            conversions = [pair[role].get('conversion') for pair in cell['pairs']]
            values = {}
            for name in ('pack_ns', 'unpack_ns', 'batch_fill_ns', 'dispatch_ns'):
                measured = [r[name] for r in conversions if r and name in r]
                if measured:
                    values[name] = {'scope': 'descriptive per-execution adapter stage probe',
                                    'n': len(measured), 'min': min(measured), 'max': max(measured)}
            rows.append({'family': row['family'], 'cell': row['cell'], 'arm_role': role,
                         'source': cited(ROOT/row['receipt']), 'stage_probes': values or 'unmeasured'})
    return rows


def assembly():
    tool = live('6e87c436/survey/make-asm-comparison.py')
    rows = []
    for suffix in ('x86/asm/ldpc_lanes.asm.txt', 'x86/asm/gf2m_gemm.asm.txt', 'x86/asm/clmul.asm.txt'):
        listing = live(suffix)
        result = subprocess.check_output([sys.executable, '-B', str(tool), '--stack-traffic',
                                          str(listing.relative_to(ROOT))], text=True)
        rows.append(json.loads(result) | {'source': cited(listing),
                    'applicability': 'assembly context; identity with each profiled executable is unmeasured'})
    return rows


def build():
    gaps = inventory()
    profiles = []
    for _, suffix in dict.fromkeys(DECODER_PROFILES):
        path = live(suffix)
        if relative(path) not in {row['profile'] for row in profiles}:
            profiles.extend(decoder_rows(path))
    decoder = HERE/'decoder-profile/profile-summary.json'
    if decoder.is_file():
        profiles.extend(decoder_rows(decoder))
    else:
        for line in (HERE/'decoder-cases.tsv').read_text().splitlines():
            if line.startswith('stat\t'):
                _, label, _, _, core, batch, _ = line.split('\t')
                profiles.append({'profile': relative(decoder), 'cell': label, 'core_arm': core,
                                 'batch': int(batch), 'status': 'awaiting-window',
                                 'mechanisms': empty_mechanisms()})
    for subject in (*arms.SUBJECTS, 'bch'):
        profiles.extend(subject_rows(subject))
    workload = read(live('1362381c/workload-set.json'))
    registry = read(live('1362381c/survey/family-registry.json'))
    integrated_families = {'FI-3', 'FI-4'}
    integrated_by_pattern = {row['pattern']: row['integrated_family']
                             for row in registry['families']
                             if row.get('integrated_family') in integrated_families}
    selected_integrated = [row | {'integrated_family': integrated_by_pattern[row['registry_pattern']]}
                           for row in workload['families']
                           if row['registry_pattern'] in integrated_by_pattern]
    pending_integrated = [row for row in workload['pending']
                          if row.get('integrated_family') in integrated_families]
    return {'decoder_build': cited(HERE/'preparation/build-identity.json'),
            'source': cited(live('1362381c/workload-set.json')),
            'drift': cited(live('1362381c/production-drift.json')),
            'material_gap_rule': 'comparator-gap pass with the external arm as candidate; canonical INTERPRETATION',
            'material_gaps': gaps,
            'integrated_families': selected_integrated,
            'pending_integrated_families': pending_integrated,
            'profiles': profiles, 'stage_probes': stages(gaps), 'assembly': assembly(),
            'allocation_witness': cited(HERE/'allocation-witness.json'),
            'context_profiles': {key: cited(live(suffix)) for key, suffix in CONTEXT.items()},
            'hardware_limit': 'unmeasured: no measured host bandwidth or instruction/dependency bound is joined; no approach claimed'}


def mechanism_text(mechanism):
    text = mechanism['status']
    metrics = mechanism.get('metrics', {})
    for name, value in metrics.items():
        if 'median' in value:
            text += (f"; {name} {value['median']:.5g} [{value['lower']:.5g}, "
                     f"{value['upper']:.5g}], n={value['n']}, coverage={value['coverage']:.4g}")
    if mechanism.get('ratios'):
        text += '; ' + '; '.join(
            f"{row['id']} {row['estimate']:.4g} [{row['lower']:.4g}, {row['upper']:.4g}], "
            f"n={mechanism['sessions']} sessions" for row in mechanism['ratios'])
    if mechanism.get('categories'):
        text += '; ' + '; '.join(
            f"{row['category']} {row['share']:.4g} [{row['wilson_lower']:.4g}, "
            f"{row['wilson_upper']:.4g}], N={mechanism['samples']} samples"
            for row in mechanism['categories'])
    return text


def markdown(record):
    lines = ['# Integrated bottleneck accounting', '', '> **Diátaxis Type:** Reference', '',
             'Generated by `make-accounting.py`; `accounting.json` carries content identities, intervals,',
             'sample counts, stage probes and assembly context. Untimed smoke outputs are excluded.', '',
             'A consumed receipt retains its production-drift class. A matching historical build',
             'covers the code that receipt measures, regardless of its difference from the current tree.', '',
             'Profiled code is `historical` when the profile runs the measured executable bytes and',
             '`integrated` when it runs the current integrated build of the cell\'s route.', '',
             '## Primary material gaps', '', '| Family | Cell | Drift | Profiled code | Coverage |',
             '|---|---|---|---|---|']
    for row in record['material_gaps']:
        coverage = row['coverage']
        source = coverage.get('profile', {}).get('path', coverage.get('launcher') or 'unresolved')
        prepared = ('; prepared launcher: `'+coverage['launcher']+'`'
                    if coverage['status'] == 'identity-unresolved' and coverage.get('launcher') else '')
        lines.append(f"| {row['family']} | {row['cell']} | {row['drift_class']} | "
                     f"{coverage.get('code', 'unresolved')} | {coverage['status']}: `{source}`{prepared} |")
    lines += ['', '## Per-profile mechanism accounting', '',
              'IPC and cache counter ratios are observations, not hardware limits. Static stack',
              'references include ordinary stack locals; compiler spill attribution and dynamic cost',
              'remain unmeasured. Assembly context is in `accounting.json`, through the shared',
              '`make-asm-comparison.py --stack-traffic` projection. The integrated allocation',
              'witness record applies to its named test workloads; it does not establish per-cell counts.', '',
              '| Profile / cell | State | Allocation | Instructions | Dependencies | Compiler spills | Packing | Cache traffic | Bandwidth | Orchestration | Hardware limit |',
              '|---|---|---|---|---|---|---|---|---|---|---|']
    for row in record['profiles']:
        values = [mechanism_text(row['mechanisms'][key]) for key in MECHANISMS]
        lines.append('| '+ ' | '.join([f"`{row['profile']}` / `{row['cell']}`", row['status'], *values])+' |')
    lines += ['', '## Adapter stage probes', '',
              '| Family / cell / arm | Descriptive stage ranges in ns |', '|---|---|']
    for stage in record['stage_probes']:
        values = stage['stage_probes']
        text = '; '.join(f"{name} [{value['min']}, {value['max']}], n={value['n']} executions"
                         for name, value in values.items()) if isinstance(values, dict) else values
        lines.append(f"| {stage['family']} / {stage['cell']} / {stage['arm_role']} | {text} |")
    lines += ['', '## Assembly context', '',
              '| Listing / symbol | Static stack-address reference lines |', '|---|---:|']
    for listing in record['assembly']:
        for symbol in listing['symbols']:
            lines.append(f"| `{listing['artefact']}` / `{symbol['symbol']}` | {symbol['stack_address_lines']} |")
    lines += ['', 'Adapter stage probes are projected separately per receipt cell and arm in',
              '`accounting.json` as descriptive ranges with their execution count. They include',
              'whole-consumer conversion costs and do not isolate dependency latency.', '',
              '## Integrated family selections', '',
              '| Family | Registry pattern | Receipt | Receipt SHA-256 |', '|---|---|---|---|']
    for family in record['integrated_families']:
        lines.append(f"| {family['integrated_family']} | `{family['registry_pattern']}` | "
                     f"`{family['receipt']}` | `{family['receipt_sha256']}` |")
    if record['pending_integrated_families']:
        lines += ['', 'Pending integrated families are listed in `accounting.json`.']
    else:
        lines += ['', 'The canonical workload set has selected receipts for FI-3 and FI-4.']
    lines += ['', record['hardware_limit']+'.', '']
    return '\n'.join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--summarize-subject', choices=(*arms.SUBJECTS, 'bch'))
    args = parser.parse_args()
    if args.summarize_subject:
        subject = args.summarize_subject
        directory = HERE/f'{subject}-profile'
        summary = {'cases': cited(directory/'cases.expanded.tsv'),
                   'profiles': subject_rows(subject)}
        (directory/'profile-summary.json').write_text(json.dumps(summary, indent=2)+'\n')
        return
    record = build()
    outputs = {HERE/'accounting.json': json.dumps(record, indent=2)+'\n',
               HERE/'accounting.md': markdown(record)}
    for path, text in outputs.items():
        if args.check:
            if not path.is_file() or path.read_text() != text:
                raise SystemExit('projection differs: '+relative(path))
        else:
            path.write_text(text)
    counts = collections.Counter(row['coverage']['status'] for row in record['material_gaps'])
    print(json.dumps({'material_gaps': len(record['material_gaps']), 'coverage': counts,
                      'profile_rows': len(record['profiles'])}, sort_keys=True))


if __name__ == '__main__':
    main()
