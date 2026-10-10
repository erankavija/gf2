#!/usr/bin/env python3
"""Record the Rust 1.95 build of the gf2 batch decoder arms (jit:ed3d490e).

Builds the arms workspace beside this script in its own target directory and
writes `gf2-arms-build.json`: the toolchain, the command this run executed,
the digest of every executable the workspace declares, and, for every
executable digest an LDPC campaign's build identity record pins, the files of
the repository target directory that carry it. The run fails when the build
changes one of those files.

A pinned executable absent from the building checkout carries the identity
that shows the arms workspace does not reach it: its producing record and the
lockfile of its producing harness are unmodified, the arms workspace builds
into a target directory no pinned executable lives in, and every commit that
touches the arms workspace or this record adds or changes paths only inside
the story directory, which holds no producer of a pinned executable.

Every value comes from a command this run executes or a file it reads, and
none is a clock reading, so a second run over an unchanged tree reproduces the
record.

Usage (from the worktree root): record-gf2-arms-build.py OUT_DIR
"""

import hashlib
import json
import os
import pathlib
import subprocess
import sys
import tomllib

TOOLCHAIN = "1.95"
RUSTFLAGS = "-C target-cpu=native"
PACKAGE = "ldpc-batch-arms"


def output(command, **kwargs):
    return subprocess.run(command, check=True, capture_output=True, text=True,
                          **kwargs).stdout.strip()


ROOT = pathlib.Path(output(["git", "rev-parse", "--show-toplevel"]))


def listed(name):
    """Root-relative live files called `name`, receipt input snapshots excluded."""
    paths = output(["git", "-C", str(ROOT), "ls-files", "--cached", "--others",
                    "--exclude-standard", "--", f":(glob)**/{name}"]).splitlines()
    return [pathlib.Path(path) for path in paths if "inputs" not in pathlib.Path(path).parts]


def located(*query):
    """Root-relative path the repository-file helper prints for `query`."""
    helper = ROOT / listed("repository_files.py")[0]
    return pathlib.Path(output([sys.executable, "-B", str(helper), *query]))


def digest(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def pinned_digests(node, inside=False):
    """(executable, digest) pairs of the executable maps in a build identity."""
    if not isinstance(node, dict):
        return
    for key, value in node.items():
        if inside and isinstance(value, str):
            yield key, value
        elif inside and isinstance(value, dict) and isinstance(value.get("sha256"), str):
            yield key, value["sha256"]
        else:
            yield from pinned_digests(value, key in ("executables", "arm_binaries"))


def pinned_executables():
    """Each pinned digest of an LDPC campaign with the target files carrying it."""
    built = {}
    for path in sorted((ROOT / "target").glob("*/release/*")):
        if path.is_file() and os.access(path, os.X_OK):
            built.setdefault(path.name, []).append(path)
    rows = []
    for record in sorted(listed("build-identity.json")):
        identity = json.loads((ROOT / record).read_text(encoding="utf-8"))
        if not {"prepared_quality", "arm_binaries"} & identity.keys():
            continue
        for name, pinned in sorted(set(pinned_digests(identity))):
            rows.append({
                "record": str(record),
                "executable": name,
                "sha256": pinned,
                "found_at": [str(path.relative_to(ROOT)) for path in built.get(name, [])
                             if digest(path) == pinned],
            })
    return rows


def story_changes(story, scopes):
    """Paths outside `story` changed by the commits that touch any of `scopes`."""
    commits = output(["git", "-C", str(ROOT), "log", "--format=%H", "--",
                      *map(str, scopes)]).split()
    changed = set()
    for commit in commits:
        names = output(["git", "-C", str(ROOT), "show", "--format=", "--name-only",
                        "-m", commit]).splitlines()
        changed.update(name for name in names if story not in pathlib.Path(name).parents)
    return commits, sorted(changed)


def producer_identity(rows, release, story, scopes):
    """Per absent executable, the identity showing the arms workspace leaves it unchanged."""
    _, outside = story_changes(story, scopes)
    if outside:
        sys.exit(f"commits of the arms workspace change paths outside the story: {outside}")
    pinned_directories = {pathlib.Path(path).parent.as_posix()
                          for row in rows for path in row["found_at"]}
    if release.as_posix() in pinned_directories:
        sys.exit("the arms target directory holds a pinned executable")
    for row in rows:
        if row["found_at"]:
            continue
        record = ROOT / row["record"]
        identity = json.loads(record.read_text(encoding="utf-8"))
        harness = identity.get("harness")
        unaffected = {
            "record_sha256": digest(record),
            "paths_changed_outside_story": outside,
        }
        if harness:
            pinned = harness["cargo_lock_sha256"]
            carriers = [str(path) for path in listed("Cargo.lock")
                        if digest(ROOT / path) == pinned]
            if not carriers:
                sys.exit(f"no Cargo.lock has the digest {pinned} the record pins")
            unaffected["harness_lockfile_sha256"] = pinned
            unaffected["harness_lockfile_found_at"] = carriers
        row["producer_unaffected"] = unaffected
    return rows


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    if pathlib.Path.cwd() != ROOT:
        sys.exit("invoke from the worktree root")
    out_dir = pathlib.Path(sys.argv[1])
    arms = located("package-directory", PACKAGE)
    manifest = arms / "Cargo.toml"
    before = pinned_executables()

    environment = {"CARGO_CI_NO_SCCACHE": "1", "RUSTFLAGS": RUSTFLAGS}
    command = ["./scripts/cargo-budget.sh", "cargo", f"+{TOOLCHAIN}", "build", "--offline",
               "--locked", "--release", "--manifest-path", str(manifest)]
    subprocess.run(command, check=True, env={**os.environ, **environment})

    metadata = json.loads(output(
        ["cargo", f"+{TOOLCHAIN}", "metadata", "--offline", "--locked", "--no-deps",
         "--format-version", "1", "--manifest-path", str(manifest)]))
    release = pathlib.Path(metadata["target_directory"]).relative_to(ROOT) / "release"
    binaries = sorted(target["name"] for package in metadata["packages"]
                      for target in package["targets"] if "bin" in target["kind"])
    after = pinned_executables()
    if after != before:
        sys.exit("the build changed a pinned executable of an LDPC campaign")
    story = os.path.commonpath([arms, out_dir.resolve().relative_to(ROOT)])
    after = producer_identity(after, release, pathlib.Path(story), [arms, out_dir])

    record = {
        "schema": "ldpc-batch-arms-build-v1",
        "rustc": output(["rustc", f"+{TOOLCHAIN}", "--version", "--verbose"]),
        "cargo": output(["cargo", f"+{TOOLCHAIN}", "--version"]),
        "build_command": " ".join(
            [f"{name}='{value}'" for name, value in environment.items()] + command),
        "release_profile": tomllib.loads(manifest.read_text(encoding="utf-8"))["profile"]["release"],
        "lockfile_sha256": digest(arms / "Cargo.lock"),
        "target_directory": str(release),
        "executables": {name: digest(release / name) for name in binaries},
        "pinned_executables": after,
    }
    out_dir.mkdir(parents=True, exist_ok=True)
    path = out_dir / "gf2-arms-build.json"
    path.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    print(path)


if __name__ == "__main__":
    main()
