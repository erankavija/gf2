#!/usr/bin/env python3
"""Record the build identity of the batch decoder arms (jit:ed3d490e).

Builds the arms workspace beside this script in its own target directory, the
member packages against `AFF3CT_ROOT`, the staged AFF3CT [Cassagne2019] tree,
and writes `build-identity.json`: the toolchain, the commands this run
executed with their environment, the digest and the inputs of every executable
the build produces, the identity of the AFF3CT tree, one row per arm of the
catalogue `arms.json` beside the workspace, and what this checkout shows about
each executable digest an LDPC campaign's build identity record pins.

An executable carries the absolute location of the checkout that builds it:
in its source paths, and in the symbol hashes cargo derives from the location
of each path dependency outside the workspace root. `build_root` is that
location as this run observes it, so the digests are those of this checkout's
build.

An arm row gives the executable's digest. A workspace arm is an executable of
this build, at `path`; a pinned arm has the digest the catalogue names, under
the campaign record that pins it, in the target files `found_at`. `inputs` is
what cargo's dep-info names for an executable, the files the C++ compiler read
included: the digests of the `sha256sum` listings, in path order, of the
repository files and of the files outside the repository.

The build runs without the inherited variables that steer rustc, cargo, the
`cc` crate or the compiler driver; `build_commands` gives those this run sets.

Content alone decides, and the run fails on any of:

- the files of the repository target directory bearing a pinned executable's
  name, or their digests, differ after the build from before it;
- the arms workspace is not its own workspace root with its target directory
  inside it;
- the arms target directory holds a file with a pinned executable's name or
  digest, or the arms workspace declares a package or target under such a name;
- the catalogue names a workspace arm the build does not produce, or a pinned
  arm whose record, found by content, does not pin that executable digest or
  whose executable no target file of this checkout carries;
- no build identity record pins the digest of the AFF3CT static library.

The rest is observation. `found_at` lists the target files carrying a pinned
digest, and is empty for an executable this checkout does not hold.
`pinned_records` gives each build identity record's digest and, per file
digest the record pins, the live repository files with that content.
`carried_by` empty says no live file has that content: the pin names an
archive member, a file outside the repository, or a file changed since. A
changed producing input leaves the pinned executable the one its campaign
measured. Git supplies the list of working-tree files; commit ids and history
are informational (`@/inv/behavioral-evidence-validity`).

Every value comes from a command this run executes or a file it reads, and
none is a clock reading, so a second run over an unchanged build reproduces
the record.

Usage (from the worktree root): record-gf2-arms-build.py OUT_DIR AFF3CT_ROOT
"""

import hashlib
import json
import os
import pathlib
import re
import subprocess
import sys
import tomllib

TOOLCHAIN = "1.95"
RUSTFLAGS = "-C target-cpu=native"
PACKAGE = "ldpc-batch-arms"
CXX = "c++"
ENVIRONMENT = {"CARGO_CI_NO_SCCACHE": "1", "RUSTFLAGS": RUSTFLAGS, "CXX": CXX}
# Variables rustc, cargo, the `cc` crate and the compiler driver read, in every
# host, target and target-suffixed form.
STEERING = re.compile(
    r"(HOST_|TARGET_)?(CC|CXX|AR|CFLAGS|CXXFLAGS|ARFLAGS|CXXSTDLIB)(_.*)?$"
    r"|RUSTC.*$|RUSTFLAGS$|CARGO_ENCODED_RUSTFLAGS$|CARGO_(BUILD|PROFILE|TARGET)_.*$"
    r"|CRATE_CC_NO_DEFAULTS$|CPATH$|C_INCLUDE_PATH$|CPLUS_INCLUDE_PATH$|LIBRARY_PATH$"
    r"|SOURCE_DATE_EPOCH$")


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


def arm_rows(named, rows, records, executables, release):
    """One row per arm of the catalogue `named`; exits on an arm without an executable digest."""
    by_digest = {record["sha256"]: name for name, record in records.items()}
    arms = {}
    for name, arm in sorted(named.items()):
        row = {"source": arm["source"], "executable": arm["executable"],
               "environment": arm["environment"]}
        if arm["source"] == "workspace":
            if arm["executable"] not in executables:
                sys.exit(f"the build produces no executable of the catalogued arm {name}")
            arms[name] = {**row, "package": arm["package"],
                          "path": str(release / arm["executable"]),
                          "executable_sha256": executables[arm["executable"]]}
            continue
        record = by_digest.get(arm["pinned_by"]["sha256"])
        found = [row for row in rows if row["record"] == record
                 and row["executable"] == arm["executable"]
                 and row["sha256"] == arm["executable_sha256"]]
        if not found:
            sys.exit(f"no live record pins the executable digest of the catalogued arm {name}")
        if not found[0]["found_at"]:
            sys.exit(f"no target file carries the pinned executable of the catalogued arm {name}")
        arms[name] = {**row, "found_at": found[0]["found_at"],
                      "executable_sha256": arm["executable_sha256"],
                      "pinned_by": {"record": record, "sha256": arm["pinned_by"]["sha256"]}}
    return arms


def listing_digest(paths, relative_to=None):
    """Digest of the `sha256sum` listing of `paths` in path order."""
    lines = [f"{digest(path)}  {path.relative_to(relative_to) if relative_to else path}\n"
             for path in sorted(paths)]
    return hashlib.sha256("".join(lines).encode()).hexdigest()


def inputs(dep_info, root):
    """Identity of the files the cargo dep-info file `dep_info` names, split at `root`."""
    listed = dep_info.read_text(encoding="utf-8").partition(": ")[2].strip()
    paths = [pathlib.Path(os.path.normpath(entry.replace("\\ ", " ")))
             for entry in re.split(r"(?<!\\) ", listed)]
    inside = [path for path in paths if root in path.parents]
    return {"repository_files_sha256": listing_digest(inside, root),
            "other_files_sha256": listing_digest(set(paths) - set(inside))}


def build_environment(inherited):
    """`inherited` without the variables that steer rustc, cargo or `cc`, plus this run's own."""
    kept = {name: value for name, value in inherited.items() if not STEERING.match(name)}
    return {**kept, **ENVIRONMENT}


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
    return {"root": str(root), "revision": revision, "revision_stated_by": stating,
            "static_library": libraries[0].name, "static_library_sha256": library,
            "static_library_pinned_by": pinning}


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    if pathlib.Path.cwd() != ROOT:
        sys.exit("invoke from the worktree root")
    out_dir = pathlib.Path(sys.argv[1])
    aff3ct = pathlib.Path(sys.argv[2]).resolve()
    arms = located("package-directory", PACKAGE)
    manifest = arms / "Cargo.toml"
    _, before = pinned_executables()

    inherited = build_environment(os.environ)
    metadata = json.loads(output(
        ["cargo", f"+{TOOLCHAIN}", "metadata", "--offline", "--locked", "--no-deps",
         "--format-version", "1", "--manifest-path", str(manifest)], env=inherited))
    release = pathlib.Path(metadata["target_directory"]).relative_to(ROOT) / "release"
    binaries = sorted(target["name"] for package in metadata["packages"]
                      for target in package["targets"] if "bin" in target["kind"])
    members = [argument for package in metadata["packages"] if package["name"] != PACKAGE
               for argument in ("--package", package["name"])]

    build = ["./scripts/cargo-budget.sh", "cargo", f"+{TOOLCHAIN}", "build", "--offline",
             "--locked", "--release", "--manifest-path", str(manifest)]
    # The default members build without the AFF3CT tree; the member packages link it.
    commands = [{"environment": ENVIRONMENT, "argv": build},
                {"environment": {**ENVIRONMENT, "GF2_AFF3CT_ROOT": str(aff3ct)},
                 "argv": build + members}]
    for command in commands:
        subprocess.run(command["argv"], check=True, env={**inherited, **command["environment"]})
    rows, after = pinned_executables()
    if after != before:
        sys.exit("the build changed a pinned executable of an LDPC campaign")

    executables = {name: digest(release / name) for name in binaries}
    records = pinned_records()
    catalogue = arms.parent / "arms.json"
    record = {
        "schema": "ldpc-batch-arms-build-v3",
        "build_root": str(ROOT),
        "rustc": output(["rustc", f"+{TOOLCHAIN}", "--version", "--verbose"], env=inherited),
        "cargo": output(["cargo", f"+{TOOLCHAIN}", "--version"], env=inherited),
        "cxx": output([CXX, "--version"], env=inherited).splitlines()[0],
        "build_commands": commands,
        "release_profile": tomllib.loads(manifest.read_text(encoding="utf-8"))["profile"]["release"],
        "executables": {
            name: {"sha256": executables[name],
                   "inputs": inputs(release / f"{name}.d", ROOT)}
            for name in binaries},
        "aff3ct": aff3ct_identity(aff3ct, records),
        "catalogue": {"path": str(catalogue), "sha256": digest(catalogue)},
        "arms": arm_rows(json.loads(catalogue.read_text(encoding="utf-8"))["arms"], rows,
                         records, executables, release),
        "isolation": isolation(metadata, arms, rows),
        "pinned_records": records,
        "pinned_executables": rows,
    }
    out_dir.mkdir(parents=True, exist_ok=True)
    path = out_dir / "build-identity.json"
    path.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    print(path)


if __name__ == "__main__":
    main()
