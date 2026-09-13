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
git-ignored scratch directory, so the repository's own checkout is never
rewritten to run either form.

Four series run in the fixed order the sampling plan declares:

1. `checker_per_file` and 2. `checker_batch` run the checker against this
   repository's committed objects, with `--revision` naming the workload
   revision, so the working tree's contents cannot reach the verdict or the
   time. Whether the run left that working tree alone is read from `git status
   --porcelain` around each repetition rather than assumed.

3. `restoration_per_file` and 4. `restoration_batch` run the restoration
   generator, which copies missing snapshot files into its working tree and
   stages them. They therefore run against a scratch `git clone --shared` of
   this repository checked out at 8cfc015de59f8e3accc51bca46d13d94b0bee267, the
   revision whose snapshots are missing the files the tool restores and the
   `base_revision` recorded in `dev/active/a203a23c/restored-inputs.json`. The
   clone is reset with `git reset --hard` and `git clean -fdx` before every
   repetition, so every repetition performs the whole restoration, and the
   form's two source files are placed into the clone afterwards, outside the
   timed region. The scratch directory sits on the filesystem the repository
   sits on, since the tool's real working tree is the repository's own and a
   scratch elsewhere would put a filesystem difference into the wall times.
   Nothing this repository tracks is written.

The per-file and batch forms of the restoration generator differ in the checker
they load as well as in their own source: `restore-receipt-inputs.py` imports
`dev/scripts/check-receipt-input-snapshots.py` from its working tree, so both
files of the form's revision are placed in the clone together.

Each record also carries an assessment of the figures 76812a4e's own message
publishes. This script holds none of them: it cites that commit by identity and
reads every figure out of the commit message at assessment time, refusing to
assess if the sentence the message carries is not in the shape the pattern
names. The same holds of every condition a record states about the run. The
niceness the wrapper asks for is read from the wrapper and set against the
process's own `RLIMIT_NICE`; the exclusive hold on the benchmark mutex is
probed with a non-blocking shared `flock` on a fresh descriptor, and a
measuring run that observes no exclusive holder refuses to start; the wrapper
itself is looked for among this process's ancestors rather than assumed; the
scratch's place in the repository is the path it resolves to and what `git
check-ignore` says of it; the checker's writing nothing is `git status
--porcelain` before and after each repetition; what the restoration stages is
counted in the clone's index; and what could draw a random number is a scan of
the very bytes each form runs. What is written rather than observed is this
harness's own protocol, and `DECLARATION_NOTE`, which every record carries,
names those fields exactly. `--assess` rebuilds
the derived prose on an already committed record, so an assessment can be
re-derived without measuring anything again; the observed fields cannot be
back-filled, and a record written before they existed simply lacks them.

Usage:
  dev/active/a203a23c/timing/measure.py --scratch <directory> [--repetitions N]
  dev/active/a203a23c/timing/measure.py --assess <record.json>

Measuring prints one JSON record to stdout; `--assess` rewrites the named
record in place. Run from the repository root, and run a measurement under
`dev/scripts/ccx1-bench-flock.sh --full-host` so host contention does not
distort the wall times; see dev/active/a203a23c/timing/README.md.
"""

from __future__ import annotations

import argparse
import ast
import fcntl
import json
import os
import re
import resource
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

# The sampling plan and the confirmation tolerance are this harness's own
# protocol constants, declared here and in the README before any run rather
# than observed from one. Every other figure a record carries is observed at
# run time, derived from the record's own samples, or an identity citation.
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

WRAPPER_PATH = "dev/scripts/ccx1-bench-flock.sh"
WRAPPER_NICENESS = re.compile(r"\bnice -n (?P<niceness>-?[0-9]+)\b")
WRAPPER_LOCK = re.compile(r'LOCK_FILE="\$\{GF2_CCX1_LOCK:-(?P<path>[^}"]+)\}"')

# Anything in a scanned source that could draw a random number, including the
# standard library's temporary-file names, which carry random components.
RANDOMNESS_PATTERN = re.compile(
    r"\b(?:import\s+(?:random|secrets|tempfile|numpy)"
    r"|from\s+(?:random|secrets|tempfile|numpy[.\w]*)\s+import"
    r"|numpy\.random|os\.urandom|random\.|secrets\.|tempfile\.)"
)

PUBLISHED_CLAIM_REVISION = BATCH_REVISION
PUBLISHED_SENTENCE = re.compile(
    r"the repository check falls from (?P<checker_per_file>[0-9.]+) s to "
    r"(?P<checker_batch>[0-9.]+) s and the restoration from "
    r"(?P<restoration_per_file>[0-9.]+) s to (?P<restoration_batch>[0-9.]+) s"
)
CONFIRMATION_TOLERANCE = 0.10

DECLARATION_NOTE = (
    "sampling_plan, scratch.reset_between_repetitions, scratch.tool_sources, "
    "scratch.filesystem.why, each series' working_directory and measures, and "
    "each probe's `probe` string declare this harness's own protocol before "
    "the run; purpose and revisions cite commits and paths by identity. Every "
    "other field is observed while the record is written or derived from these "
    "samples and from committed sources the record names."
)
WITHDRAWN = (
    "the run that wrote this record stated this without probing it. The "
    "statement is withdrawn rather than restated, since a condition of a run "
    "that has ended cannot be observed afterwards."
)


def restoration_measures(revision: str) -> str:
    """Names what a restoration series times, by citing the source it ran."""
    return (
        "whole process wall time, from start to exit, of main() in "
        f"{RESTORE_PATH} at {revision}"
    )


def checker_workload(workload_revision: str, source_revision: str) -> str:
    """Names a checker series' workload by citing the revisions that fix it."""
    return (
        f"the committed objects of {workload_revision}, read by main() of "
        f"{CHECKER_PATH} at {source_revision}; what the run reported is at "
        "`final_stdout` and what it left in the working tree is at "
        "`working_tree`"
    )


def restoration_workload(base_revision: str, source_revision: str) -> str:
    """Names a restoration series' workload by citing the revisions that fix it."""
    return (
        f"the scratch clone at {base_revision}, processed by main() of "
        f"{RESTORE_PATH} at {source_revision}; what the run reported is at "
        "`final_stdout` and what it left staged is at `staged`"
    )


def withdrawn(statement: object) -> dict:
    """Marks a statement an earlier record asserted without observing it."""
    return {"observed": False, "note": WITHDRAWN, "withdrawn_statement": statement}


def lock_path(root: Path) -> str:
    """Names the mutex file, from the environment or the wrapper's own default."""
    override = os.environ.get("GF2_CCX1_LOCK")
    if override:
        return override
    default = WRAPPER_LOCK.search((root / WRAPPER_PATH).read_text())
    if default is None:
        raise SystemExit(
            f"{WRAPPER_PATH} names no default lock file: {WRAPPER_LOCK.pattern}"
        )
    return default["path"]


def descriptors_on(path: str) -> list[int]:
    """Lists this process's descriptors already open on `path`."""
    target = os.path.realpath(path)
    found = []
    for entry in Path("/proc/self/fd").iterdir():
        try:
            if os.path.realpath(entry) == target:
                found.append(int(entry.name))
        except (OSError, ValueError):
            continue
    return sorted(found)


def observe_lock(root: Path) -> dict:
    """Probes whether an exclusive holder of the benchmark mutex exists.

    A fresh descriptor's non-blocking shared `flock` fails with EWOULDBLOCK
    exactly when some open file description holds the lock exclusively, this
    process's inherited one included, since a flock lock belongs to the
    description rather than to the process.
    """
    path = lock_path(root)
    inherited = descriptors_on(path)
    try:
        descriptor = os.open(path, os.O_RDONLY)
    except OSError as error:
        return {
            "lock_path": path,
            "exclusive_holder_observed": False,
            "probe": f"opening the lock file failed: {error.strerror}",
            "inherited_descriptors": inherited,
        }
    try:
        try:
            fcntl.flock(descriptor, fcntl.LOCK_SH | fcntl.LOCK_NB)
        except BlockingIOError:
            held, probe = True, (
                "fcntl.flock(LOCK_SH|LOCK_NB) on a fresh descriptor returned "
                "EWOULDBLOCK, so some open file description holds this lock "
                "exclusively for the duration of this run"
            )
        else:
            fcntl.flock(descriptor, fcntl.LOCK_UN)
            held, probe = False, (
                "fcntl.flock(LOCK_SH|LOCK_NB) on a fresh descriptor succeeded, "
                "so no exclusive holder of this lock exists"
            )
    finally:
        os.close(descriptor)
    return {
        "lock_path": path,
        "exclusive_holder_observed": held,
        "probe": probe,
        "inherited_descriptors": inherited,
    }


def observe_wrapper() -> dict:
    """Finds the lock wrapper among this process's ancestors, or reports none."""
    pid, walked = os.getpid(), []
    while pid > 1 and len(walked) < 64:
        try:
            status = Path(f"/proc/{pid}/stat").read_text()
            raw = Path(f"/proc/{pid}/cmdline").read_bytes()
        except OSError:
            break
        argv = [part for part in raw.decode(errors="replace").split("\0") if part]
        if any(Path(WRAPPER_PATH).name in part for part in argv):
            return {
                "observed": True,
                "argv": argv,
                "ancestor_distance": len(walked),
                "full_host": "--full-host" in argv,
            }
        walked.append(pid)
        pid = int(status[status.rindex(")") + 1 :].split()[1])
    return {
        "observed": False,
        "argv": None,
        "ancestor_distance": None,
        "full_host": False,
        "note": (
            f"no ancestor of this process has {Path(WRAPPER_PATH).name} in its "
            "argv, so this run is not wrapped by it"
        ),
    }


def working_tree_status(root: Path) -> list[str]:
    """Lists the porcelain status lines of a repository's working tree."""
    reported = git(root, "status", "--porcelain").decode().splitlines()
    return [line for line in reported if line]


def staged_paths(root: Path) -> list[str]:
    """Lists the paths staged in a repository's index against its HEAD."""
    listed = git(root, "diff", "--cached", "--name-only").decode().splitlines()
    return [line for line in listed if line]


def defined_functions(root: Path, revision: str, path: str) -> list[str]:
    """Names the functions the committed source of `path` at `revision` defines."""
    source = git(root, "show", f"{revision}:{path}").decode()
    return sorted(
        node.name
        for node in ast.parse(source).body
        if isinstance(node, ast.FunctionDef)
    )


def enclosing_function(tree: ast.Module, line: int) -> str | None:
    """Names the innermost function definition containing `line`, if any."""
    innermost = None
    for node in ast.walk(tree):
        if isinstance(node, ast.FunctionDef) and node.lineno <= line <= (
            node.end_lineno or node.lineno
        ):
            if innermost is None or node.lineno > innermost.lineno:
                innermost = node
    return None if innermost is None else innermost.name


def scan_for_randomness(sources: dict) -> dict:
    """Scans committed sources for anything that could draw random numbers.

    `sources` maps a label to the exact bytes this run executes or times, so
    the result describes what ran rather than what the harness believes ran.
    """
    matches = {}
    for label, source in sources.items():
        tree = ast.parse(source)
        found = []
        for number, line in enumerate(source.splitlines(), 1):
            if RANDOMNESS_PATTERN.search(line):
                found.append(
                    {
                        "line": number,
                        "text": line.strip(),
                        "enclosing_function": enclosing_function(tree, number),
                    }
                )
        matches[label] = found
    return matches


def randomness_report(root: Path, revisions: dict) -> dict:
    """Reports what a scan of every source this run executes or times found."""
    sources = {
        "harness:dev/active/a203a23c/timing/measure.py": Path(__file__).read_text()
    }
    for form, revision in revisions.items():
        for path in (CHECKER_PATH, RESTORE_PATH):
            sources[f"{form}:{path}"] = git(root, "show", f"{revision}:{path}").decode()
    matches = scan_for_randomness(sources)
    total = sum(len(found) for found in matches.values())
    functions = sorted(
        {
            hit["enclosing_function"] or "<module level>"
            for found in matches.values()
            for hit in found
        }
    )
    statement = (
        f"Scanning every source this run executes or times, with the pattern "
        f"in `pattern`, found {total} line(s) that could bear randomness. "
        + (
            "No scanned source names a random number generator or a "
            "randomness-bearing import, so no repetition can differ from "
            "another in the work it does."
            if total == 0
            else "Every match is recorded in `matches` with its line number, "
            "its text and the function containing it, here "
            f"{', '.join(functions)}. The arguments each invocation actually "
            "received are at series.<name>.process and what each printed is at "
            "series.<name>.final_stdout."
        )
    )
    return {
        "pattern": RANDOMNESS_PATTERN.pattern,
        "sources_scanned": sorted(sources),
        "randomness_bearing_lines_found": total,
        "matches": matches,
        "statement": statement,
    }


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


def filesystem_of(path: Path) -> dict:
    """Names the filesystem `path` sits on and the device it is mounted from."""
    header, values = run(
        "df", "--output=source,fstype", str(path)
    ).splitlines()[:2]
    del header
    source, fstype = values.split()
    return {"source": source, "fstype": fstype, "device_id": os.stat(path).st_dev}


def check_scratch(root: Path, scratch: Path) -> dict:
    """Locates the scratch against the repository and records what git says of it.

    A scratch inside the repository that git does not ignore would reach the
    repository's own status, so it stops the run.
    """
    if not scratch.is_relative_to(root):
        return {
            "location": "outside this repository",
            "git_check_ignore": "not applicable outside the repository",
        }
    relative = str(scratch.relative_to(root))
    ignored = subprocess.run(
        ["git", "-C", str(root), "check-ignore", "-v", relative],
        capture_output=True,
        text=True,
    )
    if ignored.returncode != 0:
        raise SystemExit(
            f"--scratch names {relative} inside this repository, which git does "
            "not ignore; the clone would reach the repository's status"
        )
    return {
        "location": relative,
        "git_check_ignore": ignored.stdout.strip(),
    }


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
    source: Path,
    source_revision: str,
    root: Path,
    workload_revision: str,
    repetitions: int,
) -> dict:
    """Times the whole checker process against this repository's committed objects."""
    samples, exit_codes, last_stdout = [], [], ""
    before, after = [], []
    for _ in range(repetitions):
        before.append(working_tree_status(root))
        elapsed, completed = timed_process(
            ["python3", "-B", str(source), "--revision", workload_revision], root
        )
        after.append(working_tree_status(root))
        samples.append(elapsed)
        exit_codes.append(completed.returncode)
        last_stdout = completed.stdout.decode(errors="replace").strip()
    return {
        "tool": CHECKER_PATH,
        "process": f"python3 -B <{CHECKER_PATH} of the form's revision>"
        f" --revision {workload_revision}",
        "measures": "whole process wall time, from start to exit",
        "working_directory": "the root of this repository's checkout",
        "workload": checker_workload(workload_revision, source_revision),
        "functions_defined_at_revision": defined_functions(
            root, source_revision, CHECKER_PATH
        ),
        "working_tree": {
            "probe": (
                "git status --porcelain of this repository, read immediately "
                "before and immediately after each repetition, outside the "
                "timed region"
            ),
            "status_lines_before": [len(lines) for lines in before],
            "status_lines_after": [len(lines) for lines in after],
            "unchanged_every_repetition": before == after,
        },
        "exit_codes": exit_codes,
        "final_stdout": last_stdout,
        **stats(samples),
    }


def measure_restoration(
    root: Path, clone: Path, revision: str, base_revision: str, repetitions: int
) -> dict:
    """Times the whole restoration process in a scratch clone, reset per repetition."""
    samples, exit_codes, last_stdout = [], [], ""
    staged = []
    for _ in range(repetitions):
        reset_clone(clone)
        place_form(root, clone, revision)
        elapsed, completed = timed_process(
            ["python3", "-B", RESTORE_PATH, base_revision], clone
        )
        staged.append(len(staged_paths(clone)))
        samples.append(elapsed)
        exit_codes.append(completed.returncode)
        last_stdout = completed.stdout.decode(errors="replace").strip()
    reset_clone(clone)
    return {
        "tool": RESTORE_PATH,
        "process": f"python3 -B {RESTORE_PATH} {base_revision}",
        "measures": restoration_measures(revision),
        "functions_defined_at_revision": defined_functions(root, revision, RESTORE_PATH),
        "working_directory": "the root of the scratch clone",
        "workload": restoration_workload(base_revision, revision),
        "staged": {
            "probe": (
                "git diff --cached --name-only in the clone, counted after each "
                "repetition and outside the timed region"
            ),
            "paths_staged": staged,
        },
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


def requested_niceness(root: Path) -> int:
    """Reads the niceness the lock wrapper asks for out of the wrapper itself.

    The figure is the wrapper's, not this tool's, so it is read from the
    wrapper at run time; a wrapper that names more than one niceness, or none,
    stops the run rather than have this record state a niceness it guessed.
    """
    source = (root / WRAPPER_PATH).read_text()
    named = {int(match["niceness"]) for match in WRAPPER_NICENESS.finditer(source)}
    if len(named) != 1:
        raise SystemExit(
            f"{WRAPPER_PATH} names {sorted(named) or 'no'} niceness where this "
            f"record expects exactly one: {WRAPPER_NICENESS.pattern}"
        )
    return named.pop()


def collect_scheduling(root: Path, wrapper: dict, lock: dict) -> dict:
    """Records the scheduling this run observes for itself."""
    loadavg = Path("/proc/loadavg").read_text().split()
    requested = requested_niceness(root)
    observed = os.nice(0)
    soft, hard = resource.getrlimit(resource.RLIMIT_NICE)
    floor = 20 - soft
    return {
        "requested": (
            f"{WRAPPER_PATH} requests nice -n {requested}; whether this run is "
            "wrapped by it at all is the `wrapper` field, and the cores it may "
            "use are `cpu_affinity_size`"
        ),
        "requested_niceness": requested,
        "observed_niceness": observed,
        "rlimit_nice": {"soft": soft, "hard": hard, "lowest_niceness_permitted": floor},
        "niceness_note": (
            f"RLIMIT_NICE is {soft} soft and {hard} hard, which permits a "
            f"niceness no lower than {floor}; the requested {requested} is "
            + ("attainable" if requested >= floor else "not attainable")
            + f", and the niceness that applies to this run is {observed}"
        ),
        "cpu_affinity_size": len(os.sched_getaffinity(0)),
        "load_average_at_start": [float(value) for value in loadavg[:3]],
        "runnable_at_start": loadavg[3],
        "mutex": lock,
        "wrapper": wrapper,
    }


def collect_toolchain() -> dict:
    return {
        "python3": sys.version.split()[0],
        "git": run("git", "--version").strip(),
    }


def pinned_but_missing(series: dict) -> int | None:
    """Counts the pinned files the base revision omits, from a run's own output.

    The restoration generator prints how many pinned files it restored and how
    many it could not; their sum is how many the base revision's receipts pin
    and the revision does not carry. Returns None when no restoration series in
    `series` printed a result to count, so the sentence that would state it is
    left unstated rather than guessed.
    """
    for name, measured in series.items():
        if not name.startswith("restoration"):
            continue
        try:
            result = json.loads(measured.get("final_stdout", ""))
        except json.JSONDecodeError:
            continue
        if isinstance(result, dict) and {"restored", "unrestored"} <= result.keys():
            return int(result["restored"]) + int(result["unrestored"])
    return None


def clone_description(series: dict) -> str:
    """Describes the scratch clone, counting its omissions from the run's output."""
    count = pinned_but_missing(series)
    counted = (
        ""
        if count is None
        else (
            f"; its committed receipts pin {count} snapshot files it does not "
            "carry, the restored and unrestored files the restoration series' "
            "own output reports"
        )
    )
    return (
        "git clone --shared --no-checkout of this repository, then git "
        f"checkout --detach {RESTORE_BASE_REVISION}: the working tree the "
        "restoration generator was written against, whose objects are this "
        "repository's own through the clone's alternates" + counted
    )


def published_figures(root: Path) -> tuple[dict, str]:
    """Reads the assessed figures out of the cited commit's own message.

    This tool holds no figure of its own: it cites commit
    `PUBLISHED_CLAIM_REVISION` by identity and derives every published second
    from the sentence that commit's message carries, failing loudly if that
    sentence is not there in the shape `PUBLISHED_SENTENCE` names.
    """
    message = git(
        root, "show", "-s", "--format=%B", PUBLISHED_CLAIM_REVISION
    ).decode()
    sentence = PUBLISHED_SENTENCE.search(" ".join(message.split()))
    if sentence is None:
        raise SystemExit(
            f"commit {PUBLISHED_CLAIM_REVISION}'s message does not carry the "
            f"sentence this assessment reads: {PUBLISHED_SENTENCE.pattern}"
        )
    figures = {name: float(value) for name, value in sentence.groupdict().items()}
    if set(figures) != set(SERIES_ORDER):
        raise SystemExit(
            "the assessed sentence names "
            f"{sorted(figures)}, not the series {sorted(SERIES_ORDER)}"
        )
    return figures, f'commit {PUBLISHED_CLAIM_REVISION}\'s message: "{sentence.group()}"'


def compare_to_published(series: dict, figures: dict) -> dict:
    """Sets each published figure beside the figure a record measures for it."""
    comparison = {}
    for name, published in figures.items():
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


def published_claim(root: Path, series: dict) -> dict:
    """Builds the assessment block from the cited commit and a record's series."""
    figures, source = published_figures(root)
    return {
        "source": source,
        "derivation": (
            "read from that commit's own message at assessment time and "
            "matched with the pattern "
            f"{PUBLISHED_SENTENCE.pattern!r}; "
            "dev/active/a203a23c/timing/measure.py carries no figure of its own"
        ),
        "tolerance": (
            f"a figure counts as confirmed when the measured median is "
            f"within {CONFIRMATION_TOLERANCE:.0%} of it"
        ),
        "figures": compare_to_published(series, figures),
    }


def assess(root: Path, record_path: Path) -> int:
    """Re-derives what a committed record can still derive, withdrawing the rest.

    Anything that follows from committed data the record names — the assessment
    against the cited commit, the clone's description, the randomness scan of
    both forms, what each timed source defines — is re-derived. Anything that
    was a condition of a run that has ended, and that the run did not probe, is
    withdrawn instead of restated. Samples are never touched, and no field a run
    would have had to observe is invented for a record that lacks it.
    """
    record = json.loads(record_path.read_text())
    series = record.get("series")
    if not series:
        raise SystemExit(f"{record_path} carries no series to assess")

    if "declaration_note" in record:
        record["declaration_note"] = DECLARATION_NOTE
    record["published_claim"] = published_claim(root, series)
    if "clone" in record.get("scratch", {}):
        record["scratch"]["clone"] = clone_description(series)

    revisions = record.get("revisions", {})
    forms = {
        form: revisions.get(f"{form}_source") for form in ("per_file", "batch")
    }
    if all(forms.values()):
        record["rng"] = randomness_report(root, forms)

    for name, measured in series.items():
        revision = measured.get("source_revision")
        if not revision:
            continue
        if name.startswith("checker"):
            measured["workload"] = checker_workload(
                revisions.get("checker_workload", revision), revision
            )
            measured["functions_defined_at_revision"] = defined_functions(
                root, revision, CHECKER_PATH
            )
        elif name.startswith("restoration"):
            measured["measures"] = restoration_measures(revision)
            measured["workload"] = restoration_workload(
                revisions.get("restoration_base", revision), revision
            )
            measured["functions_defined_at_revision"] = defined_functions(
                root, revision, RESTORE_PATH
            )

    scheduling = record.get("scheduling", {})
    if isinstance(scheduling.get("mutex"), str):
        scheduling["mutex"] = withdrawn(scheduling["mutex"])
    if "wrapper" not in scheduling and "requested" in scheduling:
        scheduling["requested"] = withdrawn(scheduling["requested"])
    if "rlimit_nice" not in scheduling and "niceness_note" in scheduling:
        scheduling["niceness_note"] = withdrawn(scheduling["niceness_note"])
    if isinstance(record.get("wrapper"), str):
        record["wrapper"] = withdrawn(record["wrapper"])

    record_path.write_text(json.dumps(record, indent=2) + "\n")
    return 0


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
        help="directory for the clone and the tool sources; inside this "
        "repository it must be a path git ignores",
    )
    parser.add_argument(
        "--repetitions",
        type=int,
        default=SAMPLING_PLAN["repetitions_per_series"],
        help="repetitions per series; the sampling plan fixes this at 5",
    )
    parser.add_argument(
        "--assess",
        metavar="RECORD",
        help="re-derive this committed record's published_claim and clone "
        "description from the cited commit and the record's own series, "
        "measuring nothing",
    )
    arguments = parser.parse_args()

    root = repo_root()
    if arguments.assess is not None:
        return assess(root, Path(arguments.assess))
    if arguments.scratch is None:
        parser.error("--scratch is required unless --assess names a record")
    scratch = Path(arguments.scratch).resolve()
    scratch.mkdir(parents=True, exist_ok=True)
    placement = check_scratch(root, scratch)
    repetitions = arguments.repetitions

    wrapper = observe_wrapper()
    lock = observe_lock(root)
    if not lock["exclusive_holder_observed"]:
        raise SystemExit(
            f"no exclusive holder of {lock['lock_path']} is observed, so this "
            "run cannot state that no other benchmark or build on the host "
            f"overlaps it: run it under {WRAPPER_PATH} --full-host"
        )

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

    scheduling = collect_scheduling(root, wrapper, lock)
    series = {}
    for name in SERIES_ORDER:
        if name == "checker_per_file":
            series[name] = measure_checker(
                checker_sources["per_file"],
                per_file_revision,
                root,
                batch_revision,
                repetitions,
            )
            series[name]["source_revision"] = per_file_revision
        elif name == "checker_batch":
            series[name] = measure_checker(
                checker_sources["batch"],
                batch_revision,
                root,
                batch_revision,
                repetitions,
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
        "declaration_note": DECLARATION_NOTE,
        "command": sanitized_command("--scratch"),
        "repetitions": repetitions,
        "sampling_plan": SAMPLING_PLAN,
        "rng": randomness_report(
            root, {"per_file": per_file_revision, "batch": batch_revision}
        ),
        "revisions": {
            "per_file_source": per_file_revision,
            "batch_source": batch_revision,
            "checker_workload": batch_revision,
            "restoration_base": RESTORE_BASE_REVISION,
        },
        "scratch": {
            "location": placement["location"],
            "git_check_ignore": placement["git_check_ignore"],
            "filesystem": {
                "repository": filesystem_of(root),
                "scratch": filesystem_of(scratch),
                "same_device": os.stat(root).st_dev == os.stat(scratch).st_dev,
                "why": (
                    "the restoration generator's real working tree is the "
                    "repository's own, so the scratch clone shares its "
                    "filesystem and the measured times carry no filesystem "
                    "difference of the harness's making"
                ),
            },
            "clone": clone_description(series),
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
        "published_claim": published_claim(root, series),
    }
    print(json.dumps(record, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
