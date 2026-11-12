#!/usr/bin/env python3
"""Regenerate the synthetic analysis fixtures with the campaign tools.

Datasets are produced by permanent_campaign and permanent_dataset from the
tracked configurations, then stripped of observed identity: the execution log
carries a zero digest, the host "synthetic", and an argv without output paths.
The golden files are the analysis script's output on the small-campaign dataset.

    python3 regenerate.py [--output DIR]

DIR defaults to target/permanent-statistics-analysis-fixtures under the
repository root. Pass this directory as DIR to replace the tracked fixtures.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
EXPERIMENT = "experiments/permanent-statistics"
CONFIGS = {
    "small-campaign": f"{EXPERIMENT}/config/small-campaign.json",
    "small-campaign-halted": f"{EXPERIMENT}/tests/fixtures/analysis/halted-config.json",
}
BIN = ROOT / "target/release"
SYNTHETIC_IDENTITY = {
    "executable_sha256": "0" * 64,
    "build": {"profile": "synthetic", "opt_level": "synthetic", "lto": "synthetic",
              "codegen_units": 1, "target": "synthetic", "rustflags": []},
    "toolchain_version": "synthetic",
    "host": {"cpu": {"model": "synthetic", "physical_cores": 1, "logical_threads": 1},
             "accelerator": {"runtime": None, "model": None}},
}


def run(*args: object) -> None:
    subprocess.run([str(a) for a in args], cwd=ROOT, check=True)


def sanitize(record: dict) -> dict:
    identity = record["identity"]
    for key, value in SYNTHETIC_IDENTITY.items():
        identity[key] = json.loads(json.dumps(value))
    record["source_revision"] = "0" * 40
    return record


def sanitize_log(path: Path) -> None:
    lines = []
    for line in path.read_text().splitlines():
        event = json.loads(line) if line.strip() else None
        if event and "start" in event:
            start = event["start"]
            sanitize(start["record"])
            argv = start["argv"]
            start["argv"] = ["permanent_campaign"] + [
                "config.json" if argv[i - 1] == "--config" else
                "dataset" if argv[i - 1] == "--output" else a
                for i, a in enumerate(argv) if i > 0
            ]
        lines.append(json.dumps(event, separators=(",", ":")) if event else line)
    path.write_text("\n".join(lines) + "\n")


def campaign(name: str, dataset: Path, cells: list[tuple[int, list[str]]]) -> None:
    for q, extra in cells:
        run(BIN / "permanent_campaign", "--config", CONFIGS[name], "--output", dataset,
            "--q", q, "--n", 2, *extra)
    sanitize_log(dataset / "execution.jsonl")
    run(BIN / "permanent_dataset", "finalize", dataset)
    run(BIN / "permanent_dataset", "verify", dataset)


def write_checksums(dataset: Path) -> None:
    lines = [line.split("  ", 1)[1] for line in (dataset / "checksums.sha256").read_text().splitlines()]
    (dataset / "checksums.sha256").write_text("".join(
        f"{hashlib.sha256((dataset / p).read_bytes()).hexdigest()}  {p}\n" for p in lines))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path,
                        default=ROOT / "target/permanent-statistics-analysis-fixtures")
    out = parser.parse_args().output.resolve()
    run("cargo", "build", "--release", "-p", "permanent-statistics",
        "--bin", "permanent_campaign", "--bin", "permanent_dataset")
    with tempfile.TemporaryDirectory() as scratch_name:
        scratch = Path(scratch_name)
        small = scratch / "small-campaign"
        campaign("small-campaign", small, [(3, []), (5, []), (7, [])])
        halted = scratch / "small-campaign-halted"
        campaign("small-campaign-halted", halted, [(3, []), (5, ["--on-unavailable", "halt"])])

        mismatch = scratch / "checksum-mismatch"
        shutil.copytree(small, mismatch)
        with (mismatch / "summary.csv").open("a") as handle:
            handle.write("\n")

        invalid = scratch / "invalid-q"
        shutil.copytree(small, invalid)
        manifest = json.loads((invalid / "manifest.json").read_text())
        manifest["cells"][0]["q"] = 9
        (invalid / "manifest.json").write_text(
            json.dumps(manifest, indent=2, sort_keys=True))
        manifest_line = "manifest.json"
        sums = (invalid / "checksums.sha256").read_text().splitlines()
        (invalid / "checksums.sha256").write_text("".join(
            (f"{hashlib.sha256((invalid / manifest_line).read_bytes()).hexdigest()}  {manifest_line}\n"
             if line.endswith("  " + manifest_line) else line + "\n") for line in sums))

        analysed = scratch / "analysed"
        shutil.copytree(small, analysed)
        subprocess.run([sys.executable, ROOT / "scripts/permanent_zero_fraction_analysis.py", analysed],
                       cwd=ROOT, check=True)
        derived = analysed / "derived/small-campaign/zero-fraction-analysis"

        for name in ("small-campaign", "small-campaign-halted", "checksum-mismatch", "invalid-q"):
            shutil.rmtree(out / name, ignore_errors=True)
            shutil.copytree(scratch / name, out / name,
                            ignore=shutil.ignore_patterns("checkpoint.json"))
        shutil.rmtree(out / "golden/small-campaign", ignore_errors=True)
        shutil.copytree(derived, out / "golden/small-campaign")
    print(f"fixtures written to {out}")


if __name__ == "__main__":
    main()
