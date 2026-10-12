#!/usr/bin/env python3
"""Quality record of the AFF3CT [Cassagne2019] INTRA arm (jit:4252085a).

`record` replays each full recorded bundle once, untimed, and writes the prepared quality files
the arm checks against; `check` replays again, writes nothing and exits non-zero on any
difference within the replay's closed identity set (`@/inv/behavioral-evidence-validity`).

Usage (from the worktree root): record-intra-quality.py record|check OUT_DIR
The build record is the git-listed `build-identity.json` that lists the replay executable.
"""

import array
import hashlib
import importlib.util
import json
import math
import pathlib
import shutil
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
PACKAGE = "ldpc-aff3ct-intra-arm"
# Zeroed: the pass is untimed, and the arm compares neither.
DIAGNOSTICS = ("memory_bytes", "latency_ns_p50")
INTERVALS = {
    "fer": "Wilson score interval over the frames",
    "ber": "Hoeffding interval over the independent frames",
    "confidence": 0.95,
}
RECORD = "record.json"
PROBE = "llr-scale-probe.json"
PROBE_SCALES = (1.0, 2.0, 4.0)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def untimed(quality):
    return {**quality, **dict.fromkeys(DIAGNOSTICS, 0)}


def case_of(cell, bundle, frames):
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
    left, right = untimed(replayed), untimed(committed)
    return [name for name in sorted(left.keys() | right.keys()) if left.get(name) != right.get(name)]


def host_facts(text=None):
    if text is None:
        text = pathlib.Path("/proc/cpuinfo").read_text(encoding="utf-8")
    first = text.split("\n\n")[0]
    fields = {key.strip(): value.strip() for key, _, value in
              (line.partition(":") for line in first.splitlines()) if key.strip()}
    flags = fields.get("flags", "").split()
    return {"cpu_model": fields.get("model name", ""),
            "avx2": "avx2" in flags, "fma": "fma" in flags}


def replay(executable, case, scratch):
    case_path, out = scratch / "case.json", scratch / "quality.json"
    case_path.write_text(json.dumps(case), encoding="utf-8")
    run = subprocess.run([str(executable), str(case_path), str(out)], check=True,
                         capture_output=True, text=True)
    observed = json.loads(run.stderr.strip().splitlines()[-1])
    return json.loads(out.read_text(encoding="utf-8")), observed


def arm_build(build_record):
    """The part of the shared build record that identifies this arm's executable."""
    command = next(c for c in build_record["build_commands"] if PACKAGE in c["argv"])
    return {"rustc": build_record["rustc"], "cargo": build_record["cargo"],
            "release_profile": build_record["release_profile"], "command": command,
            "executable": build_record["executables"][EXECUTABLE]}


def locate_build_record():
    for path in smoke.listed("build-identity.json"):
        record = json.loads((smoke.ROOT / path).read_text(encoding="utf-8"))
        if EXECUTABLE in record.get("executables", {}):
            return path, record
    sys.exit(f"no build record lists {EXECUTABLE}")


def replays(executable):
    base = json.loads((smoke.ROOT / smoke.located("file", smoke.FAMILIES[FAMILY][0]))
                      .read_text(encoding="utf-8"))
    archive = smoke.by_digest(smoke.ARCHIVE)
    path, build_record = locate_build_record()
    pinned = build_record["executables"][EXECUTABLE]["sha256"]
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
        "build": arm_build(build_record),
        "build_record": {"path": path, "sha256": sha256((smoke.ROOT / path).read_bytes())},
        "recorded_inputs": {"name": smoke.ARCHIVE[0], "sha256": smoke.ARCHIVE[1]},
        "intervals": {**INTERVALS, "method": next(iter(qualities.values()))["interval_method"],
                      "samples": "replayed_frames of each code"},
        "diagnostics_zeroed": list(DIAGNOSTICS),
        "host": host_facts(),
        "codes": codes,
    }
    return record, qualities


def scaled_bundle(source, target, scale):
    target.mkdir(parents=True)
    for name in ("h.alist", "codewords.bin"):
        shutil.copy(source / name, target / name)
    llrs = array.array("f")
    llrs.frombytes((source / "llrs.f32").read_bytes())
    scaled = array.array("f", (value * scale for value in llrs))
    (target / "llrs.f32").write_bytes(scaled.tobytes())
    manifest = json.loads((source / "manifest.json").read_text(encoding="utf-8"))
    manifest["llrs_sha256"] = sha256(scaled.tobytes())
    (target / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")


def llr_scale(bundle):
    manifest = json.loads((bundle / "manifest.json").read_text(encoding="utf-8"))
    llrs = array.array("f")
    llrs.frombytes((bundle / "llrs.f32").read_bytes())
    sent = (bundle / "codewords.bin").read_bytes()
    mean = math.fsum(-value if bit else value for value, bit in zip(llrs, sent)) / len(llrs)
    return {"sigma": manifest["sigma"], "mean_signed_llr_over_true_scale":
            mean / (2 / manifest["sigma"] ** 2)}


def probes(executable):
    base = json.loads((smoke.ROOT / smoke.located("file", smoke.FAMILIES[FAMILY][0]))
                      .read_text(encoding="utf-8"))
    result = {"cites": ["Cassagne2019"], "scales": list(PROBE_SCALES),
              "intervals": INTERVALS, "codes": {}}
    with tempfile.TemporaryDirectory() as temporary:
        scratch = pathlib.Path(temporary)
        with tarfile.open(smoke.by_digest(smoke.ARCHIVE)) as handle:
            handle.extractall(scratch / "inputs", filter="data")
        for cell in base["cells"]:
            cell["code_key"] = key = next(
                key for key in smoke.CODES if cell["cell_id"].startswith(key + "-"))
            source = scratch / "inputs" / smoke.CODES[key]["bundle"]
            frames = json.loads((source / "manifest.json").read_text(encoding="utf-8"))["frames"]
            replayed = []
            for scale in PROBE_SCALES:
                bundle = scratch / f"scaled-{key}-{scale}"
                scaled_bundle(source, bundle, scale)
                quality, _ = replay(executable, case_of(cell, bundle, frames), scratch)
                replayed.append({"scale": scale, "frames": quality["frames"],
                                 "frame_errors": quality["frame_errors"],
                                 "fer_interval": quality["fer_interval"],
                                 "bit_errors": quality["bit_errors"],
                                 "iterations_mean": quality["iterations"]["mean"]})
            result["codes"][key] = {**llr_scale(source), "replays": replayed}
    return result


def encoded(value):
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def record_mode(out_dir):
    record, qualities = replays(smoke.ROOT / smoke.release() / EXECUTABLE)
    out_dir.mkdir(parents=True, exist_ok=True)
    for name, quality in qualities.items():
        (out_dir / name).write_text(encoded(quality), encoding="utf-8")
        record["codes"][name.removeprefix(f"{ARM}-").removesuffix(".json")]["quality"] = {
            "file": name, "sha256": sha256(encoded(quality).encode())}
    (out_dir / RECORD).write_text(encoded(record), encoding="utf-8")
    (out_dir / PROBE).write_text(encoded(probes(smoke.ROOT / smoke.release() / EXECUTABLE)),
                                 encoding="utf-8")
    print(out_dir / RECORD)


def check_mode(out_dir):
    committed = json.loads((out_dir / RECORD).read_text(encoding="utf-8"))
    record, qualities = replays(smoke.ROOT / smoke.release() / EXECUTABLE)
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
    failures += [f"{name} differs" for name in
                 ("executable", "build", "host")
                 if record[name] != committed[name]]
    if probes(smoke.ROOT / smoke.release() / EXECUTABLE) != json.loads(
            (out_dir / PROBE).read_text(encoding="utf-8")):
        failures.append(f"{PROBE} differs")
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
