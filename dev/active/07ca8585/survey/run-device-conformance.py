#!/usr/bin/env python3
"""Run, record, and package correctness-only LDPC GPU conformance (jit:07ca8585).

The output is a durable receipt, raw device/toolchain reports, exact test logs,
and a portable snapshot of every source/build input that decides the verdict.
Test-runner durations are incidental diagnostics and are not interpreted as
measurements.
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
PRODUCING_MANIFEST = pathlib.Path(
    "dev/active/07ca8585/survey/producing-inputs-device-conformance.json"
)
PRODUCING_ROOT = pathlib.Path("inputs/producing")
RUNNER = pathlib.Path("dev/active/07ca8585/survey/run-device-conformance.py")
SOURCE_TREES = [
    pathlib.Path("crates/gf2-core/src"),
    pathlib.Path("crates/gf2-coding/src"),
    pathlib.Path("crates/gf2-algebra/src"),
    pathlib.Path("crates/gf2-stats/src"),
    pathlib.Path("crates/gf2-kernels-simd/src"),
    pathlib.Path("crates/gf2-kernels-hip/src"),
    pathlib.Path("crates/gf2-kernels-hip/hip"),
    pathlib.Path("crates/gf2-sim/src"),
]
TEST_SOURCES = [
    pathlib.Path("crates/gf2-coding/tests/ldpc_check_update_contract.rs"),
    pathlib.Path("crates/gf2-sim/tests/common/mod.rs"),
    pathlib.Path("crates/gf2-sim/tests/gpu_ldpc_byte_identity.rs"),
    pathlib.Path("crates/gf2-sim/tests/gpu_byte_identity.rs"),
    pathlib.Path("crates/gf2-sim/tests/gpu_nr_5g_byte_identity.rs"),
]
LIFECYCLE_SOURCES = [
    pathlib.Path(".config/nextest.toml"),
    RUNNER,
    pathlib.Path("scripts/cargo-budget.sh"),
]
BUILD_ONLY = [
    pathlib.Path(".cargo/config.toml"),
    pathlib.Path("Cargo.lock"),
    pathlib.Path("Cargo.toml"),
    pathlib.Path("crates/gf2-algebra/Cargo.toml"),
    pathlib.Path("crates/gf2-coding/Cargo.toml"),
    pathlib.Path("crates/gf2-core/Cargo.toml"),
    pathlib.Path("crates/gf2-kernels-hip/Cargo.lock"),
    pathlib.Path("crates/gf2-kernels-hip/Cargo.toml"),
    pathlib.Path("crates/gf2-kernels-simd/Cargo.toml"),
    pathlib.Path("crates/gf2-sim/Cargo.toml"),
    pathlib.Path("crates/gf2-stats/Cargo.toml"),
]
# The standalone HIP crate's ignored lockfile is nevertheless an executable-
# producing input. It is captured from the working tree; every other input is
# read from the source revision recorded by the receipt.
WORKTREE_BUILD_INPUTS = {pathlib.Path("crates/gf2-kernels-hip/Cargo.lock")}
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


def sha256_bytes(content):
    return hashlib.sha256(content).hexdigest()


def repository_root():
    return pathlib.Path(
        subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
    )


def tracked_tree_files(root, revision):
    listing = subprocess.run(
        ["git", "ls-tree", "-r", "--name-only", revision, "--", *map(str, SOURCE_TREES)],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.splitlines()
    return [pathlib.Path(path) for path in listing]


def producing_manifest(root, revision="HEAD"):
    behavior = set(tracked_tree_files(root, revision))
    behavior.update(TEST_SOURCES)
    behavior.update(LIFECYCLE_SOURCES)
    behavior.update(
        [
            pathlib.Path("crates/gf2-kernels-hip/build.rs"),
            pathlib.Path("crates/gf2-sim/build.rs"),
        ]
    )
    build_inputs = behavior | set(BUILD_ONLY)
    for path in sorted(build_inputs):
        if not (root / path).is_file():
            raise SystemExit(f"producing input is not a file: {path}")
    return {
        "schema": "tuning-campaign-producing-inputs-v1",
        "behavior_sources": [str(path) for path in sorted(behavior, key=str)],
        "lifecycle_sources": [str(path) for path in sorted(LIFECYCLE_SOURCES, key=str)],
        "build_inputs": [str(path) for path in sorted(build_inputs, key=str)],
    }


def write_manifest(root):
    document = producing_manifest(root)
    (root / PRODUCING_MANIFEST).write_text(
        json.dumps(document, indent=2) + "\n", encoding="utf-8"
    )
    print(
        f"{PRODUCING_MANIFEST}: {len(document['behavior_sources'])} behavior, "
        f"{len(document['lifecycle_sources'])} lifecycle, "
        f"{len(document['build_inputs'])} build inputs"
    )


def read_manifest(root):
    manifest = json.loads((root / PRODUCING_MANIFEST).read_text(encoding="utf-8"))
    if manifest.get("schema") != "tuning-campaign-producing-inputs-v1":
        raise SystemExit("producing-input manifest schema mismatch")
    groups = [
        manifest.get("behavior_sources"),
        manifest.get("lifecycle_sources"),
        manifest.get("build_inputs"),
    ]
    for group in groups:
        if not isinstance(group, list) or not group or group != sorted(set(group)):
            raise SystemExit("producing-input paths must be nonempty, sorted, and unique")
        for path in group:
            candidate = pathlib.PurePosixPath(path)
            if candidate.is_absolute() or ".." in candidate.parts:
                raise SystemExit(f"producing input is not repository-relative: {path}")
    behavior, lifecycle, build_inputs = map(set, groups)
    if not lifecycle <= behavior or not behavior <= build_inputs:
        raise SystemExit("lifecycle/behavior inputs are not nested subsets")
    return manifest


def revision_bytes(root, revision, path):
    path = pathlib.Path(path)
    if path in WORKTREE_BUILD_INPUTS:
        return (root / path).read_bytes()
    return subprocess.run(
        ["git", "show", f"{revision}:{path.as_posix()}"],
        cwd=root,
        check=True,
        capture_output=True,
    ).stdout


def prepare_snapshot(root, revision):
    manifest = read_manifest(root)
    content = {
        path: revision_bytes(root, revision, path) for path in manifest["build_inputs"]
    }
    hashes = {path: sha256_bytes(data) for path, data in content.items()}
    snapshot = {
        "manifest_path": str(PRODUCING_MANIFEST),
        "manifest_sha256": sha256(root / PRODUCING_MANIFEST),
        "behavior_sha256": {
            path: hashes[path] for path in manifest["behavior_sources"]
        },
        "lifecycle_sha256": {
            path: hashes[path] for path in manifest["lifecycle_sources"]
        },
        "build_inputs_sha256": hashes,
    }
    return snapshot, content


def verify_snapshot(output, expected):
    snapshot_root = output / PRODUCING_ROOT
    observed = json.loads(
        (snapshot_root / "producing-snapshot.json").read_text(encoding="utf-8")
    )
    if observed != expected:
        raise SystemExit("producing snapshot marker differs from expected identity")
    if expected.get("manifest_path") != str(PRODUCING_MANIFEST):
        raise SystemExit("producing snapshot names another manifest")
    selected = {expected["manifest_path"]: expected["manifest_sha256"]}
    selected.update(expected["build_inputs_sha256"])
    for path, digest in selected.items():
        candidate = snapshot_root / path
        if not candidate.is_file() or sha256(candidate) != digest:
            raise SystemExit(f"producing snapshot content differs: {path}")
    manifest = json.loads(
        (snapshot_root / expected["manifest_path"]).read_text(encoding="utf-8")
    )
    behavior = {
        path: expected["build_inputs_sha256"][path]
        for path in manifest["behavior_sources"]
    }
    lifecycle = {
        path: expected["build_inputs_sha256"][path]
        for path in manifest["lifecycle_sources"]
    }
    if behavior != expected["behavior_sha256"]:
        raise SystemExit("producing snapshot behavior selection differs from its manifest")
    if lifecycle != expected["lifecycle_sha256"]:
        raise SystemExit("producing snapshot lifecycle selection differs from its manifest")


def publish_snapshot(root, output, revision):
    snapshot_root = output / PRODUCING_ROOT
    if (snapshot_root / "producing-snapshot.json").exists():
        expected = json.loads(
            (snapshot_root / "producing-snapshot.json").read_text(encoding="utf-8")
        )
        verify_snapshot(output, expected)
        return expected
    if snapshot_root.exists():
        raise SystemExit(f"refusing to overwrite partial producing snapshot: {snapshot_root}")
    snapshot, content = prepare_snapshot(root, revision)
    snapshot_root.mkdir(parents=True)
    manifest_destination = snapshot_root / PRODUCING_MANIFEST
    manifest_destination.parent.mkdir(parents=True)
    manifest_destination.write_bytes((root / PRODUCING_MANIFEST).read_bytes())
    for path, data in content.items():
        destination = snapshot_root / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
    (snapshot_root / "producing-snapshot.json").write_text(
        json.dumps(snapshot, separators=(",", ":")), encoding="utf-8"
    )
    verify_snapshot(output, snapshot)
    return snapshot


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
    return output


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


def producing_pointer(output, snapshot):
    marker = output / PRODUCING_ROOT / "producing-snapshot.json"
    return {
        "snapshot": str(PRODUCING_ROOT / "producing-snapshot.json"),
        "sha256": sha256(marker),
        "manifest_path": snapshot["manifest_path"],
        "manifest_sha256": snapshot["manifest_sha256"],
    }


def verify_existing_evidence(root, output, receipt):
    if receipt.get("schema") != "ldpc-device-conformance-v1":
        raise SystemExit("existing receipt schema mismatch")
    if receipt.get("verdict") != "pass":
        raise SystemExit("existing receipt does not record a passing verdict")
    raw = output / "raw"
    for path, digest in receipt.get("raw_files", {}).items():
        candidate = output / path
        if not candidate.is_file() or sha256(candidate) != digest:
            raise SystemExit(f"recorded raw input differs: {path}")
    rocminfo = output / receipt["device"]["rocminfo"]
    if sha256(rocminfo) != receipt["device"]["rocminfo_sha256"]:
        raise SystemExit("recorded rocminfo differs")
    assert_test_log(
        "device-edge-cases",
        (raw / "device-edge-cases.log").read_text(encoding="utf-8"),
        EXPECTED_TESTS["device"],
        18,
    )
    assert_test_log(
        "ordinary-slow",
        (raw / "ordinary-slow.log").read_text(encoding="utf-8"),
        EXPECTED_TESTS["ordinary_slow"],
    )
    assert_test_log(
        "ordinary-smoke",
        (raw / "ordinary-smoke.log").read_text(encoding="utf-8"),
        EXPECTED_TESTS["ordinary_smoke"],
    )
    for suites in receipt.get("executables", {}).values():
        for name, executable in suites.items():
            path = root / executable["path"]
            if not path.is_file() or sha256(path) != executable["sha256"]:
                raise SystemExit(f"recorded executable is unavailable or differs: {name}")


def package_existing(root, output):
    receipt_path = output / "receipt.json"
    if not receipt_path.is_file():
        raise SystemExit(f"existing receipt is absent: {receipt_path}")
    receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
    verify_existing_evidence(root, output, receipt)
    revision = receipt["source_identity"]["revision_informational"]
    snapshot = publish_snapshot(root, output, revision)
    for path, digest in receipt["source_identity"]["files"].items():
        if snapshot["behavior_sha256"].get(path) != digest:
            raise SystemExit(f"producing snapshot disagrees with recorded source: {path}")
    receipt["producing_inputs"] = producing_pointer(output, snapshot)
    receipt_path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")

    summary_path = output / "summary.md"
    summary = summary_path.read_text(encoding="utf-8")
    old = "Source and executable content identities are in `receipt.json`."
    new = old + "\nThe portable producing-input closure is under `inputs/producing/`."
    if new in summary:
        pass
    elif old in summary:
        summary_path.write_text(summary.replace(old, new), encoding="utf-8")
    else:
        raise SystemExit("evidence summary does not contain the provenance paragraph")
    print(receipt_path)
    print(output / PRODUCING_ROOT / "producing-snapshot.json")


def resolve_output(root, output):
    if output is None:
        raise SystemExit("an output path is required")
    if output.is_absolute():
        output = output.relative_to(root)
    if not str(output).startswith(str(OUTPUT_PREFIX)):
        raise SystemExit(f"output must start with {OUTPUT_PREFIX}")
    return root / output


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=pathlib.Path, nargs="?")
    parser.add_argument(
        "--write-manifest",
        action="store_true",
        help="regenerate the committed device-conformance producing-input manifest",
    )
    parser.add_argument(
        "--package-existing",
        action="store_true",
        help="verify and package retained evidence without executing device tests",
    )
    args = parser.parse_args()

    root = repository_root()
    if args.write_manifest:
        if args.output is not None or args.package_existing:
            raise SystemExit("--write-manifest does not accept an output or another mode")
        write_manifest(root)
        return
    output = resolve_output(root, args.output)
    if args.package_existing:
        package_existing(root, output)
        return
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

    manifest = read_manifest(root)
    tracked_inputs = [
        path for path in manifest["build_inputs"] if pathlib.Path(path) not in WORKTREE_BUILD_INPUTS
    ]
    subprocess.run(
        ["git", "ls-files", "--error-unmatch", str(PRODUCING_MANIFEST)],
        cwd=root,
        check=True,
        capture_output=True,
    )
    subprocess.run(
        [
            "git", "diff", "--exit-code", "HEAD", "--", str(PRODUCING_MANIFEST),
            *tracked_inputs,
        ],
        cwd=root,
        check=True,
    )
    revision = capture(root, raw, "source-revision", ["git", "rev-parse", "HEAD"]).strip()
    snapshot = publish_snapshot(root, output, revision)
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
            "files": snapshot["behavior_sha256"],
            "git_status_informational": status.splitlines(),
        },
        "producing_inputs": producing_pointer(output, snapshot),
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
The portable producing-input closure is under `inputs/producing/`.

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
