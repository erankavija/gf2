#!/usr/bin/env bash
# Paired profiled evidence run for JIT 6c7fcb38 (amended REQ-16/REQ-17).
set -uo pipefail
REPO=/home/vkaskivuo/Projects/gf2
BIN=$REPO/target/permanent-campaign/permanent-sampling-feas-hip/release/permanent_sampling_feas
MANIFEST=$REPO/target/permanent-campaign/manifest-v1.txt
STAMP=$(date -u +%Y%m%dT%H%M%SZ)
OUT=$REPO/dev/studies/6c7fcb38/profiled-$STAMP
FLOCK=$REPO/dev/scripts/ccx1-bench-flock.sh
ROCPROF=/opt/rocm/bin/rocprofv3
mkdir -p "$OUT"
LOG=$OUT/run.log
echo "stamp: $STAMP" | tee "$LOG"

HASH=$(sha256sum "$BIN" | awk '{print $1}')
if ! grep -q "$HASH" "$MANIFEST"; then
  echo "FATAL: binary hash $HASH not in manifest" | tee -a "$LOG"; exit 2
fi
echo "binary_sha256: $HASH (matches manifest-v1.txt)" | tee -a "$LOG"

run_pass() {
  local name=$1; shift
  local prof_args=("$@")
  local only=${ONLY:?}
  local dir=$OUT/$name
  mkdir -p "$dir"
  local cmd=("$FLOCK" --full-host "$ROCPROF" "${prof_args[@]}" -f csv -d "$dir" -o "$name" -- \
    "$BIN" grid --out "$dir/grid.csv" --only "$only" --execution-id 7002 --skip-machine-warmup)
  echo "COMMAND[$name]: ${cmd[*]}" | tee -a "$LOG"
  local t0=$(date +%s)
  "${cmd[@]}" >> "$LOG" 2>&1
  local rc=$?
  echo "EXIT[$name]: $rc after $(( $(date +%s) - t0 )) s" | tee -a "$LOG"
  return $rc
}

fail=0
ONLY="q=7,n=12" run_pass trace-n12 --kernel-trace --stats || fail=1
ONLY="q=7,n=12" run_pass pmc-n12 --pmc SQ_WAVES OccupancyPercent MeanOccupancyPerCU \
  || { echo "retry pmc-n12 with OccupancyPercent only" | tee -a "$LOG"; ONLY="q=7,n=12" run_pass pmc-n12-occ --pmc OccupancyPercent || fail=1; }
ONLY="q=7,n=16,backend=f7-three-plane-permanent" run_pass trace-n16-threeplane --kernel-trace --stats || fail=1
ONLY="q=7,n=20" run_pass trace-n20 --kernel-trace --stats || fail=1
ONLY="q=7,n=20" run_pass pmc-n20 --pmc SQ_WAVES OccupancyPercent MeanOccupancyPerCU \
  || { echo "retry pmc-n20 with OccupancyPercent only" | tee -a "$LOG"; ONLY="q=7,n=20" run_pass pmc-n20-occ --pmc OccupancyPercent || fail=1; }
ONLY="q=7,n=24,backend=f7-three-plane-permanent" run_pass trace-n24-threeplane --kernel-trace --stats || fail=1

echo "overall_fail: $fail" | tee -a "$LOG"
echo "$OUT"
exit $fail
