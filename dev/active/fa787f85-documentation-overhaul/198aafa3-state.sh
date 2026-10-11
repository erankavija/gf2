#!/usr/bin/env bash
# usage: 198aafa3-state.sh <outfile>
# Prints, for the eight entries of issue 198aafa3 wherever they live under
# the active root: the `--check` result of every live closure generator, the
# relative Cargo path dependencies that name or leave an entry, and each
# line of entry code that counts directory levels.
set -u
cd "$(git rev-parse --show-toplevel)"
entries='4c1e441f|53c5a8c0|6c6b09b1|6fb89a3c|ad2a6a58|c077a88b|c7113c5a|f63a2464'
epic=$(git ls-files -- ':(glob)**/4c1e441f/**' | grep -v '/inputs/' | sed -n -E '1s#/4c1e441f/.*##p')
active=$(dirname "$epic")
{
  echo '# closure generators'
  git ls-files -- ':(glob)**/make-*-producing-inputs.py' ':(exclude,glob)**/inputs/**' | sort |
  while read -r g; do
    grep -q '"--check"' "$g" || continue
    echo "$g exit=$(python3 -B "$g" --check >/dev/null 2>&1; echo $?) closure-lines=$(python3 -B "$g" --check 2>&1 | grep -c '^[+-] ')"
  done
  echo '# relative Cargo path dependencies'
  git grep -n -E 'path *= *"\.\./' -- ':(glob)**/Cargo.toml' ':(exclude,glob)**/inputs/**' | grep -E "($entries)"
  echo '# code that counts directory levels'
  git grep -n -E 'parents\[[0-9]\]|\.\./\.\.|ancestors\(\)|"\.\./[a-z0-9]' -- ":(glob)$active/**/*.py" ":(glob)$active/**/*.sh" ":(glob)$active/**/*.rs" ':(exclude,glob)**/inputs/**' |
    grep -E "^$active/([^/]*/)?($entries)/"
} > "$1"
