#!/usr/bin/env python3
"""Validation record of the batch decoder benchmark arms (jit:5d590bac).

Drives each arm of the build record in OUT_DIR with its prepared quality file through the
runner's staged smoke on every cell of `FAMILIES` at a batch of every recorded frame; the arm
fails when a worker breaks its placement or decodes other bit errors than the file records
[Cassagne2019]. Each family stages under its own directory, which a rerun resumes and the runner
re-exports, so no completed dispatch repeats; the stages are removed once the record is written.

Usage (from the worktree root): record-arm-validation.py OUT_DIR
"""

import importlib.util
import json
import pathlib
import shutil
import subprocess
import sys

sys.dont_write_bytecode = True
PEAK = pathlib.Path(__file__).resolve().with_name("record-peak-memory.py")
peak = importlib.util.module_from_spec(importlib.util.spec_from_file_location("peak", PEAK))
peak.__spec__.loader.exec_module(peak)
smoke = peak.smoke

RECORD = "validation.jsonl"
FAMILIES = ("single-worker", "multicore")
SCRATCH = pathlib.Path("target/arm-validation-scratch")
GROUP = {"issue": "5d590bac", "subject": "validation of the benchmark arms", "arm_kind": None}
REMOVED = SCRATCH.with_name(SCRATCH.name + ".removed")
# Exit status of a staged smoke that paused at the plan's cells-per-session budget.
PAUSED = 3


def staged_smoke(runner, environment, plan, stage, record):
    """Runs, resumes or re-exports the plan's staged smoke until it writes `record`."""
    command = [runner, "smoke", str(plan), "--stage", str(stage), "--record", str(record)]
    while (status := subprocess.run(command, env=environment).returncode) == PAUSED:
        pass
    if status != 0:
        sys.exit(f"{plan}: the staged smoke exited {status}")


def write_record(out_dir, rows, name=RECORD):
    """Writes the record beside itself and renames it into place."""
    out_dir.mkdir(parents=True, exist_ok=True)
    partial = out_dir / f".{name}.partial"
    partial.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in rows),
                       encoding="utf-8")
    partial.replace(out_dir / name)


def remove_stages(scratch=None):
    """Removes the stages as a whole, so an interrupted removal leaves none half-removed."""
    scratch = scratch or SCRATCH
    removed = REMOVED if scratch == SCRATCH else scratch.with_name(scratch.name + ".removed")
    scratch.rename(removed)
    shutil.rmtree(removed)


def lines(build_path, family, plans, qualities, runner, environment):
    """The record lines of one family's smoke over each arm's plan."""
    rows = []
    for (name, key), (plan, arm, cells) in sorted(plans.items()):
        stage, record = plan.with_suffix(".stage"), plan.with_suffix(".smoke.json")
        staged_smoke(runner, environment, plan, stage, record)
        validated = {cell["cell_id"]: cell for cell in
                     json.loads(record.read_text(encoding="utf-8"))["cells"]}
        batches = {cell["cell_id"]: cell["case"]["batch_size"] for cell in
                   json.loads(plan.read_text(encoding="utf-8"))["cells"]}
        journalled = {row["cell_id"]: row for row in smoke.diagnostics(stage)
                      if row["arm"] == arm}
        for cell_id in cells:
            entry = next(entry for entry in validated[cell_id]["arms"] if entry["arm"] == arm)
            reports = {kind: value for kind, value in journalled[cell_id].items()
                       if kind not in ("cell_id", "arm")}
            if smoke.PLACEMENT not in reports:
                sys.exit(f"{arm} {cell_id}: the arm wrote no placement report")
            rows.append({
                "arm": name,
                "build_identity": peak.cited(build_path),
                "code": key,
                "family": family,
                "cell_id": cell_id,
                "workers": len(reports[smoke.PLACEMENT]["workers"]),
                "batch_frames": batches[cell_id],
                "quality": qualities[name, key],
                "validation": entry,
                **reports,
            })
    return rows


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    if pathlib.Path.cwd() != smoke.ROOT:
        sys.exit("invoke from the worktree root")
    out_dir = pathlib.Path(sys.argv[1]).resolve()
    build_path, build = peak.build_record(out_dir)
    peak.catalogued(build)
    files = peak.quality_files(build)
    rows = {(name, key): row for name, row in build["arms"].items() for key in smoke.CODES}
    batches, qualities = {}, {}
    for arm in rows:
        path = smoke.ROOT / files[arm][0]
        if peak.sha256_file(path) != files[arm][1]:
            sys.exit(f"{path} differs from its pinned digest")
        batches[arm] = json.loads(path.read_text(encoding="utf-8"))["frames"]
        qualities[arm] = {"path": str(files[arm][0]), "sha256": files[arm][1]}
    # A cleanup the last run left unfinished; its record was written.
    shutil.rmtree(REMOVED, ignore_errors=True)
    runner, environment = smoke.build_runner()
    record = []
    for family in FAMILIES:
        plans = peak.arm_plans(rows, files, batches, GROUP, family, f"validation-{family}",
                               SCRATCH / family)
        record += lines(build_path, family, plans, qualities, runner, environment)
    write_record(out_dir, record)
    remove_stages()
    print(out_dir / RECORD)


if __name__ == "__main__":
    main()
