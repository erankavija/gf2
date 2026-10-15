#!/usr/bin/env python3
"""Smoke the batch decoder arms through the shared runner (jit:ed3d490e).

Times nothing. For each base addendum of the chosen group it writes a
throwaway addendum and plan under `target/`, runs `benchmark-ab-runner check`,
then `benchmark-ab-runner smoke` over a validation stage, whose contract
`tuning_campaign_support::arm::smoke` states: every arm of every cell runs
once in the untimed validation position. It writes the runner's smoke record
and the group's diagnostics file, the placement reports and decoder
diagnostics the stage journalled from each arm's standard error, without
their clock readings. It then gives an arm an unusable prepared quality
record, requires the smoke to fail, and records the arm's refusal: a record
whose first frame count is one too high, and for the AFF3CT arm also none.

Group `gf2`, the default, smokes the gf2 arms. Group `aff3ct` smokes the
AFF3CT [Cassagne2019] sum-product INTRA arm of the workspace's member package
against the gf2 single-frame arm on the `07ca8585` comparator pilot addendum.
Its prepared quality records are the committed ones of `record-intra-quality.py`.

The throwaway addenda are the `07ca8585` before/after pilot addenda with the
family renamed and the ledger redirected into `target/`. The single-worker one
gains two cells per code: a one-frame batch, and an eleven-frame batch whose
last wave is partial and which sets the portable lane backend against the
AVX2 one. The recorded bundles and the prepared quality records are the
`c077a88b` ones, found among the repository's files by the digests below.

Usage (from the worktree root): smoke-gf2-arms.py OUT_DIR [gf2|aff3ct]
"""

import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tarfile

TOOLCHAIN = "1.95"
RUSTFLAGS = "-C target-cpu=native"
PACKAGE = "ldpc-batch-arms"
SCRATCH = pathlib.Path("target/ldpc-batch-arms-smoke")
LOCK = "/tmp/gf2-ccx1.lock"

# Identities of the frozen `c077a88b` inputs.
ARCHIVE = ("recorded-inputs.tar.gz",
           "a7c47dfb9d345b0ab01ad2feca82c512963f963776036c8d9bd761f3640cdfba")
CODES = {
    "dvb-t2-r12": {
        "bundle": "dvb-t2-r12-waterfall",
        "code": "dvb-t2-r12",
        "quality": ("gf2-nms-f32-dvb-t2-r12.json",
                    "15fd3cb1c0818b6c14780f9ba74961ea9f52d38b216f883ea3b4c28c19af3ca6"),
        "intra_quality": ("aff3ct-spa-intra-dvb-t2-r12.json",
                          "3fe8c404432a10f81a06853c3e8351795227968cbe5e4956fbf5e985264c6946"),
    },
    "nr-bg1-z384": {
        "bundle": "nr-bg1-z384-mother",
        "code": "nr-bg1-r12",
        "quality": ("gf2-nms-f32-nr-bg1-z384.json",
                    "d93ca58e2b19539ceb7e4cd71cb5e43046dc73390d218d86f4e4b3e06cefa42f"),
        "intra_quality": ("aff3ct-spa-intra-nr-bg1-z384.json",
                          "710cec4ac7235fcf89a352ad56f6566c24719fa1c7bfc42938c765cca4ca9141"),
    },
}

CATALOGUE = json.loads(pathlib.Path(__file__).resolve().with_name("arms.json")
                       .read_text(encoding="utf-8"))["arms"]
ARMS = {
    "single-frame": ("gf2-batch-single-frame-arm", {}),
    "lane-avx2": ("gf2-batch-lane-arm", {"GF2_LDPC_LANE_BACKEND": "avx2"}),
    "lane-portable": ("gf2-batch-lane-arm", {"GF2_LDPC_LANE_BACKEND": "portable"}),
    "spa-intra": ("aff3ct-intra-arm", CATALOGUE["aff3ct-spa-intra"]["environment"]),
}
# Plan name and description of each arm built outside the gf2 tree.
EXTERNAL = {
    "spa-intra": ("aff3ct-spa-intra", "AFF3CT float sum-product INTRA decoder"),
}
# Base addendum, then (cell suffix, per-worker batch, baseline, candidate) of
# the cells derived from each of its cells; no suffix keeps the cell.
FAMILIES = {
    "single-worker": ("addendum-ldpc-update-single-worker-pilot.json", [
        ("", None, "single-frame", "lane-avx2"),
        ("-b1", 1, "single-frame", "lane-avx2"),
        ("-b11", 11, "lane-portable", "lane-avx2"),
    ]),
    "multicore": ("addendum-ldpc-update-multicore-pilot.json", [
        ("", None, "single-frame", "lane-avx2"),
    ]),
    "aff3ct-intra": ("addendum-ldpc-update-comparator-single-worker-pilot.json", [
        ("", None, "single-frame", "spa-intra"),
    ]),
}
PLACEMENT = "worker-placement"
DECODER_DIAGNOSTICS = ("batch-decoder-diagnostics", "aff3ct-decoder-diagnostics")
DIFFERING = "the prepared quality records"
# Per group: the issue its throwaway plans name, what they smoke, its
# families, its diagnostics file, the key of its refusals there, the
# `decoder.arm_kind` its cells declare when not the base addendum's, and its
# refusal plans as name: (base family, arm that must refuse, how its quality
# record is unusable, text of its refusal).
GROUPS = {
    "gf2": {
        "issue": "feb60d0d",
        "subject": "gf2 batch decoder arms",
        "families": ("single-worker", "multicore"),
        "diagnostics": "arm-diagnostics.json",
        "refusal_key": "differing_quality_refused",
        "arm_kind": None,
        "refusals": {
            "refused-single-frame": ("single-worker", [("", None, "single-frame", "lane-avx2")],
                                     "single-frame", "differing", DIFFERING),
            "refused-lane": ("single-worker", [("", None, "lane-avx2", "single-frame")],
                             "lane-avx2", "differing", DIFFERING),
        },
    },
    "aff3ct": {
        "issue": "bad1aea8",
        "subject": "AFF3CT sum-product INTRA arm",
        "families": ("aff3ct-intra",),
        "diagnostics": "aff3ct-arm-diagnostics.json",
        "refusal_key": "unusable_quality_refused",
        "arm_kind": "fastest-quality-compatible",
        "refusals": {
            "refused-intra-differing": ("aff3ct-intra", [("", None, "single-frame", "spa-intra")],
                                        "spa-intra", "differing", DIFFERING),
            "refused-intra-missing": ("aff3ct-intra", [("", None, "single-frame", "spa-intra")],
                                      "spa-intra", "missing", "GF2_LDPC_QUALITY is unset"),
        },
    },
}


def output(command):
    return subprocess.run(command, check=True, capture_output=True, text=True).stdout.strip()


ROOT = pathlib.Path(output(["git", "rev-parse", "--show-toplevel"]))


def listed(name):
    """Root-relative files git lists under the name `name`."""
    return output(["git", "-C", str(ROOT), "ls-files", "--cached", "--others",
                   "--exclude-standard", "--", f":(glob)**/{name}"]).splitlines()


def located(*query):
    """Root-relative path the repository-file helper prints for `query`."""
    helper = min(listed("repository_files.py"), key=len)
    return pathlib.Path(output([sys.executable, "-B", str(ROOT / helper), *query]))


def by_digest(identity):
    """The first listed file called `identity[0]` whose SHA-256 is `identity[1]`."""
    name, pinned = identity
    for path in sorted(listed(name)):
        if hashlib.sha256((ROOT / path).read_bytes()).hexdigest() == pinned:
            return ROOT / path
    sys.exit(f"no file called {name} has the pinned digest {pinned}")


def release():
    """Root-relative release directory of the arms workspace."""
    return located("package-directory", PACKAGE) / "target/release"


def prepared(arm, key):
    return by_digest(CODES[key]["quality" if arm not in EXTERNAL else "intra_quality"])


def differing(arm, key):
    """A copy of the prepared quality record with one more error in frame 0."""
    record = json.loads(prepared(arm, key).read_text(encoding="utf-8"))
    record["frame_bit_errors"][0] += 1
    path = ROOT / SCRATCH / f"differing-{arm}-{key}.json"
    path.write_text(json.dumps(record), encoding="utf-8")
    return path


UNUSABLE = {"differing": differing, "missing": lambda arm, key: None}


def throwaway(group, family, base, derived, refusing=None, unusable=None):
    """Writes the throwaway addendum and plan of `family` and returns the plan path.

    The arm `refusing` gets its quality record from `UNUSABLE[unusable]`.
    """
    addendum = json.loads((ROOT / located("file", base)).read_text(encoding="utf-8"))
    addendum["family"].update({
        "id": f"ldpc-batch-arms-smoke-{family}",
        "issue": group["issue"],
        "description": f"Throwaway wire-contract smoke of the {group['subject']}. It "
                       "publishes no receipt and supports no performance claim.",
    })
    ledger = SCRATCH / f"{family}-ledger.jsonl"
    ledger.touch()
    addendum["family_wise"]["ledger_path"] = str(ledger)

    arms, cells, declared_cells = {}, [], []
    for declared in addendum["cells"]:
        key = next(key for key in CODES if declared["cell_id"].startswith(key + "-"))
        code = CODES[key]
        for suffix, batch, baseline, candidate in derived:
            cell = json.loads(json.dumps(declared))
            cell["cell_id"] += suffix
            size = cell["workload"]["size"]
            if batch is not None:
                size["timed_batch"] = batch
                size["timed_frames"] = batch * size["workers"]
            if group["arm_kind"] is not None:
                cell["decoder"]["arm_kind"] = group["arm_kind"]
            declared_cells.append(cell)
            decoder = cell["decoder"]
            case = {
                "bundle": str(ROOT / SCRATCH / "inputs" / code["bundle"]),
                "code": code["code"],
                "iteration_cap": decoder["iteration_cap"],
                "normalization_factor": decoder["normalization"]["factor"],
                "syndrome_stopping": decoder["stopping"]["kind"] == "syndrome",
                "batch_size": size["timed_batch"],
                "quality_frames": decoder["input"]["frames"],
            }
            names = []
            for arm in (baseline, candidate):
                executable, environment = ARMS[arm]
                build, prefix, description = ("external", *EXTERNAL[arm]) if arm in EXTERNAL else (
                    "native", f"gf2-batch-{arm}", f"gf2 LDPC batch decoder, {arm} route")
                name = f"{prefix}-{key}"
                quality = (UNUSABLE[unusable] if arm == refusing else prepared)(arm, key)
                arms[name] = {
                    "build": build,
                    "description": description,
                    "executable": str(ROOT / release() / executable),
                    "arguments": [],
                    "environment": {**environment, **(
                        {} if quality is None else {"GF2_LDPC_QUALITY": str(quality)})},
                    "rustflags": RUSTFLAGS,
                    "tuning_profile": None,
                }
                names.append(name)
            cells.append({
                "cell_id": cell["cell_id"],
                "baseline_arm": names[0],
                "candidate_arm": names[1],
                "case": case,
                "pilot_pairs": 6,
            })
    addendum["cells"] = declared_cells
    addendum_path = SCRATCH / f"{family}-addendum.json"
    addendum_path.write_text(json.dumps(addendum, indent=2) + "\n", encoding="utf-8")
    plan = {
        "schema": "zen3-benchmark-plan-v1",
        "campaign_id": f"{group['issue']}-smoke-{family}",
        "issue": group["issue"],
        "label": "pilot",
        "campaign_seed": 0,
        "addendum": str(addendum_path),
        "producing_manifest": str(located("shared-producing-manifest")),
        "lock_path": LOCK,
        "wrapper": str(located("document", "ccx1-bench-flock.sh", "#!")),
        "timing_override": None,
        "arms": arms,
        "cells": cells,
        "max_cells_per_session": len(cells),
    }
    plan_path = SCRATCH / f"{family}.plan.json"
    plan_path.write_text(json.dumps(plan, indent=2) + "\n", encoding="utf-8")
    return plan_path


def child_diagnostics(stage):
    """The child-diagnostic events of a stage's execution log.

    A trailing line without its newline is an append an interruption tore off; the runner
    recovers it when it resumes the stage, so it is not read here.
    """
    text = (stage / "execution.log").read_text(encoding="utf-8")
    lines = text.splitlines()[:text.count("\n")]
    return [event for event in map(json.loads, lines) if event["event"] == "child-diagnostic"]


def diagnostics(stage):
    """The diagnostic lines each validation child wrote, without clock readings."""
    rows = []
    for event in child_diagnostics(stage):
        reports = {}
        for text in event["details"]["stderr"].splitlines():
            try:
                report = json.loads(text)
            except ValueError:
                continue
            if isinstance(report, dict) and report.get("kind") in (
                    PLACEMENT, *DECODER_DIAGNOSTICS):
                reports[report.pop("kind")] = report
        decoder = reports.get("batch-decoder-diagnostics")
        if decoder is not None:
            # Nanosecond costs vary from run to run; the record keeps which the arm reports.
            decoder["conversion"] = sorted(decoder["conversion"])
        rows.append({"cell_id": event["case"]["cell_id"], "arm": event["case"]["arm"], **reports})
    return rows


def build_runner():
    """Path of the built runner and the environment its plans run under."""
    environment = {**os.environ, "CARGO_CI_NO_SCCACHE": "1", "RAYON_NUM_THREADS": "1",
                   "RUSTUP_TOOLCHAIN": TOOLCHAIN}
    subprocess.run(["./scripts/cargo-budget.sh", "cargo", f"+{TOOLCHAIN}", "build", "--offline",
                    "--release", "-p", "tuning-campaign-support", "--bin",
                    "benchmark-ab-runner"], check=True, env=environment)
    return str(ROOT / "target/release/benchmark-ab-runner"), environment


def main():
    if len(sys.argv) not in (2, 3) or sys.argv[2:] not in ([], *([name] for name in GROUPS)):
        sys.exit(__doc__)
    if pathlib.Path.cwd() != ROOT:
        sys.exit("invoke from the worktree root")
    out_dir = pathlib.Path(sys.argv[1])
    group = GROUPS[(sys.argv[2:] or ["gf2"])[0]]
    shutil.rmtree(SCRATCH, ignore_errors=True)
    (SCRATCH / "inputs").mkdir(parents=True)
    with tarfile.open(by_digest(ARCHIVE)) as archive:
        archive.extractall(SCRATCH / "inputs", filter="data")

    runner, environment = build_runner()
    out_dir.mkdir(parents=True, exist_ok=True)
    observed = {}
    for family in group["families"]:
        plan = str(throwaway(group, family, *FAMILIES[family]))
        stage = SCRATCH / f"{family}-stage"
        subprocess.run([runner, "check", plan], check=True, env=environment)
        subprocess.run([runner, "smoke", plan, "--stage", str(stage), "--record",
                        str(out_dir / f"smoke-{family}.json")], check=True, env=environment)
        observed[family] = diagnostics(stage)
        dispatches = 2 * len(json.loads(pathlib.Path(plan).read_text(encoding="utf-8"))["cells"])
        if len(observed[family]) != dispatches or any(
                PLACEMENT not in row or not set(DECODER_DIAGNOSTICS) & row.keys()
                for row in observed[family]):
            sys.exit(f"{family}: an arm wrote no placement report or decoder diagnostics")
    refused = []
    for family, (base, derived, refusing, unusable, refusal) in group["refusals"].items():
        plan = str(throwaway(group, family, FAMILIES[base][0], derived, refusing, unusable))
        stage = SCRATCH / f"{family}-stage"
        smoke = subprocess.run([runner, "smoke", plan, "--stage", str(stage)], env=environment,
                               capture_output=True, text=True)
        event = child_diagnostics(stage)[-1]
        message = event["details"]["stderr"].splitlines()[-1]
        if smoke.returncode == 0 or refusal not in message:
            sys.exit(f"{family}: the arm accepted a {unusable} quality record")
        refused.append({"cell_id": event["case"]["cell_id"], "arm": event["case"]["arm"],
                        "error": message})
    path = out_dir / group["diagnostics"]
    path.write_text(json.dumps({"schema": "ldpc-batch-arms-diagnostics-v1", **observed,
                                group["refusal_key"]: refused},
                               indent=2) + "\n", encoding="utf-8")
    print(path)


if __name__ == "__main__":
    main()
