#!/usr/bin/env bash
# usage: 7039d680-remaining.sh <outfile>
# Lists every tracked file outside .jit/, the 7039d680 record files, each
# progress.json and the handoffs of this record's directory
# that holds the flat path of one of the entries 26465e6c, eda07788 and
# 5cbb6545 (`dev/active/<entry>` outside the epic directory), with the class
# that decides why the path text stays; prints the files and matching lines of
# each class.
set -u
cd "$(git rev-parse --show-toplevel)"
entries='26465e6c|eda07788|5cbb6545'
class() {
  case $1 in
    */inputs/*) echo snapshot ;;
    */bench_results/*) echo receipt-directory ;;
    */migration/manifest.toml) echo manifest ;;
    */breakdown.json) echo tracker-state ;;
    */2037941f-*/dense-parity-addendum*.md) echo frozen-document ;;
    */fa787f85-documentation-overhaul/*) echo epic-record ;;
    *.json|*.jsonl|*.txt|*.tsv|*.log) echo data-record ;;
    */63bad95d/route-inventory.md) echo generated-table ;;
    */handoff-*.md|*/reviews/*) echo session-record ;;
    */26465e6c/superseded/v1/survey/*) echo pinned-script ;;
    */eda07788/survey/freeze-remeasure.py) echo addendum-text ;;
    */c04dd4ac-*/survey/make-profile-provenance.py) echo revision-path ;;
    *) echo "unclassified:$1" ;;
  esac
}
git grep -c -E "dev/active/($entries)([^0-9a-z-]|\$)" -- . ':!.jit' ':(exclude,glob)**/7039d680-*' ':(exclude,glob)**/progress.json' ':(exclude,glob)**/fa787f85-*/handoff*.md' |
while IFS=: read -r file count; do echo "$(class "$file") $count"; done |
awk '{f[$1]++; l[$1]+=$2} END{for (c in f) print c, f[c], l[c]}' | sort > "$1"
