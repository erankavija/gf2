#!/usr/bin/env bash
# Whole-process counters include setup and warm-up; they are not decode-region counters.
set -euo pipefail
here=$(dirname "${BASH_SOURCE[0]}")
python3 -B "$here/arm-profiles.py" session "$1" "$3"
