#!/usr/bin/env bash
# usage: 2e18b37c-remaining.sh <outfile>
# Lists every line of a tracked file outside .jit/, the 2e18b37c record files,
# each progress.json and the handoffs of this record's directory that holds
# the flat path of one of the entries f8dd4dde, a203a23c and 19513245
# (`dev/active/<entry>` outside the epic directory), with the class that
# decides why the path text stays; prints the file and line count of each
# class and each line of a class that names single lines.
set -u
cd "$(git rev-parse --show-toplevel)"
entries='f8dd4dde|a203a23c|19513245'
class() {
  case $1 in
    */inputs/*) echo snapshot ;;
    */bench_results/*) echo receipt-directory ;;
    */migration/manifest.toml) echo manifest ;;
    */bench-window/queue.tsv) echo bench-window ;;
    */fa787f85-documentation-overhaul/*) echo epic-record ;;
    *.json|*.jsonl|*.txt|*.tsv|*.log) echo data-record ;;
    */63bad95d/route-inventory.md) echo generated-table ;;
    */handoff-*.md|*/reviews/*) echo session-record ;;
    */a203a23c/timing/*) echo revision-path ;;
    */19513245/findings.md) echo queue-quote ;;
    *) echo unclassified ;;
  esac
}
git grep -n -E "dev/active/($entries)([^0-9a-z-]|\$)" -- . ':!.jit' ':(exclude,glob)**/2e18b37c-*' ':(exclude,glob)**/progress.json' ':(exclude,glob)**/fa787f85-*/handoff*.md' |
while IFS=: read -r file line text; do echo "$(class "$file") $file $line"; done > "$1.lines"
{
  awk '{l[$1]++; if (!s[$1,$2]++) f[$1]++} END{for (c in l) print c, f[c], l[c]}' "$1.lines" | sort
  grep -E '^(revision-path|queue-quote|unclassified) ' "$1.lines" | sed -E "s# [^ ]*/(($entries)/)# \\1#" |
    awk '{k=$1" "$2; a[k]=a[k] (a[k]?",":"") $3} END{for (k in a) print k":"a[k]}' | sort
} > "$1"
rm -f "$1.lines"
