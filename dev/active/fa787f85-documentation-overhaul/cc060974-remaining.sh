#!/usr/bin/env bash
# usage: cc060974-remaining.sh <outfile>
# Lists every line of a tracked file outside .jit/, the cc060974 record files,
# each progress.json and the handoffs of this record's directory that holds
# the flat path of the entry 00dd43c3 or c04dd4ac-zen3-shifts-and-permutations
# (`dev/active/<entry>` outside the epic directory), with the class that
# decides why the path text stays; prints the file and line count of each
# class and each line of a class that names single lines.
set -u
cd "$(git rev-parse --show-toplevel)"
entries='00dd43c3|c04dd4ac-zen3-shifts-and-permutations'
class() {
  case $1 in
    */inputs/*) echo snapshot ;;
    */bench_results/*) echo receipt-directory ;;
    */migration/manifest.toml) echo manifest ;;
    */bench-window/queue.tsv) echo bench-window ;;
    */fa787f85-documentation-overhaul/*) echo epic-record ;;
    */breakdown.json) echo breakdown-manifest ;;
    *.json|*.jsonl|*.txt|*.tsv|*.log) echo data-record ;;
    */63bad95d/*.md) echo generated-table ;;
    */c04dd4ac-*/survey/*.py) echo record-text ;;
    *) echo unclassified ;;
  esac
}
git grep -n -E "dev/active/($entries)([^0-9a-z-]|\$)" -- . ':!.jit' ':(exclude,glob)**/cc060974-*' ':(exclude,glob)**/progress.json' ':(exclude,glob)**/fa787f85-*/handoff*.md' |
while IFS=: read -r file line text; do echo "$(class "$file") $file $line"; done > "$1.lines"
{
  awk '{l[$1]++; if (!s[$1,$2]++) f[$1]++} END{for (c in l) print c, f[c], l[c]}' "$1.lines" | sort
  grep -E '^(record-text|generated-table|breakdown-manifest|data-record|unclassified) ' "$1.lines" | grep -v '/bench_results/' | sed -E "s# [^ ]*/((00dd43c3|c04dd4ac-[^/]*|63bad95d)/)# \\1#" |
    awk '{k=$1" "$2; a[k]=a[k] (a[k]?",":"") $3} END{for (k in a) print k":"a[k]}' | sort
} > "$1"
rm -f "$1.lines"
