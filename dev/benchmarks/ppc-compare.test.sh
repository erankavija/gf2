#!/usr/bin/env bash
# ppc-compare.test.sh — self-contained tests for ppc-compare.sh.
#
# Builds synthetic criterion directories under mktemp(1) so no real
# `cargo bench` data is required. Each test asserts both the exit code
# and (where it matters) a substring in the harness output.
#
# Usage:
#   ./dev/benchmarks/ppc-compare.test.sh
#
# Exit codes:
#   0 — all 8 tests passed
#   1 — at least one test failed (set -e aborts on first failure)

set -euo pipefail

THIS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT="$THIS_DIR/ppc-compare.sh"

[[ -x "$SCRIPT" ]] || { echo "FAIL: $SCRIPT not executable"; exit 1; }

# ---------------------------------------------------------------------------
# Test scaffolding helpers.
# ---------------------------------------------------------------------------

PASS_COUNT=0
TOTAL=8

# Workdir scoped to this whole test process; cleaned on exit.
WORKROOT="$(mktemp -d)"
trap 'rm -rf "$WORKROOT"' EXIT

# write_estimates <path> <median_ns>
# Writes a minimal criterion-style estimates.json with the requested median.
write_estimates() {
  local path="$1"
  local median="$2"
  mkdir -p "$(dirname "$path")"
  cat >"$path" <<EOF
{
  "median": {
    "confidence_interval": {
      "confidence_level": 0.95,
      "lower_bound": $median,
      "upper_bound": $median
    },
    "point_estimate": $median,
    "standard_error": 0.0
  },
  "mean": {
    "point_estimate": $median
  }
}
EOF
}

# write_manifest <path> <kernel_id> <baseline_name> <size1> [size2 ...]
write_manifest() {
  local path="$1" kernel="$2" baseline="$3"
  shift 3
  local sizes_json
  sizes_json=$(printf '"%s",' "$@")
  sizes_json="[${sizes_json%,}]"
  cat >"$path" <<EOF
{
  "schema_version": 1,
  "comment": "synthetic test manifest",
  "kernels": {
    "$kernel": {
      "title": "synthetic kernel",
      "bench_target": "fake_bench",
      "baseline_name": "$baseline",
      "commit_hash": "0000000",
      "design_size_class": $sizes_json
    }
  }
}
EOF
}

# assert_exit <expected> <actual> <test_name>
assert_exit() {
  local expected="$1" actual="$2" name="$3"
  if [[ "$actual" -ne "$expected" ]]; then
    echo "FAIL: $name — expected exit $expected, got $actual"
    return 1
  fi
}

# assert_contains <haystack> <needle> <test_name>
assert_contains() {
  local haystack="$1" needle="$2" name="$3"
  if [[ "$haystack" != *"$needle"* ]]; then
    echo "FAIL: $name — output missing '$needle'"
    echo "----- output -----"
    printf '%s\n' "$haystack"
    echo "------------------"
    return 1
  fi
}

# ---------------------------------------------------------------------------
# T1: same-as-baseline (speedup 1.0x) → exit 1, FAIL.
# ---------------------------------------------------------------------------
t1_same_as_baseline() {
  local name="T1 same-as-baseline (1.0x → FAIL)"
  local dir="$WORKROOT/t1"
  mkdir -p "$dir"
  local manifest="$dir/manifest.json"
  local crit="$dir/criterion"
  write_manifest "$manifest" "K1" "ppc-v0-test" "1024" "4096"
  for sz in 1024 4096; do
    write_estimates "$crit/fake_bench/$sz/ppc-v0-test/estimates.json" 1000.0
    write_estimates "$crit/fake_bench/$sz/new/estimates.json" 1000.0
  done

  set +e
  local out rc
  out=$("$SCRIPT" K1 --manifest "$manifest" --criterion-dir "$crit" 2>&1)
  rc=$?
  set -e

  assert_exit 1 "$rc" "$name" || return 1
  assert_contains "$out" "geomean speedup: 1.000x" "$name" || return 1
  assert_contains "$out" "FAIL" "$name" || return 1
  echo "PASS: $name"
  PASS_COUNT=$((PASS_COUNT + 1))
}

# ---------------------------------------------------------------------------
# T2: 1.6x speedup at every size → exit 0, PASS.
# ---------------------------------------------------------------------------
t2_above_bar() {
  local name="T2 1.6x speedup (PASS)"
  local dir="$WORKROOT/t2"
  mkdir -p "$dir"
  local manifest="$dir/manifest.json"
  local crit="$dir/criterion"
  write_manifest "$manifest" "K1" "ppc-v0-test" "1024" "4096"
  for sz in 1024 4096; do
    write_estimates "$crit/fake_bench/$sz/ppc-v0-test/estimates.json" 1600.0
    write_estimates "$crit/fake_bench/$sz/new/estimates.json" 1000.0
  done

  set +e
  local out rc
  out=$("$SCRIPT" K1 --manifest "$manifest" --criterion-dir "$crit" 2>&1)
  rc=$?
  set -e

  assert_exit 0 "$rc" "$name" || return 1
  assert_contains "$out" "PASS" "$name" || return 1
  assert_contains "$out" "geomean speedup: 1.600x" "$name" || return 1
  echo "PASS: $name"
  PASS_COUNT=$((PASS_COUNT + 1))
}

# ---------------------------------------------------------------------------
# T3: mixed (2.0x and 1.0x → geomean ~1.414x) → exit 1.
# ---------------------------------------------------------------------------
t3_mixed_below_geomean() {
  local name="T3 mixed 2.0x/1.0x → geomean ~1.414x (FAIL)"
  local dir="$WORKROOT/t3"
  mkdir -p "$dir"
  local manifest="$dir/manifest.json"
  local crit="$dir/criterion"
  write_manifest "$manifest" "K1" "ppc-v0-test" "1024" "4096"
  # 1024 → 2.0x; 4096 → 1.0x.
  write_estimates "$crit/fake_bench/1024/ppc-v0-test/estimates.json" 2000.0
  write_estimates "$crit/fake_bench/1024/new/estimates.json" 1000.0
  write_estimates "$crit/fake_bench/4096/ppc-v0-test/estimates.json" 1000.0
  write_estimates "$crit/fake_bench/4096/new/estimates.json" 1000.0

  set +e
  local out rc
  out=$("$SCRIPT" K1 --manifest "$manifest" --criterion-dir "$crit" 2>&1)
  rc=$?
  set -e

  assert_exit 1 "$rc" "$name" || return 1
  assert_contains "$out" "FAIL" "$name" || return 1
  # Expect 1.414x (sqrt(2)) — accept 1.414x or 1.415x rounding.
  if [[ "$out" != *"geomean speedup: 1.414x"* && "$out" != *"geomean speedup: 1.415x"* ]]; then
    echo "FAIL: $name — expected geomean ~1.414x, got:"
    printf '%s\n' "$out"
    return 1
  fi
  echo "PASS: $name"
  PASS_COUNT=$((PASS_COUNT + 1))
}

# ---------------------------------------------------------------------------
# T4: missing kernel-id → exit 2.
# ---------------------------------------------------------------------------
t4_missing_kernel_id() {
  local name="T4 missing kernel-id (exit 2)"
  local dir="$WORKROOT/t4"
  mkdir -p "$dir"
  local manifest="$dir/manifest.json"
  write_manifest "$manifest" "K1" "ppc-v0-test" "1024"

  set +e
  local out rc
  out=$("$SCRIPT" UNKNOWN --manifest "$manifest" --criterion-dir "$dir/criterion" 2>&1)
  rc=$?
  set -e

  assert_exit 2 "$rc" "$name" || return 1
  assert_contains "$out" "kernel-id 'UNKNOWN' not found" "$name" || return 1
  assert_contains "$out" "available:" "$name" || return 1
  echo "PASS: $name"
  PASS_COUNT=$((PASS_COUNT + 1))
}

# ---------------------------------------------------------------------------
# T5: baseline still TBD-... → exit 3.
# ---------------------------------------------------------------------------
t5_baseline_tbd() {
  local name="T5 baseline TBD (exit 3)"
  local dir="$WORKROOT/t5"
  mkdir -p "$dir"
  local manifest="$dir/manifest.json"
  write_manifest "$manifest" "K1" "TBD-b2ecd2ff" "1024"

  set +e
  local out rc
  out=$("$SCRIPT" K1 --manifest "$manifest" --criterion-dir "$dir/criterion" 2>&1)
  rc=$?
  set -e

  assert_exit 3 "$rc" "$name" || return 1
  assert_contains "$out" "baseline not pinned yet" "$name" || return 1
  assert_contains "$out" "b2ecd2ff" "$name" || return 1
  echo "PASS: $name"
  PASS_COUNT=$((PASS_COUNT + 1))
}

# ---------------------------------------------------------------------------
# T6: missing baseline estimates.json → exit 2.
# ---------------------------------------------------------------------------
t6_missing_baseline_estimates() {
  local name="T6 missing baseline estimates.json (exit 2)"
  local dir="$WORKROOT/t6"
  mkdir -p "$dir"
  local manifest="$dir/manifest.json"
  local crit="$dir/criterion"
  write_manifest "$manifest" "K1" "ppc-v0-test" "1024"
  # Only write the new/ side.
  write_estimates "$crit/fake_bench/1024/new/estimates.json" 1000.0

  set +e
  local out rc
  out=$("$SCRIPT" K1 --manifest "$manifest" --criterion-dir "$crit" 2>&1)
  rc=$?
  set -e

  assert_exit 2 "$rc" "$name" || return 1
  assert_contains "$out" "missing baseline estimates" "$name" || return 1
  assert_contains "$out" "b2ecd2ff" "$name" || return 1
  echo "PASS: $name"
  PASS_COUNT=$((PASS_COUNT + 1))
}

# ---------------------------------------------------------------------------
# T7: missing new/estimates.json → exit 2.
# ---------------------------------------------------------------------------
t7_missing_current_estimates() {
  local name="T7 missing current estimates.json (exit 2)"
  local dir="$WORKROOT/t7"
  mkdir -p "$dir"
  local manifest="$dir/manifest.json"
  local crit="$dir/criterion"
  write_manifest "$manifest" "K1" "ppc-v0-test" "1024"
  # Only write the baseline side.
  write_estimates "$crit/fake_bench/1024/ppc-v0-test/estimates.json" 1000.0

  set +e
  local out rc
  out=$("$SCRIPT" K1 --manifest "$manifest" --criterion-dir "$crit" 2>&1)
  rc=$?
  set -e

  assert_exit 2 "$rc" "$name" || return 1
  assert_contains "$out" "missing current estimates" "$name" || return 1
  assert_contains "$out" "cargo bench" "$name" || return 1
  echo "PASS: $name"
  PASS_COUNT=$((PASS_COUNT + 1))
}

# ---------------------------------------------------------------------------
# T8: strictly slower run (speedup 0.8x at every size) → exit 1, FAIL.
#
# Satisfies the literal SC2 wording: "synthetic baseline + slower run" must
# be exercised alongside the same-as-baseline exit-1 case (T1).
# ---------------------------------------------------------------------------
t8_slower_run() {
  local name="T8 slower run (0.8x → FAIL)"
  local dir="$WORKROOT/t8"
  mkdir -p "$dir"
  local manifest="$dir/manifest.json"
  local crit="$dir/criterion"
  write_manifest "$manifest" "K1" "ppc-v0-test" "1024" "4096"
  # baseline 1000ns, new 1250ns → speedup = 1000/1250 = 0.8x at each size.
  for sz in 1024 4096; do
    write_estimates "$crit/fake_bench/$sz/ppc-v0-test/estimates.json" 1000.0
    write_estimates "$crit/fake_bench/$sz/new/estimates.json" 1250.0
  done

  set +e
  local out rc
  out=$("$SCRIPT" K1 --manifest "$manifest" --criterion-dir "$crit" 2>&1)
  rc=$?
  set -e

  assert_exit 1 "$rc" "$name" || return 1
  assert_contains "$out" "FAIL" "$name" || return 1
  assert_contains "$out" "geomean speedup: 0.800x" "$name" || return 1
  # Per-row table must show a sub-1x speedup (e.g., "0.800x").
  assert_contains "$out" "0.800x" "$name (per-row <1x)" || return 1
  echo "PASS: $name"
  PASS_COUNT=$((PASS_COUNT + 1))
}

# ---------------------------------------------------------------------------
# Driver.
# ---------------------------------------------------------------------------

t1_same_as_baseline
t2_above_bar
t3_mixed_below_geomean
t4_missing_kernel_id
t5_baseline_tbd
t6_missing_baseline_estimates
t7_missing_current_estimates
t8_slower_run

echo
echo "$PASS_COUNT/$TOTAL tests passed"

if [[ "$PASS_COUNT" -eq "$TOTAL" ]]; then
  exit 0
else
  exit 1
fi
