#!/usr/bin/env python3
"""Record and check the peak memory of the batch decoder benchmark arms (jit:05c9065e).

For every arm of `arms.json` on each recorded code the recorder launches the
arm executable `LAUNCHES` times in the runner's untimed validation position
(window budget zero). The request, environment and sentinel are those
`benchmark-ab-runner launch` prints for the arm over a throwaway plan of
`smoke-gf2-arms.py`; the recorder starts the child itself, so the runner is
no ancestor and contributes no pages. The kernel's peak resident set size of
the exited child comes from `wait4`. The figure is process memory: executable,
libraries, bundle buffers, request transport and decoder state, plus the
pages the launcher interpreter held at `exec`, which the kernel folds into a
child's high-water mark. It is not decoder-owned memory (the decoder reports
that as `workspace_bytes`). The floor is the same method over `LAUNCHES`
launches of the same executable and environment without the sentinel, which
stop at the arm's guard before decoding, so it bounds the launcher and
loader share. Statistics are descriptive. Nothing is timed.

`record` writes `peak-memory.json`. `check` verifies its structure, recomputes
its statistics from its samples, requires the arm and cited-record digests to
match the files on disk and the host to match the recorded one, then launches
each cell `CHECK_LAUNCHES` times and requires every observation to lie within
the recorded envelope: the sample range widened on each side by its own width
(at least one page). A new launch of an unchanged arm falls outside a bare
sample range often, so the widening carries the launch-to-launch spread the
record states. It writes nothing into the repository.

Usage (from the worktree root): record-peak-memory.py record|check OUT_DIR
OUT_DIR holds `peak-memory.json`; the arms build identity is `OUT_DIR/build-identity.json`.
"""

import hashlib
import importlib.util
import json
import os
import pathlib
import platform
import shutil
import statistics
import subprocess
import sys
import tarfile

sys.dont_write_bytecode = True
INTRA = pathlib.Path(__file__).resolve().with_name("record-intra-quality.py")
intra = importlib.util.module_from_spec(importlib.util.spec_from_file_location("intra", INTRA))
intra.__spec__.loader.exec_module(intra)
smoke = intra.smoke

RECORD = "peak-memory.json"
LAUNCHES = 20
CHECK_LAUNCHES = 3
# The addendum whose cells every arm launch takes its decoder declaration and batch from.
FAMILY = "single-worker"
SCRATCH = pathlib.Path("target/peak-memory-scratch")
GROUP = {"issue": "05c9065e", "subject": "peak memory of the benchmark arms", "arm_kind": None}
QUALITY_ENVIRONMENT = "GF2_LDPC_QUALITY"
INTRA_ARM = intra.ARM
# The smoke arms every throwaway plan carries, per code.
PARTNERS = ("gf2-batch-single-frame", "gf2-batch-lane-avx2")
QUALITY_PIN = "prepared_quality"
# The launcher is a fresh interpreter per launch; the kernel folds the pages it
# held at `exec` into the child's high-water mark; `VmHWM` is that share.
LAUNCHER = """
import os, sys
held = next(int(line.split()[1]) for line in open("/proc/self/status") if line.startswith("VmHWM:"))
child = os.posix_spawn(sys.argv[1], sys.argv[1:], os.environ)
_, status, usage = os.wait4(child, 0)
sys.stdout.flush()
print(os.waitstatus_to_exitcode(status), usage.ru_maxrss, held)
"""
METHOD = {
    "quantity": "peak resident set size of the arm process",
    "scope": "process memory, not decoder-owned memory",
    "observation": "ru_maxrss of the exited child from wait4, in KiB",
    "includes": ["executable and shared libraries", "bundle buffers", "runner transport",
                 "decoder state", "pages the launcher held at exec"],
    "position": "validation: untimed, zero timing windows, one dispatch",
    "floor": "the same launch without the runner's fresh-child sentinel; the arm stops before decoding",
    "launcher": "VmHWM of the launcher interpreter before it spawns the arm, the share its "
                "exec folds into the child's mark",
    "statistics": "descriptive: minimum, median, maximum and population standard deviation",
}


def sha256_file(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def summary(samples):
    """Descriptive statistics of a list of KiB samples."""
    return {"samples_kib": samples, "count": len(samples), "min_kib": min(samples),
            "median_kib": statistics.median(samples), "max_kib": max(samples),
            "stdev_kib": statistics.pstdev(samples)}


def launch(command, environment, stdin):
    """One launch: return code, stdout, the child's and the launcher's `ru_maxrss` in KiB."""
    child = subprocess.run([sys.executable, "-S", "-E", "-c", LAUNCHER, *command],
                           input=stdin, capture_output=True, env=environment, cwd=smoke.ROOT,
                           check=True)
    *out, report = child.stdout.decode().splitlines()
    status, peak, launcher = map(int, report.split())
    return status, "\n".join(out), peak, launcher


def arm_launch(name, shared, floor):
    """One launch of arm `name` as the runner starts it; the floor launch omits the sentinel."""
    environment = dict(shared["environment"])
    if floor:
        del environment[shared["guard_variable"]]
    status, out, peak, launcher = launch(
        [shared["executable"], *shared["arguments"]], environment, shared["request"].encode())
    if floor:
        if status == 0 or out:
            sys.exit(f"{name}: the floor launch decoded")
    elif status != 0 or len(out.splitlines()) != 1:
        sys.exit(f"{name}: the validation launch did not write one result line")
    return peak, launcher


def runner_launches(rows, files, batches):
    """`{(arm, code): launch}` from `benchmark-ab-runner launch` over a plan of every arm.

    Each launch has a plan derived from the smoke's throwaway single-worker plan;
    the runner supplies its request, environment and sentinel.
    """
    runner, _ = smoke.build_runner()
    shutil.rmtree(SCRATCH, ignore_errors=True)
    (SCRATCH / "inputs").mkdir(parents=True)
    with tarfile.open(smoke.by_digest(smoke.ARCHIVE)) as handle:
        handle.extractall(SCRATCH / "inputs", filter="data")
    base = family_batches()
    extra = sorted(set(batches.values()) - set(base.values()))
    derived = [("", None, "single-frame", "lane-avx2"),
               *((f"-b{size}", size, "single-frame", "lane-avx2") for size in extra)]
    smoke.SCRATCH = SCRATCH
    plan_path = smoke.throwaway(GROUP, "peak-memory", smoke.FAMILIES[FAMILY][0], derived)
    base = json.loads(plan_path.read_text(encoding="utf-8"))
    launches = {}
    for (name, key), row in sorted(rows.items()):
        # The runner refuses a plan arm no cell uses, so each launch has a plan of its own: the
        # arm against one of the smoke's two arms, whichever is not the arm itself.
        arm = f"{name}-{key}"
        partner = next(f"{partner}-{key}" for partner in PARTNERS if f"{partner}-{key}" != arm)
        cell = next(cell for cell in base["cells"] if cell["cell_id"].startswith(key + "-")
                    and cell["case"]["batch_size"] == batches[name, key])
        plan = {**base, "arms": {partner: base["arms"][partner], arm: {
            "build": "native",  # the single-worker addendum declares both builds native
            "description": f"{name} on {key}", "executable": str(executable_of(row)),
            "arguments": [],
            "environment": {**row["environment"],
                            QUALITY_ENVIRONMENT: str(smoke.ROOT / files[name, key][0])},
            "rustflags": smoke.RUSTFLAGS, "tuning_profile": None}},
            "cells": [{**cell, "baseline_arm": arm, "candidate_arm": partner}],
            "max_cells_per_session": 1}
        path = plan_path.with_name(f"{arm}.plan.json")
        path.write_text(json.dumps(plan, indent=2) + "\n", encoding="utf-8")
        launches[name, key] = json.loads(subprocess.run(
            [runner, "launch", str(path), cell["cell_id"], arm], check=True,
            capture_output=True, text=True, cwd=smoke.ROOT).stdout)
    return launches


def quality_files(build_identity, out_dir):
    """`{(arm, code): (path, sha256)}` of the committed prepared quality files."""
    record = json.loads((smoke.ROOT / build_identity["arms"]["aff3ct-flooding-nms-f32"][
        "pinned_by"]["record"]).read_text(encoding="utf-8"))
    pinned = {pathlib.Path(path).name: (pathlib.Path(path), digest)
              for path, digest in record[QUALITY_PIN].items()}
    intra_record = json.loads((out_dir / "quality-intra" / intra.RECORD).read_text(encoding="utf-8"))
    files = {}
    for name, arm in build_identity["arms"].items():
        for key in smoke.CODES:
            if name == INTRA_ARM:
                held = intra_record["codes"][key]["quality"]
                files[name, key] = (out_dir / "quality-intra" / held["file"], held["sha256"])
            elif arm["executable"] == "aff3ct-throughput-arm":
                files[name, key] = pinned[f"{name}-{key}.json"]
            else:
                files[name, key] = pinned[f"gf2-nms-f32-{key}.json"]
    return files


def catalogued(build_identity):
    """The catalogue and the build-identity arm rows, which must name the same arms."""
    catalogue = json.loads((smoke.ROOT / build_identity["catalogue"]["path"])
                           .read_text(encoding="utf-8"))
    if catalogue["arms"].keys() != build_identity["arms"].keys():
        sys.exit("the build identity and the catalogue name different arms")
    if sha256_file(smoke.ROOT / build_identity["catalogue"]["path"]) != build_identity[
            "catalogue"]["sha256"]:
        sys.exit("the catalogue differs from the build identity's digest of it")
    return catalogue["arms"]


def executable_of(row):
    path = smoke.ROOT / (row["path"] if "path" in row else row["found_at"][0])
    if sha256_file(path) != row["executable_sha256"]:
        sys.exit(f"{path} differs from its pinned digest")
    return path


def host():
    return {**intra.host_facts(), "kernel": platform.release(),
            "page_bytes": os.sysconf("SC_PAGE_SIZE"),
            "cpus_available": len(os.sched_getaffinity(0))}


def family_batches():
    """`{code: timed batch}` the family's cells declare."""
    family = json.loads((smoke.ROOT / smoke.located("file", smoke.FAMILIES[FAMILY][0]))
                        .read_text(encoding="utf-8"))
    return {next(key for key in smoke.CODES if cell["cell_id"].startswith(key + "-")):
            cell["workload"]["size"]["timed_batch"] for cell in family["cells"]}


def observe(out_dir, launches):
    """Launches every arm on every code; `{arm: {code: cell}}` with the cell's samples."""
    build = json.loads((out_dir / "build-identity.json").read_text(encoding="utf-8"))
    catalogued(build)
    files = quality_files(build, out_dir)
    declared = family_batches()
    rows = {(name, key): row for name, row in build["arms"].items() for key in smoke.CODES}
    qualities, batches = {}, {}
    for (name, key) in rows:
        path = smoke.ROOT / files[name, key][0]
        if sha256_file(path) != files[name, key][1]:
            sys.exit(f"{path} differs from its pinned digest")
        qualities[name, key] = json.loads(path.read_text(encoding="utf-8"))
        # The quality file's decoder settings state the frames per SIMD wave the arm needs.
        batches[name, key] = max(declared[key], qualities[name, key]["settings"]["batch_size"])
    shared = runner_launches(rows, files, batches)
    cells = {}
    for (name, key), row in sorted(rows.items()):
        request = json.loads(shared[name, key]["request"])
        observed = {kind: [arm_launch(name, shared[name, key], kind == "floor")
                           for _ in range(launches)] for kind in ("run", "floor")}
        run, floor = (list(zip(*observed[kind])) for kind in ("run", "floor"))
        cells.setdefault(name, {})[key] = {
            "case": {**request["case"], "bundle": smoke.CODES[key]["bundle"]},
            "decoder": request["decoder"],
            "batch_frames_source": {"declared_by": f"{FAMILY} family cell timed_batch",
                                    "raised_by": "settings.batch_size of the quality file"},
            "quality": {"path": str(files[name, key][0]), "sha256": files[name, key][1]},
            "peak": list(run[0]), "launcher": list(run[1]),
            "floor": list(floor[0]), "floor_launcher": list(floor[1])}
    shutil.rmtree(SCRATCH, ignore_errors=True)
    return cells


def cited(path):
    return {"path": str(path.relative_to(smoke.ROOT)), "sha256": sha256_file(path)}


def record_mode(out_dir):
    build_path = out_dir / "build-identity.json"
    build = json.loads(build_path.read_text(encoding="utf-8"))
    observed = observe(out_dir, LAUNCHES)
    arms = {}
    for name, row in sorted(build["arms"].items()):
        arms[name] = {"executable": {"name": row["executable"],
                                     "sha256": row["executable_sha256"]},
                      "environment": row["environment"], "codes": {}}
        for key, cell in observed[name].items():
            peak, floor = summary(cell.pop("peak")), summary(cell.pop("floor"))
            launcher = summary(cell.pop("launcher"))
            floor["launcher"] = summary(cell.pop("floor_launcher"))
            arms[name]["codes"][key] = {
                **cell, **peak, "launcher": launcher, "floor": floor,
                "above_floor_median_kib": peak["median_kib"] - floor["median_kib"]}
    record = {"cites": ["Cassagne2019"], "method": {**METHOD, "launches": LAUNCHES},
              "build_identity": cited(build_path), "catalogue": build["catalogue"],
              "recorded_inputs": {"name": smoke.ARCHIVE[0], "sha256": smoke.ARCHIVE[1]},
              "host": host(),
              "arms": arms}
    (out_dir / RECORD).write_text(intra.encoded(record), encoding="utf-8")
    print(out_dir / RECORD)


def structure_failures(record, build):
    """Failures of the record's structure, statistics and recorded identities."""
    failures = []
    if record["cites"] != ["Cassagne2019"]:
        failures.append("cites")
    if record["arms"].keys() != build["arms"].keys():
        failures.append("arms differ from the build identity")
    for name, arm in record["arms"].items():
        pinned = build["arms"].get(name, {})
        if arm["executable"]["sha256"] != pinned.get("executable_sha256"):
            failures.append(f"{name}: executable digest differs from the build identity")
        if set(arm["codes"]) != set(smoke.CODES):
            failures.append(f"{name}: codes differ from the recorded codes")
        for key, cell in arm["codes"].items():
            for part, group in (("peak", cell), ("launcher", cell["launcher"]),
                                ("floor", cell["floor"]),
                                ("floor launcher", cell["floor"]["launcher"])):
                samples = group["samples_kib"]
                if group["count"] != len(samples) or len(samples) != record["method"]["launches"]:
                    failures.append(f"{name} {key} {part}: sample count differs")
                elif {field: group[field] for field in summary(samples)} != summary(samples):
                    failures.append(f"{name} {key} {part}: statistics differ from the samples")
            if cell["above_floor_median_kib"] != cell["median_kib"] - cell["floor"]["median_kib"]:
                failures.append(f"{name} {key}: above-floor median differs")
    return failures


def check_mode(out_dir):
    record = json.loads((out_dir / RECORD).read_text(encoding="utf-8"))
    build_path = out_dir / "build-identity.json"
    build = json.loads(build_path.read_text(encoding="utf-8"))
    failures = structure_failures(record, build)
    failures += [f"{name} differs" for name, current in (
        ("build_identity", cited(build_path)), ("catalogue", build["catalogue"]),
        ("recorded_inputs", {"name": smoke.ARCHIVE[0], "sha256": smoke.ARCHIVE[1]}),
        ("host", host())) if record[name] != current]
    if not failures:
        for name, keys in observe(out_dir, CHECK_LAUNCHES).items():
            for key, cell in keys.items():
                held = record["arms"][name]["codes"][key]
                failures += [f"{name} {key}: {field} differs" for field in ("case", "decoder", "quality")
                             if cell[field] != held[field]]
                slack = max(held["max_kib"] - held["min_kib"], record["host"]["page_bytes"] // 1024)
                low, high = held["min_kib"] - slack, held["max_kib"] + slack
                outside = [peak for peak in cell["peak"] if not low <= peak <= high]
                print(f"{name} {key}: observed {cell['peak']} KiB within [{low}, {high}]"
                      if not outside else f"{name} {key}: {outside} KiB outside [{low}, {high}]")
                failures += [f"{name} {key}: peak {peak} KiB outside the recorded envelope"
                             for peak in outside]
    for failure in failures:
        print(f"DIFFERENCE {failure}")
    sys.exit(1 if failures else 0)


def main():
    if len(sys.argv) != 3 or sys.argv[1] not in ("record", "check"):
        sys.exit(__doc__)
    if pathlib.Path.cwd() != smoke.ROOT:
        sys.exit("invoke from the worktree root")
    {"record": record_mode, "check": check_mode}[sys.argv[1]](pathlib.Path(sys.argv[2]).resolve())


if __name__ == "__main__":
    main()
