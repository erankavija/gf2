#!/usr/bin/env bash
# Re-derive excerpts/tree-summary.txt for JIT issue 7d7c647c.
#
# For every generated Lean tree in this probe set: the files Aeneas wrote, the
# structure and definition counts, the `sorry` count, whether the four tuning
# constants reach the tree, and — for the gf2-core trees — how many hand-carried
# literal substitutions the committed post-processing then performs.
set -uo pipefail

# `grep -c` prints 0 and exits 1 on no match, so count through a helper rather
# than an `|| echo 0` fallback that would print the zero twice.
count() { [ -f "$2" ] || { echo 0; return; }; grep -cE "$1" "$2" 2>/dev/null || true; }

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"
{
  echo "# Generated-tree summary for JIT issue 7d7c647c."
  echo "# Reproduce with: dev/active/7d7c647c/probes/tree-summary.sh"
  echo "# Captured: $(date -Is)"
  for d in "$HERE"/A*_lean; do
    [ -d "$d" ] || continue
    echo
    echo "## $(basename "$d")"
    echo "- files: $(cd "$d" && ls -1 | tr '\n' ' ')"
    echo "- structures in Types.lean: $(count '^structure ' "$d/Types.lean")"
    echo "- defs in Funs.lean: $(count '^ *def ' "$d/Funs.lean")"
    echo "- impl_def in Funs.lean: $(count '^ *impl_def' "$d/Funs.lean")"
    echo "- axioms in FunsExternal_Template.lean: $(count '^axiom ' "$d/FunsExternal_Template.lean")"
    echo "- sorry in Funs.lean: $(count sorry "$d/Funs.lean")"
    echo "- repeated structure field names in Types.lean:"
    dups="$(grep -h "Inst :" "$d/Types.lean" 2>/dev/null | awk '{print $1}' | sort | uniq -d)"
    if [ -z "$dups" ]; then echo "    (none)"; else echo "$dups" | sed 's/^/    /'; fi
    hits="$(grep -ho 'WINOGRAD_THRESHOLD\|TRI_BASE_THRESHOLD\|PLE_BASE_COLS\|PLE_PANEL_COLS' "$d"/*.lean 2>/dev/null | sort | uniq -c | tr '\n' ' ')"
    echo "- tuning-constant occurrences: ${hits:-(none)}"
    case "$(basename "$d")" in
      AS*)
        w="$(mktemp -d)"
        cp -r "$d" "$w/t"
        pp="$(cd "$ROOT" && python3 scripts/fix-aeneas-dupes.py "$w/t/Types.lean" "$w/t/Funs.lean" 2>&1; python3 "$ROOT/scripts/fix-aeneas-sorrys.py" "$w/t/Funs.lean" 2>&1)"
        echo "- committed post-processing: $(echo "$pp" | tr '\n' ' ')"
        echo "- hand-carried literals it writes:"
        grep -hE '^  (WINOGRAD_THRESHOLD|TRI_BASE_THRESHOLD|PLE_BASE_COLS|PLE_PANEL_COLS) :=' "$w/t/Funs.lean" 2>/dev/null | sed 's/^/    /' || true
        grep -qE '^  (WINOGRAD_THRESHOLD|TRI_BASE_THRESHOLD|PLE_BASE_COLS|PLE_PANEL_COLS) :=' "$w/t/Funs.lean" 2>/dev/null || echo "    (none)"
        rm -rf "$w"
        ;;
    esac
  done
} >"$HERE/excerpts/tree-summary.txt"
echo "wrote $HERE/excerpts/tree-summary.txt"
