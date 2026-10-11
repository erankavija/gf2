#!/usr/bin/env bash
# usage: cc060974-scripts.sh <outfile>
# Lists every line of a tracked .py or .sh file of the entries 00dd43c3 and
# c04dd4ac-zen3-shifts-and-permutations, outside inputs/ snapshot directories,
# that names the script location, the repository root, a directory step, the
# working directory, a lookup through repository_files or an active path.
# Each distinct line is printed once, after the scripts that hold it, named
# <entry id>:<path below survey/>; the last line names the scripts that hold
# no such line.
set -u
cd "$(git rev-parse --show-toplevel)"
entries='00dd43c3|c04dd4ac-zen3-shifts-and-permutations'
net='__file__|BASH_SOURCE|show-toplevel|dirname|relative-to|\.parent|parents\[|pardir|\.\./|cwd|chdir|(^|[ (])cd |repository_files|dev/active'
epic=$(git ls-files -- ':(glob)**/00dd43c3/**' | grep -v '/inputs/' | sed -n -E '1s#/00dd43c3/.*##p')
git ls-files -- "$epic" | grep -E "^$epic/($entries)/.*\.(py|sh)\$" | grep -v '/inputs/' |
while read -r f; do
  n=$(sed -E 's#^([0-9a-f]+)[^/]*/survey/#\1:#' <<<"${f#"$epic"/}")
  grep -E "$net" "$f" | sed -E "s#^[[:space:]]+##; s#^#$n\t#" | sort -u
  grep -qE "$net" "$f" || printf '%s\t\n' "$n"
done | awk -F'\t' '{a[$2] = a[$2] (a[$2] ? " " : "") $1} END {for (k in a) if (k != "") print a[k] ": " k; print a[""] ": no such line"}' |
  sort > "$1"
