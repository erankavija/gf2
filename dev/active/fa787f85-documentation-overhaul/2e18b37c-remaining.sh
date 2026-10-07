#!/usr/bin/env bash
# usage: 2e18b37c-remaining.sh <outfile>
# Lists every line of a tracked file outside .jit/ and outside the 2e18b37c
# record files that holds the flat path of one of the entries f8dd4dde,
# a203a23c and 19513245 (`dev/active/<entry>` outside the epic directory),
# with the class that decides why the path text stays; prints the file and
# line count of each class and each line of a class that names single lines.
set -u
cd "$(git rev-parse --show-toplevel)"
entries='f8dd4dde|a203a23c|19513245'
epic=dev/active/1a379447-zen3-cpu-performance
class() {
  case $1 in
    */inputs/*) echo snapshot ;;
    dev/bench_results/*) echo receipt-directory ;;
    */migration/manifest.toml) echo manifest ;;
    "$epic"/progress.json) echo tracker-state ;;
    "$epic"/bench-window/queue.tsv) echo bench-window ;;
    dev/active/fa787f85-documentation-overhaul/*) echo epic-record ;;
    *.json|*.jsonl|*.txt|*.tsv|*.log) echo data-record ;;
    "$epic"/63bad95d/route-inventory.md) echo generated-table ;;
    "$epic"/handoff-*.md|"$epic"/reviews/*) echo session-record ;;
    "$epic"/a203a23c/timing/*) case $2 in *restore-receipt-inputs.py*) echo revision-path ;; *) echo record-text ;; esac ;;
    "$epic"/19513245/survey/summarize-profile.py) echo record-text ;;
    "$epic"/19513245/findings.md) echo queue-quote ;;
    *) echo unclassified ;;
  esac
}
git grep -n -E "dev/active/($entries)([^0-9a-z-]|\$)" -- . ':!.jit' ':!dev/active/fa787f85-documentation-overhaul/2e18b37c-*' |
while IFS=: read -r file line text; do echo "$(class "$file" "$text") $file $line"; done > "$1.lines"
{
  awk '{l[$1]++; if (!s[$1,$2]++) f[$1]++} END{for (c in l) print c, f[c], l[c]}' "$1.lines" | sort
  grep -E '^(revision-path|record-text|queue-quote|unclassified) ' "$1.lines" | sed "s#$epic/##" |
    awk '{k=$1" "$2; a[k]=a[k] (a[k]?",":"") $3} END{for (k in a) print k":"a[k]}' | sort
} > "$1"
rm -f "$1.lines"
