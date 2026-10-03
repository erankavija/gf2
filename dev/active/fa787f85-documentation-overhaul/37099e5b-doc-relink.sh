#!/usr/bin/env bash
# Repoints the tracker document references of the three session notes from
# dev/sessions/ to epic b8206228's active directory. Run after the 37099e5b
# relocation commit is on the branch; each add rescans the document.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

old=dev/sessions
new=dev/active/b8206228-permanent-statistics/sessions

relink() { # owner, file, doc type, label
  jit doc add "$1" "$new/$2" --doc-type "$3" --label "$4"
  jit doc remove "$1" "$old/$2"
}

relink b488f02c 2026-08-07-research-frontier-handoff.md report 'Session handoff 2026-08-07 (resume contract)'
relink b8206228 2026-08-08-b488f02c-review-rca.md report 'RCA: b488f02c review cycle (2026-08-08)'
relink b8206228 2026-08-09-b8206228-planning-handoff.md notes '2026-08-09-b8206228-planning-handoff.md'

jit doc check-links --scope all
