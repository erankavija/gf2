#!/usr/bin/env python3
"""Rebuild the measured gap-cell executables from their receipts and record each attempt.

`build <subject>` rebuilds every distinct baseline executable the subject's gap
cells measured, at the receipt's recorded build root, from the source revision
its launcher log names. A digest match copies the bytes under the restoration
root that `GF2_PROFILE_ARM_ROOT` names in the series' queue line. Every attempt,
matched or not, is appended to `restoration.json`.
"""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib

sys.dont_write_bytecode = True
from locate import HERE, ROOT
from importlib import import_module
arms = import_module('arm-profiles')

RECORD = HERE/'restoration.json'
RESTORED = ROOT/'.agents/ext/78f0aafd/restored'
# Plan constants: each historical build command, cited by its script at the
# measured revision. `target` is the CARGO_TARGET_DIR relative to the build root.
RECIPES = {
    ('nr-rate-matched-encode-baselines-v1', 'gf2-native'): dict(
        script='dev/active/12fdeb5b/survey/nr-encode-build.sh', target='.agents/ext/12fdeb5b/target-nr-encode-native',
        rustflags='-C target-cpu=native', features='aff3ct,srsran',
        env={'GF2_AFF3CT_ROOT': '{aff3ct root}', 'GF2_SRSRAN_ROOT': '{srsran root}'}),
    ('byte-field-matrix-product', 'gf2-element'): dict(
        script='dev/bench_results/6c6b09b1/run-byte-field-v4.sh', target='target/6c6b09b1-survey',
        rustflags='-C target-cpu=native', features=None, env={}),
    ('polynomial-host-targeting', 'gf2-conservative'): dict(
        script='dev/active/c7113c5a/survey/fetch-build.sh', target='target/c7113c5a-ext/arms-conservative',
        rustflags='', features=None,
        env={'GF2X_PREFIX': '{root}/target/c7113c5a-ext/prefix-conservative', 'GF2X_CFLAGS': '-O2'}),
    ('polynomial-host-targeting', 'gf2-tuned'): dict(
        script='dev/active/c7113c5a/survey/fetch-build.sh', target='target/c7113c5a-ext/arms-tuned',
        rustflags='-C target-cpu=x86-64-v3', features=None,
        env={'GF2X_PREFIX': '{root}/target/c7113c5a-ext/prefix-tuned', 'GF2X_CFLAGS': '-O3 -march=x86-64-v3'}),
    ('wide-polynomial-competitiveness', 'gf2-long-product'): dict(
        script='dev/active/53c5a8c0/survey/build-arms.sh', target='target/53c5a8c0-arms-native',
        rustflags='-C target-cpu=native', features=None,
        env={'GF2X_PREFIX': '{primary}/.agents/ext/c7113c5a/prefix-native',
             'GF2X_CFLAGS': '{primary configure cflags}'}),
}
RECIPES[('byte-field-region-axpy', 'gf2-element')] = RECIPES[('byte-field-matrix-product', 'gf2-element')]
RECIPES[('byte-field-matrix-product', 'gf2-wide')] = RECIPES[('byte-field-matrix-product', 'gf2-element')]
RECIPES[('byte-field-region-axpy', 'gf2-wide')] = RECIPES[('byte-field-matrix-product', 'gf2-element')]


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def git(*args, **kwargs):
    return subprocess.run(['git', '-C', str(ROOT), *args], check=True, capture_output=True, **kwargs).stdout


def executables(subject):
    """One entry per distinct measured digest, with every receipt arm that names it."""
    found = {}
    for cell, receipt, _, planned, _, _ in arms.gaps():
        if cell['family'] not in arms.SUBJECTS[subject]:
            continue
        arm = receipt['arms'][planned['baseline_arm']]
        entry = found.setdefault(arm['executable_sha256'], dict(
            arm=arm, family=cell['family'], name=planned['baseline_arm'],
            receipt=(ROOT/cell['receipt']).parent, uses=set()))
        entry['uses'].add((cell['receipt'], planned['baseline_arm']))
    return found


def launcher_fields(receipt_dir):
    log = (receipt_dir/'launcher.log').read_text()
    return dict(re.findall(r'^# ([a-z0-9 ()]+): (\S+)$', log, re.M))


def inputs_at(revision, receipt):
    """Paths whose recorded build-input digest differs from the revision's blob."""
    differing = []
    for path, sha in receipt['source']['producing']['build_inputs_sha256'].items():
        try:
            blob = git('show', f'{revision}:{path}')
        except subprocess.CalledProcessError:
            differing.append(path)
            continue
        if hashlib.sha256(blob).hexdigest() != sha:
            differing.append(path)
    return differing


def manifest_for(root, receipt, binary):
    """The build-input manifest that declares the measured executable as a binary."""
    for path in receipt['source']['producing']['build_inputs_sha256']:
        if Path(path).name != 'Cargo.toml':
            continue
        manifest = tomllib.loads((root/path).read_text())
        discovered = manifest.get('package', {}).get('autobins', True) and (
            root/path).parent.joinpath('src/bin', binary+'.rs').is_file()
        if discovered or any(entry['name'] == binary for entry in manifest.get('bin', [])):
            return path
    raise SystemExit('no build-input manifest declares '+binary)


def stage(root, revision):
    marker = root/'.restoration-source'
    if root.exists():
        if not marker.is_file() or marker.read_text().strip() != revision:
            raise SystemExit(f'build root {root} is occupied by something other than {revision}')
        return
    root.mkdir(parents=True)
    archive = subprocess.Popen(['git', '-C', str(ROOT), 'archive', revision], stdout=subprocess.PIPE)
    subprocess.run(['tar', '-x', '-C', str(root)], stdin=archive.stdout, check=True)
    if archive.wait():
        raise SystemExit('git archive failed for '+revision)
    marker.write_text(revision+'\n')


def substitutions(root, fields):
    primary = Path(git('rev-parse', '--git-common-dir', text=True).strip()).resolve().parent
    configure = primary/'.agents/ext/c7113c5a/build-native/configure.stdout.log'
    cflags = re.search(r'^configure: *CFLAGS="(.*)"$', configure.read_text(), re.M) if configure.is_file() else None
    return {'root': str(root), 'primary': str(primary),
            'primary configure cflags': cflags[1] if cflags else '',
            'aff3ct root': fields.get('aff3ct root', ''), 'srsran root': fields.get('srsran root', '')}


def attempt(sha, entry):
    recipe = RECIPES[(entry['family'], entry['name'])]
    receipt = json.loads((entry['receipt']/'receipt.json').read_text())
    fields = launcher_fields(entry['receipt'])
    revision = fields['gf2 revision (informational)']
    measured = Path(entry['arm']['executable_path'])
    suffix = Path(recipe['target'])/'release'/measured.name
    if measured.parts[-len(suffix.parts):] != suffix.parts:
        raise SystemExit(f'{measured} is not under the recipe target {recipe["target"]}')
    root = Path(*measured.parts[:-len(suffix.parts)])
    differing = inputs_at(revision, receipt)
    if differing:
        raise SystemExit(f'{revision} differs from the recorded build inputs: {differing}')
    stage(root, revision)
    values = substitutions(root, fields)
    env ={key: re.sub(r'\{([^}]+)\}', lambda m: values[m[1]], value) for key, value in recipe['env'].items()}
    manifest = manifest_for(root, receipt, measured.name)
    command = ['./scripts/cargo-budget.sh', 'cargo', 'build', '--release', '--locked',
               '--manifest-path', manifest, '--bin', measured.name]
    if recipe['features']:
        command += ['--features', recipe['features']]
    toolchain = receipt['toolchain']
    build_env = {**os.environ, **env, 'CARGO_TARGET_DIR': str(root/recipe['target']),
                 'RUSTFLAGS': recipe['rustflags'], 'RUSTUP_TOOLCHAIN': '1.95',
                 'PATH': str(Path.home()/'.cargo/bin')+':'+os.environ['PATH']}
    build_env.pop('CARGO_BUILD_RUSTFLAGS', None)
    observed = subprocess.run(['rustc', '--version'], env=build_env, cwd=root,
                              capture_output=True, text=True, check=True).stdout.strip()
    result = subprocess.run(command, env=build_env, cwd=root, capture_output=True, text=True)
    obtained = digest(measured) if result.returncode == 0 and measured.is_file() else None
    verdict = 'restored' if obtained == sha else ('build-failed' if obtained is None else 'digest-differs')
    restored = None
    if verdict == 'restored':
        target = RESTORED/sha/measured.name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(measured, target)
        restored = target.relative_to(ROOT).as_posix()
    return dict(
        executable=measured.name, expected_sha256=sha, obtained_sha256=obtained, verdict=verdict,
        measured_by=sorted(f'{path}#{arm}' for path, arm in entry['uses']),
        build=dict(revision=revision, revision_source=(entry['receipt']/'launcher.log').relative_to(ROOT).as_posix(),
                   recorded_inputs_matching=len(receipt['source']['producing']['build_inputs_sha256']),
                   build_root=str(root), manifest=manifest, target_directory=recipe['target'],
                   rustflags=recipe['rustflags'], receipt_rustflags=entry['arm'].get('rustflags'),
                   environment=env, command=command, recipe_script=recipe['script'],
                   receipt_toolchain=toolchain, observed_toolchain=observed,
                   exit_status=result.returncode,
                   stderr_tail=result.stderr.splitlines()[-3:] if result.returncode else []),
        restored_path=restored)


def smoke(subject, runner):
    """Untimed runner smoke of each restored arm on a throwaway plan of its gap cells.

    The measured comparators are not restored, so each cell's candidate arm runs
    the restored baseline executable under the candidate's declared build class.
    """
    restored = {row['expected_sha256']: row['restored_path']
                for row in json.loads(RECORD.read_text())['attempts'] if row['verdict'] == 'restored'}
    plans = {}
    for cell, receipt, plan, planned, _, _ in arms.gaps():
        if cell['family'] not in arms.SUBJECTS[subject]:
            continue
        name = planned['baseline_arm']
        arm = dict(plan['arms'][name], executable=restored[receipt['arms'][name]['executable_sha256']])
        directory = (ROOT/cell['receipt']).parent
        throwaway = plans.setdefault(directory.name, dict(
            plan, addendum=(ROOT/receipt['addendum']['path']).relative_to(ROOT).as_posix(),
            arms={}, cells=[]))
        candidate = planned['candidate_arm']
        throwaway['arms'][name] = arm
        throwaway['arms'][candidate] = dict(arm, build=plan['arms'][candidate]['build'],
                                            description='restored '+name+' in the candidate position')
        throwaway['cells'].append(planned)
    out = HERE/'smoke'/'restored'
    out.mkdir(parents=True, exist_ok=True)
    for label, plan in sorted(plans.items()):
        path = out/f'{label}.plan.json'
        path.write_text(json.dumps(plan, indent=1, sort_keys=True)+'\n')
        result = subprocess.run([runner, 'smoke', str(path), '--record', str(out/f'{label}.smoke.json')],
                                cwd=ROOT, capture_output=True, text=True)
        (out/f'{label}.smoke.err').write_text(result.stderr)
        (out/f'{label}.smoke.status').write_text(f'{result.returncode}\n')
        print(label, 'smoke exit', result.returncode)


def main():
    if len(sys.argv) == 4 and sys.argv[1] == 'smoke' and sys.argv[2] in arms.SUBJECTS:
        return smoke(sys.argv[2], sys.argv[3])
    if len(sys.argv) != 3 or sys.argv[1] != 'build' or sys.argv[2] not in arms.SUBJECTS:
        raise SystemExit(__doc__)
    for sha, entry in sorted(executables(sys.argv[2]).items()):
        row = attempt(sha, entry)
        # Subjects build concurrently; the record is re-read under a lock.
        with open(RESTORED.parent/'restoration.lock', 'w') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            record = json.loads(RECORD.read_text()) if RECORD.is_file() else {'attempts': []}
            record['attempts'].append(row)
            RECORD.write_text(json.dumps(record, indent=1, sort_keys=True)+'\n')
        print(row['executable'], sha[:16], row['verdict'], row['obtained_sha256'])


if __name__ == '__main__':
    main()
