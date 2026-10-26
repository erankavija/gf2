#!/usr/bin/env python3
"""Project the runner plan of one BCH integration campaign (jit:1358d6ab).

Usage: make-plan.py --addendum A --label L --campaign-id ID --campaign-seed S
                    --lock PATH --executable BIN --producing-manifest M
                    --max-cells-per-session N [--pilot-pairs P] --output PLAN

The projection is the shared `campaign_plan.py`. `--executable` is the consumer
harness's `consumer-arm`; the external arm `bch-external-arm` is its sibling in
the same target directory. The cell identifier's last segments name the
external engine, `<code>-b<batch>-<engine>`.
"""

import os

import locate  # noqa: F401
import campaign_plan  # noqa: E402

EXTERNAL = "bch-external-arm"
ENGINE_VAR = "GF2_BCH_EXTERNAL"
CONSUMER_PATH_VAR = "GF2_CONSUMER_PATH"
ENVIRONMENT = {"RAYON_NUM_THREADS": "1"}
ENGINES = {
    "aff3ct-lfsr-scalar": "AFF3CT [Cassagne2019] Encoder_BCH, one frame per LFSR call",
    "aff3ct-lfsr-simd-inter": "AFF3CT [Cassagne2019] Encoder_BCH_inter, one SIMD wave per call",
    "bchlib-table-remainder": "bchlib [Bchlib2026] bch_encode, table-driven remainder",
}
ROUTES = {
    "bch-encode-batch": "BchCode::encode_batch_into through the family the dispatcher selects",
    "dvb-bch-encode": "the DVB-T2 outer BCH code's encode per message of the batch",
}


def arms_for(declared, executable):
    engine = declared["cell_id"].split("-", 2)[2]
    if engine not in ENGINES:
        raise SystemExit(f"cell {declared['cell_id']} names no known engine")
    workload = declared["workload"]["identity"]
    gf2 = (f"gf2-{workload}", campaign_plan.arm(
        campaign_plan.BUILD,
        f"consumer-arm, {campaign_plan.BUILD} release build: {ROUTES[workload]}",
        executable,
        ENVIRONMENT | {CONSUMER_PATH_VAR: "current"},
    ))
    external = (engine, campaign_plan.arm(
        "external",
        f"bch-external-arm: {ENGINES[engine]}; external sources at -O3 -march=native",
        os.path.join(os.path.dirname(executable), EXTERNAL),
        ENVIRONMENT | {ENGINE_VAR: engine},
    ))
    return [gf2, external]


def case_for(declared):
    workload = declared["workload"]
    return {"workload": workload["identity"], "size": workload["size"], "seed": workload["seed"]}


if __name__ == "__main__":
    campaign_plan.project(arms_for, case_for)
