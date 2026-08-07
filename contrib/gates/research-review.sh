#!/usr/bin/env bash
set -euo pipefail

# Research-review gate checker: deterministic citation-integrity checks (tier 1)
# followed by the AI methodology review (tier 2, delegated to ai-review.sh).
#
# Contract (same as every exec gate checker):
#   exit 0 — gate passed; exit 1 — gate failed; stdout — report text.
#
# Tier 1 operates only on the gate context file and .jit/references.toml, so it
# is deterministic and needs no JSON parser:
#   1. Every inline citekey token [AuthorYYYY] resolves in .jit/references.toml.
#   2. Every cites:<key> label resolves in .jit/references.toml.
#   3. Every cites:<key> label's key is also mentioned in the issue text
#      (label/text drift check).
# Any tier-1 finding fails the gate without spending an AI review.

if [ -z "${JIT_CONTEXT_FILE:-}" ] || [ ! -f "${JIT_CONTEXT_FILE:-}" ]; then
  echo "ERROR: JIT_CONTEXT_FILE not set or missing. This gate requires --pass-context." >&2
  exit 1
fi

REFS_FILE=".jit/references.toml"
if [ ! -f "$REFS_FILE" ]; then
  echo "ERROR: $REFS_FILE not found; the research-review gate requires the citation registry." >&2
  exit 1
fi

KEY_PATTERN='[A-Z][A-Za-z]*[0-9]{4}[a-z]?'
registry_keys=$(grep -E '^key = "' "$REFS_FILE" | sed 's/^key = "\(.*\)"/\1/')

has_key() {
  printf '%s\n' "$registry_keys" | grep -qx "$1"
}

failures=0
report() {
  failures=$((failures + 1))
  echo "TIER1-F${failures}: $1"
}

# 1. Inline citekey tokens must resolve in the registry.
inline_keys=$(grep -oE "\[${KEY_PATTERN}\]" "$JIT_CONTEXT_FILE" | tr -d '[]' | sort -u || true)
for k in $inline_keys; do
  has_key "$k" || report "inline citation [$k] does not resolve in $REFS_FILE"
done

# 2 + 3. cites: labels must resolve and be mentioned in the issue text.
label_keys=$(grep -oE "cites:${KEY_PATTERN}" "$JIT_CONTEXT_FILE" | sed 's/^cites://' | sort -u || true)
for k in $label_keys; do
  has_key "$k" || report "label cites:$k does not resolve in $REFS_FILE"
  grep -qE "\[$k\]" "$JIT_CONTEXT_FILE" || report "label cites:$k has no matching [$k] citation in the issue text"
done

if [ "$failures" -gt 0 ]; then
  echo "Total tier-1 findings: $failures"
  echo "VERDICT: FAIL"
  exit 1
fi

inline_count=$(printf '%s' "$inline_keys" | grep -c . || true)
label_count=$(printf '%s' "$label_keys" | grep -c . || true)
echo "Tier 1 (citation integrity): PASS — ${inline_count} inline key(s), ${label_count} cites label(s) resolved."
echo "--- tier 2: AI methodology review ---"
exec "$(dirname "$0")/ai-review.sh"
