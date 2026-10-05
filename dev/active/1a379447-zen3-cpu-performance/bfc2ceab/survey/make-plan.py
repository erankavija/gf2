#!/usr/bin/env python3
"""Project the runner plan of one entry-check cost campaign (jit:bfc2ceab).

Usage: make-plan.py --addendum A --label L --campaign-id ID --campaign-seed S
                    --lock PATH --executable BIN --producing-manifest M
                    --max-cells-per-session N [--pilot-pairs P] --output PLAN

The projection itself is the shared `campaign_plan.py`, which `locate.py`
beside this file puts on the import path; this file declares what is this
family's own: its two arms, which are two builds of one package, and the case
a cell decodes. `--executable` names the baseline build; the candidate is the
same binary under the sibling target directory with the `-checked` suffix,
built with the candidate's RUSTFLAGS.
"""

import locate  # noqa: F401
import campaign_plan  # noqa: E402

FORM_VAR = "GF2_ENTRY_FORM"
CANDIDATE_RUSTFLAGS = "--cfg gf2_entry_checked"
CANDIDATE_SUFFIX = "-checked"
CONSUMERS = ("sparse-matmat", "permanent-f3", "ple", "gemm")


def arms_for(declared, executable):
    if not (declared["metric_kind"] == "whole-consumer"):
        raise SystemExit(f"cell {declared['cell_id']}: every cell is whole-consumer")
    build = campaign_plan.BUILD
    return [
        ("current-entry", campaign_plan.arm(
            build,
            f"entry-check-arm, {build} release build without a flag: the kernel crate "
            "publishes the entry points it publishes today",
            executable,
            {FORM_VAR: "current"},
        )),
        ("checked-entry", campaign_plan.arm(
            build,
            f"entry-check-arm, {build} release build with RUSTFLAGS `{CANDIDATE_RUSTFLAGS}`: "
            "the kernel crate publishes the checked entry points",
            campaign_plan.sibling_executable(executable, CANDIDATE_SUFFIX),
            {FORM_VAR: "checked"},
            CANDIDATE_RUSTFLAGS,
        )),
    ]


def case_for(declared):
    identity = declared["workload"]["identity"]
    consumer = next((name for name in CONSUMERS if identity.startswith(name + "-")), None)
    if consumer is None:
        raise SystemExit(f"cell {declared['cell_id']}: workload {identity!r} names no consumer")
    return {"consumer": consumer} | declared["workload"]["size"] | {
        "seed": declared["workload"]["seed"]}


if __name__ == "__main__":
    campaign_plan.project(arms_for, case_for)
