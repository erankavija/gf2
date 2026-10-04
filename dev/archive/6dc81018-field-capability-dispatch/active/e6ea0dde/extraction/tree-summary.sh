#!/usr/bin/env bash
# Re-derive excerpts/tree-summary.txt for JIT issue e6ea0dde.
#
# For every generated Lean tree in this record: the definitions Aeneas emitted
# into Funs.lean, its `sorry` count, and the record-field names that occur more
# than once inside one generated `structure` in Types.lean. The last column is
# the one that decides verbatim elaboration: Lean rejects a structure whose
# field names repeat.
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
{
  echo "# Generated-tree summary for JIT issue e6ea0dde."
  echo "# Reproduce with: dev/active/e6ea0dde/extraction/tree-summary.sh"
  echo "# Captured: $(date -Is)"
  for d in "$HERE"/A*_lean; do
    [ -d "$d" ] || continue
    echo
    echo "## $(basename "$d")"
    for f in Types.lean Funs.lean FunsExternal_Template.lean; do
      [ -f "$d/$f" ] || echo "- $f: not written (Aeneas exited first)"
    done
    echo "- structures emitted in Types.lean: $(grep -c '^structure ' "$d/Types.lean" 2>/dev/null || echo 0)"
    echo "- Funs.lean definitions:"
    grep -h '^def ' "$d/Funs.lean" 2>/dev/null | sed 's/^def /    /'
    echo "- sorry occurrences: Funs.lean $(grep -c sorry "$d/Funs.lean" 2>/dev/null || echo 0), Types.lean $(grep -c sorry "$d/Types.lean" 2>/dev/null || echo 0)"
    echo "- repeated structure field names in Types.lean:"
    dups="$(grep -h "Inst :" "$d/Types.lean" 2>/dev/null | awk '{print $1}' | sort | uniq -d)"
    if [ -z "$dups" ]; then echo "    (none)"; else echo "$dups" | sed 's/^/    /'; fi
  done
} >"$HERE/excerpts/tree-summary.txt"
echo "wrote $HERE/excerpts/tree-summary.txt"
