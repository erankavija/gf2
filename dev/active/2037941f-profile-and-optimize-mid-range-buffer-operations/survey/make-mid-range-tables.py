#!/usr/bin/env python3
"""Project the mid-range story's committed evidence into its publication tables.

Every interval, decision and outcome is the committed acceptance summary's; every
identity is read from a receipt or hashed from committed bytes. A receipt is
admitted only when its summary pins it, its input snapshots match their pins and
its execution log passes the shared log verifier. Nothing here reads the working
tree's production sources, so later source edits leave the tables unchanged.

Usage: make-mid-range-tables.py, with no arguments
"""

import collections
import hashlib
import importlib.util
import json
import math
import os
import pathlib
import re

from repo_artifacts import ROOT, containing, ledger, receipt, tracked
from resolution_rule import ceiling as family_ceiling

OUTPUT = "mid-range-tables.md"
STORY = tracked("logical-buffer-addendum.md").parent

# Campaigns in reading order: each family's identity baseline, then its
# candidate or comparator stages.
CAMPAIGNS = (
    ("Logical buffer", "v4-r1-2037941f-logical-isolated-xor"),
    ("Logical buffer", "bc091474-u2-2037941f-logical-isolated-xor"),
    ("Logical buffer", "bc091474-u4-2037941f-logical-isolated-xor"),
    ("Logical buffer", "v4-r1-2037941f-logical-public-row-xor"),
    ("Logical buffer", "bc091474-u2-2037941f-logical-public-row-xor"),
    ("Logical buffer", "bc091474-u4-2037941f-logical-public-row-xor"),
    ("Logical buffer", "v4-r1-2037941f-logical-nr-construction"),
    ("Logical buffer", "v4-r1-2037941f-logical-isal-base-gap"),
    ("Dense parity", "v4-r1-2037941f-dense-isolated-fused-parity"),
    ("Dense parity", "v4-r1-2037941f-dense-allocated-matvec"),
    ("Dense parity", "v4-r1-2037941f-dense-matvec-vs-m4ri"),
    ("Dense parity", "v4-r1-confirmation-2037941f-dense-matvec-vs-m4ri"),
)
M4RI_CAMPAIGNS = tuple(name for _, name in CAMPAIGNS if name.endswith("dense-matvec-vs-m4ri"))
M4RI_CONFIRMATION = "v4-r1-confirmation-2037941f-dense-matvec-vs-m4ri"

# Generated stopping records and the token each states its outcome with.
OUTCOME = re.compile(r"\*\*Outcome: `([a-z-]+)`\.\*\*")
STOPPING_RECORDS = (
    ("ISA-L scalar gap", "isal-base-gap-outcome.md"),
    ("M4RI comparator", "m4ri-gap-resolution.md"),
)

# Profile sessions: the summary's distinguishing line, and the journal beside it.
PROFILES = (
    ("Logical baseline", "under `logical-profile/rep-*`", "repetitions.log"),
    ("Unroll factor 2", "under `profile-unroll2/rep-*`", "repetitions.log"),
    ("Unroll factor 4", "under `profile-unroll4/rep-*`", "repetitions.log"),
    ("Dense baseline", "# Dense baseline profile attribution", "execution.log"),
)

# Annotated release disassembly: a tracked name, or a name with the header line
# that identifies the listing.
LISTINGS = (
    ("Logical campaign arm", ("index.txt", "# binary: logical-arm")),
    ("Logical profile driver", ("index.txt", "# binary: logical-profile")),
    ("Dense arm, `simd` feature", ("index.txt", "e1f9a78f-arms/release/dense-arm")),
    ("Dense arm, scalar build", ("index.txt", "e1f9a78f-scalar-arm/release/dense-arm")),
    ("AVX2 XOR body, baseline", "baseline-avx2-xor.asm.txt"),
    ("AVX2 XOR body, factor 2", "unroll2-avx2-xor.asm.txt"),
    ("AVX2 XOR body, factor 4", "unroll4-avx2-xor.asm.txt"),
)
BINARY_DIGEST = re.compile(r"^# (?:binary )?sha256: ([0-9a-f]{64})$", re.M)

# The request digest the rejected dense profile launch journals.
FAILED_PROFILE_REQUEST = "request=5059d6056c63d6f270fe246457f11cc6a5f54b0e8f5a7dc4f566abb7b987647a"

# Story criterion, locator and role of each disposition artifact. A locator is a
# unique tracked name, a (name, content line) pair, or a callable returning the
# root-relative path.
ARTIFACTS = (
    ("REQ-01", "logical-buffer-addendum.md", "frozen logical-buffer addendum with its amendments"),
    ("REQ-01", "dense-parity-addendum.md", "frozen dense-parity addendum, version 2"),
    ("REQ-01", "dense-parity-addendum-v1.md", "dense-parity addendum, version 1 bytes"),
    ("REQ-01", "dense-parity-amendment-v2.md", "approved unavailable-row amendment"),
    ("REQ-01", lambda: STORY / "investigation.md", "production consumer sweep"),
    ("REQ-01", "logical-baselines.md", "logical baseline record"),
    ("REQ-01", "logical-tables.md", "logical baseline interval and path tables"),
    ("REQ-01", "dense-baseline-results.md", "dense baseline record"),
    ("REQ-01", "dense-baseline-preparation.md", "dense baseline source and assembly record"),
    ("REQ-02", "portfolio.md", "frozen logical candidate portfolio"),
    ("REQ-02", "readiness.md", "candidate build, oracle and smoke record"),
    ("REQ-02", "candidate-tables.md", "candidate pilot tables and frozen screen"),
    ("REQ-02", "pilot-outcome.md", "logical candidate selection outcome"),
    ("REQ-02", ("no-change-outcome.md", "(jit:fcb04d66)"), "logical production no-adoption verdict"),
    ("REQ-02", ("outcome.md", "(jit:a1ad6d4e)"), "BitSlice zero-copy no-candidate outcome"),
    ("REQ-02", ("source-evidence.json", '"callsite_search"'), "BitSlice source and callsite ledger"),
    ("REQ-02", ("outcome.md", "(jit:c73ffa25)"), "dense baseline and fusion portfolio outcome"),
    ("REQ-02", ("outcome.md", "(jit:50f0bd42)"), "dense-parity decision record"),
    ("REQ-02", ("no-change-outcome.md", "(jit:6e87c436)"), "dense production no-change verdict"),
    ("REQ-03", "isal-comparator.md", "ISA-L comparator qualification"),
    ("REQ-03", "isal-probe-record.txt", "ISA-L semantic probe record"),
    ("REQ-03", "isal-arm-build-record.txt", "ISA-L arm build provenance"),
    ("REQ-03", "isal-base-gap-results.md", "ISA-L comparison record"),
    ("REQ-03", "isal-base-gap-tables.md", "ISA-L interval, path and setup-cost tables"),
    ("REQ-03", "isal-base-gap-outcome.md", "ISA-L stopping result"),
    ("REQ-03", "isal-dispatch-availability.md", "ISA-L unavailable and inapplicable routes"),
    ("REQ-03", "isal-sampling-discrepancy.md", "ISA-PILOT-6 discrepancy and exception"),
    ("REQ-03", "m4ri-operation-match.md", "M4RI matched-operation qualification"),
    ("REQ-03", "m4ri-probe-record.txt", "M4RI semantic probe record"),
    ("REQ-03", "m4ri-gap-tables.md", "M4RI interval, path and setup-cost tables"),
    ("REQ-03", "m4ri-gap-resolution.md", "M4RI pilot resolution record"),
    ("REQ-03", "dense-matvec-vs-m4ri-confirmation-derivation.txt", "M4RI confirmation derivation"),
    ("REQ-04", lambda: STORY / "plan.md", "canonical buffer and external comparator contracts"),
    ("REQ-04", "logical-harness-validation.txt", "logical semantic oracle record"),
    ("REQ-04", "dense-harness-validation.txt", "dense and M4RI semantic oracle record"),
    ("REQ-04", "dense-baseline-validation.txt", "dense baseline semantic validation"),
    ("REQ-04", ("verification.md", "(jit:fcb04d66)"), "logical shared-suite verification"),
    ("REQ-04", ("source-evidence.json", '"issue": "fcb04d66"'), "logical dispatch and kernel source ledger"),
    ("REQ-04", ("verification.md", "(jit:6e87c436)"), "dense shared-suite and route verification"),
    ("REQ-04", ("source-evidence.json", '"issue": "6e87c436"'), "dense selection and kernel source ledger"),
    ("REQ-04", "production-drift.json", "measured-versus-recorded production source classes"),
    ("REQ-04", "route-comparison.json", "measured and re-observed path per dense cell arm"),
    ("REQ-04", ("asm-comparison.json", '"issue": "6e87c436"'), "per-symbol instruction text of the AVX2 module"),
    ("REQ-05", lambda: STORY / "breakdown.json", "authoritative breakdown manifest"),
    ("REQ-05", lambda: STORY / "review.md", "adversarial plan review"),
    ("REQ-05", "profile-provenance.md", "candidate profile invocation and fixture identity"),
    ("REQ-05", ("execution.log", FAILED_PROFILE_REQUEST), "preserved failed dense profile launch"),
    ("REQ-06", "logical-harness.md", "logical harness interface"),
    ("REQ-06", "dense-parity-harness.md", "dense-parity harness interface"),
    ("REQ-06", "dense-parity-conformance.md", "dense harness clause conformance"),
    ("REQ-06", "logical-producing-inputs.json", "logical producing-input closure"),
    ("REQ-06", "dense-producing-inputs.json", "dense producing-input closure"),
    ("REQ-06", "logical-source-evidence.json", "logical harness source ledger"),
)


def locate(locator):
    """The root-relative path a locator names."""
    if callable(locator):
        return locator()
    if isinstance(locator, tuple):
        return containing(*locator)
    return tracked(locator)


def digest(relative):
    return hashlib.sha256((ROOT / relative).read_bytes()).hexdigest()


def load(relative):
    return json.loads((ROOT / relative).read_bytes())


def code(value):
    return f"`{value}`"


def link(relative):
    """A link from the story directory to a root-relative path."""
    return f"[{code(relative)}]({os.path.relpath(relative, STORY)})"


def cell_text(value):
    return str(value).replace("|", "\\|")


def decimals(estimate):
    """Four decimals, or the count giving the estimate four significant digits below one tenth."""
    return 4 if estimate >= 0.1 else 3 - math.floor(math.log10(estimate))


def ratio(interval):
    """`estimate [lower, upper]` at one precision."""
    places = decimals(interval["estimate"])
    return (
        f"{interval['estimate']:.{places}f} [{interval['lower']:.{places}f}, "
        f"{interval['upper']:.{places}f}]"
    )


def half_width(interval):
    """The larger endpoint distance relative to the estimate, as the canonical tables define it."""
    estimate = interval["estimate"]
    return max(abs(estimate - interval["lower"]), abs(interval["upper"] - estimate)) / estimate


def tally(values):
    counts = collections.Counter(values)
    return ", ".join(f"{name} {counts[name]}" for name in sorted(counts))


def log_verifier():
    spec = importlib.util.spec_from_file_location("campaign_log", ROOT / tracked("verify-campaign-log.py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.verify


class Campaign:
    """One accepted receipt with its summary, frozen addendum and verified pins."""

    def __init__(self, family, campaign_id, verify):
        self.family = family
        self.directory = receipt(campaign_id)
        self.receipt_path = self.directory / "receipt.json"
        self.record = load(self.receipt_path)
        self.summary = load(self.directory / "acceptance-summary.json")
        self.receipt_sha256 = digest(self.receipt_path)
        if self.summary["receipt_sha256"] != self.receipt_sha256:
            raise ValueError(f"{campaign_id}: the acceptance summary does not pin its receipt")
        if self.summary["verdict"] != "accepted":
            raise ValueError(f"{campaign_id}: the receipt is not accepted")
        for kind in ("protocol", "contract", "addendum", "trial_ledger"):
            pin = self.record[kind]
            if digest(self.directory / pin["snapshot"]) != pin["sha256"]:
                raise ValueError(f"{campaign_id}: the {kind} snapshot differs from its pin")
        journal = self.record["execution_log"]
        if digest(self.directory / journal["path"]) != journal["sha256"]:
            raise ValueError(f"{campaign_id}: the execution log differs from its pin")
        verify(
            str(ROOT / self.directory / journal["path"]),
            str(ROOT / self.receipt_path),
            str(ROOT / self.directory / "plan.json"),
        )
        self.addendum = load(self.directory / self.record["addendum"]["snapshot"])
        self.declared = {cell["cell_id"]: cell for cell in self.addendum["cells"]}
        self.measured = {cell["cell_id"]: cell for cell in self.record["cells"]}
        self.cells = self.summary["cells"]
        if {cell["cell_id"] for cell in self.cells} != set(self.measured) or set(self.measured) != set(self.declared):
            raise ValueError(f"{campaign_id}: receipt, summary and addendum cells differ")
        for cell in self.cells:
            if cell["pairs"] != len(self.measured[cell["cell_id"]]["pairs"]):
                raise ValueError(f"{campaign_id}: pair count differs for {cell['cell_id']}")

    @property
    def campaign_id(self):
        return self.record["campaign_id"]

    def hosts(self):
        return sorted(
            {f"{host['hostname']}, {host['cpu_model']}, {host['os_kernel']}" for host in self.record["session_hosts"]}
        )


def authority(campaigns):
    lines = [
        "## Receipt authority",
        "",
        "Each receipt SHA-256 equals the digest its acceptance summary pins. The protocol, "
        "contract, addendum and ledger snapshots beside each receipt match the digests the "
        "receipt records, each execution log matches its pinned digest, and the shared log "
        "verifier accepts each log against its receipt and plan. Git revisions and the "
        "state of the working tree decide none of this.",
        "",
        "| Campaign | Receipt | Receipt SHA-256 | Label, verdict, qualifies | Findings | Sessions | Cells |",
        "|---|---|---|---|---:|---:|---:|",
    ]
    for item in campaigns:
        summary = item.summary
        lines.append(
            f"| {code(item.campaign_id)} | {link(item.receipt_path)} | {code(item.receipt_sha256)} | "
            f"{summary['label']}, {summary['verdict']}, {str(summary['qualifies']).lower()} | "
            f"{len(summary['findings'])} | {summary['sessions']} | {len(item.cells)} |"
        )
    lines += [
        "",
        "### Frozen inputs",
        "",
        "Protocol, contract and campaign addendum as each receipt pins them, with the "
        "protocol version its addendum snapshot declares and the family's comparison count "
        "and per-comparison confidence from the acceptance summary.",
        "",
        "| Campaign | Protocol | Protocol SHA-256 | Contract SHA-256 | Addendum SHA-256 | Comparisons | Confidence | Resamples |",
        "|---|---|---|---|---|---:|---:|---:|",
    ]
    for item in campaigns:
        family = item.summary["family"]
        lines.append(
            f"| {code(item.campaign_id)} | {code(item.record['protocol']['path'])} "
            f"v{item.addendum['protocol']['version']} | {code(item.record['protocol']['sha256'])} | "
            f"{code(item.record['contract']['sha256'])} | {code(item.record['addendum']['sha256'])} | "
            f"{family['comparisons']} | {family['per_comparison_confidence']} | {family['bootstrap_resamples']} |"
        )
    lines += [
        "",
        "### Host, toolchain and producing identity",
        "",
        "Host and kernel are the receipt's per-session runtime observations; the toolchain "
        "is the receipt's. The manifest digest pins the producing-source closure whose "
        "per-file digests the receipt carries.",
        "",
        "| Campaign | Host, CPU, kernel | Toolchain | Producing manifest SHA-256 |",
        "|---|---|---|---|",
    ]
    for item in campaigns:
        lines.append(
            f"| {code(item.campaign_id)} | {'; '.join(item.hosts())} | {code(item.record['toolchain'])} | "
            f"{code(item.record['source']['producing']['manifest_sha256'])} |"
        )
    generator = str(tracked("abtest.rs"))
    lines += [
        "",
        "### Seeds, generator and invocation",
        "",
        "The receipt records the campaign seed, the addendum snapshot each cell's workload "
        "seed and the acceptance summary each interval's bootstrap seed. The generator "
        f"source {code(generator)} defines SplitMix64 [Vigna2015] and xoshiro256** "
        "[BlackmanVigna2021]; its digest is the one each receipt pins and snapshots. The "
        "launcher log opens with the executed command; a companion holds the runner "
        "invocations where the campaign kept one.",
        "",
        "| Campaign | Campaign seed | Pinned generator source SHA-256 | Launcher log | Launcher log SHA-256 | Invocation companion |",
        "|---|---:|---|---|---|---|",
    ]
    for item in campaigns:
        producing = item.record["source"]["producing"]
        pinned = producing["behavior_sha256"][generator]
        if digest(item.directory / "inputs" / "producing" / generator) != pinned:
            raise ValueError(f"{item.campaign_id}: the generator source snapshot differs from its pin")
        launcher = item.directory / "launcher.log"
        companions = [
            item.directory / name
            for name in ("invocations.log", "invocations.md")
            if (ROOT / item.directory / name).is_file()
        ]
        lines.append(
            f"| {code(item.campaign_id)} | {item.record['campaign_seed']} | {code(pinned)} | "
            f"{link(launcher)} | {code(digest(launcher))} | "
            f"{', '.join(link(companion) for companion in companions) or 'none'} |"
        )
    lines += [
        "",
        "### Arm executables",
        "",
        "| Campaign | Arm | Build | Executable SHA-256 | Arm description in the receipt |",
        "|---|---|---|---|---|",
    ]
    for item in campaigns:
        for name, arm in sorted(item.record["arms"].items()):
            lines.append(
                f"| {code(item.campaign_id)} | {code(name)} | {arm['build']} | "
                f"{code(arm['executable_sha256'])} | {cell_text(arm['description'])} |"
            )
    return lines


def campaign_cells(item):
    arms = item.record["arms"]
    identity = sum(
        arms[cell["baseline_arm"]]["executable_sha256"] == arms[cell["candidate_arm"]]["executable_sha256"]
        for cell in item.measured.values()
    )
    margins = sorted(
        {
            (cell["objective"], json.dumps(cell["margins"], sort_keys=True))
            for cell in item.cells
        }
    )
    unresolved = tally(setting for cell in item.cells for setting in cell["unresolved_settings"])
    lines = [
        f"### {code(item.campaign_id)}",
        "",
        f"Family {code(item.record['family_id'])}, label {item.summary['label']}; "
        f"{len(item.cells)} cells, of which {identity} compare one executable with itself. "
        f"Decisions: {tally(cell['decision'] for cell in item.cells)}. "
        f"Outcomes: {tally(cell['outcome'] for cell in item.cells)}. "
        f"Flagged windows: {sum(cell['flagged_windows'] for cell in item.cells)} of "
        f"{sum(cell['total_windows'] for cell in item.cells)}. "
        f"Margins by objective: {'; '.join(f'{objective} {values}' for objective, values in margins)}. "
        f"Unresolved settings by cell count: {unresolved or 'none'}.",
        "",
        "| Cell | Metric, cache | Role, objective | Baseline arm, compared arm | Pairs | Speedup [interval] | Confidence | Half-width | Decision | Outcome |",
        "|---|---|---|---|---:|---|---:|---:|---|---|",
    ]
    for cell in item.cells:
        declared = item.declared[cell["cell_id"]]
        measured = item.measured[cell["cell_id"]]
        interval = cell["interval"]
        lines.append(
            f"| {code(cell['cell_id'])} | {declared['metric_kind']}, {declared['cache_state']} | "
            f"{cell['role']}, {cell['objective']} | {code(measured['baseline_arm'])}, "
            f"{code(measured['candidate_arm'])} | "
            f"{cell['pairs']} | {ratio(interval)} | {interval['confidence']} | {half_width(interval):.4f} | "
            f"{cell['decision']} | {cell['outcome']} |"
        )
    return lines


def cells(campaigns):
    every = [cell for item in campaigns for cell in item.cells]
    # The findings state that the story adopts nothing; either condition falsifies that.
    if any(cell["decision"] == "improved" for cell in every) or any(item.summary["qualifies"] for item in campaigns):
        raise ValueError("a campaign records an improved cell or qualifies; the findings no longer hold")
    lines = [
        "## Cells",
        "",
        "Speedup is the acceptance summary's ratio of the baseline arm's median to the "
        "compared arm's, with its bootstrap interval at the confidence shown; above one "
        "favours the compared arm. In a comparator family the compared arm is the external "
        "library. Half-width is the larger distance from the estimate to an endpoint, "
        "relative to the estimate. Metric is the frozen addendum's cost boundary of the "
        "cell: `kernel-isolated` or `whole-consumer`. Decision and outcome are the "
        "evaluator's, unchanged. Host, toolchain and executables of each campaign are in "
        "the receipt authority tables.",
        "",
        f"Across all campaigns: {len(every)} cells; decisions: "
        f"{tally(cell['decision'] for cell in every)}; outcomes: "
        f"{tally(cell['outcome'] for cell in every)}; roles: {tally(cell['role'] for cell in every)}; "
        f"cells by pair count: {tally(str(cell['pairs']) + ' pairs' for cell in every)}.",
    ]
    for family in dict.fromkeys(item.family for item in campaigns):
        lines += ["", f"## {family} cells"]
        for item in campaigns:
            if item.family == family:
                lines += [""] + campaign_cells(item)
    return lines


def comparator_rule(item):
    rules = tracked("dense-parity-addendum.md")
    limit = family_ceiling(rules, "M4RI comparator")
    effect = item.addendum["effect"]
    resolution = effect["measurement_resolution"]
    evidence = effect["resolution_evidence"]
    if digest(evidence["receipt"]) != evidence["sha256"]:
        raise ValueError("the confirmation's resolution evidence differs from its pin")
    lines = [
        "## M4RI confirmation against its frozen rule",
        "",
        f"The confirmation addendum pins resolution {resolution:.3f} from the pilot receipt "
        f"{code(evidence['receipt'])} (SHA-256 {code(evidence['sha256'])}); {link(rules)} fixes "
        f"the family ceiling {limit:.3f}. The reciprocal equivalence margin is the bound "
        "below which the external whole consumer is slower than gf2 beyond equivalence. "
        "A half-width above the pilot-derived resolution is recorded as measured.",
        "",
        "| Confirmatory cell | Pairs | Interval upper bound | Reciprocal equivalence margin | Upper bound below it | Half-width | Above pilot resolution | Within family ceiling | Decision | Outcome |",
        "|---|---:|---:|---:|---|---:|---|---|---|---|",
    ]
    for cell in item.cells:
        interval = cell["interval"]
        reciprocal = 1.0 / cell["margins"]["equivalence"]
        width = half_width(interval)
        lines.append(
            f"| {code(cell['cell_id'])} | {cell['pairs']} | "
            f"{interval['upper']:.{decimals(interval['estimate'])}f} | "
            f"{reciprocal:.4f} | {'yes' if interval['upper'] < reciprocal else 'no'} | {width:.4f} | "
            f"{'yes' if width > resolution else 'no'} | {'yes' if width <= limit else 'no'} | "
            f"{cell['decision']} | {cell['outcome']} |"
        )
    return lines


def unavailable(campaigns):
    lines = [
        "## Unavailable and inapplicable rows",
        "",
        "Rows with no arm, no sample and no comparison, as their generated companions record them.",
        "",
        "| Companion | Row | Status | Samples | Comparisons | Reason |",
        "|---|---|---|---:|---:|---|",
    ]
    for item in campaigns:
        if item.campaign_id not in M4RI_CAMPAIGNS:
            continue
        companion = item.directory / "unavailable-rows.tsv"
        rows = [row for row in (ROOT / companion).read_text().splitlines() if not row.startswith("#")]
        header = rows[0].split("\t")
        for row in rows[1:]:
            fields = dict(zip(header, row.split("\t")))
            lines.append(
                f"| {link(companion)} | {code(fields['cell_id'])}, stride {fields['stride_words']} words | "
                f"{fields['status']} | {fields['samples']} | {fields['comparisons']} | {cell_text(fields['reason'])} |"
            )
    companion = tracked("isal-dispatch-availability.md")
    for row in (ROOT / companion).read_text().splitlines():
        fields = [field.strip() for field in row.strip("|").split("|")]
        if row.startswith("| `") and len(fields) == 4:
            lines.append(f"| {link(companion)} | {fields[0]} | {fields[1]} | {fields[2]} | 0 | {fields[3]} |")
    return lines


def stopping(campaigns):
    lines = [
        "## Stopping records and reservations",
        "",
        "| Family | Generated record | SHA-256 | Recorded outcome |",
        "|---|---|---|---|",
    ]
    for family, name in STOPPING_RECORDS:
        record = tracked(name)
        outcomes = OUTCOME.findall((ROOT / record).read_text())
        if len(outcomes) != 1:
            raise ValueError(f"{record} states {len(outcomes)} outcomes rather than one")
        lines.append(f"| {family} | {link(record)} | {code(digest(record))} | {code(outcomes[0])} |")
    screen = tracked("candidate-tables.md")
    rows = [
        [field.strip() for field in row.strip("|").split("|")]
        for row in (ROOT / screen).read_text().split("## Frozen pilot screen")[1].split("##")[0].splitlines()
        if re.match(r"\| \d+ \|", row)
    ]
    eligible = sum(row[-1] == "yes" for row in rows)
    lines.append(
        f"| Logical unroll candidates | {link(screen)} | {code(digest(screen))} | "
        f"{eligible} of {len(rows)} factor-band pairs eligible |"
    )
    lines += [
        "",
        "Each family's append-only trial ledger, by line. A line with zero comparisons is "
        "an exploratory reservation.",
        "",
        "| Ledger | SHA-256 | Sequence | Campaign | Reserved comparisons |",
        "|---|---|---:|---|---:|",
    ]
    for path in dict.fromkeys(ledger(item.campaign_id) for item in campaigns):
        for line in (ROOT / path).read_text().splitlines():
            entry = json.loads(line)
            lines.append(
                f"| {link(path)} | {code(digest(path))} | {entry['sequence']} | "
                f"{code(entry['campaign'])} | {entry['comparisons']} |"
            )
    return lines


def drift():
    record_path = tracked("production-drift.json")
    record = load(record_path)
    lines = [
        "## Measured tree and recorded production tree",
        "",
        f"{link(record_path)} (SHA-256 {code(digest(record_path))}) compares the production "
        "sources the dense receipts pin with the tree its task delivered. The counts are "
        "that record's; this generator reads no production source.",
        "",
        "| Baseline receipts | Class | Files | Changed by the recording task only | Changed by other issues |",
        "|---|---|---:|---:|---:|",
    ]
    for baseline in record["baselines"]:
        receipts = ", ".join(code(entry["campaign_id"]) for entry in baseline["receipts"])
        by_class = collections.defaultdict(list)
        for entry in baseline["files"]:
            by_class[entry["class"]].append(entry)
        if {name: len(entries) for name, entries in by_class.items()} != baseline["class_counts"]:
            raise ValueError("the drift record's class counts differ from its file rows")
        for name in sorted(by_class):
            issues = [{change["issue"] for change in entry.get("changed_by", [])} for entry in by_class[name]]
            own = sum(found == {record["issue"]} for found in issues)
            other = sum(bool(found - {record["issue"]}) for found in issues)
            lines.append(f"| {receipts} | {name} | {len(by_class[name])} | {own} | {other} |")
    routes_path = tracked("route-comparison.json")
    lines += [
        "",
        f"{link(routes_path)} (SHA-256 {code(digest(routes_path))}) joins each measured "
        "pair's reported path with an untimed arm smoke of the same addendum on that tree.",
        "",
        "| Campaign | Cell arms | Differing cell arms | Rows with a different arm executable digest |",
        "|---|---:|---:|---:|",
    ]
    for campaign in load(routes_path)["campaigns"]:
        changed = sum(
            row["measured_executable_sha256"] != row["current_executable_sha256"] for row in campaign["rows"]
        )
        lines.append(
            f"| {code(campaign['campaign_id'])} | {campaign['cell_arms']} | "
            f"{campaign['differing_cell_arms']} | {changed} |"
        )
    return lines


def explanation(campaigns):
    measured = collections.defaultdict(set)
    for item in campaigns:
        for name, arm in item.record["arms"].items():
            measured[arm["executable_sha256"]].add(f"{code(name)} of {code(item.campaign_id)}")
    lines = [
        "## Assembly and profiles",
        "",
        "Each listing names the binary it disassembles by SHA-256; the last column lists the "
        "receipt arms that record the same executable digest.",
        "",
        "| Listing | File | File SHA-256 | Disassembled binary SHA-256 | Receipt arms with that executable |",
        "|---|---|---|---|---|",
    ]
    for label, locator in LISTINGS:
        listing = locate(locator)
        binary = BINARY_DIGEST.search((ROOT / listing).read_text())
        if not binary:
            raise ValueError(f"{listing} names no binary digest")
        arms = "; ".join(sorted(measured[binary.group(1)])) or "none"
        lines.append(f"| {label} | {link(listing)} | {code(digest(listing))} | {code(binary.group(1))} | {arms} |")
    lines += [
        "",
        "Each profile summary states its repetition or pass count and the order-statistic "
        "interval of every figure. Host and toolchain are read from the session's host "
        "record; the last column is the final record of the session journal.",
        "",
        "| Profile | Summary | Summary SHA-256 | Host record SHA-256 | Host CPU | Toolchain | Journal terminal record |",
        "|---|---|---|---|---|---|---|",
    ]
    for label, marker, journal in PROFILES:
        summary = containing("profile-summary.md", marker)
        host = summary.parent / "host.txt"
        text = (ROOT / host).read_text()
        model = re.search(r"^Model name:\s+(.+)$", text, re.M).group(1)
        toolchain = re.search(r"^(rustc \S+ \(.+\))$", text, re.M).group(1)
        last = (ROOT / summary.parent / journal).read_text().splitlines()[-1]
        terminal = " ".join(word for word in last.split() if not re.fullmatch(r"\d{4}-\d\d-\d\dT[\d:]+Z", word))
        lines.append(
            f"| {label} | {link(summary)} | {code(digest(summary))} | {code(digest(host))} | {model} | "
            f"{code(toolchain)} | {code(terminal)} |"
        )
    return lines


def artifacts():
    lines = [
        "## Content-pinned disposition artifacts",
        "",
        "SHA-256 of the committed bytes of each record the findings cite, by the story "
        "criterion it serves first.",
        "",
        "| Story criterion | Artifact | SHA-256 | Role |",
        "|---|---|---|---|",
    ]
    for criterion, locator, role in ARTIFACTS:
        artifact = locate(locator)
        lines.append(f"| {criterion} | {link(artifact)} | {code(digest(artifact))} | {role} |")
    return lines


def main():
    verify = log_verifier()
    campaigns = [Campaign(family, name, verify) for family, name in CAMPAIGNS]
    confirmation = next(item for item in campaigns if item.campaign_id == M4RI_CONFIRMATION)
    generator = pathlib.Path(__file__).resolve().relative_to(ROOT)
    lines = [
        "# Mid-range buffer publication tables",
        "",
        "> **Diátaxis Type:** Reference",
        "",
        f"Generated by {code(generator)} from the committed receipts, acceptance summaries "
        "and records named below; regenerate with "
        f"{code('python3 -B ' + str(generator))}. Raw pairs, windows and logs stay in the "
        "receipt directories.",
        "",
    ]
    for section in (
        authority(campaigns),
        cells(campaigns),
        comparator_rule(confirmation),
        unavailable(campaigns),
        stopping(campaigns),
        drift(),
        explanation(campaigns),
        artifacts(),
    ):
        lines += section + [""]
    (ROOT / STORY / OUTPUT).write_text("\n".join(lines).rstrip("\n") + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
