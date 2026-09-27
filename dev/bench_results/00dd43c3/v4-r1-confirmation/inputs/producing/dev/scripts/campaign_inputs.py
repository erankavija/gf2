#!/usr/bin/env python3
"""Shared producing-input manifest of a family's campaigns.

A manifest selects every repository file whose bytes can change what a campaign
measures or how it runs (behavior), the subset that decides campaign lifecycle
and resume (lifecycle), and the behavior set plus manifests, lock files, build
configuration and committed pre-timing evidence (build inputs). A family supplies
those three declarations and the source directories the behavior set is derived
from, so an added source file enters the closure without an edit to the family's
generator. Import it as

    sys.path.insert(0, os.path.join(root, "dev/scripts"))
    import campaign_inputs

and call `campaign_inputs.write_manifest(...)`.
"""

import json
import os

SCHEMA = "tuning-campaign-producing-inputs-v1"


def rust_sources(root, directory):
    """Every `.rs` file under one directory, repository-relative."""
    found = []
    for base, _, files in os.walk(os.path.join(root, directory)):
        for name in files:
            if name.endswith(".rs"):
                found.append(os.path.relpath(os.path.join(base, name), root))
    return found


def write_manifest(root, output, source_dirs, behavior_extra, lifecycle, build_extra):
    """Writes the manifest and reports its three counts.

    `source_dirs` are the directories whose Rust sources join `behavior_extra` in
    the behavior set; `build_extra` joins that whole set in the build inputs.
    """
    behavior = set(behavior_extra)
    for directory in source_dirs:
        behavior.update(rust_sources(root, directory))
    build = behavior | set(build_extra)
    manifest = {
        "schema": SCHEMA,
        "behavior_sources": sorted(behavior),
        "lifecycle_sources": sorted(lifecycle),
        "build_inputs": sorted(build),
    }
    for path in manifest["build_inputs"]:
        if not os.path.isfile(os.path.join(root, path)):
            raise SystemExit(f"missing producing input {path}")
    with open(output, "w") as handle:
        json.dump(manifest, handle, indent=2)
        handle.write("\n")
    print(f"{output}: {len(manifest['behavior_sources'])} behavior, "
          f"{len(manifest['lifecycle_sources'])} lifecycle, "
          f"{len(manifest['build_inputs'])} build inputs")
