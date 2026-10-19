#!/usr/bin/env bash
# Categorizes the 07ca8585 re-sampled profile with the spans of its own
# executable's source revision, into a sibling directory of the series so the
# summary at the series root keeps its bytes.
set -euo pipefail

cd "$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"

SERIES=dev/bench_results/07ca8585/v4-r1-resampled-profile
OUT="${SERIES}/own-source-spans"
mkdir -p "${OUT}"
python3 -B "$(python3 -B "$(git ls-files -- ':(glob)**/repository_files.py' ':(exclude,glob)**/inputs/**')" \
    file 3be770d5/survey/summarize-profile.py)" "${SERIES}" \
    --json "${OUT}/profile-summary.json" --markdown "${OUT}/profile.md" --attribution "${OUT}/attribution.tsv"
