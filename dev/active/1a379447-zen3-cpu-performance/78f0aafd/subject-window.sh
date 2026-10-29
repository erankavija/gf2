#!/usr/bin/env bash
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
[[ "${GF2_BENCH_WINDOW:-}" == 1 ]] || { echo 'profiles require GF2_BENCH_WINDOW=1' >&2; exit 2; }
subject=${1:?subject is required}
files=$(git ls-files -- ':(glob)**/repository_files.py' ':(exclude,glob)**/inputs/**')
locate() { python3 -B "$files" file "$1"; }
here=$(dirname "$(locate 78f0aafd/subject-window.sh)")
source "$(locate 3be770d5/survey/profile-series.sh)"
series_cases() { python3 -B "$here/arm-profiles.py" cases "$subject"; }
series_identity() { python3 -B "$here/arm-profiles.py" identity "$1"; }
profile_series "$here/$subject-profile" "$here" "$here/arm-session.sh" 9 "$subject whole-process profile"
python3 -B "$here/make-accounting.py" --summarize-subject "$subject"
python3 -B "$here/make-accounting.py"
