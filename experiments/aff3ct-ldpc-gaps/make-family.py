#!/usr/bin/env python3
"""Generate a family plan and addendum; --configuration emits portable configuration.

Runtime plans resolve bundles and prepared quality from GF2_LDPC_INPUT_ROOT and
executables from explicit --build records. --confirmation reads the freezer's
addendum in the output directory and plans its retained cells.
"""

import argparse
import copy
import importlib.util
from pathlib import Path
import re
import sys

from campaign import (HERE, ROOT, OUTPUT, INPUT_VAR, digest, files, input_root, output_path, read, write)

spec = importlib.util.spec_from_file_location(
    "campaign_plan", ROOT / files.live_file(ROOT, "campaign_plan.py"))
projection = importlib.util.module_from_spec(spec)
spec.loader.exec_module(projection)


def merged(base, overlay):
    result = copy.deepcopy(base)
    for key, value in overlay.items():
        result[key] = merged(result[key], value) if isinstance(value, dict) and key in result else value
    return result


def shared():
    schema = read(ROOT / files.live_file(ROOT, "addendum.schema.json"))
    protocol = schema["properties"]["protocol"]["properties"]
    identity = {key: protocol[key]["const"] for key in ("id", "version")}
    # The schema's protocol version selects the live document among its versions.
    opening = files.PROTOCOL_OPENING + f"{identity['version']}.".encode()
    document = ROOT / files.document(ROOT, "protocol*.md", opening)
    text = document.read_text().split("## Frozen shared settings", 1)[1]
    section = text.split("\n## ", 1)[0]
    import json
    settings = {name: json.loads(value) for name, value in
                re.findall(r"^\| `(\w+)` \| `([^`]+)` \|", section, re.M)}
    return schema["properties"]["schema"]["const"], identity, settings


def addendum_cell(cell, declaration, catalogue, settings):
    code = declaration["codes"][cell["code"]]
    carried = ("objective", "metric_kind", "scaling", "core_arm", "workers", "cold_calls", "cache_state")
    return {
        "cell_id": cell["cell_id"], "role": "exploratory",
        "workload": {"identity": code["workload_identity"],
                     "size": {**{k: code["bundle_contract"][k] for k in ("n", "k", "frames")},
                              "timed_batch": cell["timed_batch"],
                              "workers": cell["workers"]["declared"],
                              "timed_frames": cell["timed_batch"] * cell["workers"]["declared"],
                              **cell.get("size", {})},
                     "seed": code["bundle_contract"]["seed"]},
        **{key: cell[key] for key in carried if key in cell},
        "builds": {role: catalogue["arms"][cell[role]]["build"] for role in ("baseline", "candidate")},
        "conversion_costs_included": cell["conversion_costs_included"],
        "decoder": {**cell["decoder"], "code": code["decoder_code"],
                    "input": code["decoder_input"],
                    "quality_tolerance": {**cell["decoder"]["quality_tolerance"],
                                          "confidence": settings["quality_confidence"]}},
    }


def generate(declaration, out=None, *, configuration=False, confirmation=False, builds=()):
    declaration = read(declaration)
    catalogue = read(HERE / "arms.json")
    campaign = declaration["confirmation" if confirmation else "campaign"]
    label = "confirmation" if confirmation else "pilot"
    relative = Path("target/aff3ct-ldpc-gaps") / declaration["family"]["id"] / label
    out = Path(out or ROOT / relative).resolve()
    if not configuration:
        output_path(out)
    schema, protocol, settings = shared()
    cells = [merged(declaration["cell"], row) for row in declaration["cells"]]
    if confirmation:
        addendum = read(out / "addendum.json")
        declared = {cell["cell_id"]: cell for cell in cells}
        cells = [declared[row["cell_id"]] for row in addendum["cells"]]
    else:
        addendum = {"schema": schema, "protocol": protocol, "family": declaration["family"],
                    "frozen": {"frozen_utc": declaration["frozen_utc"]},
                    "effect": declaration["effect"], "complexity_budget": declaration["complexity_budget"],
                    "family_wise": {"alpha": settings["family_alpha"],
                                    "prior_confirmatory_trials": len(declaration["family_wise"]["prior_trials"]),
                                    **declaration["family_wise"]},
                    "search_budget": {"max_pilot_trials_per_cell": declaration["max_pilot_trials_per_cell"],
                                      "max_confirmatory_attempts_per_candidate": settings["max_confirmatory_attempts_per_candidate"]},
                    "holdout": declaration["holdout"],
                    "cells": [addendum_cell(cell, declaration, catalogue, settings) for cell in cells]}
    if not Path(addendum["family_wise"]["ledger_path"]).is_relative_to("target"):
        raise ValueError("family ledger_path must be under target/")
    root = f"${{{INPUT_VAR}}}" if configuration else str(input_root())
    records = {name: (record, row) for path in builds for record in [read(path)]
               for name, row in record["arms"].items()}
    arms, planned = {}, []
    for cell in cells:
        code = cell["code"]
        bundle = f"{root}/bundles/{catalogue['codes'][code]['bundle']}"
        if not configuration:
            manifest = read(Path(bundle) / "manifest.json")
            declared_code = declaration["codes"][code]
            for key, value in declared_code["bundle_contract"].items():
                if manifest[key] != value:
                    raise ValueError(f"{code}: bundle {key} differs from the frozen declaration")
            for key, value in [("h_sha256", declared_code["decoder_code"]["h_sha256"]),
                               ("llrs_sha256", declared_code["decoder_input"]["llr_sha256"])]:
                if manifest[key] != value:
                    raise ValueError(f"{code}: bundle {key} differs from the frozen declaration")
        names = []
        for role in ("baseline", "candidate"):
            name = cell[role]
            row = catalogue["arms"][name]
            if row.get("configuration_only") and not configuration:
                raise ValueError(f"{name} is pilot configuration only; use the tuned arm in a scoped confirmation family")
            executable = f"@build/{name}/{row['executable']}"
            flags = "-C target-cpu=native"
            if not configuration:
                if name not in records:
                    raise ValueError(f"--build records name no pinned executable for {name}")
                record, pinned = records[name]
                executable = str(Path(record["build_root"]) / pinned["executable"])
                if digest(executable) != pinned["executable_sha256"]:
                    raise ValueError(f"{name}: executable differs from its build record")
                flags = record["rustflags"]
            quality = f"{root}/quality/{row['quality']}-{code}.json"
            if not configuration and not Path(quality).is_file():
                raise ValueError(f"missing prepared quality: {quality}")
            key = f"{name}-{code}"
            arms[key] = projection.arm(row["build"], row["description"], executable,
                                       {**row["environment"], "GF2_LDPC_QUALITY": quality}, flags)
            profile = declaration.get("tuning_profiles", {}).get(name, row.get("tuning_profile"))
            if profile:
                envelope = ROOT / profile
                arms[key]["tuning_profile"] = ({"id": read(envelope)["profile_id"], "sha256": digest(envelope)}
                                                 if configuration else record.get("tuning_profile"))
                if not arms[key]["tuning_profile"]:
                    raise ValueError(f"{name}: build record names no embedded tuning profile")
            names.append(key)
        decoder = cell["decoder"]
        planned.append({"cell_id": cell["cell_id"], "baseline_arm": names[0], "candidate_arm": names[1],
                        "case": {"bundle": bundle, "code": catalogue["codes"][code]["code"],
                                 "iteration_cap": decoder["iteration_cap"],
                                 "normalization_factor": decoder["normalization"]["factor"],
                                 "syndrome_stopping": decoder["stopping"]["kind"] == "syndrome",
                                 "batch_size": cell["timed_batch"],
                                 "quality_frames": declaration["codes"][code]["bundle_contract"]["frames"],
                                 **cell.get("case", {})},
                        "pilot_pairs": campaign.get("pilot_pairs") if not confirmation else None})
    plan = {"schema": "zen3-benchmark-plan-v1", "campaign_id": campaign["id"],
            "issue": declaration["family"]["issue"], "label": label, "campaign_seed": campaign["seed"],
            "addendum": str(relative / "addendum.json") if configuration else str((out / "addendum.json").relative_to(ROOT)),
            "lock_path": "/tmp/gf2-ccx1.lock", "wrapper": files.live_file(ROOT, "ccx1-bench-flock.sh") + " --full-host",
            "timing_override": None, "arms": dict(sorted(arms.items())), "cells": planned,
            "max_cells_per_session": campaign["max_cells_per_session"]}
    if not confirmation:
        write(out / "addendum.json", addendum)
    write(out / "plan.json", plan)
    return plan, addendum


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("declaration")
    parser.add_argument("out", nargs="?")
    parser.add_argument("--configuration", action="store_true")
    parser.add_argument("--confirmation", action="store_true")
    parser.add_argument("--build", action="append", default=[], dest="builds")
    args = parser.parse_args()
    try:
        generate(**vars(args))
    except (ValueError, OSError) as error:
        sys.exit(str(error))


if __name__ == "__main__":
    main()
