#!/usr/bin/env python3
"""Replay every recorded frame through each steady-state arm path (jit:3be770d5).

For every arm of `arms.json` and both codes, runs `ldpc-throughput-validate`
with the arm's environment and the `c077a88b` prepared quality file, and
appends each verdict line to the output. Untimed: it checks decisions and
iteration distributions, never speed. Exits nonzero when any verdict fails.

Usage: validate.py --bin-dir DIR --inputs DIR --quality DIR --workers N --output FILE
"""

import argparse
import json
import os
import pathlib
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser()
    for name in ("--bin-dir", "--inputs", "--quality", "--output"):
        parser.add_argument(name, required=True)
    parser.add_argument("--workers", type=int, default=2)
    args = parser.parse_args()
    catalogue = json.loads((HERE / "arms.json").read_text(encoding="utf-8"))
    binary = os.path.join(args.bin_dir, "ldpc-throughput-validate")
    failed = 0
    with open(args.output, "x", encoding="utf-8") as output:
        for arm, spec in catalogue["arms"].items():
            for code_key, code in catalogue["codes"].items():
                environment = dict(os.environ, **spec["environment"])
                environment["GF2_LDPC_QUALITY"] = os.path.join(args.quality, f"{arm}-{code_key}.json")
                command = [binary, "--arm", spec["profile_arm"],
                           "--bundle", os.path.join(args.inputs, code["bundle"]),
                           "--code", code["code"], "--workers", str(args.workers)]
                done = subprocess.run(command, env=environment, capture_output=True, text=True)
                verdict = json.loads(done.stdout) if done.stdout.strip() else None
                record = {"arm": arm, "code_key": code_key, "command": command,
                          "environment": spec["environment"],
                          "quality": environment["GF2_LDPC_QUALITY"],
                          "exit": done.returncode, "verdict": verdict,
                          "stderr": done.stderr.strip()}
                output.write(json.dumps(record) + "\n")
                output.flush()
                print(f"{arm} {code_key}: exit {done.returncode}", file=sys.stderr)
                failed += done.returncode != 0
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
