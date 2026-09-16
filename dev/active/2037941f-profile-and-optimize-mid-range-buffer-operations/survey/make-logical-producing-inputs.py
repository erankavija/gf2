#!/usr/bin/env python3
"""Generate the content closure snapshotted by a logical-buffer receipt."""

import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[4]
STORY = pathlib.Path("dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations")
SURVEY = STORY / "survey"
HARNESS = SURVEY / "harness"
SUPPORT = pathlib.Path("dev/tools/tuning-campaign-support/src")
OUTPUT = SURVEY / "logical-producing-inputs.json"


def files_under(*roots):
    paths = []
    for root in roots:
        for path in (ROOT / root).rglob("*"):
            if path.is_file() and "target" not in path.parts:
                paths.append(str(path.relative_to(ROOT)))
    return paths


def main():
    # The measured routes are public gf2-core and gf2-coding entry points, so
    # the behavioral closure is those two crates' sources plus the harness that
    # calls them and the shared campaign support that times them.
    lifecycle = [str(SURVEY / "run-logical-harness.sh"), "dev/scripts/ccx1-bench-flock.sh"]
    lifecycle += files_under(SUPPORT)
    behavior = sorted(
        set(
            files_under(HARNESS / "src", pathlib.Path("crates/gf2-core/src"), pathlib.Path("crates/gf2-coding/src"))
            + [
                str(HARNESS / "build.rs"),
                str(STORY / "logical-buffer-addendum.md"),
                str(STORY / "isal-comparator.md"),
                str(SURVEY / "isal_xor_probe.c"),
                str(SURVEY / "run-isal-xor-probe.sh"),
            ]
            + lifecycle
        )
    )
    # Every Cargo manifest and lock file a timed executable is built from: the
    # harness workspace builds the arms from its own manifest and lock against
    # the three measured crates, and the window builds `benchmark-ab-runner`
    # and `benchmark-acceptance` from the root workspace with `--locked`, so the
    # root manifest, the root lock and the campaign-support manifest control
    # those bytes too. The closure manifest itself is a build input because the
    # window guard reads it to decide which paths to check.
    manifests = [
        "Cargo.lock",
        "Cargo.toml",
        str(HARNESS / "Cargo.lock"),
        str(HARNESS / "Cargo.toml"),
        "crates/gf2-core/Cargo.toml",
        "crates/gf2-coding/Cargo.toml",
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
                str(SURVEY / "make-logical-producing-inputs.py"),
                str(SURVEY / "logical-source-evidence.json"),
                str(STORY / "logical-harness.md"),
                "dev/active/f547c394/addendum.schema.json",
                "dev/active/f547c394/protocol.md",
                "dev/active/f547c394/amendment-v4.md",
                "dev/active/1a379447-zen3-cpu-performance/measurement-contract.md",
                "scripts/cargo-budget.sh",
            ]
            + files_under(pathlib.Path("crates/gf2-kernels-simd/src"))
        )
    )
    # The closure manifest is this script's own output, so it is the one
    # build input that need not exist before the write below.
    missing = sorted(
        path
        for path in set(build) - {str(OUTPUT)}
        if not (ROOT / path).is_file()
    )
    if missing:
        raise SystemExit(f"producing inputs are missing: {missing}")
    document = {
        "schema": "tuning-campaign-producing-inputs-v1",
        "behavior_sources": behavior,
        "lifecycle_sources": sorted(set(lifecycle)),
        "build_inputs": build,
    }
    (ROOT / OUTPUT).write_text(json.dumps(document, indent=2) + "\n")
    print(
        f"{OUTPUT}: {len(document['behavior_sources'])} behavior, "
        f"{len(document['lifecycle_sources'])} lifecycle, "
        f"{len(document['build_inputs'])} build inputs"
    )


if __name__ == "__main__":
    main()
