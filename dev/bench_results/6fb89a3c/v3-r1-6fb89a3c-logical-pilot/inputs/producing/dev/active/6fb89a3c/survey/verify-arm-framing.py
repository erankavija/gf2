#!/usr/bin/env python3
"""Exercise every child-v2 arm and validate its one-line JSON framing."""
from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent
GF2_TARGET = Path(os.environ.get("GF2_SURVEY_GF2_TARGET", ROOT / "gf2-side/target/release"))
CASES = [
    (ROOT / "m4ri_transpose_arm", {"n": 64, "seed": 1}),
    (ROOT / "m4ri_transpose_arm", {"cols": 63, "rows": 63, "seed": 1}),
    (ROOT / "bitshuffle_transpose_arm", {"n": 64, "seed": 1}),
    (ROOT / "bitshuffle_transpose_arm", {"cols": 64, "rows": 64, "seed": 1, "adapter": "padded"}),
    (ROOT / "bitshuffle_transpose_arm", {"cols": 65, "rows": 65, "seed": 1, "adapter": "padded"}),
    (ROOT / "isal_xor_arm", {"alignment_bytes": 32, "seed": 1, "words": 8}),
    (ROOT / "isal_xor_arm", {"alignment_bytes": 32, "seed": 1, "words": 64, "sources": 3}),
    (ROOT / "m4ri_genmatrix_arm", {"code": "B1", "seed": 1}),
    (GF2_TARGET / "gf2_transpose_arm", {"n": 64, "seed": 1}),
    (GF2_TARGET / "gf2_transpose_arm", {"cols": 63, "rows": 63, "seed": 1}),
    (GF2_TARGET / "gf2_logical_xor_arm", {"alignment_bytes": 32, "seed": 1, "words": 8}),
    (GF2_TARGET / "gf2_logical_xor_arm", {"alignment_bytes": 32, "seed": 1, "words": 64, "sources": 3}),
    (GF2_TARGET / "gf2_bch_genmatrix_arm", {"code": "B1", "seed": 1}),
    (GF2_TARGET / "gf2_bch_genmatrix_arm", {"code": "B1", "seed": 1, "route": "reference"}),
]
# A whole-consumer arm must report a conversion record; a kernel-isolated arm
# must not.
CONSUMER = {"rows", "words", "code"}


def main() -> None:
    results = []
    for binary, case in CASES:
        request = {
            "schema": "zen3-benchmark-arm-request-v1",
            "cell_id": "framing-check",
            "arm": binary.name,
            "role": "baseline",
            "pair": 0,
            # `serde_json::Value` uses an ordered map and re-encodes object
            # keys lexicographically in the canonical request.
            "case": dict(sorted(case.items())),
            "cache_state": "warm",
            "windows": 1,
            "window_target_ms": 1,
            "cpus": [],
            "workers_declared": 1,
        }
        env = os.environ.copy()
        env["GF2_TUNING_FRESH_CASE"] = "child-v2"
        process = subprocess.run(
            [str(binary)], input=json.dumps(request, separators=(",", ":")), text=True,
            capture_output=True, cwd=ROOT.parents[3], env=env,
        )
        if process.returncode != 0:
            raise SystemExit(f"{binary.name}: exit {process.returncode}: {process.stderr}")
        lines = process.stdout.splitlines()
        if len(lines) != 1 or not lines[0].startswith("GF2_TUNING_RESULT="):
            raise SystemExit(f"{binary.name}: invalid result framing: {process.stdout!r}")
        value = json.loads(lines[0].split("=", 1)[1])
        if value.get("schema") != "zen3-benchmark-arm-result-v1":
            raise SystemExit(f"{binary.name}: invalid result schema")
        if len(value.get("windows", [])) != 1:
            raise SystemExit(f"{binary.name}: invalid window count")
        conversion = value.get("conversion")
        expects_conversion = bool(CONSUMER & set(case))
        if (conversion is not None) != expects_conversion:
            raise SystemExit(f"{binary.name} {case}: conversion record presence {conversion is not None} differs from the metric kind")
        if conversion is not None and list(conversion) != [
            "setup_ns", "pack_ns", "unpack_ns", "batch_fill_ns", "dispatch_ns"
        ]:
            raise SystemExit(f"{binary.name}: noncanonical conversion key order")
        results.append({
            "binary": binary.name,
            "case": case,
            "status": "pass",
            "selected_path": value.get("selected_path"),
            "conversion": conversion,
        })
        print(f"PASS child-v2 framing: {binary.name} {value.get('selected_path')}")
    report = {"schema": "gf2-external-arm-framing-v2", "issue": "6fb89a3c", "arms": results}
    (ROOT / "framing-report.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
