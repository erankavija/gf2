#!/usr/bin/env python3
"""Build one named arm and record its executable, flags and toolchain under target/."""

import argparse
import os
from pathlib import Path
import subprocess
import sys
import tomllib

from campaign import HERE, ROOT, OUTPUT, digest, output_path, read, write


def record(name, out=None):
    arm = read(HERE / "arms.json")["arms"][name]
    if arm.get("configuration_only"):
        raise ValueError(f"{name} is configuration only")
    external = name.startswith("aff3ct")
    manifest = HERE / "aff3ct/Cargo.toml" if external else ROOT / "Cargo.toml"
    target = HERE / "aff3ct/target" if external else ROOT / "target"
    if external:
        root = os.environ.get("GF2_AFF3CT_ROOT")
        if not root or not (Path(root) / "build/lib/libaff3ct-4.7.0.a").is_file():
            raise ValueError("GF2_AFF3CT_ROOT must contain build/lib/libaff3ct-4.7.0.a")
    environment = dict(os.environ)
    environment.update(RUSTFLAGS="-C target-cpu=native", CARGO_INCREMENTAL="0")
    command = ["cargo", "+1.95", "build", "--locked",
               "--release", "--manifest-path", str(manifest), "--package", arm["package"],
               "--bin", arm["executable"]]
    subprocess.run(command, cwd=ROOT, env=environment, check=True)
    executable = target / "release" / arm["executable"]
    version = lambda tool: subprocess.check_output([tool, "+1.95", "--version"], text=True).strip()
    revision = subprocess.check_output(["git", "-C", str(ROOT), "rev-parse", "HEAD"], text=True).strip()
    # A pointer for rebuilding; nothing compares it.
    record = {"build_root": str(ROOT), "source_revision": revision, "rustc": version("rustc"), "cargo": version("cargo"),
              "rustflags": environment["RUSTFLAGS"],
              "release_profile": tomllib.loads(manifest.read_text())["profile"]["release"],
              "build_commands": [{"argv": command, "environment": {
                  key: environment[key] for key in ("RUSTFLAGS", "CARGO_INCREMENTAL")}}],
              "arms": {name: {"executable": str(executable.relative_to(ROOT)),
                              "executable_sha256": digest(executable)}}}
    if profile := arm.get("tuning_profile"):
        record["tuning_profile"] = {"id": read(ROOT / profile)["profile_id"], "sha256": digest(ROOT / profile)}
    destination = output_path(out or OUTPUT / "builds" / name / "build-identity.json")
    write(destination, record)
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--arm", required=True)
    parser.add_argument("--output")
    args = parser.parse_args()
    try:
        print(record(args.arm, args.output))
    except (ValueError, KeyError, OSError) as error:
        sys.exit(str(error))


if __name__ == "__main__":
    main()
