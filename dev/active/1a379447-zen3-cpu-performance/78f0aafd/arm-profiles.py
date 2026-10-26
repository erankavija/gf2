#!/usr/bin/env python3
"""Expand frozen arm plans for the shared profile session loop; run no build."""
import hashlib
import json
import os
import re
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
from locate import HERE, ROOT, live, load

SUBJECTS = {
    'nr-encode': ['nr-rate-matched-encode-baselines-v1'],
    'byte-field': ['byte-field-matrix-product', 'byte-field-region-axpy'],
    'polynomial': ['polynomial-host-targeting', 'wide-polynomial-competitiveness'],
}


def read(path):
    return json.loads(Path(path).read_text())


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def gaps():
    workload = read(live('1362381c/workload-set.json'))
    for cell in workload['cells']:
        if not cell['primary']:
            continue
        directory = (ROOT / cell['receipt']).parent
        summary = read(directory / 'acceptance-summary.json')
        result = next(row for row in summary['cells'] if row['cell_id'] == cell['cell'])
        receipt = read(directory / 'receipt.json')
        addendum = read(directory / receipt['addendum']['snapshot'])
        declared = next(row for row in addendum['cells'] if row['cell_id'] == cell['cell'])
        plan = read(directory / 'plan.json')
        planned = next(row for row in plan['cells'] if row['cell_id'] == cell['cell'])
        external_candidate = planned['candidate_arm'] in cell['external_arms']
        # The canonical interpretation describes which plan side wins; the external
        # side is read from the workload declaration, never inferred from a name.
        material = declared['objective'] == 'comparator-gap' and (
            external_candidate and result['outcome'] == 'pass')
        if material:
            yield cell, receipt, plan, planned, declared, result


def executable(record):
    """The measured executable, or an identical copy in the configured arm root."""
    path = Path(record['executable_path'])
    if path.is_file() and digest(path) == record['executable_sha256']:
        return path
    root = os.environ.get('GF2_PROFILE_ARM_ROOT')
    if root:
        for candidate in sorted(Path(root).rglob(path.name)):
            if candidate.is_file() and digest(candidate) == record['executable_sha256']:
                return candidate
    # Expansion remains reviewable before an archived arm is restored. Identity
    # refuses to launch a series until these bytes are available.
    return path


def cases(subject):
    if subject == 'bch':
        plan = read(live('1358d6ab/bch/pilot/plan.json'))
        build = read(live('1358d6ab/bch/build-identity.json'))
        addendum = read(live('1358d6ab/bch/pilot/addendum.json'))
        declared = {row['cell_id']: row for row in addendum['cells']}
        generator = load('survey/make-family.py', 'profile_family_settings')
        settings = generator.shared_settings(generator.schema())
        for cell in plan['cells']:
            for arm_name in (cell['baseline_arm'], cell['candidate_arm']):
                arm = plan['arms'][arm_name]
                binary = next(row for row in build['arms'].values()
                              if Path(row['path']).name == Path(arm['executable']).name)
                yield expanded(plan, cell, declared[cell['cell_id']], arm_name,
                               Path(build['build_root']) / binary['path'],
                               binary['executable_sha256'], settings)
        return
    for cell, receipt, plan, planned, declared, _ in gaps():
        if cell['family'] not in SUBJECTS[subject]:
            continue
        for name in (planned['baseline_arm'],):
            arm = receipt['arms'][name]
            yield expanded(plan, planned, declared, name, executable(arm),
                           arm['executable_sha256'], receipt['settings'])


def expanded(plan, cell, declared, name, binary, sha, settings):
    windows = settings['windows_per_execution']
    request = dict(schema='zen3-benchmark-arm-request-v1', cell_id=cell['cell_id'],
                   arm=name, role='baseline' if name == cell['baseline_arm'] else 'candidate', pair=0, case=cell['case'],
                   cache_state=declared['cache_state'], windows=windows,
                   window_target_ms=settings['window_target_ms'], cpus=[],
                   workers_declared=declared['workers']['declared'])
    return dict(label=cell['cell_id']+'--'+name, cell=cell['cell_id'],
                executable=str(binary), sha256=sha,
                environment=plan['arms'][name]['environment'], request=request,
                scope='whole process, including setup and warm-up')


def identity(path):
    rows = [json.loads(line) for line in Path(path).read_text().splitlines()]
    for row in rows:
        if not Path(row['executable']).is_file():
            raise SystemExit('measured executable unavailable: '+row['executable']+
                             '; restore identical bytes via GF2_PROFILE_ARM_ROOT')
        if digest(row['executable']) != row['sha256']:
            raise SystemExit('executable differs: '+row['executable'])
    print('cases='+digest(path)+' session='+digest(HERE/'arm-session.sh')+
          ' driver='+digest(Path(__file__))+' perf-settings='+digest(live('3be770d5/survey/profile-session.sh')))


def session(out, path):
    if os.environ.get('GF2_BENCH_WINDOW') != '1':
        raise SystemExit('profiles require GF2_BENCH_WINDOW=1')
    identity(path)
    directory = Path(out)
    directory.mkdir(parents=True, exist_ok=True)
    session_source = live('3be770d5/survey/profile-session.sh').read_text()
    constants = dict(re.findall(r'^(GROUP_CORE|GROUP_MEMORY|SAMPLE_PERIOD)=(.+)$', session_source, re.M))
    for row in map(json.loads, Path(path).read_text().splitlines()):
        stem = directory / row['label']
        order = ('schema', 'cell_id', 'arm', 'role', 'pair', 'case', 'cache_state',
                 'windows', 'window_target_ms', 'cpus', 'workers_declared')
        request = {key: row['request'][key] for key in order}
        # Resolve the campaign's single physical CPU from Linux affinity.
        # All prepared nondecoder subjects declare one worker.
        if request['workers_declared'] != 1:
            raise SystemExit('subject needs a worker-aware session: '+row['label'])
        request['cpus'] = [min(os.sched_getaffinity(0))]
        inherited = {k: v for k, v in os.environ.items() if not k.startswith(('GF2_', 'RAYON_'))}
        env = {**inherited, **row['environment'], 'GF2_TUNING_FRESH_CASE': 'child-v2'}
        # serde_json::Value uses ordered object keys inside the case.
        request['case'] = json.loads(json.dumps(request['case'], sort_keys=True))
        wire = json.dumps(request, separators=(',', ':'))
        for group, events in [('core', constants['GROUP_CORE']),
                              ('memory', constants['GROUP_MEMORY'])]:
            result = subprocess.run(['taskset', '-c', str(request['cpus'][0]),
                                     'perf', 'stat', '-x', ',', '-o', str(stem)+'.'+group+'.csv',
                                     '-e', events, '--', row['executable']],
                                    input=wire, text=True, capture_output=True, env=env)
            (directory/'stat').mkdir(exist_ok=True)
            (directory/'stat'/f"{row['label']}.{group}.status").write_text(str(result.returncode)+'\n')
            Path(str(stem)+'.'+group+'.jsonl').write_text(result.stdout)
            Path(str(stem)+'.'+group+'.err').write_text(result.stderr)
            if result.returncode:
                raise SystemExit('profile arm failed: '+row['label']+'; '+result.stderr[:200])
        data = str(stem)+'.data'
        result = subprocess.run(['taskset', '-c', str(request['cpus'][0]), 'perf', 'record', '-q',
                                 '-e', 'cycles:u', '-c', constants['SAMPLE_PERIOD'], '-o', data, '--',
                                 row['executable']], input=wire, text=True, capture_output=True, env=env)
        Path(str(stem)+'.record.jsonl').write_text(result.stdout)
        Path(str(stem)+'.record.err').write_text(result.stderr)
        if result.returncode:
            raise SystemExit('perf record failed: '+row['label'])
        report = subprocess.run(['perf', 'report', '-i', data, '--stdio', '--no-children', '-n',
                                 '-g', 'none', '--percent-limit', '0', '--sort', 'dso,sym'],
                                capture_output=True, text=True, check=True)
        Path(str(stem)+'.symbols.txt').write_text(report.stdout)
        Path(data).unlink()


def main():
    if sys.argv[1] == 'cases':
        for row in cases(sys.argv[2]):
            print(json.dumps(row, sort_keys=True, separators=(',', ':')))
    elif sys.argv[1] == 'identity':
        identity(sys.argv[2])
    elif sys.argv[1] == 'session':
        session(*sys.argv[2:])
    else:
        raise SystemExit(__doc__)


if __name__ == '__main__':
    main()
