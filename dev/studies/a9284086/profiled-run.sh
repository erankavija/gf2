#!/usr/bin/env bash
# Paired profiled evidence run for JIT a9284086 — runtime qualification of the
# retained F_3 and F_5 GPU permanent paths.
#
# The F_7 kernels already carry committed occupancy counters
# (dev/studies/6c7fcb38/profiled-20260815T181923Z/). This run covers the other
# two fields' retained device paths on the same hash-pinned binary, under the
# same pass discipline:
#
#   * every pass wraps exactly one device-executing workload, because the
#     harness's grid mode runs each backend in its own process and several
#     profiled processes writing one fixed -o path keep only the last writer's
#     data;
#   * every -o carries %pid% so per-process outputs survive regardless of
#     process structure;
#   * grid passes reuse each field's committed --execution-id (3002 for q=3,
#     5002 for q=5) so the profiled cells draw the same preregistered matrices
#     as the timing run 20260814T230032Z-2085453;
#   * counters are collected over early dispatch iterations only. The F_7 run
#     established that rocprofv3's full-pass derived occupancy counters divide
#     by a GRBM_GUI_ACTIVE window that drifts with cumulative dispatch count
#     (dev/studies/6c7fcb38/receipts.md section 13), so this run collects the
#     round-2 counter set from the start and never the full-pass one.
#     OccupancyPercent reads zero under iteration-range collection on this
#     stack and is omitted.
#
# Admissibility of a counter reading is the policy committed in
# dev/studies/6c7fcb38/receipts.md section 13: MeanOccupancyPerCU inside
# (0, 32], and resident waves no more than the dispatch's own
# Grid_Size/Workgroup_Size launch geometry, with one-percent rounding slack.
# SQ_WAVES is recorded but is not an admissibility test.
#
# The profiler capability probes are not repeated here: the committed F_7
# provenance records rocprofv2 and legacy rocprof aborting this workload under
# their interception on this host.
set -uo pipefail
REPO=/home/vkaskivuo/Projects/gf2
BIN=$REPO/target/permanent-campaign/permanent-sampling-feas-hip/release/permanent_sampling_feas
MANIFEST=$REPO/target/permanent-campaign/manifest-v1.txt
STAMP=$(date -u +%Y%m%dT%H%M%SZ)
OUT=$REPO/dev/studies/a9284086/profiled-$STAMP
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
echo "git_revision: $(git -C "$REPO" rev-parse HEAD)" | tee -a "$LOG"
"$ROCPROF" --version 2>&1 | sed 's/^/rocprofv3 /' | tee -a "$LOG"

# Grid pass: ONLY selects one (q, n, backend) cell set; EXEC_ID is that
# field's committed grid execution id.
run_grid_pass() {
  local name=$1; shift
  local prof_args=("$@")
  local only=${ONLY:?}
  local exec_id=${EXEC_ID:?}
  local dir=$OUT/$name
  mkdir -p "$dir"
  local cmd=("$FLOCK" --full-host "$ROCPROF" "${prof_args[@]}" -f csv -d "$dir" -o "$name-%pid%" -- \
    "$BIN" grid --out "$dir/grid.csv" --only "$only" --execution-id "$exec_id" --skip-machine-warmup)
  echo "COMMAND[$name]: ${cmd[*]}" | tee -a "$LOG"
  local t0=$(date +%s)
  "${cmd[@]}" >> "$LOG" 2>&1
  local rc=$?
  echo "EXIT[$name]: $rc after $(( $(date +%s) - t0 )) s" | tee -a "$LOG"
  return $rc
}

TRACE=(--kernel-trace --stats)
PMC=(--pmc SQ_WAVES MeanOccupancyPerCU --kernel-iteration-range '[1-8]')

fail=0

# Cells: each field's retained device paths at three orders — the two orders
# the F_7 counter evidence uses (12 and 20) plus that field's declared
# operating point from
# dev/simulation_results/permanent-zero-fraction/protocol.md:51-54
# (n=28 for q=3, n=24 for q=5). The q=5, n=24 gpu_hip cells are censored before
# running in the timing grid, so that cell would dispatch nothing and is not a
# pass.
CELLS=(
  "q3-n12-gpuhip     3002 q=3,n=12,backend=gpu_hip"
  "q3-n12-wavegf3    3002 q=3,n=12,backend=wave-gf3"
  "q3-n12-foldgf3    3002 q=3,n=12,backend=fold-gf3"
  "q3-n20-gpuhip     3002 q=3,n=20,backend=gpu_hip"
  "q3-n20-wavegf3    3002 q=3,n=20,backend=wave-gf3"
  "q3-n20-foldgf3    3002 q=3,n=20,backend=fold-gf3"
  "q3-n28-gpuhip     3002 q=3,n=28,backend=gpu_hip"
  "q3-n28-wavegf3    3002 q=3,n=28,backend=wave-gf3"
  "q3-n28-foldgf3    3002 q=3,n=28,backend=fold-gf3"
  "q5-n12-gpuhip     5002 q=5,n=12,backend=gpu_hip"
  "q5-n12-bytectl    5002 q=5,n=12,backend=f5-byte-control"
  "q5-n12-threeplane 5002 q=5,n=12,backend=f5-three-plane"
  "q5-n20-gpuhip     5002 q=5,n=20,backend=gpu_hip"
  "q5-n20-bytectl    5002 q=5,n=20,backend=f5-byte-control"
  "q5-n20-threeplane 5002 q=5,n=20,backend=f5-three-plane"
  "q5-n24-bytectl    5002 q=5,n=24,backend=f5-byte-control"
  "q5-n24-threeplane 5002 q=5,n=24,backend=f5-three-plane"
)

for cell in "${CELLS[@]}"; do
  read -r label exec_id only <<<"$cell"
  ONLY="$only" EXEC_ID="$exec_id" run_grid_pass "trace-$label" "${TRACE[@]}" || fail=1
done

for cell in "${CELLS[@]}"; do
  read -r label exec_id only <<<"$cell"
  ONLY="$only" EXEC_ID="$exec_id" run_grid_pass "pmc-$label" "${PMC[@]}" || fail=1
done

echo "overall_fail: $fail" | tee -a "$LOG"

# Packaging: the per-dispatch kernel_trace CSVs are far too large to commit, so
# they are zstd-compressed and moved to the local (uncommitted) retention
# directory under target/. rocprofv3's own kernel_stats/domain_stats
# aggregations, the iteration-range counter CSVs (small enough to commit in
# full), the agent reports, the workload CSVs, and run.log stay in place.
if [ "$fail" -eq 0 ]; then
  RAW=$REPO/target/permanent-campaign/profiled-raw-a9284086-$STAMP
  mkdir -p "$RAW"
  ( cd "$OUT" && find . -name '*_kernel_trace.csv' -exec zstd -q -19 -T0 --rm {} + \
    && find . -name '*.zst' | while read -r f; do mkdir -p "$RAW/$(dirname "$f")"; mv "$f" "$RAW/$f"; done )
  echo "raw_retention: $RAW" | tee -a "$LOG"
fi
echo "$OUT"
exit $fail
