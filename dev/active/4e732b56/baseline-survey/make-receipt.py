#!/usr/bin/env python3
"""Render the survey's benchmark receipt from a run directory (jit:4e732b56).

Every figure, file name, and provenance line the receipt carries is read out of
the run directory at render time. Nothing is embedded here, so re-rendering a
re-run produces a receipt describing that run rather than this one.

Usage:
    ./make-receipt.py <run-dir> > <run-dir>/<date>-<id>-survey-receipt.md
"""

from __future__ import annotations

import argparse
import hashlib
import pathlib
import re
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from summarize import load, summarize  # noqa: E402


def host_field(host_files: list[pathlib.Path], pattern: str) -> str:
    """First line matching `pattern` across the run's host records."""
    rx = re.compile(pattern)
    for path in host_files:
        for line in path.read_text(errors="replace").splitlines():
            if rx.search(line):
                return line.strip()
    return "(not recorded)"


def stage_prefix(stem: str) -> tuple[str, str] | None:
    """Split a measured file stem into its run prefix and library stage."""
    match = re.match(r"^(.+)-(aff3ct|bchlib|itpp|m4ri|gf2)$", stem)
    return (match.group(1), match.group(2)) if match else None


def load_derivation(run: pathlib.Path) -> dict[str, list[tuple[str, str]]]:
    """Parse the committed invocation-derivation record, if present.

    Returns stage name -> ordered list of (invocation, basis), for stages
    whose log carries no native `# command:` header. The derivation record
    is the only place a reconstructed or session-recorded invocation may be
    stated (`@/inv/runtime-observed-provenance`); this function is the sole
    reader of it.
    """
    table: dict[str, list[tuple[str, str]]] = {}
    paths = sorted(run.glob("*-invocation-derivation.md"))
    if not paths:
        return table
    in_table = False
    for line in paths[0].read_text(errors="replace").splitlines():
        if line.startswith("| Stage |"):
            in_table = True
            continue
        if not in_table or not line.startswith("|"):
            continue
        cells = [c.strip() for c in line.strip("|").split("|")]
        if len(cells) != 4 or set(cells[0]) == {"-"}:
            continue
        stage = cells[0].strip("`")
        invocation = cells[1].strip("`")
        basis = "session-recorded" if cells[2].startswith("session-recorded") else "reconstructed"
        table.setdefault(stage, []).append((invocation, basis))
    return table


def stage_commands(
    log_path: pathlib.Path, stage: str, derivation: dict[str, list[tuple[str, str]]]
) -> tuple[list[str], list[str]]:
    """Invocation strings and their basis for one stage.

    Prefers a native `# command:`/`# environment:` header pair emitted by
    the tool itself (basis `recorded (log header)`); falls back to the
    committed derivation record, matched by stage-name suffix, for stages
    whose log predates header emission.
    """
    commands: list[str] = []
    if log_path.is_file():
        for line in log_path.read_text(errors="replace").splitlines():
            if line.startswith("# command: "):
                commands.append(line.removeprefix("# command: "))
            elif line.startswith("# environment: ") and commands:
                commands[-1] = f"{line.removeprefix('# environment: ')} {commands[-1]}"
    if commands:
        return commands, ["recorded (log header)"] * len(commands)
    for deriv_stage, rows in derivation.items():
        if stage == deriv_stage or stage.endswith(f"-{deriv_stage}"):
            return [inv for inv, _ in rows], [basis for _, basis in rows]
    return [], []


def provenance_records(run: pathlib.Path) -> list[dict[str, str]]:
    """Collect command, revision, and host-manifest provenance for each output."""
    derivation = load_derivation(run)
    records = []
    for csv_path in sorted(run.glob("*.csv")):
        split = stage_prefix(csv_path.stem)
        if split is None:
            continue
        prefix, stage_lib = split
        stage = f"{prefix}-{stage_lib}"
        host_path = run / f"{prefix}-host.txt"
        log_path = run / f"{csv_path.stem}.log"
        commands, bases = stage_commands(log_path, stage, derivation)
        data_revision = ""
        if log_path.is_file():
            for line in log_path.read_text(errors="replace").splitlines():
                if line.startswith("# gf2_revision:"):
                    data_revision = line.split(":", 1)[1].strip()
                    break
        host_revision = host_field([host_path], r"^# gf2 revision:") if host_path.is_file() else "(not recorded)"
        host_revision = host_revision.split(":", 1)[-1].strip()
        revision = data_revision or host_revision
        if data_revision and data_revision != host_revision:
            revision = f"{data_revision} (host: {host_revision})"
        records.append({
            "stage": stage,
            "csv": csv_path.name,
            "log": log_path.name,
            "commands": "<br>".join(f"`{command}`" for command in commands) or "(not recorded)",
            "basis": "<br>".join(bases) or "(not recorded)",
            "revision": revision,
            "host": host_path.name,
        })

    for perf_path in sorted(run.glob("*-perf-stat.txt")):
        match = re.match(r"^(.+)-(aff3ct|bchlib|itpp|m4ri|gf2)-perf-stat\.txt$", perf_path.name)
        if match is None:
            continue
        prefix = match.group(1)
        stage = f"{prefix}-{match.group(2)}-perf"
        host_path = run / f"{prefix}-host.txt"
        commands, bases = stage_commands(perf_path, stage, derivation)
        revision = host_field([host_path], r"^# gf2 revision:") if host_path.is_file() else "(not recorded)"
        revision = revision.split(":", 1)[-1].strip()
        records.append({
            "stage": stage,
            "csv": "(perf counters)",
            "log": perf_path.name,
            "commands": "<br>".join(f"`{command}`" for command in commands) or "(not recorded)",
            "basis": "<br>".join(bases) or "(not recorded)",
            "revision": revision,
            "host": host_path.name,
        })
    return records


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("run_dir", type=pathlib.Path)
    args = ap.parse_args()
    run = args.run_dir

    host_files = sorted(run.glob("*host.txt"))
    if not host_files:
        print(f"no host record under {run}", file=sys.stderr)
        return 1

    rows = load(run)
    if not rows:
        print(f"no measurement rows under {run}", file=sys.stderr)
        return 1

    generated = [host_field([p], r"^# generated:").split(":", 1)[-1].strip() for p in host_files]
    survey_logs = sorted(run.glob("*.log"))
    nice_denied = any("nice: cannot set niceness: Permission denied" in p.read_text(errors="replace")
                      for p in survey_logs)

    print("# Receipt: external-baseline survey for BCH encoding and generator-matrix materialization")
    print()
    print("Rendered by `baseline-survey/make-receipt.py` from the run directory; every")
    print("figure below is read out of the committed CSVs at render time.")
    print()
    print("| Field | Value |")
    print("|---|---|")
    print("| Issue | `4e732b56` |")
    print(f"| Run timestamps (UTC) | {', '.join(generated)} |")
    print("| gf2 revision | recorded per stage below; T2N gf2 is an explicitly separate revision |")
    print(f"| Host | {host_field(host_files, r'Model name:').split(':', 1)[-1].strip()} |")
    posture = "nice -n -5 requested, denied, and the child ran at inherited default priority" if nice_denied else "nice -n -5 requested"
    print(f"| Cores pinned | CCX1 via `dev/scripts/ccx1-bench-flock.sh` (`taskset -c 6-11`; {posture}) |")
    print(f"| Governor | {host_field(host_files, r'scaling_governor').split(':')[-1].strip()} |")
    print(f"| Kernel | {host_field(host_files, r'^Linux ')} |")
    print(f"| C compiler | {host_field(host_files, r'^gcc ')} |")
    print(f"| C++ compiler | {host_field(host_files, r'^g\+\+ ')} |")
    print(f"| Rust | {host_field(host_files, r'^rustc ')} |")
    print()
    print("## Baseline pins")
    print()
    print("| Pin | Value |")
    print("|---|---|")
    for pat in [r"^aff3ct tag:", r"^aff3ct commit:", r"^bchlib tag:", r"^bchlib commit:",
                r"^m4ri version:", r"^m4ri tarball sha256:", r"^itpp release:", r"^itpp soname:"]:
        line = host_field(host_files, pat)
        if ":" in line:
            name, _, value = line.partition(":")
            print(f"| {name.strip()} | `{value.strip()}` |")
    print()
    print("## Reference build configuration")
    print()
    for pat in [r"^aff3ct library:", r"^aff3ct defines:", r"^m4ri library:"]:
        print(f"* `{host_field(host_files, pat)}`")
    print()
    print("## Per-stage provenance")
    print()
    print("Each row identifies the committed output, the stage's invocation(s), the basis for")
    print("each invocation, the gf2 revision supplying that stage's data, and the host manifest")
    print("covering it. Basis is `recorded (log header)` when the tool's own log carries a")
    print("`# command:`/`# environment:` header; for a stage whose log predates header emission,")
    print("it is `reconstructed` (derived from the runner's argument construction, cited from the")
    print("committed invocation-derivation record) or `session-recorded` (executed directly by an")
    print("agent session, also cited from that record). For non-gf2 stages the revision is")
    print("inherited from the host manifest; a differing data revision is shown as")
    print("`data (host: manifest)` in the same row.")
    print()
    print("| Stage | Output | Provenance log | Exact invocation(s) | Basis | gf2 revision for this stage | Host manifest |")
    print("|---|---|---|---|---|---|---|")
    for record in provenance_records(run):
        host_link = f"[{record['host']}]({record['host']})"
        output = record["csv"] if record["csv"] == "(perf counters)" else f"[{record['csv']}]({record['csv']})"
        log_link = f"[{record['log']}]({record['log']})"
        print(f"| `{record['stage']}` | {output} | {log_link} | {record['commands']} | {record['basis']} "
              f"| `{record['revision']}` | {host_link} |")
    print()
    print("## Measured cells")
    print()
    print("Throughput is information bits per second for W1 and matrix bits per second")
    print("for W2. `Trials` is the number of independent trials the cell's wall budget")
    print("allowed; a cell marked *estimate* was projected from a measured per-unit cost")
    print("and was never run at that size.")
    print()
    cells = summarize(rows, None)
    print("| Workload | Row | Batch | Library | Algorithm | Trials | Median | Min | Max | Spread |")
    print("|---|---|---|---|---|---|---|---|---|---|")
    for c in cells:
        if c["projected"]:
            print(f"| {c['workload']} | {c['code']} | {c['batch']} | {c['lib']} {c['version']} "
                  f"| `{c['algorithm']}` | *estimate* | {c['rate_med']:.2f} | — | — | — |")
        else:
            print(f"| {c['workload']} | {c['code']} | {c['batch']} | {c['lib']} {c['version']} "
                  f"| `{c['algorithm']}` | {c['trials']} | {c['rate_med']:.2f} | {c['rate_min']:.2f} "
                  f"| {c['rate_max']:.2f} | {c['spread_pct']:.1f}% |")
    print()
    print("## Files")
    print()
    print("| File | SHA-256 | Bytes |")
    print("|---|---|---|")
    for path in sorted(run.iterdir()):
        if not path.is_file() or path.name.endswith("-receipt.md"):
            continue
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        print(f"| `{path.name}` | `{digest[:16]}…` | {path.stat().st_size} |")
    print()
    print("## Reproduction")
    print()
    print("```")
    print("dev/active/4e732b56/baseline-survey/fetch-build.sh")
    print(f"dev/active/4e732b56/baseline-survey/run-survey.sh {run} <codes> <prefix>")
    print(f"dev/active/4e732b56/baseline-survey/make-receipt.py {run}")
    print("```")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
