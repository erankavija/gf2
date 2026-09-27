#!/usr/bin/env python3
"""Project the shift and permutation story's committed measurement evidence.

The receipt's own acceptance summary supplies every A/B interval and decision.
The DVB attribution renderer supplies the shared order-statistic and conversion
parts. This projector verifies receipt-local protocol and addendum snapshots;
it never treats current source-tree drift as a measured-input change.
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[4]
ACTIVE = "dev/active/c04dd4ac-zen3-shifts-and-permutations"
BENCH = "dev/bench_results/c04dd4ac"
SHIFT = f"{BENCH}/residual-shift-profile"
DVB = f"{BENCH}/dvb-interleave-profile/v4-r2-pilot"
DVB_PROFILE = f"{BENCH}/dvb-interleave-profile/v4-r3-dynamic-profile"
CONFIRM = "dev/bench_results/00dd43c3/v4-r1-confirmation"
NR = "dev/bench_results/eda07788/2026-09-10-eda07788-nr-derate-confirmation"
TICK = chr(96)

_spec = importlib.util.spec_from_file_location(
    "dvb_attribution", ROOT / ACTIVE / "survey/make-dvb-tables.py"
)
if _spec is None or _spec.loader is None:
    raise RuntimeError("the canonical DVB attribution renderer is unavailable")
_attribution = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_attribution)
order_statistic = _attribution.order_statistic
gap_cells = _attribution.gap_cells
side_parts = _attribution.side_parts


def path(relative: str) -> pathlib.Path:
    return ROOT / relative


def digest(relative: str) -> str:
    return hashlib.sha256(path(relative).read_bytes()).hexdigest()


def quoted(value: object) -> str:
    return f"{TICK}{value}{TICK}"


def interval(values: list[float], digits: int = 2) -> str:
    point, low, high, _ = order_statistic(values)
    return f"{point:,.{digits}f} [{low:,.{digits}f}, {high:,.{digits}f}]"


def accepted(relative: str) -> tuple[dict, dict]:
    receipt = json.loads(path(f"{relative}/receipt.json").read_text())
    summary = json.loads(path(f"{relative}/acceptance-summary.json").read_text())
    if summary["receipt_sha256"] != digest(f"{relative}/receipt.json"):
        raise ValueError(f"{relative}: acceptance does not pin its receipt")
    if summary["verdict"] != "accepted":
        raise ValueError(f"{relative}: receipt is not accepted")
    for kind in ("protocol", "addendum"):
        pin = receipt[kind]
        snapshot = f"{relative}/{pin['snapshot']}"
        if digest(snapshot) != pin["sha256"]:
            raise ValueError(f"{relative}: {kind} snapshot does not match its receipt")
    if set(cell["cell_id"] for cell in receipt["cells"]) != set(
        cell["cell_id"] for cell in summary["cells"]
    ):
        raise ValueError(f"{relative}: receipt and acceptance cells differ")
    measured = rows_for(receipt)
    for result in summary["cells"]:
        if result["pairs"] != len(measured[result["cell_id"]]["pairs"]):
            raise ValueError(f"{relative}: pair count differs for {result['cell_id']}")
    return receipt, summary


def rows_for(receipt: dict) -> dict[str, dict]:
    return {cell["cell_id"]: cell for cell in receipt["cells"]}


def addendum(receipt: dict, relative: str) -> dict:
    snapshot = f"{relative}/{receipt['addendum']['snapshot']}"
    return json.loads(path(snapshot).read_text())


def measured_metric(cell: dict, side: str, bits: int, scaling: str) -> str:
    values = [pair[side]["ns_per_call"] for pair in cell["pairs"]]
    if scaling == "sustained-throughput":
        values = [bits / value * 1e9 / 2**20 for value in values]
        unit = "Mibit/s"
    else:
        unit = "ns/call"
    point = order_statistic(values)[0]
    return f"{point:,.2f} [{min(values):,.2f}, {max(values):,.2f}] {unit}"


def print_authority(receipts: list[tuple[str, str, dict, dict]]) -> None:
    print("## Accepted receipt authority")
    print()
    print(
        "The receipt SHA-256 equals the acceptance summary's pinned digest. Protocol and "
        "addendum digests verify against the receipt-local snapshots. The current working "
        "tree and Git revision do not decide whether a measured receipt remains valid. "
        "Host and toolchain are observed in each receipt's sessions."
    )
    print()
    print("| Family / receipt | receipt SHA-256 | protocol snapshot | addendum snapshot | verdict / role | runtime host | toolchain |")
    print("|---|---|---|---|---|---|---|")
    for label, relative, receipt, summary in receipts:
        hosts = sorted({f"{h['hostname']} / {h['cpu_model']}" for h in receipt["session_hosts"]})
        proto = receipt["protocol"]
        family = receipt["addendum"]
        print(
            f"| {label}: {quoted(relative + '/receipt.json')} | {quoted(summary['receipt_sha256'])} | "
            f"{quoted(proto['path'])} v{addendum(receipt, relative)['protocol']['version']} "
            f"{quoted(proto['sha256'])} | {quoted(family['sha256'])} | "
            f"{summary['verdict']} / {summary['label']} | {', '.join(hosts)} | "
            f"{quoted(receipt['toolchain'])} |"
        )
    print()


def print_shift(label: str, relative: str, receipt: dict, summary: dict) -> None:
    frozen = addendum(receipt, relative)
    declared = {cell["cell_id"]: cell for cell in frozen["cells"]}
    measured = rows_for(receipt)
    print(f"## {label}")
    print()
    if label == "Residual workload profile":
        print(
            "Exploratory residual-versus-word-offset comparisons use different operations. "
            "Their ratios classify material workload cost and cannot select an implementation."
        )
    else:
        print(
            "The baseline holds the shipped scalar funnel; the candidate selects its BMI2 "
            "route for the same residual operation. The accepted interval decides each "
            "confirmatory outcome. The per-arm medians and observed ranges below describe "
            "calls within the receipt; they are not confidence intervals."
        )
    print()
    print(
        f"Source: {quoted(relative + '/receipt.json')} and "
        f"{quoted(relative + '/acceptance-summary.json')}; protocol "
        f"{quoted(receipt['protocol']['sha256'])}; pairs and confidence are in each row."
    )
    print()
    print("| Cell | scaling | pairs | baseline median [observed range] | candidate median [observed range] | ratio [confidence interval] | decision | outcome |")
    print("|---|---|---:|---|---|---|---|---|")
    for result in summary["cells"]:
        name = result["cell_id"]
        cell = measured[name]
        spec = declared[name]
        bits = spec["workload"]["size"]["length_bits"]
        scaling = spec["scaling"]
        ci = result["interval"]
        print(
            f"| {quoted(name)} | {scaling} | {result['pairs']} | "
            f"{measured_metric(cell, 'baseline', bits, scaling)} | "
            f"{measured_metric(cell, 'candidate', bits, scaling)} | "
            f"{ci['estimate']:.4f} [{ci['lower']:.4f}, {ci['upper']:.4f}] "
            f"at {ci['confidence']:.4f} | {result['decision']} | {result['outcome']} |"
        )
    print()


def print_dvb(receipt: dict, summary: dict) -> None:
    decisions = {row["cell_id"]: row for row in summary["cells"]}
    print("## DVB-T2 packed whole-consumer comparison")
    print()
    print(
        "The one-frame BitPackedBatch boundary includes xdsopl PCTITL unpack, "
        "destructive-input copy with output allocation, and pack. Values are medians "
        "and order-statistic intervals across each exploratory cell's paired calls "
        "(coverage 0.969 for six pairs). The acceptance decision remains exploratory."
    )
    print()
    print(
        f"Source: {quoted(DVB + '/receipt.json')}, {quoted(DVB + '/acceptance-summary.json')}; "
        f"protocol {quoted(receipt['protocol']['sha256'])}."
    )
    print()
    print("| Cell | pairs | gf2 stage ns/call [interval] | xdsopl packed ns/call [interval] | decision |")
    print("|---|---:|---|---|---|")
    for cell in gap_cells(receipt):
        name = cell["cell_id"]
        base = side_parts(cell, "baseline")["total"]
        candidate = side_parts(cell, "candidate")["total"]
        print(
            f"| {quoted(name)} | {len(cell['pairs'])} | {interval(base, 0)} | "
            f"{interval(candidate, 0)} | {decisions[name]['decision']} |"
        )
    print()
    print("### xdsopl conversion and remaining work")
    print()
    print(
        "Each part has its own median and order-statistic interval over the same pairs. "
        "Remainder includes PCTITL and the untimed-separately packed-batch wrap; it is "
        "not a PCTITL-only measurement. The gf2 stage has zero unpack, copy, and pack "
        "at this boundary; its whole cost appears above."
    )
    print()
    print("| Cell | pairs | unpack ns/call [interval] | input copy and output allocation [interval] | pack ns/call [interval] | remainder ns/call [interval] |")
    print("|---|---:|---|---|---|---|")
    for cell in gap_cells(receipt):
        parts = side_parts(cell, "candidate")
        print(
            f"| {quoted(cell['cell_id'])} | {len(cell['pairs'])} | "
            f"{interval(parts['unpack_ns'], 0)} | {interval(parts['batch_fill_ns'], 0)} | "
            f"{interval(parts['pack_ns'], 0)} | {interval(parts['remainder'], 0)} |"
        )
    print()


def print_profile() -> None:
    profile = path(DVB_PROFILE)
    ladder = json.loads((profile / "ladder.json").read_text())
    case_ids = [case["id"] for case in ladder]
    sessions = []
    for case_file in sorted(profile.glob("rep-*/cases.json")):
        records = {row["id"]: row for row in json.loads(case_file.read_text())}
        if set(records) != set(case_ids):
            raise ValueError(f"{case_file}: incomplete profile ladder")
        sessions.append(records)
    if len(sessions) != 9:
        raise ValueError("the fresh DVB profile is not a completed nine-session series")
    print("## DVB-T2 path and BICM composition")
    print()
    print(
        "Each path contributes one median-of-windows observation per session. "
        "The table gives the across-session median and [x(2), x(8)] "
        "order-statistic interval (coverage 0.961). Allocation counts are exact "
        "per-call censuses. Separate-process shares describe composition only; "
        "the paired receipt above decides materiality."
    )
    print()
    print(
        f"Source: {quoted(DVB_PROFILE + '/rep-NN/cases.json')}; "
        f"runtime host and invocation: {quoted(DVB_PROFILE + '-provenance.md')} "
        f"(SHA-256 {quoted(digest(DVB_PROFILE + '-provenance.md'))})."
    )
    print()
    print("| Path | sessions | ns/call [interval] | allocations/call | bytes/call |")
    print("|---|---:|---|---:|---:|")
    for case in case_ids:
        rows = [session[case] for session in sessions]
        allocations = sorted({row["allocations_per_call"] for row in rows})
        allocated = sorted({row["allocated_bytes_per_call"] for row in rows})
        if len(allocations) != 1 or len(allocated) != 1:
            raise ValueError(f"{case}: allocation census varies across sessions")
        print(
            f"| {quoted(case)} | {len(rows)} | "
            f"{interval([row['ns_per_call'] for row in rows], 0)} | "
            f"{allocations[0]} | {allocated[0]} |"
        )
    print()


def print_nr(receipt: dict, summary: dict) -> None:
    cells = rows_for(receipt)
    print("## NR de-rate-matching no-win")
    print()
    print(
        "AFF3CT Puncturer_5G::depuncture is the operation-equivalent external arm "
        "inside its adapter. The accepted protocol-v3 confirmation reports the "
        "corrected confidence interval and outcome. Adapter unpack and pack are "
        "descriptive per-execution observations, reported as median [observed range] "
        "over the pairs; they do not decide the verdict."
    )
    print()
    print(
        f"Source: {quoted(NR + '/receipt.json')}, "
        f"{quoted(NR + '/acceptance-summary.json')}; protocol "
        f"{quoted(receipt['protocol']['sha256'])}. The full conversion and "
        f"runtime-host breakdown is {quoted('dev/bench_results/eda07788/tables-nr-derate.md')}."
    )
    print()
    print("| Cell | pairs | speedup [confidence interval] | decision | outcome | AFF3CT unpack ns [range] | AFF3CT pack ns [range] |")
    print("|---|---:|---|---|---|---|---|")
    for result in summary["cells"]:
        name = result["cell_id"]
        cell = cells[name]
        values = [pair["candidate"]["conversion"] for pair in cell["pairs"]]
        unpack = [value["unpack_ns"] for value in values]
        pack = [value["pack_ns"] for value in values]
        ci = result["interval"]
        print(
            f"| {quoted(name)} | {result['pairs']} | "
            f"{ci['estimate']:.4f} [{ci['lower']:.4f}, {ci['upper']:.4f}] "
            f"at {ci['confidence']:.4f} | {result['decision']} | {result['outcome']} | "
            f"{order_statistic(unpack)[0]:.0f} [{min(unpack):.0f}, {max(unpack):.0f}] | "
            f"{order_statistic(pack)[0]:.0f} [{min(pack):.0f}, {max(pack):.0f}] |"
        )
    print()


def print_disposition_sources(profile: dict, confirmation: dict) -> None:
    context_path = "dev/active/00dd43c3/profile-context.json"
    context = json.loads(path(context_path).read_text())
    for key in ("profile_receipt", "profile_summary"):
        pin = context[key]
        if digest(pin["path"]) != pin["sha256"]:
            raise ValueError(f"{context_path}: {key} digest does not match")
    if not confirmation["qualifies"]:
        raise ValueError("the confirmation no longer qualifies for retention")
    if any(row["outcome"] in ("fail", "not-confirmatory") for row in confirmation["cells"]):
        raise ValueError("the confirmation has a disqualifying cell")
    for direction in ("left-", "right-"):
        if not any(row["cell_id"].startswith(direction) and row["outcome"] == "pass" for row in confirmation["cells"]):
            raise ValueError(f"the confirmation has no pass for {direction}")
    if profile["qualifies"]:
        raise ValueError("the exploratory workload profile has become an adoption receipt")
    outcome = "dev/active/00dd43c3/confirmation-outcome.md"
    if "**Disposition: retain the BMI2-gated residual shift route.**" not in path(outcome).read_text():
        raise ValueError("the published retention disposition differs from the acceptance")
    print("## Content-pinned dispositions")
    print()
    print(
        "The exploratory profile's receipt and summary match their context pins. The "
        "confirmation qualifies, records pass in both directions and no disqualifying "
        "cell. The predeclared retention rule in the pilot addendum and the completed "
        "outcome document select the BMI2 route. The lane-crossing pilot row stays "
        "exploratory; it is distinct from the unimplemented AVX2 nomination."
    )
    print()
    print("| Source | SHA-256 | role |")
    print("|---|---|---|")
    for relative, role in (
        (context_path, "profile context pin"),
        ("dev/active/00dd43c3/pilot-addendum.json", "frozen retention rule"),
        ("dev/active/00dd43c3/confirmation-derivation.txt", "retained and dropped pilot cells"),
        (outcome, "completed retention disposition"),
        (f"{ACTIVE}/shift-profile.md", "material workload disposition"),
        (f"{ACTIVE}/shift-feasibility-record.md", "Rust 1.95 and scalar-fallback feasibility"),
        (f"{ACTIVE}/dvb-interleave-profile.md", "DVB not-material disposition"),
        ("dev/active/eda07788/findings.md", "NR no-win and unavailable mappings"),
        (f"{ACTIVE}/investigation.md", "consumer and comparator survey"),
    ):
        print(f"| {quoted(relative)} | {quoted(digest(relative))} | {role} |")


def main() -> None:
    shift_receipt, shift_summary = accepted(SHIFT)
    confirm_receipt, confirm_summary = accepted(CONFIRM)
    dvb_receipt, dvb_summary = accepted(DVB)
    nr_receipt, nr_summary = accepted(NR)
    receipts = [
        ("Residual workload", SHIFT, shift_receipt, shift_summary),
        ("BMI2 confirmation", CONFIRM, confirm_receipt, confirm_summary),
        ("DVB packed consumer", DVB, dvb_receipt, dvb_summary),
        ("NR LLR de-rate", NR, nr_receipt, nr_summary),
    ]
    print("# Shift and permutation publication tables")
    print()
    print("> **Diátaxis Type:** Reference")
    print()
    print_authority(receipts)
    print_shift("Residual workload profile", SHIFT, shift_receipt, shift_summary)
    print_shift("BMI2 route confirmation", CONFIRM, confirm_receipt, confirm_summary)
    print_dvb(dvb_receipt, dvb_summary)
    print_profile()
    print_nr(nr_receipt, nr_summary)
    print_disposition_sources(shift_summary, confirm_summary)


if __name__ == "__main__":
    main()
