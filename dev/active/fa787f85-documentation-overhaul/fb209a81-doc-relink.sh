#!/usr/bin/env bash
# Repoints each deck's tracker document reference from docs/presentations/ to the
# owning epic's archive directory. Run from the repository root after the
# fb209a81 relocation commit is on the branch; each add rescans the deck's assets.
set -euo pipefail

relink() { # <epic> <old> <new> <doc-type> <label>
  jit doc add "$1" "$3" --doc-type "$4" --label "$5"
  jit doc remove "$1" "$2"
}

relink 6efb756b \
  docs/presentations/6efb756b-grand-sogrand/talk.html \
  dev/archive/6efb756b-grand/docs/presentations/6efb756b-grand-sogrand/talk.html \
  presentation 'Epic closeout presentation (GRAND/SOGRAND math + Fig 4/5/6 paper comparison)'
relink ae82bd73 \
  docs/presentations/ae82bd73-gf2-algebra-permanent/talk.html \
  dev/archive/ae82bd73-gf2-algebra-permanent/docs/presentations/ae82bd73-gf2-algebra-permanent/talk.html \
  notes 'Epic closing-showcase presentation (reveal.js; 14 slides, 6 new research capabilities; fitcheck-clean)'
relink bb85c68a \
  docs/presentations/bb85c68a-fieldmatrix/talk.html \
  dev/archive/bb85c68a-field-linear-algebra/docs/presentations/bb85c68a-fieldmatrix/talk.html \
  report 'FieldMatrix showcase presentation'
relink d4851c3d \
  docs/presentations/d4851c3d-modem-framework/talk.html \
  dev/archive/d4851c3d-modem-framework/docs/presentations/d4851c3d-modem-framework/talk.html \
  presentation 'Modem framework for gf2 — researcher-facing highlights (revealjs)'

jit doc check-links --scope all
