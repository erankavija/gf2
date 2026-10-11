#!/usr/bin/env python3
"""Inventory the caller-trusted safe kernel entry points (jit:bfc2ceab).

Joins the review record `review.py` with the working tree and writes, in the
directory above this script, `inventory.json`, `inventory.md`,
`decision-table.md` and `measurement-plan.md`. The script fails, before
writing, when

- a safe function of `gf2-kernels-simd` that holds an `unsafe` block outside
  test code has no verdict in the review, or a verdict names no such function;
- a use of a published pointer field in a tracked Rust file is unassigned, or
  an assignment counts differently;
- a cited claim is absent from `source-evidence.json`;
- a witness tuple does not pass the release assertions it is recorded against;
- the struct literals that build the sparse matrix type are not those the
  review assigns to its constructors, or a method of that file takes
  `&mut self`.

- the freeze refusal record of the sparse product family pins another receipt
  than that family's last committed pilot.

`witness-runs.json` and `instruction-counts.json` are read as committed, and so
are the families' receipts and the freeze refusal record: the decision table
names each interface's rows of the family tables with the role, outcome and
decision each stage's acceptance summary records.

Usage: make-inventory.py, with no arguments
"""

import importlib.util
import json
import os
import re
import sys

import review
import spmm_family
from locate import (
    HERE, ISSUE, KERNEL_PACKAGE, ROOT, family_stages, package, repository_files, rust_code_text,
    tracked,
)

OUT = HERE.parent
WORD = 2**64
SCAN_RULE = (
    "An `unsafe` block is a boundary of kind `unsafe block` under the rule of the kernel "
    "safety-contract inventory (jit:7d44b71f), whose scanner this script runs on the "
    "working tree. A block is test code when it lies inside the braces of a module that "
    "a `#[cfg(test)]` attribute introduces. Its function is safe when the nearest "
    "preceding `fn` of the enclosing item's name is not an `unsafe fn`. Every safe "
    "function with a block outside test code is either an entry point of the inventory "
    "or cleared with a basis and a source claim."
)
USE_RULE = (
    "In every tracked Rust file outside receipt input snapshots and outside this "
    "record's own directory, whose witness example calls the pointers on purpose, and "
    "outside the condition test of the checked build, which does the same, a call is "
    "a line holding `.<field>)(` for a published pointer field of the "
    "inventory, doc comments included; a value use is `.<field>` in code text that no "
    "call parenthesis follows. Each file and field is assigned in the review with the "
    "entry points its uses reach and their counts."
)
STRUCT_LITERAL_FIELD = re.compile(r"^\s*col_idx(,|: Vec::new\(\),)\s*$")
METHOD = re.compile(r"^    (?:pub(?:\([a-z]+\))? )?fn (\w+)")
TEST_MODULE = re.compile(r"#\[cfg\(test\)\]\s*(?:#\[[^\]]*\]\s*)*(?:pub(?:\([a-z]+\))?\s+)?mod\s+\w+\s*\{")


def unsafe_scanner():
    """The boundary scanner and rule of the kernel safety-contract inventory."""
    path = ROOT / repository_files.live_file(ROOT, "make-unsafe-inventory.py")
    own = sys.modules.pop("locate")
    sys.path.insert(0, str(path.parent))
    try:
        spec = importlib.util.spec_from_file_location("unsafe_inventory", path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
    finally:
        sys.path.remove(str(path.parent))
        sys.modules["locate"] = own
    return module.boundaries


def test_lines(masked):
    """One-based lines of `masked` inside a `#[cfg(test)]` module."""
    lines = set()
    for found in TEST_MODULE.finditer(masked):
        depth, position = 1, found.end()
        while depth and position < len(masked):
            depth += {"{": 1, "}": -1}.get(masked[position], 0)
            position += 1
        first = masked.count("\n", 0, found.start()) + 1
        last = masked.count("\n", 0, position) + 1
        lines.update(range(first, last + 1))
    return lines


def safe_functions_with_unsafe(boundaries):
    """Package-relative path and name of each safe function holding a non-test block."""
    prefix = package(KERNEL_PACKAGE) + "/"
    found = {}
    for path in tracked(f"{prefix}**/*.rs"):
        source = (ROOT / path).read_text()
        masked = rust_code_text.masked(source)
        code, tests = masked.splitlines(), test_lines(masked)
        for row in boundaries(source):
            if row["kind"] != "unsafe block" or row["line"] in tests:
                continue
            name = re.compile(rf"\bfn\s+{re.escape(row['item'])}\b")
            declared = next(
                line for line in reversed(code[: row["line"]]) if name.search(line)
            )
            if re.search(rf"\bunsafe\s+fn\s+{re.escape(row['item'])}\b", declared):
                continue
            found.setdefault((path.removeprefix(prefix), row["item"]), []).append(row["line"])
    return found


def fields():
    return sorted({name for interface in review.INTERFACES for name in interface["field"].split(", ")})


def scan_uses():
    """Count of calls and value uses per (root-relative path, field)."""
    names = "|".join(fields())
    call = re.compile(rf"\.({names})\)\s*\(")
    value = re.compile(rf"\.({names})\b(?!\s*\)\s*\()")
    found = {}
    own = str(OUT.relative_to(ROOT)) + "/"
    condition_test = f"{package(review.CONDITION_TEST[0])}/{review.CONDITION_TEST[1]}"
    for path in tracked("**/*.rs"):
        if path.startswith(own) or path == condition_test:
            continue
        source = (ROOT / path).read_text()
        if not re.search(names, source):
            continue
        for raw, code in zip(source.splitlines(), rust_code_text.masked(source).splitlines()):
            for hit in call.finditer(raw):
                found[(path, hit[1])] = found.get((path, hit[1]), 0) + 1
            for hit in value.finditer(code):
                found[(path, hit[1])] = found.get((path, hit[1]), 0) + 1
    return found


def witness(record):
    """The wrapped operand lengths of a witness tuple, checked against its claim."""
    dimensions = record["dimensions"]
    lengths, true_sizes = {}, {}
    for operand, factors in record["lengths"].items():
        product = 1
        for factor in factors:
            product *= dimensions[factor]
        true_sizes[operand], lengths[operand] = product, product % WORD
    short = record["short"]
    if any(value >= WORD for value in dimensions.values()) or lengths[short] >= true_sizes[short]:
        raise SystemExit(f"witness {dimensions} does not wrap {short}")
    if any(lengths[name] != true_sizes[name] for name in lengths if name != short
           and len(record["lengths"][name]) == 1):
        raise SystemExit(f"witness {dimensions}: a plain length wraps")
    return {
        "dimensions": {name: power(value) for name, value in dimensions.items()},
        "operand_lengths_passing_the_assertions": {
            name: power(value) for name, value in lengths.items()
        },
        "short_operand": short,
        "elements_the_dimensions_describe": power(true_sizes[short]),
    }


def power(value):
    """`value` as text, a power of two written as one."""
    if value > 4096 and value & (value - 1) == 0:
        return f"2^{value.bit_length() - 1}"
    return str(value)


def plural(count, noun):
    return f"{count} {noun}{'' if count == 1 else 's'}"


def cite(claims):
    return ", ".join(f"`{claim}`" for claim in claims) if claims else "—"


def table(header, rows):
    lines = ["| " + " | ".join(header) + " |", "|" + "---|" * len(header)]
    lines += ["| " + " | ".join(str(cell) for cell in row) + " |" for row in rows]
    return "\n".join(lines)


def build():
    ledger_path = HERE / "source-evidence.json"
    ledger = {row["id"] for row in json.loads(ledger_path.read_text())["claims"]}
    counts = json.loads((HERE / "instruction-counts.json").read_text())
    added = {row["interface"]: row for row in counts["interfaces"]}
    witness_runs = json.loads((HERE / "witness-runs.json").read_text())
    ended = {run["entry"]: run["outcome"] for run in witness_runs["runs"]}
    problems = []

    def need(claims, where):
        problems.extend(f"{where}: claim {claim} is not in the ledger"
                        for claim in claims if claim not in ledger)

    # Verdict coverage.
    scanned = safe_functions_with_unsafe(unsafe_scanner())
    entry_of = {}
    for interface in review.INTERFACES:
        for identifier, path, wrapper, kernel in interface["entries"]:
            entry_of[(path, wrapper)] = identifier
    cleared = {(path, function): basis for path, function, basis, *_ in review.CLEARED}
    for key in sorted(set(entry_of) & set(cleared)):
        problems.append(f"{key}: both an entry point and cleared")
    for key in sorted(set(scanned) - set(entry_of) - set(cleared)):
        problems.append(f"{key}: a safe function with an unsafe block has no verdict")
    for key in sorted((set(entry_of) | set(cleared)) - set(scanned)):
        problems.append(f"{key}: the verdict names no safe function with an unsafe block")

    # Uses of the published pointers.
    scan, assigned = scan_uses(), {}
    callers = {}
    for owner, relative, field, kind, reach, discharge, claims in review.USES:
        path = f"{package(owner)}/{relative}"
        need(claims, f"use {relative}")
        assigned[(path, field)] = assigned.get((path, field), 0) + sum(
            count for _, count in reach
        )
        for entry, count in reach:
            if entry is None:
                continue
            callers.setdefault(entry, []).append(
                {
                    "package": owner,
                    "path": path,
                    "field": field,
                    "kind": kind,
                    "calls": count,
                    "denies_unsafe_code": kind == "production" and owner in review.DENYING,
                    "discharge": discharge,
                    "claims": claims,
                }
            )
    for key in sorted(set(scan) | set(assigned)):
        if scan.get(key, 0) != assigned.get(key, 0):
            problems.append(
                f"{key}: {scan.get(key, 0)} uses in the tree, {assigned.get(key, 0)} assigned"
            )

    interfaces = []
    for interface in review.INTERFACES:
        need(interface["contract_claims"] + interface["release_checked"], interface["id"])
        conditions = []
        for text, debug, access in interface["conditions"]:
            need(debug + access, interface["id"])
            conditions.append(
                {"condition": text, "debug_build_check": debug, "unchecked_access": access}
            )
        entries = []
        for identifier, path, wrapper, kernel in interface["entries"]:
            sites = callers.get(identifier, [])
            entries.append(
                {
                    "id": identifier,
                    "safe_function": wrapper,
                    "module": f"{package(KERNEL_PACKAGE)}/{path}",
                    "kernel": kernel,
                    "witness_call_ends": ended.get(identifier),
                    "callers": sites,
                    "call_sites": {
                        kind: sum(site["calls"] for site in sites if site["kind"] == kind)
                        for kind in ("production", "bench", "test")
                    },
                }
            )
        row = {
            "id": interface["id"],
            "class": interface["class"],
            "pointer_type": interface["type"],
            "bundle": interface["bundle"],
            "fields": interface["field"].split(", "),
            "contract_claims": interface["contract_claims"],
            "release_checked": interface["release_checked"],
            "conditions": conditions,
            "entries": entries,
            "checked_form": added[interface["id"]],
        }
        if interface["id"] in review.FORWARDING_HOOKS:
            hook, claim = review.FORWARDING_HOOKS[interface["id"]]
            need([claim], interface["id"])
            row["forwarding_hook"] = {"hook": hook, "claim": claim}
        if "witness" in interface:
            row["witness"] = witness(interface["witness"])
        if "unchecked_precedent" in interface:
            need([interface["unchecked_precedent"]], interface["id"])
            row["unchecked_precedent"] = interface["unchecked_precedent"]
        interfaces.append(row)
    need(list(review.DENYING.values()), "denying crates")
    need(review.GRAIN_POSITION[1], "grain position")
    for grain in review.GRAINS:
        need(grain["claims"], f"grain {grain['id']}")
    need([claim for *_, claim in review.SPARSE_CONSTRUCTORS], "sparse constructors")
    need(review.ADMISSIBILITY_CLAIMS, "admissibility")
    problems.extend(constructor_problems())

    if problems:
        raise SystemExit("\n".join(problems))

    entry_count = sum(len(row["entries"]) for row in interfaces)
    bases = {}
    for basis in cleared.values():
        bases[basis] = bases.get(basis, 0) + 1
    record = {
        "schema": "caller-trusted-entry-inventory-v1",
        "issue": ISSUE,
        "package": KERNEL_PACKAGE,
        "source_evidence": str(ledger_path.relative_to(ROOT)),
        "instruction_counts": str((HERE / "instruction-counts.json").relative_to(ROOT)),
        "witness_runs": str((HERE / "witness-runs.json").relative_to(ROOT)),
        "scan_rule": SCAN_RULE,
        "use_rule": USE_RULE,
        "counts": {
            "safe_functions_with_an_unsafe_block": len(scanned),
            "entry_points": entry_count,
            "pointer_types": len(interfaces),
            "cost_classes": len(review.CLASSES),
            "cleared_safe_functions": len(cleared),
            "cleared_by_basis": dict(sorted(bases.items())),
            "files_and_fields_using_a_pointer": len(scan),
        },
        "classes": review.CLASSES,
        "discharges": review.DISCHARGES,
        "interfaces": interfaces,
        "cleared": [
            {
                "module": f"{package(KERNEL_PACKAGE)}/{path}",
                "safe_function": function,
                "basis": basis,
                "claim": cleared_claim(path, function),
            }
            for path, function, basis, *_ in review.CLEARED
        ],
        "bases": review.BASES,
        "uses_outside_the_inventory": [
            {"path": f"{package(owner)}/{relative}", "field": field, "uses": count,
             "reason": discharge if entry is None and len(reach) == 1 else review.OUTSIDE_NOTE}
            for owner, relative, field, kind, reach, discharge, claims in review.USES
            for entry, count in reach
            if entry is None
        ],
        "decisions": decisions(),
        "check_grains": grains(interfaces),
        "measurement_cells": pilot_cells(),
        "family_scan": family_scan(),
    }
    unknown = {row["interface"] for row in record["measurement_cells"]} - {
        row["id"] for row in interfaces
    }
    if unknown:
        raise SystemExit(f"measurement cells name unknown interfaces: {sorted(unknown)}")
    return record


def decisions():
    """The invoker's decision per class, with the reviewer's basis."""
    path = OUT / review.DECISIONS_FILE
    held = json.loads(path.read_text())
    if set(held["classes"]) != set(review.CLASSES):
        raise SystemExit(f"{review.DECISIONS_FILE} does not decide exactly the cost classes")
    return {
        "record": str(path.relative_to(ROOT)),
        "decided_by": held["decided_by"],
        "decided_on": held["decided_on"],
        "ruling": held["ruling"],
        "later_rulings": held["later_rulings"],
        "classes": {name: {**held["classes"][name], "basis": review.BASIS[name]}
                    for name in review.CLASSES},
    }


def admissibility(stages):
    """Whether the family ledger admits one more confirmatory cell under P-20."""
    last = list(stages.values())[-1]
    receipt = json.loads((last["directory"] / "receipt.json").read_text())
    family = json.loads((last["directory"] / "acceptance-summary.json").read_text())["family"]
    ledger = ROOT / receipt["trial_ledger"]["path"]
    entries = [json.loads(line) for line in ledger.read_text().splitlines() if line.strip()]
    attempt = sum(1 for entry in entries if entry["comparisons"] > 0) + 1
    comparisons = sum(entry["comparisons"] for entry in entries) + 1
    attempt_alpha = family["family_alpha"] / (attempt * (attempt + 1))
    draws = family["bootstrap_resamples"] * attempt_alpha / comparisons / 2
    return {
        "ledger": str(ledger.relative_to(ROOT)),
        "next_attempt": attempt,
        "attempt_alpha": attempt_alpha,
        "comparisons_with_one_more_cell": comparisons,
        "expected_tail_draws": draws,
        "required_tail_draws": review.TAIL_DRAWS,
        "admits_a_confirmatory_cell": draws >= review.TAIL_DRAWS,
        "claims": review.ADMISSIBILITY_CLAIMS,
    }


def grain_family():
    """The family that prices the per-product grain: its stages and its refused freeze."""
    addendum = spmm_family.pilot_addendum()
    ledger = spmm_family.ledger()[0]
    tables = os.path.relpath(ledger.parent / review.GRAIN_FAMILY_TABLES, OUT)
    stages = []
    for label, directory, receipt in spmm_family.stages():
        summary = json.loads((directory / "acceptance-summary.json").read_text())
        stages.append({
            "stage": label,
            "label": receipt["label"],
            "receipt": str(directory.relative_to(ROOT)),
            "receipt_sha256": spmm_family.sha256(directory / "receipt.json"),
            "verdict": summary["verdict"],
            "pairs": sorted({cell["pairs"] for cell in summary["cells"]}),
            "session_load_average": [host["load_average"] for host in receipt["session_hosts"]],
            "link": f"{tables}#{label}",
            "cells": {
                cell["cell_id"]: f"{cell['role']}, outcome {cell['outcome']}, "
                                 f"decision {cell['decision']}"
                for cell in summary["cells"]
            },
        })
    refusal = json.loads((OUT / review.GRAIN_FAMILY_REFUSAL).read_text())
    pilots = [stage for stage in stages if stage["label"] == "pilot"]
    if [(trial["stage"], trial["sha256"]) for trial in refusal["pilot_trials"]] != [
            (stage["stage"], stage["receipt_sha256"]) for stage in pilots]:
        raise SystemExit(f"{review.GRAIN_FAMILY_REFUSAL} pins other pilot receipts")
    if len(pilots) != len(stages):
        raise SystemExit("the sparse product family holds a stage past its refused freeze")
    return {
        "family": addendum["family"]["id"],
        "pilot_addendum": review.GRAIN_FAMILY_ADDENDUM,
        "ledger": str(ledger.relative_to(ROOT)),
        "tables": tables,
        "cells": [cell["cell_id"] for cell in addendum["cells"]],
        "committed_stages": stages,
        "freeze": {
            "record": review.GRAIN_FAMILY_REFUSAL,
            "command": refusal["command"],
            "exit_status": refusal["exit_status"],
            "standard_error": refusal["standard_error"],
            "pilot_trials": len(refusal["pilot_trials"]),
            "pilot_trials_admitted_per_cell": refusal["pilot_trials_admitted_per_cell"],
            "pilot_trials_left": refusal["pilot_trials_left"],
        },
    }


def pricing_section(pricing):
    """The measured cost of the per-product check, as the pricing family's stages record it."""
    freeze, stages = pricing["freeze"], pricing["committed_stages"]
    addendum = f"[`{pricing['pilot_addendum']}`]({pricing['pilot_addendum']})"
    return [
        "### Measured cost of the per-product check",
        "",
        f"Stages of the family `{pricing['family']}` in the order of its ledger. The rows stand "
        f"in the family's [tables]({pricing['tables']}), which `survey/make-spmm-tables.py` "
        "writes from each stage's acceptance summary with the pair count, interval and outcome "
        "of every cell. The load column holds the first three fields of `/proc/loadavg` each "
        "receipt records at the start of a session (`session_hosts[].load_average`).",
        "",
        table(
            ["Stage", "Receipt", "Verdict", "Pairs per cell", "Load average at session start",
             "Cost rows"],
            [[stage["stage"], f"`{stage['receipt']}`", f"`{stage['verdict']}`",
              ", ".join(str(pairs) for pairs in stage["pairs"]),
              "; ".join(", ".join(str(value) for value in load)
                        for load in stage["session_load_average"]),
              ", ".join(f"[`{cell}`]({stage['link']}) ({row})"
                        for cell, row in stage["cells"].items())] for stage in stages],
        ),
        "",
        f"Confirmation: none. The launcher's `freeze` action (`{' '.join(freeze['command'])}`) "
        "derives the confirmation addendum from the receipt of the last pilot trial through "
        f"the canonical freezer; it exits {freeze['exit_status']} with `"
        + " ".join(freeze["standard_error"])
        + f"` and writes no addendum ([`{freeze['record']}`]({freeze['record']}), written by "
        "`survey/make-spmm-freeze-refusal.py`). The ledger holds "
        f"{plural(freeze['pilot_trials'], 'pilot trial')} of the "
        f"{freeze['pilot_trials_admitted_per_cell']} the search budget of {addendum} admits per "
        f"cell, which leaves {freeze['pilot_trials_left']}, so the family has no confirmatory "
        "cell.",
        "",
        "Outcome, exploratory and accepted: the measured cost of the per-product check is the "
        "rows above, at pilot level and without a confirmation, and the invoker's last ruling "
        f"on REQ-04 ([`{review.DECISIONS_FILE}`]({review.DECISIONS_FILE})) records it as "
        f"accepted. The frozen decision rule (the family description of {addendum}) decides "
        "from confirmatory cells; no confirmatory cell is committed, so the acceptance stands "
        "on the ruling and the rule records nothing.",
        "",
    ]


def constructor_problems():
    """Mismatches between the sparse type's struct literals and the review."""
    owner, relative, _ = review.SPARSE_TYPE
    masked = rust_code_text.masked((ROOT / package(owner) / relative).read_text())
    tests, found, method, problems = test_lines(masked), {}, None, []
    for number, line in enumerate(masked.splitlines(), start=1):
        if number in tests:
            continue
        if named := METHOD.match(line):
            method = named[1]
        if STRUCT_LITERAL_FIELD.match(line):
            found[method] = found.get(method, 0) + 1
        if "&mut self" in line:
            problems.append(f"{relative}:{number}: a method takes `&mut self`")
    assigned = {function: count for function, count, *_ in review.SPARSE_CONSTRUCTORS}
    for function in sorted(set(found) | set(assigned)):
        if found.get(function, 0) != assigned.get(function, 0):
            problems.append(
                f"{relative}: {function} holds {found.get(function, 0)} struct literals, "
                f"{assigned.get(function, 0)} assigned"
            )
    return problems


def hook_lines():
    """Per tracked Rust file, the definitions and calls of the forwarding hook."""
    name, found = review.GRAIN_HOOK, {}
    for path in tracked("**/*.rs"):
        source = (ROOT / path).read_text()
        if name not in source:
            continue
        for line in rust_code_text.masked(source).splitlines():
            if re.search(rf"\bfn\s+{name}\b", line):
                kind = "definition"
            elif re.search(rf"\b{name}\s*\(", line):
                kind = "call"
            else:
                continue
            found.setdefault(path, {"definition": 0, "call": 0})[kind] += 1
    return found


def grains(interfaces):
    """The grains of the sparse row check, each with the sites it changes."""
    entries = [entry for row in interfaces if row["class"] == review.GRAIN_CLASS
               for entry in row["entries"]]
    calls = {}
    for entry in entries:
        for site in entry["callers"]:
            if site["kind"] == "production":
                key = (site["path"], site["field"])
                calls[key] = calls.get(key, 0) + site["calls"]
    sites = {
        "wrappers": [f"`{entry['safe_function']}` of `{entry['module']}`" for entry in entries],
        "entry-calls": [f"`{field}` in `{path}`: {plural(count, 'call')}"
                      for (path, field), count in sorted(calls.items())],
        "hook": [
            f"`{review.GRAIN_HOOK}` in `{path}`: "
            + ", ".join(plural(count[kind], kind) for kind in count if count[kind])
            for path, count in sorted(hook_lines().items())
        ],
    }
    owner, relative, name = review.SPARSE_TYPE
    return {
        "class": review.GRAIN_CLASS,
        "position": {"text": review.GRAIN_POSITION[0], "claims": review.GRAIN_POSITION[1]},
        "options": [
            {**{key: grain[key] for key in ("id", "title", "state", "design", "check",
                                             "constraint", "measured", "claims")},
             "sites": [site for key in grain["sites"] for site in sites[key]]}
            for grain in review.GRAINS
        ],
        "further_confirmation": admissibility(cost_rows()[1]),
        "pricing_family": grain_family(),
        "sparse_type": {
            "name": name,
            "path": f"{package(owner)}/{relative}",
            "constructors": [
                {"function": function, "struct_literals": count, "bound": bound, "claim": claim}
                for function, count, bound, claim in review.SPARSE_CONSTRUCTORS
            ],
        },
    }


def pilot_cells():
    """The frozen pilot addendum's cells, each with the interface it measures."""
    addendum = json.loads((OUT / review.PILOT_ADDENDUM).read_text())
    declared = [cell["cell_id"] for cell in addendum["cells"]]
    if sorted(declared) != sorted(review.PILOT_CELLS):
        raise SystemExit("the review's pilot cells are not the addendum's cells")
    return [
        {
            "cell": cell["cell_id"],
            "interface": review.PILOT_CELLS[cell["cell_id"]][0],
            "check_grain": review.PILOT_CELLS[cell["cell_id"]][1],
            "check_form": review.PILOT_CELLS[cell["cell_id"]][2],
            "retained_for_confirmation": cell["cell_id"] in review.RETAINED_CELLS,
            "workload": cell["workload"]["identity"],
            "objective": cell["objective"],
            "metric_kind": cell["metric_kind"],
        }
        for cell in addendum["cells"]
    ]


def family_scan():
    """Which measurement families of other issues in the route inventory name a consumer."""
    path = repository_files.live_file(ROOT, review.FAMILY_RECORD)
    families = [family for family in
                json.loads((ROOT / path).read_text())["measurement_families"]
                if family["issue"] != ISSUE]
    naming = sorted(
        family["family"]
        for family in families
        if any(token in json.dumps(family) for token in review.CONSUMER_TOKENS)
    )
    return {
        "record": path,
        "rule": "A family names a consumer when the JSON text of its row holds one of "
                "`tokens`.",
        "tokens": review.CONSUMER_TOKENS,
        "families": len(families),
        "families_naming_a_consumer": naming,
    }


def cleared_claim(path, function):
    module = path.removeprefix("src/").removesuffix(".rs").replace("/", "-").replace("_", "-")
    return f"cleared-{module}-{function.replace('_', '-')}"


def sites(entry, kind):
    return entry["call_sites"][kind]


def inventory_page(record):
    counts = record["counts"]
    out = [
        "# Caller-trusted safe kernel entry points",
        "",
        "> **Diátaxis Type:** Reference",
        "",
        "Generated by `survey/make-inventory.py` from `survey/review.py` and the current "
        "tree; [`inventory.json`](inventory.json) holds every row in full, and claim "
        "identifiers resolve in [`survey/source-evidence.json`](survey/source-evidence.json).",
        "",
        "An entry point is a safe function of `gf2-kernels-simd`, published through a safe "
        "`fn` pointer type, whose kernel reaches an unchecked memory access when a caller "
        "condition fails and does not check that condition itself in a release build. "
        "Each entry point checks its conditions ahead of the kernel and panics under the "
        "kernel's name; the `Contract and release check` line of each pointer type cites "
        "the Panics text and the check. The `Witness call` column states how a "
        "release-build process ends when it calls the entry point through its published "
        "pointer with arguments that break the condition "
        "([`survey/witness-runs.json`](survey/witness-runs.json)): exit status 101 is a "
        "panic.",
        "",
        "## Counts",
        "",
        table(
            ["Quantity", "Count"],
            [
                ["safe functions holding an `unsafe` block outside test code",
                 counts["safe_functions_with_an_unsafe_block"]],
                ["entry points of the inventory", counts["entry_points"]],
                ["safe pointer types they are published through", counts["pointer_types"]],
                ["cost classes", counts["cost_classes"]],
                ["safe functions cleared", counts["cleared_safe_functions"]],
            ],
        ),
        "",
        "## Coverage",
        "",
        record["scan_rule"],
        "",
        record["use_rule"],
        "",
        table(
            ["Basis of a cleared function", "Functions", "Meaning"],
            [[f"`{basis}`", count, record["bases"][basis]]
             for basis, count in counts["cleared_by_basis"].items()],
        ),
        "",
        "## Checked precedent",
        "",
        "The M4RM Gray-table builders are the house form of a checked safe entry point: "
        "their pointer type lists the conditions under Panics (`gray-check-panics-doc`), "
        "each wrapper runs one shared assertion before its `unsafe` block "
        "(`gray-check-call`), the assertion forms its products with `checked_mul` and "
        "panics under the builder's name (`gray-check-products`), and one contract of "
        "cases runs over every builder (`gray-check-contract`). The M4RM tile wrappers "
        "check eight table indices per call in the same form (`tile-index-check`). The "
        "entry points of this inventory follow that shape.",
        "",
    ]
    for name, description in record["classes"].items():
        out += [f"## {description['title']}", ""]
        for interface in (row for row in record["interfaces"] if row["class"] == name):
            out += [f"### `{interface['pointer_type']}`", ""]
            out.append(
                table(
                    ["Entry point", "Module", "Kernel", "Production calls", "Bench calls",
                     "Test calls", "Witness call"],
                    [[f"`{entry['safe_function']}`", f"`{entry['module']}`",
                      f"`{entry['kernel']}`", sites(entry, "production"),
                      sites(entry, "bench"), sites(entry, "test"),
                      entry["witness_call_ends"] or "not run"]
                     for entry in interface["entries"]],
                )
            )
            out += ["", table(
                ["Condition the entry point checks", "Kernel's debug-build check",
                 "Unchecked access it guards"],
                [[row["condition"], cite(row["debug_build_check"]),
                  cite(row["unchecked_access"])] for row in interface["conditions"]],
            ), ""]
            out.append(
                f"Contract and release check: {cite(interface['contract_claims'])}. Checked "
                f"by the kernel in release builds: {cite(interface['release_checked'])}."
            )
            if "witness" in interface:
                found = interface["witness"]
                out += ["", (
                    "Witness: dimensions "
                    + ", ".join(f"`{key}` = {value}" for key, value in found["dimensions"].items())
                    + " pass the kernel's own release assertions with operand lengths "
                    + ", ".join(f"`{key}` = {value}" for key, value in
                                found["operand_lengths_passing_the_assertions"].items())
                    + f", while the dimensions describe {found['elements_the_dimensions_describe']} "
                    f"elements of `{found['short_operand']}`."
                )]
            if "forwarding_hook" in interface:
                hook = interface["forwarding_hook"]
                out += ["", f"Forwarding caller: the safe public hook `{hook['hook']}` "
                            f"passes its caller's arguments on to this entry point "
                            f"(`{hook['claim']}`)."]
            if "unchecked_precedent" in interface:
                out += ["", f"Unchecked form the crate publishes: "
                            f"`{interface['unchecked_precedent']}`."]
            rows = []
            for entry in interface["entries"]:
                for site in entry["callers"]:
                    rows.append(
                        [f"`{entry['safe_function']}`", f"`{site['path']}`", site["kind"],
                         site["calls"], f"`{site['discharge']}`", cite(site["claims"])]
                    )
            out += [""]
            if rows:
                out.append(table(
                    ["Entry point", "Caller", "Kind", "Calls", "Discharge", "Claims"], rows))
            else:
                out.append("No tracked Rust file calls these entry points.")
            out.append("")
    out += [
        "## Discharge classes",
        "",
        table(["Class", "Meaning"],
              [[f"`{name}`", text] for name, text in record["discharges"].items()]),
        "",
        "## Uses outside the inventory",
        "",
        table(["File", "Field", "Uses", "Why"],
              [[f"`{row['path']}`", f"`{row['field']}`", row["uses"], row["reason"]]
               for row in record["uses_outside_the_inventory"]]),
        "",
    ]
    return "\n".join(out)


def cost_rows():
    """Per committed stage: its section of the family tables, its verdict and its cells.

    A cell maps to the role, outcome and decision the stage's acceptance
    summary records.
    """
    stages = family_stages(OUT / review.CONFIRMATION_ADDENDUM)
    pilot = json.loads((stages["pilot"] / "receipt.json").read_text())
    tables = (ROOT / pilot["trial_ledger"]["path"]).parent / review.TABLES
    link, found = os.path.relpath(tables, OUT), {}
    for label, directory in stages.items():
        summary = json.loads((directory / "acceptance-summary.json").read_text())
        found[label] = {
            "directory": directory,
            "link": f"{link}#{label}",
            "verdict": summary["verdict"],
            "cells": {
                cell["cell_id"]: f"{cell['role']}, outcome {cell['outcome']}, "
                                 f"decision {cell['decision']}"
                for cell in summary["cells"]
            },
            "outcomes": {cell["cell_id"]: cell["outcome"] for cell in summary["cells"]
                         if cell["role"] == "confirmatory"},
        }
    return link, found


def rule_line(stages, cells):
    """What the family's frozen decision rule records for a class with `cells`."""
    outcomes = sorted({outcome for stage in stages.values()
                       for cell, outcome in stage["outcomes"].items() if cell in cells})
    if not outcomes:
        return "No confirmatory cell of the class is committed."
    recorded = ", ".join(f"`{outcome}`" for outcome in outcomes)
    verdict = ("the measured form of the check is recorded as an accepted measured cost"
               if outcomes == ["pass"]
               else "the rule refers the class to the invoker, whose decision stands above")
    return (f"Frozen decision rule (the family description of "
            f"[`{review.CONFIRMATION_ADDENDUM}`]({review.CONFIRMATION_ADDENDUM})): the "
            f"confirmatory cells of the class record {recorded}, so {verdict}.")


def cost_cell(stages, cells, only=None):
    """The cost rows of `cells`, grouped by stage."""
    groups = []
    for label, stage in stages.items():
        rows = [f"[`{cell}`]({stage['link']}) ({stage['cells'][cell]})"
                for cell in cells if cell in stage["cells"]]
        if rows and only in (None, label):
            groups.append(f"{label}: " + ", ".join(rows))
    return "; ".join(groups) or "—"


def class_cells(record, name):
    members = {interface["id"] for interface in record["interfaces"] if interface["class"] == name}
    return [row["cell"] for row in record["measurement_cells"] if row["interface"] in members]


def grain_section(record, stages):
    """The grains at which the sparse row class's check can stand."""
    found = record["check_grains"]
    cells, last = class_cells(record, found["class"]), list(stages)[-1]
    kind, more = found["sparse_type"], found["further_confirmation"]
    pricing = found["pricing_family"]
    verdict = ("admits" if more["admits_a_confirmatory_cell"] else "admits no")
    return [
        "### Grain of the column check",
        "",
        f"{found['position']['text']} Claims: {cite(found['position']['claims'])}.",
        "",
        table(
            ["Grain", "State", "Design", "Check runs", "Sites", "Constraint", "Cost rows",
             "Claims"],
            [[f"`{grain['id']}`: {grain['title']}", grain["state"], grain["design"],
              grain["check"], "; ".join(grain["sites"]) or "—", grain["constraint"],
              cost_cell(stages, cells, last) if grain["measured"]
              else (f"family `{pricing['family']}`, below" if grain is found["options"][0]
                    else "no cell prices this grain"),
              cite(grain["claims"])] for grain in found["options"]],
        ),
        "",
        f"Cost of the `{found['options'][0]['id']}` grain: priced by the family "
        f"`{pricing['family']}` ([`{pricing['pilot_addendum']}`]({pricing['pilot_addendum']})), "
        "whose candidate is the shipped product and whose baseline is the dispatcher's row "
        "loop over the unchecked row kernel, in cells "
        + ", ".join(f"`{cell}`" for cell in pricing["cells"])
        + "; its stages and outcome stand under the measured cost below. The earlier family "
        "cannot price it: a further "
        f"confirmatory attempt on its ledger (`{more['ledger']}`) is attempt "
        f"{more['next_attempt']} and spends alpha {more['attempt_alpha']:.6g}; with one more "
        f"cell the ledger holds {more['comparisons_with_one_more_cell']} comparisons, which "
        f"leaves {more['expected_tail_draws']:.3g} expected bootstrap draws per tail against "
        f"the {more['required_tail_draws']} P-20 requires, so the ledger {verdict} further "
        f"confirmatory cell ({cite(more['claims'])}). The figures are protocol arithmetic "
        "over the ledger and the family's frozen settings, not measurements.",
        "",
        f"Constructors of `{kind['name']}` (`{kind['path']}`): the functions that hold a "
        "struct literal of the type outside test code, which the generator scans for "
        "together with the absence of a method that takes `&mut self`.",
        "",
        table(
            ["Function", "Struct literals", "Each stored column index", "Claim"],
            [[f"`{row['function']}`", row["struct_literals"], row["bound"], f"`{row['claim']}`"]
             for row in kind["constructors"]],
        ),
        "",
        *pricing_section(pricing),
    ]


def decided(record, name):
    """The decision cell of class `name`."""
    held = record["decisions"]["classes"][name]
    grain = f" at the `{held['grain']}` grain" if "grain" in held else ""
    return f"({held['option']}){grain}"


def decision_page(record):
    tables, stages = cost_rows()
    missing = {row["cell"] for row in record["measurement_cells"]} - set(stages["pilot"]["cells"])
    if missing:
        raise SystemExit(f"the pilot's acceptance summary lacks cells: {sorted(missing)}")
    ruling = record["decisions"]
    out = [
        "# Decision table: checked or unchecked kernel entry points",
        "",
        "> **Diátaxis Type:** Reference",
        "",
        "Generated by `survey/make-inventory.py`; the rows are those of "
        "[`inventory.md`](inventory.md), the instruction counts those of "
        "[`survey/instruction-counts.json`](survey/instruction-counts.json) under its "
        "`rule`. An instruction count is no timing: it states what the prototype of the "
        "check holds outside panic blocks, with each loop body counted once.",
        "",
        "Option (a) puts a release-mode check in the safe entry point, in the form of "
        "the M4RM Gray-table builders. Option (b) makes the kernel an `unsafe fn` with "
        "the condition under Safety, so each caller holds an `unsafe` block with a "
        "`// SAFETY:` comment. `gf2-core`, `gf2-algebra` and `gf2-coding` deny "
        "`unsafe_code` (`core-denies-unsafe`, `algebra-denies-unsafe`, "
        "`coding-denies-unsafe`), so a production call site there cannot hold that block: "
        "for such an interface option (b) keeps a checked safe entry in the kernel crate, "
        "at the same or a coarser grain, and makes only the inner kernel `unsafe`.",
        "",
        f"Decision of the {ruling['decided_by']}, {ruling['decided_on']} "
        f"([`{review.DECISIONS_FILE}`]({review.DECISIONS_FILE})): {ruling['ruling']} The "
        "`Decision` column states it per class, and the tree holds the decided form of "
        "every entry point.",
        "",
        *[line for later in ruling["later_rulings"] for line in (
            f"Ruling of the {later['decided_by']}, {later['decided_on']}: {later['ruling']}", "")],
        "The last column names, for each interface, its rows in the family's "
        f"[tables]({tables}), which `survey/make-tables.py` writes from each stage's "
        "acceptance summary with the pair count, interval and outcome of every cell. Each "
        "row carries the role, outcome and decision its summary records: an exploratory row "
        "sizes the confirmation and decides nothing, and a confirmatory row is the "
        "family's recorded cost of the check. Verdicts: "
        + ", ".join(f"{label} `{stage['verdict']}`" for label, stage in stages.items())
        + "; the finding counts stand on the tables' Source lines. An interface without a "
        "row has no cell in the family.",
        "",
    ]
    for name, description in record["classes"].items():
        held = ruling["classes"][name]
        out += [
            f"## {description['title']}",
            "",
            f"Check: {description['check']} Granularity: {description['granularity']}. "
            f"{description['hoist']} On a hot path: {description['hot_path']}.",
            "",
        ]
        rows = []
        for interface in (row for row in record["interfaces"] if row["class"] == name):
            form = interface["checked_form"]
            first = form["pairs"][0]
            extra = "; ".join(
                f"{pair['check']}: +{pair['added']}"
                + (f", loop body {pair['added_loop_bodies']}" if pair["added_loop_bodies"] else "")
                for pair in form["pairs"][1:]
            )
            loop = f", loop body {first['added_loop_bodies']}" if first["added_loop_bodies"] else ""
            kernel = form.get("kernel")
            rows.append(
                [
                    f"`{interface['pointer_type']}`",
                    ", ".join(f"`{entry['safe_function']}`" for entry in interface["entries"]),
                    f"+{first['added']}{loop}" + (f"; {extra}" if extra else ""),
                    (f"{kernel['outside_panic_blocks']}, loop bodies {kernel['loop_bodies']}"
                     if kernel else "no committed listing"),
                    decided(record, name),
                    cost_cell(stages, [row["cell"] for row in record["measurement_cells"]
                                       if row["interface"] == interface["id"]]),
                ]
            )
        out += [
            table(
                ["Pointer type", "Entry points", "Instructions of the check",
                 "Kernel instructions", "Decision", "Cost rows"],
                rows,
            ),
            "",
            f"Decision {decided(record, name)}: {held['cost']}. Basis: {held['basis']}",
            "",
            rule_line(stages, class_cells(record, name)),
            "",
        ]
        if name == record["check_grains"]["class"]:
            out += grain_section(record, stages)
    return "\n".join(out)


def plan_page(record):
    cells, scan = record["measurement_cells"], record["family_scan"]
    measured = {row["interface"] for row in cells}
    unmeasured = [row["id"] for row in record["interfaces"] if row["id"] not in measured]
    addendum = json.loads((OUT / review.PILOT_ADDENDUM).read_text())
    stages = cost_rows()[1]
    more = record["check_grains"]["further_confirmation"]
    out = [
        "# Measurement record of the cost of the entry checks",
        "",
        "> **Diátaxis Type:** Explanation",
        "",
        "Generated by `survey/make-inventory.py` from `survey/review.py`, the frozen "
        f"pilot addendum [`{review.PILOT_ADDENDUM}`]({review.PILOT_ADDENDUM}) and the "
        "family's committed receipts. The family follows the "
        "[measurement contract](../measurement-contract.md).",
        "",
        "## Existing families",
        "",
        f"The dispatch route inventory (jit:63bad95d, `{scan['record']}`) lists "
        f"{scan['families']} measurement families of other issues. "
        f"{len(scan['families_naming_a_consumer'])} of them name a consumer of the inventory's entry points under the `family_scan` "
        "rule of [`inventory.json`](inventory.json), whose tokens stand for the prime "
        "fields, the sparse field matrix, the PLE, the permanents and the Gray-PAM "
        "demapper. Criterion benches name three of the consumers (`bench-sparse-matmat`, "
        "`bench-ple`, `bench-permanent`); a criterion bench is no protocol arm. The cost "
        "of the checks therefore has its own family.",
        "",
        f"## Family `{addendum['family']['id']}`",
        "",
        "The family measures a tree that holds every entry point in two forms: the "
        "baseline arm is built with no flag and publishes the entry points without their "
        "checks, and the candidate arm is the same arm package built with RUSTFLAGS "
        "`--cfg gf2_entry_checked`, under which the kernel crate's detection functions "
        "publish checked wrappers. That tree, the arm package, the plan projection and the "
        "launcher are the producing snapshot of each receipt (`inputs/producing` beside "
        "it); the plan records each arm's RUSTFLAGS and executable digest, and each "
        "execution reports the entry-point form its build publishes. The addendum's family "
        "description states the check each cell measures, the selection of the "
        "confirmation's cells and the decision rule; its `effect` object states the "
        "margins and their rationale. The sparse row cells measure the `per-row` grain of "
        "[`decision-table.md`](decision-table.md), the other cells the form the tree holds.",
        "",
        table(
            ["Cell", "Interface", "Workload", "Check grain", "Checked form the cell measures",
             "Retained for confirmation"],
            [[f"`{row['cell']}`", f"`{row['interface']}`", f"`{row['workload']}`",
              row["check_grain"], row["check_form"],
              "yes" if row["retained_for_confirmation"] else "no"] for row in cells],
        ),
        "",
        "Every cell is a non-regression, whole-consumer cell. The retained cells cover "
        "each grain at which a measured check runs: per sparse row, per kernel call of the "
        "F_3 permanent, per PLE panel, per row of a dense product and per whole dense "
        "product. Interfaces without a cell: "
        + ", ".join(f"`{name}`" for name in unmeasured)
        + ". The three whole-product kernels among them hold the check form "
        "`gemm-medium-n32` measures; the Gray-PAM kernels check one product per "
        "demapped axis and the remaining packed pointers are called from no tracked Rust "
        "file, so the family prices neither.",
        "",
        "## Stages",
        "",
        "Committed stages: "
        + ", ".join(f"[{label}]({stage['link']})" for label, stage in stages.items())
        + ". The confirmation addendum "
        f"[`{review.CONFIRMATION_ADDENDUM}`]({review.CONFIRMATION_ADDENDUM}) is derived "
        "from the committed pilot receipt by the canonical freezer. `survey/make-tables.py` "
        "writes the tables from each stage's acceptance summary, and "
        "[`decision-table.md`](decision-table.md) names each interface's rows.",
        "",
        "## Further confirmation",
        "",
        f"The family ledger admits {'a' if more['admits_a_confirmatory_cell'] else 'no'} "
        "further confirmatory cell under P-20; the arithmetic stands under the sparse row "
        "class of [`decision-table.md`](decision-table.md). The `per-product` grain of the "
        "sparse row check, which the tree holds, is therefore not measured by this family; "
        f"the family `{record['check_grains']['pricing_family']['family']}` prices it, and "
        "the decision table names its committed stages.",
        "",
    ]
    return "\n".join(out)


def main():
    if sys.argv[1:]:
        raise SystemExit(__doc__)
    record = build()
    (OUT / "inventory.json").write_text(json.dumps(record, indent=2) + "\n")
    (OUT / "inventory.md").write_text(inventory_page(record))
    (OUT / "decision-table.md").write_text(decision_page(record))
    (OUT / "measurement-plan.md").write_text(plan_page(record))
    print(f"{(OUT / 'inventory.json').relative_to(ROOT)}: {json.dumps(record['counts'])}")


if __name__ == "__main__":
    main()
