#!/usr/bin/env python3
"""Times the per-file and batch forms of two committed-blob readers.

`dev/scripts/check-receipt-input-snapshots.py`'s `committed_reader` and
`dev/active/a203a23c/restore-receipt-inputs.py`'s `sources_by_digest` read
committed blobs by path. A per-file form spawns one `git cat-file blob`
process per read; a batch form serves every read through one `git cat-file
--batch` process. Commit 76812a4e6a0dd6ecf499dc98e8d3a3b052c50a31 carries the
batch form; its parent carries the per-file form. This script times both,
against the same workload revision, without writing to the repository: it
loads each form's exact historical source with `git show <revision>:<path>`,
`exec`s it as an in-memory module (no file is written, no bytecode cache is
created), and calls `check()` and `sources_by_digest()` directly against the
real repository's committed objects. Both functions are read-only;
`sources_by_digest`'s caller in `restore-receipt-inputs.py` also copies
actually-missing files into the working tree and stages them, which this
script does not do, since it must make no repository writes.

`check()` is the direct proxy for
`dev/scripts/check-receipt-input-snapshots.py`'s wall time.
`check() + sources_by_digest()` (summed per repetition index, not re-measured
as one call) is the proxy for `dev/active/a203a23c/restore-receipt-inputs.py`,
since `sources_by_digest` is that script's dominant cost: it hashes every file
the workload revision tracks, not only the files receipts pin. Excluded from
the restoration proxy is a second, redundant read of the pinned files
`restore-receipt-inputs.py`'s own `main()` performs while assembling its pins
dictionary (same mechanism and revision as `check()`, so its cost is already
represented, just not summed a second time) and the small file-copy tail
(unchanged between the two revisions).

Usage:
  dev/active/a203a23c/timing/measure.py [--repetitions N] [--revision REV]

Prints one JSON record to stdout. Run under
`dev/scripts/ccx1-bench-flock.sh --full-host` so host contention does not
distort the wall times; see dev/active/a203a23c/timing/README.md.
"""

from __future__ import annotations

import argparse
import json
import statistics
import subprocess
import sys
import time
import types
from pathlib import Path

sys.dont_write_bytecode = True

BATCH_REVISION = "76812a4e6a0dd6ecf499dc98e8d3a3b052c50a31"
CHECKER_PATH = "dev/scripts/check-receipt-input-snapshots.py"
RESTORE_PATH = "dev/active/a203a23c/restore-receipt-inputs.py"


def run(*arguments: str) -> str:
    return subprocess.run(
        list(arguments), capture_output=True, text=True, check=True
    ).stdout


def repo_root() -> Path:
    return Path(run("git", "rev-parse", "--show-toplevel").strip())


def load_module_from_revision(root: Path, revision: str, path: str, name: str):
    """Execs the committed content of `path` at `revision` as an in-memory module.

    No file is written and no bytecode cache is created; `git show` supplies the
    source directly from the object database.
    """
    source = subprocess.run(
        ["git", "-C", str(root), "show", f"{revision}:{path}"],
        capture_output=True,
        check=True,
    ).stdout.decode()
    module = types.ModuleType(name)
    module.__file__ = f"<git {revision}:{path}>"
    exec(compile(source, module.__file__, "exec"), module.__dict__)
    return module


def time_calls(fn, repetitions: int) -> list[float]:
    samples = []
    for _ in range(repetitions):
        start = time.perf_counter()
        fn()
        samples.append(time.perf_counter() - start)
    return samples


def read_text_or_none(path: str) -> str | None:
    try:
        return Path(path).read_text()
    except OSError:
        return None


def collect_host() -> dict:
    cpuinfo = Path("/proc/cpuinfo").read_text()
    cpu_model = next(
        (
            line.split(":", 1)[1].strip()
            for line in cpuinfo.splitlines()
            if line.split(":", 1)[0].strip() == "model name"
        ),
        None,
    )
    governors = {}
    cpu_root = Path("/sys/devices/system/cpu")
    for entry in sorted(cpu_root.glob("cpu[0-9]*")):
        governor_file = entry / "cpufreq/scaling_governor"
        value = read_text_or_none(str(governor_file))
        if value is not None:
            governors[entry.name] = value.strip()
    smt_raw = read_text_or_none("/sys/devices/system/cpu/smt/active")
    smt_active = {"1": True, "0": False}.get(smt_raw.strip()) if smt_raw else None
    return {
        "hostname": run("hostname").strip(),
        "cpu_model": cpu_model,
        "cpu_count": len(list(cpu_root.glob("cpu[0-9]*"))),
        "os_kernel": run("uname", "-sr").strip(),
        "governors": governors,
        "smt_active": smt_active,
    }


def collect_toolchain() -> dict:
    return {
        "python3": sys.version.split()[0],
        "git": run("git", "--version").strip(),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repetitions", type=int, default=5)
    parser.add_argument("--revision", default=BATCH_REVISION)
    arguments = parser.parse_args()

    root = repo_root()
    batch_revision = arguments.revision
    per_file_revision = run(
        "git", "-C", str(root), "rev-parse", f"{batch_revision}^"
    ).strip()

    check_per_file = load_module_from_revision(
        root, per_file_revision, CHECKER_PATH, "check_per_file"
    )
    check_batch = load_module_from_revision(
        root, batch_revision, CHECKER_PATH, "check_batch"
    )
    restore_per_file = load_module_from_revision(
        root, per_file_revision, RESTORE_PATH, "restore_per_file"
    )
    restore_batch = load_module_from_revision(
        root, batch_revision, RESTORE_PATH, "restore_batch"
    )

    reps = arguments.repetitions
    checker_check = {
        "per_file": time_calls(
            lambda: check_per_file.check(root, batch_revision), reps
        ),
        "batch": time_calls(lambda: check_batch.check(root, batch_revision), reps),
    }
    restoration_sources_by_digest = {
        "per_file": time_calls(
            lambda: restore_per_file.sources_by_digest(
                check_per_file, root, batch_revision
            ),
            reps,
        ),
        "batch": time_calls(
            lambda: restore_batch.sources_by_digest(
                check_batch, root, batch_revision
            ),
            reps,
        ),
    }
    restoration_proxy_total = {
        form: [
            checker_check[form][index] + restoration_sources_by_digest[form][index]
            for index in range(reps)
        ]
        for form in ("per_file", "batch")
    }

    def stats(series: list[float]) -> dict:
        return {
            "repetitions_s": series,
            "min_s": min(series),
            "median_s": statistics.median(series),
        }

    record = {
        "schema": "a203a23c-cat-file-batch-timing-v1",
        "purpose": (
            "Wall time of the per-file and batch git cat-file mechanisms "
            "76812a4e6a0dd6ecf499dc98e8d3a3b052c50a31 replaced, measured "
            "directly against the committed object database without writing "
            "to the repository."
        ),
        "generator": "dev/active/a203a23c/timing/measure.py",
        "wrapper": "dev/scripts/ccx1-bench-flock.sh --full-host",
        "command": "python3 -B dev/active/a203a23c/timing/measure.py "
        + " ".join(sys.argv[1:]),
        "repetitions": reps,
        "workload_revision": batch_revision,
        "per_file_source_revision": per_file_revision,
        "batch_source_revision": batch_revision,
        "host": collect_host(),
        "toolchain": collect_toolchain(),
        "measurements": {
            "checker_check": {
                "tool": CHECKER_PATH,
                "function": "check()",
                "per_file": stats(checker_check["per_file"]),
                "batch": stats(checker_check["batch"]),
            },
            "restoration_sources_by_digest": {
                "tool": RESTORE_PATH,
                "function": "sources_by_digest()",
                "per_file": stats(restoration_sources_by_digest["per_file"]),
                "batch": stats(restoration_sources_by_digest["batch"]),
            },
            "restoration_proxy_total": {
                "tool": RESTORE_PATH,
                "function": "check() + sources_by_digest(), summed per repetition",
                "per_file": stats(restoration_proxy_total["per_file"]),
                "batch": stats(restoration_proxy_total["batch"]),
            },
        },
    }
    print(json.dumps(record, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
