#!/usr/bin/env bash
set -euo pipefail

# Research-review gate checker: deterministic citation-integrity checks (tier 1)
# followed by the AI methodology review (tier 2, delegated to ai-review.sh).
#
# Contract (same as every exec gate checker):
#   exit 0 — gate passed; exit 1 — gate failed; stdout — report text.
#
# Tier 1 parses the gate context JSON with jq and reads the citation registry.
# It scopes inline citekeys and cites:<key> labels to the issue under review:
#   1. Every inline citekey token [AuthorYYYY] resolves in .jit/references.toml.
#   2. Every issue-owned cites:<key> label resolves in the registry.
#   3. Every issue-owned cites:<key> label's key is also mentioned in the issue
#      text (label/text drift check).
# Any tier-1 finding fails the gate without spending an AI review.

if [ -z "${JIT_CONTEXT_FILE:-}" ] || [ ! -f "${JIT_CONTEXT_FILE:-}" ]; then
  echo "ERROR: JIT_CONTEXT_FILE not set or missing. This gate requires --pass-context." >&2
  exit 1
fi

REFS_FILE="${RESEARCH_REVIEW_REFS_FILE:-.jit/references.toml}"
if [ ! -f "$REFS_FILE" ]; then
  echo "ERROR: $REFS_FILE not found; the research-review gate requires the citation registry." >&2
  exit 1
fi

KEY_PATTERN='[A-Z][A-Za-z]*[0-9]{4}[a-z]?'
registry_keys=$(grep -E '^key = "' "$REFS_FILE" | sed 's/^key = "\(.*\)"/\1/' || true)

issue_text=$(jq -r '
  [(.issue.title // ""), (.issue.description // "")]
  | map(if type == "string" then . else tostring end)
  | join("\n")
' "$JIT_CONTEXT_FILE")

label_keys=$(jq -r --arg key_pattern "$KEY_PATTERN" '
  (.issue.labels // [])[]
  | select(type == "string")
  | select(test("^cites:" + $key_pattern + "$"))
  | sub("^cites:"; "")
' "$JIT_CONTEXT_FILE" | sort -u || true)

has_key() {
  printf '%s\n' "$registry_keys" | grep -qx "$1"
}

failures=0
report() {
  failures=$((failures + 1))
  echo "TIER1-F${failures}: $1"
}

# 1. Inline citekey tokens must resolve in the registry.
inline_keys=$(printf '%s\n' "$issue_text" | grep -oE "\[${KEY_PATTERN}\]" | tr -d '[]' | sort -u || true)
for k in $inline_keys; do
  has_key "$k" || report "inline citation [$k] does not resolve in $REFS_FILE"
done

# 2 + 3. cites: labels must resolve and be mentioned in the issue text.
for k in $label_keys; do
  has_key "$k" || report "label cites:$k does not resolve in $REFS_FILE"
  grep -qF "[$k]" <<< "$issue_text" || report "label cites:$k has no matching [$k] citation in the issue text"
done

if [ "$failures" -gt 0 ]; then
  echo "Total tier-1 findings: $failures"
  echo "VERDICT: FAIL"
  exit 1
fi

inline_count=$(printf '%s' "$inline_keys" | grep -c . || true)
label_count=$(printf '%s' "$label_keys" | grep -c . || true)
echo "Tier 1 (citation integrity): PASS — ${inline_count} inline key(s), ${label_count} cites label(s) resolved."
if [ "${RESEARCH_REVIEW_TIER1_ONLY:-0}" = "1" ]; then
  exit 0
fi
echo "--- tier 2: AI methodology review ---"
exec "$(dirname "$0")/ai-review.sh"
