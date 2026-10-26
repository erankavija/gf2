#!/usr/bin/env python3
"""Record the peak memory of the batch decoder benchmark arms (jit:05c9065e).

`record` launches each arm of the build record in OUT_DIR on each recorded code, untimed, as
`benchmark-ab-runner launch` prints, and writes `peak-memory.json`, which states the method.
`check` compares the record's arm identities with that build record and recomputes its
statistics; it launches nothing and writes nothing.

Usage (from the worktree root): record-peak-memory.py record|check OUT_DIR
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
BUILD = pathlib.Path(__file__).resolve().with_name("arm_build.py")
arm_build = importlib.util.module_from_spec(importlib.util.spec_from_file_location("arm_build", BUILD))
arm_build.__spec__.loader.exec_module(arm_build)

RECORD = "peak-memory.json"
LAUNCHES = 20
FAMILY = "single-worker"
SCRATCH = pathlib.Path("target/peak-memory-scratch")
GROUP = {"issue": "05c9065e", "subject": "peak memory of the benchmark arms", "arm_kind": None}
QUALITY_ENVIRONMENT = "GF2_LDPC_QUALITY"
INTRA_ARM = intra.ARM
PARTNERS = ("gf2-batch-single-frame", "gf2-batch-lane-avx2")
QUALITY_PIN = "prepared_quality"
CATALOGUE_FIELDS = ("comparison", "precision", "schedule", "update_rule", "simd")
# `exec` folds the launcher's pages into the child's mark; `VmHWM` is that share.
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
    "effective": "per cell, the arm's catalogue class and the settings of its quality file",
    "request_declaration": "per cell, the decoder block the runner sent: the single-worker "
                           "cell's declaration, not the arm's settings",
    "statistics": "descriptive: minimum, median, maximum and population standard deviation",
}


def sha256_file(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def summary(samples):
    return {"samples_kib": samples, "count": len(samples), "min_kib": min(samples),
            "median_kib": statistics.median(samples), "max_kib": max(samples),
            "stdev_kib": statistics.pstdev(samples)}


def launch(command, environment, stdin):
    """Return code, stdout, the child's `ru_maxrss` and the launcher's share, in KiB."""
    child = subprocess.run([sys.executable, "-S", "-E", "-c", LAUNCHER, *command],
                           input=stdin, capture_output=True, env=environment, cwd=smoke.ROOT,
                           check=True)
    *out, report = child.stdout.decode().splitlines()
    status, peak, launcher = map(int, report.split())
    return status, "\n".join(out), peak, launcher


def arm_launch(name, shared, floor):
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


def arm_plans(rows, files, batches, group=GROUP, family=FAMILY, label="peak-memory",
              scratch=SCRATCH):
    """Per arm and code, the throwaway plan of the arm against a smoke arm and its cell ids.

    The plan holds every cell of the smoke family `family` on the code at the batch `batches`
    gives the arm. The recorded bundles are extracted under `scratch`, which keeps the rest of
    its contents, so a caller's stages there resume.
    """
    (scratch / "inputs").mkdir(parents=True, exist_ok=True)
    with tarfile.open(smoke.by_digest(smoke.ARCHIVE)) as handle:
        handle.extractall(scratch / "inputs", filter="data")
    base = family_batches(family)
    extra = sorted(set(batches.values()) - set(base.values()))
    derived = [("", None, "single-frame", "lane-avx2"),
               *((f"-b{size}", size, "single-frame", "lane-avx2") for size in extra)]
    smoke.SCRATCH = scratch
    plan_path = smoke.throwaway(group, label, smoke.FAMILIES[family][0], derived)
    base = json.loads(plan_path.read_text(encoding="utf-8"))
    plans = {}
    for (name, key), row in sorted(rows.items()):
        # The runner refuses a plan arm no cell uses: one plan per arm, against a smoke arm.
        arm = f"{name}-{key}"
        partner = next(f"{partner}-{key}" for partner in PARTNERS if f"{partner}-{key}" != arm)
        cells = [cell for cell in base["cells"] if cell["cell_id"].startswith(key + "-")
                 and cell["case"]["batch_size"] == batches[name, key]]
        plan = {**base, "arms": {partner: base["arms"][partner], arm: {
            "build": "native",  # the addendum declares native builds
            "description": f"{name} on {key}", "executable": str(executable_of(row)),
            "arguments": [],
            "environment": {**row["environment"],
                            QUALITY_ENVIRONMENT: str(smoke.ROOT / files[name, key][0])},
            "rustflags": smoke.RUSTFLAGS, "tuning_profile": None}},
            "cells": [{**cell, "baseline_arm": arm, "candidate_arm": partner} for cell in cells],
            "max_cells_per_session": len(cells)}
        path = plan_path.with_name(f"{arm}.plan.json")
        path.write_text(json.dumps(plan, indent=2) + "\n", encoding="utf-8")
        plans[name, key] = path, arm, [cell["cell_id"] for cell in cells]
    return plans


def runner_launches(rows, files, batches):
    runner, _ = smoke.build_runner()
    launches = {}
    for (name, key), (path, arm, (cell_id,)) in arm_plans(rows, files, batches).items():
        launches[name, key] = json.loads(subprocess.run(
            [runner, "launch", str(path), cell_id, arm], check=True,
            capture_output=True, text=True, cwd=smoke.ROOT).stdout)
    return launches


def committed(name, matches):
    """The one committed JSON file called `name` whose content satisfies `matches`."""
    found = [(path, json.loads((smoke.ROOT / path).read_text(encoding="utf-8")))
             for path in smoke.listed(name)]
    found = [(path, record) for path, record in found if matches(record)]
    if len(found) != 1:
        sys.exit(f"{len(found)} committed files called {name} match")
    return smoke.ROOT / found[0][0], found[0][1]


BUILD_RECORD = "build-identity.json"


def build_records():
    """Each live build record of the arms catalogue with its path: one no other record supersedes."""
    found = [(smoke.ROOT / path, json.loads((smoke.ROOT / path).read_text(encoding="utf-8")))
             for path in smoke.listed(BUILD_RECORD)]
    found = [(path, record) for path, record in found if "catalogue" in record]
    superseded = {(smoke.ROOT / record["supersedes"]["path"], record["supersedes"]["sha256"])
                  for _, record in found if "supersedes" in record}
    return [(path, record) for path, record in found
            if (path, hashlib.sha256(path.read_bytes()).hexdigest()) not in superseded]


def build_record(directory):
    """The build record of the arms catalogue in `directory`."""
    path = pathlib.Path(directory).resolve() / BUILD_RECORD
    record = json.loads(path.read_text(encoding="utf-8"))
    if "catalogue" not in record:
        sys.exit(f"{path} is no build record of the arms catalogue")
    return path, record


def catalogue_of(build_identity):
    return json.loads((smoke.ROOT / build_identity["catalogue"]["path"])
                      .read_text(encoding="utf-8"))["arms"]


def quality_files(build_identity):
    # The pinned arms' record carries the prepared quality; the catalogue pins it by digest.
    (pin,) = {arm["pinned_by"]["sha256"] for arm in catalogue_of(build_identity).values()
              if arm["source"] == "pinned"}
    record = json.loads(smoke.by_digest((BUILD_RECORD, pin)).read_text(encoding="utf-8"))
    pinned = {pathlib.Path(path).name: (pathlib.Path(path), digest)
              for path, digest in record[QUALITY_PIN].items()}
    intra_path, intra_record = committed(
        intra.RECORD, lambda record: record.get("arm") == INTRA_ARM)
    files = {}
    for name, arm in build_identity["arms"].items():
        for key in smoke.CODES:
            if name == INTRA_ARM:
                held = intra_record["codes"][key]["quality"]
                files[name, key] = (intra_path.parent.relative_to(smoke.ROOT) / held["file"],
                                    held["sha256"])
            elif arm["executable"] == "aff3ct-throughput-arm":
                files[name, key] = pinned[f"{name}-{key}.json"]
            else:
                files[name, key] = pinned[f"gf2-nms-f32-{key}.json"]
    return files


def catalogued(build_identity):
    """The catalogue's arms, once the build record holds every arm of the builds it records."""
    catalogue = catalogue_of(build_identity)
    builds = {catalogue[name]["build"] for name in build_identity["arms"] if name in catalogue}
    if {name for name, arm in catalogue.items() if arm["build"] in builds} != \
            build_identity["arms"].keys():
        sys.exit("the build identity and the catalogue name different arms")
    return catalogue


def effective(name, entry, settings):
    """What the arm states about itself: its catalogue class and the settings of its quality file."""
    rule = {"normalized-min-sum": "normalized-min-sum", "sum-product": "sum-product"}
    if (settings["precision"], settings["schedule"], settings["normalization"]["kind"]) != (
            entry["precision"], entry["schedule"], rule[entry["update_rule"]]):
        sys.exit(f"{name}: the quality file and the catalogue state different decoders")
    return {"catalogue": {field: entry.get(field) for field in CATALOGUE_FIELDS},
            "settings": settings}


def arm_identity(build, name):
    return {**arm_build.arm_build(build, name), "environment": build["arms"][name]["environment"]}


def executable_path(row, root=smoke.ROOT):
    """Where the build record found the arm's executable under the build root `root`, without
    checking it."""
    return root / (row["path"] if "path" in row else row["found_at"][0])


def executable_of(row):
    path = executable_path(row)
    if sha256_file(path) != row["executable_sha256"]:
        sys.exit(f"{path} differs from its pinned digest")
    return path


def host():
    return {**intra.host_facts(), "kernel": platform.release(),
            "page_bytes": os.sysconf("SC_PAGE_SIZE"),
            "cpus_available": len(os.sched_getaffinity(0))}


def family_batches(family=FAMILY):
    family = json.loads((smoke.ROOT / smoke.located("file", smoke.FAMILIES[family][0]))
                        .read_text(encoding="utf-8"))
    return {next(key for key in smoke.CODES if cell["cell_id"].startswith(key + "-")):
            cell["workload"]["size"]["timed_batch"] for cell in family["cells"]}


def observe(build, launches):
    catalogue = catalogued(build)
    files = quality_files(build)
    declared = family_batches()
    rows = {(name, key): row for name, row in build["arms"].items() for key in smoke.CODES}
    qualities, batches = {}, {}
    for (name, key) in rows:
        path = smoke.ROOT / files[name, key][0]
        if sha256_file(path) != files[name, key][1]:
            sys.exit(f"{path} differs from its pinned digest")
        qualities[name, key] = json.loads(path.read_text(encoding="utf-8"))
        # The quality file states the frames per SIMD wave the arm needs.
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
            "effective": effective(name, catalogue[name], qualities[name, key]["settings"]),
            "request_declaration": request["decoder"],
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
    build_path, build = build_record(out_dir)
    observed = observe(build, LAUNCHES)
    arms = {}
    for name, row in sorted(build["arms"].items()):
        arms[name] = {"executable": row["executable"], "identity": arm_identity(build, name),
                      "codes": {}}
        for key, cell in observed[name].items():
            peak, floor = summary(cell.pop("peak")), summary(cell.pop("floor"))
            launcher = summary(cell.pop("launcher"))
            floor["launcher"] = summary(cell.pop("floor_launcher"))
            arms[name]["codes"][key] = {
                **cell, **peak, "launcher": launcher, "floor": floor,
                "above_floor_median_kib": peak["median_kib"] - floor["median_kib"]}
    record = {"cites": ["Cassagne2019"], "method": {**METHOD, "launches": LAUNCHES},
              "pointers": {"build_identity": cited(build_path), "catalogue": build["catalogue"]},
              "recorded_inputs": {"name": smoke.ARCHIVE[0], "sha256": smoke.ARCHIVE[1]},
              "host": host(),
              "arms": arms}
    (out_dir / RECORD).write_text(intra.encoded(record), encoding="utf-8")
    print(out_dir / RECORD)


def structure_failures(record, build):
    """Failures of the record's structure, statistics and per-arm identities.

    Only the identity of each recorded arm decides: other arms and the shared record's
    digest change without invalidating it."""
    failures = []
    if record["cites"] != ["Cassagne2019"]:
        failures.append("cites")
    if record["arms"].keys() != build["arms"].keys():
        failures.append("arms differ from the build record")
    for name, arm in record["arms"].items():
        if name in build["arms"] and arm["identity"] != arm_identity(build, name):
            failures.append(f"{name}: identity differs from the build record")
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


def quality_failures(record):
    """Cells whose quality file or effective settings differ from the committed file."""
    failures = []
    for name, arm in record["arms"].items():
        for key, cell in arm["codes"].items():
            quality = smoke.ROOT / cell["quality"]["path"]
            if sha256_file(quality) != cell["quality"]["sha256"]:
                failures.append(f"{name} {key}: the quality file differs from its recorded digest")
            elif cell["effective"]["settings"] != json.loads(
                    quality.read_text(encoding="utf-8"))["settings"]:
                failures.append(f"{name} {key}: effective settings differ from the quality file")
    return failures


def check_mode(out_dir):
    record = json.loads((out_dir / RECORD).read_text(encoding="utf-8"))
    _, build = build_record(out_dir)
    failures = structure_failures(record, build) + quality_failures(record)
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
