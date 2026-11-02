"""Behavioral contracts for configuration, build pins and resumable launchers."""

import importlib.util
import json
import shutil
from pathlib import Path
import subprocess
import sys

import pytest

from campaign import HERE, ROOT, digest, files, input_root, output_path, read, write


def load(name):
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), HERE / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


generator = load("make-family")
runner = load("run-family-campaign")
recorder = load("record-gf2-arms-build")


@pytest.mark.parametrize("declaration", sorted((HERE / "families").glob("*.json")))
def test_declarations_reproduce_their_tracked_configuration(declaration, tmp_path):
    generator.generate(declaration, tmp_path, configuration=True)
    tracked = HERE / "plans" / declaration.stem
    for name in ("addendum.json", "plan.json"):
        assert (tmp_path / name).read_bytes() == (tracked / name).read_bytes()


def test_generation_locates_shared_files_by_content_not_directory(tmp_path):
    """A checkout that keeps the campaign library elsewhere still generates the tracked plans."""
    listing = subprocess.check_output(
        ["git", "-C", str(ROOT), "ls-files", "-z", "--cached", "--others", "--exclude-standard"], text=True)
    support = ROOT / files.package_directory(ROOT, "tuning-campaign-support")
    wanted = [path for path in listing.split("\0") if path and (ROOT / path).is_file() and (
        Path(path).is_relative_to(support.relative_to(ROOT)) and "src" not in Path(path).parts
        and "tests" not in Path(path).parts
        or path.startswith(str(HERE.relative_to(ROOT)) + "/") and Path(path).parent.name in ("aff3ct-ldpc-gaps", "families", "plans")
        or Path(path).name in ("repository_files.py", "campaign_plan.py", "ccx1-bench-flock.sh"))]
    checkout = tmp_path / "checkout"
    moved = str(support.relative_to(ROOT))
    for path in wanted:
        target = checkout / path.replace(moved, "libraries/renamed-support", 1)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / path, target)
    subprocess.run(["git", "init", "-q"], cwd=checkout, check=True)
    subprocess.run(["git", "add", "-A"], cwd=checkout, check=True)
    declaration = next((checkout / HERE.relative_to(ROOT) / "families").glob("*.json"))
    out = tmp_path / "out"
    result = subprocess.run(
        [sys.executable, str(checkout / HERE.relative_to(ROOT) / "make-family.py"),
         "--configuration", str(declaration), str(out)], capture_output=True, text=True, cwd=checkout)
    assert result.returncode == 0, result.stderr
    tracked = HERE / "plans" / declaration.stem
    for name in ("addendum.json", "plan.json"):
        assert (out / name).read_bytes() == (tracked / name).read_bytes()


def test_input_root_is_explicit_and_independent_of_cwd(tmp_path, monkeypatch):
    monkeypatch.delenv("GF2_LDPC_INPUT_ROOT", raising=False)
    monkeypatch.chdir(tmp_path)
    with pytest.raises(ValueError, match="GF2_LDPC_INPUT_ROOT"):
        input_root()
    monkeypatch.setenv("GF2_LDPC_INPUT_ROOT", str(tmp_path))
    assert input_root() == tmp_path
    assert output_path(ROOT / "target/test") == ROOT / "target/test"
    with pytest.raises(ValueError, match="under.*target"):
        output_path(tmp_path)


def materialized(tmp_path, monkeypatch):
    declaration = read(HERE / "families/55c3e433-check-variants.json")
    catalogue = read(HERE / "arms.json")
    declaration["cells"] = declaration["cells"][:1]
    code = declaration["codes"][declaration["cells"][0]["code"]]
    bundle = tmp_path / "bundles" / catalogue["codes"][declaration["cells"][0]["code"]]["bundle"]
    write(bundle / "manifest.json", {**code["bundle_contract"],
                                    "h_sha256": code["decoder_code"]["h_sha256"],
                                    "llrs_sha256": code["decoder_input"]["llr_sha256"]})
    write(tmp_path / "quality/gf2-nms-f32-dvb-t2-r12.json", {})
    records = []
    for name in (declaration["cell"]["baseline"], declaration["cells"][0]["candidate"]):
        executable = tmp_path / name
        executable.write_text(name)
        record = tmp_path / f"{name}.json"
        write(record, {"build_root": str(tmp_path), "rustflags": "-C target-cpu=native",
                       "arms": {name: {"executable": executable.name,
                                       "executable_sha256": digest(executable)}}})
        records.append(record)
    path = tmp_path / "family.json"
    write(path, declaration)
    monkeypatch.setenv("GF2_LDPC_INPUT_ROOT", str(tmp_path))
    return path, records


def test_materialization_uses_each_arms_own_build_root(tmp_path, monkeypatch):
    path, records = materialized(tmp_path, monkeypatch)
    monkeypatch.setattr(generator, "output_path", lambda path: path)
    out = ROOT / "target/aff3ct-ldpc-gaps/test-projection"
    # Capture writes so this test leaves no shared output behind.
    monkeypatch.setattr(generator, "write", lambda *args: None)
    plan, addendum = generator.generate(path, out, builds=records)
    assert not runner.unidentified(plan, [read(path) for path in records])
    assert all(Path(arm["executable"]).parent == tmp_path for arm in plan["arms"].values())
    Path(next(iter(plan["arms"].values()))["executable"]).write_text("other build")
    assert runner.unidentified(plan, [read(path) for path in records])
    assert addendum["family_wise"]["ledger_path"].startswith("target/")


def test_bundle_identity_and_missing_builds_are_refused(tmp_path, monkeypatch):
    path, records = materialized(tmp_path, monkeypatch)
    out = ROOT / "target/aff3ct-ldpc-gaps/test-projection"
    with pytest.raises(ValueError, match="no pinned executable"):
        generator.generate(path, out)
    manifest = next((tmp_path / "bundles").glob("*/manifest.json"))
    value = read(manifest)
    value["llrs_sha256"] = "0" * 64
    write(manifest, value)
    with pytest.raises(ValueError, match="differs from the frozen declaration"):
        generator.generate(path, out, builds=records)


def test_qc_configuration_cannot_dispatch_a_prototype(tmp_path, monkeypatch):
    monkeypatch.setenv("GF2_LDPC_INPUT_ROOT", str(tmp_path))
    with pytest.raises(ValueError, match="configuration only"):
        recorder.record("qc-decoder")


def test_missing_aff3ct_is_refused_before_cargo(monkeypatch):
    monkeypatch.delenv("GF2_AFF3CT_ROOT", raising=False)
    with pytest.raises(ValueError, match="GF2_AFF3CT_ROOT must contain"):
        recorder.record("aff3ct-flooding-nms-f32-iterations")


def test_timed_launcher_refuses_outside_the_window(monkeypatch):
    monkeypatch.delenv("GF2_BENCH_WINDOW", raising=False)
    with pytest.raises(ValueError, match="GF2_BENCH_WINDOW is unset"):
        runner.run("absent-plan.json", [])


def test_resume_recognizes_only_a_complete_terminal_record(tmp_path):
    log = tmp_path / "events"
    assert not runner.stage_complete(log)
    for content in [b'{"event":"paused"}\n', b'{"event":"complete"}', b'partial']:
        log.write_bytes(content)
        assert not runner.stage_complete(log)
    log.write_text('{"event":"cell-complete"}\n{"event":"complete"}\n')
    assert runner.stage_complete(log)


def test_sessions_hold_the_full_host_without_duplicating_flags():
    command = runner.session_command({"wrapper": "tools/lock.sh --full-host"},
                                     "runner", Path("stage"), Path("plan"))
    assert command.count("--full-host") == 1
    assert command[-4:] == ["runner", "run", "stage", "plan"]


@pytest.mark.parametrize("dropped_width,supplied,accepted", [(0.01, None, True), (0.3, 0.02, False)])
def test_freezer_uses_dropped_cells_in_its_resolution_floor(request, tmp_path, dropped_width, supplied, accepted):
    # All fixture data is synthesized; no campaign evidence is checked in.
    pilot = ROOT / "target" / f"freezer-pilot-{tmp_path.name}"
    pilot.mkdir(parents=True)
    request.addfinalizer(lambda: shutil.rmtree(pilot, ignore_errors=True))
    write(pilot / "receipt.json", {"family_id": "fixture"})
    summary = {"schema": "zen3-benchmark-acceptance-v2", "receipt_sha256": digest(pilot / "receipt.json"),
               "label": "pilot", "verdict": "accepted", "campaign_id": "fixture-pilot",
               "family": {"family_id": "fixture", "comparisons": 2, "family_alpha": 0.05,
                          "attempt_alpha": 0.025, "corrected_alpha": 0.0125,
                          "per_comparison_confidence": 0.9875},
               "cells": [{"cell_id": name, "pairs": 12,
                          "interval": {"estimate": 1.0, "lower": 1.0 - width, "upper": 1.0 + width}}
                         for name, width in [("keep", 0.01), ("drop", dropped_width)]]}
    write(pilot / "acceptance-summary.json", summary)
    addendum = {"family": {"id": "fixture"}, "frozen": {},
                "effect": {"worthwhile_speedup": 1.1, "equivalence_margin": 1.1, "material_gap_threshold": None},
                "cells": [{"cell_id": name, "role": "exploratory"} for name in ("keep", "drop")]}
    write(tmp_path / "addendum.json", addendum)
    import tempfile
    (ROOT / "target").mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(dir=ROOT / "target") as output:
        output = Path(output)
        command = [sys.executable, str(HERE / "freeze-confirmation.py"), "--pilot-addendum",
                   str(tmp_path / "addendum.json"), "--pilot", str(pilot), "--frozen-utc", "2026-11-03T00:00:00Z",
                   "--output", str(output / "addendum.json"), "--record", str(output / "derivation.txt"),
                   "--cell", "keep", "--selection-rationale", "bounded comparisons"]
        if supplied is not None:
            (tmp_path / "floor.txt").write_text("fixture floor")
            command += ["--resolution", str(supplied), "--resolution-derivation", str(tmp_path / "floor.txt")]
        result = subprocess.run(command, capture_output=True, text=True)
        assert (result.returncode == 0) == accepted, result.stderr
        if accepted:
            frozen = read(output / "addendum.json")
            assert [cell["cell_id"] for cell in frozen["cells"]] == ["keep"]
            assert frozen["cells"][0]["role"] == "confirmatory"
        else:
            assert "below the pilot-alpha floor" in result.stderr


def test_confirmation_plans_only_retained_cells_and_preserves_the_freeze(tmp_path):
    declaration = read(HERE / "families/55c3e433-check-variants.json")
    declaration["confirmation"] = {**declaration["campaign"], "id": "fixture-confirmation"}
    path = tmp_path / "declaration.json"
    write(path, declaration)
    _, frozen = generator.generate(path, tmp_path, configuration=True)
    frozen["cells"] = [{**frozen["cells"][1], "role": "confirmatory"}]
    frozen["effect"]["measurement_resolution"] = 0.02
    write(tmp_path / "addendum.json", frozen)
    before = (tmp_path / "addendum.json").read_bytes()
    plan, _ = generator.generate(path, tmp_path, configuration=True, confirmation=True)
    assert (tmp_path / "addendum.json").read_bytes() == before
    assert [cell["cell_id"] for cell in plan["cells"]] == [frozen["cells"][0]["cell_id"]]
    assert plan["label"] == "confirmation"
    assert all(cell["pilot_pairs"] is None for cell in plan["cells"])


def literal_relative(path):
    """The shared validator's rule for a repository path in an addendum."""
    parts = Path(path).parts
    return bool(path) and not Path(path).is_absolute() and all(
        part not in ("..", ".", "/") for part in parts) and not any(c in path for c in "*?[]")


def test_frozen_addendum_names_an_absolute_pilot_by_repository_relative_path():
    import tempfile
    (ROOT / "target").mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(dir=ROOT / "target") as scratch:
        scratch = Path(scratch)
        pilot = scratch / "pilot"
        pilot.mkdir()
        write(pilot / "receipt.json", {"family_id": "fixture"})
        write(pilot / "acceptance-summary.json", {
            "schema": "zen3-benchmark-acceptance-v2", "receipt_sha256": digest(pilot / "receipt.json"),
            "label": "pilot", "verdict": "accepted", "campaign_id": "fixture-pilot",
            "family": {"family_id": "fixture", "comparisons": 1, "family_alpha": 0.05,
                       "attempt_alpha": 0.05, "corrected_alpha": 0.05, "per_comparison_confidence": 0.95},
            "cells": [{"cell_id": "keep", "pairs": 12,
                       "interval": {"estimate": 1.0, "lower": 0.99, "upper": 1.01}}]})
        write(scratch / "pilot-addendum.json", {
            "family": {"id": "fixture"}, "frozen": {},
            "effect": {"worthwhile_speedup": 1.1, "equivalence_margin": 1.1, "material_gap_threshold": None},
            "cells": [{"cell_id": "keep", "role": "exploratory"}]})
        result = subprocess.run([sys.executable, str(HERE / "freeze-confirmation.py"),
                                 "--pilot-addendum", str(scratch / "pilot-addendum.json"),
                                 "--pilot", str(pilot.resolve()), "--frozen-utc", "2026-11-03T00:00:00Z",
                                 "--output", str(scratch / "addendum.json"),
                                 "--record", str(scratch / "derivation.txt")],
                                capture_output=True, text=True)
        assert result.returncode == 0, result.stderr
        evidence = read(scratch / "addendum.json")["effect"]["resolution_evidence"]
        assert literal_relative(evidence["receipt"]), evidence["receipt"]
        assert (ROOT / evidence["receipt"]).read_bytes() == (pilot / "receipt.json").read_bytes()
        assert len(evidence["sha256"]) == 64
