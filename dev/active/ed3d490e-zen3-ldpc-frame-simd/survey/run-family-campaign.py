#!/usr/bin/env python3
"""Benchmark-window launcher of a protocol family of the batch decoder story (jit:dbea4e10).

Runs `PLAN` in `benchmark-ab-runner run` sessions under the plan's lock wrapper until its stage
completes, then finalizes, verifies and evaluates the receipt beside the family ledger. The
plan's campaign ID fixes the stage and the receipt, so the same command resumes an interrupted
campaign without repeating a completed cell. It refuses outside the benchmark window, an addendum
differing from its committed bytes, and an arm executable whose digest `BUILD_IDENTITY` does not
name. `--untimed` runs the same stage through the runner's staged smoke, with no window, lock,
ledger or receipt.

Usage (from the worktree root): run-family-campaign.py PLAN BUILD_IDENTITY [--untimed]
"""

import importlib.util
import json
import os
import pathlib
import subprocess
import sys

sys.dont_write_bytecode = True


def load(path):
    module = importlib.util.module_from_spec(importlib.util.spec_from_file_location(
        path.stem.replace("-", "_"), path))
    module.__spec__.loader.exec_module(module)
    return module


validation = load(pathlib.Path(__file__).resolve().with_name("record-arm-validation.py"))
peak, smoke = validation.peak, validation.smoke
WINDOW = "GF2_BENCH_WINDOW"


def committed(path):
    """Whether git tracks `path` with the working-tree bytes of HEAD."""
    def quiet(*command):
        return subprocess.run(["git", "-C", str(smoke.ROOT), *command, "--", str(path)],
                              capture_output=True).returncode == 0
    return quiet("ls-files", "--error-unmatch") and quiet("diff", "--quiet", "HEAD")


def unidentified(plan, build_identity):
    """Plan arms whose executable digest no arm of the build identity record names."""
    pinned = {arm["executable_sha256"] for arm in build_identity["arms"].values()}
    absent = []
    for name, arm in sorted(plan["arms"].items()):
        try:
            digest = peak.sha256_file(smoke.ROOT / arm["executable"])
        except OSError:
            digest = None
        if digest not in pinned:
            absent.append(name)
    return absent


def genesis(ledger):
    """Creates the family ledger's empty genesis file unless the ledger exists."""
    ledger.parent.mkdir(parents=True, exist_ok=True)
    try:
        ledger.open("x").close()
    except FileExistsError:
        pass


def stage_complete(verifier, log):
    """Whether the stage journalled its terminal `complete` record."""
    try:
        return verifier.stage_complete(str(log))
    except ValueError:
        # A torn trailing append, which the next session recovers.
        return False


def timed(plan_path, plan, campaign, ledger):
    """Runs the stage's sessions in the window, then finalizes and evaluates its receipt."""
    runner, environment = smoke.build_runner("benchmark-acceptance")
    acceptance = pathlib.Path(runner).with_name("benchmark-acceptance")
    verifier = load(smoke.ROOT / smoke.located("file", "verify-campaign-log.py"))
    stage, out = campaign / "stage", ledger.parent / plan["campaign_id"]
    log = stage / "execution.log"
    environment.update(GF2_BENCH="1", CARGO_CI_NO_LOCK="1", GF2_CCX1_LOCK=plan["lock_path"])
    print(f"GF2_CAMPAIGN_EXECUTION_LOG={log}", flush=True)
    session = [str(smoke.ROOT / plan["wrapper"]), "--full-host", runner, "run", str(stage),
               str(plan_path)]
    while not stage_complete(verifier, log):
        status = subprocess.run(session, env=environment).returncode
        if status not in (0, validation.PAUSED):
            sys.exit(f"session exited {status}; the same command resumes {stage}")
    # The runner writes the receipt last, so a finalize it lacks is repeated in full.
    if not (out / "receipt.json").exists():
        subprocess.run([runner, "finalize", str(stage), str(out)], check=True)
    print(verifier.verify(str(out / "execution.log"), str(out / "receipt.json"),
                          str(out / "plan.json")), flush=True)
    sys.exit(subprocess.run([str(acceptance), str(out)]).returncode)


def main():
    arguments = sys.argv[1:]
    untimed = "--untimed" in arguments
    if untimed:
        arguments.remove("--untimed")
    if len(arguments) != 2:
        sys.exit(__doc__)
    if pathlib.Path.cwd() != smoke.ROOT:
        sys.exit("invoke from the worktree root")
    # The benchmark window has no login shell.
    os.environ["PATH"] = f"{pathlib.Path.home() / '.cargo/bin'}{os.pathsep}{os.environ['PATH']}"
    plan_path, identity_path = (pathlib.Path(argument) for argument in arguments)
    if not untimed and os.environ.get(WINDOW) != "1":
        sys.exit(f"{WINDOW} is unset: a timed campaign runs only in the benchmark window")
    plan = json.loads(plan_path.read_text(encoding="utf-8"))
    if not untimed and not committed(plan["addendum"]):
        sys.exit(f"{plan['addendum']} is uncommitted or differs from its committed bytes")
    absent = unidentified(plan, json.loads(identity_path.read_text(encoding="utf-8")))
    if absent:
        sys.exit(f"{identity_path} names no executable digest of arms {', '.join(absent)}")
    campaign = smoke.ROOT / "target/campaigns" / plan["campaign_id"]
    if untimed:
        runner, environment = smoke.build_runner()
        stage = campaign / "untimed-stage"
        print(f"GF2_CAMPAIGN_EXECUTION_LOG={stage / 'execution.log'}", flush=True)
        validation.staged_smoke(runner, environment, plan_path, stage,
                                campaign / "untimed-smoke.json")
        print(campaign / "untimed-smoke.json")
        return
    addendum = json.loads((smoke.ROOT / plan["addendum"]).read_text(encoding="utf-8"))
    ledger = smoke.ROOT / addendum["family_wise"]["ledger_path"]
    genesis(ledger)
    timed(plan_path, plan, campaign, ledger)


if __name__ == "__main__":
    main()
