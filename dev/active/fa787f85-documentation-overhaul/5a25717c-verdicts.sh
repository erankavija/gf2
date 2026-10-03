#!/usr/bin/env bash
# usage: 5a25717c-verdicts.sh <outfile>
# Runs target/ci-test/benchmark-acceptance on a scratch copy of the directory of
# every tracked receipt.json whose schema is zen3-benchmark-receipt-v1, outside
# inputs/ snapshot directories; prints each verdict line and every finding.
set -u
cd "$(git rev-parse --show-toplevel)"
tmp=$(mktemp -d)
git ls-files -z -- ':(glob)**/receipt.json' | tr '\0' '\n' | grep -v '/inputs/' | sort |
while read -r f; do
  [ "$(jq -r '.schema? // empty' "$f" 2>/dev/null)" = zen3-benchmark-receipt-v1 ] || continue
  d=$(dirname "$f"); rm -rf "$tmp/c"; mkdir -p "$tmp/c"; cp -r "$d/." "$tmp/c/"
  rm -f "$tmp/c/acceptance-summary.json"
  echo "$d $(target/ci-test/benchmark-acceptance "$tmp/c" 2>&1 | tail -1)"
  if [ -f "$tmp/c/acceptance-summary.json" ]; then
    jq -r '.findings[] | "  \(.rule) \(.severity) \(.cell // "-") \(.message)"' "$tmp/c/acceptance-summary.json"
  fi
done > "$1"
rm -rf "$tmp"
