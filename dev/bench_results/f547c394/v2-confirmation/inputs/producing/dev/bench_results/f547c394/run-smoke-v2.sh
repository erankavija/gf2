#!/usr/bin/env bash
# Reproducible bounded v2 pipeline proof. No production performance claims.
# Usage: run-smoke-v2.sh prepare|session|finalize pilot|confirmation
# Prepare freezes final addendum and plan bytes, builds all executables before
# taking the timing lock, and never overwrites a campaign that already exists.
set -euo pipefail
ACTION=${1:?prepare|session|finalize}
MODE=${2:?pilot|confirmation}
case "$MODE" in pilot|confirmation) ;; *) exit 2;; esac
OUT="dev/bench_results/f547c394/v2-$MODE"
STAGE="/tmp/gf2-f547c394-v2-$MODE-r1"
PLAN="dev/active/f547c394/plan-smoke-v2-$MODE-r1.json"
ADDENDUM="dev/active/f547c394/addendum-smoke-v2-$MODE-r1.json"
LEDGER="dev/bench_results/f547c394/v2-family-ledger.jsonl"
case "$ACTION" in
prepare)
  if [[ -e "$STAGE" || -e "$OUT" || -e "$PLAN" || -e "$ADDENDUM" ]]; then
    echo 'campaign exists; resume it, do not overwrite its frozen inputs' >&2; exit 2
  fi
  if [[ "$MODE" == pilot && ! -e "$LEDGER" ]]; then
    # Explicit genesis, never automatically recreated for confirmation.
    (set -o noclobber; : > "$LEDGER")
  fi
  test -f "$LEDGER"
  CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support \
    --bin benchmark-ab-runner --bin benchmark-acceptance --bin ab-smoke-workload
  python3 - "$MODE" "$PLAN" "$ADDENDUM" "$LEDGER" <<'PY'
import datetime, hashlib, json, pathlib, sys
mode, planpath, addendumpath, ledger = sys.argv[1:]
a=json.loads(pathlib.Path('dev/active/f547c394/addendum-protocol-smoke-pilot.json').read_bytes())
a['schema']='zen3-benchmark-addendum-v2'
a['protocol']['version']=2
a['family'].update(id='protocol-smoke-v2', purpose='decoder-family', description='Synthetic timing and deterministic decoder-quality pipeline proof; no gf2 performance or coding-quality claim.')
a['frozen']['frozen_utc']=datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')
a['family_wise']['ledger_path']=ledger
# Identical arms exercise non-inferiority, a deliberately losing arm preserves
# a negative confirmation. Pilot widths determine confirmation resolution.
for i,c in enumerate(a['cells']):
 c['cell_id']=['decoder-pipeline','cold-first-use'][i]
 c['role']='exploratory' if mode=='pilot' else 'confirmatory'
 c['objective']='non-regression' if i==0 else 'improvement'
 c['workload']['size']['words']=4096
 c['cache_state']='warm' if i==0 else 'cold'
 if i==1: c['cold_calls']=10000
c=a['cells'][0]
c['decoder']={
 'arm_kind':'fastest-quality-compatible',
 'code':{'identity':'synthetic-pipeline-code','n':64,'k':32,'h_sha256':hashlib.sha256(b'synthetic-pipeline-H-v2').hexdigest()},
 'input':{'llr_source':'synthetic deterministic frame slots, not channel samples','llr_sha256':hashlib.sha256(b'synthetic-pipeline-input-v2').hexdigest(),'frames':1000,'seed':20260908,'codeword_source':'both','snr_db':0.0},
 'precision':'f32','schedule':'flooding','normalization':{'kind':'min-sum','factor':None},'iteration_cap':1,
 'stopping':{'kind':'fixed','crc':None},'batching':{'batch_size':1,'batch_fill_included':True},
 'quality_tolerance':{'fer_ratio_max':1.5,'confidence':0.95},'rate_matching':None}
a['cells'][1]['decoder']=dict(c['decoder'])
a['effect']['equivalence_margin']=1.3
a['effect']['equivalence_rationale']='Pipeline-only tolerance for identical synthetic arms; no adoption claim.'
if mode=='confirmation':
 pilot=pathlib.Path('dev/bench_results/f547c394/v2-pilot/receipt.json')
 summary=json.loads(pilot.with_name('acceptance-summary.json').read_bytes())
 assert summary['verdict']=='accepted'
 widths=[max(x['interval']['estimate']-x['interval']['lower'],x['interval']['upper']-x['interval']['estimate'])/x['interval']['estimate'] for x in summary['cells'] if x.get('interval')]
 resolution=max(widths)*2
 assert 0<resolution<0.45, 'pilot lacks required pipeline resolution; preserve it'
 a['effect']['measurement_resolution']=resolution
 a['effect']['resolution_evidence']={'receipt':str(pilot),'sha256':hashlib.sha256(pilot.read_bytes()).hexdigest()}
 a['effect']['rationale']='Pipeline threshold exceeds twice the largest observed pilot relative interval half-width.'
arm=lambda passes: {'build':'conservative-portable','description':f'synthetic XOR fold {passes} pass; deterministic quality fixture','executable':'target/release/ab-smoke-workload','arguments':[], 'environment':{'GF2_SMOKE_PASSES':str(passes)},'rustflags':None,'tuning_profile':None}
plan={'schema':'zen3-benchmark-plan-v1','campaign_id':f'protocol-v2-{mode}-r1','issue':'f547c394','label':'pilot' if mode=='pilot' else 'smoke','campaign_seed':20260908,'addendum':addendumpath,'lock_path':'/tmp/gf2-ccx1.lock','wrapper':'dev/scripts/ccx1-bench-flock.sh','timing_override':None,'arms':{'baseline':arm(1),'candidate':arm(1),'losing':arm(2)},'cells':[{'cell_id':c['cell_id'],'baseline_arm':'baseline','candidate_arm':'candidate' if i==0 else 'losing','case':{'words':4096,'seed':i+1},'pilot_pairs':6 if mode=='pilot' else None} for i,c in enumerate(a['cells'])],'max_cells_per_session':1}
for p,v in [(addendumpath,a),(planpath,plan)]:
 with open(p,'x') as f: json.dump(v,f,indent=2); f.write('\n')
PY
  ;;
session)
  test -f "$PLAN"
  test -f "$ADDENDUM"
  LAUNCH_LOG="dev/bench_results/f547c394/v2-$MODE-r1-launcher.log"
  printf '# session start UTC: %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$LAUNCH_LOG"
  printf '# command: GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host target/release/benchmark-ab-runner run %s %s\n' "$STAGE" "$PLAN" >> "$LAUNCH_LOG"
  set +e
  GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host \
    target/release/benchmark-ab-runner run "$STAGE" "$PLAN" 2>&1 | tee -a "$LAUNCH_LOG"
  code=${PIPESTATUS[0]}
  set -e
  printf '# session exit: %s; end UTC: %s\n' "$code" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$LAUNCH_LOG"
  [[ "$code" == 0 || "$code" == 3 ]]
  ;;
finalize)
  test ! -e "$OUT"
  target/release/benchmark-ab-runner finalize "$STAGE" "$OUT"
  cp "dev/bench_results/f547c394/v2-$MODE-r1-launcher.log" "$OUT/launcher.log"
  target/release/benchmark-acceptance "$OUT"
  ;;
*) echo 'usage: run-smoke-v2.sh prepare|session|finalize pilot|confirmation' >&2; exit 2;;
esac
