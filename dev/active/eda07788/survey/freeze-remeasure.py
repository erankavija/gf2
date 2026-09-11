#!/usr/bin/env python3
"""Freeze the DVB-T2 family's exploratory re-measurement addendum (jit:3e59cb9a).

Usage: dev/active/eda07788/survey/freeze-remeasure.py <frozen-utc>  (from the repo root)

Every earlier DVB-T2 campaign ran arms whose `warm` branch skipped the
protocol's untimed pass over the working set before calibration. The
re-measurement repeats every cell of the committed v3 pilot addendum with the
repaired arms. Before writing the addendum this script checks the facts its
description states:

- The withdrawn v3 confirmation's ledger reservation holds the candidate
  identity of every comparator the repeated cells use, recomputed as
  `trial_ledger::candidate_ids` does, and each repaired executable that
  `survey/validation-output-v3-remeasure.txt` records differs from the
  reserved one: a new identity for the same comparator.
- Under the committed ledger, the next non-exploratory reservation would be a
  further attempt holding at least one more comparison, and even then P-20's
  expected draws per bootstrap tail stay below the required twenty. So no
  confirmatory cell could be confirmatory, and every cell here is exploratory.
- Pilot trials per cell count every committed campaign that started the cell
  as an exploratory cell of this question, under the family's id or the v1
  pilot's id, plus this campaign. The search budget declares the maximum and
  must not exceed the schema's cap.

The addendum keeps the v3 pilot's cells, seeds and objectives and the v3
confirmation's margins, and is written to
`dev/active/eda07788/addendum-dvb-t2-bit-interleave-v3-remeasure.json`.
"""
from __future__ import annotations

import collections
import hashlib
import json
import pathlib
import re
import sys

ISSUE = pathlib.Path("dev/active/eda07788")
RESULTS = pathlib.Path("dev/bench_results/eda07788")
PILOT = ISSUE / "addendum-dvb-t2-bit-interleave-v3-pilot.json"
CONFIRMATION = ISSUE / "addendum-dvb-t2-bit-interleave-v3-confirmation.json"
CONFIRMATION_RECEIPT = RESULTS / "2026-09-08-eda07788-dvb-t2-v3-confirmation/receipt.json"
VALIDATION = ISSUE / "survey/validation-output-v3-remeasure.txt"
SCHEMA = pathlib.Path("dev/active/f547c394/addendum.schema.json")
OUTPUT = ISSUE / "addendum-dvb-t2-bit-interleave-v3-remeasure.json"
# The v1 pilot froze its cells under its own family id; they are the same
# question as the continuing family's.
QUESTION_IDS = ("dvb-t2-bit-interleave-pilot", "dvb-t2-bit-interleave-baselines")
TAIL_DRAWS_REQUIRED = 20.0
IMPLEMENTED_CACHE_STATES = ("warm", "streaming")
# The comparator each cell-identifier suffix's candidate arm launches, named
# as the v3 confirmation's arms name it. run-dvb-t2-baselines.sh wires the
# identity cell's candidate `gf2-native-control` to the gf2-native executable.
COMPARATORS = {
    "-null-native-vs-native": "gf2-native",
    "-gap-native-vs-xdsopl": "xdsopl-external",
    "-gap-portable-vs-xdsopl": "xdsopl-external",
    "-control-portable-vs-native": "gf2-native",
}
EXECUTABLES = {
    "gf2-native": "target-native/release/gf2-dvb-t2-candidate",
    "xdsopl-external": "target-native/release/xdsopl-dvb-t2-baseline",
}
CONVENTION = (
    "Every cell is a whole-consumer comparison of two arms of the same operation on the same seeded frame. The "
    "baseline arm is always the established gf2 implementation and the candidate arm is always the arm whose lead "
    "needs attribution, so a speedup above 1 means the candidate is ahead: above 1 in a `-gap-` cell means the "
    "xdsopl/LDPC comparator is ahead of gf2, above 1 in a `-control-` cell means the `-C target-cpu=native` gf2 build "
    "is ahead of the conservative-portable gf2 control."
)


def candidate_identity(arm: dict) -> str:
    """`trial_ledger::candidate_ids`: the arm record without its description and
    executable path, serialized as a serde_json value (sorted keys, compact)."""
    record = {key: value for key, value in arm.items() if key not in ("description", "executable_path")}
    return hashlib.sha256(json.dumps(record, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def recorded_digests() -> dict[str, str]:
    """Arm digests the committed preflight output lists, by staging-relative path."""
    digests = {}
    for line in VALIDATION.read_text().splitlines():
        match = re.fullmatch(r"([0-9a-f]{64})  (target-[a-z]+/release/[a-z0-9-]+)", line)
        if match:
            digests[match.group(2)] = match.group(1)
    if len(digests) != 3:
        raise SystemExit(f"{VALIDATION} lists {len(digests)} arm digests, expected 3")
    return digests


def comparator(cell_id: str) -> str:
    for suffix, name in COMPARATORS.items():
        if cell_id.endswith(suffix):
            return name
    raise SystemExit(f"cell {cell_id!r} names no known arm pair")


def rebuilt_comparators(cells: list[dict], ledger: list[dict]) -> list[str]:
    """Checks that the confirmation reserved every comparator the cells use and
    that each repaired executable differs from the reserved one."""
    receipt = json.loads(CONFIRMATION_RECEIPT.read_text())
    reservation = [entry for entry in ledger if entry["campaign"] == receipt["campaign_id"]]
    if len(reservation) != 1 or reservation[0]["comparisons"] == 0:
        raise SystemExit(f"{CONFIRMATION_RECEIPT}: no confirmatory reservation in the ledger")
    digests = recorded_digests()
    comparators = sorted({comparator(cell["cell_id"]) for cell in cells})
    for name in comparators:
        arm = receipt["arms"][name]
        if candidate_identity(arm) not in reservation[0]["candidates"]:
            raise SystemExit(f"{name} holds no confirmatory reservation; review the re-measurement role")
        if digests[EXECUTABLES[name]] == arm["executable_sha256"]:
            raise SystemExit(f"{name}'s executable is unchanged; review the re-measurement role")
    return comparators


def pilot_history() -> dict[str, list[str]]:
    """Committed campaigns that started each cell as an exploratory cell."""
    history: dict[str, list[str]] = collections.defaultdict(list)
    for log in sorted(RESULTS.glob("*/execution.log")):
        entries = [json.loads(line) for line in log.read_text().splitlines()]
        snapshot = log.parent / entries[0]["details"]["addendum"]["snapshot"]
        addendum = json.loads(snapshot.read_text())
        if addendum["family"]["id"] not in QUESTION_IDS:
            continue
        roles = {cell["cell_id"]: cell["role"] for cell in addendum["cells"]}
        started = {entry["case"]["cell_id"] for entry in entries if entry["event"] == "cell-start"}
        for cell in sorted(started):
            if roles[cell] == "exploratory":
                history[cell].append(log.parent.name)
    return history


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    frozen_utc = sys.argv[1]
    if not re.fullmatch(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ", frozen_utc):
        raise SystemExit(f"frozen time {frozen_utc!r} is not a whole-second UTC time")
    pilot = json.loads(PILOT.read_text())
    confirmation = json.loads(CONFIRMATION.read_text())
    if CONVENTION not in pilot["family"]["description"]:
        raise SystemExit("the v3 pilot's arm convention changed; review the re-measurement text")
    for cell in pilot["cells"]:
        if cell["role"] != "exploratory" or cell["cache_state"] not in IMPLEMENTED_CACHE_STATES:
            raise SystemExit(f"pilot cell {cell['cell_id']} is not an exploratory cell the arms implement")

    ledger_path = pilot["family_wise"]["ledger_path"]
    ledger = [json.loads(line) for line in pathlib.Path(ledger_path).read_text().splitlines()]
    comparators = rebuilt_comparators(pilot["cells"], ledger)
    attempts = sum(1 for entry in ledger if entry["comparisons"] > 0)
    comparisons = sum(entry["comparisons"] for entry in ledger)
    attempt = attempts + 1
    alpha = pilot["family_wise"]["alpha"]
    settings = json.loads(CONFIRMATION_RECEIPT.read_text())["settings"]
    resamples = settings["bootstrap_resamples"]
    tail = resamples * alpha / (attempt * (attempt + 1)) / (comparisons + 1) / 2
    if tail >= TAIL_DRAWS_REQUIRED:
        raise SystemExit(f"a further confirmatory attempt could meet P-20 ({tail:.2f} draws); review the role")

    history = pilot_history()
    trials = {cell["cell_id"]: len(history.get(cell["cell_id"], [])) + 1 for cell in pilot["cells"]}
    budget = max(trials.values())
    cap = json.loads(SCHEMA.read_text())["properties"]["search_budget"]["properties"]["max_pilot_trials_per_cell"]["maximum"]
    if budget > cap:
        raise SystemExit(f"a cell would reach pilot trial {budget}, above the cap {cap}")

    description = (
        "Exploratory re-measurement of the DVB-T2 ETSI EN 302 755 v1.4.1 SS6.1.3 bit interleaver [Etsi2015] (parity "
        "interleaving composed with column twisting) with arms that apply the declared cache state. " + CONVENTION
        + " Every earlier campaign of this question (the v1 pilot and confirmation, the v3 pilot, pilot r2 and "
        "confirmation) ran arms whose `warm` branch skipped the protocol's untimed pass over the working set before "
        "calibration (claims survey-v3-gf2-arm-warm-defect and survey-v3-external-arm-warm-defect in "
        "dev/active/eda07788/survey/source-evidence.json), so their `warm` declarations are false and every warm "
        "cell's comparison is withdrawn; the `streaming` cells applied the rotation they declared. This campaign "
        f"repeats every cell of {PILOT.name} with the repaired arms (timed_windows in "
        f"dev/active/eda07788/survey/gf2-side/src/lib.rs), whose executable digests {VALIDATION} records, at the "
        f"pilot maximum of {settings['pilot_max_pairs']} pairs per cell. "
        "Every cell is exploratory. The withdrawn v3 confirmation spent the one confirmatory attempt per candidate "
        f"identity and protocol version for {' and '.join(comparators)}, whose identities its ledger reservation "
        "holds; the repaired executables are new identities for the same comparators, and this family opens no "
        f"confirmatory attempt with them. The committed ledger holds {attempts} non-exploratory reservations and "
        f"{comparisons} comparisons, so a further confirmatory attempt would be attempt {attempt} with attempt alpha "
        f"{alpha}/({attempt}*{attempt + 1}) over at least {comparisons + 1} comparisons: even one confirmatory cell "
        f"would expect {tail:.2f} draws per bootstrap tail at {resamples} resamples, against the "
        f"{TAIL_DRAWS_REQUIRED:.0f} that P-20 requires. This campaign sizes no measurement resolution and feeds no "
        "confirmation. Pilot trials per cell, counting every committed campaign that started the cell as an "
        f"exploratory cell under family id {' or '.join(QUESTION_IDS)}, and this campaign: "
        + ", ".join(f"{cell} {count}" for cell, count in trials.items())
        + f". The search budget declares the maximum, within the protocol cap of {cap}; the earlier v3 addenda "
        "counted v3 trials only. Workload seeds expand through SplitMix64 [Steele2014], implemented as "
        "tuning_campaign_support::abtest::SplitMix64 (dev/tools/tuning-campaign-support/src/abtest.rs) and consumed "
        "by seeded_word_banks in dev/active/eda07788/survey/gf2-side/src/lib.rs; the producing snapshot pins both. "
        "This survey adopts no production implementation; no cell here decides adoption."
    )
    document = json.loads(json.dumps(pilot))
    document["family"]["description"] = description
    document["frozen"] = {"frozen_utc": frozen_utc}
    effect = document["effect"]
    effect["measurement_resolution"] = None
    effect["resolution_evidence"] = None
    effect["equivalence_margin"] = confirmation["effect"]["equivalence_margin"]
    effect["material_gap_threshold"] = confirmation["effect"]["material_gap_threshold"]
    effect["equivalence_rationale"] = (
        "The one-sided non-inferiority margin the withdrawn v3 confirmation froze; exploratory cells record decisions "
        "under it and never pass.")
    effect["material_gap_rationale"] = (
        "A 20% whole-consumer wall-clock gap is the smallest gap between two arms of this operation worth attributing "
        "to representation conversion, permutation-table traffic or codegen level rather than host noise; it is the "
        "threshold the withdrawn v3 confirmation froze.")
    if f"{effect['material_gap_threshold'] - 1:.0%}" != "20%":
        raise SystemExit("the confirmation's material-gap threshold changed; review the rationale")
    document["search_budget"]["max_pilot_trials_per_cell"] = budget

    OUTPUT.write_text(json.dumps(document, indent=2) + "\n")
    print(f"{len(document['cells'])} exploratory cells; comparators {', '.join(comparators)} rebuilt; "
          f"further confirmatory attempt {attempt}: {tail:.4f} expected tail draws; "
          f"max_pilot_trials_per_cell {budget} -> {OUTPUT}")
    for cell, campaigns in sorted(history.items()):
        print(f"  {cell}: {', '.join(campaigns)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
