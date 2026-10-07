#!/usr/bin/env bash
# usage: 602652bf-remaining.sh <outfile>
# Lists every tracked file outside .jit/ and outside the 602652bf record files
# that holds the flat path of the f547c394 or bdc507a3 entry
# (`dev/active/<entry>` outside the epic directory), with the class that decides
# why the path text stays; prints class, file count and matching line count.
set -u
cd "$(git rev-parse --show-toplevel)"
epic=dev/active/1a379447-zen3-cpu-performance
story=$epic/2037941f-profile-and-optimize-mid-range-buffer-operations
entry=$epic/f547c394
class() {
  case $1 in
    */inputs/*) echo snapshot ;;
    dev/bench_results/*) echo receipt-directory ;;
    */migration/manifest.toml) echo manifest ;;
    */progress.json|*/breakdown.json) echo tracker-state ;;
    "$story"/dense-parity-addendum.md|"$story"/dense-parity-addendum-v1.md|"$story"/logical-buffer-addendum.md) echo frozen-document ;;
    "$entry"/findings.md|"$entry"/research-r3-*-resolutions.md|"$entry"/rework-validation*.md) echo line-qualified-record ;;
    dev/active/fa787f85-documentation-overhaul/*) echo epic-record ;;
    *.json|*.jsonl|*.txt|*.tsv|*.log) echo data-record ;;
    */mid-range-tables.md|*/publication-tables.md) echo generated-table ;;
    */make-voided-attempt.py|*/make-voided-attempt-record.py) echo record-text ;;
    */superseded/v1/survey/record-provenance.sh) echo pinned-script ;;
    docs/reference/performance-evidence.md) echo commit-permalink ;;
    *) echo "unclassified:$1" ;;
  esac
}
git grep -c -E 'dev/active/(f547c394|bdc507a3)([^0-9a-z-]|$)' -- . ':!.jit' ':!dev/active/fa787f85-documentation-overhaul/602652bf-*' |
while IFS=: read -r file count; do echo "$(class "$file") $count"; done |
awk '{f[$1]++; l[$1]+=$2} END{for (c in f) print c, f[c], l[c]}' | sort > "$1"
