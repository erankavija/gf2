#!/usr/bin/env python3
"""Compose integrated family plans using the shared core and decoder generators.

Usage: prepare.py {core,decoder} [--confirmation] [--smoke]
       prepare.py --self-test
       prepare.py --collect-smokes

Smoke plans use throwaway confirmation declarations under target; they assert
wire behavior and zero timing windows without inventing pilot evidence.
"""

import argparse
import copy
import json
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
from locate import HERE, ROOT, live, load  # noqa: E402

CORE = "1362381c-core-route-integration"
DECODER = "1362381c-decoder-route-integration"
SELECTED = {
    "core": ["matvec-r1024-24w-native", "popcount-1024w-native", "clmul-wide-9w-native"],
    "decoder": ["dvb-t2-r12-integrated-w1-b1", "dvb-t2-r12-integrated-w6-b8",
                "nr-bg1-z384-integrated-w12-b8", "nr-bg1-z384-integrated-w24-b8"],
}


def read(path):
    return json.loads(Path(path).read_text())


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n")


def decoder_inputs():
    generator = load("survey/make-family.py", "decoder_inputs")
    catalogue = read(ROOT / live("ed3d490e-zen3-ldpc-frame-simd/survey/arms.json"))
    return generator.bundles(catalogue, ROOT / "target/31727c57-decoder-inputs")


def decoder(out, confirmation):
    generator = load("survey/make-family.py", "decoder_family")
    declaration = read(ROOT / live("families/1362381c-decoder-route.json"))
    names = {declaration["cell"][role] for role in ("baseline", "candidate")}
    records = [copy.deepcopy(record) for _, record in generator.peak.build_records()
               if names.intersection(record["arms"])]
    for record in records:
        record["arms"] = {name: row for name, row in record["arms"].items() if name in names}
    catalogue_paths = {record["catalogue"]["path"] for record in records}
    (catalogue_path,) = catalogue_paths
    catalogue = read(ROOT / catalogue_path)
    # The live records pin executables in the read-only story build tree.
    for record in records:
        for row in record["arms"].values():
            row["path"] = str(Path(record["build_root"]) / row["path"])
    profiles = generator.tuning_profiles(declaration, catalogue["arms"])
    manifests = decoder_inputs()
    cells = [generator.merged(declaration["cell"], cell) for cell in declaration["cells"]]
    held = [generator.merged(declaration["cell"], cell)
            for cell in declaration["holdout_declaration"]["cells"]]
    if confirmation:
        declared = {cell["cell_id"]: cell for cell in cells + held}
        cells = [declared[cell["cell_id"]] for cell in read(out / "addendum.json")["cells"]]
    else:
        schema = generator.schema()
        settings = generator.shared_settings(schema)
        generator.write(out / "addendum.json", generator.addendum(
            declaration, cells, manifests, catalogue["arms"], schema, settings, profiles))
        generator.write(out.parent / "holdout-cells.json", generator.holdout_record(
            {**declaration, "holdout_declaration": {**declaration["holdout_declaration"],
                                                   "cells": held}},
            cells, manifests, catalogue["arms"], settings, profiles))
    plan = generator.plan(
        declaration, cells, manifests, catalogue, records, profiles,
        generator.repository_path(out / "addendum.json"),
        "confirmation" if confirmation else "pilot")
    for arm in plan["arms"].values():
        for key, value in arm["environment"].items():
            path = Path(value)
            if path.is_absolute() and path.is_relative_to(ROOT):
                arm["environment"][key] = str(path.relative_to(ROOT))
    for cell in plan["cells"]:
        cell["case"]["bundle"] = str(Path(cell["case"]["bundle"]).relative_to(ROOT))
    generator.write(out / "plan.json", plan)


def core(out, confirmation):
    arguments = [sys.executable, live("63bad95d/survey/make-family-plan.py"),
                 "--addendum", str((out / "addendum.json").relative_to(ROOT)),
                 "--label", "confirmation" if confirmation else "pilot",
                 "--campaign-id", f"31727c57-core-{'confirmation' if confirmation else 'pilot'}",
                 "--campaign-seed", "3172712" if confirmation else "3172711",
                 "--lock", "/tmp/gf2-ccx1.lock", "--producing-manifest",
                 str((HERE / "core/producing-inputs.json").relative_to(ROOT)),
                 "--max-cells-per-session", "1", "--output", str((out / "plan.json").relative_to(ROOT))]
    for build in ("conservative-portable", "tuned-portable", "native"):
        arguments += ["--arm", f"{build}=target/31727c57-core-{build}/release/profile-arm"]
    subprocess.run(arguments, check=True)
    plan = read(out / "plan.json")
    plan["wrapper"] = live("ccx1-bench-flock.sh")
    write(out / "plan.json", plan)


def smoke_plan(family):
    source = HERE / family / "pilot"
    out = ROOT / "target/31727c57-confirmation-smoke" / family
    out.mkdir(parents=True, exist_ok=True)
    addendum = read(source / "addendum.json")
    addendum["family"]["purpose"] = "final-integration"
    addendum["cells"] = [{**cell, "role": "confirmatory"} for cell in addendum["cells"]
                         if cell["cell_id"] in SELECTED[family]]
    held = read(HERE / family / "holdout-cells.json")["addendum_cells"]
    addendum["cells"] += held
    addendum["holdout"] = {"required": True, "cells": [cell["cell_id"] for cell in held]}
    write(out / "addendum.json", addendum)
    (core if family == "core" else decoder)(out, True)
    plan = read(out / "plan.json")
    plan["campaign_id"] += "-untimed"
    write(out / "plan.json", plan)
    return out / "plan.json"


def collect_smokes():
    generator = load("survey/make-family.py", "decoder_smoke")
    verifier = load("verify-campaign-log.py", "log_verifier")
    for family in ("core", "decoder"):
        diagnostics = []
        for phase, suffix in (("pilot", "pilot"), ("confirmation", "confirmation-untimed")):
            campaign = ROOT / "target/campaigns" / f"31727c57-{family}-{suffix}"
            record = read(campaign / "untimed-smoke.json")
            assert all(arm["role"] == "validation" and arm["windows"] == 0
                       for cell in record["cells"] for arm in cell["arms"])
            stage = campaign / "untimed-stage"
            assert verifier.stage_complete(str(stage / "execution.log"))
            write(HERE / family / f"{phase}-untimed-smoke.json", record)
            (HERE / family / f"{phase}-untimed-execution.log").write_bytes(
                (stage / "execution.log").read_bytes())
            if family == "decoder":
                rows = generator.smoke.diagnostics(stage)
                assert len(rows) == 2 * len(record["cells"])
                assert all("batch_fill_ns" in row["batch-decoder-diagnostics"]["conversion"]
                           for row in rows)
                diagnostics += [{"phase": phase, **row} for row in rows]
        if diagnostics:
            write(HERE / family / "untimed-diagnostics.json", diagnostics)
    print("both families: pilot and confirmation smokes complete with zero timing windows")


def self_test():
    for family in ("core", "decoder"):
        pilot = read(HERE / family / "pilot/addendum.json")
        held = read(HERE / family / "holdout-cells.json")["addendum_cells"]
        ids = {cell["cell_id"] for cell in pilot["cells"]}
        assert not ids.intersection(cell["cell_id"] for cell in held)
        assert set(SELECTED[family]) <= ids
        assert len(SELECTED[family]) + len(held) <= 6
        assert all(cell["role"] == "exploratory" for cell in pilot["cells"])
        assert all(cell["role"] == "holdout" for cell in held)
        assert not Path(pilot["family_wise"]["ledger_path"]).is_absolute()
    pilot = read(HERE / "decoder/pilot/addendum.json")
    coverage = {(cell["decoder"]["code"]["identity"], cell["workers"]["declared"],
                 cell["workload"]["size"]["timed_batch"]) for cell in pilot["cells"]}
    codes = {code for code, _, _ in coverage}
    assert len(codes) == 2
    assert coverage == {(code, workers, batch) for code in codes
                        for workers, batch in ((1, 1), (1, 8), (6, 8), (12, 8), (24, 8))}
    # Earlier profile-family projections remain byte-identical.
    subprocess.run([sys.executable, live("63bad95d/survey/make-family-addenda.py"),
                    "--check"], check=True)
    print("family preparation contracts pass")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("family", choices=("core", "decoder"), nargs="?")
    parser.add_argument("--confirmation", action="store_true")
    parser.add_argument("--smoke", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--collect-smokes", action="store_true")
    args = parser.parse_args()
    if args.collect_smokes:
        collect_smokes()
        return
    if args.self_test:
        self_test()
        return
    if not args.family:
        parser.error("family required")
    if args.smoke:
        print(smoke_plan(args.family).relative_to(ROOT))
        return
    out = HERE / args.family / ("confirmation" if args.confirmation else "pilot")
    out.mkdir(parents=True, exist_ok=True)
    (core if args.family == "core" else decoder)(out, args.confirmation)


if __name__ == "__main__":
    main()
