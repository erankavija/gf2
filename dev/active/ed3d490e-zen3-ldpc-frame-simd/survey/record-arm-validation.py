#!/usr/bin/env python3
"""Validation record of the batch decoder benchmark arms (jit:5d590bac).

Times nothing. Drives each arm of the build record, with its prepared quality file, through
`benchmark-ab-runner smoke` on every cell of the smoke families `FAMILIES`, at a per-worker
batch of every frame the file records, and writes `validation.jsonl`: per arm, code and cell,
the arm's entry of the runner's smoke record with the placement report and any decoder
diagnostics the stage journalled. The arm fails the smoke when a worker breaks its placement or
decodes a frame to other bit errors than the file records. The AFF3CT [Cassagne2019] arms take
the quality files `record-peak-memory.py` resolves.

Each family's stages live under their own directory of `SCRATCH`. A rerun after an interruption
resumes an unfinished stage and reuses a complete one whose plan and executables are unchanged;
the stages are removed once the record is written.

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
# Exit status of a staged smoke that paused at the plan's cells-per-session budget.
PAUSED = 3


def complete(stage):
    log = stage / "execution.log"
    return log.exists() and any(json.loads(line)["event"] == "complete"
                                for line in log.read_text(encoding="utf-8").splitlines())


def staged_smoke(runner, environment, plan, stage, record):
    """Runs or resumes the plan's staged smoke until it completes and writes `record`.

    The runner refuses to resume a complete stage, so one is reused when it holds this plan and
    its smoke record names the executables now on disk.
    """
    if complete(stage):
        validated = json.loads(record.read_text(encoding="utf-8")) if record.exists() else None
        if (validated is None or (stage / "plan.json").read_bytes() != plan.read_bytes()
                or any(peak.sha256_file(smoke.ROOT / entry["executable"])
                       != entry["executable_sha256"]
                       for cell in validated["cells"] for entry in cell["arms"])):
            sys.exit(f"{stage} is complete for another plan or build; remove it to validate again")
        return
    command = [runner, "smoke", str(plan), "--stage", str(stage), "--record", str(record)]
    while (status := subprocess.run(command, env=environment).returncode) == PAUSED:
        pass
    if status != 0:
        sys.exit(f"{plan}: the staged smoke exited {status}")


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
    build_path, build = peak.build_record()
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
    runner, environment = smoke.build_runner()
    record = []
    for family in FAMILIES:
        plans = peak.arm_plans(rows, files, batches, GROUP, family, f"validation-{family}",
                               SCRATCH / family)
        record += lines(build_path, family, plans, qualities, runner, environment)
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / RECORD).write_text("".join(json.dumps(row, sort_keys=True) + "\n"
                                          for row in record), encoding="utf-8")
    shutil.rmtree(SCRATCH)
    print(out_dir / RECORD)


if __name__ == "__main__":
    main()
