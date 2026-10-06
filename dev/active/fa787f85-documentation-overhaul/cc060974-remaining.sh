#!/usr/bin/env bash
# usage: cc060974-remaining.sh <outfile>
# Lists every line of a tracked file outside .jit/ and outside the cc060974
# record files that holds the flat path of the entry 00dd43c3 or
# c04dd4ac-zen3-shifts-and-permutations (`dev/active/<entry>` outside the epic
# directory), with the class that decides why the path text stays; prints the
# file and line count of each class and each line of a class that names
# single lines.
set -u
cd "$(git rev-parse --show-toplevel)"
entries='00dd43c3|c04dd4ac-zen3-shifts-and-permutations'
epic=dev/active/1a379447-zen3-cpu-performance
class() {
  case $1 in
    */inputs/*) echo snapshot ;;
    dev/bench_results/*) echo receipt-directory ;;
    */migration/manifest.toml) echo manifest ;;
    "$epic"/progress.json) echo tracker-state ;;
    "$epic"/bench-window/queue.tsv) echo bench-window ;;
    dev/active/fa787f85-documentation-overhaul/*) echo epic-record ;;
    */breakdown.json) echo breakdown-manifest ;;
    *.json|*.jsonl|*.txt|*.tsv|*.log) echo data-record ;;
    "$epic"/63bad95d/*.md) echo generated-table ;;
    "$epic"/*/survey/*.py) echo record-text ;;
    *) echo unclassified ;;
  esac
}
git grep -n -E "dev/active/($entries)([^0-9a-z-]|\$)" -- . ':!.jit' ':!dev/active/fa787f85-documentation-overhaul/cc060974-*' |
while IFS=: read -r file line text; do echo "$(class "$file") $file $line"; done > "$1.lines"
{
  awk '{l[$1]++; if (!s[$1,$2]++) f[$1]++} END{for (c in l) print c, f[c], l[c]}' "$1.lines" | sort
  grep -E '^(record-text|generated-table|breakdown-manifest|data-record|unclassified) ' "$1.lines" | grep -v ' dev/bench_results/' | sed "s#$epic/##" |
    awk '{k=$1" "$2; a[k]=a[k] (a[k]?",":"") $3} END{for (k in a) print k":"a[k]}' | sort
} > "$1"
rm -f "$1.lines"
