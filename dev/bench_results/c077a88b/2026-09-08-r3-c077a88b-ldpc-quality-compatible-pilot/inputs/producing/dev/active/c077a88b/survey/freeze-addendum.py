#!/usr/bin/env python3
"""Freezes a confirmatory family addendum from its pilot receipt (jit:c077a88b).

The protocol requires a confirmatory family to declare its margins against a
resolution that a pilot actually observed, and to pin that pilot by path and
digest. This script performs exactly that derivation so the freeze is
reproducible: it reads the committed pilot receipt, takes the widest relative
half-width of the pilot cells' bootstrap intervals as the family's measurement
resolution, and writes the confirmatory addendum with margins placed outside
it.

The relative half-width of a cell whose speedup interval is $[\\ell, u]$ around
the point estimate $\\hat{s}$ is $(u - \\ell) / (2\\hat{s})$: the fraction of
the estimate the interval spans on either side. A margin inside that fraction
would declare an effect the harness cannot resolve, which the addendum schema
rejects.

Usage:
  freeze-addendum.py --pilot-addendum <pilot.json> --pilot-receipt <receipt.json> \
      --family <id> --output <addendum.json> [--frozen-utc <stamp>]
"""

import argparse
import hashlib
import json
import math
import os


def relative_half_width(claim):
    """Relative half-width of one cell's speedup interval."""
    lower, upper = claim["interval"]["lower"], claim["interval"]["upper"]
    estimate = claim["interval"]["estimate"]
    if estimate <= 0:
        raise SystemExit("a pilot cell reported a non-positive speedup")
    return (upper - lower) / (2.0 * estimate)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--pilot-addendum", required=True)
    parser.add_argument("--pilot-receipt", required=True)
    parser.add_argument("--family", required=True)
    parser.add_argument("--repo-relative-receipt", required=True)
    parser.add_argument("--frozen-utc", default=None)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    pilot = json.load(open(args.pilot_addendum, encoding="utf-8"))
    receipt_bytes = open(args.pilot_receipt, "rb").read()
    receipt = json.loads(receipt_bytes)
    if receipt["label"] != "pilot":
        raise SystemExit("the resolution evidence receipt is not labelled pilot")

    summary_path = os.path.join(os.path.dirname(args.pilot_receipt), "acceptance-summary.json")
    summary = json.load(open(summary_path, encoding="utf-8"))
    if summary["receipt_sha256"] != hashlib.sha256(receipt_bytes).hexdigest() or summary["verdict"] != "accepted":
        raise SystemExit("pilot summary must accept the exact receipt bytes")
    widths = [relative_half_width(cell) for cell in summary["cells"] if cell.get("interval") is not None]
    if not widths:
        raise SystemExit("the pilot receipt carries no cell claim to measure")
    observed = max(widths)
    # Round the observed resolution up to the next half percent so the frozen
    # value is a stated decision rather than a raw float, and never claims more
    # resolution than the pilot showed.
    resolution = math.ceil(observed * 200.0) / 200.0

    document = json.loads(json.dumps(pilot))
    document["family"]["id"] = args.family
    import datetime
    document["frozen"]["frozen_utc"] = args.frozen_utc or datetime.datetime.now(datetime.timezone.utc).isoformat()
    document["effect"]["measurement_resolution"] = resolution
    document["effect"]["resolution_evidence"] = {
        "receipt": args.repo_relative_receipt,
        "sha256": hashlib.sha256(receipt_bytes).hexdigest(),
    }
    for cell in document["cells"]:
        cell["role"] = "confirmatory"
        cell["cell_id"] = cell["cell_id"].replace("-pilot", "-single-core")
    return document, resolution, observed, args


if __name__ == "__main__":
    document, resolution, observed, args = main()
    document["effect"]["rationale"] = (
        "This family reports a comparator gap and adopts nothing, so it declares no "
        "worthwhile speedup; the material-gap threshold below is the margin it decides on."
    )
    document["effect"]["equivalence_margin"] = 1.0 + 2.0 * resolution
    document["effect"]["equivalence_rationale"] = (
        "Twice the pilot-observed measurement resolution of %.3f. A comparator within that "
        "factor of the gf2 decoder is not a gap this host can resolve, so the family declares "
        "it equivalent rather than reading a difference the harness cannot see."
        % resolution
    )
    document["effect"]["material_gap_threshold"] = 1.0 + 4.0 * resolution
    document["effect"]["material_gap_rationale"] = (
        "Four times the pilot-observed measurement resolution of %.3f, and outside the "
        "equivalence margin. A comparator faster than that on a whole-consumer decode is a "
        "gap a consumer would notice on a real frame budget and one that needs an attributed "
        "explanation rather than a note." % resolution
    )
    document["effect"]["worthwhile_speedup"] = None
    with open(args.output, "w", encoding="utf-8") as handle:
        json.dump(document, handle, indent=2)
        handle.write("\n")
    print(
        "%s: observed resolution %.5f, frozen %.3f, equivalence %.3f, material gap %.3f"
        % (
            os.path.basename(args.output),
            observed,
            resolution,
            document["effect"]["equivalence_margin"],
            document["effect"]["material_gap_threshold"],
        )
    )
