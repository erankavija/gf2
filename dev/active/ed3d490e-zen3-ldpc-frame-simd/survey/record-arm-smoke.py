#!/usr/bin/env python3
"""Runner smoke record of the batch decoder benchmark arms (jit:a4476c6a).

Drives each arm through `benchmark-ab-runner smoke` over a throwaway plan of the single-worker
family's own cell on every recorded code, one untimed dispatch in the validation position and no
timing window [Cassagne2019]. The record names each arm by the digest of the build identity
record and states the runner's entry for the arm. Each arm stages under its own directory, which
a rerun resumes and the runner re-exports, so no completed dispatch repeats; the stages are
removed once the record is written. The plan builder, staged smoke, atomic write and stage
removal are those of `record-arm-validation.py`.

Usage (from the worktree root): record-arm-smoke.py OUT_DIR
"""

import importlib.util
import json
import pathlib
import shutil
import sys

sys.dont_write_bytecode = True
VALIDATION = pathlib.Path(__file__).resolve().with_name("record-arm-validation.py")
validation = importlib.util.module_from_spec(
    importlib.util.spec_from_file_location("validation", VALIDATION))
validation.__spec__.loader.exec_module(validation)
peak, smoke = validation.peak, validation.smoke

RECORD = "smoke.json"
FAMILY = "single-worker"
SCRATCH = pathlib.Path("target/arm-smoke-scratch")
GROUP = {"issue": "a4476c6a", "subject": "runner smoke of the benchmark arms", "arm_kind": None}


def entries(build_path, plans, runner, environment):
    """The record entry of each arm: the runner's smoke entry of its one dispatch."""
    rows = []
    for (name, key), (plan, arm, cells) in sorted(plans.items()):
        stage, record = plan.with_suffix(".stage"), plan.with_suffix(".smoke.json")
        validation.staged_smoke(runner, environment, plan, stage, record)
        (cell_id,) = cells
        cell = next(cell for cell in json.loads(record.read_text(encoding="utf-8"))["cells"]
                    if cell["cell_id"] == cell_id)
        entry = next(entry for entry in cell["arms"] if entry["arm"] == arm)
        if (entry["role"], entry["windows"]) != ("validation", 0):
            sys.exit(f"{arm}: the smoke opened a timing window")
        rows.append({"arm": name, "build_identity": peak.cited(build_path), "code": key,
                     "cell_id": cell_id, "smoke": entry})
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
    declared = peak.family_batches(FAMILY)
    batches = {}
    for arm in rows:
        path = smoke.ROOT / files[arm][0]
        if peak.sha256_file(path) != files[arm][1]:
            sys.exit(f"{path} differs from its pinned digest")
        # The quality file states the frames per SIMD wave the arm needs.
        settings = json.loads(path.read_text(encoding="utf-8"))["settings"]
        batches[arm] = max(declared[arm[1]], settings["batch_size"])
    # A cleanup the last run left unfinished; its record was written.
    shutil.rmtree(SCRATCH.with_name(SCRATCH.name + ".removed"), ignore_errors=True)
    runner, environment = smoke.build_runner()
    plans = peak.arm_plans(rows, files, batches, GROUP, FAMILY, "arm-smoke", SCRATCH)
    record = {"cites": ["Cassagne2019"], "arms": entries(build_path, plans, runner, environment)}
    validation.write_record(out_dir, [record], RECORD)
    validation.remove_stages(SCRATCH)
    print(out_dir / RECORD)


if __name__ == "__main__":
    main()
