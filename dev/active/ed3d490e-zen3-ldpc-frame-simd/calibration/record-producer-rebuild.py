#!/usr/bin/env python3
"""Rebuild the campaign's producer under declared flags and record the build (jit:20af7e6e).

Usage: record-producer-rebuild.py [--check]

Builds the coding producer untimed at the source revision the published
session records, in a scratch tree mounted at the recorded build path because
the executable embeds that path, with the environment cleared to `ENVIRONMENT`
and the launcher's build command made verbose. Writes
`producer-rebuild/build.log` and `producer-rebuild/record.json`. `--check`
builds nothing: it derives the record's projections of the committed log again
and compares the recorded digests with the published owner.
"""

import datetime
import hashlib
import json
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
from collections import Counter
from pathlib import Path

sys.dont_write_bytecode = True  # a check writes nothing, however invoked

from locate import CALIBRATION, HERE, ISSUE, ROOT, sha256, tracked

OUTPUT = HERE / "producer-rebuild"
LOG = OUTPUT / "build.log"
RECORD = OUTPUT / "record.json"
SECTION = "gf2-coding/encode"
TOOLCHAIN = "1.95.0"
# The build command of the campaign launcher, with `-v` added.
COMMAND = ["./scripts/cargo-budget.sh", "cargo", f"+{TOOLCHAIN}", "build", "-v", "--release",
           "-p", "gf2-coding", "--bench", "tuning_calibration",
           "--features", "parallel,simd,tuning-profile,test-support", "--message-format=json"]
# The whole build environment: what the launcher exports, and no flag variable.
ENVIRONMENT = {
    "HOME": str(Path.home()),
    "PATH": f"{Path.home()}/.cargo/bin:/usr/local/bin:/usr/bin",
    "RUSTUP_TOOLCHAIN": TOOLCHAIN,
    "RAYON_NUM_THREADS": "4",
    "CARGO_CI_NO_SCCACHE": "1",
}
RUNNING = re.compile(r"^\s*Running `(.*)`$")
# Codegen options that name the unit rather than how it is compiled.
UNIT_OPTIONS = ("metadata=", "extra-filename=", "linker=", "incremental=")


def one(paths, what):
    if len(paths) != 1:
        raise SystemExit(f"record-producer-rebuild: {len(paths)} {what}; exactly one must exist")
    return paths[0]


def published():
    """The campaign's session directory, owner path and owner measurement block."""
    staging = one(tracked(f"**/gf2-{ISSUE}-*-session/staging-manifest.json"), "session records")
    session = Path(staging).parent
    owner = one(tracked(f"**/gf2-coding/**/{session.name.removesuffix('-session')}.json"),
                "committed coding owner envelopes")
    measurement = json.loads((ROOT / owner).read_text())["sections"][SECTION]["measurement"]
    return session, owner, measurement


def projections(log):
    """What the verbose build log states about the compiler invocations."""
    invocations = [shlex.split(found.group(1)) for found in map(RUNNING.match, log.splitlines())
                   if found]
    rustc = [words for words in invocations if Path(words[0]).name == "rustc"]
    codegen = Counter()
    for words in rustc:
        options = sorted(words[index + 1] for index, word in enumerate(words[:-1])
                         if word == "-C" and not words[index + 1].startswith(UNIT_OPTIONS))
        codegen[" ".join(f"-C {option}" for option in options)] += 1
    producer = one([words for words in rustc
                    if words[words.index("--crate-name") + 1] == "tuning_calibration"],
                   "producer invocations in the build log")
    return {
        "rustc_invocations": len(rustc),
        "build_script_runs": len(invocations) - len(rustc),
        "codegen_options_by_invocation_count": dict(sorted(codegen.items())),
        "producer_invocation": shlex.join(producer),
    }


def rebuild(session):
    """Builds in a scratch tree at the recorded path; returns the log and the observations."""
    staging = json.loads((ROOT / session / "staging-manifest.json").read_text())
    recorded = Path(staging["executables"]["coding-producer"]["path"])
    build_root = Path(str(recorded).split("/target/")[0])
    revision = json.loads((ROOT / session / "build/source-after.json").read_text())["source_revision"]
    scratch = Path(tempfile.mkdtemp(prefix="gf2-producer-rebuild-"))
    try:
        archive = subprocess.run(["git", "-C", str(ROOT), "archive", revision],
                                 capture_output=True, check=True).stdout
        subprocess.run(["tar", "-x", "-C", str(scratch)], input=archive, check=True)
        mounted = ["bwrap", "--dev-bind", "/", "/", "--bind", str(scratch), str(build_root),
                   "--chdir", str(build_root)]
        build = subprocess.run([*mounted, *COMMAND], env=ENVIRONMENT, capture_output=True, text=True)
        if build.returncode != 0:
            raise SystemExit(f"record-producer-rebuild: the build failed\n{build.stderr[-2000:]}")
        executable = scratch / recorded.relative_to(build_root)
        unit = recorded.name.removeprefix("tuning_calibration")
        fingerprint = (scratch / "target/release/.fingerprint" / f"gf2-coding{unit}"
                       / "test-bench-tuning_calibration.json")
        version = subprocess.run([*mounted, "rustc", f"+{TOOLCHAIN}", "--version"],
                                 env=ENVIRONMENT, capture_output=True, check=True, text=True)
        observed = {
            "observed_utc": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "source_revision": revision,
            "build_root": str(build_root),
            "command": COMMAND,
            "environment": ENVIRONMENT,
            "toolchain": version.stdout.strip(),
            "executable": {"path": str(recorded),
                           "sha256": hashlib.sha256(executable.read_bytes()).hexdigest()},
            "cargo_fingerprint": {
                "rustflags": json.loads(fingerprint.read_text())["rustflags"],
                "sha256": hashlib.sha256(fingerprint.read_bytes()).hexdigest(),
            },
        }
        # Cargo's fingerprint of the same unit in the producing checkout, when
        # this host still holds it.
        kept = build_root / fingerprint.relative_to(scratch)
        if kept.is_file():
            observed["cargo_fingerprint"]["producing_checkout_sha256"] = hashlib.sha256(
                kept.read_bytes()).hexdigest()
        return build.stderr, observed
    finally:
        shutil.rmtree(scratch, ignore_errors=True)


def recorded_identity(owner, measurement):
    identity = json.loads((HERE / "producer" / "identity.json").read_text())
    return {
        "owner": owner,
        "owner_binary_sha256": measurement["binary_sha256"],
        "owner_toolchain": measurement["toolchain"],
        "identity_binary_sha256": identity["binary_sha256"],
    }


def check(owner, measurement):
    record = json.loads(RECORD.read_text())
    log = LOG.read_text()
    problems = []
    if sha256(str(LOG.relative_to(ROOT))) != record["build_log"]["sha256"]:
        problems.append("the build log differs from its recorded digest")
    if projections(log) != record["build"]:
        problems.append("the record's build block differs from the build log")
    if record["recorded"] != recorded_identity(owner, measurement):
        problems.append("the record's recorded identities differ from the committed ones")
    digests = {record["executable"]["sha256"], record["recorded"]["owner_binary_sha256"],
               record["recorded"]["identity_binary_sha256"]}
    if record["reproduces"] != (len(digests) == 1):
        problems.append("the record's verdict differs from its digests")
    if problems:
        raise SystemExit("\n".join(f"record-producer-rebuild: {problem}" for problem in problems))
    print(f"{RECORD.relative_to(ROOT)}: matches its build log and the published owner")


def main():
    session, owner, measurement = published()
    if sys.argv[1:] == ["--check"]:
        return check(owner, measurement)
    log, observed = rebuild(session)
    OUTPUT.mkdir(exist_ok=True)
    LOG.write_text(log)
    recorded = recorded_identity(owner, measurement)
    record = {
        "issue": ISSUE,
        "campaign_id": session.name.removesuffix("-session"),
        **observed,
        "build_log": {"path": str(LOG.relative_to(ROOT)), "sha256": sha256(str(LOG.relative_to(ROOT)))},
        "build": projections(log),
        "recorded": recorded,
        "reproduces": observed["executable"]["sha256"] == recorded["owner_binary_sha256"]
        == recorded["identity_binary_sha256"],
    }
    RECORD.write_text(json.dumps(record, indent=1) + "\n")
    print(f"wrote {RECORD.relative_to(ROOT)}: reproduces {record['reproduces']}")


if __name__ == "__main__":
    main()
