#!/usr/bin/env python3
"""Record the MSRV intrinsic feasibility of the candidate families (jit:f63a2464).

Compiles the probe crate with the repository MSRV toolchain, emits its
assembly, runs its behavioural tests, and writes a record of what it observed:
the toolchain identity, the emitted assembly's digest, and, per probe function,
the distinct instruction mnemonics the compiler actually produced. Nothing in
this record is typed in: every field comes from the commands this run executes.

Usage (from the worktree root):
  record-intrinsic-feasibility.py OUT_DIR
"""

import argparse
import hashlib
import json
import os
import pathlib
import re
import subprocess
import sys

TOOLCHAIN = "1.95"
ROOT = pathlib.Path(subprocess.run(
    ["git", "-C", str(pathlib.Path(__file__).resolve().parent), "rev-parse", "--show-toplevel"],
    check=True, capture_output=True, text=True,
).stdout.strip())
PROBE = pathlib.Path(__file__).resolve().parent.relative_to(ROOT) / "intrinsics-probe"
TARGET = pathlib.Path("target/ldpc-intrinsics-probe")

# Instructions that carry no candidate-specific meaning; the record keeps the
# SIMD and control instructions a reader judges a lowering by.
UNINTERESTING = {"ret", "nop", "push", "pop", "int3", "endbr64", "cfi"}

LABEL = re.compile(r"^([A-Za-z_$.][\w$.@]*):")
INSTRUCTION = re.compile(r"^\s+([a-z][a-z0-9_.]*)\b")


def run(command, **kwargs):
    return subprocess.run(command, check=True, capture_output=True, text=True, **kwargs)


def component(name):
    """The length-prefixed path component a legacy-mangled symbol carries."""
    return f"{len(name)}{name}17h"


def functions_from_assembly(text, wanted):
    """Distinct mnemonics per emitted function whose symbol names a probe."""
    found = {}
    current = None
    for line in text.splitlines():
        label = LABEL.match(line)
        if label:
            if label.group(1).startswith(".L"):
                continue
            symbol = label.group(1)
            current = next((name for name in wanted if component(name) in symbol), None)
            if current is not None:
                found.setdefault(current, {"symbol": label.group(1), "instructions": []})
            continue
        if current is None:
            continue
        instruction = INSTRUCTION.match(line)
        if not instruction:
            continue
        mnemonic = instruction.group(1)
        if mnemonic.startswith(".") or mnemonic in UNINTERESTING:
            continue
        entry = found[current]["instructions"]
        if mnemonic not in entry:
            entry.append(mnemonic)
    return found


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("out_dir", type=pathlib.Path)
    args = parser.parse_args()

    repo = pathlib.Path(run(["git", "rev-parse", "--show-toplevel"]).stdout.strip())
    if pathlib.Path.cwd() != repo:
        sys.exit("invoke from the worktree root")

    env = dict(os.environ, CARGO_TARGET_DIR=str(repo / TARGET))
    manifest = ["--manifest-path", str(PROBE / "Cargo.toml")]

    version = run(["cargo", f"+{TOOLCHAIN}", "--version"]).stdout.strip()
    rustc = run(["rustc", f"+{TOOLCHAIN}", "--version", "--verbose"]).stdout.strip()

    run(["cargo", f"+{TOOLCHAIN}", "clean", "--offline", "--release"] + manifest, env=env)
    run(
        ["cargo", f"+{TOOLCHAIN}", "rustc", "--offline", "--release", "--lib"]
        + manifest
        + ["--", "--emit", "asm", "-Cllvm-args=--x86-asm-syntax=intel"],
        env=env,
    )

    assembly = sorted((repo / TARGET / "release" / "deps").glob("ldpc_intrinsics_probe-*.s"))
    if len(assembly) != 1:
        sys.exit(f"{len(assembly)} emitted assembly files, want exactly one")
    text = assembly[0].read_text(encoding="utf-8")

    wanted = sorted(
        name
        # A generic function emits no code until it is instantiated, so the
        # record names only the concrete entry points.
        for name in re.findall(r"pub unsafe fn (\w+)\(", (PROBE / "src/lib.rs").read_text())
    )
    observed = functions_from_assembly(text, wanted)

    tests = subprocess.run(
        ["cargo", f"+{TOOLCHAIN}", "test", "--offline", "--release"] + manifest,
        capture_output=True,
        text=True,
        env=env,
    )

    args.out_dir.mkdir(parents=True, exist_ok=True)
    (args.out_dir / "probe.s").write_text(text, encoding="utf-8")
    (args.out_dir / "tests.log").write_text(tests.stdout + tests.stderr, encoding="utf-8")

    record = {
        "schema": "ldpc-intrinsic-feasibility-v1",
        "toolchain": {"cargo": version, "rustc": rustc},
        "assembly": {
            "path": str((args.out_dir / "probe.s").as_posix()),
            "sha256": hashlib.sha256(text.encode("utf-8")).hexdigest(),
            "source_sha256": hashlib.sha256(
                (PROBE / "src/lib.rs").read_bytes()
            ).hexdigest(),
        },
        "behavioural_tests": {
            "command": f"cargo +{TOOLCHAIN} test --offline --release",
            "exit_code": tests.returncode,
            "log": str((args.out_dir / "tests.log").as_posix()),
        },
        "probes": [
            {
                "function": name,
                "emitted": name in observed,
                "symbol": observed.get(name, {}).get("symbol"),
                "instructions": observed.get(name, {}).get("instructions", []),
            }
            for name in wanted
        ],
    }
    (args.out_dir / "feasibility.json").write_text(
        json.dumps(record, indent=2) + "\n", encoding="utf-8"
    )
    print((args.out_dir / "feasibility.json").as_posix())


if __name__ == "__main__":
    main()
