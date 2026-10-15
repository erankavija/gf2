#!/usr/bin/env python3
"""Untimed smoke of a protocol family's plan through the shared runner (jit:2a69b276).

Fails when an arm opens a timing window; the scratch stage is keyed by family so a rerun
resumes it.

Usage (from the worktree root): smoke-family.py DECLARATION RECORD
"""

import importlib.util
import json
import pathlib
import shutil
import subprocess
import sys

sys.dont_write_bytecode = True


def load(name):
    path = pathlib.Path(__file__).resolve().with_name(name)
    module = importlib.util.module_from_spec(
        importlib.util.spec_from_file_location(path.stem.replace("-", "_"), path))
    module.__spec__.loader.exec_module(module)
    return module


validation, generator = load("record-arm-validation.py"), load("make-family.py")
smoke = validation.smoke
SCRATCH = pathlib.Path("target/family-smoke")


def timed(record):
    return [f"{cell['cell_id']}/{entry['arm']}" for cell in record["cells"]
            for entry in cell["arms"] if entry["windows"] != 0]


def smoke_plan(plan, scratch, record):
    runner, environment = smoke.build_runner()
    subprocess.run([runner, "check", str(plan)], check=True, env=environment)
    validation.staged_smoke(runner, environment, plan, scratch / "stage", record)
    observed = json.loads(record.read_text(encoding="utf-8"))
    if opened := timed(observed):
        sys.exit(f"the smoke opened a timing window on {', '.join(opened)}")
    return observed


def smoke_family(declaration, out):
    declaration = pathlib.Path(declaration)
    family = json.loads(declaration.read_text(encoding="utf-8"))["family"]["id"]
    scratch = smoke.ROOT / SCRATCH / family
    shutil.rmtree(scratch.with_name(scratch.name + ".removed"), ignore_errors=True)
    scratch.mkdir(parents=True, exist_ok=True)
    generator.generate(declaration, scratch)
    observed = smoke_plan(scratch / generator.PLAN, scratch, scratch / "smoke.json")
    out = pathlib.Path(out).resolve()
    validation.write_record(out.parent, [{"family": family, "smoke": observed}], out.name)
    validation.remove_stages(scratch)
    return out


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    if pathlib.Path.cwd() != smoke.ROOT:
        sys.exit("invoke from the worktree root")
    print(smoke_family(*sys.argv[1:]))


if __name__ == "__main__":
    main()
