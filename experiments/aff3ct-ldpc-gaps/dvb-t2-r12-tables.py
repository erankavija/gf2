#!/usr/bin/env python3
"""Projects the step logs of dvb-t2-r12-diagnosis.sh into Markdown tables.

Usage: dvb-t2-r12-tables.py [LOG_DIR]

LOG_DIR defaults to target/aff3ct-ldpc-gaps/dvb-t2-r12-diagnosis of the
repository holding this script. Each table row names the log line it comes
from. Frame error rates carry their 95% Wilson score interval: the harness's
own for gf2 POINT records, recomputed from the frame and error counts for
AFF3CT result rows. FAIL records summarize per step and Eb/N0.
"""

import collections
import math
import pathlib
import re
import statistics
import subprocess
import sys

# AFF3CT rows carry counts only, so this mirrors
# gf2_stats::intervals::wilson_interval until @/issue/7f9751af moves it to the
# Rust side.
Z95 = 1.959963984540054


def wilson95(errors, frames):
    p = errors / frames
    z2 = Z95 * Z95
    centre = p + z2 / (2 * frames)
    half = Z95 * math.sqrt(p * (1 - p) / frames + z2 / (4 * frames * frames))
    denominator = 1 + z2 / frames
    low = 0.0 if errors == 0 else (centre - half) / denominator
    high = 1.0 if errors == frames else (centre + half) / denominator
    return low, high


def fields(line):
    return dict(re.findall(r"(\S+?)=(\{[^}]*\}|\[[^\]]*\]|\S+)", line))


def short(command):
    return re.sub(r"/\S*/", "", command)


def project(path):
    step = path.stem
    print(f"\n## {step}\n")
    lines = path.read_text().splitlines()
    for line in lines:
        if line.startswith(("STEP", "EXECUTABLE", "INPUT", "END")):
            print(f"- `{short(line)}`")
    rows, failures, command = [], collections.defaultdict(list), None
    for number, line in enumerate(lines, 1):
        if line.startswith("COMMAND"):
            command = short(line[len("COMMAND ") :])
        elif line.startswith("POINT"):
            f = fields(line)
            low, high = f["fer_wilson95"].strip("[]").split(",")
            extra = ""
            if "mean_beliefs_saturated" in f:
                extra = f"clipped {f['mean_channel_clipped']}, saturated {f['mean_beliefs_saturated']}"
            rows.append((number, "gf2", f["ebn0"], f["decoder"], f["frames"], f["errors"],
                         f["fer"], f"[{low}, {high}]", f["undetected"], f["mean_iterations"], extra))
        elif "||" in line and command and "aff3ct" in command:
            cells = [c.strip() for c in line.replace("||", "|").split("|")]
            ebn0, frames, errors = cells[1], int(cells[2]), int(cells[4])
            low, high = wilson95(errors, frames)
            seed = re.search(r"--sim-seed (\S+)", command).group(1)
            decoder = " ".join(re.findall(r"(?:--enc-type|-D|--dec-implem|--dec-norm|-i) \S+", command))
            rows.append((number, "AFF3CT", ebn0, decoder, frames, errors, f"{errors / frames:.3e}",
                         f"[{low:.3e}, {high:.3e}]", "", "", f"seed {seed}"))
        elif line.startswith("FAIL"):
            failures[fields(line)["ebn0"]].append(fields(line))
    if rows:
        print("\n| line | side | Eb/N0 | decoder | frames | FE | FER | 95% Wilson | undetected | mean it. | notes |")
        print("|---|---|---|---|---|---|---|---|---|---|---|")
        for row in rows:
            print("| " + " | ".join(str(c) for c in row) + " |")
    for ebn0, fs in sorted(failures.items()):
        n = len(fs)
        degrees = collections.Counter()
        for f in fs:
            for d, c in re.findall(r"(\d+): (\d+)", f["wrong_by_degree"]):
                degrees[int(d)] += int(c)
        cap = {f["iterations"] for f in fs}
        print(f"\nFAIL records at {ebn0} dB: {n} lost ({fs[0]['decoder']}); "
              f"syndrome passed {sum(f['syndrome_passed'] == 'true' for f in fs)}; "
              f"terminal iteration counts {sorted(cap, key=int)}; "
              f"syndrome weight median {statistics.median(int(f['syndrome_weight']) for f in fs)}, "
              f"max {max(int(f['syndrome_weight']) for f in fs)}; "
              f"wrong info bits median {statistics.median(int(f['wrong_info']) for f in fs)}, "
              f"max {max(int(f['wrong_info']) for f in fs)}; "
              f"wrong bits by column degree {dict(sorted(degrees.items()))}")
        if "frame_scale" in fs[0]:
            print(f"frame scales {sorted({f['frame_scale'] for f in fs})}; channel clipped median "
                  f"{statistics.median(int(f['channel_clipped']) for f in fs)}; beliefs saturated median "
                  f"{statistics.median(int(f['beliefs_saturated']) for f in fs)}")
        rechecks = [k for k in fs[0] if k.startswith("recheck[")]
        print("\n| recheck | recovered |\n|---|---|")
        for key in rechecks:
            print(f"| `{key[8:-1]}` | {sum(f[key].startswith('ok') for f in fs)}/{n} |")


def main():
    if len(sys.argv) > 1:
        root = pathlib.Path(sys.argv[1])
    else:
        here = pathlib.Path(__file__).resolve().parent
        repo = subprocess.run(["git", "-C", str(here), "rev-parse", "--show-toplevel"],
                              check=True, capture_output=True, text=True).stdout.strip()
        root = pathlib.Path(repo) / "target/aff3ct-ldpc-gaps/dvb-t2-r12-diagnosis"
    for path in sorted(root.glob("*.log")):
        project(path)


if __name__ == "__main__":
    main()
