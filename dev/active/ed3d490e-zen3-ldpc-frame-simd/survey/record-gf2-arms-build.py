#!/usr/bin/env python3
"""Record the Rust 1.95 build of the batch decoder arms (jit:ed3d490e).

Builds the arms workspace beside this script in its own target directory and
writes `gf2-arms-build.json`: the toolchain, the command this run executed,
the digest of every executable the build produces, what this checkout shows
about each executable digest an LDPC campaign's build identity record pins,
and how the arms catalogue `arms.json` beside the workspace agrees with both.

Given `AFF3CT_ROOT`, the staged AFF3CT [Cassagne2019] tree, a second command
builds the workspace's other member packages, which link it. The run then
writes `aff3ct-arms-build.json` instead and adds the tree's revision and the
digest of its static library, as this run reads them, with the build identity
records that pin each.

Content alone decides, and the run fails on any of:

- the files of the repository target directory bearing a pinned executable's
  name, or their digests, differ after the build from before it;
- the arms workspace is not its own workspace root with its target directory
  inside it;
- the arms target directory holds a file with a pinned executable's name or
  digest, or the arms workspace declares a package or target under such a name;
- the catalogue names a pinned arm whose record, found by content, does not
  pin that executable digest, or an arm of a built package that the build
  does not produce;
- no build identity record pins the digest of the AFF3CT static library.

The rest is observation. `found_at` lists the target files carrying a pinned
digest, and is empty for an executable this checkout never built.
`pinned_records` gives each build identity record's digest and, per file
digest the record pins, the live repository files with that content.
`carried_by` empty says no live file has that content: the pin names an
archive member, a file outside the repository, or a file changed since. A
changed producing input leaves the pinned executable the one its campaign
measured. Git supplies the list of working-tree files; commit ids and history
are informational (`@/inv/behavioral-evidence-validity`).

Every value comes from a command this run executes or a file it reads, and
none is a clock reading, so a second run over an unchanged tree reproduces the
record.

Usage (from the worktree root): record-gf2-arms-build.py OUT_DIR [AFF3CT_ROOT]
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


def live_files(name="*"):
    """Root-relative live files called `name`, receipt input snapshots excluded."""
    paths = output(["git", "-C", str(ROOT), "ls-files", "-z", "--cached", "--others",
                    "--exclude-standard", "--", f":(glob)**/{name}"]).split("\0")
    return sorted(pathlib.Path(path) for path in paths
                  if path and "inputs" not in pathlib.Path(path).parts
                  and (ROOT / path).is_file())


def located(*query):
    """Root-relative path the repository-file helper prints for `query`."""
    helper = ROOT / live_files("repository_files.py")[0]
    return pathlib.Path(output([sys.executable, "-B", str(helper), *query]))


def digest(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


EXECUTABLE_MAPS = ("executables", "arm_binaries", "quality_arm_binaries")


def is_digest(value):
    return isinstance(value, str) and len(value) == 64 and set(value) <= set("0123456789abcdef")


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
            yield from pinned_digests(value, key in EXECUTABLE_MAPS)


def pinned_files(node, pointer=""):
    """(JSON pointer, digest) of every digest a build identity pins outside its executable maps."""
    if isinstance(node, dict):
        for key, value in node.items():
            if key not in EXECUTABLE_MAPS:
                yield from pinned_files(value, f"{pointer}/{key}")
    elif isinstance(node, list):
        for index, value in enumerate(node):
            yield from pinned_files(value, f"{pointer}/{index}")
    elif is_digest(node):
        yield pointer, node


def identity_records():
    """Root-relative build identity record of each LDPC campaign, with its content."""
    for record in live_files("build-identity.json"):
        identity = json.loads((ROOT / record).read_text(encoding="utf-8"))
        if {"prepared_quality", "arm_binaries"} & identity.keys():
            yield record, identity


def pinned_executables():
    """Each pinned executable digest and the digest of every target file bearing its name."""
    built = {}
    for path in sorted((ROOT / "target").glob("*/release/*")):
        if path.is_file() and os.access(path, os.X_OK):
            built.setdefault(path.name, {})[str(path.relative_to(ROOT))] = digest(path)
    rows = []
    for record, identity in identity_records():
        for name, pinned in sorted(set(pinned_digests(identity))):
            rows.append({
                "record": str(record),
                "executable": name,
                "sha256": pinned,
                "found_at": [path for path, found in built.get(name, {}).items()
                             if found == pinned],
            })
    names = {row["executable"] for row in rows}
    return rows, {name: files for name, files in built.items() if name in names}


def pinned_records():
    """Per build identity record, its digest and the live files carrying each file digest it pins."""
    carriers = {}
    for path in live_files():
        carriers.setdefault(digest(ROOT / path), []).append(str(path))
    return {
        str(record): {
            "sha256": digest(ROOT / record),
            "pins": [{"pin": pointer, "sha256": pinned, "carried_by": carriers.get(pinned, [])}
                     for pointer, pinned in pinned_files(identity)],
        }
        for record, identity in identity_records()
    }


def isolation(metadata, arms, rows):
    """The content showing the arms build reaches no pinned executable; exits when it does."""
    target = pathlib.Path(metadata["target_directory"])
    if pathlib.Path(metadata["workspace_root"]) != ROOT / arms or ROOT / arms not in target.parents:
        sys.exit("the arms workspace shares a workspace root or a target directory")
    declared = sorted({package["name"] for package in metadata["packages"]}
                      | {built["name"] for package in metadata["packages"]
                         for built in package["targets"]})
    names = {row["executable"] for row in rows}
    digests = {row["sha256"] for row in rows}
    shared = sorted(names & set(declared))
    held = sorted(str(path.relative_to(ROOT)) for path in target.rglob("*")
                  if path.is_file() and (path.name in names or (
                      os.access(path, os.X_OK) and digest(path) in digests)))
    if shared or held:
        sys.exit(f"the arms workspace reaches a pinned executable: names {shared}, files {held}")
    return {
        "workspace_root": str(arms),
        "manifest_sha256": digest(arms / "Cargo.toml"),
        "lockfile_sha256": digest(arms / "Cargo.lock"),
        "target_directory": str(target.relative_to(ROOT)),
        "declared_names": declared,
        "names_shared_with_pinned_executables": shared,
        "pinned_names_or_digests_in_target_directory": held,
    }


def catalogue(arms, rows, records, executables, packages):
    """What the catalogue beside the arms workspace names; exits on a disagreement.

    A workspace arm of a package outside the built `packages` is listed unbuilt.
    """
    path = arms.parent / "arms.json"
    named = json.loads(path.read_text(encoding="utf-8"))["arms"]
    by_digest = {record["sha256"]: name for name, record in records.items()}
    pinned, workspace, unbuilt = [], [], []
    for name, arm in sorted(named.items()):
        if arm["source"] == "workspace":
            if arm["package"] not in packages:
                unbuilt.append(name)
                continue
            if arm["executable"] not in executables:
                sys.exit(f"the build produces no executable of the catalogued arm {name}")
            workspace.append({"arm": name, "executable": arm["executable"],
                              "sha256": executables[arm["executable"]]})
            continue
        record = by_digest.get(arm["pinned_by"]["sha256"])
        found = [row for row in rows if row["record"] == record
                 and row["executable"] == arm["executable"]
                 and row["sha256"] == arm["executable_sha256"]]
        if not found:
            sys.exit(f"no live record pins the executable digest of the catalogued arm {name}")
        pinned.append({"arm": name, **found[0]})
    return {"path": str(path), "sha256": digest(path), "pinned_arms": pinned,
            "workspace_arms": workspace, "unbuilt_workspace_arms": unbuilt}


def aff3ct_identity(root, records):
    """Revision and library digest of the AFF3CT tree `root`, with the records pinning each."""
    revision = output(["git", "-C", str(root), "rev-parse", "HEAD"])
    libraries = sorted((root / "build/lib").glob("libaff3ct-*.a"))
    if len(libraries) != 1:
        sys.exit(f"{root} holds {len(libraries)} AFF3CT static libraries")
    library = digest(libraries[0])
    pinning = sorted(name for name, record in records.items()
                     if any(pin["sha256"] == library for pin in record["pins"]))
    if not pinning:
        sys.exit("no build identity record pins the digest of the AFF3CT static library")
    stating = sorted(
        str(record) for record, identity in identity_records()
        if isinstance(identity.get("aff3ct"), dict) and revision in identity["aff3ct"].values())
    return {"revision": revision, "revision_stated_by": stating,
            "static_library": libraries[0].name, "static_library_sha256": library,
            "static_library_pinned_by": pinning}


def main():
    if len(sys.argv) not in (2, 3):
        sys.exit(__doc__)
    if pathlib.Path.cwd() != ROOT:
        sys.exit("invoke from the worktree root")
    out_dir = pathlib.Path(sys.argv[1])
    aff3ct = pathlib.Path(sys.argv[2]).resolve() if len(sys.argv) == 3 else None
    arms = located("package-directory", PACKAGE)
    manifest = arms / "Cargo.toml"
    _, before = pinned_executables()

    environment = {"CARGO_CI_NO_SCCACHE": "1", "RUSTFLAGS": RUSTFLAGS}
    command = ["./scripts/cargo-budget.sh", "cargo", f"+{TOOLCHAIN}", "build", "--offline",
               "--locked", "--release", "--manifest-path", str(manifest)]
    subprocess.run(command, check=True, env={**os.environ, **environment})
    commands = [" ".join([f"{name}='{value}'" for name, value in environment.items()] + command)]

    metadata = json.loads(output(
        ["cargo", f"+{TOOLCHAIN}", "metadata", "--offline", "--locked", "--no-deps",
         "--format-version", "1", "--manifest-path", str(manifest)]))
    release = pathlib.Path(metadata["target_directory"]).relative_to(ROOT) / "release"
    built = [package for package in metadata["packages"]
             if package["name"] == PACKAGE or aff3ct is not None]
    binaries = sorted(target["name"] for package in built for target in package["targets"]
                      if "bin" in target["kind"])
    if aff3ct is not None:
        command += [argument for package in built if package["name"] != PACKAGE
                    for argument in ("--package", package["name"])]
        subprocess.run(command, check=True,
                       env={**os.environ, **environment, "GF2_AFF3CT_ROOT": str(aff3ct)})
        # The tree's location is the caller's; `aff3ct` gives its identity.
        commands.append(" ".join([f"{name}='{value}'" for name, value in environment.items()]
                                 + ["GF2_AFF3CT_ROOT=<aff3ct root>"] + command))
    rows, after = pinned_executables()
    if after != before:
        sys.exit("the build changed a pinned executable of an LDPC campaign")

    executables = {name: digest(release / name) for name in binaries}
    records = pinned_records()
    record = {
        "schema": "ldpc-batch-arms-build-v3",
        "rustc": output(["rustc", f"+{TOOLCHAIN}", "--version", "--verbose"]),
        "cargo": output(["cargo", f"+{TOOLCHAIN}", "--version"]),
        "build_commands": commands,
        "release_profile": tomllib.loads(manifest.read_text(encoding="utf-8"))["profile"]["release"],
        "executables": executables,
        "aff3ct": None if aff3ct is None else aff3ct_identity(aff3ct, records),
        "catalogue": catalogue(arms, rows, records, executables,
                               {package["name"] for package in built}),
        "isolation": isolation(metadata, arms, rows),
        "pinned_records": records,
        "pinned_executables": rows,
    }
    out_dir.mkdir(parents=True, exist_ok=True)
    path = out_dir / ("gf2-arms-build.json" if aff3ct is None else "aff3ct-arms-build.json")
    path.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    print(path)


if __name__ == "__main__":
    main()
