#!/usr/bin/env python3
"""Write the producing manifest of the matvec sweep campaign (jit:63bad95d).

The manifest starts from the seam campaign's reviewed manifest, found through
the campaign declaration that names `dbd8787d`, and replaces that declaration
by this campaign's. Every tracked Rust source of the producer's library
packages that the seam manifest omits must carry a class below; an
unclassified source fails the script, so a file added to those packages cannot
stay outside the campaign's identity unnoticed.

  behavior  compiled into the producer and able to change what a cell measures
  build     compiled into the producer, test support only
  excluded  not compiled into the producer build

Usage: make-producing-build-inputs.py [--check]
"""

import json
import sys

from locate import HERE, ROOT, package, tracked

ISSUE = HERE.parent
OUTPUT = ISSUE / "producing-build-inputs.json"
DECLARATION = "campaign-declaration.json"
LIBRARY_PACKAGES = ("gf2-core", "gf2-kernels-simd", "tuning-campaign-support")

# Package-relative sources the seam manifest omits.
ADDED = {
    "gf2-core": {
        "src/gf2m/byte_table.rs": "behavior",
        "src/residual_shift.rs": "behavior",
        "src/dispatch_contract.rs": "build",
        "src/test_scratch.rs": "build",
        "src/bitvec_sync_tests.rs": "excluded",
        "src/compute/batch_tests.rs": "excluded",
        "src/field/axiom_tests.rs": "excluded",
        "src/field/test_random_matrix.rs": "excluded",
        "src/gf2m/thread_safety_tests.rs": "excluded",
        "src/kernels/test_utils.rs": "excluded",
    },
    "gf2-kernels-simd": {
        "src/m4rm.rs": "behavior",
        "src/shift_funnel.rs": "behavior",
        "src/x86/popcount.rs": "behavior",
        "src/x86/shift_funnel.rs": "behavior",
    },
    "tuning-campaign-support": {
        "src/repository.rs": "behavior",
        "src/scratch.rs": "excluded",
        "src/bin/ab-smoke-workload.rs": "excluded",
        "src/bin/benchmark-ab-runner.rs": "excluded",
        "src/bin/benchmark-acceptance.rs": "excluded",
    },
}


def declaration(issue):
    """Root-relative paths and the one content of the campaign declaration naming `issue`."""
    found = [
        (path, json.loads((ROOT / path).read_text()))
        for path in tracked(f"**/{DECLARATION}")
    ]
    paths = sorted(path for path, value in found if value.get("issue") == issue)
    contents = {json.dumps(value, sort_keys=True) for _, value in found
                if value.get("issue") == issue}
    if len(contents) != 1:
        raise SystemExit(f"{len(contents)} campaign declarations name {issue}")
    return paths, json.loads(next(iter(contents)))


def main():
    seam_paths, seam = declaration("dbd8787d")
    (own_path,), own = declaration("63bad95d")
    base = json.loads((ROOT / seam["producing_manifest"]).read_text())
    lists = {
        name: {own_path if path in seam_paths else path for path in base[name]}
        for name in ("behavior_sources", "lifecycle_sources", "build_inputs")
    }
    for name in LIBRARY_PACKAGES:
        directory = package(name)
        classes = {f"{directory}/{relative}": kind for relative, kind in ADDED[name].items()}
        sources = set(tracked(f"{directory}/src/**/*.rs"))
        omitted = sources - set(base["build_inputs"])
        if omitted != set(classes):
            raise SystemExit(
                f"{name}: sources without a class or classes without a source: "
                f"{sorted(omitted ^ set(classes))}"
            )
        for path, kind in classes.items():
            if kind in ("behavior", "build"):
                lists["build_inputs"].add(path)
            if kind == "behavior":
                lists["behavior_sources"].add(path)
    missing = sorted(path for paths in lists.values() for path in paths
                     if not (ROOT / path).is_file())
    if missing:
        raise SystemExit(f"manifest names absent files: {missing}")
    text = json.dumps(
        {"schema": base["schema"], **{name: sorted(paths) for name, paths in lists.items()}},
        indent=2,
    ) + "\n"
    if own["producing_manifest"] != str(OUTPUT.relative_to(ROOT)):
        raise SystemExit("the declaration names another producing manifest")
    if sys.argv[1:] == ["--check"]:
        if OUTPUT.read_text() != text:
            raise SystemExit(f"{OUTPUT.relative_to(ROOT)} differs from its generator")
        print(f"{OUTPUT.relative_to(ROOT)}: matches its generator")
        return
    OUTPUT.write_text(text)
    print(f"{OUTPUT.relative_to(ROOT)}: " + ", ".join(
        f"{len(paths)} {name}" for name, paths in lists.items()))


if __name__ == "__main__":
    main()
