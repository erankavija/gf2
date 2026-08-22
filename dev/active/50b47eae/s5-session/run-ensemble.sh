#!/usr/bin/env bash
# Timed phase: 256 executions, one lock session, per v1 §5.1/§5.2 as amended by v4 C5.
# usage (inside the flock wrapper): run-ensemble.sh <ref-out.csv> <cand-out.csv>
set -uo pipefail

REF_OUT="$1"; CAND_OUT="$2"
STAGE=/tmp/gf2-ens5
MAIN=/home/vkaskivuo/Projects/gf2
CTRL=/home/vkaskivuo/Projects/gf2/.agents/worktrees/control-0c072d73
LOG=/tmp/gf2-ens5/logs/timed.log
SLOTS=/tmp/gf2-ens5/logs/slots.tsv

: > "$LOG"; printf 'slot\tmember_j\tarm\texecution\tstart_unix\tstart_iso\n' > "$SLOTS"

log() { echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) $*" >> "$LOG"; }

log "timed phase start"
log "pwd=$(pwd)"
log "affinity=$(grep Cpus_allowed_list /proc/self/status | tr -d '\t')"
log "niceness=$(ps -o ni= -p $$ | tr -d ' ')"
log "loadavg=$(cat /proc/loadavg)"
log "locks=$(grep ":$(stat -c %i /tmp/gf2-ccx1.lock) " /proc/locks | tr '\n' ';')"
log "lockfile_stat=$(stat -c 'dev=%D inode=%i' /tmp/gf2-ccx1.lock)"
log "main_head=$(git -C "$MAIN" rev-parse HEAD) main_status_lines=$(git -C "$MAIN" status --porcelain --untracked-files=all | wc -l)"
log "ctrl_head=$(git -C "$CTRL" rev-parse HEAD) ctrl_status_lines=$(git -C "$CTRL" status --porcelain --untracked-files=all | wc -l)"
log "governor=$(for c in 6 7 8 9 10 11; do cat /sys/devices/system/cpu/cpu$c/cpufreq/scaling_governor; done | tr '\n' ',')"

MAIN_HEAD0=$(git -C "$MAIN" rev-parse HEAD)
CTRL_HEAD0=$(git -C "$CTRL" rev-parse HEAD)

SLOT=0

guard() {
  local h s
  h=$(git -C "$MAIN" rev-parse HEAD); s=$(git -C "$MAIN" status --porcelain --untracked-files=all | wc -l)
  if [ "$h" != "$MAIN_HEAD0" ] || [ "$s" != "0" ]; then
    log "GUARD ABORT main head=$h status_lines=$s"; exit 92
  fi
  h=$(git -C "$CTRL" rev-parse HEAD); s=$(git -C "$CTRL" status --porcelain --untracked-files=all | wc -l)
  if [ "$h" != "$CTRL_HEAD0" ] || [ "$s" != "0" ]; then
    log "GUARD ABORT ctrl head=$h status_lines=$s"; exit 93
  fi
}

run_arm() {
  local arm="$1" j="$2" dir out
  if [ "$arm" = "ref" ]; then dir="$CTRL"; out="$REF_OUT"; else dir="$MAIN"; out="$CAND_OUT"; fi
  guard
  SLOT=$(( SLOT + 1 ))
  printf '%d\t%d\t%s\t%d\t%s\t%s\n' "$SLOT" "$j" "$arm" "$(( j + 1 ))" "$(date +%s)" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$SLOTS"
  ( cd "$dir" && "$STAGE/$arm/member-$j.bin" --execution "$(( j + 1 ))" --repetitions 1 \
      --target-ms 250 --output "$out" --append ) >> "$LOG" 2>&1
  local rc=$?
  if [ $rc -ne 0 ]; then log "EXECUTION FAILED arm=$arm j=$j rc=$rc"; exit 94; fi
}

for j in $(seq 0 127); do
  if [ $(( j % 2 )) -eq 0 ]; then run_arm ref "$j"; run_arm cand "$j"
  else run_arm cand "$j"; run_arm ref "$j"; fi
done

log "timed phase end"
log "loadavg=$(cat /proc/loadavg)"
log "main_head=$(git -C "$MAIN" rev-parse HEAD) main_status_lines=$(git -C "$MAIN" status --porcelain --untracked-files=all | wc -l)"
log "ctrl_head=$(git -C "$CTRL" rev-parse HEAD) ctrl_status_lines=$(git -C "$CTRL" status --porcelain --untracked-files=all | wc -l)"
