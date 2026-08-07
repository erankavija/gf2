#!/usr/bin/env bash
# criterion-1.5x.test.sh — self-contained tests for scripts/criterion-1.5x.sh.
#
# Builds synthetic JIT_CONTEXT_FILE JSONs and a stub harness so no real
# JIT or cargo state is required. Each test asserts both the exit code and
# (where it matters) substrings in the wrapper output.
#
# Usage:
#   ./dev/scripts/criterion-1.5x.test.sh
#
# Exit codes:
#   0 — all tests passed
#   1 — at least one test failed

set -euo pipefail

THIS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$THIS_DIR/../.." && pwd)"
WRAPPER="$REPO_ROOT/scripts/criterion-1.5x.sh"

[[ -x "$WRAPPER" ]] || { echo "FAIL: $WRAPPER not executable"; exit 1; }

# ---------------------------------------------------------------------------
# Test scaffolding helpers.
# ---------------------------------------------------------------------------

PASS_COUNT=0
TOTAL=6

WORKROOT="$(mktemp -d)"
trap 'rm -rf "$WORKROOT"' EXIT

# write_context <path> <labels_json_array>
# Writes a minimal JIT context file with the given labels list.
write_context() {
  local path="$1" labels_json="$2"
  cat >"$path" <<EOF
{
  "id": "00000000-0000-0000-0000-000000000000",
  "title": "synthetic test issue",
  "labels": $labels_json,
  "prompt": ""
}
EOF
}

# write_stub_harness <path> <exit_code> [--echo-arg]
# Writes a stub ppc-compare.sh that records its first arg and exits with
# the requested code. The stub writes the captured kernel-id to its
# sibling file <path>.captured for assertion.
write_stub_harness() {
  local path="$1" rc="$2"
  cat >"$path" <<EOF
#!/usr/bin/env bash
# stub harness — records first arg, exits with $rc
echo "stub-harness called with kernel-id=\$1" >&2
echo -n "\$1" > "${path}.captured"
exit $rc
EOF
  chmod +x "$path"
}

assert_exit() {
  local expected="$1" actual="$2" name="$3"
  if [[ "$actual" -ne "$expected" ]]; then
    echo "FAIL: $name — expected exit $expected, got $actual"
    return 1
  fi
}

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

assert_eq() {
  local expected="$1" actual="$2" name="$3"
  if [[ "$expected" != "$actual" ]]; then
    echo "FAIL: $name — expected '$expected', got '$actual'"
    return 1
  fi
}

# ---------------------------------------------------------------------------
# T1: extracts kernel-id from ppc-kernel:A1 label and forwards to harness.
# ---------------------------------------------------------------------------
t1_label_extract_a1() {
  local name="T1 extract ppc-kernel:A1 + forward exit-0"
  local dir="$WORKROOT/t1"
  mkdir -p "$dir"
  local ctx="$dir/context.json"
  local stub="$dir/stub.sh"
  write_context "$ctx" '["type:task", "ppc-kernel:A1"]'
  write_stub_harness "$stub" 0

  set +e
  local out rc
  out=$(JIT_CONTEXT_FILE="$ctx" PPC_COMPARE_SCRIPT="$stub" "$WRAPPER" 2>&1)
  rc=$?
  set -e

  assert_exit 0 "$rc" "$name" || return 1
  assert_eq "A1" "$(cat "${stub}.captured")" "$name (captured kernel-id)" || return 1
  assert_contains "$out" "kernel-id=A1" "$name" || return 1
  echo "PASS: $name"
  PASS_COUNT=$((PASS_COUNT + 1))
}

# ---------------------------------------------------------------------------
# T2: extracts kernel-id from ppc-kernel:C5 (different position in labels).
# ---------------------------------------------------------------------------
t2_label_extract_c5() {
  local name="T2 extract ppc-kernel:C5 (mid-array)"
  local dir="$WORKROOT/t2"
  mkdir -p "$dir"
  local ctx="$dir/context.json"
  local stub="$dir/stub.sh"
  write_context "$ctx" '["epic:gf2-core-ppc-spiral", "type:task", "ppc-kernel:C5", "tier:c"]'
  write_stub_harness "$stub" 0

  set +e
  local rc
  JIT_CONTEXT_FILE="$ctx" PPC_COMPARE_SCRIPT="$stub" "$WRAPPER" >/dev/null 2>&1
  rc=$?
  set -e

  assert_exit 0 "$rc" "$name" || return 1
  assert_eq "C5" "$(cat "${stub}.captured")" "$name (captured kernel-id)" || return 1
  echo "PASS: $name"
  PASS_COUNT=$((PASS_COUNT + 1))
}

# ---------------------------------------------------------------------------
# T3: missing ppc-kernel:* label → exit 2 with helpful error.
# ---------------------------------------------------------------------------
t3_missing_label() {
  local name="T3 missing ppc-kernel:* label (exit 2)"
  local dir="$WORKROOT/t3"
  mkdir -p "$dir"
  local ctx="$dir/context.json"
  local stub="$dir/stub.sh"
  write_context "$ctx" '["type:task", "epic:gf2-core-ppc-spiral"]'
  write_stub_harness "$stub" 0  # should never be invoked

  set +e
  local out rc
  out=$(JIT_CONTEXT_FILE="$ctx" PPC_COMPARE_SCRIPT="$stub" "$WRAPPER" 2>&1)
  rc=$?
  set -e

  assert_exit 2 "$rc" "$name" || return 1
  assert_contains "$out" "no \`ppc-kernel:<id>\` label" "$name" || return 1
  assert_contains "$out" "criterion-1.5x" "$name" || return 1
  # Stub must NOT have been invoked.
  if [[ -f "${stub}.captured" ]]; then
    echo "FAIL: $name — stub harness was invoked despite missing label"
    return 1
  fi
  echo "PASS: $name"
  PASS_COUNT=$((PASS_COUNT + 1))
}

# ---------------------------------------------------------------------------
# T4: forwards exit-1 (FAIL) from harness unchanged.
# ---------------------------------------------------------------------------
t4_forward_exit_1() {
  local name="T4 forward exit 1 (FAIL) unchanged"
  local dir="$WORKROOT/t4"
  mkdir -p "$dir"
  local ctx="$dir/context.json"
  local stub="$dir/stub.sh"
  write_context "$ctx" '["ppc-kernel:B2"]'
  write_stub_harness "$stub" 1

  set +e
  local rc
  JIT_CONTEXT_FILE="$ctx" PPC_COMPARE_SCRIPT="$stub" "$WRAPPER" >/dev/null 2>&1
  rc=$?
  set -e

  assert_exit 1 "$rc" "$name" || return 1
  assert_eq "B2" "$(cat "${stub}.captured")" "$name (captured kernel-id)" || return 1
  echo "PASS: $name"
  PASS_COUNT=$((PASS_COUNT + 1))
}

# ---------------------------------------------------------------------------
# T5: forwards exit-3 (TBD baseline) from harness unchanged.
# ---------------------------------------------------------------------------
t5_forward_exit_3() {
  local name="T5 forward exit 3 (TBD baseline) unchanged"
  local dir="$WORKROOT/t5"
  mkdir -p "$dir"
  local ctx="$dir/context.json"
  local stub="$dir/stub.sh"
  write_context "$ctx" '["ppc-kernel:D1"]'
  write_stub_harness "$stub" 3

  set +e
  local rc
  JIT_CONTEXT_FILE="$ctx" PPC_COMPARE_SCRIPT="$stub" "$WRAPPER" >/dev/null 2>&1
  rc=$?
  set -e

  assert_exit 3 "$rc" "$name" || return 1
  assert_eq "D1" "$(cat "${stub}.captured")" "$name (captured kernel-id)" || return 1
  echo "PASS: $name"
  PASS_COUNT=$((PASS_COUNT + 1))
}

# ---------------------------------------------------------------------------
# T6: missing JIT_CONTEXT_FILE env var → exit 2 with helpful error.
# ---------------------------------------------------------------------------
t6_missing_context_env() {
  local name="T6 missing JIT_CONTEXT_FILE env (exit 2)"
  local dir="$WORKROOT/t6"
  mkdir -p "$dir"
  local stub="$dir/stub.sh"
  write_stub_harness "$stub" 0

  set +e
  local out rc
  # Explicitly unset to defeat any environmental leakage.
  out=$(env -u JIT_CONTEXT_FILE PPC_COMPARE_SCRIPT="$stub" "$WRAPPER" 2>&1)
  rc=$?
  set -e

  assert_exit 2 "$rc" "$name" || return 1
  assert_contains "$out" "JIT_CONTEXT_FILE not set" "$name" || return 1
  assert_contains "$out" "--pass-context" "$name" || return 1
  echo "PASS: $name"
  PASS_COUNT=$((PASS_COUNT + 1))
}

# ---------------------------------------------------------------------------
# Driver.
# ---------------------------------------------------------------------------

t1_label_extract_a1
t2_label_extract_c5
t3_missing_label
t4_forward_exit_1
t5_forward_exit_3
t6_missing_context_env

echo
echo "$PASS_COUNT/$TOTAL tests passed"

if [[ "$PASS_COUNT" -eq "$TOTAL" ]]; then
  exit 0
else
  exit 1
fi
