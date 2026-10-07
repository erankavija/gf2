#!/usr/bin/env bash
# usage: b5b336e5-baseline.sh <outfile>
# Runs the launcher test of the candidate entry and the `--check` of the
# logical closure generator, wherever both live, and writes each command's
# output and exit status. `<entry>` and `<survey>` stand for the directories
# of the two located scripts.
set -u
cd "$(git rev-parse --show-toplevel)"
live() { git ls-files -- ":(glob)**/$1" ':(exclude,glob)**/inputs/**'; }
{
  echo '$ bash <entry>/run-candidate.test.sh'
  bash "$(live run-candidate.test.sh)" 2>&1
  echo "exit=$?"
  echo '$ python3 -B <survey>/make-logical-producing-inputs.py --check'
  python3 -B "$(live make-logical-producing-inputs.py)" --check 2>&1
  echo "exit=$?"
} > "$1"
