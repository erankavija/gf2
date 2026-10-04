"""The frozen pilot-resolution rule the comparator renderers share.

A family's resolution is the largest relative bootstrap half-width over its
confirmatory cells at the first confirmation's corrected alpha, rounded upward
to the next 0.001 and compared with the ceiling its prose addendum fixes.
"""

import hashlib
import json
import math
import re
import subprocess

from repo_artifacts import ROOT


def accepted_pilot(pilot):
    """The receipt bytes and decoded record of an accepted, digest-matched pilot."""
    receipt = (ROOT / pilot / "receipt.json").read_bytes()
    summary = json.loads((ROOT / pilot / "acceptance-summary.json").read_bytes())
    if (
        summary["receipt_sha256"] != hashlib.sha256(receipt).hexdigest()
        or summary["verdict"] != "accepted"
        or summary["label"] != "pilot"
    ):
        raise ValueError(f"{pilot} is not an accepted, digest-matched pilot")
    return receipt, json.loads(receipt)


def half_widths(survey_analysis, pilot, alpha):
    """Every cell's half-width from `survey-analysis resolution` at `alpha`."""
    command = [survey_analysis, "resolution", str(pilot), format(alpha, ".12g")]
    raw = subprocess.check_output(command, cwd=ROOT, text=True)
    widths = {}
    for line in raw.splitlines():
        match = re.fullmatch(r"stricter alpha\s+\S+\s+(\S+)\s+([0-9.]+)", line)
        if match:
            widths[match.group(1)] = float(match.group(2))
    return widths


def rounded(width):
    """The frozen upward rounding to the next 0.001."""
    return math.ceil(width * 1000) / 1000


def ceiling(rules, family):
    """The family's entry in the prose addendum's `Resolution ceiling` column."""
    header = None
    for line in (ROOT / rules).read_text().splitlines():
        columns = [column.strip() for column in line.split("|")]
        if "Resolution ceiling" in columns:
            header = columns
        elif header and len(columns) == len(header) and columns[1] == family:
            return float(columns[header.index("Resolution ceiling")])
        elif not line.startswith("|"):
            header = None
    raise ValueError(f"{rules} fixes no resolution ceiling for {family}")
