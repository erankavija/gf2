#!/usr/bin/env bash
# Session-5 driver for 50b47eae (lead-authored, 2026-08-22, per verdict v4 / DEC-K).
# Builds both arms' 128-member translation-only ensembles (v4 C2), verifies the
# realized construction per-arm and cross-arm (v4 C2/C3) before any timed
# window, runs the timed phase in one lock session (v1 S5 via run-ensemble.sh),
# then copies all durable outputs into the repo (untracked) and runs the
# committed comparison and layout-audit modes.
# Terminal log line: "DRIVER: COMPLETE" or "DRIVER: ABORTED <reason>".
set -uo pipefail

S=/tmp/gf2-ens5
LOG="$S/logs/continue-driver.log"
MAIN=/home/vkaskivuo/Projects/gf2
CTRL=/home/vkaskivuo/Projects/gf2/.agents/worktrees/control-0c072d73
TP="$MAIN/dev/benchmarks/tuning_profiles"
OUTDIR="$MAIN/dev/active/50b47eae/s5-session"
REF_OUT=/tmp/gf2-50b47eae-s5-reference-arm.csv
CAND_OUT=/tmp/gf2-50b47eae-s5-candidate-arm.csv

mkdir -p "$S/logs"
log() { echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) $*" >> "$LOG"; }
abort() { log "DRIVER: ABORTED $*"; exit 1; }

: >> "$LOG"
log "continue-driver start pid=$$"

# 1. Build phase, both arms sequentially, no lock held.
bash "$S/build-arm.sh" ref "$CTRL" || abort "reference build phase failed (see $S/logs/build-ref.log)"
log "reference arm built"
bash "$S/build-arm.sh" cand "$MAIN" || abort "candidate build phase failed (see $S/logs/build-cand.log)"
log "candidate arm built"
date -u +%Y-%m-%dT%H:%M:%SZ > "$S/logs/build-phase-end.txt"

# 2. Verify the realized construction, per-arm and cross-arm; abort before any timed window.
for arm in ref cand; do
  if ! python3 "$S/verify-construction.py" "$S/logs/ledger-$arm.tsv" "$arm" >> "$LOG" 2>&1; then
    abort "construction verification FAILED for arm $arm (see log above)"
  fi
done
if ! python3 "$S/verify-construction.py" --cross "$S/logs/ledger-ref.tsv" "$S/logs/ledger-cand.tsv" >> "$LOG" 2>&1; then
  abort "cross-arm verification FAILED (v4 C3; goes to the owner)"
fi
log "construction verified: both arms and cross-arm"

# 3. Preconditions for the timed phase.
[ -e "$REF_OUT" ] && abort "output exists: $REF_OUT"
[ -e "$CAND_OUT" ] && abort "output exists: $CAND_OUT"
idle_deadline=$(( $(date +%s) + 3600 ))
while :; do
  load=$(cut -d' ' -f1 /proc/loadavg)
  ok=$(awk -v l="$load" 'BEGIN{print (l<0.5)?1:0}')
  [ "$ok" = "1" ] && break
  [ "$(date +%s)" -ge "$idle_deadline" ] && abort "host not idle within 1h (load=$load)"
  log "waiting for idle host, load=$load"
  sleep 60
done
log "host idle, load=$(cut -d' ' -f1 /proc/loadavg)"
s_main=$(git -C "$MAIN" status --porcelain --untracked-files=all | wc -l)
s_ctrl=$(git -C "$CTRL" status --porcelain --untracked-files=all | wc -l)
[ "$s_main" = "0" ] || abort "main checkout not clean ($s_main lines)"
[ "$s_ctrl" = "0" ] || abort "control worktree not clean ($s_ctrl lines)"
log "preflight: main_head=$(git -C "$MAIN" rev-parse HEAD) ctrl_head=$(git -C "$CTRL" rev-parse HEAD) both clean"

# 4. Timed phase: one lock session over all 256 executions (v1 S5.2 order).
cd "$MAIN" || abort "cd MAIN failed"
GF2_BENCH=1 ./dev/scripts/ccx1-bench-flock.sh bash "$S/run-ensemble.sh" "$REF_OUT" "$CAND_OUT" >> "$LOG" 2>&1
rc=$?
[ $rc -eq 0 ] || abort "timed phase exited rc=$rc (see $S/logs/timed.log)"
log "timed phase complete"

# 5. Copy durable outputs into the repo (untracked; the lead reviews and commits).
mkdir -p "$OUTDIR" || abort "mkdir OUTDIR"
cp "$REF_OUT" "$TP/2026-08-22-ensemble-reference-arm-5.csv" || abort "copy ref csv"
cp "$CAND_OUT" "$TP/2026-08-22-ensemble-candidate-arm-5.csv" || abort "copy cand csv"
bash "$S/emit-tsv.sh" "$S/logs/ledger-ref.tsv" "$TP/2026-08-22-member-provenance-ref-5.tsv" || abort "emit ref tsv"
bash "$S/emit-tsv.sh" "$S/logs/ledger-cand.tsv" "$TP/2026-08-22-member-provenance-cand-5.tsv" || abort "emit cand tsv"
cp "$S/logs/timed.log" "$S/logs/slots.tsv" "$S/logs/continue-driver.log" "$S/logs/build-ref.log" "$S/logs/build-cand.log" \
   "$S/logs/ledger-ref.tsv" "$S/logs/ledger-cand.tsv" "$S/logs/phi-ref.txt" "$S/logs/phi-cand.txt" \
   "$S/members-ref.tsv" "$S/members-cand.tsv" "$OUTDIR/" 2>>"$LOG"
cp "$S/build-arm.sh" "$S/gen-members.py" "$S/run-ensemble.sh" "$S/verify-construction.py" "$S/emit-tsv.sh" "$S/continue-driver.sh" "$OUTDIR/" 2>>"$LOG"
( cd "$TP" && sha256sum 2026-08-22-ensemble-reference-arm-5.csv 2026-08-22-ensemble-candidate-arm-5.csv 2026-08-22-member-provenance-ref-5.tsv 2026-08-22-member-provenance-cand-5.tsv ) > "$OUTDIR/sha256-manifest.txt" 2>>"$LOG"
( cd "$S" && sha256sum build-arm.sh gen-members.py run-ensemble.sh verify-construction.py emit-tsv.sh continue-driver.sh ) >> "$OUTDIR/sha256-manifest.txt" 2>>"$LOG"
log "outputs copied; manifest at $OUTDIR/sha256-manifest.txt"

# 6. Comparison (verdict) and layout-audit, from the main checkout, outputs teed.
cd "$MAIN" || abort "cd MAIN failed"
cargo +1.95.0 bench -p gf2-core --features simd --bench selector_non_regression -- \
  --compare "$TP/2026-08-22-ensemble-reference-arm-5.csv" \
  --against "$TP/2026-08-22-ensemble-candidate-arm-5.csv" \
  > "$OUTDIR/comparison.txt" 2>> "$LOG"
log "comparison rc=$? (output: $OUTDIR/comparison.txt)"
cargo +1.95.0 bench -p gf2-core --features simd --bench selector_non_regression -- \
  --layout-audit "$TP/2026-08-22-ensemble-reference-arm-5.csv" \
  --layout-candidate "$TP/2026-08-22-ensemble-candidate-arm-5.csv" \
  --compare "$TP/2026-08-19-pre-cutover-baseline.csv" \
  --against "$TP/2026-08-20-post-cutover-control-arm-2.csv" \
  > "$OUTDIR/layout-audit.txt" 2>> "$LOG"
log "layout-audit rc=$? (output: $OUTDIR/layout-audit.txt)"

log "DRIVER: COMPLETE"
