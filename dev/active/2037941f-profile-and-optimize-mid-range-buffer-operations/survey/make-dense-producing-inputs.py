#!/usr/bin/env python3
"""Generate the content closure snapshotted by a dense-parity receipt.

`--check` writes nothing and compares instead, so a timed run refuses a closure
that no longer enumerates its own tree.
"""

import argparse
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[4]
STORY = pathlib.Path("dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations")
SURVEY = STORY / "survey"
HARNESS = SURVEY / "dense-harness"
SUPPORT = pathlib.Path("dev/tools/tuning-campaign-support/src")
OUTPUT = SURVEY / "dense-producing-inputs.json"


def files_under(*roots):
    # `target` is excluded by its position under ROOT, never by a component of
    # the absolute path: ROOT itself may sit inside a build directory.
    paths = []
    for root in roots:
        for path in (ROOT / root).rglob("*"):
            relative = path.relative_to(ROOT)
            if path.is_file() and "target" not in relative.parts:
                paths.append(str(relative))
    return paths


def closure():
    """The producing-input document this tree enumerates."""
    # The measured routes are the public gf2-core matvec and the isolated
    # kernel of gf2-kernels-simd, so the behavioral closure is those two
    # crates' sources plus the harness that calls them, the C shim that reaches
    # the external comparator's public coordinates, and the shared campaign
    # support that times them.
    lifecycle = [str(SURVEY / "run-dense-harness.sh"), "dev/scripts/ccx1-bench-flock.sh"]
    lifecycle += files_under(SUPPORT)
    behavior = sorted(
        set(
            files_under(
                HARNESS / "src",
                pathlib.Path("crates/gf2-core/src"),
                pathlib.Path("crates/gf2-kernels-simd/src"),
            )
            + [
                str(HARNESS / "build.rs"),
                str(STORY / "dense-parity-addendum.md"),
                str(STORY / "m4ri-operation-match.md"),
                str(STORY / "m4ri-probe-record.txt"),
                str(SURVEY / "m4ri_matvec_arm.c"),
                str(SURVEY / "m4ri_matvec_probe.c"),
                str(SURVEY / "m4ri-build-pins.sh"),
                str(SURVEY / "run-m4ri-matvec-probe.sh"),
                str(SURVEY / "check-m4ri-build-provenance.sh"),
            ]
            + lifecycle
        )
    )
    # Every Cargo manifest and lock file a timed executable is built from: the
    # harness workspace builds the arms from its own manifest and lock against
    # the two measured crates, and the window builds `benchmark-ab-runner` and
    # `benchmark-acceptance` from the root workspace with `--locked`, so the
    # root manifest, the root lock and the campaign-support manifest control
    # those bytes too. The closure manifest itself is a build input because the
    # window guard reads it to decide which paths to check.
    manifests = [
        "Cargo.lock",
        "Cargo.toml",
        str(HARNESS / "Cargo.lock"),
        str(HARNESS / "Cargo.toml"),
        "crates/gf2-core/Cargo.toml",
        "crates/gf2-kernels-simd/Cargo.toml",
        "dev/tools/tuning-campaign-support/Cargo.toml",
    ]
    build = sorted(
        set(
            behavior
            + manifests
            + [
                ".cargo/config.toml",
                str(OUTPUT),
                str(SURVEY / "make-dense-producing-inputs.py"),
                str(SURVEY / "dense-parity-source-evidence.json"),
                str(STORY / "dense-parity-harness.md"),
                "dev/active/f547c394/addendum.schema.json",
                "dev/active/f547c394/protocol.md",
                "dev/active/f547c394/amendment-v4.md",
                "dev/active/1a379447-zen3-cpu-performance/measurement-contract.md",
                "scripts/cargo-budget.sh",
            ]
        )
    )
    # The closure manifest is this script's own output, so it is the one
    # build input that need not already exist.
    missing = sorted(
        path for path in set(build) - {str(OUTPUT)} if not (ROOT / path).is_file()
    )
    if missing:
        raise SystemExit(f"producing inputs are missing: {missing}")
    return {
        "schema": "tuning-campaign-producing-inputs-v1",
        "behavior_sources": behavior,
        "lifecycle_sources": sorted(set(lifecycle)),
        "build_inputs": build,
    }


def differences(recorded, regenerated):
    """`+`/`- ` lines for every path one closure names and the other does not."""
    lines = []
    for section, paths in regenerated.items():
        if not isinstance(paths, list):
            continue
        listed = set(recorded.get(section, []))
        lines += [f"+ {section} {path}" for path in sorted(set(paths) - listed)]
        lines += [f"- {section} {path}" for path in sorted(listed - set(paths))]
    return lines


def check(document, text):
    try:
        committed = (ROOT / OUTPUT).read_text()
    except OSError as error:
        raise SystemExit(f"{OUTPUT}: {error}") from error
    if committed == text:
        print(f"{OUTPUT}: enumerates this tree")
        return
    try:
        recorded = json.loads(committed)
    except json.JSONDecodeError:
        recorded = {}
    detail = differences(recorded, document) or [
        "the sections name the same paths, so the two differ in schema or rendering"
    ]
    raise SystemExit(
        "\n".join(
            [
                f"{OUTPUT} is not the closure of this tree; regenerate it with "
                f"{SURVEY / 'make-dense-producing-inputs.py'}",
                *detail,
            ]
        )
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="compare the committed closure with this tree's instead of writing it",
    )
    arguments = parser.parse_args()
    document = closure()
    text = json.dumps(document, indent=2) + "\n"
    if arguments.check:
        check(document, text)
        return
    (ROOT / OUTPUT).write_text(text)
    print(
        f"{OUTPUT}: {len(document['behavior_sources'])} behavior, "
        f"{len(document['lifecycle_sources'])} lifecycle, "
        f"{len(document['build_inputs'])} build inputs"
    )


if __name__ == "__main__":
    main()
