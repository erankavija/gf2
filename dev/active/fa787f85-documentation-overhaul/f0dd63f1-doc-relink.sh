#!/usr/bin/env bash
# Repoints the tracker document references of the two gruvbox.css stylesheets
# from dev/presentations/ to their owning epics' archive directories. Run after
# the f0dd63f1 relocation commit is on the branch; each add rescans the document.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

relink() { # owner, old path, new path
  jit doc add "$1" "$3" --doc-type tool --label gruvbox.css
  jit doc remove "$1" "$2"
}

relink babcf05e dev/presentations/babcf05e-gf2-core-ppc-spiral/babcf05e-ppc-spiral/themes/gruvbox.css \
  dev/archive/babcf05e-gf2-core-ppc-spiral/presentations/babcf05e-ppc-spiral/themes/gruvbox.css
relink f9717e7e dev/presentations/f9717e7e-gf2-sim/themes/gruvbox.css \
  dev/archive/f9717e7e-gf2-sim/presentations/themes/gruvbox.css

jit doc check-links --scope all
