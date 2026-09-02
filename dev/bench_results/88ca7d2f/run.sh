#!/usr/bin/env bash
# Pins the pre-cutover BCH Criterion baseline (jit:88ca7d2f).
#
# Runs the legacy-BCH benchmark groups in `bch_parallel.rs` and
# `batch_operations.rs` under the CCX1 full-host lock, captures host and
# toolchain facts at run time, copies each measured benchmark's Criterion
# JSON output, and renders the receipt from those files only.
#
# Usage (from the repository root):
#   dev/bench_results/88ca7d2f/run.sh
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$REPO_ROOT"

ISSUE=88ca7d2f
OUT_DIR="dev/bench_results/${ISSUE}"
SAMPLES_DIR="${OUT_DIR}/samples"
DATE_UTC="$(date -u +%Y-%m-%d)"
HOST_FILE="${OUT_DIR}/${DATE_UTC}-${ISSUE}-host.txt"
RECEIPT_FILE="${OUT_DIR}/${DATE_UTC}-${ISSUE}-precutover-bch-baseline.md"

# Bench target -> criterion filter regex. Both files' non-BCH groups (the two
# `ldpc_*` groups in batch_operations.rs) are excluded by the filter so only
# legacy-BCH groups are measured and copied.
BENCH_TARGETS=(bch_parallel batch_operations)
BENCH_FILTER='bch_'

if [[ -n "$(git status --porcelain)" ]]; then
  echo "ERROR: working tree is dirty; commit or stash before running the baseline." >&2
  git status --porcelain >&2
  exit 1
fi

REVISION="$(git rev-parse HEAD)"

mkdir -p "$SAMPLES_DIR"

MEASUREMENT_START_UTC="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

{
  echo "# command: $0"
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

for bench in "${BENCH_TARGETS[@]}"; do
  LOG_FILE="${OUT_DIR}/${DATE_UTC}-${ISSUE}-${bench}-criterion.txt"
  INVOCATION="CARGO_CI_NO_LOCK=1 ./dev/scripts/ccx1-bench-flock.sh --full-host ./scripts/cargo-budget.sh cargo bench -p gf2-coding --bench ${bench} -- ${BENCH_FILTER}"
  {
    echo "# command: ${INVOCATION}"
    echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "# load_avg_start: $(uptime)"
  } >"$LOG_FILE"
  echo "== running bench ${bench} (filter: ${BENCH_FILTER}) ==" >&2
  CARGO_CI_NO_LOCK=1 ./dev/scripts/ccx1-bench-flock.sh --full-host \
    ./scripts/cargo-budget.sh cargo bench -p gf2-coding --bench "$bench" -- "$BENCH_FILTER" \
    | tee -a "$LOG_FILE"
  {
    echo "# finished_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "# load_avg_end: $(uptime)"
  } >>"$LOG_FILE"
done

MEASUREMENT_END_UTC="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
{
  echo
  echo "## measurement window"
  echo "start_utc: ${MEASUREMENT_START_UTC}"
  echo "end_utc: ${MEASUREMENT_END_UTC}"
} >>"$HOST_FILE"

# Copy each measured benchmark's Criterion JSON output. Criterion's directory
# layout mirrors the benchmark id's slashes (target/criterion/<group>/<param>
# for a parameterized id, target/criterion/<group>/<function> otherwise), so
# walk for benchmark.json under a `new/` leaf and sanitize the id for the
# destination directory name.
find target/criterion -mindepth 1 -type f -name benchmark.json -path '*/new/benchmark.json' -print0 |
  while IFS= read -r -d '' bench_json; do
    src_dir="$(dirname "$bench_json")"
    full_id="$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['full_id'])" "$bench_json")"
    case "$full_id" in
      bch_*)
        sanitized="${full_id//\//_}"
        dest_dir="${SAMPLES_DIR}/${sanitized}"
        mkdir -p "$dest_dir"
        cp "$src_dir/estimates.json" "$src_dir/sample.json" "$src_dir/benchmark.json" "$dest_dir/"
        ;;
      *)
        ;;
    esac
  done

python3 "${OUT_DIR}/render_receipt.py" "$OUT_DIR" "$ISSUE" "$DATE_UTC" >"$RECEIPT_FILE"

echo "receipt written: ${RECEIPT_FILE}" >&2
