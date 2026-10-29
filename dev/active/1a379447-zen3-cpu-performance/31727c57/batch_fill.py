#!/usr/bin/env python3
"""Build, plan and run the tuned decoder arm's descriptive batch-fill record.

Usage (from the worktree root):
  batch_fill.py build             build the probed tuned arm and write its build record
  batch_fill.py plan              write the descriptive plan and addendum
  batch_fill.py run [--untimed]   run or resume the record; --untimed writes a smoke

Under the invoker ruling of 2026-10-29 the record measures `conversion.batch_fill_ns` of the
probed tuned arm alone on every FI-2 confirmation cell. It compares nothing, opens no ledger and
leaves the confirmation verdicts as recorded. Each execution is one validation dispatch of the
shared runner's staged smoke, which journals the arm's stage probe in the execution log and
resumes without repeating a completed cell. Every replicate cell dispatches the arm twice, so
each confirmation cell has `2 * REPLICATES` executions: ten give the shared median interval
[x(2), x(9)] with coverage 0.979. The decode path is the confirmed tuned arm's apart from the
probe, which runs after the timed call.
"""

import argparse
import copy
import datetime
import json
import os
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
from locate import HERE, ROOT, live, load  # noqa: E402
import prepare  # noqa: E402

DIRECTORY = HERE / "decoder/batch-fill"
TARGET = DIRECTORY / "target"
BUILD_RECORD = DIRECTORY / "arm-build.json"
CONFIRMATION = HERE / "decoder/confirmation"
CAMPAIGN = "31727c57-decoder-batch-fill"
REPLICATES = 5
PACKAGE = "ldpc-batch-tuned-arm"
EXECUTABLE = "gf2-batch-tuned-arm"
ARM = "gf2-batch-tuned-probed-portable"
BUILD = "tuned-portable"
CONFIRMED = "gf2-batch-tuned-portable"
UNTIMED = ROOT / "target/31727c57-batch-fill-untimed"
WINDOW = "GF2_BENCH_WINDOW"
RULING = ("Invoker ruling of 2026-10-29: the tuned arm's report gains the shared stage probe, "
          "and the probed arm alone records batch fill on the FI-2 confirmation cells as a "
          "descriptive record. It compares nothing, makes no ledger reservation and neither "
          "repeats nor alters the confirmation, whose verdicts stand.")


def read(path):
    return json.loads(Path(path).read_text())


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n")


def builder():
    return load("survey/record-gf2-arms-build.py", "arms_builder")


def build():
    """Builds the package alone with the confirmed portable arms' tooling, into `TARGET`."""
    tool = builder()
    arms = tool.located("package-directory", tool.PACKAGE)
    manifest = arms / "Cargo.toml"
    inherited = tool.build_environment(os.environ, "portable")
    configuration = tool.configuration_rows(tool.cargo_configuration(
        ROOT, Path(inherited.get("CARGO_HOME") or Path.home() / ".cargo")))
    rustc = tool.output(["rustc", f"+{tool.TOOLCHAIN}", "--version", "--verbose"], env=inherited)
    host = next(line.removeprefix("host: ") for line in rustc.splitlines()
                if line.startswith("host: "))
    linker = arms / tool.LINKER
    target = TARGET.relative_to(ROOT)
    command = {"environment": tool.environment("portable"), "argv": [
        "./scripts/cargo-budget.sh", "cargo", f"+{tool.TOOLCHAIN}", "build", "--offline",
        "--locked", "--release", "--manifest-path", str(manifest),
        "--config", f"target.{host}.linker={json.dumps(str(linker))}",
        "--config", f"build.target-dir={json.dumps(str(target))}", "--package", PACKAGE]}
    subprocess.run(command["argv"], check=True, env={**inherited, **command["environment"]})
    release = target / "release"
    executable = release / EXECUTABLE
    tool.refuse_newer(executable, [linker])
    confirmed_path = ROOT / live("e5f8b3ae/portable/build-identity.json")
    confirmed = read(confirmed_path)["arms"][CONFIRMED]
    write(BUILD_RECORD, {
        "build_root": str(ROOT),
        "rustc": rustc,
        "cargo": tool.output(["cargo", f"+{tool.TOOLCHAIN}", "--version"], env=inherited),
        "cxx": tool.output([tool.CXX, "--version"], env=inherited).splitlines()[0],
        "linker": {"driver": str(linker), "sha256": tool.digest(linker),
                   "cc": tool.output([str(linker), "--version"], env=inherited).splitlines()[0]},
        "cargo_configuration": configuration,
        "build_commands": [command],
        "release_profile": tool.tomllib.loads(manifest.read_text())["profile"]["release"],
        "executables": {EXECUTABLE: {
            "sha256": tool.digest(executable),
            "written_by": tool.strings(executable, ".comment"),
            "compiler_options": tool.strings(executable, ".GCC.command.line"),
            "inputs": tool.inputs(executable, release / f"{EXECUTABLE}.d",
                                  tool.link_rules(executable), ROOT, ROOT / target)}},
        "arms": {ARM: {"source": "workspace", "executable": EXECUTABLE, "environment": {},
                       "package": PACKAGE, "path": str(executable),
                       "executable_sha256": tool.digest(executable)}},
        "confirmed_arm": {"record": str(confirmed_path.relative_to(ROOT)),
                          "record_sha256": tool.digest(confirmed_path),
                          "arm": CONFIRMED,
                          "executable_sha256": confirmed["executable_sha256"]},
    })
    print(BUILD_RECORD.relative_to(ROOT))


def plan(out, campaign, frozen_utc):
    """The confirmation plan and addendum with the probed arm on both sides of `REPLICATES`
    exploratory copies of each cell, replicate-major so each round spans every cell."""
    source, declared = read(CONFIRMATION / "plan.json"), read(CONFIRMATION / "addendum.json")
    executable = read(BUILD_RECORD)["arms"][ARM]["path"]
    names = {name: name.replace(CONFIRMED, ARM) for name, arm in source["arms"].items()
             if arm["build"] == BUILD}
    arms = {names[name]: {**source["arms"][name], "executable": executable,
                          "description": source["arms"][name]["description"]
                          + ", with the stage probe"} for name in names}
    by_id = {cell["cell_id"]: cell for cell in declared["cells"]}
    cells, declarations = [], []
    for replicate in range(1, REPLICATES + 1):
        for cell in source["cells"]:
            identifier = f"{cell['cell_id']}-r{replicate}"
            arm = names[cell["candidate_arm"]]
            cells.append({**cell, "cell_id": identifier, "baseline_arm": arm,
                          "candidate_arm": arm, "pilot_pairs": None})
            declarations.append({**copy.deepcopy(by_id[cell["cell_id"]]), "cell_id": identifier,
                                 "role": "exploratory",
                                 "builds": {"baseline": BUILD, "candidate": BUILD}})
    addendum = copy.deepcopy(declared)
    addendum["family"].update(
        id=campaign, purpose="decoder-family",
        description=f"{RULING} Each cell of the frozen FI-2 confirmation plan runs as "
        f"{REPLICATES} replicate cells whose baseline and candidate are the probed tuned arm, "
        "each dispatched once in the runner's validation position; every execution reports "
        "conversion.batch_fill_ns from the arm's zero-iteration stage probe. Validation "
        "dispatches open no ledger, so ledger_path names a file the record never writes.")
    addendum["frozen"]["frozen_utc"] = frozen_utc
    addendum["family_wise"]["ledger_path"] = str((out / "ledger.jsonl").relative_to(ROOT))
    addendum["holdout"] = {"required": False, "cells": []}
    addendum["cells"] = declarations
    write(out / "addendum.json", addendum)
    write(out / "plan.json", {
        **source, "campaign_id": campaign, "label": "pilot",
        "addendum": str((out / "addendum.json").relative_to(ROOT)),
        "arms": arms, "cells": cells, "max_cells_per_session": len(source["cells"])})
    return out / "plan.json"


def executable_matches():
    record = read(BUILD_RECORD)["arms"][ARM]
    path = ROOT / record["path"]
    return path.is_file() and builder().digest(path) == record["executable_sha256"]


def run(untimed):
    """Runs the record's sessions until the stage completes; each timed session holds the
    canonical lock through the plan's wrapper."""
    runner_module = load("survey/run-family-campaign.py", "family_runner")
    validation, smoke = runner_module.validation, runner_module.smoke
    if not executable_matches():
        sys.exit(f"the probed arm differs from {BUILD_RECORD.relative_to(ROOT)}; run build")
    prepare.decoder_inputs()
    runner, environment = smoke.build_runner()
    if untimed:
        stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        if not (UNTIMED / "plan.json").exists():
            plan(UNTIMED, f"{CAMPAIGN}-untimed", stamp)
        plan_path, stage, record = (UNTIMED / "plan.json", UNTIMED / "stage",
                                    UNTIMED / "smoke.json")
        command = [runner]
    else:
        plan_path, stage, record = (DIRECTORY / "plan.json", DIRECTORY / CAMPAIGN,
                                    DIRECTORY / "smoke.json")
        for path in (plan_path, DIRECTORY / "addendum.json", BUILD_RECORD):
            if not runner_module.committed(path):
                sys.exit(f"{path.relative_to(ROOT)} is uncommitted or differs from HEAD")
        wrapper, *flags = read(plan_path)["wrapper"].split()
        environment.update(CARGO_CI_NO_LOCK="1", GF2_CCX1_LOCK=read(plan_path)["lock_path"])
        command = [str(ROOT / wrapper), *dict.fromkeys([*flags, "--full-host"]), runner]
    plan_path, stage, record = (path.relative_to(ROOT) for path in (plan_path, stage, record))
    print(f"GF2_CAMPAIGN_EXECUTION_LOG={stage / 'execution.log'}", flush=True)
    command += ["smoke", str(plan_path), "--stage", str(stage), "--record", str(record)]
    while (status := subprocess.run(command, env=environment).returncode) == validation.PAUSED:
        pass
    if status != 0:
        sys.exit(f"{plan_path}: the staged record exited {status}; the same command resumes it")
    if untimed:
        write(DIRECTORY / "untimed-smoke.json", {
            "evidence": False,
            "reason": "untimed validation of the launcher on an unlocked host outside the "
                      "benchmark window; its batch-fill values are not measurements",
            "record": read(ROOT / record)})
        (DIRECTORY / "untimed-execution.log").write_bytes((ROOT / stage / "execution.log").read_bytes())
        record = (DIRECTORY / "untimed-smoke.json").relative_to(ROOT)
    print(record)


def main():
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("action", choices=("build", "plan", "run"))
    parser.add_argument("--untimed", action="store_true")
    args = parser.parse_args()
    if Path.cwd() != ROOT:
        parser.error("invoke from the worktree root")
    os.environ.update(CARGO_NET_OFFLINE="true", CARGO_CI_NO_SCCACHE="1")
    os.environ["PATH"] = f"{Path.home() / '.cargo/bin'}{os.pathsep}{os.environ['PATH']}"
    if args.action == "build":
        build()
    elif args.action == "plan":
        stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        print(plan(DIRECTORY, CAMPAIGN, stamp).relative_to(ROOT))
    else:
        if not args.untimed and os.environ.get(WINDOW) != "1":
            parser.error("timed runs require the overnight benchmark window")
        run(args.untimed)


if __name__ == "__main__":
    main()
