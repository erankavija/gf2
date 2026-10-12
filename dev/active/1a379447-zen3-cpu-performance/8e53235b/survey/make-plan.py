#!/usr/bin/env python3
"""Project the runner plan of one codeword-assembly campaign (jit:8e53235b).

Usage: make-plan.py --addendum A --label L --campaign-id ID --campaign-seed S
                    --lock PATH --executable BIN --producing-manifest M
                    --max-cells-per-session N [--pilot-pairs P] --output PLAN

The projection is the shared `campaign_plan.py`, which `locate.py` beside this
file puts on the import path. This file declares the family's own part: the
two arms of a cell, which are the consumer harness's one executable under two
route environments, and the case a cell's arms decode.
"""

import locate  # noqa: F401
import campaign_plan  # noqa: E402

# The consumer arm's route variable and the worker pin every arm carries.
PATH_VAR = "GF2_CONSUMER_PATH"
ENVIRONMENT = {"RAYON_NUM_THREADS": "1"}
WORKLOAD = "bch-encode-batch"

# The cell identifier's route segment, to its baseline and candidate: plan arm
# name, route spelling and what the route is.
ROUTES = {
    "current": (
        ("current", "current", "the family the production dispatcher selects"),
        ("current-word-wise", "word-wise", "the family the production dispatcher selects"),
    ),
    "fold": (
        ("family-fold", "family-clmul-fold", "the carry-less-multiply fold family, forced"),
        ("family-fold-word-wise", "family-clmul-fold-word-wise",
         "the carry-less-multiply fold family, forced"),
    ),
    "bitslice": (
        ("family-bitslice", "family-bitslice-interleaved",
         "the bit-sliced interleaved family, forced"),
        ("family-bitslice-word-wise", "family-bitslice-interleaved-word-wise",
         "the bit-sliced interleaved family, forced"),
    ),
}
ASSEMBLIES = ("the bit-serial codeword assembly every encode selects",
              "the word-wise codeword assembly its workspace selects")


def arms_for(declared, executable):
    cell_id = declared["cell_id"]
    segment = cell_id.split("-")[3]
    if segment not in ROUTES:
        raise SystemExit(f"cell {cell_id} names no known route")
    build = campaign_plan.BUILD
    return [
        (name, campaign_plan.arm(
            build,
            f"consumer-arm, {build} release build: `encode_batch_into` through {family}, "
            f"with {assembly}",
            executable,
            ENVIRONMENT | {PATH_VAR: route},
        ))
        for (name, route, family), assembly in zip(ROUTES[segment], ASSEMBLIES)
    ]


def case_for(declared):
    workload = declared["workload"]
    if workload["identity"] != WORKLOAD:
        raise SystemExit(f"cell {declared['cell_id']}: workload is not {WORKLOAD}")
    return {"workload": WORKLOAD, "size": workload["size"], "seed": workload["seed"]}


if __name__ == "__main__":
    campaign_plan.project(arms_for, case_for)
