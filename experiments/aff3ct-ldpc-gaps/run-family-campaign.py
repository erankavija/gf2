#!/usr/bin/env python3
"""Resume bounded sessions of a materialized plan; --untimed uses staged smoke."""

import argparse
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys

from campaign import ROOT, OUTPUT, build_tools, digest, input_root, output_path, read


def unidentified(plan, records):
    pinned = {name: (record, row) for record in records for name, row in record["arms"].items()}
    missing = []
    for key, arm in plan["arms"].items():
        # Plan keys append the declared code, whose suffix can itself contain dashes.
        candidates = [name for name in pinned if key.startswith(name + "-")]
        name = max(candidates, key=len, default=None)
        executable = Path(arm["executable"])
        if name is None or not executable.is_absolute() or not executable.is_file():
            missing.append(key)
            continue
        record, row = pinned[name]
        if (executable.resolve() != (Path(record["build_root"]) / row["executable"]).resolve()
                or digest(executable) != row["executable_sha256"]
                or arm["rustflags"] != record["rustflags"]):
            missing.append(key)
    return missing


def preflight(plan, records):
    root = input_root()
    absent = unidentified(plan, records)
    if absent:
        raise ValueError("missing or mismatched pinned executables: " + ", ".join(absent))
    for cell in plan["cells"]:
        bundle = Path(cell["case"]["bundle"])
        if not bundle.is_absolute() or not bundle.resolve().is_relative_to(root) or not bundle.is_dir():
            raise ValueError(f"missing bundle under GF2_LDPC_INPUT_ROOT: {bundle}")
    for arm in plan["arms"].values():
        quality = Path(arm["environment"]["GF2_LDPC_QUALITY"])
        if not quality.is_absolute() or not quality.resolve().is_relative_to(root) or not quality.is_file():
            raise ValueError(f"missing prepared quality under GF2_LDPC_INPUT_ROOT: {quality}")
    ledger = Path(read(ROOT / plan["addendum"])["family_wise"]["ledger_path"])
    output_path(ROOT / ledger)
    return ROOT / ledger


def stage_complete(path):
    try:
        data = path.read_bytes()
        if not data.endswith(b"\n"):
            return False
        return json.loads(data.splitlines()[-1])["event"] == "complete"
    except (FileNotFoundError, IndexError, ValueError, KeyError):
        return False


def session_command(plan, runner, stage, plan_path):
    wrapper, *flags = shlex.split(plan["wrapper"])
    return [str(ROOT / wrapper), *dict.fromkeys([*flags, "--full-host"]), str(runner),
            "run", str(stage), str(plan_path)]


def run(plan_path, identities, untimed=False):
    if not untimed and os.environ.get("GF2_BENCH_WINDOW") != "1":
        raise ValueError("GF2_BENCH_WINDOW is unset: timed campaigns require the benchmark window")
    plan_path = output_path(plan_path)
    plan = read(plan_path)
    ledger = preflight(plan, [read(path) for path in identities])
    runner = build_tools()
    campaign = output_path(OUTPUT / plan["campaign_id"])
    campaign.mkdir(parents=True, exist_ok=True)
    stage = campaign / ("untimed-stage" if untimed else "stage")
    subprocess.run([str(runner), "check", str(plan_path)], cwd=ROOT, check=True)
    if untimed:
        command = [str(runner), "smoke", str(plan_path), "--stage", str(stage),
                   "--record", str(campaign / "smoke.json")]
        while True:
            status = subprocess.run(command, cwd=ROOT).returncode
            if status != 3:
                if status:
                    raise ValueError(f"smoke exited {status}; rerun to resume {stage}")
                return
    ledger.parent.mkdir(parents=True, exist_ok=True)
    ledger.touch(exist_ok=True)
    out = output_path(ledger.parent / plan["campaign_id"])
    environment = {**os.environ, "GF2_BENCH": "1", "CARGO_CI_NO_LOCK": "1",
                   "GF2_CCX1_LOCK": plan["lock_path"]}
    if not (out / "receipt.json").exists():
        while not stage_complete(stage / "execution.log"):
            status = subprocess.run(session_command(plan, runner, stage, plan_path),
                                    cwd=ROOT, env=environment).returncode
            if status not in (0, 3):
                raise ValueError(f"session exited {status}; rerun to resume {stage}")
        subprocess.run([str(runner), "finalize", str(stage), str(out)], cwd=ROOT, check=True)
    subprocess.run([str(runner.with_name("benchmark-acceptance")), str(out)], cwd=ROOT, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("plan")
    parser.add_argument("identities", nargs="+")
    parser.add_argument("--untimed", action="store_true")
    args = parser.parse_args()
    try:
        run(args.plan, args.identities, args.untimed)
    except (ValueError, OSError) as error:
        sys.exit(str(error))


if __name__ == "__main__":
    main()
