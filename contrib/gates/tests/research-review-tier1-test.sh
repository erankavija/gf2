#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT=$(cd "$(dirname "$0")/../../.." && pwd)
GATE="$REPO_ROOT/contrib/gates/research-review.sh"
TEST_ROOT=$(mktemp -d)
trap 'rm -rf "$TEST_ROOT"' EXIT

REFS_FILE="$TEST_ROOT/references.toml"
cat > "$REFS_FILE" <<'EOF'
[[references]]
key = "GGK2025"
citation = "Fixture citation for GGK2025."

[[references]]
key = "HKS2026"
citation = "Fixture citation for HKS2026."
EOF

make_context() {
  local output_file=$1
  local title=$2
  local description=$3
  local labels=$4
  local dependencies=$5
  local prompt=$6
  local gate_description=$7
  local run_history=$8

  jq -n \
    --arg title "$title" \
    --arg description "$description" \
    --argjson labels "$labels" \
    --argjson dependencies "$dependencies" \
    --arg prompt "$prompt" \
    --arg gate_description "$gate_description" \
    --argjson run_history "$run_history" \
    '{
      schema_version: 1,
      prompt: $prompt,
      issue: {
        id: "fixture-issue",
        short_id: "fixture",
        title: $title,
        description: $description,
        labels: $labels,
        dependencies: $dependencies,
        documents: [],
        gates: []
      },
      gate: {
        key: "research-review",
        description: $gate_description
      },
      run_history: $run_history
    }' > "$output_file"
}

assert_output_contains() {
  local output=$1
  local expected=$2
  if ! grep -Fq "$expected" <<< "$output"; then
    printf 'missing output line: %s\n%s\n' "$expected" "$output" >&2
    return 1
  fi
}

run_case() {
  local name=$1
  local expected_status=$2
  local context_file=$TEST_ROOT/$name.json
  shift 2

  "$@" "$context_file"

  local output status
  if output=$(JIT_CONTEXT_FILE="$context_file" \
    RESEARCH_REVIEW_TIER1_ONLY=1 \
    RESEARCH_REVIEW_REFS_FILE="$REFS_FILE" \
    "$GATE" 2>&1); then
    status=0
  else
    status=$?
  fi

  if [ "$status" -ne "$expected_status" ]; then
    printf 'case %s: expected exit %s, got %s\n%s\n' \
      "$name" "$expected_status" "$status" "$output" >&2
    return 1
  fi

  case "$name" in
    dependency_labels_not_attributed)
      assert_output_contains "$output" \
        "Tier 1 (citation integrity): PASS — 0 inline key(s), 0 cites label(s) resolved."
      ;;
    owned_label_text_drift)
      assert_output_contains "$output" \
        "TIER1-F1: label cites:GGK2025 has no matching [GGK2025] citation in the issue text"
      assert_output_contains "$output" "VERDICT: FAIL"
      ;;
    owned_label_cited_ok)
      assert_output_contains "$output" \
        "Tier 1 (citation integrity): PASS — 1 inline key(s), 1 cites label(s) resolved."
      ;;
    owned_label_unresolvable)
      assert_output_contains "$output" \
        "label cites:Nope2099 does not resolve in $REFS_FILE"
      assert_output_contains "$output" "VERDICT: FAIL"
      ;;
    inline_unresolvable_in_issue_text)
      assert_output_contains "$output" \
        "inline citation [Zzz9999] does not resolve in $REFS_FILE"
      ;;
    inline_pollution_ignored)
      assert_output_contains "$output" \
        "Tier 1 (citation integrity): PASS — 0 inline key(s), 0 cites label(s) resolved."
      ;;
    *)
      printf 'unknown test case: %s\n' "$name" >&2
      return 1
      ;;
  esac

  printf 'ok %s\n' "$name"
}

dependency_labels_not_attributed() {
  make_context "$1" \
    "Clean issue" \
    "The issue text contains no citation." \
    '[]' \
    '[{"labels":["cites:GGK2025"]},{"labels":["cites:HKS2026"]}]' \
    "Review prompt mentions [Zzz9999]." \
    "Clean gate description." \
    '[{"stdout":"A prior failure mentioned [GGK2025] and cites:HKS2026."}]'
}

owned_label_text_drift() {
  make_context "$1" \
    "Label drift" \
    "The issue text omits its required source." \
    '["cites:GGK2025"]' \
    '[]' \
    "Clean review prompt." \
    "Clean gate description." \
    '[]'
}

owned_label_cited_ok() {
  make_context "$1" \
    "Cited issue" \
    "The issue is grounded in [GGK2025]." \
    '["cites:GGK2025"]' \
    '[]' \
    "Clean review prompt." \
    "Clean gate description." \
    '[]'
}

owned_label_unresolvable() {
  make_context "$1" \
    "Unknown source" \
    "The issue cites [Nope2099]." \
    '["cites:Nope2099"]' \
    '[]' \
    "Clean review prompt." \
    "Clean gate description." \
    '[]'
}

inline_unresolvable_in_issue_text() {
  make_context "$1" \
    "Unknown inline source" \
    "The issue cites [Zzz9999]." \
    '[]' \
    '[]' \
    "Clean review prompt." \
    "Clean gate description." \
    '[]'
}

inline_pollution_ignored() {
  make_context "$1" \
    "Clean issue" \
    "The issue text is citation-free." \
    '[]' \
    '[]' \
    "Clean review prompt." \
    "Gate description contains [Zzz9999]." \
    '[{"stdout":"A prior failure quoted [Zzz9999]."}]'
}

run_case dependency_labels_not_attributed 0 dependency_labels_not_attributed
run_case owned_label_text_drift 1 owned_label_text_drift
run_case owned_label_cited_ok 0 owned_label_cited_ok
run_case owned_label_unresolvable 1 owned_label_unresolvable
run_case inline_unresolvable_in_issue_text 1 inline_unresolvable_in_issue_text
run_case inline_pollution_ignored 0 inline_pollution_ignored

printf 'All research-review tier-1 tests passed (6 cases).\n'
