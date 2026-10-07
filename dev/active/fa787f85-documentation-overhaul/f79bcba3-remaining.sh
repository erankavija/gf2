#!/usr/bin/env bash
# usage: f79bcba3-remaining.sh <outfile>
# Lists every tracked file outside .jit/, the f79bcba3 record files, each
# progress.json and the handoffs of this record's directory
# that holds the flat path of one of the eight entries of issue f79bcba3
# (`dev/active/<entry>` outside the epic directory), with the class that decides
# why the path text stays; prints the files and matching lines of each class.
set -u
cd "$(git rev-parse --show-toplevel)"
entries='04b85d10|07ca8585|12fdeb5b|1c602857|1d0da41f|1d4fd63d|2037941f-profile-and-optimize-mid-range-buffer-operations|3be770d5'
class() {
  case $1 in
    */inputs/*) echo snapshot ;;
    */bench_results/*) echo receipt-directory ;;
    */migration/manifest.toml) echo manifest ;;
    */bench-window/*) echo bench-window ;;
    */2037941f-*/dense-parity-addendum*.md|*/2037941f-*/logical-buffer-addendum.md|*/2037941f-*/isal-comparator.md|*/2037941f-*/m4ri-operation-match.md) echo frozen-document ;;
    */c7113c5a/survey/freeze-confirmation.test.sh) echo fixture-layout ;;
    */fa787f85-documentation-overhaul/*) echo epic-record ;;
    */handoff-*.md|*/reviews/*) echo session-record ;;
    *.json|*.jsonl|*.txt|*.tsv|*.log) echo data-record ;;
    */route-inventory.md) echo generated-table ;;
    *summarize-receipts.py) echo snapshot-path ;;
    *) echo unclassified ;;
  esac
}
git grep -c -E "(^|[^/])dev/active/($entries)([^0-9a-z-]|$)|/dev/active/($entries)([^0-9a-z-]|$)" -- . ':!.jit' ':(exclude,glob)**/f79bcba3-*' ':(exclude,glob)**/progress.json' ':(exclude,glob)**/fa787f85-*/handoff*.md' |
while IFS=: read -r file count; do echo "$(class "$file") $count"; done |
awk '{f[$1]++; l[$1]+=$2} END{for (c in f) print c, f[c], l[c]}' | sort > "$1"
