#!/usr/bin/env python3
"""Capture the survey source/build/input closure with the canonical manifest.

Run after builds and quality validation, before preparing a timed plan. The
archives preserve exact producing bytes; the canonical runner snapshots and
hashes them before its first measurement. Excludes build trees from upstream
source archives, while retaining observed build flags and static-library hash.
"""
import argparse
import gzip
import io
import json
import pathlib
import subprocess
import tarfile

p = argparse.ArgumentParser()
p.add_argument('--external', type=pathlib.Path, required=True)
p.add_argument('--evidence', type=pathlib.Path, required=True)
a = p.parse_args()
archives = a.evidence / 'source-inputs'
archives.mkdir(exist_ok=True)

def archive(root, output, exclude):
    with output.open('wb') as raw, gzip.GzipFile(filename='', mode='wb', fileobj=raw, mtime=0) as gz:
        with tarfile.open(fileobj=gz, mode='w|') as tar:
            for path in sorted(root.rglob('*')):
                relative = path.relative_to(root)
                if any(part in exclude for part in relative.parts) or not path.is_file() or path.is_symlink():
                    continue
                info = tarfile.TarInfo(str(relative))
                content = path.read_bytes()
                info.size = len(content)
                info.mode = path.stat().st_mode & 0o777
                tar.addfile(info, io.BytesIO(content))

archive(a.external / 'aff3ct', archives / 'aff3ct-source.tar.gz', {'.git', 'build', 'build-cxx17', '__pycache__'})
archive(a.external / 'inputs', archives / 'recorded-inputs.tar.gz', set())
# Pin the surveyed source (without radio-stack builds) for review of exclusions.
for name in ['srsran', 'xdsopl-ldpc', 'oai', 'simde']:
    archive(a.external / name, archives / (name + '-source.tar.gz'), {'.git', 'build', 'build-ldpc', '__pycache__'})
base = json.loads(pathlib.Path('dev/active/f547c394/producing-inputs.json').read_text())
survey = pathlib.Path('dev/active/c077a88b/survey')
behavior = [str(p) for p in survey.rglob('*') if p.is_file() and p.suffix in {'.rs', '.cpp', '.py', '.sh'} and 'target' not in p.parts]
# Cargo's resolved local dependency graph is the source of the producing closure.
# This includes transitive gf2-sim dependencies rather than a private crate list.
metadata = json.loads(subprocess.check_output([
    './scripts/cargo-budget.sh', 'cargo', '+1.95', 'metadata', '--offline', '--locked',
    '--format-version', '1', '--manifest-path', str(survey / 'harness/Cargo.toml'),
    '--features', 'aff3ct',
]))
repo = pathlib.Path.cwd().resolve()
resolved = {node['id'] for node in metadata['resolve']['nodes']}
for package in metadata['packages']:
    manifest = pathlib.Path(package['manifest_path'])
    if package['id'] not in resolved or not manifest.is_relative_to(repo):
        continue
    directory = manifest.parent.relative_to(repo)
    behavior += [str(p) for p in (directory / 'src').rglob('*') if p.is_file()]
    base['build_inputs'].append(str(manifest.relative_to(repo)))
    build = directory / 'build.rs'
    if build.exists(): behavior.append(str(build))
behavior += ['dev/bench_results/c077a88b/run-campaign.sh']
base['behavior_sources'] = sorted(set(base['behavior_sources'] + [p for p in behavior if not p.endswith(('freeze-addendum.py', 'summarize.py', 'prepare-v3.py'))]))
base['build_inputs'] = sorted(set(base['build_inputs'] + behavior + [
    str(p) for p in archives.iterdir()] + [str(p) for p in (a.evidence / 'quality').glob('*.json')] + [
    str(survey / 'harness/Cargo.toml'), str(survey / 'harness/Cargo.lock'),
    str(a.evidence / 'build-identity.json'), str(a.evidence / 'validation.json'),
    'dev/bench_results/c077a88b/2026-09-08-r4-c077a88b-preparation/build-identity.json',
]))
(survey / 'producing-inputs.json').write_text(json.dumps(base, indent=2) + '\n')
