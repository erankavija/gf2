#!/usr/bin/env bash
# Re-derive the committed surface diffs for JIT issue 7d7c647c.
#
# `AS1_lean/` is committed in full as the baseline; the three other gf2-core
# trees are 99 % identical to it, so each is committed as a complete diff
# against its comparison base rather than as a fourth copy. `apply-diff.sh`
# reconstructs any of them, and `run.sh` regenerates them from source.
#
# Generated module prefixes are normalised to `SGf2Core` / `XGf2Core` so the
# diffs show only what the source change did.
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
norm() { sed -e 's/S[1-4]Gf2Core/SGf2Core/g' -e 's/X[1-2]Gf2Core/XGf2Core/g' "$1"; }

emit() { # <from-tree> <to-tree> <out>
  local from="$1" to="$2" out="$HERE/excerpts/$3"
  : >"$out"
  for f in Types.lean Funs.lean FunsExternal_Template.lean TypesExternal_Template.lean; do
    if [ -f "$HERE/$from/$f" ] || [ -f "$HERE/$to/$f" ]; then
      echo "=== $f ===" >>"$out"
      diff <(norm "$HERE/$from/$f" 2>/dev/null) <(norm "$HERE/$to/$f" 2>/dev/null) >>"$out"
    fi
  done
  echo "wrote $out"
}

emit AS1_lean AS2_lean AS1-to-AS2.diff
emit AS2_lean AS3_lean AS2-to-AS3.diff
emit AS1_lean AS4_lean AS1-to-AS4.diff
emit AX1_lean AX2_lean AX1-to-AX2.diff
