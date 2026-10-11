#!/usr/bin/env bash
# usage: 198aafa3-selfchecks.sh <outfile>
# Runs, from the repository root, every tracked script test under the active root
# (*_test.py, test-*.py, test_*.py, *.test.sh, verify-records.py) outside
# inputs/ snapshot directories, the fd9d5416 entry and this record's
# directory, and `--self-test` of each script of the eight entries of issue
# 198aafa3 that offers it; prints one line per run: the path below the active root
# without the epic directory, and the exit status.
set -u
cd "$(git rev-parse --show-toplevel)"
entries='4c1e441f|53c5a8c0|6c6b09b1|6fb89a3c|ad2a6a58|c077a88b|c7113c5a|f63a2464'
epic=$(git ls-files -- ':(glob)**/4c1e441f/**' | grep -v '/inputs/' | sed -n -E '1s#/4c1e441f/.*##p')
active=$(dirname "$epic")
name() { local m=${1#"$active"/}; echo "${m#"${epic#"$active"/}"/}"; }
{
  git ls-files -- ":(glob)$active/**/*_test.py" ":(glob)$active/**/test-*.py" \
    ":(glob)$active/**/test_*.py" ":(glob)$active/**/*.test.sh" \
    ":(glob)$active/**/verify-records.py" |
    grep -v -e /inputs/ -e /fd9d5416/ -e /fa787f85-documentation-overhaul/ |
  while read -r t; do
    case $t in *.sh) bash "$t" ;; *) python3 -B "$t" ;; esac >/dev/null 2>&1
    echo "$(name "$t") exit=$?"
  done
  git grep -l -e '--self-test' -- ":(glob)$active/**/*.py" ':(exclude,glob)**/inputs/**' |
    grep -E "/($entries)/" |
  while read -r t; do
    python3 -B "$t" --self-test >/dev/null 2>&1
    echo "$(name "$t") --self-test exit=$?"
  done
} | sort > "$1"
