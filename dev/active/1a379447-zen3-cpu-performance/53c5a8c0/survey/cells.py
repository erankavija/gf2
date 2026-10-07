#!/usr/bin/env python3
"""The frozen cell grid of the 53c5a8c0 study.

One module defines every cell of both families so the addendum generator and
the plan generator cannot drift apart: a cell's identity, workload, metric kind,
cache state, objective and arm assignment are written once and read by both.

The grid spans four size regimes per operation, named by the sizes themselves
rather than by a claim about this host's caches:

* small      - a vector or operand short enough that per-call overhead is a
               large share of the work.
* crossover  - the neighbourhood the batched paths' documented advice names.
* resident   - a working set of a few tens of kilobytes.
* streaming  - a working set rotated through the protocol's fixture banks.

`streaming` is the protocol's declared cache state: rotation and the working-set
size are established, eviction from any cache level is not.

Every cell keeps its operation separate. A raw independent carry-less batch, a
reduced GF(2^m) element-wise batch, a GF(2^m) dot product, an unreduced long
polynomial product and a whole-consumer wide-field product are five operations,
never averaged and never substituted for one another.
"""

# Family identifiers; each owns an independent append-only ledger.
CROSSOVER_FAMILY = "gf2m-clmul-crossover"
POLYNOMIAL_FAMILY = "wide-polynomial-competitiveness"

# Arm names, shared by the addenda's build identities and the plans' arm table.
ELEMENT_ARM = "element-path"
BATCH_ARM = "batch-path"
GF2_ARM = "gf2-long-product"
GF2X_ARM = "gf2x-long-product"


def _cell(
    cell_id,
    case,
    metric_kind,
    objective,
    baseline,
    candidate,
    *,
    scaling="single-core-latency",
    cache_state="warm",
    core_arm="single-core",
    role="exploratory",
    regime="small",
    builds=("conservative-portable", "conservative-portable"),
):
    return {
        "cell_id": cell_id,
        "regime": regime,
        "case": case,
        "metric_kind": metric_kind,
        "objective": objective,
        "role": role,
        "scaling": scaling,
        "cache_state": cache_state,
        "core_arm": core_arm,
        "baseline_arm": baseline,
        "candidate_arm": candidate,
        "builds": {"baseline": builds[0], "candidate": builds[1]},
    }


def _raw_batch(count, inner, seed):
    return {"kind": "raw-batch", "count": count, "inner": inner, "seed": seed}


def _field_dot(count, inner, seed):
    return {"kind": "field-dot", "count": count, "inner": inner, "seed": seed}


def _field_batch_mul(count, inner, seed):
    return {"kind": "field-batch-mul", "count": count, "inner": inner, "seed": seed}


def _poly_mul(words, inner, seed):
    return {"kind": "poly-mul", "words": words, "inner": inner, "seed": seed}


def _wide_field_mul(words, inner, seed):
    return {"kind": "wide-field-mul", "words": words, "inner": inner, "seed": seed}


# ---------------------------------------------------------------- crossover
#
# Baseline is the per-element path and candidate the batched path, at every
# cell and both roles, so one frozen direction serves the whole grid: a speedup
# above one means the batched path is faster at that size. The crossover is the
# size at which the frozen decision rule stops recording the batched path as
# not faster and starts recording it as improved; no cell's arms are assigned
# after a result.
CROSSOVER_CELLS = [
    _cell("raw-batch-8", _raw_batch(8, 256, 5301), "kernel-isolated",
          "improvement", ELEMENT_ARM, BATCH_ARM, regime="small"),
    _cell("raw-batch-64", _raw_batch(64, 64, 5302), "kernel-isolated",
          "improvement", ELEMENT_ARM, BATCH_ARM, regime="crossover"),
    _cell("raw-batch-1024", _raw_batch(1024, 4, 5303), "kernel-isolated",
          "improvement", ELEMENT_ARM, BATCH_ARM, regime="resident"),
    _cell("raw-batch-65536-streaming", _raw_batch(65536, 1, 5304),
          "kernel-isolated", "improvement", ELEMENT_ARM, BATCH_ARM,
          scaling="sustained-throughput", cache_state="streaming",
          regime="streaming"),
    _cell("field-dot-8", _field_dot(8, 256, 5311), "whole-consumer",
          "improvement", ELEMENT_ARM, BATCH_ARM, regime="small"),
    _cell("field-dot-64", _field_dot(64, 64, 5312), "whole-consumer",
          "improvement", ELEMENT_ARM, BATCH_ARM, regime="crossover"),
    _cell("field-dot-1024", _field_dot(1024, 4, 5313), "whole-consumer",
          "improvement", ELEMENT_ARM, BATCH_ARM, regime="resident"),
    _cell("field-dot-65536-streaming", _field_dot(65536, 1, 5314),
          "whole-consumer", "improvement", ELEMENT_ARM, BATCH_ARM,
          scaling="sustained-throughput", cache_state="streaming",
          regime="streaming"),
    _cell("field-batch-mul-8", _field_batch_mul(8, 256, 5321), "whole-consumer",
          "improvement", ELEMENT_ARM, BATCH_ARM, regime="small"),
    _cell("field-batch-mul-32", _field_batch_mul(32, 128, 5322),
          "whole-consumer", "improvement", ELEMENT_ARM, BATCH_ARM,
          regime="crossover"),
    _cell("field-batch-mul-1024", _field_batch_mul(1024, 4, 5323),
          "whole-consumer", "improvement", ELEMENT_ARM, BATCH_ARM,
          regime="resident"),
]

# --------------------------------------------------------------- polynomial
#
# gf2 is the baseline and the pinned gf2x build the candidate, so a speedup of
# medians below one means gf2 is faster. Every cell is a comparator-gap cell:
# the study adopts no gf2x dependency and changes no gf2 multiplication.
POLYNOMIAL_CELLS = [
    _cell("wide-field-256-composed", _wide_field_mul(4, 64, 5401),
          "whole-consumer", "comparator-gap", GF2_ARM, GF2X_ARM,
          regime="small", builds=("native", "external")),
    _cell("wide-field-571-composed", _wide_field_mul(9, 32, 5402),
          "whole-consumer", "comparator-gap", GF2_ARM, GF2X_ARM,
          regime="small", builds=("native", "external")),
    _cell("poly-4w", _poly_mul(4, 64, 5411), "kernel-isolated",
          "comparator-gap", GF2_ARM, GF2X_ARM, regime="small",
          builds=("native", "external")),
    _cell("poly-9w", _poly_mul(9, 64, 5412), "kernel-isolated",
          "comparator-gap", GF2_ARM, GF2X_ARM, regime="small",
          builds=("native", "external")),
    _cell("poly-16w", _poly_mul(16, 16, 5413), "kernel-isolated",
          "comparator-gap", GF2_ARM, GF2X_ARM, regime="crossover",
          builds=("native", "external")),
    _cell("poly-64w", _poly_mul(64, 4, 5414), "kernel-isolated",
          "comparator-gap", GF2_ARM, GF2X_ARM, regime="resident",
          builds=("native", "external")),
    _cell("poly-256w", _poly_mul(256, 1, 5415), "kernel-isolated",
          "comparator-gap", GF2_ARM, GF2X_ARM, regime="resident",
          builds=("native", "external")),
    _cell("raw-batch-1024-composed", _raw_batch(1024, 1, 5421),
          "kernel-isolated", "comparator-gap", GF2_ARM, GF2X_ARM,
          regime="resident", builds=("native", "external")),
]

# ------------------------------------------------------------------ holdout
#
# Two sizes that appear in no pilot cell, declared before the pilot runs, each
# carrying a direction the selector recommendation must satisfy. Their samples
# take no part in choosing a threshold; they test one.
#
# The short cell swaps the arms: its hypothesis is that the per-element dot
# product is not worse than the dispatched one at sixteen elements, which is
# what a length gate below the threshold would rely on. The long cell keeps the
# grid's direction: its hypothesis is that the dispatched dot product is
# materially faster at five hundred and twelve elements, which is what
# retaining the established path above the threshold relies on. Both
# directions are fixed here, before any pilot result exists.
HOLDOUT_CELLS = [
    _cell("field-dot-16-holdout", _field_dot(16, 128, 5331), "whole-consumer",
          "non-regression", BATCH_ARM, ELEMENT_ARM, role="holdout",
          regime="small"),
    _cell("field-dot-512-holdout", _field_dot(512, 8, 5332), "whole-consumer",
          "improvement", ELEMENT_ARM, BATCH_ARM, role="holdout",
          regime="resident"),
]

FAMILIES = {
    CROSSOVER_FAMILY: CROSSOVER_CELLS,
    POLYNOMIAL_FAMILY: POLYNOMIAL_CELLS,
}


def cells_of(family):
    """Every pilot cell of `family`, in declaration order."""
    return FAMILIES[family]


def addendum_cell(cell, role=None):
    """The addendum shape of `cell`, with `role` overriding its pilot role."""
    case = cell["case"]
    size = {
        key: value for key, value in case.items() if key not in ("kind", "seed")
    }
    return {
        "cell_id": cell["cell_id"],
        "objective": cell["objective"],
        "role": role or cell["role"],
        "workload": {
            "identity": f"{case['kind']}-{cell['regime']}",
            "size": size,
            "seed": case["seed"],
        },
        "metric_kind": cell["metric_kind"],
        "scaling": cell["scaling"],
        "core_arm": cell["core_arm"],
        "workers": {"declared": 1, "nested_pools_allowed": False},
        "cache_state": cell["cache_state"],
        "builds": cell["builds"],
        "conversion_costs_included": cell["metric_kind"] == "whole-consumer",
        "decoder": None,
    }
