#!/usr/bin/env bash
set -euo pipefail

# Fail if any CLAUDE.md in the repo exceeds 200 lines.
# Excludes .git/ and .claude/worktrees/ (ephemeral agent scratch).

LIMIT=200
failed=0

while IFS= read -r file; do
  lines=$(wc -l < "$file")
  if [ "$lines" -gt "$LIMIT" ]; then
    echo "FAIL: $file has $lines lines (limit: $LIMIT)"
    failed=1
  fi
done < <(find . -name "CLAUDE.md" \
           -not -path "./.git/*" \
           -not -path "./.claude/worktrees/*")

if [ "$failed" -ne 0 ]; then
  exit 1
fi
