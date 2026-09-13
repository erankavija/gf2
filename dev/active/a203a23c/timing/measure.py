#!/usr/bin/env python3
"""Times both receipt-input tools end to end, per-file form against batch form.

Commit 76812a4e6a0dd6ecf499dc98e8d3a3b052c50a31 replaces one `git cat-file
blob` process per committed-blob read with a single `git cat-file --batch`
process serving every read, in `dev/scripts/check-receipt-input-snapshots.py`
and in `dev/active/a203a23c/restore-receipt-inputs.py` alike. Its message
claims a wall time for each whole tool in each form. This harness measures that
quantity: the time from process start to process exit of

    python3 -B <tool source of the form's revision> <the tool's arguments>

for both forms of both tools, against one fixed workload. The per-file form is
the source committed at 76812a4e's parent, a6427615adf607caca5e4adb3be917a20f18891f;
the batch form is the source committed at 76812a4e itself. Each form's source
is read out of the object database with `git show <revision>:<path>` into a
scratch directory outside this repository, so the repository's own checkout is
never rewritten to run either form.

Four series run in the fixed order the sampling plan declares:

1. `checker_per_file` and 2. `checker_batch` run the checker against this
   repository's committed objects, with `--revision` naming the workload
   revision, so the working tree's contents cannot reach the verdict or the
   time. The checker writes nothing.

3. `restoration_per_file` and 4. `restoration_batch` run the restoration
   generator, which copies missing snapshot files into its working tree and
   stages them. They therefore run against a scratch `git clone --shared` of
   this repository checked out at 8cfc015de59f8e3accc51bca46d13d94b0bee267, the
   revision whose snapshots are missing the files the tool restores and the
   `base_revision` recorded in `dev/active/a203a23c/restored-inputs.json`. The
   clone is reset with `git reset --hard` and `git clean -fdx` before every
   repetition, so every repetition performs the whole restoration, and the
   form's two source files are placed into the clone afterwards, outside the
   timed region. Nothing is written to this repository.

The per-file and batch forms of the restoration generator differ in the checker
they load as well as in their own source: `restore-receipt-inputs.py` imports
`dev/scripts/check-receipt-input-snapshots.py` from its working tree, so both
files of the form's revision are placed in the clone together.

Usage:
  dev/active/a203a23c/timing/measure.py --scratch <directory> [--repetitions N]

Prints one JSON record to stdout. Run from the repository root under
`dev/scripts/ccx1-bench-flock.sh --full-host` so host contention does not
distort the wall times; see dev/active/a203a23c/timing/README.md.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path

sys.dont_write_bytecode = True

BATCH_REVISION = "76812a4e6a0dd6ecf499dc98e8d3a3b052c50a31"
RESTORE_BASE_REVISION = "8cfc015de59f8e3accc51bca46d13d94b0bee267"
CHECKER_PATH = "dev/scripts/check-receipt-input-snapshots.py"
RESTORE_PATH = "dev/active/a203a23c/restore-receipt-inputs.py"

SERIES_ORDER = (
    "checker_per_file",
    "checker_batch",
    "restoration_per_file",
    "restoration_batch",
)

SAMPLING_PLAN = {
    "declared_in": "dev/active/a203a23c/timing/README.md",
    "repetitions_per_series": 5,
    "statistic_of_record": ["min_s", "median_s"],
    "early_stopping": (
        "none: every series runs its full five repetitions, every repetition "
        "enters the statistics, and no repetition is dropped as an outlier"
    ),
    "series_order": list(SERIES_ORDER),
    "warm_up": (
        "none: no repetition is discarded as a warm-up. The series order is "
        "fixed instead, so each series meets the same preceding sequence of "
        "reads on every run of this harness"
    ),
    "interleaving": (
        "none: a series' repetitions run consecutively, and the forms are "
        "compared across series rather than repetition by repetition"
    ),
}

RNG = {
    "used": False,
    "statement": (
        "No random number generator takes part. This harness draws no random "
        "numbers and fixes the workload, the arguments and the series order as "
        "constants. Neither tool imports a random number generator in either "
        "form; the checker's only randomness-bearing import, `tempfile`, is "
        "reached solely from its `--self-test` path, which these invocations "
        "do not take. Every repetition therefore runs the same work, and the "
        "spread between repetitions is host timing noise alone."
    ),
}

PUBLISHED_FIGURES_S = {
    "checker_per_file": 23.1,
    "checker_batch": 2.6,
    "restoration_per_file": 60.2,
    "restoration_batch": 10.1,
}
PUBLISHED_SOURCE = (
    "commit 76812a4e6a0dd6ecf499dc98e8d3a3b052c50a31's message: "
    '"the repository check falls from 23.1 s to 2.6 s and the restoration '
    'from 60.2 s to 10.1 s"'
)
CONFIRMATION_TOLERANCE = 0.10


def git(root: Path, *arguments: str) -> bytes:
    """Runs one git command in `root` and returns its raw stdout."""
    return subprocess.run(
        ["git", "-C", str(root), *arguments], capture_output=True, check=True
    ).stdout


def run(*arguments: str) -> str:
    return subprocess.run(
        list(arguments), capture_output=True, text=True, check=True
    ).stdout


def repo_root() -> Path:
    return Path(run("git", "rev-parse", "--show-toplevel").strip())


def source_of(root: Path, revision: str, path: str, destination: Path) -> None:
    """Writes the content `path` has at `revision` to `destination`, executable."""
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(git(root, "show", f"{revision}:{path}"))
    destination.chmod(0o755)


def prepare_clone(root: Path, clone: Path, revision: str) -> None:
    """Creates, if absent, a shared clone of `root` detached at `revision`."""
    if not (clone / ".git").exists():
        if clone.exists():
            shutil.rmtree(clone)
        clone.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(
            ["git", "clone", "--shared", "--no-checkout", "-q", str(root), str(clone)],
            check=True,
        )
        git(clone, "checkout", "--detach", "-q", revision)
    head = git(clone, "rev-parse", "HEAD").decode().strip()
    if head != revision:
        raise RuntimeError(f"scratch clone is at {head}, not {revision}")


def reset_clone(clone: Path) -> None:
    """Returns the scratch clone to its detached revision, untracked files gone."""
    git(clone, "reset", "--hard", "-q")
    git(clone, "clean", "-fdxq")


def place_form(root: Path, clone: Path, revision: str) -> None:
    """Puts both tool files of `revision` into the scratch clone's working tree."""
    for path in (CHECKER_PATH, RESTORE_PATH):
        source_of(root, revision, path, clone / path)


def timed_process(argv: list[str], cwd: Path) -> tuple[float, subprocess.CompletedProcess]:
    """Runs `argv` in `cwd` and returns its wall time from start to exit."""
    start = time.perf_counter()
    completed = subprocess.run(argv, cwd=str(cwd), capture_output=True)
    return time.perf_counter() - start, completed


def stats(samples: list[float]) -> dict:
    return {
        "repetitions_s": samples,
        "min_s": min(samples),
        "median_s": statistics.median(samples),
    }


def measure_checker(
    source: Path, root: Path, workload_revision: str, repetitions: int
) -> dict:
    """Times the whole checker process against this repository's committed objects."""
    samples, exit_codes, last_stdout = [], [], ""
    for _ in range(repetitions):
        elapsed, completed = timed_process(
            ["python3", "-B", str(source), "--revision", workload_revision], root
        )
        samples.append(elapsed)
        exit_codes.append(completed.returncode)
        last_stdout = completed.stdout.decode(errors="replace").strip()
    return {
        "tool": CHECKER_PATH,
        "process": f"python3 -B <{CHECKER_PATH} of the form's revision>"
        f" --revision {workload_revision}",
        "measures": "whole process wall time, from start to exit",
        "working_directory": "the root of this repository's checkout",
        "workload": (
            "every input pinned by every campaign receipt committed at the "
            "workload revision, read from this repository's object database; "
            "the checker writes nothing"
        ),
        "exit_codes": exit_codes,
        "final_stdout": last_stdout,
        **stats(samples),
    }


def measure_restoration(
    root: Path, clone: Path, revision: str, base_revision: str, repetitions: int
) -> dict:
    """Times the whole restoration process in a scratch clone, reset per repetition."""
    samples, exit_codes, last_stdout = [], [], ""
    for _ in range(repetitions):
        reset_clone(clone)
        place_form(root, clone, revision)
        elapsed, completed = timed_process(
            ["python3", "-B", RESTORE_PATH, base_revision], clone
        )
        samples.append(elapsed)
        exit_codes.append(completed.returncode)
        last_stdout = completed.stdout.decode(errors="replace").strip()
    reset_clone(clone)
    return {
        "tool": RESTORE_PATH,
        "process": f"python3 -B {RESTORE_PATH} {base_revision}",
        "measures": (
            "whole process wall time, from start to exit: the checker load, the "
            "committed-object listing, the check, the digest index over every "
            "file the base revision tracks, the pinned-file read that assembles "
            "the pins, and the copy-and-stage tail"
        ),
        "working_directory": "the root of the scratch clone",
        "workload": (
            "the snapshot files the base revision's committed receipts pin and "
            "the base revision does not carry, restored into the scratch clone's "
            "working tree and staged there"
        ),
        "exit_codes": exit_codes,
        "final_stdout": last_stdout,
        **stats(samples),
    }


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
        value = read_text_or_none(str(entry / "cpufreq/scaling_governor"))
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


def collect_scheduling() -> dict:
    """Records the scheduling the run achieves, not the one the wrapper requests."""
    loadavg = Path("/proc/loadavg").read_text().split()
    return {
        "requested": (
            "dev/scripts/ccx1-bench-flock.sh requests nice -n -5 and, with "
            "--full-host, no taskset pin"
        ),
        "requested_niceness": -5,
        "observed_niceness": os.nice(0),
        "niceness_note": (
            "`nice: cannot set niceness` is the expected warning for an "
            "unprivileged user; the observed niceness is the one that applies"
        ),
        "cpu_affinity_size": len(os.sched_getaffinity(0)),
        "load_average_at_start": [float(value) for value in loadavg[:3]],
        "runnable_at_start": loadavg[3],
        "mutex": (
            "the run holds /tmp/gf2-ccx1.lock exclusively for its whole "
            "duration, so no other benchmark or build on this host overlaps it"
        ),
    }


def collect_toolchain() -> dict:
    return {
        "python3": sys.version.split()[0],
        "git": run("git", "--version").strip(),
    }


def compare_to_published(series: dict) -> dict:
    """Sets each published figure beside the figure this run measures for it."""
    comparison = {}
    for name, published in PUBLISHED_FIGURES_S.items():
        measured = series[name]
        comparison[name] = {
            "published_s": published,
            "measured_min_s": measured["min_s"],
            "measured_median_s": measured["median_s"],
            "median_over_published": measured["median_s"] / published,
            "confirmed_within_tolerance": abs(measured["median_s"] - published)
            <= CONFIRMATION_TOLERANCE * published,
        }
    return comparison


def sanitized_command(scratch_flag: str) -> str:
    """Renders the invocation with the scratch path replaced by a placeholder."""
    parts, arguments = [], list(sys.argv[1:])
    while arguments:
        argument = arguments.pop(0)
        if argument == scratch_flag:
            parts.extend([argument, "<scratch-directory>"])
            if arguments:
                arguments.pop(0)
        elif argument.startswith(f"{scratch_flag}="):
            parts.append(f"{scratch_flag}=<scratch-directory>")
        else:
            parts.append(argument)
    return "python3 -B dev/active/a203a23c/timing/measure.py " + " ".join(parts)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--scratch",
        required=True,
        help="directory outside this repository for the clone and the tool sources",
    )
    parser.add_argument(
        "--repetitions",
        type=int,
        default=SAMPLING_PLAN["repetitions_per_series"],
        help="repetitions per series; the sampling plan fixes this at 5",
    )
    arguments = parser.parse_args()

    root = repo_root()
    scratch = Path(arguments.scratch).resolve()
    if scratch.is_relative_to(root):
        raise SystemExit("--scratch must name a directory outside this repository")
    repetitions = arguments.repetitions

    batch_revision = BATCH_REVISION
    per_file_revision = git(root, "rev-parse", f"{batch_revision}^").decode().strip()

    sources = scratch / "sources"
    checker_sources = {
        "per_file": sources / per_file_revision / Path(CHECKER_PATH).name,
        "batch": sources / batch_revision / Path(CHECKER_PATH).name,
    }
    source_of(root, per_file_revision, CHECKER_PATH, checker_sources["per_file"])
    source_of(root, batch_revision, CHECKER_PATH, checker_sources["batch"])

    clone = scratch / "clone"
    prepare_clone(root, clone, RESTORE_BASE_REVISION)

    scheduling = collect_scheduling()
    series = {}
    for name in SERIES_ORDER:
        if name == "checker_per_file":
            series[name] = measure_checker(
                checker_sources["per_file"], root, batch_revision, repetitions
            )
            series[name]["source_revision"] = per_file_revision
        elif name == "checker_batch":
            series[name] = measure_checker(
                checker_sources["batch"], root, batch_revision, repetitions
            )
            series[name]["source_revision"] = batch_revision
        elif name == "restoration_per_file":
            series[name] = measure_restoration(
                root, clone, per_file_revision, RESTORE_BASE_REVISION, repetitions
            )
            series[name]["source_revision"] = per_file_revision
        else:
            series[name] = measure_restoration(
                root, clone, batch_revision, RESTORE_BASE_REVISION, repetitions
            )
            series[name]["source_revision"] = batch_revision
        series[name]["form"] = "per-file" if name.endswith("per_file") else "batch"

    record = {
        "schema": "a203a23c-cat-file-batch-timing-v2",
        "purpose": (
            "Whole-process wall time of dev/scripts/check-receipt-input-snapshots.py "
            "and dev/active/a203a23c/restore-receipt-inputs.py, in the per-file "
            "form committed at 76812a4e's parent and the batch form committed at "
            "76812a4e, against one fixed workload."
        ),
        "generator": "dev/active/a203a23c/timing/measure.py",
        "wrapper": "dev/scripts/ccx1-bench-flock.sh --full-host",
        "command": sanitized_command("--scratch"),
        "repetitions": repetitions,
        "sampling_plan": SAMPLING_PLAN,
        "rng": RNG,
        "revisions": {
            "per_file_source": per_file_revision,
            "batch_source": batch_revision,
            "checker_workload": batch_revision,
            "restoration_base": RESTORE_BASE_REVISION,
        },
        "scratch": {
            "location": (
                "a session scratch directory outside this repository; its path "
                "is not recorded because it is checkout-specific"
            ),
            "filesystem": run(
                "stat", "-f", "-c", "%T", str(scratch)
            ).strip(),
            "clone": (
                "git clone --shared --no-checkout of this repository, then "
                f"git checkout --detach {RESTORE_BASE_REVISION}: the working "
                "tree the restoration generator was written against, whose "
                "receipts pin 23 snapshot files the revision does not carry, "
                "and whose objects are this repository's own through the "
                "clone's alternates"
            ),
            "reset_between_repetitions": (
                "git reset --hard and git clean -fdx inside the clone, before "
                "each repetition and once after the last, so every repetition "
                "performs the whole restoration and nothing it writes survives"
            ),
            "tool_sources": (
                "git show <revision>:<path> of both tool files into the clone's "
                "working tree after each reset, outside the timed region"
            ),
        },
        "host": collect_host(),
        "scheduling": scheduling,
        "toolchain": collect_toolchain(),
        "series": series,
        "published_claim": {
            "source": PUBLISHED_SOURCE,
            "tolerance": (
                f"a figure counts as confirmed when the measured median is "
                f"within {CONFIRMATION_TOLERANCE:.0%} of it"
            ),
            "figures": compare_to_published(series),
        },
    }
    print(json.dumps(record, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
