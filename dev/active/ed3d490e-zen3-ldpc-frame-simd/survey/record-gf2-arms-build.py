#!/usr/bin/env python3
"""Record the build identity of the batch decoder arms (jit:ed3d490e).

Builds the arms workspace beside this script and writes `build-identity.json`
from the commands it runs and the files it reads; no value is a clock reading.
Given the staged AFF3CT [Cassagne2019] tree `AFF3CT_ROOT`, it records the native
flavour: the catalogue's native and external arms, host-targeted. Given
`--portable`, it records the portable arms, built for the target's baseline
into a target directory of their own. `--supersedes` names, by root-relative path, the record
this one replaces; the record pins it by digest, and the arms tooling reads only the replacement.
Given `--arm NAME`, it records that one native-flavour arm of the catalogue, marked `own_record`,
built from the current tree by its package alone, so another pinned tree gives the name its own
`build_root`; such an arm may share an executable name with a pinned campaign.

Usage (from the worktree root):
  record-gf2-arms-build.py OUT_DIR AFF3CT_ROOT|--portable [--supersedes RECORD]
  record-gf2-arms-build.py OUT_DIR --arm NAME [AFF3CT_ROOT] [--supersedes RECORD]
  record-gf2-arms-build.py OUT_DIR --profile-only
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
PACKAGE = "ldpc-batch-arms"
CXX = "c++"
RECORD = "build-identity.json"
# Per flavour: its RUSTFLAGS, its target directory inside the arms workspace (None: the
# workspace's own) and the catalogue builds whose arms it records.
FLAVOURS = {
    "native": {"rustflags": "-C target-cpu=native", "target": None,
               "builds": ("native", "external")},
    "portable": {"rustflags": None, "target": "target/portable", "builds": ("portable",)},
}
AFF3CT_VARIABLE = "GF2_AFF3CT_ROOT"
# The arms workspace's linker driver and its suffix for the linker's dependency file.
LINKER = "link-inputs-cc.sh"
LINK_RULES = ".link.d"
# Variables rustc, cargo, the `cc` crate, the compiler driver and the build
# scripts read, in every host, target and target-suffixed form.
STEERING = re.compile(
    r"(HOST_|TARGET_)?(CC|CXX|AR|CFLAGS|CXXFLAGS|ARFLAGS|CXXSTDLIB)(_.*)?$"
    r"|RUSTC.*$|RUSTFLAGS$|CARGO_ENCODED_RUSTFLAGS$|CARGO_INCREMENTAL$"
    r"|CARGO_(BUILD|PROFILE|TARGET|HOST|UNSTABLE)_.*$"
    r"|CRATE_CC_NO_DEFAULTS$|CPATH$|C_INCLUDE_PATH$|CPLUS_INCLUDE_PATH$|LIBRARY_PATH$"
    r"|LD_RUN_PATH$|GCC_EXEC_PREFIX$|COMPILER_PATH$|SOURCE_DATE_EPOCH$"
    rf"|{AFF3CT_VARIABLE}$")


def output(command, **kwargs):
    return subprocess.run(command, check=True, capture_output=True, text=True,
                          **kwargs).stdout.strip()


ROOT = pathlib.Path(output(["git", "rev-parse", "--show-toplevel"]))


def live_files(name="*"):
    paths = output(["git", "-C", str(ROOT), "ls-files", "-z", "--cached", "--others",
                    "--exclude-standard", "--", f":(glob)**/{name}"]).split("\0")
    return sorted(pathlib.Path(path) for path in paths
                  if path and "inputs" not in pathlib.Path(path).parts
                  and (ROOT / path).is_file())


def located(*query):
    helper = ROOT / live_files("repository_files.py")[0]
    return pathlib.Path(output([sys.executable, "-B", str(helper), *query]))


def digest(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


EXECUTABLE_MAPS = ("executables", "arm_binaries", "quality_arm_binaries")


def is_digest(value):
    return isinstance(value, str) and len(value) == 64 and set(value) <= set("0123456789abcdef")


def pinned_digests(node, inside=False):
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
    for record in live_files("build-identity.json"):
        identity = json.loads((ROOT / record).read_text(encoding="utf-8"))
        if {"prepared_quality", "arm_binaries"} & identity.keys():
            yield record, identity


def pinned_executables():
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
    # An empty `carried_by` means an archive member, a file outside the repository or a file changed since.
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
    lines = [f"{digest(path)}  {path.relative_to(relative_to) if relative_to else path}\n"
             for path in sorted(paths)]
    return hashlib.sha256("".join(lines).encode()).hexdigest()


def prerequisites(rules):
    def quoted(match):
        backslashes, character = match.groups()
        literal = len(backslashes) % 2 and character == " "
        return backslashes[:len(backslashes) // 2] + ("\0" if literal else character)

    found = []
    for rule in rules.replace("\\\n", " ").splitlines():
        parts = re.split(r":(?=\s|$)", rule, maxsplit=1)
        if len(parts) == 2:
            found += [word.replace("\0", " ").replace("$$", "$")
                      for word in re.sub(r"(\\+)([ #])", quoted, parts[1]).split()]
    return found


def link_rules(executable):
    found = [path for path in sorted((executable.parent / "deps").glob(f"*{LINK_RULES}"))
             if digest(path.with_name(path.name.removesuffix(LINK_RULES))) == digest(executable)]
    if len(found) != 1:
        sys.exit(f"{len(found)} links wrote the content of {executable}")
    return found[0]


def refuse_newer(executable, files):
    linked_at = executable.stat().st_mtime_ns
    newer = [str(path) for path in files if path.stat().st_mtime_ns > linked_at]
    if newer:
        sys.exit(f"{executable} is older than {newer}, which its link used")


def inputs(executable, dep_info, link_rules, root, target):
    # The listings are `sha256sum` listings in path order: files inside `root`, then the others.
    def named(rules):
        return {pathlib.Path(os.path.normpath(entry))
                for entry in prerequisites(rules.read_text(encoding="utf-8"))}

    paths = named(dep_info)
    inside = {path for path in paths if root in path.parents}
    linked = sorted(path for path in named(link_rules) if target not in path.parents)
    refuse_newer(executable, linked)
    return {"repository_files_sha256": listing_digest(inside, root),
            "other_files_sha256": listing_digest(paths - inside),
            "linked": {str(path): digest(path) for path in linked}}


def strings(executable, section):
    dump = output(["readelf", f"--string-dump={section}", str(executable)])
    return re.findall(r"^ +\[ *[0-9a-f]+\]  (.*)$", dump, re.MULTILINE)


def cargo_configuration(directory, home):
    folders = [folder / ".cargo" for folder in (directory, *directory.parents)]
    return [path for folder in dict.fromkeys([*folders, home])
            for path in (folder / "config", folder / "config.toml") if path.is_file()]


def configuration_rows(files):
    rows = []
    for path in files:
        settings = tomllib.loads(path.read_text(encoding="utf-8"))
        environment = settings.pop("env", {})
        # `PATH` steers the drivers of the build past the version queries.
        unstated = sorted(settings) + sorted(f"env.{name}" for name in environment
                                             if name == "PATH" or STEERING.match(name))
        if unstated:
            sys.exit(f"{path} sets {unstated}, which the record does not state")
        rows.append({"path": str(path), "sha256": digest(path), "env": environment})
    return rows


def environment(flavour):
    rustflags = FLAVOURS[flavour]["rustflags"]
    return {"CARGO_CI_NO_SCCACHE": "1", **({"RUSTFLAGS": rustflags} if rustflags else {}),
            "CXX": CXX}


def build_environment(inherited, flavour):
    kept = {name: value for name, value in inherited.items() if not STEERING.match(name)}
    return {**kept, **environment(flavour)}


def aff3ct_identity(root, records):
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


def separate_members(metadata):
    """Each member outside the default build, and whether a dependency of it links AFF3CT."""
    default = set(metadata["workspace_default_members"])
    return [(["--package", package["name"]],
             any("aff3ct" in dependency["features"] for dependency in package["dependencies"]))
            for package in metadata["packages"] if package["id"] not in default]


def flavour_arms(catalogue, flavour):
    return {name: arm for name, arm in catalogue.items()
            if arm["build"] in FLAVOURS[flavour]["builds"] and not arm.get("own_record")}


def own_arm_commands(build, arm, aff3ct):
    """The one command that builds an arm recorded alone; a feature linking AFF3CT sees the tree."""
    features = arm.get("features", [])
    variables = environment("native")
    if "aff3ct" in features:
        variables = {**variables, AFF3CT_VARIABLE: str(aff3ct)}
    selected = ["--features", ",".join(features)] if features else []
    return [{"environment": variables, "argv": [*build, "--package", arm["package"], *selected]}]


def build_commands(flavour, build, metadata, named, aff3ct):
    """The native flavour builds the default members, then each other member alone, so its
    dependency features unify with no other package's and only a member that links AFF3CT sees
    the tree; the portable flavour builds the package of each of its arms alone."""
    variables = environment(flavour)
    if flavour != "native":
        return [{"environment": variables, "argv": build + ["--package", package]}
                for package in sorted({arm["package"] for arm in named.values()})]
    return [{"environment": variables, "argv": build},
            *({"environment": {**variables, **({AFF3CT_VARIABLE: str(aff3ct)} if links else {})},
               "argv": build + member} for member, links in separate_members(metadata))]


def built_binaries(metadata, commands):
    """Binary targets of the packages a command names, and of the default members when a command
    names none."""
    named = [{argv[index + 1] for index, word in enumerate(argv[:-1]) if word == "--package"}
             for argv in (command["argv"] for command in commands)]
    default = set(metadata["workspace_default_members"]) if not all(named) else set()
    return sorted(target["name"] for package in metadata["packages"]
                  if package["name"] in set().union(*named) or package["id"] in default
                  for target in package["targets"] if "bin" in target["kind"])


def write_record(out_dir, record):
    """Writes the record beside itself and renames it into place."""
    out_dir.mkdir(parents=True, exist_ok=True)
    partial = out_dir / f".{RECORD}.partial"
    partial.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    partial.replace(out_dir / RECORD)


def own_arm(argv):
    """The name after `--arm`, if any, and the other arguments."""
    if "--arm" not in argv[:-1]:
        return None, argv
    at = argv.index("--arm")
    return argv[at + 1], argv[:at] + argv[at + 2:]


def arguments(argv, alone=False):
    """OUT_DIR, flavour, AFF3CT tree and the identity of the superseded record, if any."""
    supersedes = None
    match argv:
        case [*rest, "--supersedes", superseded]:
            supersedes = {"path": superseded, "sha256": digest(ROOT / superseded)}
        case rest:
            pass
    match rest:
        case [out_dir, "--portable"]:
            return pathlib.Path(out_dir), "portable", None, supersedes
        case [out_dir, aff3ct] if not aff3ct.startswith("--"):
            return pathlib.Path(out_dir), "native", pathlib.Path(aff3ct).resolve(), supersedes
        case [out_dir] if alone:
            return pathlib.Path(out_dir), "native", None, supersedes
        case _:
            sys.exit(__doc__)


def profile_build(out_dir):
    """Build only the production profiler; campaign arms retain their bytes."""
    arms = located("package-directory", PACKAGE)
    manifest = arms / "Cargo.toml"
    target = out_dir / "target"
    inherited = build_environment(os.environ, "native")
    rustc = output(["rustc", f"+{TOOLCHAIN}", "--version", "--verbose"], env=inherited)
    host = re.search(r"^host: (.*)$", rustc, re.MULTILINE).group(1)
    linker = arms / LINKER
    command = ["./scripts/cargo-budget.sh", "cargo", f"+{TOOLCHAIN}", "build", "--offline",
               "--release", "--locked", "--manifest-path", str(manifest), "--package", "ldpc-batch-profile",
               "--features", "committed-envelope", "--target-dir", str(target),
               "--config", f"target.{host}.linker={json.dumps(str(linker))}"]
    subprocess.run(command, check=True, env=inherited)
    release = target / "release"
    executable = release / "ldpc-batch-profile"
    record = {
        "schema": "ldpc-batch-arms-build-v3",
        "build_root": str(ROOT),
        "rustc": rustc,
        "cargo": output(["cargo", f"+{TOOLCHAIN}", "--version"], env=inherited),
        "build_commands": [{"environment": environment("native"), "argv": command}],
        "release_profile": tomllib.loads(manifest.read_text())["profile"]["release"],
        "production_inputs": {
            str(path.relative_to(ROOT)): digest(path)
            for path in sorted({pathlib.Path(os.path.normpath(entry)) for entry in prerequisites(
                (release / "ldpc-batch-profile.d").read_text())})
            if path.is_relative_to(ROOT / "crates")
        },
        "executables": {"ldpc-batch-profile": {
            "sha256": digest(executable),
            "written_by": strings(executable, ".comment"),
            "compiler_options": strings(executable, ".GCC.command.line"),
            "inputs": inputs(executable, release / "ldpc-batch-profile.d",
                             link_rules(executable), ROOT, ROOT / target)}},
        "isolation": {"target_directory": str(target)},
    }
    write_record(out_dir, record)
    print(out_dir / RECORD)


def main():
    if pathlib.Path.cwd() != ROOT:
        sys.exit("invoke from the worktree root")
    if len(sys.argv) == 3 and sys.argv[2] == "--profile-only":
        return profile_build(pathlib.Path(sys.argv[1]))
    alone, rest = own_arm(sys.argv[1:])
    out_dir, flavour, aff3ct, supersedes = arguments(rest, alone is not None)
    arms = located("package-directory", PACKAGE)
    catalogue = arms.parent / "arms.json"
    catalogued = json.loads(catalogue.read_text(encoding="utf-8"))["arms"]
    named = flavour_arms(catalogued, flavour) if alone is None else {alone: catalogued[alone]}
    manifest = (arms if alone is None
                else located("package-directory", named[alone]["package"])) / "Cargo.toml"
    target = FLAVOURS[flavour]["target"]
    target_dir = [] if target is None else [
        "--config", f"build.target-dir={json.dumps(str(arms / target))}"]
    _, before = pinned_executables()

    inherited = build_environment(os.environ, flavour)
    configuration = configuration_rows(cargo_configuration(
        ROOT, pathlib.Path(inherited.get("CARGO_HOME") or pathlib.Path.home() / ".cargo")))
    rustc = output(["rustc", f"+{TOOLCHAIN}", "--version", "--verbose"], env=inherited)
    host = re.search(r"^host: (.*)$", rustc, re.MULTILINE).group(1)
    linker = arms / LINKER
    metadata = json.loads(output(
        ["cargo", f"+{TOOLCHAIN}", "metadata", "--offline", "--locked", "--no-deps",
         "--format-version", "1", "--manifest-path", str(manifest), *target_dir], env=inherited))
    release = pathlib.Path(metadata["target_directory"]).relative_to(ROOT) / "release"

    build = ["./scripts/cargo-budget.sh", "cargo", f"+{TOOLCHAIN}", "build", "--offline",
             "--locked", "--release", "--manifest-path", str(manifest),
             "--config", f"target.{host}.linker={json.dumps(str(linker))}", *target_dir]
    commands = (build_commands(flavour, build, metadata, named, aff3ct) if alone is None
                else own_arm_commands(build, named[alone], aff3ct))
    binaries = built_binaries(metadata, commands)
    for command in commands:
        subprocess.run(command["argv"], check=True, env={**inherited, **command["environment"]})
    rows, after = pinned_executables()
    if after != before:
        sys.exit("the build changed a pinned executable of an LDPC campaign")

    for name in binaries:
        refuse_newer(release / name, [linker])
    executables = {name: digest(release / name) for name in binaries}
    records = pinned_records()
    record = {
        "schema": "ldpc-batch-arms-build-v3",
        "build_root": str(ROOT),
        "rustc": rustc,
        "cargo": output(["cargo", f"+{TOOLCHAIN}", "--version"], env=inherited),
        "cxx": output([CXX, "--version"], env=inherited).splitlines()[0],
        "linker": {"driver": str(linker), "sha256": digest(linker),
                   "cc": output([str(linker), "--version"], env=inherited).splitlines()[0]},
        "cargo_configuration": configuration,
        "build_commands": commands,
        "release_profile": tomllib.loads(manifest.read_text(encoding="utf-8"))["profile"]["release"],
        "executables": {
            name: {"sha256": executables[name],
                   "written_by": strings(release / name, ".comment"),
                   "compiler_options": strings(release / name, ".GCC.command.line"),
                   "inputs": inputs(release / name, release / f"{name}.d",
                                    link_rules(release / name), ROOT, ROOT / release.parent)}
            for name in binaries},
        **({} if aff3ct is None else {"aff3ct": aff3ct_identity(aff3ct, records)}),
        "catalogue": {"path": str(catalogue), "sha256": digest(catalogue)},
        **({} if supersedes is None else {"supersedes": supersedes}),
        "arms": arm_rows(named, rows, records, executables, release),
        "isolation": (isolation(metadata, arms, rows) if alone is None
                      else {"target_directory": str(release.parent)}),
        "pinned_records": records,
        "pinned_executables": rows,
    }
    write_record(out_dir, record)
    print(out_dir / RECORD)


if __name__ == "__main__":
    main()
