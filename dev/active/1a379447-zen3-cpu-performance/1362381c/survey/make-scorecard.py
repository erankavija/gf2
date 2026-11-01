#!/usr/bin/env python3
"""Per-cell competitive scorecard of the frozen workload set (jit:1362381c).

Reads the workload declaration, the production-drift record, the gap register and the committed
receipts they name, verifies each campaign from its execution log, and writes `scorecard.json`
and `scorecard.md`. Refuses to write when a gap cell has no register entry.

Usage (from the worktree root): make-scorecard.py OUT [--check]

OUT is the results directory: it holds the two outputs and the `determinism/` and
`receipt-reevaluation.json` verification records.
"""

import argparse
import functools
import hashlib
import importlib.util
import json
import pathlib
import re
import subprocess
import sys

sys.dont_write_bytecode = True
HERE = pathlib.Path(__file__).resolve().parent
ROOT = pathlib.Path(subprocess.check_output(
    ["git", "-C", str(HERE), "rev-parse", "--show-toplevel"], text=True).strip())
# The one bootstrap step every family locator shares; `repository_files` resolves the rest.
LOCATOR = subprocess.check_output(
    ["git", "ls-files", "--", ":(glob)**/repository_files.py", ":(exclude,glob)**/inputs/**"],
    text=True, cwd=ROOT).splitlines()
sys.path.insert(0, str((ROOT / min(LOCATOR, key=len)).parent))
import repository_files  # noqa: E402


def live(suffix):
    return ROOT / repository_files.live_file(ROOT, suffix)


sys.path.insert(0, str(live("31727c57/locate.py").parent))
import make_outcome  # noqa: E402

tables = make_outcome.tables
shared = tables.outcome
batch_fill = make_outcome.batch_fill
intervals_required = 6


def load(path):
    spec = importlib.util.spec_from_file_location(path.stem.replace("-", "_"), path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def read_json(path):
    return json.loads(pathlib.Path(path).read_bytes())


WORKLOAD = live("1362381c/workload-set.json")
DRIFT = live("1362381c/production-drift.json")
GAPS = live("1362381c/gaps.json")
REGISTRY = live("1362381c/survey/family-registry.json")
ACCOUNTING = live("78f0aafd/accounting.md")
JSON_NAME, MD_NAME = "scorecard.json", "scorecard.md"
# The clause of the deterministic contract each suite exercises.
DETERMINISM_SUITES = {
    "ldpc_batch_contract": "batching and routes", "pooled_batch_decoder": "batching",
    "batch_decode_worker_determinism": "worker counts", "core_dispatch_fallback": "fallback",
    "ldpc_simd_validation": "fallback", "executor_oom_fallback_run": "fallback",
    "checkpoint_compat": "resume", "hybrid_resume": "resume"}
DISPOSITIONS = ("measured-limit", "preserved-experiment", "tracked-issue")
NOT_RECORDED = "not recorded"
# Arm-name prefixes of the external projects; the receipt arm records name no citation key.
COMPARATOR_CITATIONS = (
    ("aff3ct", "Cassagne2019"), ("bchlib", "Bchlib2026"), ("bitshuffle", "Bitshuffle2026"),
    ("external-bitshuffle", "Bitshuffle2026"), ("external-m4ri", "AlbrechtBard2026"),
    ("ext-gfcomplete", "GfComplete2026"), ("ext-isal", "IsaL2026"), ("ext-m4rie", "Mfourrie2026"),
    ("gf2x", "GfTwoX2026"), ("gfcomplete", "GfComplete2026"), ("isal", "IsaL2026"),
    ("libpopcnt", "Libpopcnt2026"), ("m4rie", "Mfourrie2026"), ("m4ri", "AlbrechtBard2026"),
    ("mula", "Mula2018"), ("srsran", "Srsran2026"), ("xdsopl", "Xdsopl2026"),
)


def citation(arm):
    for prefix, key in sorted(COMPARATOR_CITATIONS, key=lambda item: -len(item[0])):
        if arm.startswith(prefix):
            return key
    sys.exit(f"external arm {arm} has no citation key")


@functools.lru_cache(maxsize=None)
def stage(receipt):
    """The verified campaign of a receipt path: its receipt, summary, addendum, plan and rows."""
    directory = (ROOT / receipt).parent
    tables.verifier.verify(str(directory / "execution.log"), str(directory / "receipt.json"),
                           str(directory / "plan.json"))
    pinned = read_json(directory / "receipt.json")
    summary, plan, cells = shared.outcomes(directory)
    decoders = {}
    if any(cell["decoder_quality"] for cell in pinned["cells"]):
        decoders = {row["summary"]["cell_id"]: row for row in tables.campaign(directory)["cells"]}
    return {"directory": directory, "receipt": pinned, "summary": summary, "plan": plan,
            "addendum": read_json(ROOT / pinned["addendum"]["path"]),
            "outcomes": {cell["summary"]["cell_id"]: cell for cell in cells},
            "measured": {cell["cell_id"]: cell for cell in pinned["cells"]},
            "decoders": decoders}


def spread(values):
    """Median with its order-statistic interval, or the median alone below the interval's n."""
    if len(values) < intervals_required:
        return {"median": sorted(values)[len(values) // 2] if values else None, "n": len(values),
                "interval": "unmeasured: too few executions for a 95 % median interval"}
    median, lower, upper, coverage = tables.median_interval(values)
    return {"median": median, "lower": lower, "upper": upper, "coverage": coverage,
            "n": len(values)}


def rate(latency, per_call, unit):
    if latency["median"] is None or "lower" not in latency:
        return {"status": "unmeasured"}
    return {"unit": unit, "median": per_call * 1e9 / latency["median"],
            "lower": per_call * 1e9 / latency["upper"],
            "upper": per_call * 1e9 / latency["lower"]}


def quality_of(record):
    if not record:
        return None
    keys = ("frames", "frame_errors", "fer", "fer_interval", "bits", "bit_errors", "ber",
            "ber_interval", "interval_method", "iterations", "settings")
    return {key: record[key] for key in keys if key in record}


def settings_differences(baseline, candidate):
    """Schedule and precision settings the two arms' quality runs state differently."""
    if not baseline or not candidate:
        return []
    left, right = baseline.get("settings") or {}, candidate.get("settings") or {}
    return sorted(key for key in left.keys() | right.keys()
                  if key != "batch_size" and left.get(key) != right.get(key))


def fill_of(cell, side, decoder, measured_pairs, tuned):
    """Batch-fill latency of one arm: the probed tuned record, its own probe, or a stated absence."""
    if not decoder:
        return {"status": "not applicable"}
    sides = {row["side"]: row for row in decoder["sides"]}
    if sides[side]["batch_fill"] is not None:
        return {"source": "receipt stage probe", **spread(sides[side]["batch_fill"])}
    recorded = tuned.get(cell) if side == "candidate" else None
    if recorded:
        return {"source": "probed tuned arm, descriptive record", "routes": recorded["routes"],
                "executable_sha256": sorted(recorded["executables"]), **spread(recorded["fill"])}
    if all(pair[side]["conversion"]["batch_fill_ns"] == 0 for pair in measured_pairs):
        return {"status": "unmeasured; recorded 0"}
    return {"status": NOT_RECORDED}


def memory_of(record, decoder, side):
    memory = {}
    if record and "memory_bytes" in record:
        memory["quality_run_bytes"] = record["memory_bytes"]
    if decoder:
        workspace = next(row for row in decoder["sides"] if row["side"] == side)["workspace"]
        if workspace:
            memory["lane_workspace_bytes"] = workspace
    return memory or NOT_RECORDED


def arm_side(cell_id, side, record, tuned):
    measured = record["measured"][cell_id]
    name = measured[f"{side}_arm"]
    arm = record["receipt"]["arms"][name]
    executions = [pair[side] for pair in measured["pairs"]]
    decoder = record["decoders"].get(cell_id)
    quality = (measured["decoder_quality"] or {}).get(side)
    latency = spread([execution["ns_per_call"] for execution in executions])
    workers = sorted({execution["workers_observed"] for execution in executions})
    throughput = rate(latency, 1, "calls/s")
    if decoder and len(workers) == 1:
        throughput = rate(latency, decoder["batch"] * workers[0], "frames/s")
    return {
        "arm": name, "build": arm["build"], "executable_sha256": arm["executable_sha256"],
        "rustflags": arm.get("rustflags"),
        "selected_path": sorted({execution["selected_path"] for execution in executions}),
        "workers_observed": workers, "latency_ns_per_call": latency, "throughput": throughput,
        "memory": memory_of(quality, decoder, side),
        "batch_fill_ns": fill_of(cell_id, side, decoder, measured["pairs"], tuned),
        "quality": quality_of(quality),
    }


def provenance(receipt):
    host = receipt["host"]
    producing = receipt["source"].get("producing") or {}
    return {
        "producing_manifest_sha256": producing.get("manifest_sha256", NOT_RECORDED),
        "protocol_sha256": receipt["protocol"]["sha256"],
        "contract_sha256": receipt["contract"]["sha256"],
        "addendum_sha256": receipt["addendum"]["sha256"],
        "execution_log_sha256": receipt["execution_log"]["sha256"],
        "toolchain": receipt["toolchain"],
        "host": {"cpu_model": host["cpu_model"], "os_kernel": host["os_kernel"],
                 "smt_active": host["smt_active"],
                 "governors": sorted(set(host["governors"].values())),
                 "observed_utc": host["observed_utc"]},
    }


def target_outcome(primary, external, planned, result):
    """The aspirational fastest-open-source outcome, from the cell's recorded decision."""
    if not primary:
        return None
    if planned["candidate_arm"] not in external:
        sys.exit(f"{result['cell_id']}: the outcome mapping reads the external arm as candidate")
    comparator = planned["candidate_arm"]
    key = citation(comparator)
    if result["status"] != "measured":
        return {"outcome": "unavailable", "citation": key, "comparator": comparator,
                "reason": result.get("reason") or result["status"]}
    if result["outcome"] == "not-confirmatory":
        return {"outcome": "inconclusive", "citation": key, "comparator": comparator,
                "reason": f"evaluator outcome not-confirmatory; decision {result['decision']} "
                          "is descriptive"}
    by_decision = {"regressed": "met", "improved": "not met"}
    named = by_decision.get(result["decision"], "inconclusive")
    return {"outcome": named, "citation": key, "comparator": comparator,
            "reason": f"decision {result['decision']}, outcome {result['outcome']}"}


def is_gap(row):
    """A primary cell recording a material gap, or whose estimate exceeds the frozen threshold."""
    if not row["primary"] or row["objective"] != "comparator-gap":
        return False
    return row["outcome"] == "pass" or (
        row["interval"] is not None and row["material_gap_threshold"] is not None
        and row["interval"]["estimate"] > row["material_gap_threshold"])


def gap_basis(row):
    return ("material gap recorded" if row["outcome"] == "pass" else
            f"estimate above the frozen threshold; outcome `{row['outcome']}` as evaluated")


def search_budget(record):
    """The family's declared budget and the attempts its receipt-pinned ledger prefix spent."""
    budget = record["addendum"].get("search_budget")
    pin = record["receipt"].get("trial_ledger")
    if not pin:
        return {"declared": budget, "spent": NOT_RECORDED}
    family = record["addendum"]["family"]["id"]
    entries = [json.loads(line) for line in
               (ROOT / pin["path"]).read_text().splitlines() if line.strip()]
    own = []
    for entry in entries:
        if entry["family"] == family:
            own.append(entry)
        if entry["campaign"] == record["receipt"]["campaign_id"]:
            break
    return {"declared": budget, "spent": {
        "reservations": len(own),
        "confirmatory_attempts": sum(entry["comparisons"] > 0 for entry in own),
        "comparisons": sum(entry["comparisons"] for entry in own)}}


def cell_row(cell, classes, integrated, tuned):
    record = stage(cell["receipt"])
    identifier = cell["cell"]
    outcome = record["outcomes"][identifier]
    declared, planned, result = outcome["declared"], outcome["planned"], outcome["summary"]
    measured = record["measured"][identifier]
    baseline = arm_side(identifier, "baseline", record, tuned.get(cell["family"], {}))
    candidate = arm_side(identifier, "candidate", record, tuned.get(cell["family"], {}))
    bounds = result.get("interval")
    drift = classes[cell["receipt"]]
    return {
        "domain": cell["domain"], "family": cell["family"], "cell": identifier,
        "role": cell["role"], "primary": cell["primary"],
        "evidence": "integrated" if cell["family"] in integrated else "consumed",
        "integrated_family": integrated.get(cell["family"]),
        "receipt": cell["receipt"], "receipt_sha256": cell["receipt_sha256"],
        "raw_samples": cell["receipt"],
        "drift_class": drift["class"], "drift_basis": drift["basis"],
        "provenance": provenance(record["receipt"]),
        "workload": declared["workload"], "metric_kind": declared["metric_kind"],
        "objective": declared["objective"],
        "comparator_kind": (declared.get("decoder") or {}).get("arm_kind")
        or declared["metric_kind"],
        "scaling": declared.get("scaling"), "core_arm": result["core_arm"],
        "resolved_cpus": len(measured["resolved_cpus"]),
        "baseline": baseline, "candidate": candidate,
        "settings_differences": settings_differences(baseline["quality"], candidate["quality"]),
        "status": result["status"], "interval": bounds, "pairs": result["pairs"],
        "margins": result.get("margins"),
        "material_gap_threshold": record["addendum"]["effect"].get("material_gap_threshold"),
        "decision": result["decision"], "outcome": result["outcome"],
        "interpretation": outcome["interpretation"],
        "reason": result.get("reason"), "unresolved_settings": result.get("unresolved_settings"),
        "verdict": record["summary"]["verdict"], "qualifies": record["summary"]["qualifies"],
        "target": target_outcome(cell["primary"], cell["external_arms"], planned, result),
    }


def anchor_slug(heading):
    text = re.sub(r"[`*]", "", heading.strip().lower())
    text = re.sub(r"[^\w\- ]", "", text)
    return text.replace(" ", "-")


def check_pointer(pointer, context):
    path, _, fragment = pointer.partition("#")
    target = ROOT / path
    if not target.is_file():
        sys.exit(f"{context}: {path} does not exist")
    if fragment:
        slugs = {anchor_slug(match[1]) for match in
                 re.finditer(r"^#+\s+(.*)$", target.read_text(encoding="utf-8"), re.M)}
        if fragment not in slugs:
            sys.exit(f"{context}: {path} has no heading #{fragment}")


def check_issue(identifier, context):
    if not list((ROOT / ".jit/issues").glob(f"{identifier}*.json")):
        sys.exit(f"{context}: no issue {identifier}")


def check_entry(entry):
    """A disposition names its kind's own evidence, and every pointer resolves."""
    context = "/".join(sorted({cell["family"] for cell in entry["cells"]}))
    kind = entry["disposition"]
    if kind not in DISPOSITIONS:
        sys.exit(f"{context}: disposition {kind} is not one of {DISPOSITIONS}")
    if kind == "tracked-issue":
        if not entry.get("issue") or not entry.get("next_experiment"):
            sys.exit(f"{context}: a tracked issue needs its id and next falsifiable experiment")
        check_issue(entry["issue"], context)
    elif not entry.get("record"):
        sys.exit(f"{context}: {kind} needs a record pointer")
    for pointer in [entry.get("record"), *entry.get("see_also", [])]:
        if pointer:
            check_pointer(pointer, context)


def register(rows, gaps):
    """One disposition per gap cell; refuses a missing, duplicate or non-gap entry."""
    assigned = {}
    for entry in gaps["entries"]:
        check_entry(entry)
        for cell in entry["cells"]:
            key = (cell["family"], cell["cell"])
            if key in assigned:
                sys.exit(f"gap register assigns {key} twice")
            assigned[key] = entry
    gap_rows = {(row["family"], row["cell"]): row for row in rows if is_gap(row)}
    missing = sorted(gap_rows.keys() - assigned.keys())
    if missing:
        sys.exit(f"refusing to write: material-gap cells without a register entry: {missing}")
    stale = sorted(assigned.keys() - gap_rows.keys())
    if stale:
        sys.exit(f"gap register names cells that record no gap: {stale}")
    return [{
        "domain": row["domain"], "family": row["family"], "cell": row["cell"],
        "basis": gap_basis(row), "outcome": row["outcome"], "decision": row["decision"],
        "disposition": assigned[key]["disposition"], "issue": assigned[key].get("issue"),
        "next_experiment": assigned[key].get("next_experiment"),
        "record": assigned[key].get("record"), "statement": assigned[key].get("statement"),
        "see_also": assigned[key].get("see_also", []),
        "search_budget": search_budget(stage(row["receipt"])),
    } for key, row in sorted(gap_rows.items(), key=lambda item: (item[1]["domain"], item[0]))]


def pilot_only_fill(rows):
    """FI-2 pilot configurations outside confirmation, with their descriptive batch fill."""
    record = make_outcome.batch_fill_record(batch_fill.PILOT_DIRECTORY / batch_fill.PILOT_CAMPAIGN)
    confirmed = {row["cell"] for row in rows}
    return [{"cell": cell, "workers": item["workers"], "batch_per_worker": item["batch"],
             "routes": item["routes"], "executable_sha256": sorted(item["executables"]),
             "stage_probes": sorted(item["probes"]), "statistic": sorted(item["statistics"]),
             "batch_fill_ns": spread(item["fill"])}
            for cell, item in sorted(record["cells"].items()) if cell not in confirmed]


def determinism_record(directory):
    """Per suite: the contract clause and the passed and skipped tests of the committed log."""
    log = (directory / "determinism/nextest.log").read_text(encoding="utf-8")
    identity = (directory / "determinism/run-identity.txt").read_text(encoding="utf-8").splitlines()
    rows = []
    for suite, clause in DETERMINISM_SUITES.items():
        rows.append({"suite": suite, "clause": clause,
                     "passed": len(re.findall(rf"^\s+PASS .*::{suite} ", log, re.M)),
                     "failed": len(re.findall(rf"^\s+FAIL .*::{suite} ", log, re.M)),
                     "skipped": len(re.findall(rf"^\s+SKIP .*::{suite} ", log, re.M))})
    return {"toolchain": identity[0], "source": cited(directory / "determinism/nextest.log"),
            "summary": re.search(r"Summary \[[^]]*\] (.*)", log)[1], "suites": rows}


def reevaluation_record(directory, rows):
    """Fresh-checkout verdict of every receipt the scorecard consumes, read from its record."""
    record = read_json(directory / "receipt-reevaluation.json")
    verdicts = {row["receipt"]: row for row in record["receipts"]}
    consumed = sorted({row["receipt"] for row in rows})
    missing = [receipt for receipt in consumed if receipt not in verdicts]
    if missing:
        sys.exit(f"receipts missing from the re-evaluation record: {missing}")
    return {"source": cited(directory / "receipt-reevaluation.json"), "evidence_revision": record["evidence_revision"],
            "toolchain": record["toolchain"], "receipts": len(consumed),
            "differs": [receipt for receipt in consumed if not verdicts[receipt]["reproduces"]]}


def pending_rows(workload):
    rows = []
    for entry in workload["pending"]:
        reason = entry.get("context_only") or (
            "no receipt declares a family this pattern matches; the design's D-3 ruling names "
            "no integrated family beyond its four")
        rows.append({"pattern": entry["pattern"], "domain": entry["domain"], "reason": reason,
                     "context": entry.get("context")})
    return rows


def qualify(rows, rulings):
    """Attach each published ruling on a receipt; a ruled primary cell's target is inconclusive."""
    for ruling in rulings:
        for row in rows:
            if (row["family"] in ruling["families"]
                    and row["cell"] in ruling.get("cells", [row["cell"]])):
                row["evidence_ruling"] = {"ruling": ruling["ruling"], "record": ruling["record"]}
                if row["target"]:
                    row["target"] |= {"outcome": "inconclusive",
                                      "reason": f"{ruling['ruling']} ({ruling['record']})"}


def workload_key(row):
    """Rows sharing a key time the same gf2 path on the same operation, size and configuration."""
    return (row["workload"]["identity"], json.dumps(row["workload"]["size"], sort_keys=True),
            row["metric_kind"], row["core_arm"], row["baseline"]["build"],
            tuple(row["baseline"]["selected_path"]))


def apply_workload_targets(rows):
    """The target holds per workload over every other project's measured arm.

    Each row keeps its own comparison as `pairwise`. A confirmed loss to another project makes
    the workload `not met`; otherwise an undecided comparison makes it `inconclusive`. Rows
    against the same project stay apart: they measure that project at other gf2 generations.
    """
    primary = [row for row in rows if row["target"]]
    for row in primary:
        row["target"]["pairwise"] = row["target"]["outcome"]
    for row in primary:
        others = [other for other in primary if workload_key(other) == workload_key(row)
                  and other["target"]["citation"] != row["target"]["citation"]]
        pairwise = row["target"]["pairwise"]
        for level, phrase in (("not met", "loses to"), ("inconclusive", "undecided against")):
            deciding = [other for other in others if other["target"]["pairwise"] == level]
            if deciding and pairwise != "not met" and (level == "not met" or pairwise == "met"):
                row["target"]["outcome"] = level
                row["target"]["reason"] += "; workload " + phrase + " " + ", ".join(
                    f"[{other['target']['citation']}] in `{other['cell']}`" for other in deciding)
                break


def build(directory):
    workload = read_json(WORKLOAD)
    selector = load(live("a203a23c/select-receipts.py"))
    selected, _ = selector.select(ROOT)
    missing = sorted({cell["receipt"] for cell in workload["cells"]} - set(selected))
    if missing:
        sys.exit(f"workload receipts the selector does not enumerate: {missing}")
    drift = read_json(DRIFT)
    classes = {entry["receipt"]: row for row in drift["rows"] for entry in row["receipts"]}
    registry = read_json(REGISTRY)
    integrated = {entry["pattern"]: entry["integrated_family"] for entry in registry["families"]
                  if entry.get("integrated_family") and "*" not in entry["pattern"]}
    decoder_family = next(pattern for pattern, name in integrated.items() if name == "FI-2")
    tuned = {decoder_family: make_outcome.batch_fill_record()["cells"]}
    rows = [cell_row(cell, classes, integrated, tuned) for cell in workload["cells"]]
    gaps = read_json(GAPS)
    for ruling in gaps["evidence_rulings"]:
        check_pointer(ruling["record"], ruling["families"][0])
    qualify(rows, gaps["evidence_rulings"])
    apply_workload_targets(rows)
    for entry in gaps["unavailable_comparisons"]:
        check_pointer(entry["record"], entry["comparator"])
    return {
        "workload_set": cited(WORKLOAD), "drift": cited(DRIFT), "gap_register": cited(GAPS),
        "accounting": cited(ACCOUNTING),
        "gap_rule": "a primary comparator-gap cell whose outcome is pass, or whose estimate "
                    "exceeds the family's frozen material_gap_threshold; each keeps its "
                    "outcome as evaluated",
        "target_rule": "per comparison, decision regressed: met; decision improved: not met; any "
                       "other decision, a not-confirmatory outcome or a published ruling on the "
                       "receipt: inconclusive; an unmeasured cell: unavailable. The target "
                       "holds per workload: a loss to another project's measured arm on the same "
                       "operation, size, consumer scope, core arm, gf2 build and gf2 path makes "
                       "the cell not met, and an undecided comparison there makes a met cell "
                       "inconclusive; `pairwise` keeps the cell's own comparison",
        "cells": rows,
        "gaps": register(rows, gaps),
        "pilot_only_batch_fill": pilot_only_fill(
            [row for row in rows if row["family"] == decoder_family]),
        "unavailable_comparisons": gaps["unavailable_comparisons"],
        "pending": pending_rows(workload),
        "determinism": determinism_record(directory),
        "reevaluation": reevaluation_record(directory, rows),
    }


def cited(path):
    return {"path": path.relative_to(ROOT).as_posix(),
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


def number(value):
    return f"{value:.4g}"


def span(value):
    if isinstance(value, str):
        return value
    if value.get("status"):
        return value["status"]
    if "lower" not in value:
        return f"{number(value['median'])} (n={value['n']}, no interval)"
    return (f"{number(value['median'])} [{number(value['lower'])}, {number(value['upper'])}], "
            f"n={value['n']}")


def rate_text(value):
    return value.get("status") or (f"{number(value['median'])} [{number(value['lower'])}, "
                                   f"{number(value['upper'])}] {value['unit']}")


def memory_text(value):
    if isinstance(value, str):
        return value
    parts = []
    if "quality_run_bytes" in value:
        parts.append(f"quality run {value['quality_run_bytes']} B")
    if "lane_workspace_bytes" in value:
        parts.append("lane workspace " + "/".join(str(v) for v in value["lane_workspace_bytes"]) + " B")
    return "; ".join(parts)


def quality_text(quality):
    if not quality:
        return "—"
    fer_low, fer_high = quality["fer_interval"]
    ber_low, ber_high = quality["ber_interval"]
    return (f"FER {quality['frame_errors']}/{quality['frames']} [{fer_low:.4g}, {fer_high:.4g}]; "
            f"BER {quality['bit_errors']}/{quality['bits']} [{ber_low:.4g}, {ber_high:.4g}]")


def both(row, render):
    return f"B: {render(row['baseline'])}<br>C: {render(row['candidate'])}"


def escape(text):
    return text.replace("|", "\\|")


def paths(side):
    return escape("; ".join(side["selected_path"]))


def link(target, directory):
    return shared.link(directory / MD_NAME, ROOT / target)


def interval_text(row):
    bounds = row["interval"]
    if not bounds:
        return "—"
    return (f"{bounds['estimate']:.4f} [{bounds['lower']:.4f}, {bounds['upper']:.4f}]; "
            f"alpha {1 - bounds['confidence']:.4g}; {row['pairs']} pairs")


def provenance_text(row):
    item = row["provenance"]
    manifest = item["producing_manifest_sha256"]
    host = item["host"]
    return (f"inputs `{manifest[:12]}`; {item['toolchain']}; {host['cpu_model']}, "
            f"{host['os_kernel']}, SMT {str(host['smt_active']).lower()}, "
            f"{'/'.join(host['governors'])}")


def target_text(row):
    target = row["target"]
    if not target:
        return "no external arm" if not row["primary"] else "—"
    return (f"{target['outcome']} [{target['citation']}]; own comparison {target['pairwise']}; "
            f"{target['reason']}")


CELL_HEADER = ["Family / cell", "Role", "Evidence and drift", "Receipt", "Provenance",
               "Selected path", "Time per call, ns", "Throughput", "Workload size", "Memory",
               "Scaling / core arm", "Speedup, baseline / candidate", "Decision / outcome",
               "Batch fill, ns", "BER / FER", "Target outcome"]


def cell_line(row, directory):
    settings = (f"; differing settings: {', '.join(row['settings_differences'])}"
                if row["settings_differences"] else "")
    receipt = f"[`{row['receipt_sha256'][:12]}`]({link(row['receipt'], directory)})"
    return [
        f"`{row['family']}` / `{row['cell']}`", row["role"],
        f"{row['evidence']}; `{row['drift_class']}`", receipt, provenance_text(row),
        both(row, paths), both(row, lambda side: span(side["latency_ns_per_call"])),
        both(row, lambda side: rate_text(side["throughput"])),
        f"`{json.dumps(row['workload']['size'], sort_keys=True)}`",
        both(row, lambda side: memory_text(side["memory"])),
        f"{row['scaling']} / {row['core_arm']}, {row['resolved_cpus']} CPUs",
        interval_text(row),
        f"`{row['decision']}` / `{row['outcome']}`: {row['interpretation']}"
        + (f"; {row['reason']}" if row["reason"] else "")
        + (f"; **{row['evidence_ruling']['ruling']}** (`{row['evidence_ruling']['record']}`)"
           if row.get("evidence_ruling") else ""),
        both(row, lambda side: span(side["batch_fill_ns"])),
        both(row, lambda side: quality_text(side["quality"])) + settings,
        target_text(row),
    ]


def render(record, directory):
    lines = [
        "# Integrated competitive scorecard", "",
        "> **Diátaxis Type:** Reference", "",
        f"Generated by `{pathlib.Path(__file__).name}` from "
        f"`{record['workload_set']['path']}`, `{record['drift']['path']}` and "
        f"`{record['gap_register']['path']}`; `scorecard.json` holds every field at full "
        "precision. The raw samples are the per-pair records of each row's `receipt.json`. "
        "Each campaign is verified from its execution log.", "",
        "Row labels: `consumed` rows are evidence of the code their receipt pins, labelled with "
        "the drift class; they are never measurements of the current tree. `integrated` rows "
        "belong to the families of this issue. Speedup is baseline median time over candidate "
        "median time. Latency, throughput and batch-fill intervals are descriptive "
        "order-statistic intervals of the per-execution medians; speedup intervals are the "
        "acceptance summary's percentile bootstrap. B and C name the baseline and candidate "
        "arms. Decoder per-worker differences include any route change `selected_path` records.",
        "", f"Target outcome rule: {record['target_rule']}. Gap rule: {record['gap_rule']}.", "",
    ]
    domains = sorted({row["domain"] for row in record["cells"]})
    for domain in domains:
        lines += [f"## {domain}", ""]
        rows = [row for row in record["cells"] if row["domain"] == domain]
        for kind in sorted({row["comparator_kind"] for row in rows}):
            lines += [f"### {domain}: {kind}", "",
                      *shared.table(CELL_HEADER, [cell_line(row, directory) for row in rows
                                                  if row["comparator_kind"] == kind]), ""]
    lines += [
        "## Gap register", "",
        f"Mechanism accounting per profiled cell: `{record['accounting']['path']}`.", "",
        *shared.table(
            ["Family / cell", "Basis", "Disposition", "Pointer", "Next falsifiable experiment "
             "or statement", "Search budget declared / spent"],
            [[f"`{gap['family']}` / `{gap['cell']}`", gap["basis"], gap["disposition"],
              f"`@/issue/{gap['issue']}`" if gap["issue"] else f"`{gap['record']}`",
              gap["next_experiment"] or gap["statement"] or "—",
              f"`{json.dumps(gap['search_budget']['declared'], sort_keys=True)}` / "
              f"`{json.dumps(gap['search_budget']['spent'], sort_keys=True)}`"]
             for gap in record["gaps"]]), "",
        "## Tuned-arm batch fill of FI-2 pilot-only configurations", "",
        "Descriptive records of the probed tuned arm; these configurations have no confirmation.",
        "", *shared.table(
            ["Cell", "Workers", "Batch per worker", "Routes", "Stage probes", "Batch fill, ns"],
            [[f"`{row['cell']}`", str(row["workers"]), str(row["batch_per_worker"]), row["routes"],
              ", ".join(str(item) for item in row["stage_probes"]), span(row["batch_fill_ns"])]
             for row in record["pilot_only_batch_fill"]]), "",
        "## Unavailable comparisons", "",
        *shared.table(["Comparator", "Scope", "Record"],
                      [[f"{row['comparator']} [{row['citation']}]", row["scope"],
                        f"`{row['record']}`"] for row in record["unavailable_comparisons"]]), "",
        "## Registry entries with no cell", "",
        *shared.table(["Pattern", "Domain", "Reason"],
                      [[f"`{row['pattern']}`", row["domain"], row["reason"]]
                       for row in record["pending"]]), "",
        "## Verification records", "",
        f"Determinism suites ({record['determinism']['toolchain']}, untimed, "
        f"`{record['determinism']['source']['path']}`): {record['determinism']['summary']}.", "",
        *shared.table(["Suite", "Clause", "Passed", "Failed", "Skipped"],
                      [[f"`{row['suite']}`", row["clause"], str(row["passed"]),
                        str(row["failed"]), str(row["skipped"])]
                       for row in record["determinism"]["suites"]]), "",
        f"Fresh-checkout verdicts of the {record['reevaluation']['receipts']} scorecard receipts "
        f"(`{record['reevaluation']['source']['path']}`, evidence revision "
        f"`{record['reevaluation']['evidence_revision'][:12]}`): "
        + (", ".join(f"`{receipt}`" for receipt in record["reevaluation"]["differs"])
           + " differ from the committed verdict." if record["reevaluation"]["differs"]
           else "each reproduces its committed verdict.")
        , ""]
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("out", type=pathlib.Path)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    if pathlib.Path.cwd() != ROOT:
        sys.exit("invoke from the worktree root")
    directory = arguments.out.resolve()
    record = build(directory)
    outputs = {directory / JSON_NAME: json.dumps(record, indent=1, ensure_ascii=False) + "\n",
               directory / MD_NAME: render(record, directory)}
    for path, text in outputs.items():
        if arguments.check:
            if not path.is_file() or path.read_text(encoding="utf-8") != text:
                sys.exit(f"projection differs: {path.relative_to(ROOT)}")
        else:
            shared.write(path, text)
    print(json.dumps({"cells": len(record["cells"]), "gaps": len(record["gaps"])}))


if __name__ == "__main__":
    main()
