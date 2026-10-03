#!/usr/bin/env bash
# usage: 112f23c6-verdicts.sh <outfile>; benchmark-acceptance on scratch copies of every committed acceptance-summary receipt directory (needs target/ci-test/benchmark-acceptance)
set -u
cd "$(git rev-parse --show-toplevel)"
tmp=$(mktemp -d)
for f in $(find dev/bench_results -name acceptance-summary.json | sort); do
  d=$(dirname "$f"); rm -rf "$tmp/c"; mkdir -p "$tmp/c"; cp -r "$d/." "$tmp/c/"
  echo "$d $(target/ci-test/benchmark-acceptance "$tmp/c" 2>&1 | tail -1)"
done > "$1"
rm -rf "$tmp"
