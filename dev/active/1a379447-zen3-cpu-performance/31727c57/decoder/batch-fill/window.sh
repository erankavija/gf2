#!/usr/bin/env bash
# Usage from the repository root: bash window.sh [--untimed]
set -euo pipefail
locator=$(git ls-files -- ':(glob)**/repository_files.py' ':(exclude,glob)**/inputs/**')
launcher=$(python3 -B "$locator" file 31727c57/batch_fill.py)
exec python3 -B "$launcher" run "$@"
