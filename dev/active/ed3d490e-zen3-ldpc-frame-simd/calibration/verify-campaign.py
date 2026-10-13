#!/usr/bin/env python3
"""Verify the published decoder calibration campaign from its journal (jit:20af7e6e).

Usage: verify-campaign.py [--check]

Reads the campaign's execution journal and owner manifest from the host-local
evidence archive, each under the digest the committed checksum manifest pins,
and requires that every declared unit starts, validates, checkpoints and
completes once with a clean child exit, that every cell holds its declared
probe and timed children, that every timed child reports its declared windows,
and that the journal ends in a terminal `complete` record. It then requires
that the measurement block of the committed coding owner envelope names the
producer, behaviour token and executable of `producer/identity.json`, the
producer reports and the producing manifest the campaign record pins.

Writes `campaign-verification.json`; a failed requirement writes nothing.
"""

import hashlib
import json
import sys
from collections import Counter

sys.dont_write_bytecode = True  # a check writes nothing, however invoked

from locate import CALIBRATION, HERE, ISSUE, ROOT, emit, sha256, tracked

OUTPUT = HERE / "campaign-verification.json"
SECTION = "gf2-coding/encode"


def require(condition, message):
    if not condition:
        raise SystemExit(f"verify-campaign: {message}")


def one(paths, what):
    require(len(paths) == 1, f"{len(paths)} {what}; exactly one must exist")
    return paths[0]


def pinned(rows, suffix):
    """The root-relative path ending in `suffix` among `rows`, read under its digest."""
    path = one([path for path in rows if path.endswith(suffix)], f"pinned rows end in {suffix}")
    require((ROOT / path).is_file(), f"{path} of the host-local evidence archive is absent")
    data = (ROOT / path).read_bytes()
    require(hashlib.sha256(data).hexdigest() == rows[path], f"{path} differs from its pinned digest")
    return path, data


def checksum_rows(text):
    return {path: digest for digest, path in (line.split("  ", 1) for line in text.splitlines())}


def cell(identity):
    return identity["field"], identity["stratum"], identity["candidate"]


def journal_checks(records, manifest, listing, declared_cells):
    """The journal's accounting against the declared counts, as a record."""
    units = {unit["key"]: unit["identity"] for unit in manifest["ordered_units"]}
    timed = listing["accepted_results"] - listing["cells"]
    executions, windows = timed // listing["cells"], listing["windows"] // timed
    require(listing["cells"] == declared_cells, "the grid listing's cells differ from the declaration's")
    require(len(units) == listing["accepted_results"], "the manifest's units differ from the grid listing's")

    require([record["sequence"] for record in records] == list(range(len(records))),
            "the journal's sequence is not contiguous")
    events = Counter(record["event"] for record in records)
    keyed = {"cell-start": "key", "cell-complete": "key", "result-validated": "unit_key",
             "checkpoint-accepted": "unit_key", "child-exit": "unit_key"}
    for event, key in keyed.items():
        seen = Counter(record["details"][key] for record in records if record["event"] == event)
        require(seen == Counter(units.keys()), f"{event} does not occur once per declared unit")
    require(all(record["details"]["outcome"]["exit_code"] == 0 for record in records
                if record["event"] == "child-exit"), "a child exits with a failure")

    kinds = {}
    for identity in units.values():
        kinds.setdefault(cell(identity), Counter())[identity["task"]["kind"]] += 1
    require(len(kinds) == declared_cells, "the manifest's cells differ from the declaration's")
    require(all(kind == {"probe": 1, "measure": executions} for kind in kinds.values()),
            "a cell does not hold one probe and its declared timed children")

    reported = Counter()
    for record in records:
        if record["event"] == "window-progress":
            progress = record["details"]["record"]
            require(progress["progress"]["event"] == "window-complete", "a window does not complete")
            identity = progress["identity"]
            reported[cell(identity), identity["task"]["execution"]] += 1
    require(len(reported) == timed and set(reported.values()) == {windows},
            "a timed child does not report its declared windows")

    terminal = records[-1]
    require(terminal["event"] == "complete"
            and terminal["details"]["session_transition"]["transition"]
            == {"outcome": "complete", "transition": "terminal"},
            "the journal does not end in a terminal complete record")
    return {
        "declared": {"cells": declared_cells, "timed_children_per_cell": executions,
                     "windows_per_timed_child": windows, "units": len(units)},
        "events": dict(sorted(events.items())),
        "sessions": len({record["session_id"] for record in records}),
        "terminal": terminal["event"],
    }


def envelope_checks(owner, campaign, checksums):
    """The owner envelope's measurement identities against the committed producer records."""
    require(sha256(owner) == checksums[owner], f"{owner} differs from its pinned digest")
    measurement = json.loads((ROOT / owner).read_text())["sections"][SECTION]["measurement"]
    identity = json.loads((HERE / "producer" / "identity.json").read_text())
    listing = json.loads(json.loads((HERE / "producer" / "coding-list-grid.json").read_text())["evidence"])
    manifest_path = f"{CALIBRATION}/producing-build-inputs.json"
    manifest = json.loads((ROOT / manifest_path).read_text())
    (producer,) = [process for process in campaign["processes"] if process["id"] == "coding-producer"]

    require(measurement["harness"] == identity["producer"], "the harness is not the recorded producer")
    require(all(measurement["harness"] in manifest[part] for part in ("behavior_sources", "build_inputs")),
            "the producing manifest does not name the harness")
    require(measurement["harness_schema"] == listing["behavior_token"],
            "the harness schema is not the producer's behaviour token")
    require(measurement["binary_sha256"] == identity["binary_sha256"] == producer["executable_sha256"],
            "the producer executable is not the recorded one")
    require(measurement["toolchain"] == identity["toolchain"], "the toolchain is not the recorded one")
    require(campaign["producing_manifest"]["sha256"] == sha256(manifest_path),
            "the campaign record pins another producing manifest")
    return {"path": owner, "sha256": checksums[owner],
            **{key: measurement[key] for key in ("harness", "harness_schema", "binary_sha256", "receipt")}}


def main():
    checksum_path = one(tracked(f"**/gf2-{ISSUE}-*.sha256"), f"checksum manifests of {ISSUE}")
    campaign_id = checksum_path.rsplit("/", 1)[1].removesuffix(".sha256")
    checksums = checksum_rows((ROOT / checksum_path).read_text())
    journal_path, journal = pinned(checksums, "/execution.log")
    archive = checksum_rows(pinned(checksums, "/SHA256SUMS")[1].decode())
    manifest_path, manifest = pinned(archive, "/coding-manifest.json")
    campaign = json.loads(pinned(archive, f"{campaign_id}/campaign.json")[1])
    records = [json.loads(line) for line in journal.decode().splitlines()]
    require({record["campaign_id"] for record in records} == {campaign_id},
            "the journal holds another campaign's records")

    declaration = json.loads((HERE / "campaign-declaration.json").read_text())
    (measured,) = declaration["measured_owners"]
    listing = json.loads(json.loads((HERE / "producer" / "coding-list-grid.json").read_text())["evidence"])
    owner = one(tracked(f"**/gf2-coding/**/{campaign_id}.json"), "committed coding owner envelopes")
    record = {
        "issue": ISSUE,
        "campaign_id": campaign_id,
        "checksum_manifest": {"path": checksum_path, "sha256": sha256(checksum_path)},
        "execution_log": {"path": journal_path, "sha256": checksums[journal_path]},
        "owner_manifest": {"path": manifest_path, "sha256": archive[manifest_path]},
        "journal": journal_checks(records, json.loads(manifest), listing, measured["cells"]),
        "owner_envelope": envelope_checks(owner, campaign, checksums),
    }
    emit(OUTPUT, json.dumps(record, indent=1) + "\n", sys.argv[1:] == ["--check"],
         f"{CALIBRATION}/verify-campaign.py")


if __name__ == "__main__":
    main()
