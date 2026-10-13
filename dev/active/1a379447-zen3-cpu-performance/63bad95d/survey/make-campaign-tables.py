#!/usr/bin/env python3
"""Tabulate what the core calibration campaigns publish per selector (jit:63bad95d).

Reads every committed extent-campaign receipt of the core producer, the owner
envelope each one publishes, the conservative owner and this issue's campaign
declaration. Writes `campaign-tables.md` into the issue directory. The campaign
this issue declares is the subject; the others are its comparison.

A sweep point's margin is `(conservative - asymptotic) / conservative` over the
two arm medians and its band the larger of the two relative interquartile
ranges, as the producer's retained crossover rule computes them.

Usage: make-campaign-tables.py, with no arguments
"""

import hashlib
import json
import re

from locate import HERE, ROOT, tracked

ISSUE = HERE.parent
SECTION = "gf2-core/selectors"
RECEIPT = re.compile(r"^(?P<dir>.*)/(?P<campaign>gf2-(?P<issue>[0-9a-f]{8})-\d{8}t\d{6}z-\d+)\.md$")
DECISIONS = re.compile(r"```json\n(.*?)\n```", re.S)


def campaigns():
    """Committed campaign receipts that embed owner decisions, oldest first.

    Byte copies of one receipt stand for its campaign once, under the last of
    their paths in sorted order.
    """
    found, seen = [], {}
    for path in reversed(tracked("gf2-*.md")):
        match = RECEIPT.match(path)
        if not match:
            continue
        content = (ROOT / path).read_bytes()
        if match["campaign"] in seen:
            if seen[match["campaign"]] != content:
                raise SystemExit(f"{path} differs from another receipt of its campaign")
            continue
        seen[match["campaign"]] = content
        block = DECISIONS.search((ROOT / path).read_text())
        if not block:
            continue
        owners = tracked(f"gf2-core/**/{match['campaign']}.json")
        if not owners:
            # A campaign of another producer publishes no core owner.
            continue
        if len(owners) != 1:
            raise SystemExit(f"{match['campaign']} has {len(owners)} committed core owners")
        owner_bytes = (ROOT / owners[0]).read_bytes()
        owner_sha256 = hashlib.sha256(owner_bytes).hexdigest()
        projection = [
            entry for entry in json.loads(block.group(1))
            if entry["operation"] == "emit-owner" and entry["artifact"]["sha256"] == owner_sha256
        ]
        if len(projection) != 1:
            raise SystemExit(f"{path} projects {owners[0]} {len(projection)} times")
        found.append(
            {
                "campaign": match["campaign"],
                "issue": match["issue"],
                "receipt": path,
                "owner": owners[0],
                "owner_sha256": owner_sha256,
                "envelope": json.loads(owner_bytes),
                "decisions": json.loads(projection[0]["decisions"]),
            }
        )
    return sorted(found, key=lambda entry: entry["campaign"].split("-", 2)[2])


def leaves(envelope):
    groups = envelope["sections"][SECTION]["selectors"]
    return {f"{group}.{field}": value for group, fields in groups.items()
            for field, value in fields.items()}


def cell(value):
    return "omitted" if value is None else f"`{value}`"


def fmt(value, digits):
    return f"{value:.{digits}f}"


def selection_text(selection):
    if selection["kind"] == "crossover":
        return f"crossover at `{selection['crossover']}`"
    reason = selection["reason"]
    if reason["kind"] == "non_monotone":
        return (f"default kept: non-monotone (first win `{reason['first_win']}`, "
                f"later loss `{reason['later_loss']}`)")
    return f"default kept: {reason['kind'].replace('_', ' ')}"


def sweep(decisions, field, variant="standard"):
    rows = [entry for entry in decisions["retained_thresholds"]
            if entry["field"] == field and entry["variant"] == variant]
    return rows[0] if rows else None


def sweep_table(lines, entry, low, high):
    lines += [
        f"| Size | {low} median (ns) | {low} rel. IQR | {high} median (ns) | {high} rel. IQR "
        "| Windows per arm | Margin | Band | Margin above band |",
        "|---:|---:|---:|---:|---:|---:|---:|---:|---|",
    ]
    for point in entry["points"]:
        conservative, asymptotic = point["conservative"], point["asymptotic"]
        margin = (conservative["median"] - asymptotic["median"]) / conservative["median"]
        band = max(conservative["spread"], asymptotic["spread"])
        windows = sorted({conservative["sample_count"], asymptotic["sample_count"]})
        lines.append(
            f"| {point['size']} | {fmt(conservative['median'], 3)} "
            f"| {fmt(conservative['spread'], 4)} | {fmt(asymptotic['median'], 3)} "
            f"| {fmt(asymptotic['spread'], 4)} | {'/'.join(map(str, windows))} "
            f"| {fmt(margin, 4)} | {fmt(band, 4)} | {'yes' if margin > band else 'no'} |"
        )
    lines.append("")


def main():
    declaration = json.loads((ISSUE / "campaign-declaration.json").read_text())
    found = campaigns()
    subjects = [entry for entry in found if entry["issue"] == declaration["issue"]]
    if len(subjects) != 1:
        raise SystemExit(f"{len(subjects)} published campaigns of issue {declaration['issue']}")
    subject = subjects[0]
    conservative_path = tracked("gf2-core/**/tuning-profiles/conservative.json")
    if len(conservative_path) != 1:
        raise SystemExit("exactly one conservative core owner must exist")
    conservative = leaves(json.loads((ROOT / conservative_path[0]).read_text()))
    measurement = subject["envelope"]["sections"][SECTION]["measurement"]

    lines = [
        "# Core calibration campaign: published selectors",
        "",
        "> **Diátaxis Type:** Reference",
        "",
        "Generated by `survey/make-campaign-tables.py` from the committed campaign receipts and "
        "owner envelopes. Timing figures are the producer's own statistic: the median and the "
        "relative interquartile range of the timing windows of one arm at one grid point, with "
        "the window count in each row. They are descriptive; the producer's crossover rule "
        "compares the margin with the band and computes no interval.",
        "",
        "## Campaigns",
        "",
        "| Campaign | Receipt | Core owner | Owner SHA-256 | Harness schema | Producer SHA-256 |",
        "|---|---|---|---|---|---|",
    ]
    for entry in found:
        block = entry["envelope"]["sections"][SECTION]["measurement"]
        lines.append(
            f"| `{entry['campaign']}` | `{entry['receipt']}` | `{entry['owner']}` "
            f"| `{entry['owner_sha256']}` | `{block['harness_schema']}` "
            f"| `{block['binary_sha256']}` |"
        )
    lines += [
        "",
        f"The subject of the tables below is `{subject['campaign']}`, produced at source "
        f"revision `{measurement['source_revision']}` by `{measurement['harness']}`.",
        "",
        "## Selectors per owner",
        "",
        "`Differs` names the owners whose value the subject does not repeat.",
        "",
        "| Selector | Conservative | " + " | ".join(f"`{entry['issue']}`" for entry in found)
        + " | Differs |",
        "|---|---:|" + "---:|" * len(found) + "---|",
    ]
    subject_leaves = leaves(subject["envelope"])
    for name in sorted(conservative):
        values = [leaves(entry["envelope"]).get(name) for entry in found]
        own = subject_leaves.get(name)
        differs = []
        if own is not None and own != conservative[name]:
            differs.append("conservative")
        differs += [entry["issue"] for entry, value in zip(found, values)
                    if entry is not subject and value != own]
        lines.append(
            f"| `{name}` | `{conservative[name]}` | " + " | ".join(map(cell, values))
            + f" | {', '.join(differs)} |"
        )

    lines += [
        "",
        "## Retained-threshold decisions",
        "",
        "One row per swept field and variant of the subject, with the decision each earlier "
        "campaign records for the same sweep.",
        "",
        "| Sweep field | Variant | Value | Decision | "
        + " | ".join(f"`{entry['issue']}` decision" for entry in found if entry is not subject)
        + " |",
        "|---|---|---:|---|" + "---|" * (len(found) - 1),
    ]
    for entry in subject["decisions"]["retained_thresholds"]:
        earlier = []
        for other in found:
            if other is subject:
                continue
            row = sweep(other["decisions"], entry["field"], entry["variant"])
            earlier.append(
                "not swept" if row is None
                else f"`{row['selection']['value']}`; {selection_text(row['selection'])}"
            )
        lines.append(
            f"| `{entry['field']}` | {entry['variant']} | `{entry['selection']['value']}` "
            f"| {selection_text(entry['selection'])} | " + " | ".join(earlier) + " |"
        )

    lines += [
        "",
        "## Extent and joint decisions",
        "",
        "| Field | Reason | Grid boundary limited |",
        "|---|---|---|",
    ]
    decisions = subject["decisions"]
    for entry in decisions["extents"]:
        decision = entry["decision"]
        lines.append(
            f"| `{entry['field']}` | {decision['reason']} | {decision['grid_boundary_limited']} |"
        )
    lines.append(f"| `gemm` (row and column tiles) | {decisions['gemm']['reason']} | "
                 f"{decisions['gemm']['grid_boundary_limited']} |")
    lines.append(f"| `joint_m4rm` | {decisions['joint_m4rm']['reason']} |  |")

    lines += ["", "## Matvec lane sweep", ""]
    matvec = sweep(decisions, "matvec_simd_min_words")
    lines += [
        f"`bit_matrix.matvec_simd_min_words`, `{subject['campaign']}`: "
        f"{selection_text(matvec['selection'])}; published value "
        f"`{matvec['selection']['value']}`. Size is the row stride in words.",
        "",
    ]
    sweep_table(lines, matvec, "Scalar lane", "SIMD lane")

    lines += ["## Bit-backend threshold sweep", ""]
    for entry in found:
        row = sweep(entry["decisions"], "simd_min_words")
        if row is None:
            continue
        lines += [
            f"`bit_backend.simd_min_words`, `{entry['campaign']}`: "
            f"{selection_text(row['selection'])}; published value "
            f"`{row['selection']['value']}`. Size is the buffer length in words.",
            "",
        ]
        sweep_table(lines, row, "Scalar backend", "SIMD backend")

    lines += ["## PLE scalar base sweep", ""]
    for entry in found:
        row = sweep(entry["decisions"], "ple_scalar_base_max_cols")
        if row is None:
            continue
        lines += [
            f"`ple.scalar_base_max_cols`, `{entry['campaign']}`: "
            f"{selection_text(row['selection'])}; published value "
            f"`{row['selection']['value']}`.",
            "",
        ]
        sweep_table(lines, row, "Conservative arm", "Asymptotic arm")

    output = ISSUE / "campaign-tables.md"
    output.write_text("\n".join(lines).rstrip("\n") + "\n")
    print(f"{output.relative_to(ROOT)}: {len(found)} campaigns, subject {subject['campaign']}")


if __name__ == "__main__":
    main()
