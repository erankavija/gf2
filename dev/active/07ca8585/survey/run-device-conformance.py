#!/usr/bin/env python3
"""Run and record correctness-only LDPC GPU conformance (jit:07ca8585).

The output is a durable receipt, raw device/toolchain reports, exact test logs,
and hashes of every source and executable that decides the verdict. Test-runner
durations are incidental diagnostics and are not interpreted as measurements.
"""

import argparse
import datetime
import hashlib
import json
import os
import pathlib
import re
import subprocess
import sys


ISSUE = "07ca8585"
OUTPUT_PREFIX = pathlib.Path("dev/bench_results") / ISSUE / "device-conformance-"
SOURCES = [
    pathlib.Path("crates/gf2-kernels-hip/hip/ldpc_bp.hip"),
    pathlib.Path("crates/gf2-kernels-hip/src/ffi.rs"),
    pathlib.Path("crates/gf2-kernels-hip/src/launch_ldpc_bp.rs"),
    pathlib.Path("crates/gf2-coding/src/ldpc/min_sum.rs"),
    pathlib.Path("crates/gf2-coding/tests/ldpc_check_update_contract.rs"),
    pathlib.Path("crates/gf2-sim/tests/gpu_ldpc_byte_identity.rs"),
    pathlib.Path("crates/gf2-sim/tests/gpu_byte_identity.rs"),
    pathlib.Path("crates/gf2-sim/tests/gpu_nr_5g_byte_identity.rs"),
    pathlib.Path("dev/active/07ca8585/survey/run-device-conformance.py"),
]
EXPECTED_TESTS = {
    "device": ["device_min_sum_matches_cpu_on_signed_zero_and_nan"],
    "ordinary_slow": [
        "gpu_ldpc_hard_decision_byte_identical_to_cpu",
        "gpu_chain_verdict_byte_identical_r12_16qam",
        "gpu_chain_verdict_byte_identical_r23_64qam",
        "gpu_chain_verdict_byte_identical_r34_16qam",
        "gpu_nr_5g_bg1_z384_r12_byte_identical_to_cpu",
    ],
    "ordinary_smoke": ["gpu_nr_5g_smoke_byte_identical_to_cpu"],
}


def sha256(path):
    digest = hashlib.sha256()
    with pathlib.Path(path).open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def command_text(argv):
    return " ".join(subprocess.list2cmdline([part]) for part in argv)


def run_logged(root, raw, execution, name, argv, extra_env=None):
    env = os.environ.copy()
    if extra_env:
        env.update(extra_env)
    command = command_text(argv)
    with execution.open("a", encoding="utf-8") as log:
        log.write(f"command[{name}]={command}\n")
        log.flush()
        process = subprocess.Popen(
            argv,
            cwd=root,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        lines = []
        assert process.stdout is not None
        for line in process.stdout:
            sys.stdout.write(line)
            sys.stdout.flush()
            log.write(line)
            log.flush()
            lines.append(line)
        status = process.wait()
        log.write(f"exit[{name}]={status}\n")
    output = "".join(lines)
    (raw / f"{name}.log").write_text(output, encoding="utf-8")
    if status != 0:
        raise SystemExit(f"{name} failed with exit status {status}")
    return command


def capture(root, raw, name, argv):
    result = subprocess.run(
        argv,
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    output = result.stdout + result.stderr
    (raw / f"{name}.txt").write_text(output, encoding="utf-8")
    return output


def capture_json(root, raw, execution, name, argv):
    result = subprocess.run(
        argv,
        cwd=root,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    with execution.open("a", encoding="utf-8") as log:
        log.write(f"command[{name}]={command_text(argv)}\n")
        log.write(result.stderr)
        log.write(f"exit[{name}]={result.returncode}\n")
    if result.returncode != 0:
        raise SystemExit(f"{name} failed with exit status {result.returncode}")
    path = raw / f"{name}.json"
    path.write_text(result.stdout, encoding="utf-8")
    return json.loads(result.stdout), command_text(argv)


def assert_test_log(name, output, expected, edge_case_rows=None):
    lowered = output.lower()
    if "skipping" in lowered:
        raise SystemExit(f"{name} reported a skipped path")
    for test in expected:
        if not re.search(rf"PASS[^\n]*{re.escape(test)}", output):
            raise SystemExit(f"{name} did not record PASS for {test}")
    if edge_case_rows is not None:
        rows = [line for line in output.splitlines() if "device-min-sum algorithm=" in line]
        if len(rows) != edge_case_rows:
            raise SystemExit(
                f"{name} recorded {len(rows)} device edge-case rows, expected {edge_case_rows}"
            )


def suite_executables(root, listing):
    executables = {}
    for suite, record in listing["rust-suites"].items():
        path = pathlib.Path(record["binary-path"]).resolve()
        executables[suite] = {
            "path": str(path.relative_to(root)),
            "sha256": sha256(path),
        }
    return executables


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=pathlib.Path)
    args = parser.parse_args()

    root = pathlib.Path(
        subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
    )
    output = args.output
    if output.is_absolute():
        output = output.relative_to(root)
    if not str(output).startswith(str(OUTPUT_PREFIX)):
        raise SystemExit(f"output must start with {OUTPUT_PREFIX}")
    output = root / output
    if output.exists():
        raise SystemExit(f"refusing to overwrite existing evidence: {output}")
    raw = output / "raw"
    raw.mkdir(parents=True)
    execution = output / "execution.log"
    execution.write_text(
        "schema=ldpc-device-conformance-execution-v1\n"
        "purpose=correctness-only; no performance measurement\n",
        encoding="utf-8",
    )

    subprocess.run(
        ["git", "diff", "--exit-code", "HEAD", "--", *map(str, SOURCES)],
        cwd=root,
        check=True,
    )
    revision = capture(root, raw, "source-revision", ["git", "rev-parse", "HEAD"]).strip()
    status = capture(root, raw, "git-status", ["git", "status", "--porcelain=v1"])
    rustc = capture(root, raw, "rustc", ["rustc", "+1.95", "--version", "--verbose"])
    hipcc = capture(root, raw, "hipcc", ["/opt/rocm/bin/hipcc", "--version"])
    nextest = capture(root, raw, "nextest", ["cargo", "nextest", "--version"])
    uname = capture(root, raw, "uname", ["uname", "-a"]).strip()
    rocminfo = capture(root, raw, "rocminfo", ["/opt/rocm/bin/rocminfo"])
    plain_rocminfo = re.sub(r"\x1b\[[0-9;]*m", "", rocminfo)
    if not re.search(r"Name:\s+gfx1030\b", plain_rocminfo):
        raise SystemExit("rocminfo does not report a gfx1030 device")
    marketing_match = re.search(
        r"Name:\s+gfx1030\b.*?Marketing Name:\s+([^\n]+)",
        plain_rocminfo,
        re.DOTALL,
    )
    marketing_name = marketing_match.group(1).strip() if marketing_match else "unresolved"

    commands = []
    device_args = [
        "./scripts/cargo-budget.sh", "--test", "cargo", "+1.95", "nextest", "run",
        "--manifest-path", "crates/gf2-kernels-hip/Cargo.toml", "--release",
        "--features", "hip", "--lib", "--color", "never",
        "--no-capture", "-E", "test(device_min_sum_matches_cpu_on_signed_zero_and_nan)",
    ]
    device_output = run_logged(
        root, raw, execution, "device-edge-cases", device_args, {"GF2_REQUIRE_GPU": "1"}
    )
    commands.append(command_text(device_args))
    assert_test_log("device-edge-cases", device_output, EXPECTED_TESTS["device"], 18)

    slow_args = [
        "./scripts/cargo-budget.sh", "--test", "cargo", "+1.95", "nextest", "run",
        "-p", "gf2-sim", "--features", "hip", "--release", "--profile", "slow",
        "--run-ignored", "ignored-only", "--color", "never",
        "--test", "gpu_ldpc_byte_identity", "--test", "gpu_byte_identity",
        "--test", "gpu_nr_5g_byte_identity",
    ]
    slow_output = run_logged(root, raw, execution, "ordinary-slow", slow_args)
    commands.append(command_text(slow_args))
    assert_test_log("ordinary-slow", slow_output, EXPECTED_TESTS["ordinary_slow"])

    smoke_args = [
        "./scripts/cargo-budget.sh", "--test", "cargo", "+1.95", "nextest", "run",
        "-p", "gf2-sim", "--features", "hip", "--release", "--profile", "ci",
        "--color", "never", "--test", "gpu_nr_5g_byte_identity",
        "-E", "test(gpu_nr_5g_smoke_byte_identical_to_cpu)",
    ]
    smoke_output = run_logged(root, raw, execution, "ordinary-smoke", smoke_args)
    commands.append(command_text(smoke_args))
    assert_test_log("ordinary-smoke", smoke_output, EXPECTED_TESTS["ordinary_smoke"])

    device_list_args = [
        "./scripts/cargo-budget.sh", "cargo", "+1.95", "nextest", "list",
        "--manifest-path", "crates/gf2-kernels-hip/Cargo.toml", "--release",
        "--features", "hip", "--lib", "--message-format", "json", "--color", "never",
    ]
    device_listing, device_list_command = capture_json(
        root, raw, execution, "device-executables", device_list_args
    )
    commands.append(device_list_command)
    ordinary_list_args = [
        "./scripts/cargo-budget.sh", "cargo", "+1.95", "nextest", "list",
        "-p", "gf2-sim", "--features", "hip", "--release",
        "--test", "gpu_ldpc_byte_identity", "--test", "gpu_byte_identity",
        "--test", "gpu_nr_5g_byte_identity", "--message-format", "json", "--color", "never",
    ]
    ordinary_listing, ordinary_list_command = capture_json(
        root, raw, execution, "ordinary-executables", ordinary_list_args
    )
    commands.append(ordinary_list_command)

    sources = {str(path): sha256(root / path) for path in SOURCES}
    raw_files = {
        str(path.relative_to(output)): sha256(path)
        for path in sorted(raw.iterdir())
        if path.is_file()
    }
    receipt = {
        "schema": "ldpc-device-conformance-v1",
        "issue": ISSUE,
        "purpose": "correctness validation; test-runner durations are not performance measurements",
        "observed_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
        "verdict": "pass",
        "source_identity": {
            "revision_informational": revision,
            "files": sources,
            "git_status_informational": status.splitlines(),
        },
        "toolchain": {
            "rustc": rustc.strip(),
            "hipcc": hipcc.strip(),
            "nextest": nextest.strip(),
            "host": uname,
        },
        "device": {
            "target": "gfx1030",
            "marketing_name": marketing_name,
            "rocminfo": "raw/rocminfo.txt",
            "rocminfo_sha256": sha256(raw / "rocminfo.txt"),
        },
        "executables": {
            "device_edge_cases": suite_executables(root, device_listing),
            "ordinary_byte_identity": suite_executables(root, ordinary_listing),
        },
        "tests": {
            "device_edge_cases": {
                "verdict": "pass",
                "comparison": "bitwise f32 output equality against min_sum_check_row",
                "algorithms": ["MinSum", "NormalizedMinSum(0.75)", "OffsetMinSum(0.5)"],
                "cases": [
                    "negative-zero", "both-zeros", "positive-nan", "negative-nan",
                    "all-nan", "nan-and-negative-zero",
                ],
                "device_rows": 18,
                "log": "raw/device-edge-cases.log",
            },
            "ordinary_slow": {
                "verdict": "pass",
                "names": EXPECTED_TESTS["ordinary_slow"],
                "log": "raw/ordinary-slow.log",
            },
            "ordinary_smoke": {
                "verdict": "pass",
                "names": EXPECTED_TESTS["ordinary_smoke"],
                "log": "raw/ordinary-smoke.log",
            },
        },
        "commands": commands,
        "raw_files": raw_files,
    }
    receipt_path = output / "receipt.json"
    receipt_path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    execution_sha = sha256(execution)
    summary = f"""# LDPC gfx1030 device conformance (jit:{ISSUE})

> **Diátaxis Type:** Reference

This correctness receipt validates the current HIP min-sum check update on the
{marketing_name} (`gfx1030`). It interprets no runtime duration as a performance
measurement. Source and executable content identities are in `receipt.json`.

## Verdict

- The device check-update kernel matches `min_sum_check_row` bit for bit for
  MinSum, NormalizedMinSum(0.75), and OffsetMinSum(0.5).
- The device matrix covers positive zero, negative zero, positive-sign NaN,
  negative-sign NaN, an all-NaN excluded set, and NaN with negative zero.
- The five ignored ordinary DVB-T2/5G NR correctness legs pass.
- The unignored 5G NR smoke correctness leg passes.

The raw transcript is `execution.log` (SHA-256 `{execution_sha}`). Per-command
output, toolchain reports, runtime `rocminfo`, and nextest executable manifests
are under `raw/`; their digests are recorded in `receipt.json`.
"""
    (output / "summary.md").write_text(summary, encoding="utf-8")
    print(receipt_path)
    print(output / "summary.md")


if __name__ == "__main__":
    main()
