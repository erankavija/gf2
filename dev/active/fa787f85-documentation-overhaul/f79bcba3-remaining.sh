#!/usr/bin/env bash
# usage: f79bcba3-remaining.sh <outfile>
# Lists every tracked file outside .jit/ and outside the f79bcba3 record files
# that holds the flat path of one of the eight entries of issue f79bcba3
# (`dev/active/<entry>` outside the epic directory), with the class that decides
# why the path text stays. Snapshot and receipt-directory files are counted; the
# others are listed with the commit that last changed the file's content.
set -u
cd "$(git rev-parse --show-toplevel)"
entries='04b85d10|07ca8585|12fdeb5b|1c602857|1d0da41f|1d4fd63d|2037941f-profile-and-optimize-mid-range-buffer-operations|3be770d5'
epic=dev/active/1a379447-zen3-cpu-performance
story=$epic/2037941f-profile-and-optimize-mid-range-buffer-operations
class() {
  case $1 in
    */inputs/*) echo snapshot ;;
    dev/bench_results/*) echo receipt-directory ;;
    dev/archive/*) echo archive ;;
    */migration/manifest.toml) echo manifest ;;
    "$epic"/bench-window/*) echo bench-window ;;
    */progress.json) echo lead-state ;;
    "$story"/dense-parity-addendum.md|"$story"/dense-parity-addendum-v1.md|"$story"/logical-buffer-addendum.md|"$story"/isal-comparator.md|"$story"/m4ri-operation-match.md) echo frozen-document ;;
    dev/active/c7113c5a/survey/freeze-confirmation.test.sh) echo fixture-layout ;;
    dev/active/fa787f85-documentation-overhaul/*) echo epic-record ;;
    *.json|*.jsonl|*.txt|*.tsv|*.log) echo data-record ;;
    */route-inventory.md) echo generated-table ;;
    *summarize-receipts.py) echo snapshot-path ;;
    *) echo unclassified ;;
  esac
}
git grep -c -E "(^|[^/])dev/active/($entries)([^0-9a-z-]|$)|/dev/active/($entries)([^0-9a-z-]|$)" -- . ':!.jit' ':!dev/active/fa787f85-documentation-overhaul/f79bcba3-*' |
while IFS=: read -r file count; do echo "$(class "$file") $file $count"; done | sort > "$1.all"
{
  echo '# files and matching lines per class'
  awk '{f[$1]++; l[$1]+=$3} END{for (c in f) print c, f[c], l[c]}' "$1.all" | sort
  echo '# files of the listed classes: class, file, matching lines, last content commit'
  grep -v -E '^(snapshot|receipt-directory) ' "$1.all" |
  while read -r c file count; do
    echo "$c $file $count $(git log -1 --follow --diff-filter=AM --format=%h -- "$file")"
  done
} > "$1"
rm -f "$1.all"
