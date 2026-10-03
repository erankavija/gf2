#!/usr/bin/env bash
# Repoints task 886af072's tracker document reference from docs/ to epic
# e095a100's archive directory. Run from the repository root after the
# 130d5fd7 relocation commit is on the branch; the add rescans the document.
set -euo pipefail

old=docs/lean4-verification-pipeline.md
new=dev/archive/e095a100-gfpm-arithmetic/docs/lean4-verification-pipeline.md
jit doc add 886af072 "$new" --doc-type implementation --label 'Lean4 verification pipeline documentation'
jit doc remove 886af072 "$old"

jit doc check-links --scope all
