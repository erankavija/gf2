#!/usr/bin/env bash
# Timed phase of the eb9b324c pilot: 16 executions, one lock session.
# Invoked inside dev/scripts/ccx1-bench-flock.sh. Diagnosis-scale evidence,
# not a receipt: single arm, 16 translations, no comparison checker is run.
set -uo pipefail

S=/tmp/gf2-pilot-eb9b324c
STAGE="$S/cand-fixed"
OUT=/tmp/gf2-eb9b324c-pilot.csv
MAIN=/home/vkaskivuo/Projects/gf2
LOG="$S/timed.log"
SLOTS="$S/slots.tsv"

: > "$LOG"
printf 'slot\tE\texecution\tstart_unix\tstart_iso\n' > "$SLOTS"

log() { echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) $*" >> "$LOG"; }

if [ -e "$OUT" ]; then log "ABORT: $OUT already exists"; exit 90; fi

log "pilot timed phase start"
log "pwd=$(pwd)"
log "affinity=$(grep Cpus_allowed_list /proc/self/status | tr -d '\t')"
log "niceness=$(ps -o ni= -p $$ | tr -d ' ')"
log "loadavg=$(cat /proc/loadavg)"
log "locks=$(grep ":$(stat -c %i /tmp/gf2-ccx1.lock) " /proc/locks | tr '\n' ';')"
log "lockfile_stat=$(stat -c 'dev=%D inode=%i' /tmp/gf2-ccx1.lock)"
log "main_head=$(git -C "$MAIN" rev-parse HEAD) tracked_dirty=$(git -C "$MAIN" status --porcelain | wc -l) untracked_incl=$(git -C "$MAIN" status --porcelain --untracked-files=all | wc -l)"
log "governor=$(for c in 6 7 8 9 10 11; do cat /sys/devices/system/cpu/cpu$c/cpufreq/scaling_governor; done | tr '\n' ',')"

HEAD0=$(git -C "$MAIN" rev-parse HEAD)
SLOT=0
for e in $(seq 0 15); do
  h=$(git -C "$MAIN" rev-parse HEAD)
  if [ "$h" != "$HEAD0" ]; then log "GUARD ABORT head moved to $h"; exit 92; fi
  SLOT=$((SLOT + 1))
  printf '%d\t%d\t%d\t%s\t%s\n' "$SLOT" "$e" "$((e + 1))" "$(date +%s)" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$SLOTS"
  ( cd "$MAIN" && "$STAGE/member-$e.bin" --execution "$((e + 1))" --repetitions 1 \
      --target-ms 250 --output "$OUT" --append ) >> "$LOG" 2>&1
  rc=$?
  if [ $rc -ne 0 ]; then log "EXECUTION FAILED E=$e rc=$rc"; exit 94; fi
done

log "pilot timed phase end"
log "loadavg=$(cat /proc/loadavg)"
log "rows=$(wc -l < "$OUT")"
