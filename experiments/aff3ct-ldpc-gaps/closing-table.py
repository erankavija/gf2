#!/usr/bin/env python3
"""Project preserved comparator pilots and prepared quality into a closing table.

This reports the evaluator's intervals without pooling pilots or promoting an
exploratory result to confirmation. Output is restricted to target/.
"""

import argparse
from pathlib import Path

from campaign import HERE, digest, output_path, read


def load_pilot(directory, declaration):
    directory = Path(directory)
    receipt = read(directory / "receipt.json")
    summary = read(directory / "acceptance-summary.json")
    sha = digest(directory / "receipt.json")
    if summary["receipt_sha256"] != sha:
        raise ValueError("acceptance summary evaluates a different receipt")
    family = declaration["family"]["id"]
    if receipt["family_id"] != family or summary["family"]["family_id"] != family:
        raise ValueError("pilot belongs to another comparator family")
    if receipt["campaign_id"] != summary["campaign_id"]:
        raise ValueError("receipt and summary name different campaigns")
    if receipt["label"] != "pilot" or summary["label"] != "pilot" or summary["verdict"] != "accepted":
        raise ValueError("closing evidence must be an accepted pilot")
    expected = {cell["cell_id"] for cell in declaration["cells"]}
    for source in (receipt, summary):
        ids = [cell["cell_id"] for cell in source["cells"]]
        if len(ids) != len(expected) or set(ids) != expected:
            raise ValueError("pilot must cover every declared comparator cell exactly once")
    return receipt, summary, sha


def relationship(interval):
    """Interpret gf2/AFF3CT time, independently of candidate-oriented labels."""
    if interval["upper"] < 1:
        return "gf2 exceeds AFF3CT"
    if interval["lower"] > 1:
        return "gf2 does not meet AFF3CT"
    return "inconclusive against AFF3CT"


def render(primary, historical, quality_dir):
    declaration = read(HERE / "families/55c3e433-adopted-vs-aff3ct.json")
    catalogue = read(HERE / "arms.json")["arms"]
    pilots = [load_pilot(path, declaration) for path in [primary, *historical]]
    qualities = {}
    for code in declaration["codes"]:
        for side in ("baseline", "candidate"):
            arm = declaration["cell"][side]
            path = Path(quality_dir) / f"{catalogue[arm]['quality']}-{code}.json"
            quality = read(path)
            if quality["interval_method"] != "frame-hoeffding-95+fer-wilson-95":
                raise ValueError("quality record must supply the prepared 95% Wilson FER interval")
            qualities[code, side] = (quality, digest(path))

    lines = ["# Integrated LDPC comparator closing table", "",
             f"Family: {declaration['family']['id']}; comparator AFF3CT [Cassagne2019].",
             "Ratio direction throughout: gf2 time / AFF3CT time (ratio of medians).",
             "Below one favors gf2. Evaluator improved favors AFF3CT; regressed favors gf2.",
             "Intervals and evaluator labels are projected from acceptance summaries.",
             "No comparator confirmation ran. All losing-cell closures preserve an experiment.",
             "Historical pilots remain separate; no samples are pooled.", ""]
    for index, (receipt, summary, sha) in enumerate(pilots):
        lines += ["## Freeze-accepted pilot" if index == 0 else (
            "## Historical pilots" if index == 1 else ""), "",
            f"Campaign: {summary['campaign_id']}; host {receipt['host']['hostname']} "
            f"({receipt['host']['cpu_model']}).",
            f"Receipt SHA-256: {sha}.",
            f"Evaluator label: {summary['label']}; verdict: {summary['verdict']}; "
            f"qualifies: {str(summary['qualifies']).lower()}; findings: {len(summary['findings'])}.", "",
            "| Cell | Pairs | gf2 time / AFF3CT time | Interval | Confidence | Evaluator decision / outcome | gf2 comparison | Closure or lead | Receipt SHA-256 |",
            "|---|---:|---:|---|---:|---|---|---|---|"]
        measured = {cell["cell_id"]: cell for cell in receipt["cells"]}
        evaluated = {cell["cell_id"]: cell for cell in summary["cells"]}
        for configured in declaration["cells"]:
            cell_id, code = configured["cell_id"], configured["code"]
            cell, observed = evaluated[cell_id], measured[cell_id]
            if cell["pairs"] != len(observed["pairs"]):
                raise ValueError(f"{cell_id}: summary pair count differs from receipt")
            for side in ("baseline", "candidate"):
                expected_arm = f"{declaration['cell'][side]}-{code}"
                if observed[f"{side}_arm"] != expected_arm:
                    raise ValueError(f"{cell_id}: ratio arms differ from declaration")
                if any(pair[side]["quality"] != qualities[code, side][0] for pair in observed["pairs"]):
                    raise ValueError(f"{cell_id}: prepared quality {side} differs from receipt")
            interval = cell["interval"]
            comparison = relationship(interval)
            reported = configured.get("core_arm", declaration["cell"]["core_arm"]) == "single-core" and configured["timed_batch"] == 8
            closure = "preserved negative or inconclusive experiment"
            if reported:
                closure = "lead preserved in pilot" if interval["upper"] < 1 else "lead not established in pilot"
            if index:
                closure = "historical pilot"
            lines.append(
                f"| {cell_id} | {cell['pairs']} | {interval['estimate']:.6f} | "
                f"[{interval['lower']:.6f}, {interval['upper']:.6f}] | {interval['confidence']:.3f} | "
                f"{cell['decision']} / {cell['outcome']} | {comparison} | {closure} | {sha} |")
        lines.append("")

    lines += ["## Prepared quality", "",
              "These fixed-seed frames are shared by all timing cells and pilots; repetitions do not add frames.",
              "FER intervals are the prepared 95% Wilson intervals, not timing intervals.", "",
              "| Code | Arm | Frame errors / frames | FER Wilson 95% interval | Quality SHA-256 | Freeze-accepted pilot receipt SHA-256 |",
              "|---|---|---:|---|---|---|"]
    for (code, side), (quality, sha) in qualities.items():
        lower, upper = quality["fer_interval"]
        lines.append(f"| {code} | {declaration['cell'][side]} | {quality['frame_errors']}/{quality['frames']} | "
                     f"[{lower:.8f}, {upper:.8f}] | {sha} | {pilots[0][2]} |")
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--primary-pilot", required=True, help="pilot accepted by the freeze")
    parser.add_argument("--historical-pilot", action="append", default=[])
    parser.add_argument("--quality-dir", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    path = output_path(args.output)
    table = render(args.primary_pilot, args.historical_pilot, args.quality_dir)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(table)
    print(f"{args.output} sha256={digest(path)}")


if __name__ == "__main__":
    main()
