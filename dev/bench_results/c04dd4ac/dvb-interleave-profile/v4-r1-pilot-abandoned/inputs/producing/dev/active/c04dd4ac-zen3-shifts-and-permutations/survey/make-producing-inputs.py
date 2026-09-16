#!/usr/bin/env python3
"""Generate the content closure snapshotted by the DVB profile receipt."""

import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[4]
SURVEY = pathlib.Path("dev/active/c04dd4ac-zen3-shifts-and-permutations/survey")
OLD = pathlib.Path("dev/active/eda07788/survey")
SUPPORT = pathlib.Path("dev/tools/tuning-campaign-support/src")
OUTPUT = SURVEY / "dvb-producing-inputs.json"


def files_under(*roots):
    paths = []
    for root in roots:
        for path in (ROOT / root).rglob("*"):
            if path.is_file() and "target" not in path.parts:
                paths.append(str(path.relative_to(ROOT)))
    return paths


def main():
    behavior = [
        "crates/gf2-coding/src/dvb_t2_bicm_harness.rs",
        "crates/gf2-coding/src/ldpc/dvb_t2/bit_interleaver.rs",
        "crates/gf2-sim/src/batch.rs",
        "crates/gf2-sim/src/stage.rs",
        "crates/gf2-sim/src/stages/mod.rs",
        str(SURVEY / "build-dvb-harness.sh"),
        str(SURVEY / "make-dvb-addendum.py"),
        str(SURVEY / "make-dvb-plan.py"),
        str(SURVEY / "make-source-evidence.py"),
        str(SURVEY / "profile-cases.txt"),
        str(SURVEY / "run-dvb-campaign.sh"),
        str(SURVEY / "run-profile.sh"),
        str(SURVEY / "summarize-profile.py"),
        str(OLD / "gf2-side/src/bin/dvb-profile-arm.rs"),
        str(OLD / "gf2-side/src/bin/dvb-profile.rs"),
        str(OLD / "gf2-side/src/lib.rs"),
        str(OLD / "xdsopl-pin.txt"),
        str(OLD / "xdsopl-shim/xdsopl_shim.cpp"),
    ]
    lifecycle = [
        str(SURVEY / "run-dvb-campaign.sh"),
        "dev/scripts/ccx1-bench-flock.sh",
    ] + files_under(SUPPORT)
    behavior += lifecycle + files_under(
        pathlib.Path("crates/gf2-core/src"),
        pathlib.Path("crates/gf2-coding/src"),
        pathlib.Path("crates/gf2-sim/src"),
    )
    build = [
        ".cargo/config.toml",
        "Cargo.lock",
        "Cargo.toml",
        str(SURVEY / "build-dvb-harness.sh"),
        str(SURVEY / "dvb-source-evidence.json"),
        str(SURVEY / "make-producing-inputs.py"),
        str(SURVEY / "profile-cases.txt"),
        "dev/active/c04dd4ac-zen3-shifts-and-permutations/dvb-profile-addendum.json",
        str(OLD / "gf2-side/Cargo.lock"),
        str(OLD / "gf2-side/Cargo.toml"),
        str(OLD / "gf2-side/build.rs"),
        str(OLD / "fetch-build.sh"),
        "dev/active/f547c394/addendum.schema.json",
        "dev/active/f547c394/protocol.md",
        "dev/active/1a379447-zen3-cpu-performance/measurement-contract.md",
        "scripts/cargo-budget.sh",
    ] + files_under(
        pathlib.Path("crates/gf2-core/src"),
        pathlib.Path("crates/gf2-coding/src"),
        pathlib.Path("crates/gf2-sim/src"),
    ) + behavior + lifecycle
    missing = [path for path in set(build) if not (ROOT / path).is_file()]
    if missing:
        raise SystemExit(f"producing inputs are missing: {sorted(missing)}")
    document = {
        "schema": "tuning-campaign-producing-inputs-v1",
        "behavior_sources": sorted(set(behavior)),
        "lifecycle_sources": sorted(set(lifecycle)),
        "build_inputs": sorted(set(build)),
    }
    (ROOT / OUTPUT).write_text(json.dumps(document, indent=2) + "\n")
    print(
        f"{OUTPUT}: {len(document['behavior_sources'])} behavior, "
        f"{len(document['lifecycle_sources'])} lifecycle, "
        f"{len(document['build_inputs'])} build inputs"
    )


if __name__ == "__main__":
    main()
