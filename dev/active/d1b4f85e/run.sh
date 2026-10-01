#!/usr/bin/env bash
# Measures the BCH workload benches the perf-receipts task (jit:fd9d5416)
# consumes, inside a benchmark window.
#
# Runs, each under dev/scripts/ccx1-bench-flock.sh:
#   1. the baseline-comparable groups of batch_operations.rs and
#      bch_parallel.rs (Criterion filter `bch_`), full host, default features:
#      the configuration of dev/bench_results/88ca7d2f/run.sh;
#   2. bch_encode_w1.rs (workload W1), CCX1 pin, `parallel` feature,
#      RAYON_NUM_THREADS=6;
#   3. bch_genmatrix.rs (workload W2), CCX1 pin, GF2_BENCH=1.
#
# Every BCH benchmark appends its dispatch path to <out-dir>/dispatch.jsonl
# (GF2_BCH_DISPATCH_RECORD; format in crates/gf2-coding/benches/bch_workloads.rs).
# After the runs, the Criterion estimates.json, sample.json and benchmark.json
# of every benchmark this run wrote are copied to <out-dir>/samples/, and the
# run fails if a recorded ID has no fresh Criterion output or a W1 record saw
# a parallel pool other than six threads.
#
# Usage (from the repository root, with GF2_BENCH_WINDOW=1 exported by the
# window runner):
#   dev/active/d1b4f85e/run.sh <out-dir>
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$REPO_ROOT"

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <out-dir>" >&2
  exit 2
fi
OUT_DIR="$1"
ISSUE=d1b4f85e
PARALLEL_WORKERS=6

if [[ -n "$(git status --porcelain)" ]]; then
  echo "ERROR: working tree is dirty; commit or stash before measuring." >&2
  git status --porcelain >&2
  exit 1
fi
if [[ -e "$OUT_DIR" ]]; then
  echo "ERROR: ${OUT_DIR} exists; every run writes a fresh directory." >&2
  exit 1
fi
mkdir -p "$OUT_DIR/samples" "$OUT_DIR/logs"
OUT_DIR="$(cd "$OUT_DIR" && pwd)"
RECORD="${OUT_DIR}/dispatch.jsonl"
HOST_FILE="${OUT_DIR}/host.txt"
REVISION="$(git rev-parse HEAD)"

# Build every configuration before taking the host lock, so the locked runs
# compile nothing.
./scripts/cargo-budget.sh cargo bench -p gf2-coding \
  --bench batch_operations --bench bch_parallel --bench bch_genmatrix --no-run
./scripts/cargo-budget.sh cargo bench -p gf2-coding --features parallel \
  --bench bch_encode_w1 --no-run

RUN_START_EPOCH="$(date +%s)"
MEASUREMENT_START_UTC="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
{
  echo "# command: $0 $*"
  echo "# issue: ${ISSUE}"
  echo "# generated: ${MEASUREMENT_START_UTC}"
  echo "# gf2 revision: ${REVISION}"
  echo "# tree state at run start: clean (git status --porcelain was empty)"
  echo
  echo "## uptime at run start"
  uptime
  echo
  echo "## uname"
  uname -srm
  echo
  echo "## CPU model (/proc/cpuinfo)"
  grep -m1 '^model name' /proc/cpuinfo | sed 's/^model name[[:space:]]*: //'
  echo
  echo "## CPU flags relevant to the encode kernel bundle"
  grep -m1 '^flags' /proc/cpuinfo | tr ' ' '\n' | grep -xE 'avx2|pclmulqdq|vpclmulqdq|bmi2' | sort | tr '\n' ' '
  echo
  echo
  echo "## nproc"
  nproc
  echo
  echo "## CPU frequency governor (cpu0)"
  cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor 2>/dev/null || echo "unavailable"
  echo
  echo "## toolchain"
  rustc -V
  cargo -V
} >"$HOST_FILE"

# run <label> <wrapper-mode> <env...> -- <cargo bench args...>
run() {
  local label="$1" mode="$2"
  shift 2
  local envs=()
  while [[ "$1" != "--" ]]; do
    envs+=("$1")
    shift
  done
  shift
  local log="${OUT_DIR}/logs/${label}-criterion.txt"
  local wrapper=(./dev/scripts/ccx1-bench-flock.sh)
  [[ "$mode" == full-host ]] && wrapper+=(--full-host)
  local invocation=(env CARGO_CI_NO_LOCK=1 "GF2_BCH_DISPATCH_RECORD=${RECORD}" "${envs[@]}"
    "${wrapper[@]}" ./scripts/cargo-budget.sh cargo bench -p gf2-coding "$@")
  {
    echo "# command: ${invocation[*]}"
    echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "# load_avg_start: $(uptime)"
  } >"$log"
  echo "== ${label} ==" >&2
  "${invocation[@]}" 2>&1 | tee -a "$log"
  {
    echo "# finished_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "# load_avg_end: $(uptime)"
  } >>"$log"
}

run batch_operations full-host -- --bench batch_operations -- bch_ --noplot
run bch_parallel full-host -- --bench bch_parallel -- bch_ --noplot
run bch_encode_w1 ccx1 "RAYON_NUM_THREADS=${PARALLEL_WORKERS}" -- \
  --features parallel --bench bch_encode_w1 -- --noplot
run bch_genmatrix ccx1 GF2_BENCH=1 -- --bench bch_genmatrix -- --noplot

{
  echo
  echo "## measurement window"
  echo "start_utc: ${MEASUREMENT_START_UTC}"
  echo "end_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} >>"$HOST_FILE"

# Collect every Criterion output this run wrote, keyed by the full ID each
# benchmark.json records, and check the dispatch record against it.
python3 - "$RUN_START_EPOCH" "$RECORD" "$OUT_DIR/samples" "$PARALLEL_WORKERS" <<'PY'
import json, os, shutil, sys
from pathlib import Path

start, record, samples, workers = float(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]), int(sys.argv[4])
fresh = {}
for bench_json in Path("target/criterion").glob("**/new/benchmark.json"):
    if bench_json.stat().st_mtime < start:
        continue
    full_id = json.loads(bench_json.read_text())["full_id"]
    if full_id in fresh:
        sys.exit(f"ERROR: two fresh Criterion outputs record {full_id}")
    fresh[full_id] = bench_json.parent

lines = [json.loads(line) for line in record.read_text().splitlines() if line.strip()]
ids = [line["id"] for line in lines]
if len(ids) != len(set(ids)):
    sys.exit("ERROR: the dispatch record lists an ID twice")
missing = [i for i in ids if i not in fresh]
if missing:
    sys.exit("ERROR: no fresh Criterion output for: " + ", ".join(missing))
narrow = [l["id"] for l in lines if l.get("workload") == "W1" and l.get("rayon_pool_width") != workers]
if narrow:
    sys.exit(f"ERROR: W1 cells ran on a pool other than {workers} threads: " + ", ".join(narrow))

for full_id, source in sorted(fresh.items()):
    dest = samples / full_id.replace("/", "__")
    dest.mkdir(parents=True)
    for name in ("estimates.json", "sample.json", "benchmark.json"):
        shutil.copy2(source / name, dest / name)
print(f"collected {len(fresh)} Criterion outputs; {len(ids)} carry a dispatch record", file=sys.stderr)
PY

echo "run written: ${OUT_DIR}" >&2
