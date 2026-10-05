#!/usr/bin/env python3
"""Regenerates the producing-input manifests of the LDPC campaigns (jit:b2e09d41).

Each manifest is written by its own committed generator, and each generator
also records a build identity from executables it is given. This script
rewrites the manifests alone:

- a generator that exposes `producing_inputs()` is imported and that function's
  result is written the way the generator's `main` writes it;
- the `f63a2464` generator computes its manifest inside `main`, so it runs
  unmodified at the root of a scratch copy of the working tree's files, on the
  arguments its committed build identity records and on stub executables, and
  only its manifest is copied back.

A manifest holds paths and no digest, so no stub byte reaches it.

Usage (from any directory of the checkout):
  regenerate-manifests.py
"""

import importlib.util
import json
import os
import pathlib
import shutil
import subprocess
import sys

# (generator, manifest it writes), in dependency order: each later manifest
# extends the one before it.
IMPORTED = [
    ("3be770d5/survey/record-preparation.py", "producing-inputs.json"),
    ("07ca8585/survey/record-preparation.py", "producing-inputs.json"),
    ("07ca8585/survey/record-kernel-preparation.py", "producing-inputs-kernel.json"),
]
SCRATCH_RUN = "f63a2464/survey/record-preparation.py"
SCRATCH_IDENTITY = "f63a2464/preparation/build-identity.json"

HERE = pathlib.Path(__file__).resolve().parent


def git(*arguments, cwd):
    return subprocess.run(["git", *arguments], cwd=cwd, check=True,
                          capture_output=True, text=True).stdout


ROOT = pathlib.Path(git("rev-parse", "--show-toplevel", cwd=HERE).strip())


def live(suffix):
    """Root-relative path of the one live file whose path ends with `suffix`."""
    (path,) = [path for path in git("ls-files", "--", f":(glob)**/{suffix}", cwd=ROOT).split()
               if "inputs" not in pathlib.Path(path).parts]
    return pathlib.Path(path)


def load(path):
    spec = importlib.util.spec_from_file_location(path.stem.replace("-", "_"), ROOT / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def scratch_tree():
    """A git repository holding the working tree's tracked and unignored files."""
    target = json.loads(subprocess.run(
        ["cargo", "metadata", "--offline", "--no-deps", "--format-version", "1"],
        cwd=ROOT, check=True, capture_output=True, text=True).stdout)["target_directory"]
    tree = pathlib.Path(target) / "b2e09d41-manifests"
    shutil.rmtree(tree, ignore_errors=True)
    tree.mkdir(parents=True)
    listing = git("ls-files", "-z", "--cached", "--others", "--exclude-standard", cwd=ROOT)
    for name in filter(None, listing.split("\0")):
        if (ROOT / name).is_file():
            (tree / name).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / name, tree / name)
    git("init", "-q", cwd=tree)
    return tree


def run_in_scratch():
    generator = live(SCRATCH_RUN)
    identity_path = live(SCRATCH_IDENTITY)
    identity = json.loads((ROOT / identity_path).read_text())
    module = load(generator)
    tree = scratch_tree()
    stubs = tree / "target/stubs"
    for generation, names in (("baseline", module.BASELINE_ARMS),
                              ("candidate", module.CANDIDATE_ARMS)):
        (stubs / generation).mkdir(parents=True)
        for name in names:
            (stubs / generation / name).write_text(name)
    (stubs / "aff3ct").mkdir()
    (quality,) = {str(pathlib.Path(path).parent) for path in identity["prepared_quality"]}
    subprocess.run(
        [sys.executable, "-B", str(generator), str(identity_path.parent),
         "--aff3ct-root", str(stubs / "aff3ct"),
         "--baseline-dir", str(stubs / "baseline"),
         "--candidate-dir", str(stubs / "candidate"),
         "--inputs-archive", identity["inputs"]["archive"],
         "--quality-dir", quality],
        cwd=tree, check=True, capture_output=True, text=True)
    manifest = generator.parent / "producing-inputs.json"
    shutil.copyfile(tree / manifest, ROOT / manifest)
    return manifest


def main():
    os.chdir(ROOT)
    sys.dont_write_bytecode = True
    for generator, name in IMPORTED:
        generator = live(generator)
        manifest = generator.parent / name
        manifest.write_text(json.dumps(load(generator).producing_inputs(), indent=2) + "\n")
        print(manifest)
    print(run_in_scratch())


if __name__ == "__main__":
    main()
