#!/usr/bin/env bash
# Build/stage outside the mutex; the Rust driver owns all campaign mechanics.
# Usage: tuning-extent-campaign.sh /absolute/stage [campaign-id]
set -euo pipefail
if [[ $# -lt 1 || $# -gt 2 || $1 != /* ]]; then
  echo 'usage: tuning-extent-campaign.sh /absolute/stage [campaign-id]' >&2
  exit 2
fi
repo=$(git rev-parse --show-toplevel)
cd "$repo"
stage=$1
mkdir -p "$stage"
stage=$(realpath "$stage")
lock=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
touch "$lock"
lock=$(realpath "$lock")
export GF2_CCX1_LOCK="$lock"
export RUSTUP_TOOLCHAIN=1.95.0
export RAYON_NUM_THREADS=4
export CARGO_CI_NO_SCCACHE=1
pending=null
if [[ -x "$stage/bin/driver" ]]; then
  pending=$("$stage/bin/driver" discover-preparation "$stage")
fi
identity=$(python3 - "$stage" "$pending" <<'PY_IDENTITY'
import json,pathlib,sys
stage=pathlib.Path(sys.argv[1])
pending=json.loads(sys.argv[2])
if pending:
    print(pending['campaign_id']); print(pending['session_id']); print('staged')
elif (stage/'campaign.json').exists():
    print(json.loads((stage/'campaign.json').read_text())['campaign_id']); print(''); print('staged')
else:
    print(''); print(''); print('build')
PY_IDENTITY
)
mapfile -t identity_lines <<< "$identity"
campaign=${identity_lines[0]:-${2:-gf2-a83583e0-$(date -u +%Y%m%dT%H%M%S)-$$}}
session=${identity_lines[1]:-session-$(date -u +%Y%m%dT%H%M%S)-$$}
if [[ $# == 2 && $2 != "$campaign" ]]; then
  echo 'requested campaign ID differs from the immutable stage' >&2
  exit 2
fi
if [[ ${identity_lines[2]} == staged ]]; then
  "$stage/bin/driver" prepare-session "$stage" "$campaign" "$session" "$lock"
else
  mkdir -p "$stage/build"
  echo "GF2_CAMPAIGN_BUILD_LOG=$stage/build/build.log"
  ./scripts/cargo-budget.sh cargo +1.95.0 build --release -p tuning-campaign-support --bin tuning-extent-campaign-driver --message-format=json >"$stage/build/driver.jsonl" 2>>"$stage/build/build.log"
  ./scripts/cargo-budget.sh cargo +1.95.0 build --release -p gf2-core --bench tuning_calibration --features parallel,simd,tuning-profile,test-support --message-format=json >"$stage/build/core.jsonl" 2>>"$stage/build/build.log"
  ./scripts/cargo-budget.sh cargo +1.95.0 build --release -p gf2-algebra --bench tuning_calibration --features parallel,simd,tuning-profile,test-support --message-format=json >"$stage/build/algebra.jsonl" 2>>"$stage/build/build.log"
  ./scripts/cargo-budget.sh cargo +1.95.0 build --release --manifest-path dev/tools/tuning-profile-compose/Cargo.toml --message-format=json >"$stage/build/composer.jsonl" 2>>"$stage/build/build.log"
  driver_executable=$(python3 - "$stage" <<'PY_BUILD'
import hashlib, json, os, pathlib, sys
stage=pathlib.Path(sys.argv[1])
executables={}
for source,dest,target in [('driver','driver','tuning-extent-campaign-driver'),('core','core-producer','tuning_calibration'),('algebra','algebra-producer','tuning_calibration'),('composer','composer','tuning-profile-compose')]:
    rows=[json.loads(line) for line in (stage/'build'/f'{source}.jsonl').read_text().splitlines()]
    bins={row['executable'] for row in rows if row.get('reason')=='compiler-artifact' and row.get('target',{}).get('name')==target and row.get('executable')}
    if len(bins)!=1: raise SystemExit(f'expected one executable for {source}: {bins}')
    path=pathlib.Path(next(iter(bins))).resolve(strict=True)
    executables[dest]={'path':str(path),'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}
manifest={'schema':'tuning-campaign-staging-v1','executables':dict(sorted(executables.items()))}
# This build-discovery input is expendable. The driver publishes and replays
# the authoritative staging manifest and executable bytes through shared intents.
with open(stage/'build'/'staging-input.json','w') as output:
    output.write(json.dumps(manifest,separators=(',',':')))
    output.flush(); os.fsync(output.fileno())
print(executables['driver']['path'])
PY_BUILD
)
  "$driver_executable" prepare-session "$stage" "$campaign" "$session" "$lock" "$stage/build/staging-input.json"
fi
# The wrapper is the sole full-host mutex. Its child performs no builds.
trap ' : ' INT TERM
set +e
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 "$repo/dev/scripts/ccx1-bench-flock.sh" --full-host "$stage/bin/driver" run-session "$stage" "$session"
wrapper_exit=$?
trap - INT TERM
set -e
"$stage/bin/driver" finalize-session "$stage" "$session" "$wrapper_exit"
