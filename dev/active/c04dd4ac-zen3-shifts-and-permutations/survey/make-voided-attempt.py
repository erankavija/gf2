#!/usr/bin/env python3
"""Write the voided-attempt record of the aborted DVB profile attempt (jit:9fb40c83).

Every count, identity and digest comes from the preserved stage: its execution
log, its plan, its frozen addendum snapshot and its reservation snapshot. The
prose names the defect, the rule and what the abort spends.
"""

import collections
import hashlib
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[4]
STAGE = pathlib.Path(
    "dev/bench_results/c04dd4ac/dvb-interleave-profile/v4-r1-pilot-abandoned"
)
OUTPUT = pathlib.Path(
    "dev/bench_results/c04dd4ac/dvb-interleave-profile/v4-voided-profile-attempt.json"
)
ADDENDUM = pathlib.Path(
    "dev/active/c04dd4ac-zen3-shifts-and-permutations/dvb-profile-addendum.json"
)

SUMMARY = (
    "The executor aborted this exploratory attempt for a procedural defect in its own "
    "launch, before reading any of its results, and voided it under the protocol's "
    "voided-attempt rule. Its reservation does not enter the chain the replacement "
    "attempt reserves on, so it is never interior to that chain and it spends no "
    "comparison and no candidate attempt. The aborted stage is preserved whole beside "
    "the published outcomes. The rule reaches only an attempt whose results were "
    "unread; an attempt whose results were read spends its reservation like any other."
)

DEFECT = (
    "The arm executable this attempt launched required GF2_BENCH_WINDOW=1 and "
    "GF2_BENCH=1 before it read its request. benchmark-ab-runner clears the child "
    "environment and installs only PATH, HOME, RAYON_NUM_THREADS, RUSTUP_TOOLCHAIN, "
    "the plan's per-arm variables and the child-v2 sentinel, so the window variables "
    "the launcher exported never reached the child. The first arm exited 2 without "
    "reading stdin, the runner's request write returned a broken pipe, and the child's "
    "diagnostics died with the pipe. Behind that defect the arm's request mirror "
    "spelled the absent cold_calls and decoder as null, which transport::decode_case "
    "rejects as noncanonical, so the handshake would have failed again on the next "
    "byte. Both defects are in the arm crate and both are repaired before the "
    "replacement attempt."
)

RESULTS_READ_DETAIL = (
    "The stage reached no cell-complete and wrote no checkpoint unit, so the attempt "
    "produced no window, no execution and no interval. Nothing of it is quoted "
    "anywhere in this survey."
)

SPENDS = (
    "nothing: no comparison and no candidate attempt. This family declares only "
    "exploratory cells, whose reservations spend zero comparisons in any case, and the "
    "replacement attempt reserves on the empty chain this attempt's reservation does "
    "not enter."
)


def main():
    events = [
        json.loads(line)
        for line in (ROOT / STAGE / "execution.log").read_text().splitlines()
    ]
    counts = collections.Counter(event["event"] for event in events)
    terminal = [event for event in events if event["event"] == "failed"]
    reservation = [
        json.loads(line)
        for line in (ROOT / STAGE / "inputs" / "trial-ledger.jsonl")
        .read_text()
        .splitlines()
    ]
    plan = json.loads((ROOT / STAGE / "plan.json").read_text())
    addendum_bytes = (ROOT / ADDENDUM).read_bytes()
    declared = json.loads(addendum_bytes)["cells"]
    started = {
        event["case"]["cell_id"] for event in events if event["event"] == "cell-start"
    }
    completed = {
        event["case"]["cell_id"] for event in events if event["event"] == "cell-complete"
    }

    record = {
        "kind": "voided-family-ledger-attempt",
        "summary": SUMMARY,
        "attempt": {
            "family": json.loads(addendum_bytes)["family"]["id"],
            "campaign": plan["campaign_id"],
            "protocol_version": json.loads(addendum_bytes)["protocol"]["version"],
            "rule": (
                "dev/active/f547c394/protocol.md, the voided-attempt paragraph of the "
                "family-ledger section; dev/active/f547c394/amendment-v4.md records the entry."
            ),
            "stage": str(STAGE),
            "addendum": str(ADDENDUM),
            "addendum_sha256": hashlib.sha256(addendum_bytes).hexdigest(),
            "defect": DEFECT,
            "results_read": False,
            "results_read_detail": RESULTS_READ_DETAIL,
            "abort": {
                "actor": "runner",
                "signal": None,
                "launcher_exit_code": 1,
                "receipt_written": (ROOT / STAGE / "receipt.json").exists(),
                "terminal_event": terminal[0]["event"] if terminal else None,
                "terminal_error": terminal[0]["details"]["error"] if terminal else None,
            },
            "reservation": {
                "comparisons": sum(line["comparisons"] for line in reservation),
                "candidates": sorted(
                    identity for line in reservation for identity in line["candidates"]
                ),
                "in_family_ledger": False,
                "snapshot": str(STAGE / "inputs" / "trial-ledger.jsonl"),
                "spends": SPENDS,
            },
            "cells_declared": len(declared),
            "cells_measured": len(completed),
            "cells_measured_ids": sorted(completed),
            "cells_unmeasured_ids": sorted(
                cell["cell_id"] for cell in declared if cell["cell_id"] not in completed
            ),
            "cells_started_unfinished_ids": sorted(started - completed),
            "execution_log": str(STAGE / "execution.log"),
            "execution_log_events": dict(sorted(counts.items())),
            "checkpoint_units": len(
                list((ROOT / STAGE / "checkpoints" / "units").glob("*.json"))
            ),
            "measured_data_location": (
                "none: the stage carries no checkpoint unit and no execution-progress "
                "record, so this attempt left no measured window anywhere."
            ),
        },
    }
    with open(ROOT / OUTPUT, "w") as handle:
        json.dump(record, handle, indent=2)
        handle.write("\n")
    print(
        f"{OUTPUT}: {record['attempt']['cells_measured']} cells measured, "
        f"{len(record['attempt']['cells_unmeasured_ids'])} unmeasured"
    )


if __name__ == "__main__":
    main()
