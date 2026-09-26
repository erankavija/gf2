#!/usr/bin/env python3
"""Render historical logical invocation companions from committed logs and source.

The window commands are observed in the committed window-log excerpt. Inner
commands are reconstructed from the launchers at the measured revision and
bound to paths and arguments retained by the campaign and profile logs. They
are labelled as reconstructed because no process-exec trace was retained.
"""

import argparse
import hashlib
import json
import pathlib
import re
import shlex
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[4]
STORY = pathlib.Path("dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations")
SURVEY = STORY / "survey"
RESULTS = pathlib.Path("dev/bench_results/2037941f")
WINDOW = RESULTS / "logical-window-jobs.log"
SOURCE_HARNESS = SURVEY / "run-logical-harness.sh"
SOURCE_PROFILE = SURVEY / "run-logical-profile.sh"
SOURCE_SWEEP = SURVEY / "sweep-logical-profile.sh"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def data(path):
    return (ROOT / path).read_bytes()


def source(revision, path, required):
    blob = subprocess.check_output(["git", "show", f"{revision}:{path}"], cwd=ROOT)
    text = blob.decode()
    for fragment in required:
        if fragment not in text:
            raise ValueError(f"{revision}:{path} lacks pinned command fragment {fragment!r}")
    return text, digest(blob)


def field(text, prefix):
    values = [line[len(prefix):] for line in text.splitlines() if line.startswith(prefix)]
    if len(values) != 1:
        raise ValueError(f"expected one {prefix!r} field, got {len(values)}")
    return values[0]


def jobs():
    lines = data(WINDOW).decode().splitlines()
    starts = {}
    exits = {}
    for line in lines:
        match = re.fullmatch(r"(\S+) job (start|exit) issue=18a87159 key=([0-9a-f]+) (.*)", line)
        if not match:
            raise ValueError(f"unexpected window excerpt line: {line}")
        _, kind, key, rest = match.groups()
        if kind == "start":
            if key in starts or " command=" not in rest:
                raise ValueError(f"duplicate or incomplete window start {key}")
            starts[key] = (line, rest.split(" command=", 1)[1])
        else:
            if key in exits or rest != "rc=0":
                raise ValueError(f"duplicate or unsuccessful window exit {key}: {rest}")
            exits[key] = line
    if len(starts) != 4 or starts.keys() != exits.keys():
        raise ValueError("window excerpt must contain four successful paired jobs")
    return [(start, command, exits[key]) for key, (start, command) in starts.items()]


def select_job(all_jobs, marker):
    matches = [job for job in all_jobs if marker in job[1]]
    if len(matches) != 1:
        raise ValueError(f"expected one window job for {marker}, got {len(matches)}")
    return matches[0]


def command(*args):
    return shlex.join(str(arg) for arg in args)


def header(title, inputs):
    lines = [f"# {title}", "", "> **Diátaxis Type:** Reference", "",
             "Window argv comes from the committed window-log excerpt. Inner argv is",
             "reconstructed from the pinned measured launcher and retained run inputs;",
             "the historical run did not retain a process-exec trace. Paths and redirects",
             "below are the operands the pinned scripts compute from those inputs.", "",
             "## Input identities", "", "| Input | SHA-256 |", "|---|---|"]
    lines += [f"| `{path}` | `{digest(data(path))}` |" for path in inputs]
    return lines + [""]


def campaign(all_jobs, name):
    receipt_dir = RESULTS / name / "v4-r1-pilot"
    receipt_path = receipt_dir / "receipt.json"
    launch_path = receipt_dir / "launcher.log"
    receipt = json.loads(data(receipt_path))
    launch = data(launch_path).decode()
    events = [json.loads(line)["event"] for line in data(receipt_dir / "execution.log").decode().splitlines()]
    campaign_id = receipt["campaign_id"]
    if campaign_id != f"v4-r1-{name}":
        raise ValueError(f"unexpected campaign id in {receipt_path}")
    start, top, end = select_job(all_jobs, f"--family {name} ")
    if not top.startswith(str(SOURCE_HARNESS) + " window "):
        raise ValueError(f"unexpected top-level command: {top}")
    revision = field(launch, "# gf2 revision (informational): ")
    _, source_sha = source(revision, SOURCE_HARNESS, (
        '"${runner}" run "${stage}" "${plan}"',
        '"${runner}" finalize "${stage}" "${out}"',
        'GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host',
    ))
    plan = field(launch, "# plan: ").split(" sha256=", 1)[0]
    suffix = "/target/bb769456-campaigns/"
    if suffix not in plan or not plan.endswith(".plan.json"):
        raise ValueError(f"unrecognized retained plan path {plan}")
    worktree = plan.split(suffix, 1)[0]
    stage = plan.removesuffix(".plan.json")
    if not stage.endswith(campaign_id):
        raise ValueError("retained plan does not name the receipt campaign")
    runner = f"{worktree}/target/release/benchmark-ab-runner"
    sessions = re.findall(r"^# session (\d+) exit: (\d+)$", launch, re.MULTILINE)
    if not sessions or [int(i) for i, _ in sessions] != list(range(1, len(sessions) + 1)):
        raise ValueError(f"missing session sequence in {launch_path}")
    if [status for _, status in sessions[:-1]] != ["3"] * (len(sessions) - 1) or sessions[-1][1] != "0":
        raise ValueError(f"unexpected runner session exits in {launch_path}")
    if events.count("orchestration-start") != len(sessions) or events.count("complete") != 1:
        raise ValueError(f"runner sessions disagree with execution log in {receipt_dir}")
    inputs = (WINDOW, receipt_path, launch_path, receipt_dir / "execution.log")
    lines = header(f"Invocation record: {name}", inputs)
    lines += ["## Observed window dispatch", "", f"`{top}`", "",
              f"Source lines: `{start}`; `{end}`.", "", "## Reconstructed runner dispatch", "",
              f"Measured source: `{revision}:{SOURCE_HARNESS}` (SHA-256 `{source_sha}`).", "",
              f"The launcher log records {len(sessions)} runner sessions, ending with exit 0.", "",
              "```sh"]
    run = command("dev/scripts/ccx1-bench-flock.sh", "--full-host", runner, "run", stage, plan)
    lines += [f"GF2_BENCH=1 CARGO_CI_NO_LOCK=1 {run}" for _ in sessions]
    lines += [command(runner, "finalize", stage, str(pathlib.Path(worktree) / receipt_dir)), "```", "",
              "The runner's child arm argv, environment and executable digest are in",
              f"`{receipt_path}` and its append-only `execution.log`.", ""]
    return receipt_dir / "invocations.md", "\n".join(lines)


def profile(all_jobs, worktree):
    profile_dir = RESULTS / "logical-profile"
    host_path = profile_dir / "host.txt"
    log_path = profile_dir / "repetitions.log"
    cases_path = profile_dir / "cases.txt"
    recorded_path = profile_dir / "recorded-cases.txt"
    host = data(host_path).decode()
    log = data(log_path).decode()
    cases = data(cases_path).decode().splitlines()
    recorded = data(recorded_path).decode().splitlines()
    if not cases or not recorded or not set(recorded) <= set(cases):
        raise ValueError("invalid retained profile case lists")
    reps = re.findall(r"^(rep-\d+) done ", log, re.MULTILINE)
    if reps != [f"rep-{i:02d}" for i in range(1, 10)] or "series done" not in log:
        raise ValueError("profile lacks nine completed repetitions")
    start, top, end = select_job(all_jobs, "run-logical-profile.sh window ")
    if top != f"{SOURCE_PROFILE} window {profile_dir}":
        raise ValueError(f"unexpected profile window command {top}")
    revision = field(host, "# gf2 revision (informational): ")
    profile_source, profile_sha = source(revision, SOURCE_PROFILE, (
        'GF2_BENCH=1 GF2_BENCH_WINDOW=1 CARGO_CI_NO_LOCK=1 "${FLOCK}" --full-host',
        'bash "${HERE}/sweep-logical-profile.sh"',
    ))
    _, sweep_sha = source(revision, SOURCE_SWEEP, (
        'perf stat -x, -e "${GROUP_ISSUE}"',
        'perf stat -x, -e "${GROUP_MEMORY}"',
        'perf record --quiet -F 4999',
        'perf report --stdio --no-children --percent-limit 0.5',
    ))
    def constant(name):
        values = re.findall(rf"^{name}=(\S+)$", profile_source, re.MULTILINE)
        if len(values) != 1:
            raise ValueError(f"missing pinned {name}")
        return values[0]
    seconds = constant("SECONDS_PER_PASS")
    issue = constant("GROUP_ISSUE")
    memory = constant("GROUP_MEMORY")
    driver = f"{worktree}/target/bb769456-arms/release/logical-profile"
    output = worktree / profile_dir
    sweep = worktree / SOURCE_SWEEP
    wrapper = worktree / "dev/scripts/ccx1-bench-flock.sh"
    inputs = (WINDOW, host_path, log_path, cases_path, recorded_path)
    lines = header("Invocation record: logical profile", inputs)
    lines += ["## Observed window dispatch", "", f"`{top}`", "",
              f"Source lines: `{start}`; `{end}`.", "",
              "## Reconstructed runner and perf dispatch", "",
              f"Measured sources: `{revision}:{SOURCE_PROFILE}` (SHA-256 `{profile_sha}`) and",
              f"`{revision}:{SOURCE_SWEEP}` (SHA-256 `{sweep_sha}`).", "",
              "Every completed repetition uses the same retained case lists. Commands below",
              "are reconstructed in the loop order of those pinned sources.", "", "```sh"]
    for rep in reps:
        rep_dir = output / rep
        wrapper_argv = command("bash", sweep, rep_dir, driver, seconds, issue, memory,
                               output / "cases.txt", output / "recorded-cases.txt")
        lines.append(f"GF2_BENCH=1 GF2_BENCH_WINDOW=1 CARGO_CI_NO_LOCK=1 {command(wrapper, '--full-host')} {wrapper_argv}")
        for case in cases:
            path = rep_dir / case
            relative = profile_dir / rep / case
            for suffix in (".issue.csv", ".issue.json", ".memory.csv", ".memory.json"):
                if not (ROOT / pathlib.Path(str(relative) + suffix)).is_file():
                    raise ValueError(f"missing measured profile output: {relative}{suffix}")
            run = (driver, "run", "--case", case, "--seconds", seconds)
            lines.append(command("perf", "stat", "-x,", "-e", issue, "-o", f"{path}.issue.csv", "--", *run)
                         + f" > {shlex.quote(str(path) + '.issue.json')}")
            lines.append(command("perf", "stat", "-x,", "-e", memory, "-o", f"{path}.memory.csv", "--", *run)
                         + f" > {shlex.quote(str(path) + '.memory.json')}")
        for case in recorded:
            path = rep_dir / case
            relative = profile_dir / rep / case
            for suffix in (".record.json", ".report.txt"):
                if not (ROOT / pathlib.Path(str(relative) + suffix)).is_file():
                    raise ValueError(f"missing measured profile output: {relative}{suffix}")
            run = (driver, "run", "--case", case, "--seconds", seconds)
            lines.append(command("perf", "record", "--quiet", "-F", "4999", "-o", f"{path}.perf.data", "--", *run)
                         + f" > {shlex.quote(str(path) + '.record.json')}")
            lines.append(command("perf", "report", "--stdio", "--no-children", "--percent-limit", "0.5", "-i", f"{path}.perf.data")
                         + f" > {shlex.quote(str(path) + '.report.txt')}")
    lines += ["```", "", "The raw counter outputs, rendered symbol reports, driver digest and",
              "host record remain under the profile directory. The pinned sweep deletes",
              "each perf data file after rendering its report.", ""]
    return profile_dir / "invocations.md", "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="compare generated records")
    args = parser.parse_args()
    all_jobs = jobs()
    names = []
    for _, top, _ in all_jobs:
        argv = shlex.split(top)
        if not argv[0].endswith("/run-logical-harness.sh"):
            continue
        if argv[1] != "window" or argv.count("--family") != 1:
            raise ValueError(f"unrecognized logical campaign command: {top}")
        name = argv[argv.index("--family") + 1]
        if not name.startswith("2037941f-logical-"):
            raise ValueError(f"unrecognized logical campaign family: {name}")
        names.append(name)
    if len(names) != 3 or len(set(names)) != len(names):
        raise ValueError("window excerpt does not identify three distinct logical campaigns")
    records = [campaign(all_jobs, name) for name in names]
    plans = [field(data(RESULTS / name / "v4-r1-pilot" / "launcher.log").decode(), "# plan: ").split(" sha256=", 1)[0] for name in names]
    roots = {plan.split("/target/bb769456-campaigns/", 1)[0] for plan in plans}
    if len(roots) != 1:
        raise ValueError("campaign launch logs disagree about the measured worktree")
    records.append(profile(all_jobs, pathlib.Path(roots.pop())))
    for path, rendered in records:
        output = ROOT / path
        if args.check:
            if output.read_text() != rendered:
                raise ValueError(f"stale invocation companion: {path}")
        else:
            output.write_text(rendered)
        print(path)


if __name__ == "__main__":
    main()
