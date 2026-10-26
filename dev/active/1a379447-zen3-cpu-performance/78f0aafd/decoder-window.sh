#!/usr/bin/env bash
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
[[ "${GF2_BENCH_WINDOW:-}" == 1 ]] || { echo 'profiles require GF2_BENCH_WINDOW=1' >&2; exit 2; }
files=$(git ls-files -- ':(glob)**/repository_files.py' ':(exclude,glob)**/inputs/**')
locate() { python3 -B "$files" file "$1"; }
here=$(dirname "$(locate 78f0aafd/decoder-window.sh)")
record="$here/preparation/build-identity.json"
out="$here/decoder-profile"
bash "$(locate ed3d490e/run-profile.sh)" --record "$record" \
    --arms-record "$(locate e5f8b3ae/native/build-identity.json)" "$here/decoder-cases.tsv" "$out"
mapfile -t binary < <(python3 - "$record" <<'PY'
import json,sys
r=json.load(open(sys.argv[1]))
print(r['isolation']['target_directory']+'/release/ldpc-batch-profile')
print(r['executables']['ldpc-batch-profile']['sha256'])
PY
)
python3 -B "$(locate 3be770d5/survey/summarize-profile.py)" "$out" \
    --json "$out/profile-summary.json" --markdown "$out/profile.md" \
    --binary "${binary[0]}" --binary-sha256 "${binary[1]}"
