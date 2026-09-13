#!/usr/bin/env python3
"""Cell tables for the count-optimization families (jit:5cbb6545).

One table per stage feeds both the frozen addendum and the runner plan, so a
cell's identifier, arms, workload and declarations have a single source. The
addendum generator adds the family-level effect rule and the plan generator
adds the executable identities; neither invents a cell.

Stages:

- `sweep` locates the word count at which each carry-save kernel overtakes the
  per-vector nibble lookup, and compares both against the pinned external
  references over alignment offsets and bit patterns. Every cell is
  exploratory, so the stage decides nothing and spends no confirmatory
  comparison; its receipt is not resolution evidence.
- `sweep2` narrows the crossover each `sweep` bracketed, over the word counts
  between the last width the established lookup wins and the first width the
  carry-save loop clears the family's worthwhile margin.
- `pilot` measures exactly the cells the confirmation will decide, so the
  frozen measurement resolution comes from the same workloads.
- `smoke` names every arm identity once, over all three case shapes, so the
  wire contract between the runner and the arm binary is established by an
  execution rather than by reading code. Its receipt is a throwaway.
"""

POPCOUNT_FAMILY = "popcount-route-selection"
FUSED_FAMILY = "fused-count-consumers"

#: Words in one carry-save block, the smallest width at which the kernel's
#: main loop runs at all.
CSA_BLOCK_WORDS = 64


def popcount_case(words, seed, pattern="random", word_offset=0):
    return {
        "op": "popcount",
        "words": words,
        "seed": seed,
        "pattern": pattern,
        "word_offset": word_offset,
    }


def and_case(words, seed, pattern="random", word_offset=0, whole_consumer=False):
    return {
        "op": "and_popcnt",
        "words": words,
        "seed_lhs": seed,
        "seed_rhs": seed + 7717,
        "pattern": pattern,
        "word_offset": word_offset,
        "whole_consumer": whole_consumer,
    }


def matvec_case(rows, cols, seed):
    return {"op": "matvec", "rows": rows, "cols": cols, "seed": seed}


def bytes_per_call(case):
    """Useful operand bytes one call of this case feeds to its route.

    Every generator that turns a receipt's times into a rate reads this, so the
    byte count behind a rate has one definition: the operand bytes the route
    reads, both operands of a fused case included.
    """
    if case["op"] == "popcount":
        return case["words"] * 8
    if case["op"] == "and_popcnt":
        return case["words"] * 8 * 2
    if case["op"] == "matvec":
        row_bytes = (case["cols"] + 63) // 64 * 8
        return case["rows"] * row_bytes * 2
    raise SystemExit(f"unknown case op {case['op']!r}")


def cell(
    cell_id,
    baseline,
    candidate,
    case,
    objective,
    role,
    identity,
    metric_kind="kernel-isolated",
    cache_state="warm",
    conversion_costs_included=False,
):
    """One cell in the shared representation both generators read."""
    return {
        "cell_id": cell_id,
        "baseline_arm": baseline,
        "candidate_arm": candidate,
        "case": case,
        "objective": objective,
        "role": role,
        "identity": identity,
        "metric_kind": metric_kind,
        "cache_state": cache_state,
        "conversion_costs_included": conversion_costs_included,
    }


def popcount_sweep():
    """Exploratory crossover search for the population-count family."""
    cells = []
    seed = 5100
    # Below the bit-backend threshold: the scalar POPCNT kernel against the
    # route the library took there.
    for words in (1, 2, 4, 7):
        cells.append(
            cell(
                f"sweep-popcount-w{words}-scalar-popcnt",
                "legacy-dispatch",
                "scalar-popcnt",
                popcount_case(words, seed + words),
                "improvement",
                "exploratory",
                f"popcount-{words}-words-random-aligned",
            )
        )
    # At and above the threshold: the scalar kernel against the vector one, to
    # locate the width at which the vector route repays its setup.
    for words in (8, 12, 16):
        cells.append(
            cell(
                f"sweep-popcount-w{words}-nibble-vs-scalar",
                "scalar-popcnt",
                "nibble-lut",
                popcount_case(words, seed + words),
                "improvement",
                "exploratory",
                f"popcount-{words}-words-random-aligned",
            )
        )
    # The carry-save crossover: one cell per candidate width.
    for words in (16, 32, 64, 96, 128, 256, 1024, 16384):
        cells.append(
            cell(
                f"sweep-popcount-w{words}-csa",
                "nibble-lut",
                "csa",
                popcount_case(words, seed + words),
                "improvement",
                "exploratory",
                f"popcount-{words}-words-random-aligned",
            )
        )
    # Alignment and bit patterns at one mid width.
    cells.append(
        cell(
            "sweep-popcount-w1024-off3-csa",
            "nibble-lut",
            "csa",
            popcount_case(1024, seed + 1024, word_offset=3),
            "improvement",
            "exploratory",
            "popcount-1024-words-random-offset-3-words",
        )
    )
    for pattern, label in (("all_one", "ones"), ("all_zero", "zeros")):
        cells.append(
            cell(
                f"sweep-popcount-w1024-{label}-csa",
                "nibble-lut",
                "csa",
                popcount_case(1024, seed + 1024, pattern=pattern),
                "improvement",
                "exploratory",
                f"popcount-1024-words-{pattern}-aligned",
            )
        )
    # The pinned external references on identical buffers.
    for words in (1024, 16384):
        for arm in ("libpopcnt", "mula-avx2-harley-seal"):
            cells.append(
                cell(
                    f"sweep-popcount-w{words}-vs-{arm}",
                    "csa",
                    arm,
                    popcount_case(words, seed + words),
                    "comparator-gap",
                    "exploratory",
                    f"popcount-{words}-words-random-aligned",
                )
            )
    # A streaming set twice the last-level cache.
    cells.append(
        cell(
            "sweep-popcount-w1m-streaming-csa",
            "nibble-lut",
            "csa",
            popcount_case(1 << 20, seed + 1),
            "improvement",
            "exploratory",
            "popcount-1048576-words-random-aligned-streaming",
            cache_state="streaming",
        )
    )
    return cells


def fused_sweep():
    """Exploratory crossover search for the fused AND-count family."""
    cells = []
    seed = 6100
    for words in (8, 16, 32, 64, 96, 128, 512, 4096):
        cells.append(
            cell(
                f"sweep-and-w{words}-csa",
                "and-legacy-fused",
                "and-csa-fused",
                and_case(words, seed + words),
                "improvement",
                "exploratory",
                f"and-popcnt-{words}-words-random-aligned",
            )
        )
    # The route a gf2-core consumer has without a fused kernel.
    cells.append(
        cell(
            "sweep-and-w4096-vs-two-pass",
            "and-two-pass",
            "and-csa-fused",
            and_case(4096, seed + 4096, whole_consumer=True),
            "improvement",
            "exploratory",
            "and-popcnt-4096-words-random-aligned-whole-consumer",
            metric_kind="whole-consumer",
            conversion_costs_included=True,
        )
    )
    return cells


def smoke():
    """Every arm identity once, over all three case shapes."""
    cells = []
    seed = 4100
    pairs = [
        ("legacy-dispatch", "resolved-dispatch"),
        ("nibble-lut", "scalar-popcnt"),
        ("csa", "compiler-count-ones"),
        ("libpopcnt", "mula-avx2-harley-seal"),
    ]
    for index, (baseline, candidate) in enumerate(pairs):
        cells.append(
            cell(
                f"smoke-popcount-{index}",
                baseline,
                candidate,
                popcount_case(CSA_BLOCK_WORDS, seed + index),
                "improvement",
                "exploratory",
                f"popcount-{CSA_BLOCK_WORDS}-words-random-aligned",
            )
        )
    for index, (baseline, candidate) in enumerate(
        [("and-legacy-fused", "and-csa-fused"), ("and-resolved-fused", "and-scalar-control")]
    ):
        cells.append(
            cell(
                f"smoke-and-{index}",
                baseline,
                candidate,
                and_case(CSA_BLOCK_WORDS, seed + 10 + index),
                "improvement",
                "exploratory",
                f"and-popcnt-{CSA_BLOCK_WORDS}-words-random-aligned",
            )
        )
    cells.append(
        cell(
            "smoke-and-two-pass",
            "and-two-pass",
            "and-csa-fused",
            and_case(CSA_BLOCK_WORDS, seed + 20, whole_consumer=True),
            "improvement",
            "exploratory",
            f"and-popcnt-{CSA_BLOCK_WORDS}-words-random-aligned-whole-consumer",
            metric_kind="whole-consumer",
            conversion_costs_included=True,
        )
    )
    cells.append(
        cell(
            "smoke-matvec",
            "matvec-legacy",
            "matvec-resolved",
            matvec_case(64, 4096, seed + 30),
            "improvement",
            "exploratory",
            "matvec-64-rows-4096-cols-random",
            metric_kind="whole-consumer",
            conversion_costs_included=True,
        )
    )
    return cells


def popcount_sweep2():
    """Narrows the population-count crossover the first sweep bracketed."""
    seed = 5200
    return [
        cell(
            f"sweep2-popcount-w{words}-csa",
            "nibble-lut",
            "csa",
            popcount_case(words, seed + words),
            "improvement",
            "exploratory",
            f"popcount-{words}-words-random-aligned",
        )
        for words in (160, 192, 224, 256, 320, 384)
    ]


def fused_sweep2():
    """Narrows the fused crossover the first sweep bracketed."""
    seed = 6200
    return [
        cell(
            f"sweep2-and-w{words}-csa",
            "and-legacy-fused",
            "and-csa-fused",
            and_case(words, seed + words),
            "improvement",
            "exploratory",
            f"and-popcnt-{words}-words-random-aligned",
        )
        for words in (160, 192, 224, 256, 320, 384)
    ]


#: The adopted carry-save boundary: the first swept width at which the
#: carry-save population count clears this family's worthwhile margin over the
#: per-vector nibble lookup, and at which the fused carry-save kernel is
#: already materially faster.
CSA_MIN_WORDS = 256


def popcount_pilot():
    """The cells the population-count confirmation decides.

    Two widths keep the established route and must not pay for the new
    boundary, two take the carry-save loop, one takes the scalar POPCNT
    kernel, and one measures what is left of the gap to the pinned external
    implementation that led this family before the change.
    """
    seed = 5300
    cells = [
        cell(
            "popcount-w4-dispatch",
            "legacy-dispatch",
            "resolved-dispatch",
            popcount_case(4, seed + 4),
            "improvement",
            "exploratory",
            "popcount-4-words-random-aligned",
        ),
        cell(
            "popcount-w8-dispatch",
            "legacy-dispatch",
            "resolved-dispatch",
            popcount_case(8, seed + 8),
            "non-regression",
            "exploratory",
            "popcount-8-words-random-aligned",
        ),
        cell(
            "popcount-w128-dispatch",
            "legacy-dispatch",
            "resolved-dispatch",
            popcount_case(128, seed + 128),
            "non-regression",
            "exploratory",
            "popcount-128-words-random-aligned",
        ),
        cell(
            "popcount-w1024-dispatch",
            "legacy-dispatch",
            "resolved-dispatch",
            popcount_case(1024, seed + 1024),
            "improvement",
            "exploratory",
            "popcount-1024-words-random-aligned",
        ),
        cell(
            "popcount-w16384-dispatch",
            "legacy-dispatch",
            "resolved-dispatch",
            popcount_case(16384, seed + 16384),
            "improvement",
            "exploratory",
            "popcount-16384-words-random-aligned",
        ),
        cell(
            "popcount-w16384-vs-libpopcnt",
            "resolved-dispatch",
            "libpopcnt",
            popcount_case(16384, seed + 16384),
            "comparator-gap",
            "exploratory",
            "popcount-16384-words-random-aligned",
        ),
    ]
    return cells


def fused_pilot():
    """The cells the fused-consumer confirmation decides.

    One width keeps the established fused lookup, two take the fused
    carry-save loop, one measures the fused route against the two-pass route a
    gf2-core consumer has without it, and two measure whole matrix-vector
    products whose strides fall either side of the boundary.
    """
    seed = 6300
    return [
        cell(
            "and-w128-fused",
            "and-legacy-fused",
            "and-resolved-fused",
            and_case(128, seed + 128),
            "non-regression",
            "exploratory",
            "and-popcnt-128-words-random-aligned",
        ),
        cell(
            "and-w512-fused",
            "and-legacy-fused",
            "and-resolved-fused",
            and_case(512, seed + 512),
            "improvement",
            "exploratory",
            "and-popcnt-512-words-random-aligned",
        ),
        cell(
            "and-w4096-fused",
            "and-legacy-fused",
            "and-resolved-fused",
            and_case(4096, seed + 4096),
            "improvement",
            "exploratory",
            "and-popcnt-4096-words-random-aligned",
        ),
        cell(
            "and-w4096-vs-two-pass",
            "and-two-pass",
            "and-resolved-fused",
            and_case(4096, seed + 4096, whole_consumer=True),
            "improvement",
            "exploratory",
            "and-popcnt-4096-words-random-aligned-whole-consumer",
            metric_kind="whole-consumer",
            conversion_costs_included=True,
        ),
        cell(
            "matvec-1024x16384",
            "matvec-legacy",
            "matvec-resolved",
            matvec_case(1024, 16384, seed + 1),
            "improvement",
            "exploratory",
            "matvec-1024-rows-16384-cols-random",
            metric_kind="whole-consumer",
            conversion_costs_included=True,
        ),
        cell(
            "matvec-1024x4096",
            "matvec-legacy",
            "matvec-resolved",
            matvec_case(1024, 4096, seed + 2),
            "non-regression",
            "exploratory",
            "matvec-1024-rows-4096-cols-random",
            metric_kind="whole-consumer",
            conversion_costs_included=True,
        ),
    ]
