#!/usr/bin/env bash
# Build/stage outside the mutex; the Rust driver owns all campaign mechanics.
# Usage: tuning-extent-campaign.sh <issue> | <campaign-id>
# An issue (eight lowercase hex digits) selects the one campaign-declaration.json
# in this checkout whose issue field names it and creates a fresh run ID
# gf2-<issue>-<yyyymmddthhmmssz UTC>-<launcher-pid> with its exact /tmp stage.
# Passing a run ID resumes that already-created stage; arbitrary destinations
# are invalid. The ID becomes the emitted profiles' ProfileId, so it stays
# lowercase kebab case. A complete campaign ends by publishing its stage into
# this checkout; resuming a complete campaign only finishes that publication.
set -euo pipefail
usage='usage: tuning-extent-campaign.sh <issue> | <campaign-id>'
if [[ $# -ne 1 ]]; then
  echo "$usage" >&2
  exit 2
fi
argument=$1
requested_campaign=
if [[ $argument =~ ^[0-9a-f]{8}$ ]]; then
  issue=$argument
elif [[ $argument =~ ^gf2-([0-9a-f]{8})-[0-9]{8}t[0-9]{6}z-[1-9][0-9]*$ ]]; then
  issue=${BASH_REMATCH[1]}
  requested_campaign=$argument
else
  echo "$usage; a run ID has the form gf2-<issue>-<yyyymmddthhmmssz UTC>-<launcher-pid>" >&2
  exit 2
fi
repo=$(git rev-parse --show-toplevel)
cd "$repo"
# The declaration names the issue, owners and staged executables; the driver
# and validator locate it by the same rule and verify it in full.
producers=$(python3 - "$issue" <<'PY_DECLARATION'
import json,os,subprocess,sys
listing=subprocess.run(['git','ls-files','-z','--cached','--others','--exclude-standard','--',':(glob)**/campaign-declaration.json'],check=True,capture_output=True).stdout.decode()
candidates=[path for path in listing.split('\0') if path and os.path.isfile(path)]
matches=[declaration for declaration in (json.load(open(path)) for path in candidates) if declaration.get('issue')==sys.argv[1]]
if len(matches)!=1:
    raise SystemExit(f'{len(matches)} campaign declarations name issue {sys.argv[1]}; exactly one must')
declaration=matches[0]
if declaration.get('schema')!='tuning-campaign-declaration-v1':
    raise SystemExit('campaign declaration schema mismatch')
print(' '.join(owner['name'] for owner in declaration['measured_owners']))
PY_DECLARATION
)
if [[ -n $requested_campaign ]]; then
  campaign=$requested_campaign
else
  campaign=gf2-$issue-$(date -u +%Y%m%dt%H%M%Sz)-$$
fi
stage=/tmp/$campaign
stage_preexisted=false
if [[ -e $stage ]]; then
  stage_preexisted=true
fi
if [[ -n $requested_campaign && $stage_preexisted == false ]]; then
  echo 'an explicit campaign ID resumes an existing stage; omit it to create a run' >&2
  exit 2
fi
if [[ -L $stage ]]; then
  echo 'campaign stage must not be a symlink' >&2
  exit 2
fi
mkdir -p "$stage"
if [[ $(realpath "$stage") != "$stage" ]]; then
  echo 'campaign stage must be the exact non-symlink /tmp/<campaign-id> path' >&2
  exit 2
fi
lock=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
touch "$lock"
lock=$(realpath "$lock")
export GF2_CCX1_LOCK="$lock"
export RUSTUP_TOOLCHAIN=1.95.0
export RAYON_NUM_THREADS=4
export CARGO_CI_NO_SCCACHE=1
# publish-campaign exits 3 while the campaign is not complete.
publish() {
  set +e
  "$stage/bin/driver" publish-campaign "$stage"
  publication=$?
  set -e
}
if [[ -x "$stage/bin/driver" && -f "$stage/campaign.json" ]]; then
  publish
  if [[ $publication != 3 ]]; then
    exit "$publication"
  fi
fi
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
campaign=${identity_lines[0]:-$campaign}
session=${identity_lines[1]:-session-$(date -u +%Y%m%dT%H%M%S)-$$}
if [[ -n $requested_campaign && $requested_campaign != "$campaign" ]]; then
  echo 'requested campaign ID differs from the immutable stage' >&2
  exit 2
fi
if [[ ${identity_lines[2]} == staged ]]; then
  "$stage/bin/driver" prepare-session "$stage" "$campaign" "$session" "$lock"
else
  mkdir -p "$stage/build"
  echo "GF2_CAMPAIGN_BUILD_LOG=$stage/build/build.log"
  build_head=$(git rev-parse HEAD)
  build_tree=$(git rev-parse 'HEAD^{tree}')
  build_status=$(git status --porcelain --untracked-files=all)
  if [[ -n $build_status ]]; then
    echo 'measurement build requires a clean source tree before any Cargo command' >&2
    exit 2
  fi
  python3 - "$stage/build/source-before.json" "$build_head" "$build_tree" <<'PY_SOURCE_BEFORE'
import json,os,pathlib,sys
path=pathlib.Path(sys.argv[1])
value={'schema':'tuning-campaign-build-source-v1','phase':'before-build','source_revision':sys.argv[2],'source_tree':sys.argv[3],'porcelain':''}
with path.open('x') as output:
    output.write(json.dumps(value,separators=(',',':')))
    output.flush(); os.fsync(output.fileno())
PY_SOURCE_BEFORE
  ./scripts/cargo-budget.sh cargo +1.95.0 build --release -p tuning-campaign-support --bin tuning-extent-campaign-driver --message-format=json >"$stage/build/driver.jsonl" 2>>"$stage/build/build.log"
  for owner in $producers; do
    ./scripts/cargo-budget.sh cargo +1.95.0 build --release -p "gf2-$owner" --bench tuning_calibration --features parallel,simd,tuning-profile,test-support --message-format=json >"$stage/build/$owner.jsonl" 2>>"$stage/build/build.log"
  done
  ./scripts/cargo-budget.sh cargo +1.95.0 build --release --manifest-path dev/tools/tuning-profile-compose/Cargo.toml --message-format=json >"$stage/build/composer.jsonl" 2>>"$stage/build/build.log"
  after_head=$(git rev-parse HEAD)
  after_tree=$(git rev-parse 'HEAD^{tree}')
  after_status=$(git status --porcelain --untracked-files=all)
  if [[ $after_head != "$build_head" || $after_tree != "$build_tree" || -n $after_status ]]; then
    echo 'source identity moved or became dirty during measurement builds' >&2
    exit 2
  fi
  python3 - "$stage/build/source-after.json" "$after_head" "$after_tree" <<'PY_SOURCE_AFTER'
import json,os,pathlib,sys
path=pathlib.Path(sys.argv[1])
value={'schema':'tuning-campaign-build-source-v1','phase':'after-build','source_revision':sys.argv[2],'source_tree':sys.argv[3],'porcelain':''}
with path.open('x') as output:
    output.write(json.dumps(value,separators=(',',':')))
    output.flush(); os.fsync(output.fileno())
PY_SOURCE_AFTER
  driver_executable=$(python3 - "$stage" $producers <<'PY_BUILD'
import hashlib, json, os, pathlib, sys
stage=pathlib.Path(sys.argv[1])
executables={}
builds=[('driver','driver','tuning-extent-campaign-driver')]
builds+=[(owner,f'{owner}-producer','tuning_calibration') for owner in sys.argv[2:]]
builds+=[('composer','composer','tuning-profile-compose')]
for source,dest,target in builds:
    rows=[json.loads(line) for line in (stage/'build'/f'{source}.jsonl').read_text().splitlines()]
    bins={row['executable'] for row in rows if row.get('reason')=='compiler-artifact' and row.get('target',{}).get('name')==target and row.get('executable')}
    if len(bins)!=1: raise SystemExit(f'expected one executable for {source}: {bins}')
    path=pathlib.Path(next(iter(bins))).resolve(strict=True)
    executables[dest]={'path':str(path),'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}
def identity(path):
    path=path.resolve(strict=True)
    return {'path':str(path),'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}
manifest={'schema':'tuning-campaign-staging-v1','source_before':identity(stage/'build'/'source-before.json'),'source_after':identity(stage/'build'/'source-after.json'),'executables':dict(sorted(executables.items()))}
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
publish
if [[ $publication == 3 ]]; then
  # Paused or budget-exhausted: resume later under the same campaign ID.
  exit 0
fi
exit "$publication"
