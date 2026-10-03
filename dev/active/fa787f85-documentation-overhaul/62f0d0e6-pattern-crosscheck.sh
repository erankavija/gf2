#!/bin/sh
# Per-crate count of comment lines matching the sweep pattern, via git grep
# (PCRE, case-insensitive) on the committed tree of the commit in $1 (default
# HEAD); independent of 62f0d0e6-comment-census.py. Comment line: first
# non-blank characters `//`.
set -eu
P='non-goal|out of scope|not in scope|future work|deferred to|follow-on|phase [a-e]\b|wave [a-z0-9]|(task|issue|story) `?[0-9a-f]{8}|previously|no longer|legacy|migrat'
C=$(git rev-parse "${1:-HEAD}")
echo "commit: $C"
git ls-tree --name-only "$C" crates/ | sort | while read -r d; do
  n=$(git grep -I -P -i -c "^\\s*//.*($P)" "$C" -- "${d%/}/*.rs" | awk -F: '{s+=$NF} END{print s+0}')
  echo "${d%/} $n"
done
