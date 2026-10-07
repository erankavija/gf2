#!/usr/bin/env bash
# usage: 7039d680-remaining.sh <outfile>
# Lists every tracked file outside .jit/ and outside the 7039d680 record files
# that holds the flat path of one of the entries 26465e6c, eda07788 and
# 5cbb6545 (`dev/active/<entry>` outside the epic directory), with the class
# that decides why the path text stays; prints the files and matching lines of
# each class.
set -u
cd "$(git rev-parse --show-toplevel)"
entries='26465e6c|eda07788|5cbb6545'
epic=dev/active/1a379447-zen3-cpu-performance
shifts=dev/active/c04dd4ac-zen3-shifts-and-permutations
class() {
  case $1 in
    */inputs/*) echo snapshot ;;
    dev/bench_results/*) echo receipt-directory ;;
    */migration/manifest.toml) echo manifest ;;
    */progress.json|*/breakdown.json) echo tracker-state ;;
    "$epic"/2037941f-*/dense-parity-addendum.md|"$epic"/2037941f-*/dense-parity-addendum-v1.md) echo frozen-document ;;
    dev/active/fa787f85-documentation-overhaul/*) echo epic-record ;;
    *.json|*.jsonl|*.txt|*.tsv|*.log) echo data-record ;;
    "$epic"/63bad95d/route-inventory.md) echo generated-table ;;
    "$epic"/handoff-*.md|"$epic"/reviews/*) echo session-record ;;
    "$epic"/26465e6c/superseded/v1/survey/*) echo pinned-script ;;
    "$epic"/eda07788/survey/freeze-remeasure.py) echo addendum-text ;;
    "$epic"/eda07788/survey/inspect-sources.py) echo snapshot-path ;;
    "$shifts"/survey/make-profile-provenance.py) echo revision-path ;;
    *) echo "unclassified:$1" ;;
  esac
}
git grep -c -E "dev/active/($entries)([^0-9a-z-]|\$)" -- . ':!.jit' ':!dev/active/fa787f85-documentation-overhaul/7039d680-*' |
while IFS=: read -r file count; do echo "$(class "$file") $count"; done |
awk '{f[$1]++; l[$1]+=$2} END{for (c in f) print c, f[c], l[c]}' | sort > "$1"
