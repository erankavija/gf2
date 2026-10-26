#!/usr/bin/env python3
"""Run one integration family's pilot, canonical freeze and confirmation.

Usage: window.py {core,decoder} [--untimed]

The untimed path uses the same session runner with throwaway confirmation
inputs; only the overnight path freezes and commits confirmation outputs.
"""

import argparse
import datetime
import json
import os
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
from locate import HERE, ROOT, live, load  # noqa: E402
import prepare  # noqa: E402


def run(*arguments):
    subprocess.run([str(argument) for argument in arguments], check=True)


def identities(family):
    if family == "core":
        return [str((HERE / "core/arm-builds.json").relative_to(ROOT))]
    return [live("e5f8b3ae/portable/build-identity.json")]


def campaign(plan, family, untimed=False):
    run(sys.executable, live("survey/run-family-campaign.py"), plan,
        *identities(family), *(["--untimed"] if untimed else []))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("family", choices=("core", "decoder"))
    parser.add_argument("--untimed", action="store_true")
    args = parser.parse_args()
    os.environ.update(CARGO_NET_OFFLINE="true", CARGO_CI_NO_SCCACHE="1",
                      RUSTUP_TOOLCHAIN="1.95")
    os.environ["PATH"] = f"{Path.home() / '.cargo/bin'}{os.pathsep}{os.environ['PATH']}"
    os.environ["CARGO_HOME"] = str(ROOT / "target/31727c57-cargo-home")
    directory = (HERE / args.family).relative_to(ROOT)
    pilot_plan = directory / "pilot/plan.json"
    if not args.untimed and os.environ.get("GF2_BENCH_WINDOW") != "1":
        parser.error("timed runs require the overnight benchmark window")
    if args.family == "decoder":
        prepare.decoder_inputs()
    if args.untimed:
        campaign(pilot_plan, args.family, True)
        campaign(prepare.smoke_plan(args.family).relative_to(ROOT), args.family, True)
        return
    runner = load("survey/run-family-campaign.py", "family_runner")
    holdout = directory / "holdout-cells.json"
    if not runner.committed(holdout):
        parser.error("commit the holdout declaration before the pilot")
    campaign(pilot_plan, args.family)
    addendum = prepare.read(ROOT / directory / "pilot/addendum.json")
    family = addendum["family"]["id"]
    budget_command = ["target/release/benchmark-acceptance", "--next-attempt",
                      addendum["family_wise"]["ledger_path"], family]
    budget = subprocess.check_output(budget_command, text=True)
    print(budget, flush=True)
    selected = prepare.SELECTED[args.family]
    held = prepare.read(holdout)["addendum_cells"]
    confirmation = directory / "confirmation"
    confirmation.mkdir(exist_ok=True)
    frozen, derivation, plan = (confirmation / name for name in
                                ("addendum.json", "derivation.txt", "plan.json"))
    if not frozen.exists():
        if len(selected) + len(held) > json.loads(budget)["next_attempt"]["max_cells"]:
            parser.error("confirmation selection exceeds the ledger's P-20 bound")
        pilot_campaign = prepare.read(pilot_plan)["campaign_id"]
        pilot = Path(addendum["family_wise"]["ledger_path"]).parent / pilot_campaign
        description = addendum["family"]["description"].replace(
            "Pilot outcomes are exploratory.",
            "Confirmation uses fresh samples; the pilot supplies resolution alone.")
        selection = [argument for cell in selected for argument in ("--cell", cell)]
        run(sys.executable, live("c7113c5a/survey/freeze-confirmation.py"),
            "--pilot-addendum", directory / "pilot/addendum.json", "--pilot", pilot,
            "--frozen-utc", datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "--family-description", description, "--purpose", "final-integration",
            "--holdout-cells", holdout, *selection,
            "--selection-rationale", "The predeclared subset spans build levels or worker counts; "
            "holdouts consume the remaining first-attempt comparisons. Resolution uses every pilot cell.",
            "--output", frozen, "--record", derivation)
    if not derivation.is_file():
        parser.error("incomplete freeze: preserve and inspect the freeze outputs")
    if not plan.exists():
        run(sys.executable, (HERE / "prepare.py").relative_to(ROOT), args.family, "--confirmation")
    files = [str(path) for path in (frozen, derivation, plan)]
    run("git", "add", "--", *files)
    if subprocess.run(["git", "diff", "--quiet", "HEAD", "--", *files]).returncode:
        run("git", "commit", "--only", "-m",
            f"docs(jit:31727c57): freeze {args.family} route confirmation", "--", *files)
    campaign(plan, args.family)


if __name__ == "__main__":
    main()
