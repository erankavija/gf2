"""Comparison arithmetic the lane-threshold generators share (jit:4337c02e).

The resample count, the P-20 tail rule and the attempt allocation are read
from the campaign-support sources, the reservations from the family ledger and
the confirmatory cells from the harness's own listing (`dense-campaign list`).
"""

import json
import re
import subprocess

from locate import FAMILY, ROOT, repository_files

TAIL_RULE = re.compile(
    r"f64::from\(settings\.bootstrap_resamples\) \* corrected_alpha / 2\.0 < (\d+)\.0;"
)
ATTEMPT_RULE = "    Ok(family.family_wise.alpha / (attempts * (attempts + 1.0)))"


def protocol_rules():
    """Bootstrap resamples and the expected draws P-20 requires in each tail."""
    support = ROOT / repository_files.package_directory(ROOT, "tuning-campaign-support") / "src"
    resamples = re.search(
        r"^    bootstrap_resamples: ([\d_]+),$", (support / "protocol.rs").read_text(), re.M
    )
    tail = TAIL_RULE.search((support / "receipt.rs").read_text())
    if not resamples or not tail or ATTEMPT_RULE not in (support / "trial_ledger.rs").read_text():
        raise SystemExit("the campaign-support sources no longer carry the rules this table reads")
    return int(resamples.group(1).replace("_", "")), int(tail.group(1))


def listing(tool):
    rows = {}
    raw = subprocess.check_output([tool, "list", "--family", FAMILY], cwd=ROOT, text=True)
    for line in raw.splitlines():
        ordinal, cell_id, *fields = line.split()
        rows[cell_id] = dict(field.split("=", 1) for field in fields) | {"ordinal": ordinal}
    return rows


def budget(addendum, confirmatory, through=None):
    """The next confirmation's comparison budget for `confirmatory` cells.

    `through` names the campaign whose ledger line ends the chain read; without
    it the whole ledger counts.
    """
    ledger = ROOT / addendum["family_wise"]["ledger_path"]
    entries = [json.loads(line) for line in ledger.read_text().splitlines() if line]
    if through is not None:
        campaigns = [entry["campaign"] for entry in entries]
        entries = entries[: campaigns.index(through) + 1]
    spending = [entry for entry in entries if entry["comparisons"] > 0]
    attempt = len(spending) + 1
    reserved = sum(entry["comparisons"] for entry in spending)
    attempt_alpha = addendum["family_wise"]["alpha"] / (attempt * (attempt + 1))
    resamples, tail_draws = protocol_rules()
    corrected = attempt_alpha / (reserved + confirmatory)
    return {
        "entries": len(entries),
        "spending": len(spending),
        "attempt": attempt,
        "comparisons": reserved + confirmatory,
        "attempt_alpha": attempt_alpha,
        "corrected": corrected,
        "resamples": resamples,
        "tail_draws": tail_draws,
        "draws": resamples * corrected / 2,
        "capacity": int(resamples * attempt_alpha / (2 * tail_draws)) - reserved,
    }
