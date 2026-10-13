"""Re-validates committed tuning-campaign records with the current validator.

Each published campaign commits its session record; a declared campaign that
has not run commits only its declaration. Both must stay valid under the
validator of the checkout, whatever fields later campaigns add to its table.

A calibrating issue also commits `selector-outcomes.json`, which cites each
measurement outcome that bears on a selector by path, SHA-256 and outcome. A
citation must name the cited record's bytes and outcome, and the committed
tuning profiles must hold what the outcome says: after `no-change`, every
profile stating the selector holds the value of its conservative definition.

`CodingStage` builds the stage of a coding campaign from synthetic timings: it
measures `gf2-coding` and imports the committed core and algebra envelopes.
The validator must accept its parts and reject a stage that differs from the
declared owner in one identity.

Run from any directory:
    python3 validate_tuning_extent_campaign_test.py
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.dont_write_bytecode = True

from repository_files import repository_root, tracked_files  # noqa: E402

VALIDATOR = Path(__file__).with_name("validate-tuning-extent-campaign.py")
ROOT = repository_root(Path(__file__).resolve())
OUTPUT = "GF2_TUNING_VALIDATION="
SELECTOR_OUTCOMES = "selector-outcomes.json"
CORE_SECTION = "gf2-core/selectors"


def validate(*arguments: str) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, str(VALIDATOR), *arguments],
                          capture_output=True, text=True)


def session_record(issue: str) -> Path:
    """The one committed session record of a campaign of `issue`."""
    found = {(ROOT / path).parent for path in tracked_files(ROOT, "core-list-grid.json")
             if (ROOT / path).parent.name.startswith(f"gf2-{issue}-")}
    if len(found) != 1:
        raise LookupError(f"{len(found)} committed session records belong to {issue}")
    return found.pop()


def stated(profile: dict, selector: str):
    """The value `profile` states for the core `selector`, or `None`."""
    value = profile.get("sections", {}).get(CORE_SECTION, {}).get("selectors", {})
    for part in selector.split("."):
        value = value.get(part) if isinstance(value, dict) else None
    return value


def committed_profiles() -> dict[str, dict]:
    """Every tracked profile envelope lying beside a conservative envelope."""
    profiles = {}
    for conservative in tracked_files(ROOT, "conservative.json"):
        listing = subprocess.run(
            ["git", "-C", str(ROOT), "ls-files", "-z", "--", str(Path(conservative).parent)],
            capture_output=True, check=True, text=True).stdout
        for path in filter(None, listing.split("\0")):
            if path.endswith(".json"):
                envelope = json.loads((ROOT / path).read_bytes())
                if isinstance(envelope, dict) and "profile_format_version" in envelope:
                    profiles[path] = envelope
    return profiles


def conservative_definition(ledger: dict, claim: str, source: str) -> int:
    """The integer the ledger's `claim` line defines, read from `source`."""
    entry = next(item for item in ledger["claims"] if item["id"] == claim)
    if entry["verbatim"] not in source.splitlines():
        raise LookupError(f"{entry['path']} no longer holds the line of claim {claim}")
    return int(re.search(r"= ([0-9_]+);", entry["verbatim"]).group(1).replace("_", ""))


def outcome_problems(citation: dict, record: bytes, definition: int,
                     profiles: dict[str, dict]) -> list[str]:
    """Why `citation` disagrees with the cited `record` or with `profiles`."""
    selector, problems = citation["selector"], []
    if hashlib.sha256(record).hexdigest() != citation["sha256"]:
        return [f"{citation['record']} does not have sha256 {citation['sha256']}"]
    outcome = json.loads(record)
    for field in ["selector", "issue", "outcome"]:
        if outcome[field] != citation[field]:
            problems.append(f"cited {field} {citation[field]!r} is {outcome[field]!r} in the record")
    values = {path: stated(profile, selector) for path, profile in profiles.items()}
    values = {path: value for path, value in values.items() if value is not None}
    conservative = {path: value for path, value in values.items()
                    if profiles[path]["profile_id"] == "conservative"}
    if not conservative:
        problems.append(f"no conservative profile states {selector}")
    problems += [f"{path} states {value}, the conservative definition is {definition}"
                 for path, value in conservative.items() if value != definition]
    measured = {path: value for path, value in values.items() if path not in conservative}
    if outcome["outcome"] == "no-change":
        problems += [f"{path} states {value} after a no-change outcome at {definition}"
                     for path, value in measured.items() if value != definition]
    elif outcome["threshold_words"] not in measured.values():
        problems.append(f"no measured profile states the outcome's {outcome['threshold_words']}")
    return problems


class SelectorOutcomes(unittest.TestCase):
    def test_committed_citations_agree_with_the_profiles(self):
        profiles = committed_profiles()
        records = tracked_files(ROOT, SELECTOR_OUTCOMES)
        self.assertTrue(records, "no selector outcome record is committed")
        for path in records:
            for citation in json.loads((ROOT / path).read_bytes())["outcomes"]:
                cited = citation["conservative_definition"]
                ledger = json.loads((ROOT / cited["ledger"]).read_bytes())
                claim = next(item for item in ledger["claims"] if item["id"] == cited["claim"])
                definition = conservative_definition(
                    ledger, cited["claim"], (ROOT / claim["path"]).read_text())
                self.assertEqual(outcome_problems(
                    citation, (ROOT / citation["record"]).read_bytes(), definition, profiles), [])

    def test_a_disagreement_is_reported(self):
        selector = "bit_matrix.matvec_simd_min_words"
        def profile(identity, value):
            return {"profile_id": identity, "sections": {CORE_SECTION: {"selectors": {
                "bit_matrix": {"matvec_simd_min_words": value}}}}}
        def record(outcome, threshold):
            return json.dumps({"selector": selector, "issue": "0000aaaa", "outcome": outcome,
                               "threshold_words": threshold}).encode()
        def citation(cited, outcome="no-change"):
            return {"selector": selector, "issue": "0000aaaa", "record": "outcome.json",
                    "sha256": hashlib.sha256(cited).hexdigest(), "outcome": outcome}
        unchanged, moved = record("no-change", None), record("threshold", 10)
        agreeing = {"conservative.json": profile("conservative", 8), "owner.json": profile("m", 8)}
        tuned = agreeing | {"owner.json": profile("m", 10)}
        self.assertEqual(outcome_problems(citation(unchanged), unchanged, 8, agreeing), [])
        self.assertEqual(outcome_problems(citation(moved, "threshold"), moved, 8, tuned), [])
        for cited, bytes_, definition, profiles in [
            (citation(unchanged), unchanged + b" ", 8, agreeing),       # other bytes
            (citation(moved), moved, 8, tuned),                         # other outcome
            (citation(unchanged), unchanged, 8, tuned),                 # a profile moved
            (citation(unchanged), unchanged, 9, agreeing),              # the definition moved
            (citation(moved, "threshold"), moved, 8, agreeing),         # the value not installed
            (citation(unchanged), unchanged, 8, {"owner.json": profile("m", 8)}),
        ]:
            self.assertNotEqual(outcome_problems(cited, bytes_, definition, profiles), [])
        self.assertRaises(LookupError, conservative_definition,
                          {"claims": [{"id": "c", "path": "x.rs", "verbatim": "const A: usize = 8;"}]},
                          "c", "const A: usize = 9;\n")


class CommittedRecords(unittest.TestCase):
    def accepted(self, *arguments: str) -> dict:
        result = validate(*arguments)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(result.stdout.startswith(OUTPUT), result.stdout)
        return json.loads(result.stdout.removeprefix(OUTPUT))

    def rejected(self, *arguments: str) -> None:
        result = validate(*arguments)
        self.assertEqual(result.returncode, 2, result.stdout)

    def test_a83583e0_session_record_validates(self):
        record = session_record("a83583e0")
        self.assertEqual(self.accepted("--session-record", str(record))["campaign_id"],
                         record.name.removesuffix("-session"))

    def test_dbd8787d_session_record_validates(self):
        record = session_record("dbd8787d")
        self.assertEqual(self.accepted("--session-record", str(record))["campaign_id"],
                         record.name.removesuffix("-session"))

    def test_each_record_selects_its_own_field_table(self):
        earlier, later = (self.accepted("--session-record", str(session_record(issue)))
                          for issue in ["a83583e0", "dbd8787d"])
        self.assertLess(earlier["retained_fields"], later["retained_fields"])
        self.assertLess(earlier["cells"], later["cells"])

    def test_63bad95d_declaration_covers_the_complete_table(self):
        declared = self.accepted("--declaration-check", "63bad95d")
        published = self.accepted("--session-record", str(session_record("dbd8787d")))
        self.assertGreater(declared["retained_fields"], published["retained_fields"])

    def test_a_narrower_declaration_needs_its_inventory(self):
        self.rejected("--declaration-check", "dbd8787d")

    def test_a_record_is_rejected_under_another_campaigns_declaration(self):
        source, other = session_record("a83583e0"), session_record("dbd8787d")
        with tempfile.TemporaryDirectory() as directory:
            forged = Path(directory) / other.name
            shutil.copytree(source, forged)
            self.rejected("--session-record", str(forged))


def load_validator():
    spec = importlib.util.spec_from_file_location("tuning_campaign_validator", VALIDATOR)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


USIZE_MAX = 2**64 - 1
CODING_ISSUE = "c0d1f1ed"
CODING_HARNESS = "crates/gf2-coding/benches/tuning_calibration.rs"
# The coding producer's grid, spelled apart from the validator's tables. A code
# is (name, frame length, edges, per-mille noise deviation).
CODING_CODES = [("nr5g-bg2-z16", 832, 3152, 1200), ("dvb-t2-short-r1-2", 16200, 48599, 800),
                ("dvb-t2-normal-r1-2", 64800, 226799, 750)]
# Per decode field: candidates with the conservative value among them, strata
# as (code, frames, open lanes), seed tag, batch entry point, and the rank of
# the candidate the synthetic timings make fastest.
CODING_FIELDS = {
    "lane_min_frames": ([1, 2, 3, 4, 6, 8, USIZE_MAX], USIZE_MAX,
                        [(0, frames, True) for frames in (1, 2, 3, 4, 6, 8)], 32,
                        "ldpc-batch-decoder", 6),
    "lane_max_edges": ([0, 3152, 48599, 226799], 0, [(code, 8, True) for code in range(3)],
                       33, "ldpc-batch-decoder", 0),
    "granules_per_task": ([1, 2, 4, 8, 16], 1,
                          [(code, 128, opened) for code in (0, 1) for opened in (False, True)],
                          34, "pooled-ldpc-batch-decoder", 2),
}


def stratum_token(code: int, frames: int, opened: bool) -> str:
    return f"{CODING_CODES[code][0]}-f{frames}-{'open' if opened else 'closed'}"


def installed(key: str, frames: int, code: int, opened: bool, candidate: int):
    """The decode selectors a coding case installs and the schedule they give."""
    selectors = {"lane_min_frames": 1 if opened else USIZE_MAX,
                 "lane_max_edges": USIZE_MAX if opened else 0, "granules_per_task": 1,
                 key: candidate}
    lane = (min(frames, 8) >= selectors["lane_min_frames"]
            and CODING_CODES[code][2] <= selectors["lane_max_edges"])
    task = min(8 * candidate, frames) if key == "granules_per_task" else frames
    return selectors, {"route": "lane-portable" if lane else "single-frame",
                       "task_frames": task, "tasks": -(-frames // task)}


class CodingStage:
    """The stage and repository of a campaign that measures the coding owner
    and imports the core and algebra envelopes one committed campaign
    published. `behavior` and `section` are the identities the stage records
    for its owner; `declared_cells` is the cell count its declaration states;
    `source_dirty` is the checkout status its campaign record observes; `rng`
    is the fixture generator its capability report states, if any."""

    CLEAN = {"outcome": "exited", "pid": 7, "exit_code": 0, "elapsed_ns": 1,
             "all_descendants_reaped": True}
    STAMP = "1970-01-01T00:00:00Z"
    REVISION = "0" * 40
    protocol_sha = hashlib.sha256(b"protocol").hexdigest()

    def __init__(self, validator, directory: Path, *,
                 behavior: str = "coding-tuning-calibration-v1",
                 section: str = "gf2-coding/encode", declared_cells: int | None = None,
                 source_dirty: bool = False, rng: dict | None = None):
        self.v, self.behavior, self.section = validator, behavior, section
        self.source_dirty, self.rng = source_dirty, rng
        self.root, self.stage = directory / "repository", directory / "stage"
        self.campaign = f"gf2-{CODING_ISSUE}-19700101t000000z-1"
        self.layout = validator.publication_layout(self.campaign)
        self.records: list[dict] = []
        self.event("campaign-start", {})
        self.manifest = self.owner_manifest(self.staged_processes())
        cells = len(self.manifest["ordered_units"]) // 6
        self.declaration = self.declare(cells if declared_cells is None else declared_cells)
        self.bundle, self.checkpoints = self.accept()
        self.config = self.campaign_record()
        self.response = self.emit_owner()
        self.compose()
        self.reports = self.preflight_reports()
        self.projection = self.receipt_documents()

    def write(self, relative: str, data: bytes) -> dict:
        target = self.stage / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        return {"path": str(target), "sha256": hashlib.sha256(data).hexdigest()}

    def event(self, kind: str, details: dict) -> dict:
        self.records.append({"schema": "tuning-campaign-journal-v1", "timestamp_utc": self.STAMP,
                             "campaign_id": self.campaign, "session_id": "session-1",
                             "sequence": len(self.records), "event": kind, "details": details})
        return self.records[-1]

    def budget(self, process: str, boundary: str) -> None:
        self.event("driver-diagnostic", {
            "kind": "session-budget-observation", "boundary": boundary, "process": process,
            "unit_key": None, "active_elapsed_ns": 1, "may_launch_child": True,
            "session_budget_seconds": 10_800, "child_timeout_seconds": 120,
            "child_kill_grace_seconds": 5})

    def staged_processes(self) -> list[dict]:
        self.stage.mkdir(parents=True)
        self.processes = []
        for name in ["coding-producer", "composer", "driver"]:
            staged = self.write(f"bin/{name}", name.encode())
            self.processes.append({
                "id": name, "executable": staged["path"], "executable_sha256": staged["sha256"],
                "arguments": ["--fresh-child"] if name == "coding-producer" else [],
                "environment": self.v.ENVIRONMENT, "working_directory": str(self.stage)})
        return self.processes

    def declare(self, cells: int) -> dict:
        """The driver fixture's coding declaration: the algebra import of the
        committed seam campaign and the core envelope of the same run."""
        algebra = self.v.issue_declaration("dbd8787d", ROOT)["imported_owners"][0]
        run = Path(algebra["envelope"]["path"]).stem
        core_path = f"crates/gf2-core/data/tuning-profiles/{run}.json"
        core = {"owner": "gf2-core", "name": "core", "section": "gf2-core/selectors",
                "envelope": {"path": core_path,
                             "sha256": hashlib.sha256((ROOT / core_path).read_bytes()).hexdigest()},
                "complete": algebra["complete"]}
        declaration = {"schema": "tuning-campaign-declaration-v1", "issue": CODING_ISSUE,
                       "protocol": "protocol.md",
                       "producing_manifest": "producing-build-inputs.json",
                       "measured_owners": [{"owner": "gf2-coding", "name": "coding",
                                            "cells": cells}],
                       "imported_owners": [core, algebra], "evidence_index": True}
        self.root.mkdir()
        subprocess.run(["git", "-C", str(self.root), "init", "-q"], check=True)
        for path in {core_path, algebra["envelope"]["path"], algebra["complete"]["path"]}:
            (self.root / path).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / path, self.root / path)
        (self.root / "campaign-declaration.json").write_bytes(self.v.compact(declaration))
        return declaration

    def owner_manifest(self, processes: list[dict]) -> dict:
        channels = {"journal_schema": "tuning-campaign-journal-v1",
                    "checkpoint_schema": "tuning-campaign-checkpoint-v1",
                    "raw_sample_schema": "raw-timing-samples-v3",
                    "execution_log": str(self.stage / "execution.log"),
                    "checkpoints": str(self.stage / "checkpoints")}
        blocks, units = [], []
        for key, (candidates, _, strata, tag, _, _) in CODING_FIELDS.items():
            for index, (code, frames, opened) in enumerate(strata):
                labels = [f"v{candidate}" for candidate in candidates]
                blocks.append({"phase": "decode-extent", "field": f"decode.{key}",
                               "stratum": stratum_token(code, frames, opened),
                               "base_candidates": labels})
                orders = [({"kind": "probe"}, candidates)]
                for execution in range(5):
                    shift = execution % len(candidates)
                    order = candidates[shift:] + candidates[:shift]
                    orders.append(({"kind": "measure", "execution": execution},
                                   order[::-1] if execution % 2 else order))
                for task, order in orders:
                    for candidate in order:
                        identity = {"protocol": "coding-tuning-campaign-v1",
                                    "owner": "gf2-coding", "campaign_id": self.campaign,
                                    "phase": "decode-extent", "field": f"decode.{key}",
                                    "stratum": stratum_token(code, frames, opened),
                                    "candidate": f"v{candidate}", "task": task}
                        role = lambda bank: 0xD00 + (bank << 16)
                        case = {"schema": "coding-tuning-campaign-v1", "identity": identity,
                                "protocol_sha256": self.protocol_sha, "field": key,
                                "stratum_index": index,
                                "stratum": {"code": code, "frames": frames, "open_lanes": opened},
                                "code": CODING_CODES[code][0], "candidate": candidate,
                                "pool_threads": 4, "max_iterations": 50,
                                "seeds": {"schema": "fixture-seeds-v3",
                                          "root": 0x5ECC9BF800000000,
                                          "derivation": "gf2-calibration-seed-v1",
                                          "field_tag": tag, "shape_key": code,
                                          "streams": [{"bank": bank, "role": role(bank),
                                                       "seed": self.v.mix_seed(
                                                           0x5ECC9BF800000000, tag, code,
                                                           role(bank))}
                                                      for bank in range(8)]},
                                "channels": channels}
                        units.append({"ordinal": len(units), "identity": identity,
                                      "key": self.sha(identity), "process": "coding-producer",
                                      "case": self.v.compact(case).decode(),
                                      "expected_progress": 0 if task["kind"] == "probe" else 6})
        return self.sealed({"schema": "tuning-campaign-owner-manifest-v1", "owner": "gf2-coding",
                            "owner_protocol": "coding-tuning-campaign-v1",
                            "behavior_token": self.behavior, "campaign_id": self.campaign,
                            "phases": ["decode-extent"], "candidate_blocks": blocks,
                            "counts": self.counts(len(units)), "processes": processes[:1],
                            "ordered_units": units})

    def sha(self, value) -> str:
        return hashlib.sha256(self.v.compact(value)).hexdigest()

    def sealed(self, body: dict) -> dict:
        body.pop("manifest_sha256", None)
        return {**body, "manifest_sha256": self.sha(body)}

    @staticmethod
    def counts(units: int) -> dict:
        cells = units // 6
        return {"cells": cells, "probes": cells, "timed_children": units - cells,
                "accepted_results": units, "windows": 5 * (units - cells),
                "progress_records": 6 * (units - cells)}

    def accept(self) -> tuple[dict, dict]:
        """Each unit's result under timings that rise with the rank distance
        from the field's fastest candidate, alike in every stratum."""
        accepted, checkpoints = [], {}
        for unit in self.manifest["ordered_units"]:
            case = json.loads(unit["case"])
            key, stratum, candidate = case["field"], case["stratum"], case["candidate"]
            candidates, _, _, _, entry, fastest = CODING_FIELDS[key]
            code, frames = stratum["code"], stratum["frames"]
            selectors, schedule = installed(key, frames, code, stratum["open_lanes"], candidate)
            hashes = self.v.forced_hashes("gf2-coding", {"encode": self.v.CODING_ENCODE,
                                                         "decode": selectors})
            witness = lambda what: self.sha([what, key, unit["identity"]["stratum"]])
            payload = {
                "schema": "coding-tuning-campaign-v1", "field": key,
                "stratum_index": case["stratum_index"], "stratum": stratum,
                "candidate": candidate, "schedule": schedule,
                "code": dict(zip(["name", "frame_len", "edges", "workspace_bytes"],
                                 [*CODING_CODES[code][:3], 1])),
                "profile": {"profile_id": "coding-decode-forced",
                            "owner_section_id": self.section, "section_schema_version": 1,
                            "harness_schema": self.behavior, "resolution": "installed",
                            "active_decode": selectors, "envelope_sha256": hashes["envelope"],
                            "content_sha256": hashes["content"],
                            "wrapper_sha256": hashes["section"]},
                "capability": {"required_features": self.v.FEATURES, "pool_threads": 4,
                               "lane_backend": "portable", "entry": entry},
                "semantics": {"fixture_count": 8, "frames_per_fixture": frames,
                              "operands_sha256": witness("operands"),
                              "batch_results_sha256": witness("results"),
                              "single_frame_results_sha256": witness("results"),
                              "single_frame_equal": True, "converged_frames": 8 * frames,
                              "iterations": 8 * frames}}
            task = unit["identity"]["task"]
            cost = 100 + 10 * abs(candidates.index(candidate) - fastest)
            samples = [] if task["kind"] == "probe" else [
                {"execution": task["execution"], "repetition": repetition, "calls": 1,
                 "elapsed_ns": cost} for repetition in range(5)]
            result = {"schema": "tuning-campaign-child-result-v1", "identity": unit["identity"],
                      "case_sha256": hashlib.sha256(unit["case"].encode()).hexdigest(),
                      "outcome": "complete", "samples": samples,
                      "payload": self.v.compact(payload).decode()}
            bound = {"result": result, "journal_prefix": None, "attempt": "attempt-0",
                     "spawn_sequence": 0, "validation_sequence": 0}
            checkpoint = {"result": bound, "result_sha256": self.sha(bound), "case": unit,
                          "case_sha256": self.sha(unit)}
            checkpoints[unit["key"]] = (checkpoint, self.sha(checkpoint))
            accepted.append({"unit": unit, "result": result,
                             "checkpoint_sha256": self.sha(checkpoint)})
        bundle = {"schema": "tuning-campaign-owner-input-v1",
                  "manifest_sha256": self.manifest["manifest_sha256"], "accepted": accepted}
        self.write("coding-accepted.json", self.v.compact(bundle))
        self.write("coding-manifest.json", self.v.compact(self.manifest))
        self.write("checkpoints/manifest.json", b"{}")
        return bundle, checkpoints

    def campaign_record(self) -> dict:
        receipt = self.layout["receipt.md"]
        declaration = (self.root / "campaign-declaration.json").read_bytes()
        composer = self.processes[1]
        imported = []
        for declared in self.declaration["imported_owners"]:
            name, source = declared["name"], (self.root / declared["envelope"]["path"])
            staged = self.write(f"{name}-owner.json", source.read_bytes())
            complete = (self.root / declared["complete"]["path"]).read_bytes()
            wrapper = json.loads(source.read_bytes())["sections"][declared["section"]]
            directory = f"imported/{name}-session-1-{len(self.records)}"
            other = next(owner for owner in ("core", "algebra") if owner != name)
            probe = self.write(f"{directory}/probe-{other}.json", b"{}")
            probe_complete = self.write(f"{directory}/probe-complete.json", complete)
            tail = [self.campaign, self.STAMP, self.REVISION, json.dumps(self.source_dirty),
                    composer["executable_sha256"]]
            inputs = {name: staged["path"], other: probe["path"]}
            commands = [[f"{other}-owner", probe["path"]],
                        ["complete", inputs["core"], inputs["algebra"], probe_complete["path"]]]
            runs = []
            for index, command in enumerate(commands):
                request = self.write(f"{directory}/request-{index}.json", self.v.compact({
                    "schema": "tuning-campaign-imported-owner-request-v1",
                    "process": composer, "args": command + tail}))
                leave = self.write(f"{directory}/exit-{index}.json", self.v.compact(
                    {"outcome": self.CLEAN, "stdout": [], "stderr": []}))
                self.budget("composer", "before-launch")
                self.event("orchestration-start", {"kind": "orchestration-start",
                                                   "process": "composer", "request": request})
                self.event("orchestration-exit", {"kind": "orchestration-exit",
                                                  "process": "composer", "exit": leave,
                                                  "outcome": self.CLEAN})
                self.budget("composer", "after-result")
                runs.append({"request": request, "exit": leave})
            reopen = self.write(f"{directory}/reopen.json", self.v.compact({
                "schema": "tuning-campaign-imported-owner-v1", "owner": declared["owner"],
                "section": declared["section"], "source": declared["envelope"],
                "reference": declared["complete"], "staged": staged,
                "wrapper_sha256": self.sha(self.v.sorted_json(wrapper)),
                "probe_complete": probe_complete, "runs": runs, f"probe_{other}": probe}))
            imported.append({"owner": declared["owner"], "source": declared["envelope"],
                             "staged": staged, "reopen": reopen})
        runtime = {"source_dirty": self.source_dirty, "toolchain": "rustc 1.95.0",
                   "cpu_model": "fixture",
                   "cpu_features": ["avx2"], "os_kernel": "fixture", "governor": "{}",
                   "receipt": receipt}
        return {"campaign_id": self.campaign, "affinity": [0], "receipt": receipt,
                "identity": {"source_revision": self.REVISION, "host_identity": "host;cpus=[0]",
                             "executable_sha256": {process["id"]: process["executable_sha256"]
                                                   for process in self.processes},
                             "behavior_sha256": {
                                 "campaign-declaration.json":
                                     hashlib.sha256(declaration).hexdigest()}},
                "manifests": [self.manifest], "processes": self.processes,
                "runtime": self.v.compact(runtime).decode(),
                "protocol": {"path": "/protocol.md", "sha256": self.protocol_sha},
                "imported_owners": imported}

    def analysis(self) -> dict:
        """The owner's decisions over the accepted timings; a field is emitted
        when its decision selects another candidate than the conservative one."""
        medians = {}
        for entry in self.bundle["accepted"]:
            identity, samples = entry["unit"]["identity"], entry["result"]["samples"]
            if samples:
                medians.setdefault((identity["field"], identity["candidate"],
                                    identity["stratum"]), []).append(
                    float(samples[0]["elapsed_ns"]) / samples[0]["calls"])
        fields = []
        for key, (candidates, conservative, strata, *_) in CODING_FIELDS.items():
            series = [{"candidate": candidate,
                       "schedules": [installed(key, frames, code, opened, candidate)[1]
                                     for code, frames, opened in strata],
                       "strata": [medians[(f"decode.{key}", f"v{candidate}",
                                           stratum_token(*stratum))] for stratum in strata]}
                      for candidate in candidates]
            decision = self.v.analyze_extent(series, conservative)
            fields.append({"field": f"decode.{key}", "selected": decision["selected"],
                           "emitted": decision["reason"] == "selected-non-default",
                           "decision": decision})
        return {"schema": "coding-tuning-analysis-v1", "fields": fields,
                "accepted_result_count": len(self.bundle["accepted"])}

    def emit_owner(self) -> dict:
        producer, identity = self.processes[0], self.config["identity"]
        runtime = json.loads(self.config["runtime"])
        directory = f"candidates/coding-producer-session-1-{len(self.records)}"
        analysis = self.analysis()
        stated = {item["field"].removeprefix("decode."): item["selected"]
                  for item in analysis["fields"] if item["emitted"]}
        measurement = {"kind": "calibrated", "measured_at": self.STAMP,
                       "source_revision": self.REVISION, "source_dirty": self.source_dirty,
                       "harness": CODING_HARNESS, "harness_schema": self.behavior,
                       "binary_sha256": producer["executable_sha256"],
                       "toolchain": runtime["toolchain"], "host": identity["host_identity"],
                       **{key: runtime[key] for key in
                          ["cpu_model", "cpu_features", "os_kernel", "governor", "receipt"]}}
        self.sections = {self.section: {"schema_version": 1, "measurement": measurement,
                                        "selectors": {"decode": stated} if stated else {}}}
        output = self.write(f"{directory}/output.json", self.v.compact(self.envelope(
            self.sections, CODING_HARNESS, producer["executable_sha256"])))
        observed = lambda runtime: {"identity": identity, "process": "coding-producer",
                                    "observed_utc": self.STAMP, "runtime": runtime}
        request = self.write(f"{directory}/request.json", self.v.compact({
            "operation": "emit-owner", "request": {
                "campaign_id": self.campaign,
                "manifest_sha256": self.manifest["manifest_sha256"],
                "accepted_results": self.v.identity_for(self.stage / "coding-accepted.json"),
                "measurement": observed(self.config["runtime"]),
                "assembly": observed(self.v.compact({
                    "source_dirty": self.source_dirty, "tool": CODING_HARNESS,
                    "tool_sha256": producer["executable_sha256"]}).decode()),
                "output": output["path"]}}))
        response = {"operation": "emit-owner", "artifact": output,
                    "decisions": self.v.compact(analysis).decode()}
        stdout = b"GF2_TUNING_RESULT=" + self.v.compact(response)
        write = self.event("owner-write", {"owner": "gf2-coding", "request": request})
        self.budget("coding-producer", "before-launch")
        start = self.event("orchestration-start", {
            "kind": "orchestration-start", "process": "coding-producer",
            "request_sha256": request["sha256"], "arguments": ["--owner-operation"]})
        assert start["sequence"] == write["sequence"] + 2
        self.event("orchestration-exit", {
            "kind": "orchestration-exit", "process": "coding-producer",
            "start_sequence": start["sequence"], "outcome": self.CLEAN,
            "stdout_sha256": hashlib.sha256(stdout).hexdigest(),
            "stderr_sha256": hashlib.sha256(b"").hexdigest(), "stderr": []})
        self.write(f"{directory}/exit.json", self.v.compact(
            {"outcome": self.CLEAN, "stdout": list(stdout), "stderr": []}))
        for relative in [f"{directory}/response.json", "coding-owner-response.json"]:
            self.write(relative, self.v.compact(response))
        promoted = self.write("coding-owner.json", (self.stage / directory / "output.json")
                              .read_bytes())
        self.event("owner-reopen", {"owner": "gf2-coding", "candidate": output,
                                    "artifact": promoted, "decisions": response["decisions"]})
        return response

    def envelope(self, sections: dict, tool: str, tool_sha: str) -> dict:
        return {"profile_format_version": 2, "profile_id": self.campaign,
                "assembly": {"kind": "assembled", "assembled_at": self.STAMP,
                             "source_revision": self.REVISION,
                             "source_dirty": self.source_dirty, "tool": tool, "tool_sha256": tool_sha,
                             "content_sha256": self.v.envelope_content(self.campaign, sections)},
                "sections": sections}

    def compose(self) -> None:
        composer = self.processes[1]
        owners = ["core", "algebra", "coding"]
        inputs = {name: self.v.identity_for(self.stage / f"{name}-owner.json")
                  for name in owners}
        sections = dict(self.sections)
        for declared in self.declaration["imported_owners"]:
            staged = json.loads((self.stage / f"{declared['name']}-owner.json").read_bytes())
            sections[declared["section"]] = staged["sections"][declared["section"]]
            reference = json.loads((self.root / declared["complete"]["path"]).read_bytes())
        directory = f"candidates/composer-session-1-{len(self.records)}"
        # A complete envelope names the composer as its assembling tool.
        candidate = self.write(f"{directory}/output.json", self.v.compact(self.envelope(
            sections, reference["assembly"]["tool"], composer["executable_sha256"])))
        args = ["complete", *(inputs[name]["path"] for name in owners), candidate["path"],
                self.campaign, self.STAMP, self.REVISION, json.dumps(self.source_dirty),
                composer["executable_sha256"]]
        request = self.write(f"{directory}/request.json", self.v.compact({
            "schema": "tuning-campaign-composition-request-v1", "process": composer,
            "args": args, **inputs}))
        self.event("composition", {"request": request, "args": args, "process": composer})
        self.budget("composer", "before-launch")
        self.event("orchestration-start", {"kind": "orchestration-start", "process": "composer",
                                           "request": request})
        leave = self.write(f"{directory}/exit.json", self.v.compact(
            {"outcome": self.CLEAN, "stdout": [], "stderr": []}))
        self.event("orchestration-exit", {"kind": "orchestration-exit", "process": "composer",
                                          "exit": leave, "outcome": self.CLEAN})
        self.budget("composer", "after-result")
        record = {"schema": "tuning-campaign-composition-v1", "args": args,
                  "source_revision": self.REVISION, "source_dirty": self.source_dirty,
                  "tool_sha256": composer["executable_sha256"], "candidate": candidate,
                  "request": request, "exit": leave, **inputs}
        for relative in [f"{directory}/response.json", "composition-candidate.json"]:
            self.write(relative, self.v.compact(record))
        complete = self.write("complete.json", (self.stage / directory / "output.json")
                              .read_bytes())
        self.write("composition.json", self.v.compact({**record, "output": complete}))
        self.event("composition-reopen", {"candidate": candidate, "artifact": complete})
        self.write("execution.log", b"".join(self.v.compact(record) + b"\n"
                                             for record in self.records))

    def preflight_reports(self) -> dict:
        counts = self.manifest["counts"]

        def report(mode: str, scope: str, details: list[str], inventory: list) -> dict:
            evidence = {"schema": "coding-tuning-owner-report-v1", "mode": mode, "scope": scope,
                        "owner": "gf2-coding", "owner_protocol": "coding-tuning-campaign-v1",
                        "behavior_token": self.behavior, "seed_schema": "fixture-seeds-v3",
                        "raw_sample_schema": "raw-timing-samples-v3", "cells": counts["cells"],
                        "accepted_results": counts["accepted_results"],
                        "windows": counts["windows"], "details": details,
                        **({"rng": self.rng} if self.rng and mode == "capability-report"
                           else {}),
                        "inventory": inventory}
            response = {"operation": mode, "evidence": self.v.compact(evidence).decode()}
            self.write(f"coding-{mode}.json", self.v.compact(response))
            return response
        grid, inventory = [], []
        for key, (candidates, conservative, strata, tag, entry, _) in CODING_FIELDS.items():
            grid.append(f"decode.{key}: candidates={candidates} conservative={conservative}")
            grid += [f"decode.{key}: stratum={stratum_token(*stratum)} entry={entry}"
                     for stratum in strata]
            case = lambda code: json.loads(next(
                unit["case"] for unit in self.manifest["ordered_units"]
                if unit["identity"]["field"] == f"decode.{key}"
                and json.loads(unit["case"])["stratum"]["code"] == code))
            for code in dict.fromkeys(code for code, _, _ in strata):
                name, frame_len, edges, sigma = CODING_CODES[code]
                inventory.append({
                    "field": f"decode.{key}", "code": name, "frame_len": frame_len,
                    "edges": edges, "noise_sigma_milli": sigma,
                    "sigma_scale_milli": [940, 1000, 880, 1030, 970, 910, 1060, 850],
                    "max_iterations": 50,
                    "batch_frames": list(dict.fromkeys(
                        frames for other, frames, _ in strata if other == code)),
                    "seeds": case(code)["seeds"]})
        return {
            "coding-capability-report": report(
                "capability-report", "representative-capabilities-no-grid-execution",
                ["dedicated-pool-threads=4", "lane-backend=portable", "lanes=8",
                 f"required-features={self.v.FEATURES}", "full-grid-checked=false",
                 "profile-installation=not-performed"], []),
            "coding-list-grid": report(
                "list-grid", "full-grid-no-measurement",
                grid + ["decoder-config=DecoderConfig { algorithm: MinSum, "
                        "early_termination: true }", "codeword=all-zero channel=bpsk-awgn",
                        "profile-installation=not-performed"], inventory),
            "coding-self-check": report(
                "self-check", "complete-declaration-no-measurement",
                ["codec-inventory=3/7", "emission-round-trips=8/8",
                 "profile-installation=not-performed"], [])}

    def receipt_documents(self) -> dict:
        """The receipt projection, the raw result index, the archived owner
        decisions and the receipt of a campaign that measures no core."""
        counts = self.manifest["counts"]
        orchestration = sum(record["event"] == "orchestration-start" for record in self.records)
        projection = {"owners": [self.response], "counts": counts, "attempts": counts["probes"]
                      + counts["timed_children"], "orchestration": orchestration, "sessions": 1}
        self.write("receipt-projection.json", self.v.compact(projection))
        index = "# Raw result index\n\n"
        for entry in self.bundle["accepted"]:
            identity = entry["unit"]["identity"]
            task = ("Probe" if identity["task"]["kind"] == "probe"
                    else f"Measure {{ execution: {identity['task']['execution']} }}")
            index += (f"- `{entry['unit']['key']}`: `{identity['field']}` / "
                      f"`{identity['stratum']}` / `{identity['candidate']}` / `{task}`\n")
        index_sha = self.write("raw-result-index.md", index.encode())["sha256"]
        decisions_sha = self.write("owner-decisions.json",
                                   self.v.compact([self.response]))["sha256"]
        archive, session = self.layout["archive"], self.layout["session"]
        artifact = Path(self.response["artifact"]["path"]).relative_to(self.stage).as_posix()
        shown = [{**self.response, "artifact": {"path": f"{archive}/{artifact}",
                                                "sha256": self.response["artifact"]["sha256"]}}]
        text = self.v.RECEIPT_TEMPLATE.format(
            self.campaign, self.declaration["protocol"], self.protocol_sha,
            *next(iter(self.config["identity"]["behavior_sha256"].items())), self.REVISION,
            self.layout["checksum"], "Owner projections preserve every field's extent decision",
            json.dumps(shown, indent=2))
        text += "## Imported owners\n\n"
        for owner in self.declaration["imported_owners"]:
            text += (f"Owner `{owner['owner']}` is imported unmeasured from "
                     f"`{owner['envelope']['path']}` (SHA-256 `{owner['envelope']['sha256']}`). "
                     f"The complete envelope carries its `{owner['section']}` section wrapper "
                     f"unchanged; the committed complete envelope `{owner['complete']['path']}` "
                     f"(SHA-256 `{owner['complete']['sha256']}`) holds the same wrapper.\n\n")
        text += ("## Cited evidence\n\nArchived paths lie in the host-local evidence archive "
                 "that the committed checksum manifest pins.\n\n"
                 "| Evidence | Path | SHA-256 |\n|---|---|---|\n")
        self.write("campaign.json", self.v.compact(self.config))
        reopen = [(f"{entry['owner']} imported-owner reopen record",
                   Path(entry["reopen"]["path"]).relative_to(self.stage).as_posix())
                  for entry in self.config["imported_owners"]]
        for label, source in [("Campaign record", "campaign.json"),
                              ("gf2-coding owner manifest", "coding-manifest.json"),
                              ("gf2-coding owner response", "coding-owner-response.json"),
                              *reopen, ("Composition record", "composition.json"),
                              ("Checkpoint manifest", "checkpoints/manifest.json"),
                              ("Receipt projection", "receipt-projection.json"),
                              ("Raw result index", "raw-result-index.md"),
                              ("Owner decisions", "owner-decisions.json")]:
            home = session if source == "composition.json" else archive
            sha = hashlib.sha256((self.stage / source).read_bytes()).hexdigest()
            text += f"| {label} | `{home}/{source}` | `{sha}` |\n"
        text += ("\n## Raw result index\n\nPreparation CPU affinity: `[0]`. Held-lock "
                 "observations are recorded in each session journal and must equal this set."
                 f"\n\nOwner `gf2-coding` uses protocol `coding-tuning-campaign-v1` and behavior "
                 f"`{self.behavior}`; executable `{self.processes[0]['executable_sha256']}`.\n\n"
                 f"Accepted accounting: {counts['cells']} cells, {counts['probes']} probes, "
                 f"{counts['timed_children']} timed children, {counts['accepted_results']} "
                 f"accepted results, {counts['windows']} raw windows, "
                 f"{counts['progress_records']} timing progress records. Observed "
                 f"{projection['attempts']} attempts, {orchestration} orchestration actions, 1 "
                 "sessions at the receipt projection journal_sequence. Later finalization and "
                 "resume events remain in the authoritative journal.\n\n"
                 f"The archived `{archive}/raw-result-index.md` lists every accepted raw key "
                 f"with its field, stratum, candidate and task; SHA-256 `{index_sha}`.\n"
                 f"The archived `{archive}/owner-decisions.json` holds the complete owner "
                 f"decisions, every raw timing window included; SHA-256 `{decisions_sha}`. The "
                 "decisions above replace each raw `samples` array with its `sample_count`.\n")
        self.write("receipt.md", text.encode())
        return projection

    def publish(self, repository: Path) -> dict:
        """Publishes the stage into `repository` under the validator's own
        destination mapping, as the publication self-test of the validator
        does, and returns the mapping."""
        v, layout = self.v, self.layout
        self.write("sessions/session-1/checksum.json", b"{}")
        self.write("repository-publication/validation.json", b"{}")
        self.write(v.EVIDENCE_INDEX, v.expected_evidence_index(
            self.stage, self.declaration).encode())
        mapping = v.expected_publication(self.stage, self.campaign)
        entries = []
        for destination in sorted(mapping):
            data = (self.stage / mapping[destination]).read_bytes()
            entries.append({"source": mapping[destination], "destination": destination,
                            "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()})
            (repository / destination).parent.mkdir(parents=True, exist_ok=True)
            (repository / destination).write_bytes(data)
        record = f"{layout['archive']}/repository-publication/plan.json"
        verification = f"{layout['session']}/repository-reopen.json"
        plan = v.compact({"schema": v.REPOSITORY_PLAN_SCHEMA, "campaign_id": self.campaign,
                          "entries": entries, "committed_directories": [layout["session"]],
                          "archive_root": layout["archive"], "record": record,
                          "archive_manifest": layout["archive-manifest"],
                          "pinned_archive": [layout["execution.log"]],
                          "verification": verification,
                          "checksum_manifest": layout["checksum"]})
        self.write("repository-publication/plan.json", plan)
        (repository / record).parent.mkdir(parents=True, exist_ok=True)
        (repository / record).write_bytes(plan)
        archived = lambda entry: entry["destination"].startswith(layout["archive"] + "/")
        rows = lambda chosen: [(entry["destination"], entry["sha256"])
                               for entry in entries if archived(entry) == chosen]
        archive_text = v.checksum_rows(rows(True) + [(record, hashlib.sha256(plan).hexdigest())])
        (repository / layout["archive-manifest"]).write_bytes(archive_text)
        published = {"coding": {"path": layout["coding-owner.json"], "sha256": hashlib.sha256(
            (self.stage / "coding-owner.json").read_bytes()).hexdigest()}}
        for owner in self.declaration["imported_owners"]:
            published[owner["name"]] = owner["envelope"]
            target = repository / owner["envelope"]["path"]
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(self.root / owner["envelope"]["path"], target)
        composition = json.loads((self.stage / "composition.json").read_bytes())
        evidence = v.compact({
            **published, "complete": {"path": layout["complete.json"],
                                      "sha256": composition["output"]["sha256"]},
            "composer_sha256": composition["tool_sha256"],
            "profile_arguments": composition["args"][-5:],
            "schema": v.REPOSITORY_REOPEN_SCHEMA})
        (repository / verification).write_bytes(evidence)
        log = hashlib.sha256((self.stage / "execution.log").read_bytes()).hexdigest()
        (repository / layout["checksum"]).write_bytes(
            v.checksum_rows(rows(False) + [(verification, hashlib.sha256(evidence).hexdigest())])
            + v.checksum_rows([(layout["execution.log"], log),
                               (layout["archive-manifest"],
                                hashlib.sha256(archive_text).hexdigest())]))
        return mapping


class CodingOwner(unittest.TestCase):
    """A campaign that measures `gf2-coding` and imports `gf2-core` and
    `gf2-algebra`, checked by the parts of the stage validation."""

    @classmethod
    def setUpClass(cls):
        cls.v = load_validator()

    def staged(self, **identities) -> CodingStage:
        directory = tempfile.TemporaryDirectory(prefix="gf2-validator-coding-stage-")
        self.addCleanup(directory.cleanup)
        stage = CodingStage(self.v, Path(directory.name).resolve(), **identities)
        root = mock.patch.object(self.v, "repository_root", return_value=stage.root)
        root.start()
        self.addCleanup(root.stop)
        return stage

    def manifest(self, stage: CodingStage) -> list:
        return self.v.validate_manifest(stage.manifest, "gf2-coding", stage.stage,
                                        stage.protocol_sha, {})

    def results(self, stage: CodingStage) -> list:
        return self.v.validate_bundle(stage.bundle, stage.manifest, stage.checkpoints)

    def decisions(self, stage: CodingStage) -> dict:
        declaration = self.v.load_declaration(stage.campaign)
        return self.v.validate_decisions({"coding": stage.bundle}, {"coding": stage.response},
                                         declaration, {})

    def preflight(self, stage: CodingStage) -> dict:
        return self.v.validate_preflight_responses(stage.reports,
                                                   self.v.load_declaration(stage.campaign))

    def receipt(self, stage: CodingStage) -> None:
        self.v.validate_receipt_documents(stage.stage, stage.campaign, [stage.bundle],
                                          stage.config, stage.projection)

    def composition(self, stage: CodingStage, decisions: dict | None = None) -> None:
        self.v.validate_envelopes(stage.stage, stage.campaign, {"coding": stage.response},
                                  decisions or self.decisions(stage),
                                  {"coding": stage.manifest}, stage.config, stage.records)

    def entry_point(self, stage: CodingStage, *arguments: str) -> subprocess.CompletedProcess:
        """Runs the validator from inside the fixture repository, which it
        then resolves as its own."""
        script = stage.root / VALIDATOR.name
        shutil.copyfile(VALIDATOR, script)
        return subprocess.run([sys.executable, "-B", str(script), *arguments],
                              capture_output=True, text=True)

    def accepted(self, stage: CodingStage, *arguments: str) -> dict:
        result = self.entry_point(stage, *arguments)
        self.assertEqual(result.returncode, 0, result.stderr)
        return json.loads(result.stdout.removeprefix(OUTPUT))

    def test_the_declaration_check_accepts_the_coding_grid(self):
        stage = self.staged()
        self.assertEqual(self.accepted(stage, "--declaration-check", CODING_ISSUE)["status"],
                         "declaration-valid")
        other = self.staged(declared_cells=75)
        result = self.entry_point(other, "--declaration-check", CODING_ISSUE)
        self.assertEqual(result.returncode, 2, result.stderr)

    def test_the_session_record_check_accepts_the_coding_reports(self):
        stage = self.staged()
        record = stage.root.parent / Path(stage.layout["session"]).name
        record.mkdir()
        for name in stage.reports:
            shutil.copyfile(stage.stage / f"{name}.json", record / f"{name}.json")
        self.assertEqual(self.accepted(stage, "--session-record", str(record))["campaign_id"],
                         stage.campaign)

    def test_the_stage_checks_accept_the_coding_stage(self):
        stage = self.staged()
        for mode in ["--receipt-check", "--imported-owner-check"]:
            self.accepted(stage, "--stage", str(stage.stage), mode)
        repository = stage.root.parent / "published"
        stage.publish(repository)
        self.assertEqual(self.accepted(stage, "--stage", str(stage.stage), "--publication",
                                       str(repository))["status"], "published")

    def rejects(self, check, stage: CodingStage, *arguments) -> None:
        with self.assertRaises((self.v.ValidationError, KeyError)):
            check(stage, *arguments)

    def test_the_owner_manifest_and_the_declared_counts_are_accepted(self):
        stage = self.staged()
        declared = self.v.declared_counts(self.v.load_declaration(stage.campaign))
        self.assertEqual(len(self.manifest(stage)), declared["accepted_results"])
        self.assertEqual(len(self.results(stage)), declared["accepted_results"])
        self.assertEqual(self.preflight(stage), {})
        self.assertEqual(stage.manifest["counts"], declared)

    def test_the_owner_decisions_are_recomputed(self):
        stage = self.staged()
        decided = self.decisions(stage)["coding_decisions"]["fields"]
        self.assertEqual([(item["selected"], item["emitted"]) for item in decided],
                         [(USIZE_MAX, False), (0, False), (4, True)])

    def test_the_raw_result_index_and_the_receipt_are_accepted(self):
        stage = self.staged()
        self.receipt(stage)
        receipt = (stage.stage / "receipt.md").read_text()
        self.assertNotIn("resolved owner manifest", receipt)
        self.assertNotIn("GEMM", receipt)
        index = stage.stage / "raw-result-index.md"
        index.write_text(index.read_text().replace("Probe", "Measure { execution: 0 }", 1))
        self.rejects(self.receipt, stage)

    def test_the_composition_of_three_owner_envelopes_is_accepted(self):
        stage = self.staged()
        self.composition(stage)
        complete = json.loads((stage.stage / "complete.json").read_bytes())
        self.assertEqual(set(complete["sections"]),
                         {"gf2-core/selectors", "gf2-algebra/permanent", "gf2-coding/encode"})
        self.assertEqual(len(json.loads((stage.stage / "composition.json").read_bytes())["args"]),
                         10)
        for entry in stage.config["imported_owners"]:
            self.assertIn("probe_algebra" if entry["owner"] == "gf2-core" else "probe_core",
                          json.loads(Path(entry["reopen"]["path"]).read_bytes()))

    def test_the_publication_paths_are_accepted(self):
        stage = self.staged()
        repository = stage.root.parent / "published"
        mapping = stage.publish(repository)
        layout = stage.layout
        self.assertEqual(self.v.validate_publication(str(stage.stage), str(repository))["status"],
                         "published")
        self.assertEqual(mapping[layout["coding-owner.json"]], "coding-owner.json")
        self.assertTrue(layout["coding-owner.json"].startswith("crates/gf2-coding/"))
        for name in ["core", "algebra"]:
            self.assertNotIn(layout[f"{name}-owner.json"], mapping)
            self.assertEqual(mapping[f"{layout['archive']}/{name}-owner.json"],
                             f"{name}-owner.json")
        for mode in ["capability-report", "list-grid", "self-check"]:
            self.assertEqual(mapping[f"{layout['session']}/coding-{mode}.json"],
                             f"coding-{mode}.json")
        index = (repository / layout["evidence-index"]).read_text()
        self.assertIn(f"| gf2-coding owner envelope | `{layout['coding-owner.json']}` |", index)
        self.assertEqual(index.count("owner envelope (imported)"), 2)
        self.assertNotIn("resolved owner manifest", index)
        (repository / layout["coding-owner.json"]).write_bytes(b"{}")
        self.rejects(lambda stage: self.v.validate_publication(str(stage.stage), str(repository)),
                     stage)

    def test_another_behaviour_token_is_rejected(self):
        stage = self.staged(behavior="coding-tuning-calibration-v0")
        decisions = self.decisions(stage)
        for check in [self.manifest, self.results, self.preflight, self.receipt]:
            self.rejects(check, stage)
        self.rejects(self.composition, stage, decisions)

    def test_another_section_identity_is_rejected(self):
        stage = self.staged(section="gf2-coding/decode")
        self.manifest(stage)
        self.rejects(self.results, stage)
        self.rejects(self.composition, stage)

    def test_counts_other_than_the_declared_ones_are_rejected(self):
        for cells in [73, 75]:
            with self.subTest(declared_cells=cells):
                stage = self.staged(declared_cells=cells)
                for check in [self.manifest, self.decisions, self.preflight, self.receipt]:
                    self.rejects(check, stage)

    def test_a_campaign_recorded_from_a_dirty_checkout_is_accepted(self):
        stage = self.staged(source_dirty=True)
        self.composition(stage)
        self.accepted(stage, "--stage", str(stage.stage), "--imported-owner-check")
        complete = json.loads((stage.stage / "complete.json").read_bytes())
        self.assertIs(complete["assembly"]["source_dirty"], True)

    def test_a_checkout_status_other_than_the_recorded_one_is_rejected(self):
        stage = self.staged()
        runtime = {**json.loads(stage.config["runtime"]), "source_dirty": True}
        stage.config["runtime"] = self.v.compact(runtime).decode()
        self.rejects(self.composition, stage)


    # A capability report may state the fixture generator of its owner.
    STATED = {"implementation": "rand::rngs::std::StdRng",
              "version": "rand 0.8.5, rand_chacha 0.3.1, rand_distr 0.4.3"}

    def test_the_stated_generator_is_accepted_and_read_from_the_capability_report(self):
        stage = self.staged(rng=self.STATED)
        self.preflight(stage)
        self.assertEqual(self.v.stated_generators(stage.reports), {"coding": self.STATED})
        self.assertEqual(self.v.stated_generators(self.staged().reports), {})

    def test_another_generator_or_an_unlocked_version_is_rejected(self):
        for change in [{"implementation": "gf2_core::rng::Lcg"},
                       {"version": "rand 0.8.5, rand_distr 0.4.3"},
                       {"version": "rand 0.8, rand_chacha 0.3.1, rand_distr 0.4.3"},
                       {"version": None}, {"revision": "x"}]:
            with self.subTest(change=change):
                self.rejects(self.preflight, self.staged(rng={**self.STATED, **change}))

    def test_a_report_of_another_mode_states_no_generator(self):
        stage = self.staged(rng=self.STATED)
        listing = stage.reports["coding-list-grid"]
        evidence = {**json.loads(listing["evidence"]), "rng": self.STATED}
        listing["evidence"] = self.v.compact(evidence).decode()
        self.rejects(self.preflight, stage)


class CommittedPins(unittest.TestCase):
    """Every file a published campaign pins by digest resolves from the
    checkout: its own bytes, or the campaign's snapshot of them."""

    RECEIPT_PINS = re.compile(r"Protocol: `[^`]+`; SHA-256 `([0-9a-f]{64})`\."
                              r"(?: Declaration: `[^`]+`; SHA-256 `([0-9a-f]{64})`\.)?")
    ARCHIVE = "-evidence-archive.tar.zst"

    @classmethod
    def setUpClass(cls):
        cls.v = load_validator()

    def test_every_published_receipt_pins_a_resolvable_protocol_and_declaration(self):
        receipts = [ROOT / path for path in tracked_files(ROOT, "gf2-*.md")
                    if self.v.RUN_ID.fullmatch(Path(path).stem)]
        published = {Path(path).parent.name.removesuffix("-session")
                     for path in tracked_files(ROOT, "*-self-check.json")
                     if Path(path).parent.name.endswith("-session")}
        self.assertEqual({receipt.stem for receipt in receipts}, published)
        for receipt in receipts:
            with self.subTest(campaign=receipt.stem):
                declaration = self.v.load_declaration(receipt.stem)
                protocol, declared = self.RECEIPT_PINS.search(receipt.read_text()).groups()
                self.v.pinned_file(ROOT, declaration, declaration["protocol"], protocol)
                if declared is not None:
                    located = self.v.locate_declaration(declaration["issue"], ROOT)
                    self.v.pinned_file(ROOT, declaration, located, declared)

    def test_every_committed_campaign_record_pins_resolvable_inputs(self):
        archives = tracked_files(ROOT, f"gf2-*{self.ARCHIVE}")
        self.assertTrue(archives)
        for archive in archives:
            campaign = Path(archive).name.removesuffix(self.ARCHIVE)
            with self.subTest(campaign=campaign):
                record = subprocess.run(
                    ["tar", "--zstd", "-xOf", str(ROOT / archive),
                     f"{self.v.EVIDENCE_ARCHIVE}/{campaign}/campaign.json"],
                    capture_output=True, check=True).stdout
                self.v.validate_pinned_inputs(json.loads(record),
                                              self.v.load_declaration(campaign))


class ContentIdentity(unittest.TestCase):
    """The protocol and the producing manifest are read under their recorded
    digests, and the recorded input digests are the campaign's pins."""

    DECLARATION = {"issue": "0badc0de"}

    @classmethod
    def setUpClass(cls):
        cls.v = load_validator()

    def checkout(self) -> Path:
        directory = tempfile.TemporaryDirectory(prefix="gf2-validator-content-identity-")
        self.addCleanup(directory.cleanup)
        located = mock.patch.object(self.v, "locate_declaration",
                                    return_value="campaign/campaign-declaration.json")
        located.start()
        self.addCleanup(located.stop)
        root = Path(directory.name).resolve()
        (root / "campaign").mkdir()
        (root / "campaign/protocol.md").write_bytes(b"frozen protocol")
        return root

    def pinned(self, root: Path, data: bytes) -> bytes:
        return self.v.pinned_file(root, self.DECLARATION, "campaign/protocol.md",
                                  hashlib.sha256(data).hexdigest())

    def test_an_uncommitted_file_with_the_recorded_digest_is_read(self):
        root = self.checkout()
        self.assertFalse((root / ".git").exists())
        self.assertEqual(self.pinned(root, b"frozen protocol"), b"frozen protocol")

    def test_a_moved_file_is_read_from_the_campaign_snapshot(self):
        root = self.checkout()
        snapshot = root / "campaign" / self.v.PRODUCING_SNAPSHOT / "campaign/protocol.md"
        snapshot.parent.mkdir(parents=True)
        snapshot.write_bytes(b"frozen protocol")
        (root / "campaign/protocol.md").write_bytes(b"later protocol")
        self.assertEqual(self.pinned(root, b"frozen protocol"), b"frozen protocol")

    def test_a_changed_frozen_input_is_refused_by_digest(self):
        root = self.checkout()
        (root / "campaign/protocol.md").write_bytes(b"changed protocol")
        with self.assertRaisesRegex(self.v.ValidationError, "recorded digest"):
            self.pinned(root, b"frozen protocol")

    def test_recorded_digests_cover_exactly_the_manifest_paths(self):
        sha = hashlib.sha256(b"input").hexdigest()
        self.assertEqual(self.v.recorded_hashes({"b.rs": sha, "a.rs": sha}, ["a.rs", "b.rs"], "x"),
                         {"a.rs": sha, "b.rs": sha})
        for recorded in [{"a.rs": sha}, {"a.rs": sha, "b.rs": sha, "c.rs": sha},
                         {"a.rs": sha, "b.rs": "0" * 40}]:
            with self.assertRaises(self.v.ValidationError):
                self.v.recorded_hashes(recorded, ["a.rs", "b.rs"], "x")

    def test_a_resumed_session_on_another_checkout_state_keeps_its_identity(self):
        def inputs(revision, dirty, behavior):
            return {"revision": revision, "tree": revision, "behavior": {"a.rs": behavior},
                    "runtime": self.v.compact({"source_dirty": dirty, "toolchain": "t"}).decode()}
        first = self.v.session_inputs(inputs("1" * 40, False, "a" * 64), "first")
        self.assertEqual(self.v.session_inputs(inputs("2" * 40, True, "a" * 64), "resumed"), first)
        self.assertNotEqual(self.v.session_inputs(inputs("1" * 40, False, "b" * 64), "changed"),
                            first)


if __name__ == "__main__":
    unittest.main()
