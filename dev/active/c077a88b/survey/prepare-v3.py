#!/usr/bin/env python3
"""Freeze the v3 pilots and seed canonical ledgers from immutable v1 attempts.

The imported zero-comparison entries are retrospective history, not claims that
v1 acquired a premeasurement ledger reservation. Receipt paths and hashes retain
the evidence; the shared trial_ledger schema validates the resulting chain.
"""
import datetime
import hashlib
import json
from pathlib import Path

base = Path('dev/bench_results/c077a88b')
active = Path('dev/active/c077a88b')
sha = lambda data: hashlib.sha256(data).hexdigest()
for family in ['matched-algorithm', 'quality-compatible']:
    family_id = f'ldpc-{family}-alist-v1'
    ledger = base / f'v3-{family}-family-ledger.jsonl'
    if ledger.exists():
        raise SystemExit(f'{ledger} exists; resume it without recreating history')
    lines, sources = [], []
    predecessor = '0' * 64
    for plan_path in sorted(base.glob(f'2026-*-ldpc-{family}-pilot/plan.json')):
        plan = json.loads(plan_path.read_bytes())
        addendum_path = plan_path.parent / 'inputs/family-addendum.json'
        addendum = json.loads(addendum_path.read_bytes())
        assert addendum['protocol']['version'] == 1
        assert all(c['role'] == 'exploratory' for c in addendum['cells'])
        entry = dict(sequence=len(lines), predecessor=predecessor, family=family_id,
                     campaign=plan['campaign_id'], addendum_sha256=sha(addendum_path.read_bytes()),
                     comparisons=0, protocol_version=1, candidates=[])
        line = json.dumps(entry, separators=(',', ':')).encode() + b'\n'
        lines.append(line)
        predecessor = sha(line)
        source = dict(sequence=entry['sequence'], line_sha256=predecessor,
                      plan=str(plan_path), plan_sha256=sha(plan_path.read_bytes()),
                      addendum=str(addendum_path), addendum_sha256=entry['addendum_sha256'])
        receipt = plan_path.parent / 'receipt.json'
        if receipt.exists():
            source.update(receipt=str(receipt), receipt_sha256=sha(receipt.read_bytes()))
        else:
            source['disposition'] = 'unfinished attempt; preserved plan, log and checkpoint'
        sources.append(source)
    with ledger.open('xb') as output:
        output.write(b''.join(lines))
    record = dict(kind='retrospective-v1-exploratory-history', family=family_id,
                  ledger=str(ledger), initial_sha256=sha(ledger.read_bytes()), sources=sources)
    (base / f'v3-{family}-ledger-origin.json').write_text(json.dumps(record, indent=2) + '\n')
    original = active / 'superseded' / f'v1-addendum-ldpc-{family}-pilot.json'
    pilot = json.loads(original.read_bytes())
    pilot['schema'] = 'zen3-benchmark-addendum-v3'
    pilot['protocol']['version'] = 3
    pilot['family']['id'] = family_id
    pilot['family']['description'] += (
        ' Protocol v3 retains the frozen corpus, operation and quality tolerance; '
        'its independent-frame BER and paired FER rules determine quality admission. '
        'All declared candidates remain visible when non-inferiority is unestablished. '
        'Prior v1 receipts are immutable superseded evidence and supply no v3 resolution.')
    pilot['family_wise']['ledger_path'] = str(ledger)
    pilot['frozen']['frozen_utc'] = datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')
    (active / f'addendum-ldpc-{family}-pilot.json').write_text(json.dumps(pilot, indent=2) + '\n')
    print(ledger)
