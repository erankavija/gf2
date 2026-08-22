#!/usr/bin/env bash
# E2 mechanism probe for issue eb9b324c.
# Runs the four staged session-5 member binaries of opposite bit-5 phase under
# perf stat (whole-suite counters) and perf record (symbol-level samples).
# Invoked inside dev/scripts/ccx1-bench-flock.sh; writes only to /tmp scratch.
set -uo pipefail

S=/tmp/gf2-ens5
W=/tmp/gf2-e2
MAIN=/home/vkaskivuo/Projects/gf2
CTRL=$MAIN/.agents/worktrees/control-0c072d73
mkdir -p "$W"
LOG=$W/e2.log
: > "$LOG"

SETA=cycles:u,instructions:u,ex_ret_brn_misp:u,op_cache_hit_miss.op_cache_miss:u,ic_fetch_stall.ic_stall_any:u
SETB=cycles:u,ic_tag_hit_miss.instruction_cache_miss:u,op_cache_hit_miss.all_op_cache_accesses:u,ic_oc_mode_switch.ic_oc_mode_switch:u,ic_fetch_stall.ic_stall_dq_empty:u

log() { echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) $*" >> "$LOG"; }

dir_of() { if [ "$1" = ref ]; then echo "$CTRL"; else echo "$MAIN"; fi; }

log "E2 start"
log "affinity=$(grep Cpus_allowed_list /proc/self/status | tr -d '\t')"
log "niceness=$(ps -o ni= -p $$ | tr -d ' ')"
log "loadavg=$(cat /proc/loadavg)"
log "lock=$(grep ":$(stat -c %i /tmp/gf2-ccx1.lock) " /proc/locks | tr '\n' ';')"
log "governor=$(for c in 6 7 8 9 10 11; do cat /sys/devices/system/cpu/cpu$c/cpufreq/scaling_governor; done | tr '\n' ',')"

stat_run() { # arm j set rep
  local arm=$1 j=$2 set=$3 rep=$4 d ev
  d=$(dir_of "$arm")
  case "$set" in A) ev=$SETA ;; B) ev=$SETB ;; esac
  log "stat arm=$arm j=$j set=$set rep=$rep"
  ( cd "$d" && perf stat -x, -e "$ev" -o "$W/stat-$arm-$j-$set-$rep.csv" \
      "$S/$arm/member-$j.bin" --execution "$((j + 1))" --repetitions 1 --target-ms 250 \
      --output "$W/bench-$arm-$j-$set-$rep.csv" ) >> "$LOG" 2>&1
  log "  rc=$?"
}

rec_run() { # arm j event tag
  local arm=$1 j=$2 ev=$3 tag=$4 d
  d=$(dir_of "$arm")
  log "record arm=$arm j=$j ev=$ev tag=$tag"
  ( cd "$d" && perf record --no-buildid -q -F 9999 -e "$ev" -o "$W/perf-$arm-$j-$tag.data" \
      "$S/$arm/member-$j.bin" --execution "$((j + 1))" --repetitions 1 --target-ms 250 \
      --output "$W/bench-rec-$arm-$j-$tag.csv" ) >> "$LOG" 2>&1
  log "  rc=$?"
}

MEMBERS="cand:1 cand:3 ref:0 ref:1"

for rep in 1 2; do
  for set in A B; do
    for m in $MEMBERS; do
      stat_run "${m%%:*}" "${m##*:}" "$set" "$rep"
    done
  done
done

for m in $MEMBERS; do
  rec_run "${m%%:*}" "${m##*:}" cycles:u cyc
done
for m in $MEMBERS; do
  rec_run "${m%%:*}" "${m##*:}" ex_ret_brn_misp:u misp
done

log "loadavg=$(cat /proc/loadavg)"
log "E2 end"
