#!/usr/bin/env python3
"""Freeze a family's exploratory re-measurement addendum after the warm-pass fix.

Usage: dev/active/6fb89a3c/survey/freeze-remeasure.py transpose|logical|bch <frozen-utc>

The family's v3 pilot and confirmation ran gf2 arms that reported `warm`
without the protocol's untimed pass before calibration. The re-measurement
repeats every cell of the family's committed v3 pilot addendum with the fixed
arms, and it is exploratory for a reason this script checks before writing:
the withdrawn confirmation's ledger reservation holds the identity of every
external candidate the repeated cells use, and each candidate executable is
still the byte-identical one, so `trial_ledger::reserve` would refuse a
second confirmatory reservation (one attempt per candidate identity and
protocol version). The script also counts, per cell, the committed campaigns
that started a pilot of the same question under protocol v1 or v3 and
declares the most-sampled cell's count, this campaign included, as the search
budget, refusing a count above the schema's cap. The addendum keeps the
pilot's cells, seeds and margins and is written to `<stem>-v3-remeasure.json`.
"""
from __future__ import annotations

import collections
import hashlib
import json
import pathlib
import sys

ISSUE = pathlib.Path("dev/active/6fb89a3c")
RESULTS = pathlib.Path("dev/bench_results/6fb89a3c")
SCHEMA = pathlib.Path("dev/active/f547c394/addendum.schema.json")
STEMS = {"transpose": "addendum-transpose", "logical": "addendum-logical-buffer", "bch": "addendum-bch-genmatrix"}
SPLITMIX = ("Workload seeds expand through SplitMix64 [Steele2014], implemented as tuning_campaign_support::abtest::SplitMix64 "
            "(dev/tools/tuning-campaign-support/src/abtest.rs) in the gf2 arms and as splitmix64_next "
            "(dev/active/6fb89a3c/survey/harness_common.h) in the C arms; both are pinned by the producing snapshot.")
# The confirmation addenda's seed statements, per family.
SEEDS = {
    "transpose": SPLITMIX,
    "logical": SPLITMIX + " Buffers draw one word per SplitMix64 output; source s uses seed + s.",
    "bch": ("The workload seed is carried for schema uniformity and no generator consumes it: the generator matrix is a "
            "deterministic function of the code."),
}


def records(log: pathlib.Path) -> list[dict]:
    return [json.loads(line) for line in log.read_text().splitlines()]


def candidate_identity(arm: dict) -> str:
    """`trial_ledger::candidate_ids`: the arm record without its description and
    executable path, serialized as a serde_json value (sorted keys, compact)."""
    record = {key: value for key, value in arm.items() if key not in ("description", "executable_path")}
    return hashlib.sha256(json.dumps(record, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def spent_candidates(family_id: str, ledger_path: str, pilot_receipt: dict, confirmation_dir: pathlib.Path) -> list[str]:
    """Checks that every candidate arm of the repeated cells is reserved by the
    confirmation and unchanged; returns the arm names."""
    receipt = json.loads((confirmation_dir / "receipt.json").read_text())
    reservation = [e for e in map(json.loads, pathlib.Path(ledger_path).read_text().splitlines())
                   if e["campaign"] == receipt["campaign_id"]]
    if receipt["family_id"] != family_id or len(reservation) != 1 or reservation[0]["comparisons"] == 0:
        raise SystemExit(f"{confirmation_dir}: no confirmatory reservation for {family_id}")
    names = sorted({cell["candidate_arm"] for cell in pilot_receipt["cells"]})
    for name in names:
        arm = receipt["arms"].get(name)
        if arm is None or candidate_identity(arm) not in reservation[0]["candidates"]:
            raise SystemExit(f"candidate {name} holds no confirmatory reservation; review the re-measurement role")
        current = ISSUE / "survey" / pathlib.Path(arm["executable_path"]).name
        if hashlib.sha256(current.read_bytes()).hexdigest() != arm["executable_sha256"]:
            raise SystemExit(f"candidate {name} executable changed since the confirmation; review the re-measurement role")
    return names


def pilot_history(family_ids: set[str]) -> dict[str, list[str]]:
    """Committed campaigns that started each exploratory cell of these families."""
    history: dict[str, list[str]] = collections.defaultdict(list)
    for log in sorted(RESULTS.glob("*/execution.log")):
        entries = records(log)
        snapshot = log.parent / entries[0]["details"]["addendum"]["snapshot"]
        addendum = json.loads(snapshot.read_text())
        if addendum["family"]["id"] not in family_ids:
            continue
        roles = {cell["cell_id"]: cell["role"] for cell in addendum["cells"]}
        started = {entry["case"]["cell_id"] for entry in entries if entry["event"] == "cell-start"}
        for cell in sorted(started):
            if roles[cell] == "exploratory":
                history[cell].append(log.parent.name)
    return history


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    family, frozen_utc = sys.argv[1], sys.argv[2]
    stem = STEMS[family]
    pilot = json.loads((ISSUE / f"{stem}-v3-pilot.json").read_text())
    confirmation = json.loads((ISSUE / f"{stem}-v3-confirmation.json").read_text())
    family_id = pilot["family"]["id"]
    ledger_path = pilot["family_wise"]["ledger_path"]
    evidence = confirmation["effect"]["resolution_evidence"]
    pilot_bytes = pathlib.Path(evidence["receipt"]).read_bytes()
    if hashlib.sha256(pilot_bytes).hexdigest() != evidence["sha256"]:
        raise SystemExit(f"{evidence['receipt']} differs from the confirmation's resolution evidence")
    pilot_receipt = json.loads(pilot_bytes)
    candidates = spent_candidates(family_id, ledger_path, pilot_receipt,
                                  RESULTS / f"v3-r1-6fb89a3c-{family}-confirmation")

    origin = json.loads((RESULTS / "v3-ledger-origin.json").read_text())
    predecessor = next(f["predecessor_question"] for f in origin["families"] if f["family"] == family_id)
    predecessor_id = predecessor.split()[-1]
    history = pilot_history({family_id, predecessor_id})
    trials = {cell["cell_id"]: len(history.get(cell["cell_id"], [])) + 1 for cell in pilot["cells"]}
    budget = max(trials.values())
    cap = json.loads(SCHEMA.read_text())["properties"]["search_budget"]["properties"]["max_pilot_trials_per_cell"]["maximum"]
    if budget > cap:
        raise SystemExit(f"{family}: a cell would reach pilot trial {budget}, above the cap {cap}")

    document = json.loads(json.dumps(pilot))
    description = pilot["family"]["description"]
    if not description.startswith("Exploratory pilot of"):
        raise SystemExit("pilot description changed; review the re-measurement text")
    description = "Exploratory re-measurement of" + description[len("Exploratory pilot of"):]
    for pilot_clause, replacement in ((" This pilot sizes the family's measurement resolution; no cell here decides adoption.", ""),
                                      ("; no cell here decides adoption.", ".")):
        description = description.replace(pilot_clause, replacement)
    description += (
        " The family's v3 pilot and confirmation ran gf2 arms that reported `warm` without the protocol's untimed pass "
        "over the working set before calibration, so their gf2 `warm` declarations are false and their comparisons are "
        "withdrawn. This campaign re-measures every cell of the v3 pilot addendum with fixed gf2 arms that make that pass "
        "(timed_windows in dev/active/6fb89a3c/survey/gf2-side/src/lib.rs). Protocol v3 allows one confirmatory attempt "
        f"per candidate identity and version; the withdrawn confirmation's ledger reservation holds the identity of every "
        f"external candidate these cells use ({', '.join(candidates)}), whose executables are unchanged, so every cell here "
        "is exploratory and no interval carries a confirmatory decision. Pilot trials per cell, counting every committed "
        f"campaign that started a pilot of this question under protocol v1 (family {predecessor_id}) or v3 and this "
        "campaign: " + ", ".join(f"{cell} {n}" for cell, n in trials.items())
        + f". The search budget declares the maximum, within the protocol cap of {cap}; the earlier v3 addenda counted v3 "
        "trials only. " + SEEDS[family] + " This survey adopts no production implementation; no cell here decides adoption.")
    document["family"]["description"] = description
    document["frozen"] = {"frozen_utc": frozen_utc}
    effect = document["effect"]
    effect["measurement_resolution"] = None
    effect["resolution_evidence"] = None
    effect["equivalence_margin"] = confirmation["effect"]["equivalence_margin"]
    effect["material_gap_threshold"] = confirmation["effect"]["material_gap_threshold"]
    effect["equivalence_rationale"] = (
        "The one-sided non-inferiority margin the withdrawn confirmation froze; exploratory cells record decisions under "
        "it and never pass.")
    effect["material_gap_rationale"] = (
        confirmation["effect"]["material_gap_rationale"].split("; it exceeds one plus the frozen resolution")[0]
        + "; it is the threshold the withdrawn confirmation froze.")
    document["search_budget"]["max_pilot_trials_per_cell"] = budget
    for cell in document["cells"]:
        if cell["role"] != "exploratory" or cell["cache_state"] != "warm":
            raise SystemExit(f"pilot cell {cell['cell_id']} is not an exploratory warm cell")

    def cell_text(c):
        return ("    {\n"
                f"      \"cell_id\": {json.dumps(c['cell_id'])}, \"objective\": {json.dumps(c['objective'])}, \"role\": {json.dumps(c['role'])},\n"
                f"      \"workload\": {json.dumps(c['workload'], separators=(', ', ': '))},\n"
                f"      \"metric_kind\": {json.dumps(c['metric_kind'])}, \"scaling\": {json.dumps(c['scaling'])}, \"core_arm\": {json.dumps(c['core_arm'])},\n"
                f"      \"workers\": {json.dumps(c['workers'], separators=(', ', ': '))}, \"cache_state\": {json.dumps(c['cache_state'])},\n"
                f"      \"builds\": {json.dumps(c['builds'], separators=(', ', ': '))}, \"conversion_costs_included\": {json.dumps(c['conversion_costs_included'])}, \"decoder\": null\n"
                "    }")
    head = {k: v for k, v in document.items() if k != "cells"}
    text = "{\n" + ",\n".join(
        f"  {json.dumps(k)}: " + (json.dumps(v, indent=2).replace("\n", "\n  ") if k in ("family", "effect") else json.dumps(v, separators=(', ', ': ')))
        for k, v in head.items())
    text += ",\n  \"cells\": [\n" + ",\n".join(cell_text(c) for c in document["cells"]) + "\n  ]\n}\n"
    if json.loads(text) != document:
        raise SystemExit("rendered addendum differs from the derived document")
    output = ISSUE / f"{stem}-v3-remeasure.json"
    output.write_text(text)
    print(f"{family}: {len(document['cells'])} exploratory cells; spent candidates {', '.join(candidates)}; "
          f"pilot trials {trials}; max_pilot_trials_per_cell {budget} -> {output}")
    for cell, campaigns in sorted(history.items()):
        print(f"  {cell}: {', '.join(campaigns)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
