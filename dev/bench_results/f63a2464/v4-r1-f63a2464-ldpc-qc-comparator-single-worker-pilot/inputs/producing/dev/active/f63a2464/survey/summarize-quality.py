#!/usr/bin/env python3
"""Project the candidate quality evidence into its generated tables (jit:f63a2464).

Reads only committed evidence: this issue's untimed cell records and the frozen
`c077a88b` prepared quality records. Every number is derived here from those
bytes, so a rerun reproduces the file exactly. The statistics are the frozen
protocol's: the frame-bounded Hoeffding interval for BER, Wilson at the frozen
quality confidence for FER, and the paired bounded-mean upper bound for
non-inferiority.

Usage (from the worktree root):
  summarize-quality.py QUALITY_DIR > QUALITY_DIR/tables.md
"""

import argparse
import json
import math
import pathlib
import sys

# Frozen shared settings of the protocol, read from its source of truth.
PROTOCOL_SOURCE = pathlib.Path("dev/tools/tuning-campaign-support/src/protocol.rs")
FROZEN_QUALITY = pathlib.Path("dev/bench_results/c077a88b/v3-preparation/quality")
TOLERANCES = pathlib.Path("dev/active/f63a2464/quality-tolerances.md")

CODES = ["dvb-t2-r12", "nr-bg1-r12"]
CELLS = ["recorded", "punctured", "filler"]
BASELINE = "canonical-f32"

# Each candidate arm's matched external record under the contract it declares.
# A contract with no measured external arm on this host has no entry.
EXTERNAL = {
    ("dvb-t2-r12", "canonical-f32"): "aff3ct-flooding-nms-f32-dvb-t2-r12",
    ("nr-bg1-r12", "canonical-f32"): "aff3ct-flooding-nms-f32-nr-bg1-z384",
    ("dvb-t2-r12", "qc-f32"): "aff3ct-flooding-nms-f32-dvb-t2-r12",
    ("nr-bg1-r12", "qc-f32"): "aff3ct-flooding-nms-f32-nr-bg1-z384",
    ("dvb-t2-r12", "layered-f32"): "aff3ct-layered-nms-f32-dvb-t2-r12",
    ("nr-bg1-r12", "layered-f32"): "aff3ct-layered-nms-f32-nr-bg1-z384",
}
EXTERNAL_QUANTIZED = {
    "dvb-t2-r12": "aff3ct-flooding-nms-i16-dvb-t2-r12",
    "nr-bg1-r12": "aff3ct-flooding-nms-i16-nr-bg1-z384",
}

GF2_FROZEN = {
    "dvb-t2-r12": "gf2-nms-f32-dvb-t2-r12",
    "nr-bg1-r12": "gf2-nms-f32-nr-bg1-z384",
}


def frozen_settings():
    """The frozen `family_alpha` and `quality_confidence` from the protocol."""
    values = {}
    for line in PROTOCOL_SOURCE.read_text(encoding="utf-8").splitlines():
        text = line.strip()
        for name in ("family_alpha", "quality_confidence"):
            if text.startswith(f"{name}:") and text.endswith(","):
                values[name] = float(text.split(":", 1)[1].strip().rstrip(","))
    missing = {"family_alpha", "quality_confidence"} - set(values)
    if missing:
        sys.exit(f"{PROTOCOL_SOURCE} does not carry {sorted(missing)}")
    return values


def tolerance_ratio():
    """The `fer_ratio_max` the predeclared tolerances freeze."""
    for line in TOLERANCES.read_text(encoding="utf-8").splitlines():
        if line.startswith("`fer_ratio_max = "):
            return float(line.split("=", 1)[1].split("`")[0])
    sys.exit(f"{TOLERANCES} does not declare fer_ratio_max")


def wilson(successes, trials, confidence):
    """Wilson interval for a binomial proportion."""
    if trials == 0:
        return (0.0, 0.0)
    z = normal_quantile(1.0 - (1.0 - confidence) / 2.0)
    p = successes / trials
    denominator = 1.0 + z * z / trials
    centre = (p + z * z / (2 * trials)) / denominator
    spread = z * math.sqrt(p * (1 - p) / trials + z * z / (4 * trials * trials)) / denominator
    return (max(0.0, centre - spread), min(1.0, centre + spread))


def normal_quantile(p):
    """Inverse standard normal, by bisection on `erf`; deterministic."""
    low, high = -10.0, 10.0
    for _ in range(200):
        middle = (low + high) / 2.0
        if 0.5 * (1.0 + math.erf(middle / math.sqrt(2.0))) < p:
            low = middle
        else:
            high = middle
    return (low + high) / 2.0


def hoeffding(mean, trials, alpha):
    """The protocol's frame-bounded two-sided BER interval."""
    if trials == 0:
        return (0.0, 0.0)
    half = math.sqrt(math.log(2.0 / alpha) / (2.0 * trials))
    return (max(0.0, mean - half), min(1.0, mean + half))


def paired_bound(candidate, baseline, ratio, alpha):
    """The protocol's paired upper bound; `U <= 0` certifies compatibility."""
    trials = len(candidate)
    if trials == 0 or trials != len(baseline):
        return None
    differences = [c - ratio * b for c, b in zip(candidate, baseline)]
    mean = sum(differences) / trials
    return mean + (1.0 + ratio) * math.sqrt(math.log(2.0 / alpha) / (2.0 * trials))


def quantiles(values):
    ordered = sorted(values)
    def at(fraction):
        index = min(len(ordered) - 1, int(math.ceil(fraction * len(ordered))) - 1)
        return ordered[max(0, index)]
    return at(0.5), at(0.9), ordered[-1]


def load(directory):
    records = {}
    for path in sorted((directory / "cells").glob("*.json")):
        record = json.loads(path.read_text(encoding="utf-8"))
        records[(record["bundle"]["code"], record["cell"], record["arm"])] = record
    return records


def arms(records, code, cell):
    """Arms of one cell, baseline first then in a stable declared order."""
    present = [arm for (c, k, arm) in records if c == code and k == cell]
    present.sort(key=lambda arm: (arm != BASELINE, arm))
    return present


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=pathlib.Path)
    args = parser.parse_args()

    settings = frozen_settings()
    ratio = tolerance_ratio()
    quality_alpha = 1.0 - settings["quality_confidence"]
    # The corrected level a family's first confirmatory attempt spends over the
    # largest cell count the protocol admits: alpha / (1 * 2) over six cells.
    corrected_alpha = settings["family_alpha"] / 2.0 / 6.0

    records = load(args.directory)
    out = []
    add = out.append

    add("# Candidate quality on the measured DVB-T2 and NR cells")
    add("")
    add("<!-- Generated by dev/active/f63a2464/survey/summarize-quality.py. Do not edit. -->")
    add("")
    add("Source: this issue's untimed cell records under `cells/`, the frozen")
    add("`c077a88b` prepared quality records, the protocol's frozen shared settings")
    add("and the predeclared tolerances. Marginal FER intervals are Wilson at the")
    add("frozen quality confidence; marginal BER intervals are the protocol's")
    add("frame-bounded Hoeffding interval at the same level. The paired bound uses")
    add(f"the corrected level `{corrected_alpha:.9f}`, which is the frozen family alpha")
    add("divided by a first attempt's factor of two and by the six cells the")
    add("bootstrap tail-support rule admits; a confirmation recomputes it from its")
    add("own ledger.")
    add("")

    add("## Quality on the measured cells")
    add("")
    add("| Code | Cell | Arm | Frames | Bits | Frame errors | FER [Wilson] | Bit errors | BER [Hoeffding] | Iterations p50/p90/max | Syndrome failures |")
    add("|---|---|---|---|---|---|---|---|---|---|---|")
    for code in CODES:
        for cell in CELLS:
            for arm in arms(records, code, cell):
                record = records[(code, cell, arm)]
                frames = record["frames"]
                fer = record["frame_errors"] / frames if frames else 0.0
                fer_low, fer_high = wilson(record["frame_errors"], frames, settings["quality_confidence"])
                ber = record["bit_errors"] / record["bits"] if record["bits"] else 0.0
                per_frame = [errors / record["bundle"]["k"] for errors in record["per_frame_information_bit_errors"]]
                mean = sum(per_frame) / frames if frames else 0.0
                ber_low, ber_high = hoeffding(mean, frames, quality_alpha)
                p50, p90, top = quantiles(record["iterations"])
                add(
                    f"| {code} | {cell} | `{arm}` | {frames} | {record['bits']} | "
                    f"{record['frame_errors']} | {fer:.6f} [{fer_low:.6f}, {fer_high:.6f}] | "
                    f"{record['bit_errors']} | {ber:.3e} [{ber_low:.6f}, {ber_high:.6f}] | "
                    f"{p50}/{p90}/{top} | {record['syndrome_failures']} |"
                )
    add("")
    add("The iteration unit of `layered-f32` is a layer sweep and of every other")
    add("arm a flooding iteration; the two are not compared.")
    add("")

    add("## Paired non-inferiority against the canonical arm")
    add("")
    add(f"`fer_ratio_max` is {ratio}. Only `U <= 0` certifies compatibility; a positive")
    add("bound is insufficient evidence, never evidence of a worse frame error rate.")
    add("")
    add("| Code | Cell | Arm | Frames | Candidate frame errors | Canonical frame errors | U | Certified |")
    add("|---|---|---|---|---|---|---|---|")
    for code in CODES:
        for cell in CELLS:
            base = records.get((code, cell, BASELINE))
            if base is None:
                continue
            base_indicators = [1.0 if errors > 0 else 0.0 for errors in base["per_frame_information_bit_errors"]]
            for arm in arms(records, code, cell):
                if arm == BASELINE:
                    continue
                record = records[(code, cell, arm)]
                indicators = [1.0 if errors > 0 else 0.0 for errors in record["per_frame_information_bit_errors"]]
                bound = paired_bound(indicators, base_indicators, ratio, corrected_alpha)
                verdict = "yes" if bound is not None and bound <= 0.0 else "no"
                shown = "n/a" if bound is None else f"{bound:+.6f}"
                add(
                    f"| {code} | {cell} | `{arm}` | {record['frames']} | "
                    f"{record['frame_errors']} | {base['frame_errors']} | {shown} | {verdict} |"
                )
    add("")

    add("## The exploratory screen")
    add("")
    add("Condition 1: the arm introduces no frame error on a frame the canonical arm")
    add("decodes with no information-bit error. Condition 2: its aggregate")
    add("information-bit error count does not exceed the canonical arm's. Both are")
    add("descriptive admission conditions on the recorded corpus.")
    add("")
    add("| Code | Cell | Arm | New frame errors | Aggregate bit errors | Canonical bit errors | Admitted |")
    add("|---|---|---|---|---|---|---|")
    for code in CODES:
        for cell in CELLS:
            base = records.get((code, cell, BASELINE))
            if base is None:
                continue
            base_frames = base["per_frame_information_bit_errors"]
            for arm in arms(records, code, cell):
                if arm == BASELINE:
                    continue
                record = records[(code, cell, arm)]
                introduced = sum(
                    1
                    for candidate, baseline in zip(record["per_frame_information_bit_errors"], base_frames)
                    if candidate > 0 and baseline == 0
                )
                admitted = introduced == 0 and record["bit_errors"] <= base["bit_errors"]
                add(
                    f"| {code} | {cell} | `{arm}` | {introduced} | {record['bit_errors']} | "
                    f"{base['bit_errors']} | {'yes' if admitted else 'no'} |"
                )
    add("")

    add("## Difficult frames")
    add("")
    add("The subset of each recorded cell's frames the canonical arm leaves with at")
    add("least one information-bit error at the iteration cap. The subset is defined")
    add("by the canonical arm alone, so it is the same subset for every candidate.")
    add("")
    add("| Code | Difficult frames | Arm | Frames still in error | Bit errors on the subset |")
    add("|---|---|---|---|---|")
    for code in CODES:
        base = records.get((code, "recorded", BASELINE))
        if base is None:
            continue
        subset = [
            index
            for index, errors in enumerate(base["per_frame_information_bit_errors"])
            if errors > 0
        ]
        for arm in arms(records, code, "recorded"):
            record = records[(code, "recorded", arm)]
            errors = [record["per_frame_information_bit_errors"][index] for index in subset]
            add(
                f"| {code} | {len(subset)} | `{arm}` | {sum(1 for value in errors if value > 0)} | "
                f"{sum(errors)} |"
            )
    add("")

    add("## Bit-exactness of family QC")
    add("")
    add("The candidate's per-frame information-bit error vector and its per-frame")
    add("iteration vector against the canonical arm's, on the same frames of the same")
    add("cell. The posterior-level assertion is the prototype crate's behavioural")
    add("suite; this is its cell-level projection over the measured workloads.")
    add("")
    add("| Code | Cell | Frames | Error vectors equal | Iteration vectors equal |")
    add("|---|---|---|---|---|")
    for code in CODES:
        for cell in CELLS:
            candidate = records.get((code, cell, "qc-f32"))
            base = records.get((code, cell, BASELINE))
            if candidate is None or base is None:
                continue
            same_errors = (
                candidate["per_frame_information_bit_errors"]
                == base["per_frame_information_bit_errors"]
            )
            same_iterations = candidate["iterations"] == base["iterations"]
            add(
                f"| {code} | {cell} | {candidate['frames']} | "
                f"{'yes' if same_errors else 'no'} | {'yes' if same_iterations else 'no'} |"
            )
    add("")

    add("## External conformance under each declared contract")
    add("")
    add("Each row compares one arm of the recorded cell with the frozen `c077a88b`")
    add("record produced under the nearest declared contract on the same bundle. The")
    add("quantized arms have no measured external `i8` arm on this host, so their")
    add("comparator is the `i16` record; the difference in declared contract is")
    add("stated rather than absorbed.")
    add("")
    add("| Code | Arm | External record | Candidate FE/BE | External FE/BE | Per-frame vectors equal |")
    add("|---|---|---|---|---|---|")
    for code in CODES:
        for arm in arms(records, code, "recorded"):
            name = EXTERNAL.get((code, arm))
            if name is None and arm.startswith("quantized-"):
                name = EXTERNAL_QUANTIZED[code]
            if name is None:
                continue
            path = FROZEN_QUALITY / f"{name}.json"
            if not path.exists():
                continue
            external = json.loads(path.read_text(encoding="utf-8"))
            record = records[(code, "recorded", arm)]
            equal = record["per_frame_information_bit_errors"] == external["frame_bit_errors"]
            add(
                f"| {code} | `{arm}` | `{name}` | "
                f"{record['frame_errors']}/{record['bit_errors']} | "
                f"{external['frame_errors']}/{external['bit_errors']} | "
                f"{'yes' if equal else 'no'} |"
            )
    add("")
    add("### The canonical baseline against its own frozen record")
    add("")
    add("| Code | Frozen record | Candidate FE/BE | Frozen FE/BE | Per-frame vectors equal |")
    add("|---|---|---|---|---|")
    for code in CODES:
        name = GF2_FROZEN[code]
        external = json.loads((FROZEN_QUALITY / f"{name}.json").read_text(encoding="utf-8"))
        record = records[(code, "recorded", BASELINE)]
        equal = record["per_frame_information_bit_errors"] == external["frame_bit_errors"]
        add(
            f"| {code} | `{name}` | {record['frame_errors']}/{record['bit_errors']} | "
            f"{external['frame_errors']}/{external['bit_errors']} | "
            f"{'yes' if equal else 'no'} |"
        )
    add("")

    add("## Cell identities")
    add("")
    add("| Code | Cell | Bundle | Es/N0 (dB) | H SHA-256 | LLR SHA-256 | Punctured prefix | Filler positions |")
    add("|---|---|---|---|---|---|---|---|")
    seen = set()
    for code in CODES:
        for cell in CELLS:
            key = (code, cell)
            for arm in arms(records, code, cell):
                if key in seen:
                    continue
                seen.add(key)
                record = records[(code, cell, arm)]
                bundle = record["bundle"]
                transform = record["input_transform"]
                add(
                    f"| {code} | {cell} | `{pathlib.Path(bundle['path']).name}` | "
                    f"{bundle['esn0_db']} | `{bundle['h_sha256'][:16]}` | "
                    f"`{bundle['llrs_sha256'][:16]}` | {transform['punctured_prefix']} | "
                    f"{transform['filler_positions']} |"
                )
    add("")

    print("\n".join(out))


if __name__ == "__main__":
    main()
