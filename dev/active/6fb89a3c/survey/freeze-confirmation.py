#!/usr/bin/env python3
"""Freeze a family's confirmatory addendum from its accepted v3 pilot.

Usage: dev/active/6fb89a3c/survey/freeze-confirmation.py transpose|logical|bch <pilot-dir> <frozen-utc>

The confirmatory addendum is derived, not transcribed: the pilot's
receipt-local addendum snapshot supplies every setting, the pilot's
acceptance summary (checked against the exact committed receipt bytes)
supplies the widest relative bootstrap half-width that becomes
`effect.measurement_resolution` (rounded up to two decimals), the pilot
receipt path and digest become `effect.resolution_evidence`, and the margins
are kept at the pilot's provisional values only when they strictly exceed one
plus the resolution (otherwise they are raised to the next step above it and
the rationales say so). The cells that enter confirmation are the family's
fixed confirmatory set; the pilot's other cells are named in the description
as exploratory-only evidence.
"""
from __future__ import annotations

import hashlib
import json
import math
import pathlib
import sys

ISSUE = pathlib.Path("dev/active/6fb89a3c")
CONFIRMATORY = {
    "transpose": {
        "stem": "addendum-transpose",
        "cells": ["transpose-64-canonical-vs-m4ri", "transpose-64-canonical-vs-bitshuffle",
                  "transpose-consumer-63-vs-m4ri", "transpose-consumer-64-vs-m4ri", "transpose-consumer-65-vs-m4ri",
                  "transpose-consumer-64-vs-bitshuffle-adapter"],
        "why": "The confirmatory set is the six canonical cells: the 64x64 kernel against both comparators, the 63/64/65 consumer against M4RI, and the 64x64 consumer against the Bitshuffle geometry adapter (which pays no copy at that geometry). The padded-adapter consumer cells at 63x63 and 65x65 stay exploratory: they are measured in the pilot with intervals but do not enter confirmation, so the first attempt reserves six comparisons, the largest set for which the protocol's twenty-draw tail-support rule still yields confirmatory cells.",
    },
    "logical": {
        "stem": "addendum-logical-buffer",
        "cells": ["logical-xor-7w-vs-isal-base", "logical-xor-8w-vs-isal-base", "logical-xor-9w-vs-isal-base",
                  "logical-xor-63w-vs-isal-base", "logical-xor-64w-vs-isal-base", "logical-xor-65w-vs-isal-base"],
        "why": "The confirmatory set is the six boundary cells (7, 8, 9 words around gf2's SIMD dispatch threshold; 63, 64, 65 words around the 64-word boundary). The 16- and 32-word cells and the three-source parity cell stay exploratory: measured in the pilot with intervals, not confirmed, so the first attempt reserves six comparisons, the largest set for which the protocol's twenty-draw tail-support rule still yields confirmatory cells.",
    },
    "bch": {
        "stem": "addendum-bch-genmatrix",
        "cells": ["bch-genmatrix-b1-vs-m4ri", "bch-genmatrix-b2-vs-m4ri", "bch-genmatrix-b3-vs-m4ri"],
        "why": "The confirmatory set is the production materialization at B1, B2 and B3. The reference-oracle cells stay exploratory: they exist to name what the protocol-v1 pilot measured, not to decide anything.",
    },
}


def half_width(interval):
    estimate = interval["estimate"]
    return max(abs(estimate - interval["lower"]), abs(interval["upper"] - estimate)) / estimate


def step_above(value: float, step: float) -> float:
    """Smallest multiple of `step` strictly above `value`."""
    return math.floor(value / step + 1e-9) * step + step


def main() -> int:
    if len(sys.argv) != 4:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    family, pilot_dir, frozen_utc = sys.argv[1], pathlib.Path(sys.argv[2]), sys.argv[3]
    spec = CONFIRMATORY[family]
    receipt_bytes = (pilot_dir / "receipt.json").read_bytes()
    receipt = json.loads(receipt_bytes)
    digest = hashlib.sha256(receipt_bytes).hexdigest()
    summary = json.loads((pilot_dir / "acceptance-summary.json").read_text())
    if summary["receipt_sha256"] != digest or summary["verdict"] != "accepted" or receipt["label"] != "pilot":
        raise SystemExit("the pilot must be an accepted receipt whose summary matches its exact bytes")
    snapshot = (pilot_dir / receipt["addendum"]["snapshot"]).read_bytes()
    if hashlib.sha256(snapshot).hexdigest() != receipt["addendum"]["sha256"]:
        raise SystemExit("pilot addendum snapshot digest differs")
    pilot = json.loads(snapshot)
    if pilot["family"]["id"] != receipt["family_id"]:
        raise SystemExit("pilot family identity differs from its receipt")

    widths = {cell["cell_id"]: half_width(cell["interval"]) for cell in summary["cells"] if cell.get("interval")}
    observed = max(widths.values())
    resolution = math.ceil(observed * 100.0 - 1e-9) / 100.0
    equivalence = pilot["effect"]["equivalence_margin"]
    material = pilot["effect"]["material_gap_threshold"]
    raised = []
    if equivalence <= 1.0 + resolution:
        equivalence = round(step_above(1.0 + resolution, 0.05), 2)
        raised.append(f"equivalence margin raised to {equivalence}")
    if material <= 1.0 + resolution:
        material = round(step_above(1.0 + resolution, 0.05), 2)
        raised.append(f"material-gap threshold raised to {material}")

    document = json.loads(json.dumps(pilot))
    document["frozen"] = {"frozen_utc": frozen_utc}
    document["family"]["description"] = (
        pilot["family"]["description"].replace("Exploratory pilot of", "Confirmatory comparison of", 1)
        .replace(" This pilot sizes the family's measurement resolution; no cell here decides adoption.", "")
        + " " + spec["why"]
        + f" Resolution evidence is the accepted v3 pilot receipt {receipt['receipt_path']} (SHA-256 {digest}); its widest relative bootstrap half-width at the ledger-derived pilot alpha is {observed:.4f}, frozen as {resolution:.2f}."
        + " This survey adopts no production implementation, so the receipt is not expected to qualify; the outcomes are comparator-gap decisions.")
    document["effect"]["measurement_resolution"] = resolution
    document["effect"]["resolution_evidence"] = {"receipt": receipt["receipt_path"], "sha256": digest}
    document["effect"]["equivalence_margin"] = equivalence
    document["effect"]["equivalence_rationale"] = (
        f"One-sided non-inferiority margin {equivalence}: the pilot's widest relative half-width over all its cells is {observed:.4f} "
        f"(frozen resolution {resolution:.2f}), so the margin lies strictly outside the measured resolution as protocol v3 requires."
        + (" " + "; ".join(r for r in raised if "equivalence" in r) + " above the pilot's provisional 1.1 because the pilot resolution exceeded it." if any("equivalence" in r for r in raised) else " It is the pilot's provisional value, kept because the resolution permits it."))
    document["effect"]["material_gap_rationale"] = (
        f"Material-gap threshold {material}: " + pilot["effect"]["material_gap_rationale"].split(";")[0].rstrip(".")
        + f"; it exceeds one plus the frozen resolution {resolution:.2f}."
        + (" " + "; ".join(r for r in raised if "material" in r) + " above the pilot's provisional 1.2 because the pilot resolution exceeded it." if any("material" in r for r in raised) else ""))
    kept = []
    for cell in pilot["cells"]:
        if cell["cell_id"] in spec["cells"]:
            cell = json.loads(json.dumps(cell))
            cell["role"] = "confirmatory"
            kept.append(cell)
    if [c["cell_id"] for c in kept] != spec["cells"]:
        raise SystemExit("pilot addendum lacks a confirmatory cell or orders them differently")
    document["cells"] = kept

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
    text += ",\n  \"cells\": [\n" + ",\n".join(cell_text(c) for c in kept) + "\n  ]\n}\n"
    json.loads(text)
    output = ISSUE / f"{spec['stem']}-v3-confirmation.json"
    output.write_text(text)
    derivation = ISSUE / f"pilot-resolution-v3-{family}.txt"
    lines = [f"pilot {pilot_dir}", f"receipt_sha256 {digest}", f"family {receipt['family_id']}",
             f"ledger-derived per-comparison confidence {summary['family']['per_comparison_confidence']}", ""]
    lines += [f"{cell_id:<52}{width:>10.4f}" for cell_id, width in widths.items()]
    lines += ["", f"widest relative half-width {observed:.6f}", f"frozen measurement_resolution {resolution:.2f}",
              f"equivalence_margin {equivalence}", f"material_gap_threshold {material}",
              f"raised: {'; '.join(raised) if raised else 'none'}", f"confirmatory cells {len(kept)}: {', '.join(spec['cells'])}",
              f"output {output}"]
    derivation.write_text("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
