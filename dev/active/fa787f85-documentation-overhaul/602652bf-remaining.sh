#!/usr/bin/env bash
# usage: 602652bf-remaining.sh <outfile>
# Lists every tracked file outside .jit/, the 602652bf record files, each
# progress.json and the handoffs of this record's directory
# that holds the flat path of the f547c394 or bdc507a3 entry
# (`dev/active/<entry>` outside the epic directory), with the class that decides
# why the path text stays; prints class, file count and matching line count.
set -u
cd "$(git rev-parse --show-toplevel)"
class() {
  case $1 in
    */inputs/*) echo snapshot ;;
    */bench_results/*) echo receipt-directory ;;
    */migration/manifest.toml) echo manifest ;;
    */breakdown.json) echo tracker-state ;;
    */2037941f-*/dense-parity-addendum*.md|*/2037941f-*/logical-buffer-addendum.md) echo frozen-document ;;
    */f547c394/findings.md|*/f547c394/research-r3-*-resolutions.md|*/f547c394/rework-validation*.md) echo line-qualified-record ;;
    */fa787f85-documentation-overhaul/*) echo epic-record ;;
    *.json|*.jsonl|*.txt|*.tsv|*.log) echo data-record ;;
    */mid-range-tables.md|*/publication-tables.md) echo generated-table ;;
    */superseded/v1/survey/record-provenance.sh) echo pinned-script ;;
    */reference/performance-evidence.md) echo commit-permalink ;;
    *) echo "unclassified:$1" ;;
  esac
}
git grep -c -E 'dev/active/(f547c394|bdc507a3)([^0-9a-z-]|$)' -- . ':!.jit' ':(exclude,glob)**/602652bf-*' ':(exclude,glob)**/progress.json' ':(exclude,glob)**/fa787f85-*/handoff*.md' |
while IFS=: read -r file count; do echo "$(class "$file") $count"; done |
awk '{f[$1]++; l[$1]+=$2} END{for (c in f) print c, f[c], l[c]}' | sort > "$1"
