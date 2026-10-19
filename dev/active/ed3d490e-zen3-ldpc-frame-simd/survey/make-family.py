#!/usr/bin/env python3
"""Pilot addendum and runner plan of one batch decoder family from its declaration (jit:4d8fd613).

Each setting comes from the declaration, the protocol's frozen shared settings table, the
recorded input bundles or the arms' build identity records; the generator holds none. A cell is
the declaration's `cell` overlaid with its own entry, and its `case` overrides the runner case
its decoder block derives, where the arms' shared case differs from that block; a family's declaration is
`families/<family>.json` beside this script. The plan names its addendum relative to the repository root when
it lies inside it, and the bundles extracted beside it by absolute host path; one declaration and one output directory give the same bytes.
`--confirmation` plans the declaration's `confirmation` campaign over the cells of the confirmation
addendum the canonical freezer wrote to OUT_DIR, which the generator reads and never writes.
The declaration's optional `tuning_profiles` names, per arm, the root-relative envelope file the arm
installs; the plan pins it by profile id and digest, and it makes a portable arm's build
`tuned-portable`, which is `conservative-portable` otherwise.

Usage (from the worktree root): make-family.py DECLARATION OUT_DIR [--confirmation]
"""

import hashlib
import importlib.util
import json
import pathlib
import re
import sys
import tarfile

sys.dont_write_bytecode = True
PEAK = pathlib.Path(__file__).resolve().with_name("record-peak-memory.py")
peak = importlib.util.module_from_spec(importlib.util.spec_from_file_location("peak", PEAK))
peak.__spec__.loader.exec_module(peak)
smoke, arm_build = peak.smoke, peak.arm_build

ADDENDUM = "addendum.json"
PLAN = "plan.json"
INPUTS = "inputs"
# The declaration block that identifies the campaign of each plan label.
CAMPAIGN = {"pilot": "campaign", "confirmation": "confirmation"}
# `protocol::PLAN_SCHEMA_ID` of the campaign support crate.
PLAN_SCHEMA = "zen3-benchmark-plan-v1"
SHARED_SECTION = "## Frozen shared settings"
SHARED_ROW = re.compile(r"^\| `(?P<name>\w+)` \| `(?P<value>[^`]+)` \|")
# Keys of a declared cell the addendum cell carries unchanged, in schema order.
CARRIED = ("objective", "metric_kind", "scaling", "core_arm", "workers", "cold_calls",
           "cache_state")


def repository_files():
    path = smoke.ROOT / min(smoke.listed("repository_files.py"), key=len)
    module = importlib.util.module_from_spec(
        importlib.util.spec_from_file_location("repository_files", path))
    module.__spec__.loader.exec_module(module)
    return module


def schema():
    return json.loads((smoke.ROOT / smoke.located("file", "addendum.schema.json"))
                      .read_text(encoding="utf-8"))


def protocol_identity(addendum_schema):
    protocol = addendum_schema["properties"]["protocol"]["properties"]
    return {"id": protocol["id"]["const"], "version": protocol["version"]["const"]}


def protocol_document(addendum_schema):
    """Root-relative path of the protocol document of the schema's version."""
    files = repository_files()
    version = protocol_identity(addendum_schema)["version"]
    return files.document(smoke.ROOT, "protocol*.md",
                          files.PROTOCOL_OPENING + f"{version}.".encode())


def shared_settings(addendum_schema):
    """The shared settings table of the protocol document of the schema's version."""
    text = (smoke.ROOT / protocol_document(addendum_schema)).read_text(encoding="utf-8")
    _, _, following = text.partition(SHARED_SECTION)
    section, _, _ = following.partition("\n## ")
    return {match["name"]: json.loads(match["value"])
            for match in map(SHARED_ROW.match, section.splitlines()) if match}


def merged(base, overlay):
    """`base` with `overlay`'s values, merging the objects both hold."""
    result = dict(base)
    for key, value in overlay.items():
        both = isinstance(value, dict) and isinstance(result.get(key), dict)
        result[key] = merged(result[key], value) if both else value
    return result


def bundles(catalogue, out_dir):
    """The extracted bundle directory and manifest of each catalogued code."""
    inputs = out_dir / INPUTS
    inputs.mkdir(parents=True, exist_ok=True)
    with tarfile.open(smoke.by_digest(smoke.ARCHIVE)) as archive:
        archive.extractall(inputs, filter="data")
    found = {}
    for key, code in catalogue["codes"].items():
        directory = inputs / code["bundle"]
        found[key] = {"directory": directory,
                      "manifest": json.loads((directory / "manifest.json").read_text())}
    return found


def tuning_profiles(declaration, catalogue):
    """Per arm the declaration names, the identity of the tuning profile it installs."""
    profiles = {}
    for name, path in declaration.get("tuning_profiles", {}).items():
        if name not in catalogue:
            sys.exit(f"the declaration names a tuning profile of the uncatalogued arm {name}")
        envelope = (smoke.ROOT / path).read_bytes()
        profiles[name] = {"id": json.loads(envelope)["profile_id"],
                          "sha256": hashlib.sha256(envelope).hexdigest()}
    return profiles


def build_identity(arm, profile):
    """The protocol build identity of a catalogue arm installing `profile`."""
    if arm["build"] != "portable":
        return arm["build"]
    return "conservative-portable" if profile is None else "tuned-portable"


def addendum_cell(declared, codes, manifests, arms, settings, profiles):
    bundle = manifests[declared["code"]]
    manifest = bundle["manifest"]
    archive, archive_sha256 = smoke.ARCHIVE
    code = codes[declared["code"]]
    workers = declared["workers"]["declared"]
    batch = declared["timed_batch"]
    decoder = declared["decoder"]
    carried = {key: declared[key] for key in CARRIED if key in declared}
    return {
        "cell_id": declared["cell_id"],
        "objective": carried.pop("objective"),
        "role": "exploratory",
        "workload": {
            "identity": code["workload_identity"],
            "size": {"n": manifest["n"], "k": manifest["k"], "frames": manifest["frames"],
                     "timed_batch": batch, "workers": workers, "timed_frames": batch * workers,
                     **declared.get("size", {})},
            "seed": manifest["seed"],
        },
        **carried,
        "builds": {role: build_identity(arms[declared[role]], profiles.get(declared[role]))
                   for role in ("baseline", "candidate")},
        "conversion_costs_included": declared["conversion_costs_included"],
        "decoder": {
            "arm_kind": decoder["arm_kind"],
            "code": {"identity": code["code_identity"], "n": manifest["n"], "k": manifest["k"],
                     "h_sha256": manifest["h_sha256"]},
            "input": {
                "llr_source": f"{archive} (SHA-256 {archive_sha256}) bundle "
                              f"{bundle['directory'].name}",
                "llr_sha256": manifest["llrs_sha256"],
                "frames": manifest["frames"],
                "seed": manifest["seed"],
                "codeword_source": manifest["codeword_source"],
                "snr_db": manifest["esn0_db"],
            },
            **{key: value for key, value in decoder.items() if key != "arm_kind"},
            "quality_tolerance": {**decoder["quality_tolerance"],
                                  "confidence": settings["quality_confidence"]},
        },
    }


def addendum(declaration, cells, manifests, arms, addendum_schema, settings, profiles):
    family_wise = declaration["family_wise"]
    return {
        "schema": addendum_schema["properties"]["schema"]["const"],
        "protocol": protocol_identity(addendum_schema),
        "family": declaration["family"],
        "frozen": {"frozen_utc": declaration["frozen_utc"]},
        "effect": declaration["effect"],
        "complexity_budget": declaration["complexity_budget"],
        "family_wise": {
            "alpha": settings["family_alpha"],
            "prior_confirmatory_trials": len(family_wise["prior_trials"]),
            "prior_trials": family_wise["prior_trials"],
            "ledger_path": family_wise["ledger_path"],
        },
        "search_budget": {
            "max_pilot_trials_per_cell": declaration["max_pilot_trials_per_cell"],
            "max_confirmatory_attempts_per_candidate":
                settings["max_confirmatory_attempts_per_candidate"],
        },
        "holdout": declaration["holdout"],
        "cells": [addendum_cell(cell, declaration["codes"], manifests, arms, settings, profiles)
                  for cell in cells],
    }


def rustflags(build, name):
    """The RUSTFLAGS the build record states for the arm's executable."""
    row = build["arms"][name]
    if row["source"] != "workspace":
        pinned = smoke.ROOT / row["pinned_by"]["record"]
        return json.loads(pinned.read_text(encoding="utf-8"))["rustflags"]
    (flags,) = {entry["rustflags"] for entry in arm_build.arm_build(build, name)["builds"]}
    return flags


def plan(declaration, cells, manifests, catalogue, records, profiles, addendum_path, label):
    builds = {name: build for build in records for name in build["arms"]}
    files = {arm: quality for build in records for arm, quality in peak.quality_files(build).items()}
    campaign = declaration[CAMPAIGN[label]]
    arms, planned = {}, []
    for declared in cells:
        key = declared["code"]
        names = []
        for role in ("baseline", "candidate"):
            name = declared[role]
            row = builds[name]["arms"][name]
            quality, _ = files[name, key]
            arms[f"{name}-{key}"] = {
                "build": build_identity(catalogue["arms"][name], profiles.get(name)),
                "description": catalogue["arms"][name]["description"],
                "executable": str(peak.executable_path(row)),
                "arguments": [],
                "environment": {**row["environment"],
                                peak.QUALITY_ENVIRONMENT: str(smoke.ROOT / quality)},
                "rustflags": rustflags(builds[name], name),
                "tuning_profile": profiles.get(name),
            }
            names.append(f"{name}-{key}")
        baseline, candidate = names
        decoder = declared["decoder"]
        planned.append({
            "cell_id": declared["cell_id"],
            "baseline_arm": baseline,
            "candidate_arm": candidate,
            "case": {
                "bundle": str(manifests[key]["directory"]),
                "code": catalogue["codes"][key]["code"],
                "iteration_cap": decoder["iteration_cap"],
                "normalization_factor": decoder["normalization"]["factor"],
                "syndrome_stopping": decoder["stopping"]["kind"] == "syndrome",
                "batch_size": declared["timed_batch"],
                "quality_frames": manifests[key]["manifest"]["frames"],
                **declared.get("case", {}),
            },
            "pilot_pairs": campaign.get("pilot_pairs"),
        })
    return {
        "schema": PLAN_SCHEMA,
        "campaign_id": campaign["id"],
        "issue": declaration["family"]["issue"],
        "label": label,
        "campaign_seed": campaign["seed"],
        "addendum": str(addendum_path),
        "producing_manifest": str(smoke.located("shared-producing-manifest")),
        "lock_path": smoke.LOCK,
        "wrapper": str(smoke.located("document", "ccx1-bench-flock.sh", "#!")),
        "timing_override": None,
        "arms": dict(sorted(arms.items())),
        "cells": planned,
        "max_cells_per_session": campaign["max_cells_per_session"],
    }


def repository_path(path):
    """`path` relative to the repository root, the form the runner pins; absolute outside it."""
    try:
        return path.relative_to(smoke.ROOT)
    except ValueError:
        return path


def write(path, document):
    partial = path.with_name(f".{path.name}.partial")
    partial.write_text(json.dumps(document, indent="  ") + "\n", encoding="utf-8")
    partial.replace(path)


def generate(declaration_path, out_dir, confirmation=False):
    out_dir = pathlib.Path(out_dir).resolve()
    declaration = json.loads(pathlib.Path(declaration_path).read_text(encoding="utf-8"))
    records = [build for _, build in peak.build_records()]
    (catalogue_path,) = {build["catalogue"]["path"] for build in records}
    catalogue = json.loads((smoke.ROOT / catalogue_path).read_text(encoding="utf-8"))
    profiles = tuning_profiles(declaration, catalogue["arms"])
    manifests = bundles(catalogue, out_dir)
    cells = [merged(declaration["cell"], cell) for cell in declaration["cells"]]
    if confirmation:
        frozen = json.loads((out_dir / ADDENDUM).read_text(encoding="utf-8"))
        cells = [cell for cell in cells
                 if cell["cell_id"] in {entry["cell_id"] for entry in frozen["cells"]}]
    else:
        addendum_schema = schema()
        write(out_dir / ADDENDUM, addendum(declaration, cells, manifests, catalogue["arms"],
                                           addendum_schema, shared_settings(addendum_schema),
                                           profiles))
    write(out_dir / PLAN, plan(declaration, cells, manifests, catalogue, records, profiles,
                               repository_path(out_dir / ADDENDUM),
                               "confirmation" if confirmation else "pilot"))


def main():
    match sys.argv:
        case [_, declaration, out_dir]:
            confirmation = False
        case [_, declaration, out_dir, "--confirmation"]:
            confirmation = True
        case _:
            sys.exit(__doc__)
    out_dir = pathlib.Path(out_dir).resolve()
    if pathlib.Path.cwd() != smoke.ROOT:
        sys.exit("invoke from the worktree root")
    generate(declaration, out_dir, confirmation)
    print(out_dir / ADDENDUM, out_dir / PLAN)


if __name__ == "__main__":
    main()
