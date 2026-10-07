#!/usr/bin/env bash
# usage: 2e18b37c-scripts.sh <outfile>
# Prints, per tracked shell or Python script of the entries f8dd4dde, a203a23c
# and 19513245 outside inputs/ snapshot directories, each line that takes the
# script's own location, asks git for the repository root, names
# repository_files or names the directory that holds the epic directories.
set -u
cd "$(git rev-parse --show-toplevel)"
entries='f8dd4dde|a203a23c|19513245'
epic=$(git ls-files -- ':(glob)**/a203a23c/**' | grep -v '/inputs/' | sed -n -E '1s#/a203a23c/.*##p')
git ls-files -- ":(glob)$epic/**/*.py" ":(glob)$epic/**/*.sh" | grep -E "^$epic/($entries)/" | grep -v '/inputs/' |
while read -r s; do
  echo "== ${s#"$epic"/}"
  grep -E "__file__|BASH_SOURCE|show-toplevel|repository_files|$(dirname "$epic")/" "$s" | sed -E 's/^ +//'
done > "$1"
