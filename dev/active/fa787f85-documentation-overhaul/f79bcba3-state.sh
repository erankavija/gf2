#!/usr/bin/env bash
# usage: f79bcba3-state.sh <outfile>
# Prints, for the eight entries of issue f79bcba3 wherever they live under
# dev/active/: the `--check` result of every live closure generator, the
# relative Cargo path dependencies that name or leave an entry, and each
# line of entry code that counts directory levels.
set -u
cd "$(git rev-parse --show-toplevel)"
entries='04b85d10|07ca8585|12fdeb5b|1c602857|1d0da41f|1d4fd63d|2037941f-profile-and-optimize-mid-range-buffer-operations|3be770d5'
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
  git grep -n -E 'parents\[[0-9]\]|\.\./\.\.|ancestors\(\)|"\.\./[a-z0-9]' -- ':(glob)dev/active/**/*.py' ':(glob)dev/active/**/*.sh' ':(glob)dev/active/**/*.rs' ':(exclude,glob)**/inputs/**' |
    grep -E "^dev/active/([^/]*/)?($entries)/"
} > "$1"
