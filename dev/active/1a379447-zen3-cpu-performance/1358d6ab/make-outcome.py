#!/usr/bin/env python3
"""Outcome of the integrated comparator families and the consumed primary cells (jit:1358d6ab).

Reads each family's pilot and confirmation receipts with their acceptance summaries, plans and
pinned addenda, the BCH arm build and equivalence records, the workload-set
declaration and the production-drift record, and writes `outcome.md` beside this script. Cell
rows and their interpretations come from the shared `make-collection-outcome.py`.

Usage (from the worktree root): make-outcome.py [--check]
"""

import collections
import importlib.util
import json
import pathlib
import sys

sys.dont_write_bytecode = True
HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE / "bch"))
from locate import ROOT, repository_files  # noqa: E402

OUTPUT = HERE / "outcome.md"
EQUIVALENCE = HERE / "bch" / "external-equivalence.json"
BUILD = HERE / "bch" / "build-identity.json"
EQUAL_CODEWORDS = "codewords equal gf2's"
FAMILIES = (
    {"id": "FI-3", "family": "1362381c-ldpc-aff3ct-integration",
     "campaign": "1358d6ab-ldpc-aff3ct-integration",
     "comparator": "AFF3CT's matched flooding normalized min-sum decoder [Cassagne2019]"},
    {"id": "FI-4", "family": "1362381c-bch-integration", "campaign": "1358d6ab-bch-integration",
     "comparator": "the AFF3CT [Cassagne2019] and bchlib [Bchlib2026] BCH encoders"},
)
# Rationale key of each effect margin in the addendum schema.
RATIONALE = {"worthwhile_speedup": "rationale", "equivalence_margin": "equivalence_rationale",
             "material_gap_threshold": "material_gap_rationale"}


def live(suffix):
    return ROOT / repository_files.live_file(ROOT, suffix)


def load(path):
    module = importlib.util.module_from_spec(importlib.util.spec_from_file_location(
        path.stem.replace("-", "_"), path))
    module.__spec__.loader.exec_module(module)
    return module


shared = load(live("make-collection-outcome.py"))


def read(path):
    return json.loads(path.read_bytes())


def rel(path):
    return pathlib.Path(path).resolve().relative_to(ROOT).as_posix()


def anchor(target, text=None):
    """A Markdown link from `outcome.md` to `target`, labelled `text` or the target's path."""
    return f"[`{text or rel(target)}`]({shared.link(OUTPUT, pathlib.Path(target))})"


def interval(cell):
    bounds = cell["interval"]
    return (f"{bounds['estimate']:.4f}", f"[{bounds['lower']:.4f}, {bounds['upper']:.4f}] "
            f"at {bounds['confidence']:.4f}")


def fer(quality):
    low, high = quality["fer_interval"]
    return f"{quality['frame_errors']}/{quality['frames']} [{low:.4f}, {high:.4f}]"


def campaign(name):
    directory = shared.receipt_directory(name)
    receipt = read(directory / "receipt.json")
    return {"directory": directory, "receipt": receipt,
            "summary": read(directory / "acceptance-summary.json"),
            "addendum": read(ROOT / receipt["addendum"]["path"]),
            "freeze": ROOT / receipt["addendum"]["path"]}


def margin_changes(pilot, confirmation):
    """Each effect margin the confirmation addendum holds at another value than the pilot's."""
    before, after = pilot["addendum"]["effect"], confirmation["addendum"]["effect"]
    return [(name, before[name], after[name], after[RATIONALE[name]])
            for name in RATIONALE if before[name] != after[name]]


def family_section(family):
    pilot = campaign(family["campaign"] + "-pilot")
    confirmation = campaign(family["campaign"] + "-confirmation")
    summary, plan, cells = shared.outcomes(confirmation["directory"])
    if summary["family"]["family_id"] != family["family"]:
        raise SystemExit(f"{summary['campaign_id']} belongs to {summary['family']['family_id']}")
    effect = confirmation["addendum"]["effect"]
    receipt_cells = {cell["cell_id"]: cell for cell in confirmation["receipt"]["cells"]}
    stats = summary["family"]
    derivation = confirmation["freeze"].parent / "derivation.txt"
    lines = [f"## {family['id']} `{family['family']}`", "",
             f"Baseline: the gf2 production arm. Candidate: {family['comparator']}.", ""]
    for stage, record in (("Pilot", pilot), ("Confirmation", confirmation)):
        directory = record["directory"]
        lines.append(
            f"- {stage}: {anchor(directory / 'receipt.json')}, "
            f"{anchor(directory / 'acceptance-summary.md', 'acceptance-summary.md')}, "
            f"{anchor(directory / 'execution.log', 'execution.log')}; verdict "
            f"`{record['summary']['verdict']}`, qualifies "
            f"`{str(record['summary']['qualifies']).lower()}`, "
            f"{len(record['summary']['cells'])} cells, {record['summary']['sessions']} sessions, "
            f"{len(record['summary']['findings'])} findings.")
    lines += [
        f"- Ledger: {anchor(ROOT / confirmation['addendum']['family_wise']['ledger_path'])}. "
        f"Freeze outputs: {anchor(confirmation['freeze'])}, {anchor(derivation)}.",
        f"- Frozen effect: measurement resolution {effect['measurement_resolution']} from the "
        f"pilot, material-gap threshold {effect['material_gap_threshold']}, equivalence margin "
        f"{effect['equivalence_margin']}. Confirmation family statistics: {stats['comparisons']} "
        f"comparisons, attempt alpha {stats['attempt_alpha']}, corrected alpha "
        f"{stats['corrected_alpha']:.6f} per comparison, {stats['bootstrap_resamples']} "
        "bootstrap resamples.", ""]
    for name, before, after, rationale in margin_changes(pilot, confirmation):
        lines += [
            f"Margin ruling: the canonical freezer refused the pre-pilot `{name}` {before}, "
            f"which does not strictly exceed one plus the pilot resolution "
            f"{effect['measurement_resolution']}. The confirmation addendum holds {after}; its "
            f"recorded rationale: \"{rationale}\" {anchor(derivation, 'derivation.txt')} records "
            "the replacement.", ""]
    decoder = any(cell["decoder_quality"] for cell in receipt_cells.values())
    header = ["Cell", "Role", "Core arm", "Candidate arm", "Pairs", "Speedup", "Interval",
              "Margins (gap / eq)", "Decision", "Outcome", "Interpretation"]
    if decoder:
        header += ["FER gf2", "FER candidate"]
    rows = []
    for cell in cells:
        recorded = cell["summary"]
        row = [f"`{recorded['cell_id']}`", cell["declared"]["role"], recorded["core_arm"],
               f"`{cell['planned']['candidate_arm']}`", str(recorded["pairs"]),
               *interval(recorded),
               f"{recorded['margins']['improvement']} / {recorded['margins']['equivalence']}",
               f"`{recorded['decision']}`", f"`{recorded['outcome']}`", cell["interpretation"]]
        if decoder:
            quality = receipt_cells[recorded["cell_id"]]["decoder_quality"]
            row += [fer(quality["baseline"]), fer(quality["candidate"])]
        rows.append(row)
    lines += shared.table(header, rows)
    outcomes = collections.Counter(cell["summary"]["outcome"] for cell in cells
                                   if cell["declared"]["role"] != "exploratory")
    other = {key: count for key, count in outcomes.items() if key != "pass"}
    lines += ["", f"`qualifies` is `{str(summary['qualifies']).lower()}`: the protocol sets it "
              "only when every confirmatory and holdout cell records `pass`; here "
              + ", ".join(f"{count} record `{key}`" for key, count in sorted(outcomes.items()))
              + ". "
              + ("" if not other else "The addendum records \""
                 + confirmation["addendum"]["complexity_budget"]["maintenance_rationale"]
                 + "\", so `qualifies` selects no production change.")]
    if decoder:
        lines += ["", "FER columns: frame errors over frames of the receipt's quality run with "
                  "the receipt's Wilson 95 % interval."]
    lines += [""]
    if family["family"].endswith("bch-integration"):
        lines += equivalence_section(pilot, confirmation)
    return lines


def equivalence_section(pilot, confirmation):
    """REQ-02: the build record's equivalence run and each timed pair's own equality report."""
    build = read(BUILD)
    pinned = {arm["executable_sha256"] for arm in build["arms"].values()}
    served = collections.defaultdict(list)
    for stage, record in (("pilot", pilot), ("confirmation", confirmation)):
        measured = {arm["executable_sha256"] for arm in record["receipt"]["arms"].values()}
        if measured != pinned:
            raise SystemExit(f"{record['summary']['campaign_id']} measures executables "
                             "the build record does not pin")
        declared = {cell["cell_id"]: cell for cell in record["addendum"]["cells"]}
        for cell in record["receipt"]["cells"]:
            workload = declared[cell["cell_id"]]["workload"]
            equal = sum(EQUAL_CODEWORDS in pair["candidate"]["selected_path"]
                        for pair in cell["pairs"])
            key = (cell["candidate_arm"], workload["identity"],
                   json.dumps(workload["size"], sort_keys=True))
            served[key].append(f"{stage} `{cell['cell_id']}` {equal}/{len(cell['pairs'])}")
    rows = []
    for case in read(EQUIVALENCE):
        key = (case["engine"], case["case"]["workload"],
               json.dumps(case["case"]["size"], sort_keys=True))
        size = ", ".join(f"{name} {value}" for name, value in sorted(case["case"]["size"].items()))
        rows.append([f"`{case['engine']}`", f"`{case['case']['workload']}`", size,
                     f"`{case['outcome']}`", "; ".join(served.pop(key, [])) or "not measured"])
    if served:
        raise SystemExit(f"measured workloads without an equivalence case: {sorted(served)}")
    return [
        "### Bit-exact equivalence of the external arms", "",
        f"{anchor(BUILD)} pins the executables both receipts measure; its equivalence run "
        f"{anchor(EQUIVALENCE)} compares each external engine's codewords, mapped into gf2's "
        "user coordinates, with gf2's on every code class and batch size. Each timed pair's "
        f"candidate also reports `{EQUAL_CODEWORDS}` for its own messages before timing; the "
        "last column counts those pairs per measured cell.", "",
        *shared.table(["Engine", "Workload", "Size", "Equivalence run", "Timed pairs reporting "
                       "equal codewords"], rows), ""]


def consumed_section():
    workload_path = live("1362381c/workload-set.json")
    drift_path = live("1362381c/production-drift.json")
    workload, drift = read(workload_path), read(drift_path)
    classes = {entry["receipt"]: row for row in drift["rows"] for entry in row["receipts"]}
    integrated = {family["family"] for family in FAMILIES}
    cells = [cell for cell in workload["cells"]
             if cell["primary"] and cell["family"] not in integrated]
    missing = sorted({cell["receipt"] for cell in cells} - classes.keys())
    if missing:
        raise SystemExit(f"receipts without a drift row: {missing}")
    counts = collections.Counter(classes[cell["receipt"]]["class"] for cell in cells)
    rows = [[cell["domain"], f"`{cell['family']}`", f"`{cell['cell']}`",
             anchor(ROOT / cell["receipt"], pathlib.Path(cell["receipt"]).parent.name),
             f"`{classes[cell['receipt']]['class']}`", classes[cell["receipt"]]["basis"]]
            for cell in cells]
    return [
        "## Consumed primary cells", "",
        f"Every primary cell of {anchor(workload_path)} outside FI-3 and FI-4, with the class "
        f"{anchor(drift_path)} gives its receipt ("
        + ", ".join(f"{count} `{name}`" for name, count in sorted(counts.items())) + "). "
        "Each row is consumed by identity: the receipt and its digest in the workload set are "
        "evidence of the code that receipt pins, never a measurement of the current tree.", "",
        *(f"- `{name}`: {meaning}." for name, meaning in drift["receipt_classes"].items()), "",
        *shared.table(["Domain", "Family", "Cell", "Receipt", "Drift class", "Basis"], rows), ""]


def render():
    lines = [
        "# Comparator gaps on the integrated tree", "",
        "> **Diátaxis Type:** Reference", "",
        f"Generated by {anchor(pathlib.Path(__file__), pathlib.Path(__file__).name)} from the "
        "committed receipts, acceptance summaries, plans and pinned addenda named "
        f"below; from the worktree root, `{rel(pathlib.Path(__file__))}` regenerates it.", "",
        "Each cell's estimate is the speedup of medians, gf2 time over external time, with its "
        "percentile-bootstrap interval at the corrected per-comparison confidence. Values above "
        "1 favour the external arm. `improved` (`pass`) records a material gap in the external "
        "arm's favour, `regressed` (`fail`) records gf2 faster beyond the equivalence margin.", ""]
    for family in FAMILIES:
        lines += family_section(family)
    lines += consumed_section()
    return "\n".join(lines)


def main():
    if sys.argv[1:] not in ([], ["--check"]):
        sys.exit(__doc__)
    if pathlib.Path.cwd() != ROOT:
        sys.exit("invoke from the worktree root")
    text = render()
    if sys.argv[1:] == ["--check"]:
        if not OUTPUT.is_file() or OUTPUT.read_text(encoding="utf-8") != text:
            sys.exit(f"projection differs: {rel(OUTPUT)}")
        return
    shared.write(OUTPUT, text)
    print(rel(OUTPUT))


if __name__ == "__main__":
    main()
