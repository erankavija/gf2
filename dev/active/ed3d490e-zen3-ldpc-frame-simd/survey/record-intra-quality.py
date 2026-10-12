#!/usr/bin/env python3
"""Quality record of the AFF3CT [Cassagne2019] INTRA arm (jit:4252085a).

`record` replays each full recorded bundle once, untimed, under the arm's declared environment,
and writes the prepared quality files the arm checks against and the LLR factor probe; `check`
replays again, writes nothing and exits non-zero when the replay or its closed identity set
(`@/inv/behavioral-evidence-validity`) differs from the record.

Usage (from the worktree root): record-intra-quality.py record|check OUT_DIR
The build record is the git-listed `build-identity.json` that lists the replay executable.
"""

import array
import contextlib
import hashlib
import importlib.util
import json
import math
import os
import pathlib
import subprocess
import sys
import tarfile
import tempfile

sys.dont_write_bytecode = True


def sibling(name):
    path = pathlib.Path(__file__).resolve().with_name(name)
    module = importlib.util.module_from_spec(importlib.util.spec_from_file_location(path.stem, path))
    module.__spec__.loader.exec_module(module)
    return module


smoke = sibling("smoke-gf2-arms.py")
arm_build = sibling("arm_build.py")

ARM = "aff3ct-spa-intra"
EXECUTABLE = "aff3ct-intra-quality"
FAMILY = "aff3ct-intra"
# Zeroed: the pass is untimed, and the arm compares neither.
DIAGNOSTICS = ("memory_bytes", "latency_ns_p50")
INTERVALS = {
    "fer": "Wilson score interval over the frames",
    "ber": "Hoeffding interval over the independent frames",
    "confidence": 0.95,
}
METHOD = {
    "replay": "one untimed pass over every frame of each bundle, under build.environment",
    "request_declaration": "per code, the decoder case the replay receives: the comparator "
                           "cell's declaration, not the arm's settings",
    "effective": "per code, the decoder and LLR factor the replay reports and the settings of "
                 "its quality file",
}
RECORD = "record.json"
PROBE = "llr-scale-probe.json"
FACTOR_VARIABLE = "GF2_AFF3CT_LLR_FACTOR"
PROBE_FACTORS = ("1", "2", "4")
PROBE_METHOD = {
    "corpus_scale": "descriptive: the mean over the `llrs` recorded LLRs of a bundle of the LLR "
                    "signed by its transmitted bit, over the BPSK-AWGN closed-form mean 2/sigma^2",
    "replays": f"one full replay of each recorded bundle per value of {FACTOR_VARIABLE}",
}
# Per code, what a second replay must reproduce beside the quality file.
REPLAYED = ("bundle_digests", "bundle_seed", "request_declaration", "effective", "linked_aff3ct",
            "replayed_frames")


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


def replay(executable, case, environment, scratch):
    case_path, out = scratch / "case.json", scratch / "quality.json"
    case_path.write_text(json.dumps(case), encoding="utf-8")
    run = subprocess.run([str(executable), str(case_path), str(out)], check=True,
                         capture_output=True, text=True, env={**os.environ, **environment})
    observed = json.loads(run.stderr.strip().splitlines()[-1])
    return json.loads(out.read_text(encoding="utf-8")), observed


def identity(build):
    """The replay executable's closed build identity and the arm's declared environment."""
    return {**arm_build.arm_build(build, ARM, EXECUTABLE),
            "environment": build["arms"][ARM]["environment"]}


def locate_build_record():
    for path in smoke.listed("build-identity.json"):
        record = json.loads((smoke.ROOT / path).read_text(encoding="utf-8"))
        if EXECUTABLE in record.get("executables", {}):
            return path, record
    sys.exit(f"no build record lists {EXECUTABLE}")


def pinned_executable(build):
    executable = smoke.ROOT / smoke.release() / EXECUTABLE
    pinned = build["executables"][EXECUTABLE]["sha256"]
    digest = sha256(executable.read_bytes())
    if digest != pinned:
        sys.exit(f"{EXECUTABLE} digest {digest} differs from the build identity's {pinned}")
    return executable


@contextlib.contextmanager
def recorded_cells():
    """A scratch directory and, per comparator cell, its code key, bundle and manifest."""
    base = json.loads((smoke.ROOT / smoke.located("file", smoke.FAMILIES[FAMILY][0]))
                      .read_text(encoding="utf-8"))
    with tempfile.TemporaryDirectory() as temporary:
        scratch = pathlib.Path(temporary)
        with tarfile.open(smoke.by_digest(smoke.ARCHIVE)) as handle:
            handle.extractall(scratch / "inputs", filter="data")
        cells = []
        for cell in base["cells"]:
            cell["code_key"] = key = next(
                key for key in smoke.CODES if cell["cell_id"].startswith(key + "-"))
            bundle = scratch / "inputs" / smoke.CODES[key]["bundle"]
            manifest = json.loads((bundle / "manifest.json").read_text(encoding="utf-8"))
            cells.append((key, cell, bundle, manifest))
        yield scratch, cells


def replays():
    path, build = locate_build_record()
    executable = pinned_executable(build)
    codes, qualities = {}, {}
    with recorded_cells() as (scratch, cells):
        for key, cell, bundle, manifest in cells:
            case = case_of(cell, bundle, manifest["frames"])
            quality, observed = replay(executable, case, build["arms"][ARM]["environment"], scratch)
            qualities[f"{ARM}-{key}.json"] = untimed(quality)
            codes[key] = {
                "bundle": smoke.CODES[key]["bundle"],
                "bundle_digests": {name: manifest[name] for name in
                                   ("h_sha256", "codewords_sha256", "llrs_sha256")},
                "bundle_seed": manifest["seed"],
                "codeword_source": manifest["codeword_source"],
                "request_declaration": {name: value for name, value in case.items()
                                        if name != "bundle"},
                "effective": {"decoder": observed["decoder"],
                              "llr_factor": observed["llr_factor"],
                              "settings": quality["settings"]},
                "linked_aff3ct": observed["linked_aff3ct"],
                "replayed_frames": quality["frames"],
            }
    record = {
        "cites": ["Cassagne2019"],
        "arm": ARM,
        "executable": EXECUTABLE,
        "method": METHOD,
        "build": identity(build),
        "build_record": {"path": path, "sha256": sha256((smoke.ROOT / path).read_bytes())},
        "recorded_inputs": {"name": smoke.ARCHIVE[0], "sha256": smoke.ARCHIVE[1]},
        "intervals": {**INTERVALS, "method": next(iter(qualities.values()))["interval_method"],
                      "samples": "replayed_frames of each code"},
        "diagnostics_zeroed": list(DIAGNOSTICS),
        "host": host_facts(),
        "codes": codes,
    }
    return record, qualities


def corpus_scale(bundle, manifest):
    llrs = array.array("f")
    llrs.frombytes((bundle / "llrs.f32").read_bytes())
    sent = (bundle / "codewords.bin").read_bytes()
    mean = math.fsum(-value if bit else value for value, bit in zip(llrs, sent)) / len(llrs)
    return {"sigma": manifest["sigma"], "llrs": len(llrs),
            "mean_signed_llr_over_closed_form_mean": mean / (2 / manifest["sigma"] ** 2)}


def probes():
    _, build = locate_build_record()
    executable = pinned_executable(build)
    declared = build["arms"][ARM]["environment"]
    result = {"cites": ["Cassagne2019"], "arm": ARM, "executable": EXECUTABLE,
              "build": identity(build), "method": PROBE_METHOD,
              "recorded_inputs": {"name": smoke.ARCHIVE[0], "sha256": smoke.ARCHIVE[1]},
              "intervals": INTERVALS, "codes": {}}
    with recorded_cells() as (scratch, cells):
        for key, cell, bundle, manifest in cells:
            case = case_of(cell, bundle, manifest["frames"])
            replayed = []
            for factor in PROBE_FACTORS:
                quality, observed = replay(executable, case,
                                           {**declared, FACTOR_VARIABLE: factor}, scratch)
                replayed.append({FACTOR_VARIABLE: factor, "llr_factor": observed["llr_factor"],
                                 **{name: quality[name] for name in
                                    ("frames", "frame_errors", "fer_interval",
                                     "bits", "bit_errors", "ber_interval")}})
            result["codes"][key] = {**corpus_scale(bundle, manifest), "replays": replayed}
    return result


def encoded(value):
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def record_mode(out_dir):
    record, qualities = replays()
    out_dir.mkdir(parents=True, exist_ok=True)
    for name, quality in qualities.items():
        (out_dir / name).write_text(encoded(quality), encoding="utf-8")
        record["codes"][name.removeprefix(f"{ARM}-").removesuffix(".json")]["quality"] = {
            "file": name, "sha256": sha256(encoded(quality).encode())}
    (out_dir / RECORD).write_text(encoded(record), encoding="utf-8")
    (out_dir / PROBE).write_text(encoded(probes()), encoding="utf-8")
    print(out_dir / RECORD)


def check_failures(committed, stored, record, qualities):
    """Where the second replay `record`, `qualities` differs from the committed record and its
    quality files `stored`, per code and then for the replay's identity."""
    failures = {key: [] for key in committed["codes"]}
    for key, code in committed["codes"].items():
        replayed = record["codes"].get(key, {})
        name = code["quality"]["file"]
        failures[key] += [f"quality field {field} differs"
                          for field in differences(qualities.get(name, {}), stored[name])]
        failures[key] += [f"{field} differs" for field in REPLAYED
                          if replayed.get(field) != code[field]]
    failures[""] = [f"{name} differs" for name in ("build", "host")
                    if record[name] != committed[name]]
    if record["codes"].keys() != committed["codes"].keys():
        failures[""].append("codes differ")
    return failures


def check_mode(out_dir):
    committed = json.loads((out_dir / RECORD).read_text(encoding="utf-8"))
    stored = {code["quality"]["file"]: json.loads(
        (out_dir / code["quality"]["file"]).read_text(encoding="utf-8"))
        for code in committed["codes"].values()}
    record, qualities = replays()
    failures = check_failures(committed, stored, record, qualities)
    for key in committed["codes"]:
        print(f"{key}: second replay {'differs' if failures[key] else 'reproduces the record'}")
    print(f"executable {EXECUTABLE} sha256 {record['build']['executable_sha256']}")
    found = [f"{key}: {failure}" if key else failure
             for key, group in failures.items() for failure in group]
    for failure in found:
        print(f"DIFFERENCE {failure}")
    sys.exit(1 if found else 0)


def main():
    if len(sys.argv) != 3 or sys.argv[1] not in ("record", "check"):
        sys.exit(__doc__)
    if pathlib.Path.cwd() != smoke.ROOT:
        sys.exit("invoke from the worktree root")
    {"record": record_mode, "check": check_mode}[sys.argv[1]](pathlib.Path(sys.argv[2]).resolve())


if __name__ == "__main__":
    main()
