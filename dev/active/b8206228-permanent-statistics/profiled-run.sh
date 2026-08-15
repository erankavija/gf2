#!/usr/bin/env bash
# Paired profiled evidence run for JIT 6c7fcb38 (amended REQ-16/REQ-17).
#
# Pass structure: every pass wraps exactly one device-executing workload.
# The harness's grid mode runs each backend in its own process, and several
# profiled processes writing one fixed -o path keep only the last writer's
# data, so each grid pass filters to a single backend and every -o carries
# %pid% so per-process outputs survive regardless of process structure.
# The gray-update and horizontal-product component-isolate modes are
# profiled as their own passes: the micro kernels and their compiler-barrier
# baselines execute only there, never in grid mode.
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

# Grid pass: ONLY selects one (q, n, backend) cell set.
run_grid_pass() {
  local name=$1; shift
  local prof_args=("$@")
  local only=${ONLY:?}
  local dir=$OUT/$name
  mkdir -p "$dir"
  local cmd=("$FLOCK" --full-host "$ROCPROF" "${prof_args[@]}" -f csv -d "$dir" -o "$name-%pid%" -- \
    "$BIN" grid --out "$dir/grid.csv" --only "$only" --execution-id 7002 --skip-machine-warmup)
  echo "COMMAND[$name]: ${cmd[*]}" | tee -a "$LOG"
  local t0=$(date +%s)
  "${cmd[@]}" >> "$LOG" 2>&1
  local rc=$?
  echo "EXIT[$name]: $rc after $(( $(date +%s) - t0 )) s" | tee -a "$LOG"
  return $rc
}

# Component-isolate pass: MODE is gray-update or horizontal-product,
# invoked exactly as the timing campaign invokes it (--q 7).
run_mode_pass() {
  local name=$1; shift
  local prof_args=("$@")
  local mode=${MODE:?}
  local dir=$OUT/$name
  mkdir -p "$dir"
  local cmd=("$FLOCK" --full-host "$ROCPROF" "${prof_args[@]}" -f csv -d "$dir" -o "$name-%pid%" -- \
    "$BIN" "$mode" --out "$dir/$mode.csv" --q 7)
  echo "COMMAND[$name]: ${cmd[*]}" | tee -a "$LOG"
  local t0=$(date +%s)
  "${cmd[@]}" >> "$LOG" 2>&1
  local rc=$?
  echo "EXIT[$name]: $rc after $(( $(date +%s) - t0 )) s" | tee -a "$LOG"
  return $rc
}

PMC=(--pmc SQ_WAVES OccupancyPercent MeanOccupancyPerCU)
TRACE=(--kernel-trace --stats)

# pmc pass with single-counter fallback, mirroring the grid/mode split.
pmc_grid() { # name only
  local name=$1 only=$2
  ONLY=$only run_grid_pass "$name" "${PMC[@]}" \
    || { echo "retry $name with OccupancyPercent only" | tee -a "$LOG"; ONLY=$only run_grid_pass "$name-occ" --pmc OccupancyPercent; }
}
pmc_mode() { # name mode
  local name=$1 mode=$2
  MODE=$mode run_mode_pass "$name" "${PMC[@]}" \
    || { echo "retry $name with OccupancyPercent only" | tee -a "$LOG"; MODE=$mode run_mode_pass "$name-occ" --pmc OccupancyPercent; }
}

fail=0
# Kernel-trace passes: per-kernel durations (REQ-16 preparation vs Gray walk)
ONLY="q=7,n=12,backend=gpu_hip"                    run_grid_pass trace-n12-gpuhip     "${TRACE[@]}" || fail=1
ONLY="q=7,n=12,backend=f7-lookup-table-control"    run_grid_pass trace-n12-lookup     "${TRACE[@]}" || fail=1
ONLY="q=7,n=12,backend=f7-three-plane-permanent"   run_grid_pass trace-n12-threeplane "${TRACE[@]}" || fail=1
ONLY="q=7,n=16,backend=f7-three-plane-permanent"   run_grid_pass trace-n16-threeplane "${TRACE[@]}" || fail=1
ONLY="q=7,n=20,backend=gpu_hip"                    run_grid_pass trace-n20-gpuhip     "${TRACE[@]}" || fail=1
ONLY="q=7,n=20,backend=f7-lookup-table-control"    run_grid_pass trace-n20-lookup     "${TRACE[@]}" || fail=1
ONLY="q=7,n=20,backend=f7-three-plane-permanent"   run_grid_pass trace-n20-threeplane "${TRACE[@]}" || fail=1
ONLY="q=7,n=24,backend=f7-three-plane-permanent"   run_grid_pass trace-n24-threeplane "${TRACE[@]}" || fail=1
MODE=gray-update        run_mode_pass trace-grayupdate "${TRACE[@]}" || fail=1
MODE=horizontal-product run_mode_pass trace-horizprod  "${TRACE[@]}" || fail=1

# Counter passes: achieved occupancy (REQ-17) for every measured F_7 kernel
pmc_grid pmc-n12-gpuhip     "q=7,n=12,backend=gpu_hip"                  || fail=1
pmc_grid pmc-n12-lookup     "q=7,n=12,backend=f7-lookup-table-control"  || fail=1
pmc_grid pmc-n12-threeplane "q=7,n=12,backend=f7-three-plane-permanent" || fail=1
pmc_grid pmc-n20-gpuhip     "q=7,n=20,backend=gpu_hip"                  || fail=1
pmc_grid pmc-n20-lookup     "q=7,n=20,backend=f7-lookup-table-control"  || fail=1
pmc_grid pmc-n20-threeplane "q=7,n=20,backend=f7-three-plane-permanent" || fail=1
pmc_mode pmc-grayupdate gray-update        || fail=1
pmc_mode pmc-horizprod  horizontal-product || fail=1

# Round-2 counter passes: the full-pass derived counters above drift with
# cumulative dispatch count (rocprofv3's accumulate/max windows), so achieved
# occupancy is read from early dispatch iterations only, where the derived
# values are exact. OccupancyPercent reads zero under iteration-range
# collection on this stack, so round 2 records MeanOccupancyPerCU beside
# SQ_WAVES; admissibility is the physical bound
# 0 < MeanOccupancyPerCU <= min(32, SQ_WAVES/80) per collected dispatch.
PMC2=(--pmc SQ_WAVES MeanOccupancyPerCU --kernel-iteration-range '[1-8]')
ONLY="q=7,n=12,backend=gpu_hip"                  run_grid_pass pmc2-n12-gpuhip     "${PMC2[@]}" || fail=1
ONLY="q=7,n=12,backend=f7-lookup-table-control"  run_grid_pass pmc2-n12-lookup     "${PMC2[@]}" || fail=1
ONLY="q=7,n=12,backend=f7-three-plane-permanent" run_grid_pass pmc2-n12-threeplane "${PMC2[@]}" || fail=1
ONLY="q=7,n=20,backend=gpu_hip"                  run_grid_pass pmc2-n20-gpuhip     "${PMC2[@]}" || fail=1
ONLY="q=7,n=20,backend=f7-lookup-table-control"  run_grid_pass pmc2-n20-lookup     "${PMC2[@]}" || fail=1
ONLY="q=7,n=20,backend=f7-three-plane-permanent" run_grid_pass pmc2-n20-threeplane "${PMC2[@]}" || fail=1
MODE=gray-update        run_mode_pass pmc2-grayupdate "${PMC2[@]}" || fail=1
MODE=horizontal-product run_mode_pass pmc2-horizprod  "${PMC2[@]}" || fail=1
ONLY="q=7,n=16,backend=f7-three-plane-permanent" run_grid_pass pmc2-n16-threeplane "${PMC2[@]}" || fail=1

# Profiler capability probes: rocprofv2 and legacy rocprof abort this
# workload under their interception (device batch failure in the candidate
# worker). The recorded failures ground the receipt's statement that
# rocprofv3 is the one profiler on this host that both runs the workload
# and produces counters; a probe unexpectedly SUCCEEDING here invalidates
# that statement and must be investigated, so success is the failure mode.
V2PMC=$OUT/v2-pmc.txt
printf 'pmc: SQ_WAVES MeanOccupancyPerCU\n' > "$V2PMC"
probe() {
  local name=$1 tool=$2
  local dir=$OUT/$name; mkdir -p "$dir"
  local cmd=("$FLOCK" --full-host "$tool" -i "$V2PMC" -d "$dir" -o "$dir/results.csv" \
    "$BIN" grid --out "$dir/grid.csv" --only q=7,n=20,backend=f7-three-plane-permanent --execution-id 7002 --skip-machine-warmup)
  echo "COMMAND[$name]: ${cmd[*]}" | tee -a "$LOG"
  local t0=$(date +%s); "${cmd[@]}" >> "$LOG" 2>&1; local rc=$?
  echo "EXIT[$name]: $rc after $(( $(date +%s) - t0 )) s" | tee -a "$LOG"
  if [ $rc -eq 0 ]; then echo "PROBE-UNEXPECTED-SUCCESS[$name]" | tee -a "$LOG"; fail=1; fi
}
probe rocprofv2-n20-threeplane /opt/rocm/bin/rocprofv2
probe rocprofv1-n20-threeplane /opt/rocm/bin/rocprof
rm -f "$V2PMC"

echo "overall_fail: $fail" | tee -a "$LOG"

# Packaging: per-dispatch counter and trace CSVs are too large to commit.
# Compress them, move them to the local (uncommitted) retention directory
# under target/, and aggregate the counters into the committed
# counters-aggregate.csv via the aggregation script committed beside the
# artifact. Only aggregates, rocprofv3's own kernel_stats, workload CSVs,
# run.log, and provenance are committed.
if [ "$fail" -eq 0 ]; then
  RAW=$REPO/target/permanent-campaign/profiled-raw-$STAMP
  mkdir -p "$RAW"
  ( cd "$OUT" && find . \( -name '*_counter_collection.csv' -o -name '*_kernel_trace.csv' \) -exec zstd -q -19 -T0 --rm {} + \
    && find . -name '*.zst' | while read -r f; do mkdir -p "$RAW/$(dirname "$f")"; mv "$f" "$RAW/$f"; done )
  cp "$REPO/dev/studies/6c7fcb38/profiled-20260815T181923Z/aggregate-counters.py" "$OUT/" 2>/dev/null || true
  python3 "$OUT/aggregate-counters.py" "$RAW" | tee -a "$LOG"
  echo "raw_retention: $RAW" | tee -a "$LOG"
fi
echo "$OUT"
exit $fail
