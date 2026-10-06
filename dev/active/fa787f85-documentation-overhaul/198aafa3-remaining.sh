#!/usr/bin/env bash
# usage: 198aafa3-remaining.sh <outfile>
# Lists every tracked file outside .jit/ and outside the 198aafa3 record files
# that holds the flat path of one of the eight entries of issue 198aafa3
# (`dev/active/<entry>` outside the epic directory), with the class that decides
# why the path text stays; prints the files and matching lines of each class.
set -u
cd "$(git rev-parse --show-toplevel)"
entries='4c1e441f|53c5a8c0|6c6b09b1|6fb89a3c|ad2a6a58|c077a88b|c7113c5a|f63a2464'
epic=dev/active/1a379447-zen3-cpu-performance
story=$epic/2037941f-profile-and-optimize-mid-range-buffer-operations
class() {
  case $1 in
    */inputs/*) echo snapshot ;;
    dev/bench_results/*) echo receipt-directory ;;
    */migration/manifest.toml) echo manifest ;;
    "$epic"/bench-window/*) echo bench-window ;;
    */progress.json|*/breakdown.json) echo tracker-state ;;
    "$story"/dense-parity-addendum.md|"$story"/dense-parity-addendum-v1.md) echo frozen-document ;;
    dev/active/fa787f85-documentation-overhaul/*) echo epic-record ;;
    dev/scripts/receipt-input-omissions.json) echo snapshot-path ;;
    *.json|*.jsonl|*.txt|*.tsv|*.log) echo data-record ;;
    "$epic"/63bad95d/route-inventory.md) echo generated-table ;;
    "$epic"/handoff-23.md) echo manifest-quote ;;
    "$epic"/c7113c5a/survey/summarize-v3.py) echo recorded-value ;;
    "$epic"/6c6b09b1/survey/make-addenda.py) echo addendum-text ;;
    *) echo "unclassified:$1" ;;
  esac
}
git grep -c -E "dev/active/($entries)([^0-9a-z-]|\$)" -- . ':!.jit' ':!dev/active/fa787f85-documentation-overhaul/198aafa3-*' |
while IFS=: read -r file count; do echo "$(class "$file") $count"; done |
awk '{f[$1]++; l[$1]+=$2} END{for (c in f) print c, f[c], l[c]}' | sort > "$1"
