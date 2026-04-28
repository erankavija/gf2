#!/usr/bin/env bash
# ppc-compare.sh — PPC-spiral baseline-vs-current speedup checker.
#
# Reads a kernel→bench mapping from dev/benchmarks/ppc-baselines.json,
# parses criterion estimates.json files for each design size, and reports
# the geomean speedup (baseline_ns / new_ns). Exits 0 iff geomean >= 1.5.
# Most kernels compare a pinned saved baseline against the current `new/`
# estimate. Entries that add a new benchmark leaf after baseline pinning may
# instead set `baseline_bench_target` to compare two current leaves.
#
# Usage:
#   ./dev/benchmarks/ppc-compare.sh <kernel-id> [--manifest path]
#                                               [--criterion-dir path]
#
# Defaults:
#   --manifest        dev/benchmarks/ppc-baselines.json
#   --criterion-dir   target/criterion
#
# Exit codes:
#   0 — geomean speedup >= 1.5x (PASS)
#   1 — geomean speedup <  1.5x (FAIL — kernel below the bar)
#   2 — environment / infrastructure error (missing manifest, missing
#       estimates.json, unknown kernel-id, jq parse failure, ...)
#   3 — baseline name in manifest is still "TBD-..." (b2ecd2ff pending);
#       distinct from FAIL because nothing is wrong with the kernel —
#       the manifest just hasn't been pinned to a real saved baseline yet.
#
# Discovered in jit issue 4f845881 (criterion-1.5x gate scaffolding).

set -euo pipefail

# --- helpers ----------------------------------------------------------------

die_env() {
  echo "ERROR: $*" >&2
  exit 2
}

usage() {
  cat <<'USAGE'
Usage: ppc-compare.sh <kernel-id> [--manifest <path>] [--criterion-dir <path>]

Compares criterion benchmark results for a PPC-spiral kernel against its
pinned baseline. Reads kernel→bench mappings from
dev/benchmarks/ppc-baselines.json (override with --manifest).

Exits 0 iff geomean(baseline_ns / new_ns) across the kernel's design
sizes is >= 1.5x.
USAGE
}

# --- argument parsing -------------------------------------------------------

KERNEL_ID=""
MANIFEST=""
CRITERION_DIR=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    --manifest)
      [[ $# -ge 2 ]] || die_env "--manifest requires a path argument"
      MANIFEST="$2"
      shift 2
      ;;
    --criterion-dir)
      [[ $# -ge 2 ]] || die_env "--criterion-dir requires a path argument"
      CRITERION_DIR="$2"
      shift 2
      ;;
    -*)
      die_env "unknown flag: $1 (try --help)"
      ;;
    *)
      if [[ -n "$KERNEL_ID" ]]; then
        die_env "unexpected positional argument: $1 (kernel-id already set to '$KERNEL_ID')"
      fi
      KERNEL_ID="$1"
      shift
      ;;
  esac
done

if [[ -z "$KERNEL_ID" ]]; then
  usage >&2
  exit 2
fi

MANIFEST="${MANIFEST:-dev/benchmarks/ppc-baselines.json}"
CRITERION_DIR="${CRITERION_DIR:-target/criterion}"

# --- tooling sanity ---------------------------------------------------------

command -v jq >/dev/null 2>&1 || die_env "jq not on PATH (required to parse manifest + criterion estimates)"
command -v awk >/dev/null 2>&1 || die_env "awk not on PATH (required for geomean computation)"

[[ -f "$MANIFEST" ]] || die_env "manifest not found: $MANIFEST"

# --- manifest lookup --------------------------------------------------------

if ! jq -e --arg k "$KERNEL_ID" '.kernels[$k]' "$MANIFEST" >/dev/null 2>&1; then
  available=$(jq -r '.kernels | keys | join(", ")' "$MANIFEST" 2>/dev/null || echo "<unparseable>")
  echo "ERROR: kernel-id '$KERNEL_ID' not found in $MANIFEST; available: $available" >&2
  exit 2
fi

TITLE=$(jq -r --arg k "$KERNEL_ID" '.kernels[$k].title' "$MANIFEST")
BENCH_TARGET=$(jq -r --arg k "$KERNEL_ID" '.kernels[$k].bench_target' "$MANIFEST")
BASELINE_NAME=$(jq -r --arg k "$KERNEL_ID" '.kernels[$k].baseline_name' "$MANIFEST")
BASELINE_BENCH_TARGET=$(jq -r --arg k "$KERNEL_ID" '.kernels[$k].baseline_bench_target // ""' "$MANIFEST")
COMMIT_HASH=$(jq -r --arg k "$KERNEL_ID" '.kernels[$k].commit_hash' "$MANIFEST")
# Read sizes as a newline-delimited list to survive size labels with
# spaces or unusual characters.
mapfile -t SIZES < <(jq -r --arg k "$KERNEL_ID" '.kernels[$k].design_size_class[]' "$MANIFEST")

for required in TITLE BENCH_TARGET BASELINE_NAME COMMIT_HASH; do
  val="${!required}"
  if [[ -z "$val" || "$val" == "null" ]]; then
    die_env "manifest entry for '$KERNEL_ID' missing field: $(echo "$required" | tr '[:upper:]' '[:lower:]')"
  fi
done

if [[ ${#SIZES[@]} -eq 0 ]]; then
  die_env "manifest entry for '$KERNEL_ID' has empty design_size_class"
fi

# Baseline still TBD → exit 3, not 1. This is "infrastructure not ready,"
# not "kernel below the bar."
if [[ "$BASELINE_NAME" == TBD-* ]]; then
  echo "ERROR: kernel-id '$KERNEL_ID' has baseline_name='$BASELINE_NAME' — baseline not pinned yet (see jit:b2ecd2ff)." >&2
  echo "       Once b2ecd2ff replaces 'TBD-*' in $MANIFEST with a real --save-baseline name," >&2
  echo "       this gate will run against pinned criterion data." >&2
  exit 3
fi

# --- per-size measurement ---------------------------------------------------

declare -a SPEEDUPS=()
declare -a TABLE_ROWS=()

for size in "${SIZES[@]}"; do
  if [[ -n "$BASELINE_BENCH_TARGET" ]]; then
    baseline_path="$CRITERION_DIR/$BASELINE_BENCH_TARGET/$size/new/estimates.json"
  else
    baseline_path="$CRITERION_DIR/$BENCH_TARGET/$size/$BASELINE_NAME/estimates.json"
  fi
  new_path="$CRITERION_DIR/$BENCH_TARGET/$size/new/estimates.json"

  if [[ ! -f "$baseline_path" ]]; then
    echo "ERROR: missing baseline estimates: $baseline_path" >&2
    if [[ -n "$BASELINE_BENCH_TARGET" ]]; then
      echo "       Hint: run \`cargo bench\` for both current leaves:" >&2
      echo "             $BASELINE_BENCH_TARGET and $BENCH_TARGET" >&2
    else
      echo "       Hint: jit:b2ecd2ff pins the baseline with" >&2
      echo "             cargo bench --bench $BENCH_TARGET -- --save-baseline $BASELINE_NAME" >&2
    fi
    exit 2
  fi
  if [[ ! -f "$new_path" ]]; then
    echo "ERROR: missing current estimates: $new_path" >&2
    echo "       Hint: run \`cargo bench --bench $BENCH_TARGET\` first to populate target/criterion/<bench>/<size>/new/." >&2
    exit 2
  fi

  baseline_ns=$(jq -r '.median.point_estimate' "$baseline_path")
  new_ns=$(jq -r '.median.point_estimate' "$new_path")

  if [[ -z "$baseline_ns" || "$baseline_ns" == "null" ]]; then
    die_env "could not read .median.point_estimate from $baseline_path"
  fi
  if [[ -z "$new_ns" || "$new_ns" == "null" ]]; then
    die_env "could not read .median.point_estimate from $new_path"
  fi

  # Speedup = baseline / new. >1 means the new code is faster (good).
  speedup=$(awk -v b="$baseline_ns" -v n="$new_ns" 'BEGIN {
    if (n <= 0) { print "0"; exit }
    printf "%.6f", b / n
  }')

  if [[ "$speedup" == "0" ]]; then
    die_env "non-positive new_ns at $new_path; cannot compute speedup"
  fi

  SPEEDUPS+=("$speedup")
  TABLE_ROWS+=("$(awk -v s="$size" -v b="$baseline_ns" -v n="$new_ns" -v sp="$speedup" \
    'BEGIN { printf "  %-10s %12.1f  %12.1f  %7.3fx", s, b, n, sp }')")
done

# --- geomean ---------------------------------------------------------------

# geomean = exp(mean(ln(s_i)))
GEOMEAN=$(awk -v n="${#SPEEDUPS[@]}" 'BEGIN {
  sum_ln = 0
} {
  sum_ln += log($1)
} END {
  if (n == 0) { print "0"; exit }
  printf "%.6f", exp(sum_ln / n)
}' < <(printf '%s\n' "${SPEEDUPS[@]}"))

# --- report ----------------------------------------------------------------

# Comma-separated size list for the header.
sizes_csv=$(IFS=','; printf '%s' "${SIZES[*]}")
sizes_csv=${sizes_csv//,/, }

printf 'PPC compare — kernel %s (%s)\n' "$KERNEL_ID" "$TITLE"
printf '  bench_target:  %s\n' "$BENCH_TARGET"
if [[ -n "$BASELINE_BENCH_TARGET" ]]; then
  printf '  baseline:      current leaf %s @ %s\n' "$BASELINE_BENCH_TARGET" "$COMMIT_HASH"
else
  printf '  baseline:      %s @ %s\n' "$BASELINE_NAME" "$COMMIT_HASH"
fi
printf '  design sizes:  %s\n' "$sizes_csv"
printf '  -----------------------------------------\n'
printf '  %-10s %12s  %12s  %8s\n' "size" "baseline_ns" "new_ns" "speedup"
for row in "${TABLE_ROWS[@]}"; do
  printf '%s\n' "$row"
done
printf '  -----------------------------------------\n'
printf '  geomean speedup: %.3fx   (target >= 1.500x)\n' "$GEOMEAN"

# Final pass/fail decision (awk for portable float comparison).
verdict=$(awk -v g="$GEOMEAN" 'BEGIN { print (g + 0 >= 1.5) ? "PASS" : "FAIL" }')
printf '%s\n' "$verdict"

if [[ "$verdict" == "PASS" ]]; then
  exit 0
else
  exit 1
fi
