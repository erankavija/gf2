#!/usr/bin/env bash
# usage: b5b336e5-remaining.sh <outfile>
# Lists every tracked file outside .jit/ and outside the b5b336e5 record files
# that holds the flat path `dev/active/bc091474`, with the class that decides
# why the path text stays; prints class, files and matching lines.
set -u
cd "$(git rev-parse --show-toplevel)"
class() {
  case $1 in
    */inputs/*) echo snapshot ;;
    dev/bench_results/*) echo receipt-directory ;;
    */migration/manifest.toml) echo manifest ;;
    */progress.json) echo tracker-state ;;
    */bench-window/*) echo bench-window ;;
    */plan-checks.tsv|*/flag-readers.json|*/7bac1303-harness-path-trials.txt) echo tool-output ;;
    */decoupling-findings.md) echo investigation-at-commit ;;
    *) echo unclassified ;;
  esac
}
git grep -c -F 'dev/active/bc091474' -- . ':!.jit' ':!dev/active/fa787f85-documentation-overhaul/b5b336e5-*' |
while IFS=: read -r file count; do echo "$(class "$file") $count"; done |
awk '{f[$1]++; l[$1]+=$2} END{for (c in f) print c, f[c], l[c]}' | sort > "$1"
