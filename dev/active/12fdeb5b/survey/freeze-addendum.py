#!/usr/bin/env python3
"""Freeze a family addendum for the NR encoder survey (jit:12fdeb5b).

Usage:
  freeze-addendum.py smoke|pilot <output.json> [frozen-utc]
  freeze-addendum.py confirmation <output.json> <pilot-receipt-dir> [frozen-utc]

Cell workload sizes are read from `nr-encode-parameters.json`, the dump the
native flavour produced, so no dimension is transcribed by hand. The stage's
cell list, seeds, margins and rationales are the declarations below; everything
else comes from the protocol's frozen shared settings.

The confirmation stage owns no freezing logic of its own: it delegates to the
canonical `dev/active/c7113c5a/survey/freeze-confirmation.py`, which derives the
measurement resolution from the committed pilot receipt, pins that receipt by
path and digest, and writes the derivation record beside the addendum. This
module supplies the cell selection P-20's tail-support bound admits, its
rationale, the prose the confirmatory stage carries, and the resolution this
family's rule derives at the confirmation's own corrected alpha. Run it from
the worktree root so every recorded path stays repository-relative.

`frozen-utc` defaults to the current whole-second UTC time. It must not follow
the campaign's opening record, so freeze before launching.
"""

import datetime
import json
import pathlib
import subprocess
import sys

FAMILY_ID = "nr-rate-matched-encode-baselines-v1"
LEDGER = "dev/active/12fdeb5b/nr-rate-matched-encode-trial-ledger.jsonl"
PILOT_ADDENDUM = "dev/active/12fdeb5b/addendum-nr-encode-pilot.json"
CANONICAL_FREEZER = "dev/active/c7113c5a/survey/freeze-confirmation.py"
RESOLUTION_DERIVATION = "dev/active/12fdeb5b/pilot-resolution-nr-encode.txt"
FAMILY_ALPHA = 0.05

# (cell suffix, configuration, seed)
SMOKE_CELLS = [
    ("null-native-vs-native", "bg2-n1024-k400", 401),
    ("gap-native-vs-srsran", "bg2-n256-k121", 402),
    ("gap-native-vs-srsran", "bg1-n8448-k4224", 403),
    ("gap-native-vs-aff3ct", "bg1-n2560-k2048", 404),
    ("control-portable-vs-native", "bg1-n2560-k2048", 404),
]

PILOT_CELLS = [
    ("null-native-vs-native", "bg2-n1024-k400", 401),
    ("gap-native-vs-srsran", "bg2-n256-k121", 402),
    ("gap-native-vs-srsran", "bg2-n1440-k720", 405),
    ("gap-native-vs-srsran", "bg1-n2560-k2048", 404),
    ("gap-native-vs-srsran", "bg1-n8448-k4224", 403),
    ("gap-native-vs-aff3ct", "bg2-n256-k121", 402),
    ("gap-native-vs-aff3ct", "bg1-n2560-k2048", 404),
    ("control-portable-vs-native", "bg1-n2560-k2048", 404),
]

BUILDS = {
    "null-native-vs-native": ("native", "native"),
    "gap-native-vs-srsran": ("native", "external"),
    "gap-native-vs-aff3ct": ("native", "external"),
    "control-portable-vs-native": ("conservative-portable", "native"),
}

COMMON_DESCRIPTION = (
    "5G NR rate-matched LDPC encoding as the whole consumer: target_k information "
    "bits in, target_n rate-matched codeword bits out, in gf2's BitVec "
    "representation. The baseline arm is gf2's BlockEncoder::encode on "
    "Nr5gRateMatchedCode, which fuses TS 38.212 Section 5.4.2.1 bit selection into "
    "the encode. The candidate arms are srsRAN Project 25.10's ldpc_encoder::encode "
    "plus ldpc_rate_matcher::rate_match and AFF3CT v4.7.0's Encoder_LDPC_QC_fast "
    "encode plus Puncturer_5G puncture, each behind a whole-consumer adapter that "
    "converts into the project's representation and back inside the timed call. "
    "Modulation is one bit per symbol, where TS 38.212 Section 5.4.2.2 interleaving "
    "is the identity, and the redundancy version is 0, the only offset gf2 and "
    "AFF3CT implement. Every timed configuration passed the committed bit-exact "
    "equivalence gate in dev/active/12fdeb5b/survey/nr-encode-validation.json. The "
    "baseline is always gf2 and the candidate is the arm whose lead needs "
    "attribution, so a speedup above 1 in a -gap- cell means the comparator is "
    "ahead and above 1 in the -control- cell means the native gf2 build is ahead of "
    "the conservative-portable control. The -null- identity cell measures the noise "
    "floor. Comparator revisions, licences, compiled units, flags and selected "
    "backends are recorded in dev/active/12fdeb5b/survey/build-evidence.json."
)

# The confirmatory stage keeps the pilot's comparator-gap cells and drops the
# two identity controls, the largest set P-20's tail-support bound admits.
CONFIRMATION_CELLS = [
    "nr-enc-bg2-n256-k121-gap-native-vs-srsran",
    "nr-enc-bg2-n1440-k720-gap-native-vs-srsran",
    "nr-enc-bg1-n2560-k2048-gap-native-vs-srsran",
    "nr-enc-bg1-n8448-k4224-gap-native-vs-srsran",
    "nr-enc-bg2-n256-k121-gap-native-vs-aff3ct",
    "nr-enc-bg1-n2560-k2048-gap-native-vs-aff3ct",
]

CONFIRMATION_SELECTION_RATIONALE = (
    "P-20 rejects a confirmatory cell whose corrected tail holds fewer than twenty "
    "expected bootstrap draws. This family's first attempt derives alpha "
    "0.05/(1*2) from the ledger, the corrected alpha divides it by the number of "
    "comparisons, and each tail holds bootstrap_resamples * corrected_alpha / 2 "
    "draws; six comparisons clear twenty draws and seven do not, so at most six of "
    "the eight pilot cells become confirmatory. The dropped pair are the identity "
    "controls: the null cell compares the native gf2 executable with itself and the "
    "portable cell compares two builds of the same gf2 source, so each answers a "
    "validity question about the harness rather than attributing a comparator gap. "
    "The pilot settled both, and this record's table carries their intervals. Every "
    "gf2-versus-comparator pair the pilot measured is retained."
)

CONFIRMATION_EQUIVALENCE_RATIONALE = (
    "A 10% equivalence margin, unchanged from the pilot that sized the resolution "
    "and strictly above one plus it. Two arms of this whole-consumer operation "
    "within 10% of each other are not separated by any cause this survey can name."
)

CONFIRMATION_MATERIAL_GAP_RATIONALE = (
    "A 20% whole-consumer wall-clock gap is the smallest gap between two arms of "
    "this operation worth attributing to a named cause (representation conversion, "
    "parity solve, selection gather or per-call dispatch) rather than host noise. "
    "It is unchanged from the pilot and strictly exceeds one plus the frozen "
    "measurement resolution."
)

CONFIRMATION_DESCRIPTION = (
    "Confirmatory stage for the NR rate-matched encoder family. Its cells attribute "
    "the whole-consumer wall-clock gap between gf2 and each comparator at the frozen "
    "measurement resolution; the family proposes no production change, so no cell "
    "decides adoption. " + COMMON_DESCRIPTION
)

STAGES = {
    "smoke": {
        "cells": SMOKE_CELLS,
        "description": (
            "End-to-end smoke for the NR rate-matched encoder family. It reaches a "
            "result line from every arm the pilot names and sizes the pilot's wall "
            "clock on the largest and smallest configurations of the grid. It is "
            "not a performance result about gf2 and decides nothing. "
            + COMMON_DESCRIPTION
        ),
    },
    "pilot": {
        "cells": PILOT_CELLS,
        "description": (
            "Exploratory pilot for the NR rate-matched encoder family. It sizes the "
            "family's measurement resolution across both base graphs, both "
            "comparators, the build control and the identity floor; no cell decides "
            "adoption. " + COMMON_DESCRIPTION
        ),
    },
}


def now_utc():
    return (datetime.datetime.now(datetime.timezone.utc)
            .replace(microsecond=0)
            .strftime("%Y-%m-%dT%H:%M:%SZ"))


def supplied_resolution():
    """Read the resolution `bootstrap-resolution` derived, and check its alpha.

    The family sizes its resolution at the confirmation's own corrected alpha,
    not at the pilot's, because the DVB-T2 family's use of the pilot's alpha was
    contradicted by a threefold wider confirmation interval. The derivation is
    the committed output of `eda07788`'s `bootstrap-resolution`, which also
    reports P-20's endpoint shift; the value frozen is the larger of the two
    rounded up. Reading the number from that output keeps it untranscribed, and
    checking the alpha it was computed at against this stage's comparison count
    rejects a derivation left over from a different cell selection.
    """
    text = pathlib.Path(RESOLUTION_DERIVATION).read_text()
    # The ledger spends alpha / (attempts * (attempts + 1)) on attempt number
    # `attempts`; this is the family's first, and the comparison count divides
    # what that attempt spends.
    attempts = 1
    expected = FAMILY_ALPHA / (attempts * (attempts + 1)) / len(CONFIRMATION_CELLS)
    alphas = [float(line.split()[1]) for line in text.splitlines() if line.startswith("alpha ")]
    if alphas != [expected]:
        sys.exit(f"{RESOLUTION_DERIVATION} reports alpha {alphas}, not [{expected!r}]")
    marker = "larger of the two rounded up to two decimals:"
    values = [line.split(marker)[1] for line in text.splitlines() if marker in line]
    if len(values) != 1:
        sys.exit(f"{RESOLUTION_DERIVATION} does not state one rounded resolution")
    return values[0].strip()


def freeze_confirmation(output, pilot_receipt, frozen):
    """Delegate to the canonical freezer with this family's selection."""
    record = output.with_name(f"resolution-{output.stem.removeprefix('addendum-')}.txt")
    command = [
        sys.executable, CANONICAL_FREEZER,
        "--pilot-addendum", PILOT_ADDENDUM,
        "--pilot", pilot_receipt,
        "--frozen-utc", frozen,
        "--output", str(output),
        "--record", str(record),
        "--selection-rationale", CONFIRMATION_SELECTION_RATIONALE,
        "--equivalence-rationale", CONFIRMATION_EQUIVALENCE_RATIONALE,
        "--material-gap-rationale", CONFIRMATION_MATERIAL_GAP_RATIONALE,
        "--family-description", CONFIRMATION_DESCRIPTION,
        "--resolution", supplied_resolution(),
        "--resolution-derivation", RESOLUTION_DERIVATION,
    ]
    for cell_id in CONFIRMATION_CELLS:
        command += ["--cell", cell_id]
    subprocess.run(command, check=True)
    print(f"confirmation: {len(CONFIRMATION_CELLS)} cells frozen at {frozen} -> {output}")


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    stage = sys.argv[1]
    if stage == "confirmation":
        if len(sys.argv) not in (4, 5):
            sys.exit(__doc__)
        freeze_confirmation(pathlib.Path(sys.argv[2]), sys.argv[3],
                            sys.argv[4] if len(sys.argv) == 5 else now_utc())
        return
    if len(sys.argv) not in (3, 4):
        sys.exit(__doc__)
    if stage not in STAGES:
        sys.exit(f"unknown stage {stage!r}")
    output = pathlib.Path(sys.argv[2])
    frozen = sys.argv[3] if len(sys.argv) == 4 else now_utc()

    survey = pathlib.Path(__file__).resolve().parent
    parameters = {
        row["configuration"]: row["gf2"]
        for row in json.loads((survey / "nr-encode-parameters.json").read_text())["rows"]
    }

    cells = []
    for suffix, configuration, seed in STAGES[stage]["cells"]:
        gf2 = parameters[configuration]
        baseline, candidate = BUILDS[suffix]
        cells.append({
            "cell_id": f"nr-enc-{configuration}-{suffix}",
            "objective": "comparator-gap",
            "role": "exploratory",
            "workload": {
                "identity": f"nr-encode-{configuration}",
                "size": {
                    "base_graph": gf2["base_graph"],
                    "target_n": gf2["target_n"],
                    "target_k": gf2["target_k"],
                    "lifting": gf2["lifting_factor"],
                    "full_n": gf2["full_n"],
                    "fillers": gf2["num_shortened"],
                },
                "seed": seed,
            },
            "metric_kind": "whole-consumer",
            "scaling": "single-core-latency",
            "core_arm": "single-core",
            "workers": {"declared": 1, "nested_pools_allowed": False},
            "cache_state": "warm",
            "builds": {"baseline": baseline, "candidate": candidate},
            "conversion_costs_included": True,
            "decoder": None,
        })

    seen = set()
    for cell in cells:
        if cell["cell_id"] in seen:
            sys.exit(f"duplicate cell {cell['cell_id']}")
        seen.add(cell["cell_id"])

    addendum = {
        "schema": "zen3-benchmark-addendum-v4",
        "protocol": {"id": "zen3-benchmark-protocol", "version": 4},
        "family": {
            "id": FAMILY_ID,
            "issue": "12fdeb5b",
            "purpose": "consumer-family",
            "description": STAGES[stage]["description"],
        },
        "frozen": {"frozen_utc": frozen},
        "effect": {
            "worthwhile_speedup": None,
            "rationale": (
                "Comparator-gap cells only. This issue is a baseline survey: it "
                "proposes no production change, so no worthwhile-speedup threshold "
                "applies and none is declared."
            ),
            "measurement_resolution": None,
            "resolution_evidence": None,
            "equivalence_margin": 1.1,
            "equivalence_rationale": (
                "A 10% equivalence margin, carried unchanged into the confirmatory "
                "addendum so this stage's recorded decisions are computed under the "
                "rule the confirmation uses. Exploratory cells never yield a pass."
            ),
            "material_gap_threshold": 1.2,
            "material_gap_rationale": (
                "A 20% whole-consumer wall-clock gap is the smallest gap between two "
                "arms of this operation worth attributing to a named cause "
                "(representation conversion, parity solve, selection gather or "
                "per-call dispatch) rather than host noise. This stage checks that "
                "the threshold sits above the measured resolution before the "
                "confirmatory addendum freezes it."
            ),
        },
        "complexity_budget": {
            "max_new_unsafe_kernels": 0,
            "max_added_source_lines": 0,
            "maintenance_rationale": (
                "A survey adopts nothing. No production kernel changes, so the budget "
                "is zero by construction; the whole diff is under dev/."
            ),
        },
        "family_wise": {
            "alpha": 0.05,
            "prior_confirmatory_trials": 0,
            "prior_trials": [],
            "ledger_path": LEDGER,
        },
        "search_budget": {
            "max_pilot_trials_per_cell": 2,
            "max_confirmatory_attempts_per_candidate": 1,
        },
        "holdout": {"required": False, "cells": []},
        "cells": cells,
    }
    output.write_text(json.dumps(addendum, indent=2) + "\n")
    print(f"{stage}: {len(cells)} cells frozen at {frozen} -> {output}")


if __name__ == "__main__":
    main()
