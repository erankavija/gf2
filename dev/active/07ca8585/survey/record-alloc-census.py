#!/usr/bin/env python3
"""Record the allocation census of both gf2 generations (jit:07ca8585).

Runs the `3be770d5` harness's `ldpc-alloc-census` from each generation's build
over the same recorded frames and writes one JSON record per frame per
generation. The census counts heap requests around each frame's decode under a
counting global allocator; the counts are exact and deterministic for a fixed
decoder and input, and are not a timing, so this runs outside the benchmark
mutex.

Both generations decode through the harness's owning entry point
(`decode_to_codeword`), which returns a codeword vector. So the `after`
generation's floor is one request per frame, not zero: the zero steady-state
claim of REQ-07 is about the prepared-workspace entry point and is asserted by
`crates/gf2-coding/tests/ldpc_decode_allocations.rs`. What this census measures
is the per-edge and per-iteration allocation the change removes, over the
frozen workloads, beside each frame's iteration count and bit errors.

Usage (from the worktree root):
  record-alloc-census.py --before-dir DIR --after-dir DIR --inputs DIR
      --identity PATH --output PATH [--batch 8]
"""

import argparse
import json
import pathlib
import subprocess


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--before-dir", required=True, type=pathlib.Path)
    parser.add_argument("--after-dir", required=True, type=pathlib.Path)
    parser.add_argument("--inputs", required=True, type=pathlib.Path)
    parser.add_argument("--identity", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    parser.add_argument("--batch", type=int, default=8)
    args = parser.parse_args()

    identity = json.loads(args.identity.read_text(encoding="utf-8"))
    records = []
    for generation, directory in (("before", args.before_dir), ("after", args.after_dir)):
        binary = directory / "ldpc-alloc-census"
        digest = identity["generations"][generation]["executables"]["ldpc-alloc-census"]
        for manifest_path in sorted(args.inputs.glob("*/manifest.json")):
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            completed = subprocess.run(
                [
                    str(binary),
                    "--bundle", str(manifest_path.parent),
                    "--code", manifest["code"],
                    "--batch", str(args.batch),
                ],
                check=True, capture_output=True, text=True,
            )
            for line in completed.stdout.splitlines():
                if not line.strip():
                    continue
                record = json.loads(line)
                record["generation"] = generation
                record["executable_sha256"] = digest
                record["bundle"] = manifest_path.parent.name
                records.append(record)

    args.output.parent.mkdir(parents=True, exist_ok=True)
    with open(args.output, "w", encoding="utf-8") as handle:
        for record in records:
            handle.write(json.dumps(record, sort_keys=True) + "\n")
    print(args.output, len(records), "records")


if __name__ == "__main__":
    main()
