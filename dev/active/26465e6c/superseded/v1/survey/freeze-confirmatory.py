#!/usr/bin/env python3
"""Freeze a confirmatory addendum's effect settings from its pilot receipt.

    survey/freeze-confirmatory.py freeze <popcount|and-popcnt> <pilot-receipt-dir>
    survey/freeze-confirmatory.py selftest

The protocol requires every numeric setting a confirmatory cell depends on to be
committed before the run it governs, and requires the measurement resolution to
be measured by a pilot whose receipt the addendum pins by digest. This script
performs that derivation mechanically, so the numbers in the addendum are
reproducible from the cited receipt rather than chosen by hand:

  measurement_resolution  the largest relative confidence-interval half-width
                          any pilot cell observed, rounded up to the next whole
                          percent. This is the smallest relative difference the
                          design resolved on this host.
  resolution_evidence     the pilot receipt's repository-relative path and the
                          SHA-256 of its exact bytes.
  material_gap_threshold  1.10 -- a comparator must run at least 10% faster
                          before replacing gf2's own kernel, or moving its
                          dispatch threshold, could repay the maintenance and
                          dependency cost. Chosen on consumer benefit, not on
                          the noise floor.
  equivalence_margin      1.05 -- an arm within 5% of gf2's own implementation
                          is not a materially different implementation for any
                          consumer of these leaf kernels.

Both margins are declared once rather than per cell because every cell in both
families asks the same question of the same baseline.

A margin that does not lie strictly outside the measured resolution is widened
to the next whole percent that does, and the addendum records that it was.
Every comparison and every step here is computed on the integer percent: in
binary floating point ``1.05 - 1`` is ``0.05000000000000004``, so a margin
sitting exactly on a 0.05 resolution would otherwise pass a strict comparison by
accident and an equivalence claim would rest on the noise floor. Which side of
the boundary such a margin falls on has to be decided by the rule, not by the
representation, so the boundary is pinned by `selftest`.
"""

import hashlib
import json
import sys
from datetime import datetime, timezone
from decimal import Decimal, ROUND_CEILING

PERCENT = Decimal("0.01")

ADDENDA = {
    "popcount": "dev/active/26465e6c/addendum-popcount.json",
    "and-popcnt": "dev/active/26465e6c/addendum-and-popcnt.json",
}
MATERIAL_GAP = 1.10
EQUIVALENCE = 1.05


def percent(value):
    """The whole percent a factor's excess over 1 occupies, as an integer."""
    return round(value * 100)


def resolution_from(half_widths):
    """The measurement resolution: the widest relative half-width, rounded up
    to the next whole percent.

    The ceiling runs on the shortest decimal that round-trips the observed
    float, not on the float scaled by 100: in binary floating point
    ``0.07 * 100`` is ``7.000000000000001``, so a plain ``math.ceil`` over that
    product reports a resolution of 0.08 for a half-width of exactly seven
    percent. Five whole percents in 1..99 -- 7, 14, 28, 55 and 56 -- are
    affected, and each would overstate the resolution the pilot measured and
    could widen a margin that did not need widening.
    """
    return float(Decimal(repr(max(half_widths))).quantize(PERCENT, rounding=ROUND_CEILING))


def widen(margin, resolution):
    """The operative margin and whether the declared one had to be widened.

    A margin is operative only when its excess over 1 lies strictly outside the
    resolution. Both sides are compared as integer percents so the decision
    never depends on binary floating-point representation.
    """
    if percent(margin - 1) > percent(resolution):
        return margin, False
    return round(1 + (percent(resolution) + 1) / 100, 2), True


def derive(half_widths):
    """The whole numeric derivation, independent of any file."""
    resolution = resolution_from(half_widths)
    material_gap, gap_widened = widen(MATERIAL_GAP, resolution)
    equivalence, equivalence_widened = widen(EQUIVALENCE, resolution)
    return resolution, material_gap, gap_widened, equivalence, equivalence_widened


def widening_note(name, declared, operative, resolution, widened):
    if not widened:
        return f"The declared {declared} lies outside the pilot-measured resolution {resolution}."
    return (f"The declared {declared} does not lie outside the pilot-measured resolution "
            f"{resolution}, so {name} is widened to {operative}, the next whole percent that "
            f"does; the operative margin is {operative}.")


def freeze(family, pilot_dir):
    addendum_path = ADDENDA[family]
    summary = json.load(open(f"{pilot_dir}/acceptance-summary.json"))
    if summary["verdict"] != "accepted":
        raise SystemExit(f"pilot receipt is {summary['verdict']}, not accepted")
    if summary["label"] != "pilot":
        raise SystemExit(f"resolution evidence must be labelled pilot, not {summary['label']}")

    observed = []
    for cell in summary["cells"]:
        interval = cell.get("interval")
        if cell["outcome"] != "pilot" or not interval:
            raise SystemExit(f"pilot cell {cell['cell_id']} carries no usable interval")
        half_width = (interval["upper"] - interval["lower"]) / 2
        relative = half_width / interval["estimate"]
        observed.append((relative, cell["cell_id"]))
        print(f"  {cell['cell_id']}: estimate {interval['estimate']:.4f} "
              f"interval [{interval['lower']:.4f}, {interval['upper']:.4f}] "
              f"relative half-width {relative:.4f} over {interval['pairs']} pairs")

    worst, worst_cell = max(observed)
    resolution, material_gap, gap_widened, equivalence, equivalence_widened = derive(
        [value for value, _ in observed])
    print(f"  widest relative half-width {worst:.4f} in {worst_cell}; "
          f"measurement_resolution rounds up to {resolution}")

    pilot_receipt = f"{pilot_dir}/receipt.json"
    digest = hashlib.sha256(open(pilot_receipt, "rb").read()).hexdigest()
    resolution_sentence = (
        f"The pilot receipt {pilot_receipt} observed a widest relative confidence-interval "
        f"half-width of {worst:.4f} in cell {worst_cell}, which rounds up to a measurement "
        f"resolution of {resolution}.")

    addendum = json.load(open(addendum_path))
    effect = addendum["effect"]
    effect["measurement_resolution"] = resolution
    effect["resolution_evidence"] = {"receipt": pilot_receipt, "sha256": digest}
    effect["material_gap_threshold"] = material_gap
    effect["equivalence_margin"] = equivalence
    effect["material_gap_rationale"] = (
        f"A comparator arm has to run at least a factor {material_gap} faster than gf2's own "
        "implementation before replacing that implementation, or moving its dispatch threshold, "
        "could repay the maintenance and dependency cost; below that the survey's conclusion is "
        "that gf2's existing kernel is retained. The threshold is chosen on consumer benefit, "
        f"not on the noise floor. {resolution_sentence} "
        + widening_note("material_gap_threshold", MATERIAL_GAP, material_gap, resolution, gap_widened))
    effect["equivalence_rationale"] = (
        f"An arm within a factor {equivalence} of gf2's own implementation is not a materially "
        "different implementation for any consumer of these leaf kernels, so a cell whose "
        f"interval lower bound stays above 1/{equivalence} is reported as not materially "
        f"different rather than as a gap. The intended margin is {EQUIVALENCE}, chosen on "
        "consumer benefit. "
        + widening_note("equivalence_margin", EQUIVALENCE, equivalence, resolution, equivalence_widened))
    addendum["frozen"]["frozen_utc"] = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    with open(addendum_path, "w") as out:
        json.dump(addendum, out, indent=2)
        out.write("\n")
    print(f"froze {addendum_path}: resolution {resolution}, material gap {material_gap}, "
          f"equivalence {equivalence}, evidence {digest[:16]}...")


def selftest():
    """Pin the widening rule, especially at the boundary a float comparison
    decides by representation rather than by the rule."""
    cases = [
        # (half-widths, resolution, material gap, gap widened, equivalence, eq widened)
        # A margin sitting exactly on the resolution is inside it and widens.
        # 1.05 - 1 is 0.05000000000000004 in binary floating point, so a naive
        # `margin - 1 > resolution` would wrongly leave this one alone.
        ([0.0401], 0.05, 1.10, False, 1.06, True),
        ([0.05], 0.05, 1.10, False, 1.06, True),
        # Comfortably below both margins: neither widens.
        ([0.0216], 0.03, 1.10, False, 1.05, False),
        ([0.0001], 0.01, 1.10, False, 1.05, False),
        # Exactly on the equivalence margin from below: 0.04 < 0.05, no widening.
        ([0.04], 0.04, 1.10, False, 1.05, False),
        # The gap threshold's own boundary: a 0.10 resolution swallows 1.10.
        ([0.0901], 0.10, 1.11, True, 1.11, True),
        # And just inside it: 0.09 < 0.10 leaves the gap alone.
        ([0.0801], 0.09, 1.10, False, 1.10, True),
        # The widest cell wins, not the last or the first.
        ([0.0102, 0.0401, 0.0088], 0.05, 1.10, False, 1.06, True),
    ]
    for half_widths, resolution, gap, gap_widened, equivalence, eq_widened in cases:
        actual = derive(half_widths)
        expected = (resolution, gap, gap_widened, equivalence, eq_widened)
        assert actual == expected, f"derive({half_widths}) = {actual}, expected {expected}"
    # The rounding is a ceiling, not a round-half: 0.0401 must not become 0.04.
    assert resolution_from([0.0401]) == 0.05
    assert resolution_from([0.04]) == 0.04
    # A half-width that is already a whole percent is its own resolution, at
    # every whole percent. Scaling the float by 100 before the ceiling fails
    # this at 7, 14, 28, 55 and 56.
    for hundredths in range(1, 100):
        exact = hundredths / 100
        assert resolution_from([exact]) == exact, (hundredths, resolution_from([exact]))
    # Widening always lands strictly outside the resolution it was widened for.
    for hundredths in range(1, 100):
        resolution = hundredths / 100
        for margin in (MATERIAL_GAP, EQUIVALENCE):
            operative, _ = widen(margin, resolution)
            assert percent(operative - 1) > percent(resolution), (margin, resolution, operative)
    print(f"selftest passed: {len(cases)} derivation cases, 99 whole-percent resolutions "
          "and 198 widening cases")


def main(argv):
    if len(argv) == 2 and argv[1] == "selftest":
        return selftest()
    if len(argv) == 4 and argv[1] == "freeze" and argv[2] in ADDENDA:
        return freeze(argv[2], argv[3].rstrip("/"))
    raise SystemExit(__doc__.strip().splitlines()[2].strip())


if __name__ == "__main__":
    main(sys.argv)
