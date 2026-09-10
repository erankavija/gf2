#!/usr/bin/env python3
"""Freeze a family's confirmatory addendum from its accepted v3 pilot.

Usage: dev/active/6fb89a3c/survey/freeze-confirmation.py transpose|logical|bch <pilot-dir> <frozen-utc>

The confirmatory addendum is derived, not transcribed: the pilot's
receipt-local addendum snapshot supplies every cell and setting, the pilot's
acceptance summary (checked against the exact committed receipt bytes)
supplies the widest relative bootstrap half-width over all its measured cells,
which becomes `effect.measurement_resolution` rounded up to two decimals, the
pilot receipt path and digest become `effect.resolution_evidence`, and the
margins keep the pilot's provisional values only when they strictly exceed one
plus the resolution (otherwise they are raised to the next 0.05 step above it
and the rationale says so).

Each family confirms its whole question: every cell the pilot measured for it
enters confirmation, so the transpose (8 cells) and logical-buffer (9 cells)
families exceed the m <= 6 bound under which a first attempt can meet P-20's
twenty-draw tail support and are reported not-confirmatory rather than trimmed
to fit. The BCH family confirms its three production-route cells; the
pilot's reference-oracle cells were declared outside confirmation in the pilot
addendum because they do not measure the production route.
"""
from __future__ import annotations

import hashlib
import json
import math
import pathlib
import sys

ISSUE = pathlib.Path("dev/active/6fb89a3c")
BOOTSTRAP_RESAMPLES = 10000
FAMILY_ALPHA = 0.05
MIN_TAIL_DRAWS = 20

SPLITMIX = ("Workload seeds expand through SplitMix64 [Steele2014], implemented as tuning_campaign_support::abtest::SplitMix64 "
            "(dev/tools/tuning-campaign-support/src/abtest.rs) in the gf2 arms and as splitmix64_next "
            "(dev/active/6fb89a3c/survey/harness_common.h) in the C arms; both are pinned by the producing snapshot.")
NO_ADOPTION = ("This survey adopts no production implementation; the outcomes are comparator-gap decisions and the receipt is "
               "not expected to qualify.")

FAMILIES = {
    "transpose": {
        "stem": "addendum-transpose",
        "cells": None,
        "description": (
            "Confirmatory comparison of the transpose/conversion family: gf2's canonical 64x64 word transpose against M4RI "
            "mzd_transpose and Bitshuffle bshuf_bitshuffle (kernel-isolated, preallocated output), and the whole-consumer "
            "BitMatrix::transpose at 63x63, 64x64 and 65x65 against M4RI's fresh-output mzd_transpose and against Bitshuffle "
            "through a measured geometry adapter (row padding to a multiple of eight, byte packing, plane unpacking; the copies "
            "are skipped where the canonical and Bitshuffle layouts coincide, so the 64x64 consumer pays no copy). The baseline "
            "arm is always the established gf2 route and the candidate is always the external arm, so the speedup of medians is "
            "median(gf2)/median(external) and a value above 1 means the external arm is ahead; `improved` in a comparator-gap "
            "cell is a material gap in the external arm's favour that needs attribution and `regressed` means gf2 is materially "
            "ahead. Unpadded Bitshuffle at 63 and 65 rows is unavailable (element count must be a multiple of eight) and is not "
            "substituted. All eight pilot cells enter confirmation: the question covers the canonical kernel, the 63/64/65 "
            "consumer boundary against both comparators and the Bitshuffle geometry adaptation, which only the 63x63 and 65x65 "
            "adapter cells pay. {tail} " + SPLITMIX + " Fixed cells draw one word per row; consumer cells draw one bit per "
            "matrix element in row-major order. The failed v3-r1 pilot measured five of these cells before a gf2 arm defect "
            "stopped it and the accepted v3-r2 pilot re-measured all eight, so the family used up to two pilot trials per cell "
            "where its pilot addendum declared one; this addendum's search budget records two. Between the v3-r2 pilot and "
            "this confirmation only the Bitshuffle arm was rebuilt: its adapter pack/unpack probes, outside the timed windows, "
            "now average 100000 warm repetitions where the pilot reported one cold pass, and the timed calls are unchanged; the "
            "gf2 and M4RI transpose executables are byte-identical to the pilot's. {resolution} " + NO_ADOPTION),
        "material_gap_rationale": (
            "a 20% gap in a nanosecond-scale transpose is the smallest difference worth attributing to a named cause (tile "
            "kernel, allocation, partial-word handling, geometry conversion) rather than to host noise"),
        "max_pilot_trials_per_cell": 2,
    },
    "logical": {
        "stem": "addendum-logical-buffer",
        "cells": None,
        "description": (
            "Confirmatory comparison of the logical-buffer family: gf2's dispatched xor_inplace route against ISA-L's "
            "operation-equivalent xor_gen_base (the portable C reference of xor_gen, compiled -O3 -march=native; the NASM-built "
            "multi-binary xor_gen dispatcher is unavailable on this host and is not substituted). Every call produces a fresh "
            "32-byte-aligned destination from `sources` aligned inputs, the arrangement raid.h documents for xor_gen (vects = "
            "sources + 1, destination last, 32-byte alignment): gf2 pays a destination copy plus one in-place XOR per remaining "
            "source inside the timed call and ISA-L pays its pointer-array formation inside the timed call; each arm also "
            "reports its arrangement alone, averaged over one million repetitions outside the timed windows (gf2: the "
            "destination copy as pack_ns; ISA-L: the pointer-array formation as dispatch_ns). Word counts 7, 8 and 9 straddle "
            "gf2's scalar/SIMD dispatch threshold of eight words; 63, 64 and 65 straddle the 64-word boundary; 16 and 32 fill "
            "the range; the parity cell uses three sources (vects 4). Source/destination aliasing (gf2's accumulate form) and "
            "alignments other than 32 bytes are outside ISA-L's documented contract and are recorded unavailable. The baseline "
            "arm is always gf2 and the candidate is always ISA-L, so a speedup above 1 means ISA-L is ahead. All nine pilot "
            "cells enter confirmation: the question covers the dispatch threshold, the 64-word boundary, the fill sizes and the "
            "parity arity. {tail} " + SPLITMIX + " Buffers draw one word per SplitMix64 output; source s uses seed + s. "
            "Between the v3-r1 pilot and this confirmation both arms were rebuilt: the gf2 arm's source was reformatted by "
            "rustfmt (its .text and .rodata sections are byte-identical; six panic-location line numbers and the build ID "
            "differ), and the ISA-L arm's arrangement probe now forms the pointer array in every repetition behind a compiler "
            "barrier, where the pilot's probe loop had been deleted by GCC and reported 0 ns; the timed calls are unchanged. "
            "{resolution} " + NO_ADOPTION),
        "material_gap_rationale": (
            "a 20% whole-operation gap on a buffer XOR of at most 65 words is the smallest difference worth attributing to a "
            "named cause (vector width, dispatch, destination copy, arrangement) rather than to host noise"),
        "max_pilot_trials_per_cell": 1,
    },
    "bch": {
        "stem": "addendum-bch-genmatrix",
        "cells": ["bch-genmatrix-b1-vs-m4ri", "bch-genmatrix-b2-vs-m4ri", "bch-genmatrix-b3-vs-m4ri"],
        "description": (
            "Confirmatory comparison of the BCH contextual conversion family: gf2's production generator-matrix "
            "materialization BchCode::generator_matrix (the `materialize/fresh-alloc` row of "
            "crates/gf2-coding/benches/bch_genmatrix.rs, B1 (15,5), B2 (127,64), B3 (255,223)) against the established M4RI "
            "construction of issue 4e732b56 (fill the k shifted copies of the generator polynomial, then mzd_echelonize_m4ri). "
            "Each timed call constructs a fresh k x n generator matrix from an already-constructed code (gf2 allocates a "
            "BitMatrix and fills it from the generator polynomial; M4RI copies the prefilled matrix through its cached "
            "allocator and reduces the copy); the routes yield the same code in different systematic layouts, which the "
            "correctness gate checks by row-space equality. Code construction is setup outside the windows and is reported per "
            "execution. The baseline arm is always gf2 and the candidate is always M4RI, so a speedup above 1 means M4RI is "
            "ahead. These are current pinned measurements of both implementations, not a historical receipt and not an encoder "
            "throughput campaign. The pilot's three reference-oracle cells (bch_generator_matrix_by_encoding, the test-support "
            "oracle the protocol-v1 pilot measured as the gf2 route) do not enter confirmation, as the pilot addendum declared, "
            "because they do not measure the production route this question is about. {tail} The workload seed is carried for "
            "schema uniformity and no generator consumes it: the generator matrix is a deterministic function of the code. Both "
            "executables are byte-identical to the pilot's. {resolution} " + NO_ADOPTION),
        "material_gap_rationale": (
            "the arms use different algorithms (gf2 fills the systematic generator column by column from a recurrence on the "
            "generator polynomial; M4RI reduces the matrix of shifted polynomial copies), so a 20% whole-construction gap is "
            "the smallest difference worth attributing to a named cause (operation count, allocation, word-level elimination) "
            "rather than to host noise"),
        "max_pilot_trials_per_cell": 1,
    },
}


def half_width(interval):
    estimate = interval["estimate"]
    return max(abs(estimate - interval["lower"]), abs(interval["upper"] - estimate)) / estimate


def step_above(value: float, step: float) -> float:
    """Smallest multiple of `step` strictly above `value`."""
    return math.floor(value / step + 1e-9) * step + step


def tail_sentence(m: int) -> tuple[str, float]:
    alpha_c = FAMILY_ALPHA / 2 / m
    draws = BOOTSTRAP_RESAMPLES * alpha_c / 2
    verdict = ("so P-20 can yield confirmatory outcomes" if draws >= MIN_TAIL_DRAWS else
               "below the twenty P-20 requires, so every cell is reported not-confirmatory; the question is run whole "
               "rather than trimmed to fit that bound, and its intervals are fresh-sample evidence without a confirmatory "
               "decision")
    return (f"With m = {m} reservations on the family's first confirmatory attempt the corrected per-comparison alpha is "
            f"0.025/{m} and each bootstrap tail expects {draws:.1f} of {BOOTSTRAP_RESAMPLES} draws, {verdict}."), draws


def main() -> int:
    if len(sys.argv) != 4:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    family, pilot_dir, frozen_utc = sys.argv[1], pathlib.Path(sys.argv[2]), sys.argv[3]
    spec = FAMILIES[family]
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
    widest_cell = max(widths, key=widths.get)
    observed = widths[widest_cell]
    resolution = math.ceil(observed * 100.0 - 1e-9) / 100.0
    equivalence = pilot["effect"]["equivalence_margin"]
    material = pilot["effect"]["material_gap_threshold"]
    raised = []
    if equivalence <= 1.0 + resolution:
        equivalence = round(step_above(1.0 + resolution, 0.05), 2)
        raised.append(f"equivalence margin raised to {equivalence} above the pilot's provisional value")
    if material <= 1.0 + resolution:
        material = round(step_above(1.0 + resolution, 0.05), 2)
        raised.append(f"material-gap threshold raised to {material} above the pilot's provisional value")

    names = [cell["cell_id"] for cell in pilot["cells"]]
    wanted = names if spec["cells"] is None else spec["cells"]
    if any(name not in names for name in wanted):
        raise SystemExit("a confirmatory cell is absent from the pilot addendum")
    kept = []
    for cell in pilot["cells"]:
        if cell["cell_id"] in wanted:
            cell = json.loads(json.dumps(cell))
            cell["role"] = "confirmatory"
            kept.append(cell)
    tail, draws = tail_sentence(len(kept))
    pilot_alpha = 1.0 - summary["family"]["per_comparison_confidence"]
    resolution_text = (f"Resolution evidence is the accepted pilot receipt {receipt['receipt_path']} (SHA-256 {digest}); its "
                       f"widest relative bootstrap half-width over all {len(widths)} pilot cells at the ledger-derived pilot "
                       f"alpha {pilot_alpha:.3f} is {observed:.4f} ({widest_cell}), frozen as {resolution:.2f}.")

    document = json.loads(json.dumps(pilot))
    document["frozen"] = {"frozen_utc": frozen_utc}
    document["family"]["description"] = spec["description"].format(tail=tail, resolution=resolution_text)
    document["effect"]["measurement_resolution"] = resolution
    document["effect"]["resolution_evidence"] = {"receipt": receipt["receipt_path"], "sha256": digest}
    document["effect"]["equivalence_margin"] = equivalence
    document["effect"]["equivalence_rationale"] = (
        f"One-sided non-inferiority margin {equivalence}: the pilot's widest relative half-width over all its cells is "
        f"{observed:.4f} (frozen resolution {resolution:.2f}), so the margin lies strictly outside the measured resolution as "
        "protocol v3 requires." + (" " + "; ".join(r for r in raised if "equivalence" in r) + "." if any("equivalence" in r for r in raised)
                                   else " It is the pilot's provisional value, kept because the resolution permits it."))
    document["effect"]["material_gap_threshold"] = material
    document["effect"]["material_gap_rationale"] = (
        f"Material-gap threshold {material}: " + spec["material_gap_rationale"]
        + f"; it exceeds one plus the frozen resolution {resolution:.2f}."
        + (" " + "; ".join(r for r in raised if "material" in r) + "." if any("material" in r for r in raised) else ""))
    document["search_budget"]["max_pilot_trials_per_cell"] = spec["max_pilot_trials_per_cell"]
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
    if json.loads(text) != document:
        raise SystemExit("rendered addendum differs from the derived document")
    output = ISSUE / f"{spec['stem']}-v3-confirmation.json"
    output.write_text(text)
    derivation = ISSUE / f"pilot-resolution-v3-{family}.txt"
    lines = [f"pilot {pilot_dir}", f"receipt_sha256 {digest}", f"family {receipt['family_id']}",
             f"ledger-derived per-comparison confidence {summary['family']['per_comparison_confidence']}", ""]
    lines += [f"{cell_id:<52}{width:>10.4f}" for cell_id, width in widths.items()]
    lines += ["", f"widest relative half-width {observed:.6f} ({widest_cell})", f"frozen measurement_resolution {resolution:.2f}",
              f"equivalence_margin {equivalence}", f"material_gap_threshold {material}",
              f"raised: {'; '.join(raised) if raised else 'none'}",
              f"confirmatory cells m={len(kept)}: {', '.join(c['cell_id'] for c in kept)}",
              f"expected draws per bootstrap tail at attempt 1: {draws:.2f} (P-20 requires {MIN_TAIL_DRAWS})",
              f"max_pilot_trials_per_cell {spec['max_pilot_trials_per_cell']}",
              f"output {output}"]
    derivation.write_text("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
