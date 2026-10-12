#!/usr/bin/env bash
# Untimed rehearsal of the decoder calibration campaign (jit:0a77ecfc).
#
# Usage: rehearse-campaign.sh, with no arguments, from a clean checkout
#
# Runs the campaign launcher end to end over a throwaway stage in a throwaway
# clone of HEAD, then the validator over the stage and its publication, and
# writes rehearsal/record.json. The clone commits two stand-ins and nothing
# else differs from HEAD:
#   rehearsal/untimed-windows.patch  a timed child dispatches each window once
#                                    and reports constants; no clock is read
#   rehearsal/private-lock.sh        replaces the host mutex wrapper and holds
#                                    a lock file of the rehearsal
# The clone, its lock and its build cache stay under the checkout's target/;
# the launcher fixes the stage at /tmp/<run-id>. A stage the validator accepts
# is removed with the clone; a failed rehearsal keeps both for inspection.
set -euo pipefail
issue=0a77ecfc
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(git -C "$here" rev-parse --show-toplevel)
calibration=${here#"$repo"/}
cd "$repo"
if [[ -n $(git status --porcelain --untracked-files=all) ]]; then
  echo 'the rehearsal clones HEAD; commit or remove local changes first' >&2
  exit 2
fi
shopt -s nullglob
existing=(/tmp/gf2-"$issue"-*)
if [[ ${#existing[@]} -ne 0 ]]; then
  echo "a stage of $issue exists: ${existing[*]}" >&2
  exit 2
fi
revision=$(git rev-parse HEAD)
work=$repo/target/calibration-rehearsal
clone=$work/checkout
if [[ -e $clone ]]; then
  echo "an earlier rehearsal clone remains at $clone; inspect and remove it" >&2
  exit 2
fi
mkdir -p "$work"
: > "$work/host.lock"
git clone --quiet --no-checkout "$repo" "$clone"
git -C "$clone" -c advice.detachedHead=false checkout --quiet --detach "$revision"
locate() {
  git -C "$clone" ls-files -- ":(glob)**/$1" ':(exclude,glob)**/inputs/**'
}
lock_wrapper=$(locate ccx1-bench-flock.sh)
launcher=$(locate tuning-extent-campaign.sh)
validator=$(locate validate-tuning-extent-campaign.py)
git -C "$clone" apply "$here/rehearsal/untimed-windows.patch"
install -m 755 "$here/rehearsal/private-lock.sh" "$clone/$lock_wrapper"
git -C "$clone" -c user.name=rehearsal -c user.email=rehearsal@localhost \
  commit --quiet --all --message "chore(jit:$issue): apply the rehearsal stand-ins"
status=0
(
  cd "$clone"
  env -u GF2_BENCH_WINDOW GF2_CCX1_LOCK="$work/host.lock" CARGO_TARGET_DIR="$repo/target" \
    nice -n 19 "./$launcher" "$issue"
) > "$work/launcher.log" 2>&1 || status=$?
stages=(/tmp/gf2-"$issue"-*)
if [[ ${#stages[@]} -ne 1 ]]; then
  echo "the launcher exited $status and left ${#stages[@]} stages; see $work/launcher.log" >&2
  exit 1
fi
stage=${stages[0]}
validation=0
(
  cd "$clone"
  python3 -B "$validator" --stage "$stage" > "$work/stage-validation.txt" 2> "$work/stage-validation.err"
  python3 -B "$validator" --stage "$stage" --publication "$clone" \
    > "$work/publication-validation.txt" 2> "$work/publication-validation.err"
) || validation=$?
if [[ $status -ne 0 || $validation -ne 0 ]]; then
  echo "rehearsal failed: launcher $status, validator $validation; stage $stage and $work are kept" >&2
  exit 1
fi
python3 -B - "$stage" "$clone" "$work" "$revision" "$calibration" "$issue" <<'PY_RECORD'
import collections, hashlib, json, pathlib, subprocess, sys
stage, clone, work = map(pathlib.Path, sys.argv[1:4])
revision, calibration, issue = sys.argv[4:7]
PREFIX = "GF2_TUNING_VALIDATION="
sha256 = lambda path: hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()
def validation(name):
    (line,) = (work / name).read_text().splitlines()
    return json.loads(line.removeprefix(PREFIX))
config = json.loads((stage / "campaign.json").read_text())
events = collections.Counter()
active_ns = 0
for line in (stage / "execution.log").read_text().splitlines():
    record = json.loads(line)
    events[record["event"]] += 1
    evidence = (record["details"].get("session_transition", {}).get("transition", {})
                .get("evidence", {}))
    if record["event"] == "work-finished":
        active_ns += evidence["active_elapsed_ns"]
published = subprocess.run(
    ["git", "-C", str(clone), "status", "--porcelain", "--untracked-files=all"],
    capture_output=True, check=True, text=True,
).stdout.splitlines()
record = {
    "issue": issue,
    "source_revision": revision,
    "stand_ins": {
        f"{calibration}/rehearsal/{name}": sha256(clone / calibration / "rehearsal" / name)
        for name in ("private-lock.sh", "untimed-windows.patch")
    },
    "campaign_id": config["campaign_id"],
    "staged_executables_sha256": {
        process["id"]: process["executable_sha256"] for process in config["processes"]
    },
    "journal_events": dict(sorted(events.items())),
    "active_seconds": active_ns / 1e9,
    "stage_validation": validation("stage-validation.txt"),
    "publication_validation": validation("publication-validation.txt"),
    "published_paths": sorted(line[3:].replace(config["campaign_id"], "<run-id>")
                              for line in published),
}
output = pathlib.Path(calibration) / "rehearsal" / "record.json"
output.write_text(json.dumps(record, indent=1) + "\n")
print(f"wrote {output}")
PY_RECORD
# Both directories are this run's own: the stage carries the campaign ID the
# record names, and the clone was created above. The journal outlives them
# beside the launcher log.
cp "$stage/execution.log" "$stage/campaign.json" "$work/"
rm -rf -- "$stage" "$clone"
