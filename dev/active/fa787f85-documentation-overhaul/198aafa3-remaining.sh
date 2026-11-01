#!/usr/bin/env bash
# usage: 198aafa3-remaining.sh <outfile>
# Lists every tracked file outside .jit/, the 198aafa3 record files, each
# progress.json and the handoffs of this record's directory
# that holds the flat path of one of the eight entries of issue 198aafa3
# (`<active root>/<entry>`, outside the epic directory), with the class that decides
# why the path text stays; prints the files and matching lines of each class.
set -u
cd "$(git rev-parse --show-toplevel)"
entries='4c1e441f|53c5a8c0|6c6b09b1|6fb89a3c|ad2a6a58|c077a88b|c7113c5a|f63a2464'
epic=$(git ls-files -- ':(glob)**/4c1e441f/**' | grep -v '/inputs/' | sed -n -E '1s#/4c1e441f/.*##p')
active=$(dirname "$epic")
class() {
  case $1 in
    */inputs/*) echo snapshot ;;
    */bench_results/*) echo receipt-directory ;;
    */migration/manifest.toml) echo manifest ;;
    */bench-window/*) echo bench-window ;;
    */breakdown.json) echo tracker-state ;;
    */2037941f-*/dense-parity-addendum*.md) echo frozen-document ;;
    */fa787f85-documentation-overhaul/*) echo epic-record ;;
    *.json|*.jsonl|*.txt|*.tsv|*.log) echo data-record ;;
    */63bad95d/route-inventory.md) echo generated-table ;;
    */handoff-23.md) echo manifest-quote ;;
    */c7113c5a/survey/summarize-v3.py) echo recorded-value ;;
    */6c6b09b1/survey/make-addenda.py) echo addendum-text ;;
    *) echo "unclassified:$1" ;;
  esac
}
git grep -c -E "$active/($entries)([^0-9a-z-]|\$)" -- . ':!.jit' ':(exclude,glob)**/198aafa3-*' ':(exclude,glob)**/progress.json' ':(exclude,glob)**/fa787f85-*/handoff*.md' |
while IFS=: read -r file count; do echo "$(class "$file") $count"; done |
awk '{f[$1]++; l[$1]+=$2} END{for (c in f) print c, f[c], l[c]}' | sort > "$1"
