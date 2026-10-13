#!/usr/bin/env bash
# Untimed rehearsal of the decoder calibration campaign (jit:0a77ecfc).
#
# Usage: rehearse-campaign.sh [record name under rehearsal/]
#
# Runs the queue launcher over a throwaway stage in a clone of HEAD, which
# holds no uncommitted change of the checkout, and that differs by the two
# stand-ins under rehearsal/: timed children report
# constant samples without reading a clock, and a private lock file replaces
# the host mutex. A failed rehearsal keeps its stage and clone.
set -euo pipefail
issue=0a77ecfc
record=${1:-record.json}
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(git -C "$here" rev-parse --show-toplevel)
calibration=${here#"$repo"/}
cd "$repo"
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
validator=$(locate validate-tuning-extent-campaign.py)
git -C "$clone" apply "$here/rehearsal/untimed-windows.patch"
install -m 755 "$here/rehearsal/private-lock.sh" "$clone/$lock_wrapper"
git -C "$clone" -c user.name=rehearsal -c user.email=rehearsal@localhost \
  commit --quiet --all --message "chore(jit:$issue): apply the rehearsal stand-ins"
launch() {
  (
    cd "$clone"
    env -u GF2_BENCH_WINDOW GF2_CCX1_LOCK="$work/host.lock" CARGO_TARGET_DIR="$repo/target" \
      nice -n 19 "./$calibration/run-campaign.sh"
  ) >> "$work/launcher.log" 2>&1
}
: > "$work/launcher.log"
status=0
launch || status=$?
stages=(/tmp/gf2-"$issue"-*)
if [[ ${#stages[@]} -ne 1 ]]; then
  echo "the launcher exited $status and left ${#stages[@]} stages; see $work/launcher.log" >&2
  exit 1
fi
stage=${stages[0]}
resumed=0
launch || resumed=$?
validation=0
(
  cd "$clone"
  python3 -B "$validator" --stage "$stage" > "$work/stage-validation.txt" 2> "$work/stage-validation.err"
  python3 -B "$validator" --stage "$stage" --publication "$clone" \
    > "$work/publication-validation.txt" 2> "$work/publication-validation.err"
) || validation=$?
if [[ $status -ne 0 || $resumed -ne 0 || $validation -ne 0 ]]; then
  echo "rehearsal failed: launcher $status, resumed $resumed, validator $validation;" \
    "stage $stage and $work are kept" >&2
  exit 1
fi
python3 -B - "$stage" "$clone" "$work" "$revision" "$calibration" "$issue" "$status" "$resumed" "$record" <<'PY_RECORD'
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
owner = json.loads(next(
    clone / line[3:] for line in published
    if line[3:].startswith("crates/") and line.endswith(f"{config['campaign_id']}.json")
).read_text())
record = {
    "issue": issue,
    "source_revision": revision,
    "stand_ins": {
        f"{calibration}/rehearsal/{name}": sha256(clone / calibration / "rehearsal" / name)
        for name in ("private-lock.sh", "untimed-windows.patch")
    },
    "campaign_id": config["campaign_id"],
    "launcher_exit": {"first": int(sys.argv[7]), "resumed": int(sys.argv[8])},
    "staged_executables_sha256": {
        process["id"]: process["executable_sha256"] for process in config["processes"]
    },
    "journal_events": dict(sorted(events.items())),
    "active_seconds": active_ns / 1e9,
    "stage_validation": validation("stage-validation.txt"),
    "publication_validation": validation("publication-validation.txt"),
    "published_owner_selectors": {
        section: body["selectors"] for section, body in owner["sections"].items()
    },
    "published_paths": sorted(line[3:].replace(config["campaign_id"], "<run-id>")
                              for line in published),
}
output = pathlib.Path(calibration) / "rehearsal" / sys.argv[9]
output.write_text(json.dumps(record, indent=1) + "\n")
print(f"wrote {output}")
PY_RECORD
cp "$stage/execution.log" "$stage/campaign.json" "$work/"
rm -rf -- "$stage" "$clone"
