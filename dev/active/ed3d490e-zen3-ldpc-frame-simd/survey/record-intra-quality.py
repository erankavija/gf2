#!/usr/bin/env python3
"""Prepare and check the quality record of the AFF3CT [Cassagne2019] INTRA arm (jit:4252085a).

`record` replays each full recorded bundle once, untimed, with the arm
workspace's `aff3ct-intra-quality` and writes, per code, the prepared quality
file the arm takes in `GF2_LDPC_QUALITY` plus `record.json`, which pins the
executable, bundles and frozen configuration and states the decoder the
replay observed. `check` replays again and exits non-zero on any difference
from the committed record; it writes nothing into the repository.

The memory and latency diagnostics of the quality form are zero: nothing is
timed. The bundles, frozen decoder settings and file names are those of the
`smoke-gf2-arms.py` group `aff3ct` and the `07ca8585` comparator addendum.

Usage (from the worktree root): record-intra-quality.py record|check OUT_DIR
OUT_DIR holds `record.json`; the arms build identity is `OUT_DIR/../build-identity.json`.
"""

import hashlib
import importlib.util
import json
import pathlib
import subprocess
import sys
import tarfile
import tempfile

sys.dont_write_bytecode = True
SMOKE = pathlib.Path(__file__).resolve().with_name("smoke-gf2-arms.py")
smoke = importlib.util.module_from_spec(importlib.util.spec_from_file_location("smoke", SMOKE))
smoke.__spec__.loader.exec_module(smoke)

ARM = "aff3ct-spa-intra"
EXECUTABLE = "aff3ct-intra-quality"
FAMILY = "aff3ct-intra"
# Quality-form fields that are diagnostics of a timed or resident-memory pass.
DIAGNOSTICS = ("memory_bytes", "latency_ns_p50")
# Confidence the quality form fixes in its `interval_method` identifier.
INTERVALS = {
    "fer": "Wilson score interval over the frames",
    "ber": "Hoeffding interval over the independent frames",
    "confidence": 0.95,
}
RECORD = "record.json"


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def untimed(quality):
    """`quality` with the timing and memory diagnostics zeroed."""
    return {**quality, **dict.fromkeys(DIAGNOSTICS, 0)}


def case_of(cell, bundle, frames):
    """The decoder case of an addendum cell over the whole of `bundle`."""
    decoder = cell["decoder"]
    return {
        "bundle": str(bundle),
        "code": smoke.CODES[cell["code_key"]]["code"],
        "iteration_cap": decoder["iteration_cap"],
        "normalization_factor": decoder["normalization"]["factor"],
        "syndrome_stopping": decoder["stopping"]["kind"] == "syndrome",
        "batch_size": cell["workload"]["size"]["timed_batch"],
        "quality_frames": frames,
    }


def differences(replayed, committed):
    """Names of the fields in which two quality records differ."""
    left, right = untimed(replayed), untimed(committed)
    return [name for name in sorted(left.keys() | right.keys()) if left.get(name) != right.get(name)]


def host_facts():
    """The CPU the replay ran on, as /proc/cpuinfo reports it."""
    text = pathlib.Path("/proc/cpuinfo").read_text(encoding="utf-8")
    first = text.split("\n\n")[0]
    fields = dict(line.split(":", 1) for line in first.splitlines() if ":" in line)
    flags = fields.get("flags", "").split()
    return {"cpu_model": fields.get("model name", "").strip(),
            "avx2": "avx2" in flags, "fma": "fma" in flags}


def replay(executable, case, scratch):
    """One replay of `case`: its quality record and the decoder it observed."""
    case_path, out = scratch / "case.json", scratch / "quality.json"
    case_path.write_text(json.dumps(case), encoding="utf-8")
    run = subprocess.run([str(executable), str(case_path), str(out)], check=True,
                         capture_output=True, text=True)
    observed = json.loads(run.stderr.strip().splitlines()[-1])
    return json.loads(out.read_text(encoding="utf-8")), observed


def replays(executable, build_identity):
    """Replays every code of the recorded bundles; returns `(record, qualities)`."""
    base = json.loads((smoke.ROOT / smoke.located("file", smoke.FAMILIES[FAMILY][0]))
                      .read_text(encoding="utf-8"))
    archive = smoke.by_digest(smoke.ARCHIVE)
    pinned = build_identity["executables"][EXECUTABLE]["sha256"]
    digest = sha256(executable.read_bytes())
    if digest != pinned:
        sys.exit(f"{EXECUTABLE} digest {digest} differs from the build identity's {pinned}")
    codes, qualities = {}, {}
    with tempfile.TemporaryDirectory() as temporary:
        scratch = pathlib.Path(temporary)
        with tarfile.open(archive) as handle:
            handle.extractall(scratch / "inputs", filter="data")
        for cell in base["cells"]:
            cell["code_key"] = key = next(
                key for key in smoke.CODES if cell["cell_id"].startswith(key + "-"))
            bundle = scratch / "inputs" / smoke.CODES[key]["bundle"]
            manifest = json.loads((bundle / "manifest.json").read_text(encoding="utf-8"))
            case = case_of(cell, bundle, manifest["frames"])
            quality, observed = replay(executable, case, scratch)
            qualities[f"{ARM}-{key}.json"] = untimed(quality)
            codes[key] = {
                "bundle": smoke.CODES[key]["bundle"],
                "bundle_digests": {name: manifest[name] for name in
                                   ("h_sha256", "codewords_sha256", "llrs_sha256")},
                "bundle_seed": manifest["seed"],
                "codeword_source": manifest["codeword_source"],
                "frozen_case": {name: value for name, value in case.items() if name != "bundle"},
                "replayed_frames": quality["frames"],
                "observed_decoder": observed["decoder"],
                "linked_aff3ct": observed["linked_aff3ct"],
                "settings": quality["settings"],
            }
    record = {
        "cites": ["Cassagne2019"],
        "arm": ARM,
        "executable": {"name": EXECUTABLE, "sha256": digest},
        "build_identity": {"path": str(build_identity["path"]),
                           "sha256": build_identity["sha256"]},
        "recorded_inputs": {"name": smoke.ARCHIVE[0], "sha256": smoke.ARCHIVE[1]},
        "intervals": {**INTERVALS, "method": next(iter(qualities.values()))["interval_method"],
                      "samples": "replayed_frames of each code"},
        "diagnostics_zeroed": list(DIAGNOSTICS),
        "host": host_facts(),
        "codes": codes,
    }
    return record, qualities


def load_build_identity(out_dir):
    path = out_dir.parent / "build-identity.json"
    data = path.read_bytes()
    return {**json.loads(data), "path": path.relative_to(smoke.ROOT), "sha256": sha256(data)}


def encoded(value):
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def record_mode(out_dir):
    record, qualities = replays(
        smoke.ROOT / smoke.release() / EXECUTABLE, load_build_identity(out_dir))
    out_dir.mkdir(parents=True, exist_ok=True)
    for name, quality in qualities.items():
        (out_dir / name).write_text(encoded(quality), encoding="utf-8")
        record["codes"][name.removeprefix(f"{ARM}-").removesuffix(".json")]["quality"] = {
            "file": name, "sha256": sha256(encoded(quality).encode())}
    (out_dir / RECORD).write_text(encoded(record), encoding="utf-8")
    print(out_dir / RECORD)


def check_mode(out_dir):
    committed = json.loads((out_dir / RECORD).read_text(encoding="utf-8"))
    record, qualities = replays(
        smoke.ROOT / smoke.release() / EXECUTABLE, load_build_identity(out_dir))
    failures = []
    for key, code in committed["codes"].items():
        replayed = record["codes"][key]
        stored = json.loads((out_dir / code["quality"]["file"]).read_text(encoding="utf-8"))
        if sha256((out_dir / code["quality"]["file"]).read_bytes()) != code["quality"]["sha256"]:
            failures.append(f"{key}: the quality file differs from its recorded digest")
        found = [f"{key}: quality field {name} differs"
                 for name in differences(qualities[code["quality"]["file"]], stored)]
        found += [f"{key}: {name} differs" for name in
                  ("bundle_digests", "bundle_seed", "frozen_case", "observed_decoder",
                   "linked_aff3ct", "settings", "replayed_frames")
                  if replayed[name] != code[name]]
        print(f"{key}: second replay {'differs' if found else 'reproduces the record'}")
        failures += found
    failures += [f"{name} differs" for name in ("executable", "build_identity", "recorded_inputs")
                 if record[name] != committed[name]]
    print(f"executable {record['executable']['name']} sha256 {record['executable']['sha256']}")
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
