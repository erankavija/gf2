#!/usr/bin/env python3
"""Freeze a matched confirmation addendum from its accepted pilot (jit:3be770d5).

The protocol requires a confirmatory family to declare its margins against a
resolution an accepted pilot of the same family observed, and to pin that
pilot by path and digest. The confirmations run under protocol v4, which
derives the pilot resolution exactly as v3 does and admits a v3 pilot as the
resolution evidence of a v4 receipt (`amendment-v4.md`), so the pilots stay
v3 and this script raises the addendum's protocol version. It performs the
derivation mechanically:
it verifies that the independent acceptance summary accepts the exact pilot
receipt bytes, takes the widest conservative relative half-width of the pilot
cells' bootstrap intervals (the larger endpoint distance over the estimate),
rounds it up to the next half percent, and writes the confirmatory addendum:
every cell confirmatory, equivalence margin one plus twice and material-gap
threshold one plus four times the frozen resolution, both strictly outside it
as the protocol requires. A derivation record beside the addendum pins the
pilot receipt, summary, addendum and ledger snapshots.

Usage (from the worktree root):
  freeze-addendum.py single-worker|multicore RUN_ID
"""

import datetime
import hashlib
import json
import math
import pathlib
import sys

ACTIVE = pathlib.Path("dev/active/3be770d5")
RESULTS = pathlib.Path("dev/bench_results/3be770d5")
IDENTITY = RESULTS / "preparation/build-identity.json"
# The protocol version the confirmations run under; the pilots they derive
# from stay at the version they were measured under.
PROTOCOL_VERSION = 4


def sha(data):
    return hashlib.sha256(data).hexdigest()


def relative_half_width(cell):
    interval = cell["interval"]
    if interval["estimate"] <= 0:
        raise SystemExit("a pilot cell reported a non-positive speedup")
    return max(abs(interval["estimate"] - interval["lower"]),
               abs(interval["upper"] - interval["estimate"])) / interval["estimate"]


def main():
    if len(sys.argv) != 3 or sys.argv[1] not in ("single-worker", "multicore"):
        sys.exit(__doc__)
    family_key, run_id = sys.argv[1:]
    family = f"ldpc-steady-matched-{family_key}-v1"
    pilot_path = ACTIVE / f"addendum-ldpc-steady-matched-{family_key}-pilot.json"
    output = ACTIVE / f"addendum-ldpc-steady-matched-{family_key}.json"
    receipt_path = RESULTS / f"{run_id}-3be770d5-ldpc-steady-{family_key}-pilot/receipt.json"
    if output.exists():
        sys.exit(f"{output} exists; a frozen addendum is not rewritten")
    receipt_bytes = receipt_path.read_bytes()
    receipt = json.loads(receipt_bytes)
    summary_path = receipt_path.parent / "acceptance-summary.json"
    summary_bytes = summary_path.read_bytes()
    summary = json.loads(summary_bytes)
    if receipt["label"] != "pilot" or receipt["family_id"] != family:
        sys.exit("the resolution evidence must be a pilot receipt of the same family")
    if summary["receipt_sha256"] != sha(receipt_bytes) or summary["verdict"] != "accepted":
        sys.exit("the acceptance summary must accept the exact pilot receipt bytes")
    for pin in (receipt["addendum"], receipt["trial_ledger"]):
        if sha((receipt_path.parent / pin["snapshot"]).read_bytes()) != pin["sha256"]:
            sys.exit("the pilot addendum or ledger snapshot differs from its pin")
    widths = {cell["cell_id"]: relative_half_width(cell) for cell in summary["cells"] if cell.get("interval")}
    if len(widths) != len(receipt["cells"]):
        sys.exit("every pilot cell must carry an interval")
    observed = max(widths.values())
    resolution = math.ceil(observed * 200.0) / 200.0

    document = json.loads(pilot_path.read_text())
    if document["family"]["id"] != family:
        sys.exit("the pilot addendum belongs to another family")
    pilot_version = document["protocol"]["version"]
    document["schema"] = f"zen3-benchmark-addendum-v{PROTOCOL_VERSION}"
    document["protocol"]["version"] = PROTOCOL_VERSION
    description = document["family"]["description"].replace("Exploratory pilot of", "Confirmatory comparison of", 1)
    description = description.replace(
        "It observes the measurement resolution that the confirmatory addendum of this family freezes; "
        "it decides nothing.",
        "It estimates comparator gaps under frozen decision margins and adopts nothing.", 1)
    identity = IDENTITY.read_bytes()
    document["family"]["description"] = description + (
        f" Executables, toolchain and AFF3CT identity are pinned by {IDENTITY} (SHA-256 {sha(identity)}); "
        "the receipt snapshots this record and the complete producing-input closure.")
    document["frozen"]["frozen_utc"] = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    document["effect"].update({
        "worthwhile_speedup": None,
        "rationale": ("This family reports a comparator gap and adopts nothing, so it declares no worthwhile "
                      "speedup; the material-gap threshold is the margin it decides on."),
        "measurement_resolution": resolution,
        "resolution_evidence": {"receipt": str(receipt_path), "sha256": sha(receipt_bytes)},
        "equivalence_margin": 1.0 + 2.0 * resolution,
        "equivalence_rationale": (
            f"Twice the pilot-observed measurement resolution of {resolution:.3f}: a comparator within that "
            "factor is not a gap this host resolves, so the family reads it as equivalent."),
        "material_gap_threshold": 1.0 + 4.0 * resolution,
        "material_gap_rationale": (
            f"Four times the pilot-observed measurement resolution of {resolution:.3f}, outside the equivalence "
            "margin: a steady-state gap that large changes a receiver's frame budget and needs attribution."),
    })
    for cell in document["cells"]:
        cell["role"] = "confirmatory"
    derivation = {
        "pilot_receipt": str(receipt_path), "receipt_sha256": sha(receipt_bytes),
        "acceptance_summary_sha256": sha(summary_bytes),
        "pilot_addendum": receipt["addendum"], "pilot_ledger": receipt["trial_ledger"],
        "pilot_corrected_alpha": summary["family"]["family_alpha"] / summary["family"]["comparisons"],
        "cell_relative_half_widths": widths,
        "observed_resolution": observed, "frozen_resolution": resolution,
        "rule": "ceil(widest relative half-width x 200) / 200; equivalence 1 + 2r; material gap 1 + 4r",
        "pilot_protocol_version": pilot_version,
        "confirmation_protocol_version": PROTOCOL_VERSION,
    }
    output.with_name(output.stem + "-derivation.json").write_text(json.dumps(derivation, indent=2) + "\n")
    output.write_text(json.dumps(document, indent=2) + "\n")
    print(output)


if __name__ == "__main__":
    main()
