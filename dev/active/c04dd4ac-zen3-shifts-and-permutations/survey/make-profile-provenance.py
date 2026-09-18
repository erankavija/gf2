#!/usr/bin/env python3
"""Render the provenance closure of a committed DVB-T2 dynamic profile session.

The session records an executable digest and a case-declaration digest per
repetition but no source or build closure, so this generator rebuilds that
closure from committed objects and refuses to publish one it cannot verify by
content:

* the measured tree is located by the revision the session's `host.txt` carries
  as informational, and is then accepted only when that tree's committed build
  record states the same executable digest the session logs and its case
  declaration hashes to the digest the session records;
* the build closure is every path the producing-input manifest of that tree
  enumerates, with the SHA-256 of its content at that tree;
* the RNG declaration and the sampling plan are extracted from the pinned
  sources and the pinned launcher, never restated here.

Limits this record does not overcome: the tie between the executable digest and
the sources is the committed build record of the measured tree, not a rebuild.
The release profile of the survey workspace embeds its checkout location, and
the measured build ran in another worktree, so the digest is not reproducible
elsewhere and no rebuild is attempted. The `rustc` block of `host.txt` is the
ambient toolchain of the session shell; the build toolchain is the one the build
record states.

Usage: make-profile-provenance.py <profile-dir> [<output>]
"""

from __future__ import annotations

import hashlib
import json
import pathlib
import re
import subprocess
import sys

MANIFEST = "dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/dvb-producing-inputs.json"
BUILD_RECORD = "dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/harness-validation.txt"
BUILD_GATE = "dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/build-dvb-harness.sh"
LAUNCHER = "dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/run-profile.sh"
CASES = "dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/profile-cases.txt"
QUEUE = "dev/active/1a379447-zen3-cpu-performance/bench-window/queue.tsv"
RENDERER = "dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/render-hot-instructions.sh"
PROFILE_SOURCE = "dev/active/eda07788/survey/gf2-side/src/bin/dvb-profile.rs"
LIBRARY_SOURCE = "dev/active/eda07788/survey/gf2-side/src/lib.rs"
ENTROPY = re.compile(r"thread_rng|rand::|from_entropy|getrandom|SystemTime|RandomState")
SEEDED = re.compile(r"seeded_word_banks\(")
MIXER = re.compile(r"SplitMix64::new")
REVISION = re.compile(r"^# gf2 revision \(informational\): ([0-9a-f]{40})$")
EXECUTABLE = re.compile(r"^# dvb-profile sha256: ([0-9a-f]{64})$")
LOGGED = re.compile(r"^(rep-[0-9]+) start \S+ dvb-profile=([0-9a-f]{64})")
SERIES = re.compile(r"^series done \S+ sessions=([0-9]+)$")
DIGEST_LINE = re.compile(r"^([0-9a-f]{64})\s+(\S+)$")


def fail(message: str) -> None:
    raise SystemExit(f"make-profile-provenance.py: {message}")


class Tree:
    """Committed blobs of one revision, read through a single `git cat-file`."""

    def __init__(self, repo: pathlib.Path, revision: str) -> None:
        self.repo = repo
        self.revision = revision
        self.batch = subprocess.Popen(
            ["git", "-C", str(repo), "cat-file", "--batch"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
        )

    def blob(self, path: str) -> bytes:
        assert self.batch.stdin and self.batch.stdout
        self.batch.stdin.write(f"{self.revision}:{path}\n".encode())
        self.batch.stdin.flush()
        header = self.batch.stdout.readline().decode().split()
        if len(header) != 3 or header[1] != "blob":
            fail(f"{path} is no committed blob at {self.revision}: {' '.join(header)}")
        content = self.batch.stdout.read(int(header[2]))
        self.batch.stdout.read(1)
        return content

    def text(self, path: str) -> str:
        return self.blob(path).decode()

    def close(self) -> None:
        assert self.batch.stdin
        self.batch.stdin.close()
        self.batch.wait()


def sha256(content: bytes) -> str:
    return hashlib.sha256(content).hexdigest()


def host_facts(host: str) -> tuple[str, str, dict[str, str]]:
    """Revision locator, logged executable digest and the record's own digests."""
    revision = executable = None
    digests: dict[str, str] = {}
    for line in host.splitlines():
        found = REVISION.match(line)
        if found:
            revision = found.group(1)
        found = EXECUTABLE.match(line)
        if found:
            executable = found.group(1)
        found = DIGEST_LINE.match(line)
        if found:
            digests[pathlib.PurePosixPath(found.group(2)).name] = found.group(1)
    if not revision or not executable:
        fail("host.txt carries no revision locator and executable digest pair")
    return revision, executable, digests


def logged_digests(log: str) -> tuple[set[str], list[str], str]:
    repetitions = [(found.group(1), found.group(2)) for found in map(LOGGED.match, log.splitlines()) if found]
    if not repetitions:
        fail("the session log opens no repetition")
    series = [found.group(1) for found in map(SERIES.match, log.splitlines()) if found]
    return {digest for _, digest in repetitions}, [rep for rep, _ in repetitions], series[0] if series else "unclosed"


def quoted(source: str, pattern: re.Pattern[str], path: str) -> list[tuple[int, str]]:
    return [
        (number, line.strip())
        for number, line in enumerate(source.splitlines(), start=1)
        if pattern.search(line)
    ]


def closure(manifest: dict) -> list[tuple[str, str]]:
    """Every producing-input path with the roles the manifest gives it."""
    roles = {
        "behavior_sources": "behavior",
        "lifecycle_sources": "lifecycle",
        "build_inputs": "build",
    }
    collected: dict[str, set[str]] = {}
    for key, role in roles.items():
        for path in manifest.get(key, []):
            collected.setdefault(path, set()).add(role)
    return [(path, " ".join(sorted(collected[path]))) for path in sorted(collected)]


def main() -> None:
    if not 2 <= len(sys.argv) <= 3:
        fail("usage: make-profile-provenance.py <profile-dir> [<output>]")
    repo = pathlib.Path(__file__).resolve().parents[4]
    session = pathlib.Path(sys.argv[1])
    out = open(sys.argv[2], "w", encoding="utf-8") if len(sys.argv) == 3 else sys.stdout

    host = (session / "host.txt").read_text()
    log = (session / "repetitions.log").read_text()
    revision, recorded, host_digests = host_facts(host)
    logged, repetitions, series = logged_digests(log)
    if logged != {recorded}:
        fail(f"the session log carries {sorted(logged)} against the host record's {recorded}")

    tree = Tree(repo, revision)
    build_record = tree.text(BUILD_RECORD)
    built = [found.group(1) for found in map(EXECUTABLE.match, build_record.splitlines()) if found]
    if built != [recorded]:
        fail(
            f"the build record at {revision} states {built} and cannot pin the profiled {recorded}"
        )
    cases = tree.blob(CASES)
    if sha256(cases) != host_digests.get("profile-cases.txt"):
        fail("the case declaration at the measured tree is not the one the session recorded")

    launcher = tree.blob(LAUNCHER)
    manifest_bytes = tree.blob(MANIFEST)
    manifest = json.loads(manifest_bytes)
    paths = closure(manifest)
    pinned = [(path, role, sha256(tree.blob(path))) for path, role in paths]
    identity = sha256("".join(f"{digest}  {path}\n" for path, _, digest in pinned).encode())
    profile_source = tree.text(PROFILE_SOURCE)
    library_source = tree.text(LIBRARY_SOURCE)
    queue = [
        line
        for line in tree.text(QUEUE).splitlines()
        if not line.startswith("#") and session.name in line
    ]
    entropy = [
        (path, role, digest)
        for path, role, digest in pinned
        if path.endswith(".rs") and path.startswith("dev/active/eda07788/") and ENTROPY.search(tree.text(path))
    ]
    header = [line.strip() for line in build_record.splitlines() if line.startswith("# ")]
    tree.close()

    print("# Provenance of the DVB-T2 interleaver dynamic profile", file=out)
    print(file=out)
    print("> **Diátaxis Type:** Reference", file=out)
    print(file=out)
    print(
        f"Generated by `survey/make-profile-provenance.py` from `{session.as_posix()}` and the "
        f"committed objects of the measured tree. Methods, verification and the limits of this "
        f"record are in the generator's docstring. The session retains {len(repetitions)} "
        f"repetitions and its log closes at {series} sessions.",
        file=out,
    )
    print(file=out)

    print("## Invocation", file=out)
    print(file=out)
    print(
        "The scheduled window runs the queued command below; the launcher it names re-invokes "
        "itself once per repetition under the host mutex. Every quoted line is read from the "
        "pinned blob the row names.",
        file=out,
    )
    print(file=out)
    print("| Source | path | SHA-256 | line |", file=out)
    print("|---|---|---|---|", file=out)
    for line in queue:
        fields = line.split("\t")
        print(f"| queued command | `{QUEUE}` | | `{fields[-1]}` |", file=out)
    print(f"| launcher | `{LAUNCHER}` | `{sha256(launcher)}` | |", file=out)
    for number, line in quoted(launcher.decode(), re.compile(r'--session "\$\{OUT\}"|FLOCK\}" --full-host'), LAUNCHER):
        print(f"| per-repetition dispatch | `{LAUNCHER}:{number}` | | `{line}` |", file=out)
    print(f"| case declaration | `{CASES}` | `{sha256(cases)}` | |", file=out)
    print(file=out)
    print("## Session artifact", file=out)
    print(file=out)
    print(
        "Every figure of this session is read from the files below; their digests identify the "
        "bytes this record interprets.",
        file=out,
    )
    print(file=out)
    print("| file | SHA-256 |", file=out)
    print("|---|---|", file=out)
    for path in sorted(
        [session / "host.txt", session / "repetitions.log", session / "ladder.json"]
        + sorted(session.glob("rep-*/cases.json"))
    ):
        print(
            f"| `{path.relative_to(session).as_posix()}` | `{sha256(path.read_bytes())}` |",
            file=out,
        )
    perf = sorted(
        path
        for pattern in ("rep-*/counters/*", "rep-*/hot/*")
        for path in session.glob(pattern)
        if path.is_file()
    )
    aggregate = sha256(
        "".join(
            f"{sha256(path.read_bytes())}  {path.relative_to(session).as_posix()}\n"
            for path in perf
        ).encode()
    )
    print(
        f"| counter and sample files ({len(perf)}), aggregate | `{aggregate}` |",
        file=out,
    )
    print(f"| `{RENDERER}` at this checkout | `{sha256((repo / RENDERER).read_bytes())}` |", file=out)
    print(file=out)
    print(
        "The aggregate is the SHA-256 of the `<digest>  <path>` lines of every counter and "
        "sample file the session retains, in path order. The instruction listings beside those "
        "samples are the renderer's output over them and the executable the samples name, which "
        "is why the renderer's identity belongs to this record.",
        file=out,
    )
    print(file=out)

    print("## Executable and build closure", file=out)
    print(file=out)
    print(
        "The profiled executable is tied to the measured tree by that tree's committed build "
        "record, which the build gate writes from the build that produced the binary: the gate "
        f"`{BUILD_GATE}` builds the survey binaries and writes the record in one invocation, so "
        "a tree whose closure differs carries a different record. Each "
        "repetition of the session logs the same digest. The revision below locates the "
        "committed blobs and decides nothing: what this record publishes, and what a reader "
        "checks it against, are the content identities.",
        file=out,
    )
    print(file=out)
    print("| Fact | value |", file=out)
    print("|---|---|", file=out)
    print(f"| profiled executable SHA-256 | `{recorded}` |", file=out)
    print(f"| repetitions logging that digest | {len(repetitions)} |", file=out)
    print(f"| build record | `{BUILD_RECORD}` at `{revision}` |", file=out)
    for line in header:
        print(f"| build record header | `{line}` |", file=out)
    print(f"| producing-input manifest | `{MANIFEST}` |", file=out)
    print(f"| manifest SHA-256 | `{sha256(manifest_bytes)}` |", file=out)
    print(f"| closure paths | {len(pinned)} |", file=out)
    print(f"| closure identity | `{identity}` |", file=out)
    print(file=out)
    print(
        "The closure identity is the SHA-256 of the `<digest>  <path>` lines of the table below, "
        "in path order, so one value stands for the whole closure.",
        file=out,
    )
    print(file=out)
    print("| # | path | role | SHA-256 at the measured tree |", file=out)
    print("|---:|---|---|---|", file=out)
    for index, (path, role, digest) in enumerate(pinned, start=1):
        print(f"| {index} | `{path}` | {role} | `{digest}` |", file=out)
    print(file=out)

    print("## RNG declaration", file=out)
    print(file=out)
    print(
        "Every fixture bank of the profile comes from the seeded generator the pinned sources "
        "below fix; the scan reports the pinned survey sources that name an entropy source "
        "outside them.",
        file=out,
    )
    print(file=out)
    print("| Source | line | statement |", file=out)
    print("|---|---:|---|", file=out)
    for number, line in quoted(profile_source, SEEDED, PROFILE_SOURCE):
        print(f"| `{PROFILE_SOURCE}` | {number} | `{line}` |", file=out)
    for number, line in quoted(library_source, MIXER, LIBRARY_SOURCE):
        print(f"| `{LIBRARY_SOURCE}` | {number} | `{line}` |", file=out)
    print(file=out)
    print(f"Pinned survey sources naming another entropy source: {len(entropy)}.", file=out)
    print(file=out)

    print("## Sampling plan", file=out)
    print(file=out)
    print(
        "The plan is the one the pinned sources declare: the per-case window budget and window "
        "target of the profile binary, the per-case statistic it reports, and the repetition "
        "count and counter selection of the launcher.",
        file=out,
    )
    print(file=out)
    print("| Source | line | statement |", file=out)
    print("|---|---:|---|", file=out)
    plan = re.compile(r"const WINDOWS|const WINDOW_TARGET|ns_per_call: ordered")
    for number, line in quoted(profile_source, plan, PROFILE_SOURCE):
        print(f"| `{PROFILE_SOURCE}` | {number} | `{line}` |", file=out)
    for number, line in quoted(launcher.decode(), re.compile(r"REPETITIONS=9|counters=yes"), LAUNCHER):
        print(f"| `{LAUNCHER}` | {number} | `{line}` |", file=out)
    print(file=out)
    if out is not sys.stdout:
        out.close()


if __name__ == "__main__":
    main()
