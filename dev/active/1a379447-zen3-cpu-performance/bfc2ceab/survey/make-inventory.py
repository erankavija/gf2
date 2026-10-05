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
- a witness tuple does not pass the release assertions it is recorded against.

`witness-runs.json` and `instruction-counts.json` are read as committed.

Usage: make-inventory.py, with no arguments
"""

import importlib.util
import json
import re
import sys

import review
from locate import HERE, ISSUE, KERNEL_PACKAGE, ROOT, package, repository_files, rust_code_text, tracked

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
    "record's own directory, whose witness example calls the pointers on purpose, a "
    "call is a line holding `.<field>)(` for a published pointer field of the "
    "inventory, doc comments included; a value use is `.<field>` in code text that no "
    "call parenthesis follows. Each file and field is assigned in the review with the "
    "entry points its uses reach and their counts."
)
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
    for path in tracked("**/*.rs"):
        if path.startswith(own):
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
        assigned[(path, field)] = sum(count for _, count in reach)
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
        if "witness" in interface:
            row["witness"] = witness(interface["witness"])
        if "unchecked_precedent" in interface:
            need([interface["unchecked_precedent"]], interface["id"])
            row["unchecked_precedent"] = interface["unchecked_precedent"]
        interfaces.append(row)
    need(list(review.DENYING.values()), "denying crates")

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
        "decisions": {
            name: {"recommended": option, "reason": reason}
            for name, (option, reason) in review.DECISIONS.items()
        },
        "measurement_cells": [
            {"cell": cell, "interface": name, "workload": workload, "why": why}
            for cell, name, workload, why in review.MEASUREMENT_CELLS
        ],
        "family_scan": family_scan(),
    }
    unknown = {row["interface"] for row in record["measurement_cells"]} - {
        row["id"] for row in interfaces
    }
    if unknown:
        raise SystemExit(f"measurement cells name unknown interfaces: {sorted(unknown)}")
    return record


def family_scan():
    """Which measurement families of the dispatch route inventory name a consumer."""
    path = repository_files.live_file(ROOT, review.FAMILY_RECORD)
    families = json.loads((ROOT / path).read_text())["measurement_families"]
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
        "`fn` pointer type, that reaches an unchecked memory access when its caller breaks "
        "a condition no release build checks. The `Witness call` column states how a "
        "release-build process ends when it calls the entry point through its published "
        "pointer with arguments that break the condition "
        "([`survey/witness-runs.json`](survey/witness-runs.json)); such a call has "
        "undefined behaviour, so the column records one build on one host.",
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
        "prototypes under `survey/check-prototype` follow that shape.",
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
                ["Condition no release build checks", "Checked in a debug build at",
                 "Unchecked access"],
                [[row["condition"], cite(row["debug_build_check"]),
                  cite(row["unchecked_access"])] for row in interface["conditions"]],
            ), ""]
            out.append(
                f"Contract text: {cite(interface['contract_claims'])}. Checked in release "
                f"builds: {cite(interface['release_checked'])}."
            )
            if "witness" in interface:
                found = interface["witness"]
                out += ["", (
                    "Witness: dimensions "
                    + ", ".join(f"`{key}` = {value}" for key, value in found["dimensions"].items())
                    + " pass the release assertions with operand lengths "
                    + ", ".join(f"`{key}` = {value}" for key, value in
                                found["operand_lengths_passing_the_assertions"].items())
                    + f", while the dimensions describe {found['elements_the_dimensions_describe']} "
                    f"elements of `{found['short_operand']}`."
                )]
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


def option_b(interface):
    """What an `unsafe fn` pointer changes for one interface."""
    entries = interface["entries"]
    denied = sum(site["calls"] for entry in entries for site in entry["callers"]
                 if site["denies_unsafe_code"])
    other = sum(sites(entry, kind) for entry in entries for kind in ("bench", "test"))
    other += sum(sites(entry, "production") for entry in entries) - denied
    return (
        f"{len(interface['fields'])} bundle field(s) and table entr"
        f"{'y' if len(entries) == 1 else 'ies'}; `// SAFETY:` sites: {denied} production "
        f"in a crate that denies `unsafe_code`, {other} in benches and tests"
    )


def decision_page(record):
    out = [
        "# Decision table: checked or unchecked kernel entry points",
        "",
        "> **Diátaxis Type:** Reference",
        "",
        "Generated by `survey/make-inventory.py`; the rows are those of "
        "[`inventory.md`](inventory.md), the instruction counts those of "
        "[`survey/instruction-counts.json`](survey/instruction-counts.json) under its "
        "`rule`. An instruction count is no timing: it states what the prototype of the "
        "check adds outside panic blocks, with each loop body counted once.",
        "",
        "Option (a) puts a release-mode check in the safe entry point, in the form of "
        "the M4RM Gray-table builders. Option (b) makes the pointer an `unsafe fn` with "
        "the condition under Safety, so each caller holds an `unsafe` block with a "
        "`// SAFETY:` comment. `gf2-core`, `gf2-algebra` and `gf2-coding` deny "
        "`unsafe_code` (`core-denies-unsafe`, `algebra-denies-unsafe`, "
        "`coding-denies-unsafe`), so a production call site there cannot hold that block: "
        "for such an interface option (b) keeps a checked safe entry in the kernel crate, "
        "at the same or a coarser grain, and makes only the inner kernel `unsafe`.",
        "",
    ]
    for name, description in record["classes"].items():
        decision = record["decisions"][name]
        out += [
            f"## {description['title']}",
            "",
            f"Check (a): {description['check']} Granularity: {description['granularity']}. "
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
                    option_b(interface),
                    f"({decision['recommended']})",
                    ", ".join(f"`{row['cell']}`" for row in record["measurement_cells"]
                              if row["interface"] == interface["id"]) or "—",
                ]
            )
        out += [
            table(
                ["Pointer type", "Entry points", "Instructions (a) adds",
                 "Kernel instructions", "Option (b)", "Recommended", "Cells in the plan"],
                rows,
            ),
            "",
            f"Reason for option ({decision['recommended']}): {decision['reason']}",
            "",
        ]
    return "\n".join(out)


def plan_page(record):
    cells, scan = record["measurement_cells"], record["family_scan"]
    out = [
        "# Measurement plan for checks that stay on a hot path",
        "",
        "> **Diátaxis Type:** Explanation",
        "",
        "Generated by `survey/make-inventory.py` from `survey/review.py`. Nothing here is "
        "frozen or queued; the plan states what a before/after measurement under the "
        "[measurement contract](../measurement-contract.md) needs once a check is chosen.",
        "",
        "## Existing families",
        "",
        f"The dispatch route inventory (jit:63bad95d, `{scan['record']}`) lists "
        f"{scan['families']} measurement families. {len(scan['families_naming_a_consumer'])} "
        "of them name a consumer of the inventory's entry points under the `family_scan` "
        "rule of [`inventory.json`](inventory.json), whose tokens stand for the prime "
        "fields, the sparse field matrix, the PLE, the permanents and the Gray-PAM "
        "demapper. Criterion benches name three of the consumers (`bench-sparse-matmat`, "
        "`bench-ple`, `bench-permanent`); a criterion bench is no protocol arm. A "
        "before/after cell therefore needs one new family with one new arm executable, "
        "built once against the kernel crate without the check and once with it, each "
        "build pinned by content digest.",
        "",
        "## Candidate cells",
        "",
        "One family, non-regression objective, whole-consumer metric, single-core arm, "
        "warm cache. A first attempt admits at most six confirmatory cells (worker brief, "
        "P-20); the freeze recomputes the bound from the family's ledger.",
        "",
        table(
            ["Cell", "Interface", "Workload", "Why this cell"],
            [[f"`{row['cell']}`", f"`{row['interface']}`", row["workload"], row["why"]]
             for row in cells],
        ),
        "",
        "The interfaces without a cell run their check once per matrix product or per "
        "demapped axis, or are called from no tracked Rust file; "
        "[`decision-table.md`](decision-table.md) lists each.",
        "",
        "## Window estimate",
        "",
        f"Estimate, by analogy with two committed queue lines of warm confirmations "
        f"(`queue-dense-product`, `queue-axpy`): {review.PILOT_MINUTES} minutes for the "
        f"pilot and {review.CONFIRMATION_MINUTES} minutes for the confirmation of "
        f"{len(cells)} cells, {review.PILOT_MINUTES + review.CONFIRMATION_MINUTES} minutes "
        "of window in two queue lines. The arm crate, its smoke through "
        "`benchmark-ab-runner smoke` and both builds are untimed work ahead of the window.",
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
