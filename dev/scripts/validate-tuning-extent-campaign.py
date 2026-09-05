#!/usr/bin/env python3
"""Independent, read-only validator for the a83583e0 tuning campaign.

The validator deliberately does not import the campaign driver or trust its
summary counters.  It reconstructs the accepted protocol from sealed owner
manifests, immutable checkpoint bytes, the append-only journal, raw windows,
owner decisions, and composed profile wrappers.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import functools
import itertools
import json
import math
import re
import statistics
import subprocess
import sys
import tempfile
from collections import defaultdict
from pathlib import Path
from typing import Any, Iterable, NoReturn


CAMPAIGN_SCHEMA = "tuning-extent-campaign-v1"
MANIFEST_SCHEMA = "tuning-campaign-owner-manifest-v1"
RESULT_SCHEMA = "tuning-campaign-child-result-v1"
INPUT_SCHEMA = "tuning-campaign-owner-input-v1"
JOURNAL_SCHEMA = "tuning-campaign-journal-v1"
CHECKPOINT_SCHEMA = "tuning-campaign-checkpoint-v1"
CHECKPOINT_UNIT_SCHEMA = "tuning-campaign-checkpoint-unit-v1"
LIFECYCLE_SCHEMA = "tuning-campaign-session-v1"
FEATURES = "parallel,simd,tuning-profile,test-support"
THREADS = "RAYON_NUM_THREADS=4;dedicated_pool_width=4"
ENVIRONMENT = {
    "GF2_BENCH": "1",
    "GF2_TUNING_FRESH_CASE": "child-v2",
    "RAYON_NUM_THREADS": "4",
    "RUSTUP_TOOLCHAIN": "1.95.0",
}
COUNTS = {
    "cells": 717,
    "probes": 717,
    "timed_children": 3585,
    "accepted_results": 4302,
    "windows": 17925,
    "progress_records": 21510,
}
SESSION_BUDGET_SECONDS = 10_800
CHILD_TIMEOUT_SECONDS = 120
CHILD_KILL_GRACE_SECONDS = 5
OWNER_COUNTS = {
    "gf2-core": {
        "cells": 702,
        "probes": 702,
        "timed_children": 3510,
        "accepted_results": 4212,
        "windows": 17550,
        "progress_records": 21060,
    },
    "gf2-algebra": {
        "cells": 15,
        "probes": 15,
        "timed_children": 75,
        "accepted_results": 90,
        "windows": 375,
        "progress_records": 450,
    },
}
SHA = re.compile(r"[0-9a-f]{64}\Z")
RUN_ID = re.compile(r"gf2-a83583e0-[0-9]{8}T[0-9]{6}Z-[1-9][0-9]*\Z")
TOKEN = re.compile(r"[A-Za-z0-9][A-Za-z0-9._:/+-]*\Z")
JOURNAL_EVENTS = {
    "campaign-start", "session-start", "session-prepared", "session-recovery",
    "work-finished", "wrapper-returned", "release-unobserved", "result-validated",
    "pending-recovery", "phase-start", "phase-complete", "cell-start", "cell-complete",
    "execution-progress", "window-progress", "child-spawn", "child-diagnostic",
    "child-exit", "child-timeout", "orchestration-start", "orchestration-exit",
    "driver-diagnostic", "raw-streams", "checkpoint-accepted", "omission", "fallback",
    "owner-write", "owner-reopen", "composition", "composition-reopen", "lock-hold",
    "lock-release", "recovery", "interrupted", "complete", "failed", "paused",
    "budget-exhausted",
}

CORE_PROTOCOL = "core-tuning-campaign-v4"
ALGEBRA_PROTOCOL = "algebra-tuning-campaign-v1"
CORE_BEHAVIOR = "tuning-calibration-v4"
ALGEBRA_BEHAVIOR = "algebra-tuning-calibration-v1"
PRODUCING_INPUTS_SCHEMA = "tuning-campaign-producing-inputs-v1"
PRODUCING_MANIFEST = "dev/active/a83583e0/producing-build-inputs.json"
HOST_ADMISSION_POLICY = {
    "required_observations": [
        "affinity", "available-memory", "competing-cpu-gpu-work",
        "cpu-features", "cpu-model", "governor", "load", "os-kernel",
    ],
    "no_competing_substantial_work_required": True,
    "isolated_process_listing_is_sufficient": False,
    "held_mutex_is_sufficient": False,
}

ALGEBRA_CANDIDATES = [4096, 16384, 65536, 262144, 1048576]
ALGEBRA_DIMS = [20, 22, 24]
EXTENT_GRIDS: dict[str, list[Any]] = {
    "bit_matrix.transpose_macro_tile_blocks": [2, 4, 8, 16, 32],
    "soa_batch.parallel_chunk_len": [4096, 8192, 16384, 32768, 65536],
    "m4rm.default_table_bytes": [16384, 32768, 65536, 131072, 262144],
    "m4rm.mid_table_bytes": [32768, 65536, 131072, 262144, 524288],
    "m4rm.wide_table_bytes": [65536, 131072, 262144, 524288, 1048576],
    "m4rm.wide_max_k": [4, 5, 6, 7, 8, 9, 10],
    "m4rm.small_n_max_k": [4, 5, 6, 7, 8, 9, 10],
    "triangular.trsm_panel_rows": [8, 16, 32, 64, 128],
    "gemm.tiles": [(16, 32), (16, 64), (16, 128), (32, 32), (32, 64),
                   (32, 128), (64, 32), (64, 64), (64, 128)],
    "field_vec.dot_chunk_len": [128, 256, 512],
}
EXTENT_DEFAULTS: dict[str, Any] = {
    "bit_matrix.transpose_macro_tile_blocks": 8,
    "soa_batch.parallel_chunk_len": 16384,
    "m4rm.default_table_bytes": 65536,
    "m4rm.mid_table_bytes": 131072,
    "m4rm.wide_table_bytes": 262144,
    "m4rm.wide_max_k": 9,
    "m4rm.small_n_max_k": 8,
    "triangular.trsm_panel_rows": 64,
    "gemm.tiles": (32, 64),
    "field_vec.dot_chunk_len": 256,
    "permanent.gray_chunk_subsets": 65536,
}
EXTENT_STRATA = {
    "bit_matrix.transpose_macro_tile_blocks": 3,
    "soa_batch.parallel_chunk_len": 3,
    "m4rm.default_table_bytes": 3,
    "m4rm.mid_table_bytes": 3,
    "m4rm.wide_table_bytes": 3,
    "m4rm.wide_max_k": 9,
    "m4rm.small_n_max_k": 3,
    "triangular.trsm_panel_rows": 3,
    "gemm.tiles": 21,
    "field_vec.dot_chunk_len": 3,
}
GEMM_SITES = [
    "MatrixGemm", "MatrixGemmIntoView", "MatrixGemmAxpyIntoView",
    "MatrixGemmAxpyIntoViewDiag", "ExprGemmWithBeta", "ExprGemmTransA",
    "ExprGemmTransAWithBeta",
]


def expected_core_extent_grid() -> list[dict[str, Any]]:
    specs = [
        ("transpose", "bit_matrix.transpose_macro_tile_blocks", 3, None),
        ("soa", "soa_batch.parallel_chunk_len", 3, None),
        ("m4rm_default_bytes", "m4rm.default_table_bytes", 3, None),
        ("m4rm_mid_bytes", "m4rm.mid_table_bytes", 3, None),
        ("m4rm_wide_bytes", "m4rm.wide_table_bytes", 3, None),
        ("m4rm_wide_cap", "m4rm.wide_max_k", 9, None),
        ("m4rm_small_cap", "m4rm.small_n_max_k", 3, None),
        ("trsm", "triangular.trsm_panel_rows", 3, None),
        ("gemm_tiles", "gemm.tiles", 3,
         [re.sub(r"(?<!^)(?=[A-Z])", "_", site).lower() for site in GEMM_SITES]),
        ("dot", "field_vec.dot_chunk_len", 3, None),
    ]
    cells = []
    for field, path, shape_count, sites in specs:
        candidates = EXTENT_GRIDS[path]
        candidates = ([{"kind": "tiles", "row": row, "col": col}
                       for row, col in candidates] if path == "gemm.tiles" else
                      [{"kind": "scalar", "value": value} for value in candidates])
        for shape_index in range(shape_count):
            for site in sites or [None]:
                for candidate in candidates:
                    cells.append({"field": field, "shape_index": shape_index,
                                  "site": site, "candidate": candidate})
    require(len(cells) == 372, "internal core extent report grid count changed")
    return cells

THRESHOLD_GRIDS = {
    "bit_backend.simd_min_words": [1, 2, 4, 7, 8, 9, 16, 32, 64],
    "polynomial.karatsuba_min_degree": [4, 8, 16, 31, 32, 33, 64, 128, 256],
    "polynomial.karatsuba_max_out_len": [15, 31, 63, 127, 129, 191, 255, 383, 511],
    "polynomial.div_rem_fast_min_len": [64, 128, 256, 512, 1024, 2047, 2048, 2049, 4096],
    "polynomial.subproduct_min_len": [128, 256, 512, 1024, 2048, 4095, 4096, 4097, 8192],
    "bit_matrix.transpose_simple_max_blocks": [2, 4, 8, 15, 16, 17, 32, 64, 128],
    "soa_batch.parallel_min_len": [4096, 8192, 16384, 32767, 32768, 32769, 65536, 131072, 262144],
    "m4rm.wide_tier_min_stride_words": [2, 4, 8, 15, 16, 17, 32, 64, 128],
    "m4rm.tiled_min_stride_words": [4, 5, 6, 8, 12, 16, 24, 32, 64],
    "dense_inverse.m4ri_min_dim": [1, 2, 4, 7, 8, 9, 16, 32, 64],
    "dense_inverse.blocked_min_dim": [2, 4, 8, 15, 16, 17, 32, 64, 128],
    "triangular.trsm_blocked_min_dim": [8, 16, 32, 63, 64, 65, 96, 128, 256],
    "ple.panel_base_max_cols": [16, 32, 64, 96, 127, 128, 129, 160, 256],
    "ple.blocked_back_sub_min_dim": [16, 32, 64, 96, 127, 128, 129, 192, 256],
    "gemm.axpy_fast_path_min_volume": [64, 512, 1728, 3375, 4096, 4913, 8000, 13824, 32768],
    "polynomial.interpolate_fast_min_points": [2, 4, 8, 15, 16, 17, 32, 64, 128],
}
THRESHOLD_ENUMS = {
    "bit_backend.simd_min_words": "simd_min_words",
    "polynomial.karatsuba_min_degree": "karatsuba_min_degree",
    "polynomial.karatsuba_max_out_len": "karatsuba_max_out_len",
    "polynomial.div_rem_fast_min_len": "div_rem_fast_min_len",
    "polynomial.subproduct_min_len": "subproduct_min_len",
    "bit_matrix.transpose_simple_max_blocks": "transpose_simple_max_blocks",
    "soa_batch.parallel_min_len": "soa_parallel_min_len",
    "m4rm.wide_tier_min_stride_words": "m4rm_wide_tier_min_stride_words",
    "m4rm.tiled_min_stride_words": "m4rm_tiled_min_stride_words",
    "dense_inverse.m4ri_min_dim": "dense_inverse_m4ri_min_dim",
    "dense_inverse.blocked_min_dim": "dense_inverse_blocked_min_dim",
    "triangular.trsm_blocked_min_dim": "trsm_blocked_min_dim",
    "ple.panel_base_max_cols": "ple_panel_base_max_cols",
    "ple.blocked_back_sub_min_dim": "ple_blocked_back_sub_min_dim",
    "gemm.axpy_fast_path_min_volume": "gemm_axpy_fast_path_min_volume",
    "polynomial.interpolate_fast_min_points": "interpolate_fast_min_points",
}
THRESHOLD_DEFAULTS = {
    "bit_backend.simd_min_words": 8,
    "polynomial.karatsuba_min_degree": 32,
    "polynomial.karatsuba_max_out_len": 128,
    "polynomial.div_rem_fast_min_len": 2048,
    "polynomial.subproduct_min_len": 4096,
    "bit_matrix.transpose_simple_max_blocks": 16,
    "soa_batch.parallel_min_len": 32768,
    "m4rm.wide_tier_min_stride_words": 16,
    "m4rm.tiled_min_stride_words": 4,
    "dense_inverse.m4ri_min_dim": 8,
    "dense_inverse.blocked_min_dim": 16,
    "triangular.trsm_blocked_min_dim": 64,
    "ple.panel_base_max_cols": 128,
    "ple.blocked_back_sub_min_dim": 128,
    "gemm.axpy_fast_path_min_volume": 4096,
    "polynomial.interpolate_fast_min_points": 16,
}


def expected_core_retained_grid() -> list[dict[str, Any]]:
    enum_names = {
        "soa_batch.parallel_min_len": "soa_parallel_min_len",
        "m4rm.wide_tier_min_stride_words": "m4rm_wide_tier_min_stride_words",
        "m4rm.tiled_min_stride_words": "m4rm_tiled_min_stride_words",
        "dense_inverse.m4ri_min_dim": "dense_inverse_m4ri_min_dim",
        "dense_inverse.blocked_min_dim": "dense_inverse_blocked_min_dim",
        "triangular.trsm_blocked_min_dim": "trsm_blocked_min_dim",
        "ple.panel_base_max_cols": "ple_panel_base_max_cols",
        "ple.blocked_back_sub_min_dim": "ple_blocked_back_sub_min_dim",
        "gemm.axpy_fast_path_min_volume": "gemm_axpy_fast_path_min_volume",
    }
    result = []
    for path, grid in THRESHOLD_GRIDS.items():
        variants = (["generic_interpolation", "two_adic_interpolation"]
                    if path == "polynomial.interpolate_fast_min_points" else ["standard"])
        field = enum_names.get(path, path.split(".")[1])
        for variant in variants:
            result.append({"field": field, "variant": variant, "grid": grid,
                           "default": THRESHOLD_DEFAULTS[path]})
    require(len(result) == 17, "internal retained report grid count changed")
    return result


class ValidationError(Exception):
    pass


def fail(message: str) -> NoReturn:
    raise ValidationError(message)


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def require_keys(value: Any, keys: Iterable[str], where: str) -> dict[str, Any]:
    require(isinstance(value, dict), f"{where} must be an object")
    expected = set(keys)
    require(set(value) == expected,
            f"{where} keys differ: expected {sorted(expected)}, got {sorted(value)}")
    return value


def duplicate_checked(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            fail(f"duplicate JSON key {key!r}")
        result[key] = value
    return result


def compact(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, allow_nan=False,
                      separators=(",", ":")).encode()


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def git_output(root: Path, *args: str) -> str:
    completed = subprocess.run(["git", *args], cwd=root, check=False,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    require(completed.returncode == 0,
            f"git {' '.join(args)} failed: {completed.stderr.decode(errors='replace').strip()}")
    return completed.stdout.decode("utf-8").strip()


def require_sha(value: Any, where: str) -> str:
    require(isinstance(value, str) and SHA.fullmatch(value) is not None,
            f"{where} is not a canonical SHA-256")
    return value


def validate_affinity(value: Any, where: str) -> list[int]:
    require(isinstance(value, list) and value
            and all(type(cpu) is int and cpu >= 0 for cpu in value)
            and value == sorted(set(value)),
            f"{where} must be a nonempty increasing CPU set")
    return value


def load_json_bytes(data: bytes, where: str, canonical: bool = True) -> Any:
    try:
        text = data.decode("utf-8")
        value = json.loads(text, object_pairs_hook=duplicate_checked,
                           parse_constant=lambda x: fail(f"nonfinite JSON {x}"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail(f"{where} is not strict UTF-8 JSON: {error}")
    if canonical:
        require(compact(value) == data, f"{where} is not canonical compact JSON")
    return value


def load_json(path: Path, *, canonical: bool = True) -> Any:
    require(path.is_file() and not path.is_symlink(), f"missing or linked file {path}")
    return load_json_bytes(path.read_bytes(), str(path), canonical)


def embedded(value: Any, where: str) -> Any:
    require(isinstance(value, str), f"{where} must be embedded canonical JSON")
    return load_json_bytes(value.encode(), where)


def artifact(value: Any, where: str, stage: Path, *, canonical: bool = True) -> tuple[Path, Any]:
    require_keys(value, ["path", "sha256"], where)
    path = Path(value["path"])
    require(path.is_absolute() and path.resolve() == path and path.is_relative_to(stage),
            f"{where} path is not canonical and inside stage")
    require(path.is_file() and not path.is_symlink(), f"{where} is not a regular file")
    data = path.read_bytes()
    require(digest(data) == require_sha(value["sha256"], where + ".sha256"),
            f"{where} artifact digest mismatch")
    return path, load_json_bytes(data, str(path), canonical)


def counts(cells: int) -> dict[str, int]:
    return {"cells": cells, "probes": cells, "timed_children": 5 * cells,
            "accepted_results": 6 * cells, "windows": 25 * cells,
            "progress_records": 30 * cells}


def producing_inputs(root: Path, revision: str | None = None) -> dict[str, Any]:
    value = load_json(root / PRODUCING_MANIFEST, canonical=False)
    require_keys(value, ["schema", "behavior_sources", "lifecycle_sources",
                         "build_inputs"], "producing input manifest")
    require(value["schema"] == PRODUCING_INPUTS_SCHEMA,
            "producing input manifest schema mismatch")
    for name in ["behavior_sources", "lifecycle_sources", "build_inputs"]:
        paths = value[name]
        require(isinstance(paths, list) and paths
                and all(isinstance(path, str) for path in paths)
                and paths == sorted(set(paths)),
                f"{name} must be a nonempty sorted unique path list")
        for relative in paths:
            path = Path(relative)
            require(not path.is_absolute() and path.parts
                    and all(part not in {"", ".", ".."} for part in path.parts)
                    and not any(character in relative for character in "*?[]"),
                    f"{name} contains a nonliteral repository path")
            source = root / path
            require(source.is_file() and not source.is_symlink(),
                    f"{name} source is absent or linked: {relative}")
    require(set(value["behavior_sources"]) <= set(value["build_inputs"]),
            "behavior sources are not a subset of build inputs")
    require(set(value["lifecycle_sources"]) <= set(value["behavior_sources"]),
            "lifecycle sources are not a subset of behavior sources")
    if revision is not None:
        tracked = set(git_output(root, "ls-tree", "-r", "--name-only", revision).splitlines())
        require(set(value["build_inputs"]) <= tracked,
                "producing input manifest names files absent from the producing revision")
    return value


def source_hashes(root: Path, paths: Iterable[str]) -> dict[str, str]:
    return {relative: digest((root / relative).read_bytes()) for relative in paths}


def task_index(task: Any) -> int:
    require(isinstance(task, dict), "task must be an object")
    kind = task.get("kind")
    if kind == "probe":
        require(set(task) == {"kind"}, "probe task has unknown fields")
        return 0
    require(kind == "measure" and set(task) == {"kind", "execution"},
            "unknown task")
    execution = task["execution"]
    require(type(execution) is int and 0 <= execution < 5, "execution outside 0..4")
    return execution + 1


def rotated(base: list[str], execution: int) -> list[str]:
    shift = execution % len(base)
    result = base[shift:] + base[:shift]
    if execution % 2:
        result.reverse()
    return result


def validate_process(process: Any, expected_id: str, expected_args: list[str],
                     stage: Path) -> None:
    require_keys(process, ["id", "executable", "executable_sha256", "arguments",
                           "environment", "working_directory"], "process descriptor")
    require(process["id"] == expected_id and process["arguments"] == expected_args,
            f"{expected_id} entry point mismatch")
    require(process["environment"] == ENVIRONMENT, f"{expected_id} environment mismatch")
    executable = Path(process["executable"])
    workdir = Path(process["working_directory"])
    require(executable.is_absolute() and executable.resolve() == executable
            and executable.is_relative_to(stage) and executable.is_file(),
            f"{expected_id} executable is not a canonical staged file")
    require(workdir.is_absolute() and workdir.resolve() == workdir and workdir.is_dir(),
            f"{expected_id} workdir is invalid")
    require(digest(executable.read_bytes()) == process["executable_sha256"],
            f"{expected_id} executable digest mismatch")


def expected_core_blocks() -> list[tuple[str, str, str, list[str]]]:
    blocks = []
    for field, grid in THRESHOLD_GRIDS.items():
        variants = ["generic", "two_adic"] if field.endswith("interpolate_fast_min_points") else ["standard"]
        for variant in variants:
            for size in grid:
                blocks.append(("retained-thresholds", field, f"size-{size}-{variant}",
                               ["conservative", "asymptotic"]))
    for field, grid in EXTENT_GRIDS.items():
        shape_count = 3 if field != "m4rm.wide_max_k" else 9
        for shape in range(shape_count):
            sites = GEMM_SITES if field == "gemm.tiles" else [None]
            for site in sites:
                stratum = f"shape-{shape}" + (f"-{site}" if site else "")
                candidates = []
                for candidate in grid:
                    if field == "gemm.tiles":
                        candidates.append(f"row-{candidate[0]}-col-{candidate[1]}")
                    else:
                        candidates.append(f"candidate-{candidate}")
                blocks.append(("core-extents", field, stratum, candidates))
    for shape in range(12):
        blocks.append(("m4rm-joint", "m4rm.joint", f"shape-{shape}",
                       ["conservative", "proposed"]))
    return blocks


def expected_algebra_blocks() -> list[tuple[str, str, str, list[str]]]:
    return [("algebra-extent", "permanent.gray_chunk_subsets", f"n{n}",
             [f"q{x}" for x in ALGEBRA_CANDIDATES]) for n in ALGEBRA_DIMS]


def validate_manifest(manifest: Any, owner: str, stage: Path,
                      resolved: bool = False) -> list[dict[str, Any]]:
    require_keys(manifest, ["schema", "owner", "owner_protocol", "behavior_token",
                            "campaign_id", "phases", "candidate_blocks", "counts",
                            "processes", "ordered_units", "manifest_sha256"],
                 owner + " manifest")
    body = {key: value for key, value in manifest.items() if key != "manifest_sha256"}
    require(manifest["schema"] == MANIFEST_SCHEMA and
            digest(compact(body)) == manifest["manifest_sha256"],
            f"{owner} manifest seal mismatch")
    protocol = CORE_PROTOCOL if owner == "gf2-core" else ALGEBRA_PROTOCOL
    behavior = CORE_BEHAVIOR if owner == "gf2-core" else ALGEBRA_BEHAVIOR
    require(manifest["owner"] == owner and manifest["owner_protocol"] == protocol
            and manifest["behavior_token"] == behavior,
            f"{owner} manifest identity mismatch")
    protocol_path = Path(__file__).resolve().parents[1] / "active/a83583e0/premeasurement-protocol.md"
    protocol_sha = digest(protocol_path.read_bytes())
    require(manifest["counts"] == OWNER_COUNTS[owner], f"{owner} counts mismatch")
    phases = ["retained-thresholds", "core-extents", "m4rm-joint"] if owner == "gf2-core" else ["algebra-extent"]
    require(manifest["phases"] == phases, f"{owner} phases mismatch")
    require(len(manifest["processes"]) == 1, f"{owner} must have one process")
    if owner == "gf2-core":
        validate_process(manifest["processes"][0], "core-producer",
                         ["--fresh-tuning-process-child"], stage)
        expected_blocks = expected_core_blocks()
    else:
        validate_process(manifest["processes"][0], "algebra-producer", ["--fresh-child"], stage)
        expected_blocks = expected_algebra_blocks()
    actual_blocks = [(b.get("phase"), b.get("field"), b.get("stratum"), b.get("base_candidates"))
                     for b in manifest["candidate_blocks"]]
    require(actual_blocks == expected_blocks, f"{owner} candidate blocks differ from protocol")
    units = manifest["ordered_units"]
    expected_order: list[tuple[str, str, str, str, int]] = []
    for phase, field, stratum, base in expected_blocks:
        for candidate in base:
            expected_order.append((phase, field, stratum, candidate, 0))
        for execution in range(5):
            for candidate in rotated(base, execution):
                expected_order.append((phase, field, stratum, candidate, execution + 1))
    require(len(units) == OWNER_COUNTS[owner]["accepted_results"] == len(expected_order),
            f"{owner} unit count mismatch")
    seen = set()
    for ordinal, (unit, expected) in enumerate(zip(units, expected_order)):
        require_keys(unit, ["ordinal", "identity", "key", "process", "case",
                            "expected_progress"], f"{owner} unit {ordinal}")
        identity = require_keys(unit["identity"], ["protocol", "owner", "campaign_id", "phase",
                                                   "field", "stratum", "candidate", "task"],
                                f"{owner} identity {ordinal}")
        task = task_index(identity["task"])
        coordinate = (identity["phase"], identity["field"], identity["stratum"],
                      identity["candidate"], task)
        require(coordinate == expected and unit["ordinal"] == ordinal,
                f"{owner} acquisition order mismatch at {ordinal}")
        require(identity["owner"] == owner and identity["protocol"] == protocol
                and identity["campaign_id"] == manifest["campaign_id"],
                f"{owner} unit identity mismatch at {ordinal}")
        key = digest(compact(identity))
        require(unit["key"] == key and key not in seen, f"{owner} unit key mismatch/duplicate")
        seen.add(key)
        require(unit["expected_progress"] == (0 if task == 0 else 6),
                f"{owner} expected progress mismatch")
        case = embedded(unit["case"], f"{owner} case {ordinal}")
        require(case.get("protocol_sha256") == protocol_sha,
                f"{owner} case protocol binding mismatch")
        validate_case(case, identity, owner, stage, resolved)
    return units


def mix_seed(root: int, tag: int, key: int, role: int) -> int:
    mask = (1 << 64) - 1
    value = root ^ ((role * 0x9E3779B97F4A7C15) & mask)
    for word in (tag, key):
        value ^= word
        value = (value * 0xBF58476D1CE4E5B9) & mask
        value = ((value << 27) | (value >> 37)) & mask
        value = (value + 0x94D049BB133111EB) & mask
    return value ^ (value >> 31)


def validate_seed_inventory(seeds: Any, field: str, shape: int) -> None:
    require(isinstance(seeds, dict), "seed inventory must be object")
    if field == "permanent.gray_chunk_subsets":
        require_keys(seeds, ["schema", "root", "derivation", "field_tag", "shape_key",
                             "streams"], "algebra seed inventory")
        root, tag, key = seeds["root"], 27, shape
        expected = [{"bank": bank, "role": 0xC00 + (bank << 16),
                     "seed": mix_seed(root, tag, key, 0xC00 + (bank << 16))}
                    for bank in range(8)]
        require(seeds["schema"] == "fixture-seeds-v3"
                and seeds["derivation"] == "gf2-calibration-seed-v1"
                and root == 0x5ECC9BF800000000 and seeds["field_tag"] == tag
                and seeds["shape_key"] == key and seeds["streams"] == expected,
                "algebra deterministic seed inventory mismatch")
        return
    require_keys(seeds, ["schema", "derivation", "seed_root", "fixture_tag", "shape_key",
                         "streams"], "core extent seed inventory")
    root = seeds["seed_root"]
    m4rm = field.startswith("m4rm.")
    tag_map = {"bit_matrix.transpose_macro_tile_blocks": 16,
               "soa_batch.parallel_chunk_len": 17,
               "triangular.trsm_panel_rows": 23, "gemm.tiles": 24,
               "field_vec.dot_chunk_len": 26}
    tag = 18 if m4rm else tag_map[field]
    if m4rm:
        strides = [16, 24, 31, 32, 48, 63, 64, 96, 128]
        if field in {"m4rm.small_n_max_k"} or (field == "m4rm.joint" and shape >= 9):
            offset = shape - 9 if field == "m4rm.joint" else shape
            key = 0x10000 + [512, 768, 960][offset]
        elif field == "m4rm.mid_table_bytes":
            key = strides[shape + 3]
        elif field == "m4rm.wide_table_bytes":
            key = strides[shape + 6]
        else:
            key = strides[shape]
        roles = [("lhs", 0x300), ("rhs", 0x301)]
    else:
        key = shape
        roles = {
            "bit_matrix.transpose_macro_tile_blocks": [("matrix", 0x100)],
            "soa_batch.parallel_chunk_len": [("quadratic_lhs", 0x200),
                                               ("quadratic_rhs", 0x201),
                                               ("cubic_lhs", 0x202), ("cubic_rhs", 0x203)],
            "triangular.trsm_panel_rows": [("unit_lower", 0x600),
                                            ("unit_upper", 0x601), ("rhs", 0x602)],
            "gemm.tiles": [("lhs", 0x900), ("rhs", 0x901), ("addend", 0x902)],
            "field_vec.dot_chunk_len": [("lhs", 0xB00), ("rhs", 0xB01)],
        }[field]
    expected = [{"name": f"{name}[{bank}]", "role": role + (bank << 16),
                 "seed": mix_seed(root, tag, key, role + (bank << 16))}
                for bank in range(8) for name, role in roles]
    require(seeds["schema"] == "fixture-seeds-v3"
            and seeds["derivation"] == "gf2-calibration-seed-v1"
            and root == 0x5ECC9BF800000000 and seeds["fixture_tag"] == tag
            and seeds["shape_key"] == key and seeds["streams"] == expected,
            f"core deterministic seed inventory mismatch for {field}/{shape}")


def validate_retained_seeds(seeds: Any, field: str, size: int) -> None:
    require_keys(seeds, ["schema", "derivation", "seed_root", "field", "field_tag",
                         "size", "streams"], "retained seed inventory")
    tag = list(THRESHOLD_GRIDS).index(field)
    require(seeds["schema"] == "fixture-seeds-v2"
            and seeds["derivation"] == "gf2-calibration-seed-v1"
            and seeds["seed_root"] == 0x5ECC9BF800000000
            and seeds["field"] == THRESHOLD_ENUMS[field]
            and seeds["field_tag"] == tag and seeds["size"] == size,
            "retained seed identity mismatch")
    streams = seeds["streams"]
    require(isinstance(streams, list) and streams, "retained seed streams are empty")
    if field == "bit_backend.simd_min_words":
        expected_streams = ([(f"dst[{bank}]", 0xD0000000 + bank) for bank in range(8)]
                            + [(f"src[{bank}]", 0xA0000000 + bank) for bank in range(8)])
    elif field in {"polynomial.karatsuba_min_degree",
                   "polynomial.karatsuba_max_out_len"}:
        expected_streams = [("lhs", 0xA), ("rhs", 0xB)]
    elif field == "polynomial.div_rem_fast_min_len":
        expected_streams = [("dividend", 0xD), ("divisor", 0xE)]
    elif field == "polynomial.subproduct_min_len":
        expected_streams = [("polynomial", 0xC), ("points", 0xF)]
    else:
        roles = {
            "bit_matrix.transpose_simple_max_blocks": [("matrix", 0x100)],
            "soa_batch.parallel_min_len": [("quadratic_lhs", 0x200),
                                             ("quadratic_rhs", 0x201),
                                             ("cubic_lhs", 0x202), ("cubic_rhs", 0x203)],
            "m4rm.wide_tier_min_stride_words": [("lhs", 0x300), ("rhs", 0x301)],
            "m4rm.tiled_min_stride_words": [("lhs", 0x310), ("rhs", 0x311)],
            "dense_inverse.m4ri_min_dim": [("unit_lower", 0x400),
                                             ("unit_upper", 0x401)],
            "dense_inverse.blocked_min_dim": [("unit_lower", 0x500),
                                                ("unit_upper", 0x501)],
            "triangular.trsm_blocked_min_dim": [("unit_lower", 0x600),
                                                  ("unit_upper", 0x601), ("rhs", 0x602)],
            "ple.panel_base_max_cols": [("unit_lower", 0x700), ("unit_upper", 0x701)],
            "ple.blocked_back_sub_min_dim": [("designated_nonzero", 0x800),
                                               ("non_designated", 0x801),
                                               ("row_mix", 0x802)],
            "gemm.axpy_fast_path_min_volume": [("lhs", 0x900), ("rhs", 0x901)],
            "polynomial.interpolate_fast_min_points": [("coefficients", 0xA00),
                                                         ("point_offset", 0xA01)],
        }[field]
        expected_streams = [(f"{name}[{bank}]", role + (bank << 16))
                            for bank in range(8) for name, role in roles]
    identities = []
    for stream in streams:
        require_keys(stream, ["name", "role", "seed"], "retained seed stream")
        role = stream["role"]
        require(isinstance(stream["name"], str) and stream["name"]
                and type(role) is int
                and stream["seed"] == mix_seed(seeds["seed_root"], tag, size, role),
                "retained deterministic seed mismatch")
        identities.append((stream["name"], role))
    require(identities == expected_streams,
            "retained seed names/roles differ from the fixed inventory")



MAX_U64 = (1 << 64) - 1
MAX_CALLS = 1 << 32


def sorted_json(value: Any) -> Any:
    if isinstance(value, dict):
        return {key: sorted_json(value[key]) for key in sorted(value)}
    if isinstance(value, list):
        return [sorted_json(item) for item in value]
    return value


def flatten_selectors(selectors: Any) -> list[dict[str, Any]]:
    return [{"family": family, "field": field, "value": value}
            for family, fields in sorted(selectors.items())
            for field, value in sorted(fields.items())]


def threshold_forcing(case: Any) -> list[dict[str, Any]]:
    field = case["identity"]["field"]
    spec = case["kind"]["spec"]
    size, conservative = spec["size"], spec["arm"] == "conservative"
    family, leaf = field.split(".")
    value = size + int(conservative)
    if field == "bit_backend.simd_min_words":
        value = size
    elif field == "polynomial.karatsuba_min_degree":
        value = MAX_U64 if conservative else size
    elif field in {"polynomial.karatsuba_max_out_len",
                   "bit_matrix.transpose_simple_max_blocks", "ple.panel_base_max_cols"}:
        value = size - int(not conservative)
    changes = [(family, leaf, value)]
    companions = {
        "soa_batch.parallel_min_len": ("soa_batch", "parallel_chunk_len", 16384),
        "m4rm.wide_tier_min_stride_words": ("m4rm", "tiled_min_stride_words", MAX_U64),
        "triangular.trsm_blocked_min_dim": ("triangular", "trsm_panel_rows", 64),
        "ple.panel_base_max_cols": ("ple", "panel_byte_lane_max_cols", 256),
    }
    if field in companions:
        changes.append(companions[field])
    return [{"family": a, "field": b, "value": c} for a, b, c in changes]


def expected_forced_selectors(case: Any) -> Any:
    selectors = copy.deepcopy(conservative_core_selectors())
    field = case["identity"]["field"]
    if case["kind"]["experiment"] == "threshold":
        changes = threshold_forcing(case)
    else:
        candidate = case["kind"]["cell"]["candidate"]
        if field == "gemm.tiles":
            leaves = {"gemm.row_tile": candidate["row"], "gemm.col_tile": candidate["col"],
                      "gemm.axpy_fast_path_min_volume": MAX_U64}
        elif field == "m4rm.joint":
            values = candidate["values"]
            require_keys(values, ["default_table_bytes", "mid_table_bytes", "wide_table_bytes",
                                  "wide_max_k", "small_n_max_k"], "M4RM vector")
            leaves = {f"m4rm.{key}": value for key, value in values.items()}
        else:
            leaves = {field: candidate["value"]}
        controls = {"bit_matrix.transpose_macro_tile_blocks": "bit_matrix.transpose_simple_max_blocks",
                    "soa_batch.parallel_chunk_len": "soa_batch.parallel_min_len",
                    "triangular.trsm_panel_rows": "triangular.trsm_blocked_min_dim"}
        if field in controls:
            leaves[controls[field]] = 0
        if field.startswith("m4rm."):
            leaves.update({"m4rm.wide_tier_min_stride_words": 16,
                           "m4rm.tiled_min_stride_words": MAX_U64})
        changes = [{"family": key.split(".")[0], "field": key.split(".")[1], "value": value}
                   for key, value in leaves.items()]
    for change in changes:
        selectors[change["family"]][change["field"]] = change["value"]
    return selectors


def extent_dimensions(field: str, shape: int) -> list[int]:
    if field.startswith("m4rm."):
        strides = [16, 24, 31, 32, 48, 63, 64, 96, 128]
        if field == "m4rm.small_n_max_k" or field == "m4rm.joint" and shape >= 9:
            return [64, 2048, [512, 768, 960][shape % 9]]
        offset = {"m4rm.mid_table_bytes": 3, "m4rm.wide_table_bytes": 6}.get(field, 0)
        return [64, 512, 64 * strides[shape + offset]]
    if field == "gemm.tiles":
        return [[65, 64, 129], [129, 128, 257], [193, 192, 385]][shape]
    dimension = {"bit_matrix.transpose_macro_tile_blocks": [2048, 4096, 8192],
                 "soa_batch.parallel_chunk_len": [65536, 131072, 262144],
                 "triangular.trsm_panel_rows": [129, 193, 257],
                 "field_vec.dot_chunk_len": [4097, 16385, 65537]}[field][shape]
    return [dimension] * 3


def m4rm_schedule(selectors: Any, k: int, n: int) -> tuple[Any, Any]:
    stride = (n + 63) // 64
    wide = stride >= selectors["wide_tier_min_stride_words"]
    band = ("wide" if stride >= 64 else "mid" if stride >= 32 else "default") if wide else "small_n"
    budget = selectors[band + "_table_bytes"] if wide else None
    cap = selectors["wide_max_k" if wide else "small_n_max_k"]
    if k == 0 or n == 0:
        panel = 0
    elif k == 1:
        panel = 1
    elif wide:
        entries = budget // (stride * 8)
        panel = min(cap, k, 63, max(1, entries.bit_length() - 1))
    else:
        panel = min(k, cap, max(2, math.floor(0.8 * math.log2(min(k, n)) + 0.5)))
    admitted = stride >= selectors["tiled_min_stride_words"]
    return ({"route": "m4rm", "tier": "wide" if wide else "small_n",
             "panel_width": panel, "c_update": "row_wise"},
            {"band": band, "table_bytes": budget, "panel_width_cap": cap,
             "tiled_stride_admitted": admitted})


def threshold_shape(field: str, size: int) -> str:
    formats = {"bit_backend.simd_min_words": f"8 aligned banks x {size} words",
               "polynomial.karatsuba_min_degree": f"degree {size} x degree {size}",
               "polynomial.karatsuba_max_out_len": f"product_len={size}",
               "polynomial.div_rem_fast_min_len": f"dividend_len={2*size} divisor_len={size}",
               "polynomial.subproduct_min_len": f"coefficients={size} points={size}",
               "bit_matrix.transpose_simple_max_blocks": f"{64*size}x{64*size}",
               "soa_batch.parallel_min_len": f"quadratic+cubic length={size}",
               "polynomial.interpolate_fast_min_points": f"{size} distinct points"}
    if field in formats:
        return formats[field]
    if field.startswith("m4rm."):
        return f"64x512 * 512x{64*size}"
    if field == "gemm.axpy_fast_path_min_volume":
        size = round(size ** (1 / 3))
        return f"{size}x{size} * {size}x{size}"
    if field == "triangular.trsm_blocked_min_dim":
        return f"{size}x{size} * {size}x{size}"
    return f"{size}x{size}"


def validate_threshold_route(case: Any, outcome: Any) -> None:
    field = case["identity"]["field"]
    index = list(THRESHOLD_GRIDS).index(field)
    arm = case["kind"]["spec"]["arm"] == "asymptotic"
    size = case["kind"]["spec"]["size"]
    conservative = ["scalar", "schoolbook", "karatsuba", "div_rem", "eval_batch", "simple",
                    "sequential", "small_n", "row_wise", "scalar", "scalar_ple", "recursive",
                    "panel_base", "scalar", "per_cell", "barycentric"]
    asymptotic = ["simd", "karatsuba", "mul_ntt", "div_rem_fast", "subproduct_auto", "macro_tiled",
                  "parallel", "wide", "register_tiled", "m4ri", "blocked_panelized", "blocked",
                  "sub_panel_recursion", "blocked", "whole_gemm", "subproduct_tree"]
    route = (asymptotic if arm else conservative)[index]
    effective, capability = "not_required", "not_required"
    if index == 0:
        effective = "baked_selector_direct_backend"
        capability = outcome["capability_observation"] if arm else "scalar_backend"
        require(not arm or capability == "simd_backend=avx2",
                "unknown concrete SIMD capability")
    elif index in {1, 2, 3, 4}:
        effective = "production_dispatch"
    elif index == 6:
        effective = "parallel_chunk=16384" if arm else "sequential_no_chunk"
        capability = "dedicated_pool_width=4"
    elif index == 7:
        schedule, _ = m4rm_schedule(expected_forced_selectors(case)["m4rm"], 512, 64 * size)
        effective = f"panel_width={schedule['panel_width']}"
    elif index == 8:
        effective, capability = ("RegisterTiled" if arm else "RowWise"), "simd_tile8xn=resolved"
    elif index == 11:
        effective = "panel_rows=Some(64)" if arm else "panel_rows=None"
        capability = "fp251_whole_gemm_available=true"
    elif index == 12:
        effective = f"max_panel_cols={size-int(arm)}"
        capability = "carrier_lane=byte panel_byte_lane_max_cols=256"
    elif index == 13:
        effective = f"rank={size//2} free_cols={size-size//2}"
    elif index == 14:
        effective, capability = ("WholeGemm" if arm else "PerCell"), "fp251_whole_gemm_available=true"
    elif index == 15:
        effective = case["kind"]["spec"]["variant"]
    require(outcome["requested_route"] == outcome["observed_route"] == route
            and outcome["effective_observation"] == effective
            and outcome["capability_observation"] == capability,
            "threshold route/effectiveness/capability differs from forced production contract")


def u64(value: int) -> bytes:
    return value.to_bytes(8, 'little')


def tuple_hash(domain: bytes, parts: Iterable[str]) -> str:
    h = hashlib.sha256(domain)
    for part in parts:
        h.update(u64(len(part)))
        h.update(part.encode())
    return h.hexdigest()


def draws(seed: int) -> Iterable[int]:
    while True:
        seed = (seed * 6364136223846793005 + 1442695040888963407) & MAX_U64
        yield seed


def values_hash(domain: bytes, dimensions: Iterable[int], values: Iterable[int]) -> str:
    h = hashlib.sha256(domain)
    for dimension in dimensions:
        h.update(u64(dimension))
    for value in values:
        h.update(u64(value))
    return h.hexdigest()


def random_values(seed: int, count: int, modulus: int, offset: int = 0) -> list[int]:
    generator = draws(seed)
    return [next(generator) % modulus + offset for _ in range(count)]


def bit_words(rows: int, cols: int, seed: int) -> list[int]:
    generator = draws(seed)
    stride = (cols + 63) // 64
    return [next(generator) & ((1 << (cols % 64)) - 1 if cols % 64 and col == stride - 1 else MAX_U64)
            for _ in range(rows) for col in range(stride)]


def lu_product(n: int, lower_seed: int, upper_seed: int, modulus: int) -> list[int]:
    lower_rng, upper_rng = draws(lower_seed), draws(upper_seed)
    lower = [[int(i == j) for j in range(n)] for i in range(n)]
    upper = [[int(i == j) for j in range(n)] for i in range(n)]
    for i in range(n):
        for j in range(i):
            lower[i][j] = next(lower_rng) % modulus
    for i in range(n):
        for j in range(i + 1, n):
            upper[i][j] = next(upper_rng) % modulus
    columns = list(zip(*upper))
    return [sum(a*b for a, b in zip(row[:min(i,j)+1], col[:min(i,j)+1])) % modulus
            for i, row in enumerate(lower) for j, col in enumerate(columns)]


@functools.lru_cache(maxsize=None)
def operand_identity(owner: str, field: str, shape: int, retained: bool = False) -> str:
    """Reconstruct logical operand bytes from fixed seeds; never execute a staged tool."""
    root = 0x5ECC9BF800000000
    if owner == 'gf2-algebra':
        n = ALGEBRA_DIMS[shape]
        parts = []
        for bank in range(8):
            data = bytes(random_values(mix_seed(root, 27, shape, 0xC00 + (bank << 16)), n*n, 3))
            parts.append(digest(b'gf2-a83583e0-permanent-fixture-v1\0' +
                                u64(n) + u64(bank) + u64(0xC00) + u64(n*n) + data))
        return tuple_hash(b'gf2-a83583e0-operands-v1\0', parts)
    if retained:
        tag, key = list(THRESHOLD_GRIDS).index(field), shape
        m = k = n = shape
        if tag == 5:
            m = k = n = shape * 64
        elif tag in {7, 8}:
            m, k, n = 64, 512, shape * 64
        elif tag == 14:
            m = k = n = round(shape ** (1/3))
    else:
        m, k, n = extent_dimensions(field, shape)
        tag = {"bit_matrix.transpose_macro_tile_blocks":16, "soa_batch.parallel_chunk_len":17,
               "triangular.trsm_panel_rows":23, "gemm.tiles":24, "field_vec.dot_chunk_len":26}.get(field,18)
        key = n//64 if field.startswith('m4rm.') and k == 512 else 0x10000+n if field.startswith('m4rm.') else shape
    seed = lambda role, bank=0: mix_seed(root, tag, key, role + (bank << 16))
    poly = lambda count, role: values_hash(b'gf2-calibration-fp65537-polynomial-v1', [count], random_values(seed(role),count,65536,1))
    if retained and tag <= 4:
        if tag == 0:
            values = [word for bank in range(8) for role in [0xD0000000,0xA0000000]
                      for word in random_values(mix_seed(root,tag,key,role+bank),shape,1<<64)]
            return values_hash(b'gf2-calibration-direct-operands-v3',[],values)
        if tag in {1,2}:
            count = shape+1 if tag == 1 else (shape+1)//2
            parts = [poly(count,0xA),poly(count,0xB)]
        elif tag == 3:
            parts = [poly(2*shape,0xD),poly(shape,0xE)]
        else:
            offset = next(draws(seed(0xF))) % 65536
            points = [(offset+i*1000003)%65536+1 for i in range(shape)]
            parts = [poly(shape,0xC), values_hash(b'direct-points',[shape],points)]
        return digest(b'gf2-calibration-direct-operands-v3'+''.join(parts).encode())
    parts=[]
    for bank in range(8):
        bit = lambda rows,cols,role: values_hash(b'gf2-calibration-bit-matrix-v1',[rows,cols],bit_words(rows,cols,seed(role,bank)))
        prime_domain = b'gf2-calibration-fp251-matrix-v1' if retained else b'gf2-extent-prime-matrix-v1'
        prime = lambda rows,cols,values,p=251: values_hash(prime_domain,([rows,cols] if retained else [p,rows,cols]),values)
        if field.startswith('bit_matrix.'):
            parts.append(bit(m,n,0x100))
        elif field.startswith('soa_batch.'):
            for degree,role in [(2,0x200),(2,0x201),(3,0x202),(3,0x203)]:
                parts.append(values_hash(b'gf2-calibration-fp65537-soa-v1',[degree,m],random_values(seed(role,bank),degree*m,65537)))
        elif field.startswith('m4rm.'):
            role=0x310 if retained and tag == 8 else 0x300
            parts.extend([bit(m,k,role),bit(k,n,role+1)])
        elif field.startswith('triangular.') or retained and tag in {10,12}:
            role = {10:0x500,12:0x700}.get(tag,0x600) if retained else 0x600
            parts.append(prime(m,m,lu_product(m,seed(role,bank),seed(role+1,bank),251)))
            if field.startswith('triangular.'):
                parts.append(prime(m,n,random_values(seed(0x602,bank),m*n,251)))
        elif field.startswith('gemm.'):
            for rows,cols,role in [(m,k,0x900),(k,n,0x901)]+([] if retained else [(m,n,0x902)]):
                parts.append(prime(rows,cols,random_values(seed(role,bank),rows*cols,251 if retained else 65537),251 if retained else 65537))
        elif field == 'field_vec.dot_chunk_len':
            parts.extend(values_hash(b'gf2-extent-runtime-gf256-vector-v1',[m],random_values(seed(role,bank),m,256)) for role in [0xB00,0xB01])
        elif retained and tag == 9:
            values=lu_product(m,seed(0x400,bank),seed(0x401,bank),2)
            words=[sum(values[row*m+col] << (col%64) for col in range(start,min(m,start+64))) for row in range(m) for start in range(0,m,64)]
            parts.append(values_hash(b'gf2-calibration-bit-matrix-v1',[m,m],words))
        elif retained and tag == 13:
            rank=m//2
            embedded_matrix=[[0]*m for _ in range(m)]
            designated,ordinary,mixer=draws(seed(0x800,bank)),draws(seed(0x801,bank)),draws(seed(0x802,bank))
            for row in range(rank):
                embedded_matrix[row][2*row]=next(designated)%250+1
            for row in range(rank):
                for col in range(m):
                    if col%2 or col>=2*rank:
                        embedded_matrix[row][col]=next(ordinary)%251
            values=[]
            for row in range(m):
                coefficients=[next(mixer)%251 for _ in range(row)]+[1]
                values.extend(sum(coefficients[i]*embedded_matrix[i][col] for i in range(min(row+1,rank)))%251 for col in range(m))
            parts.append(prime(m,m,values))
        elif retained and tag == 15:
            coefficients=random_values(seed(0xA00,bank),m,65536,1)
            offset=next(draws(seed(0xA01,bank)))%65536
            values=[]
            for index in range(m):
                x=(offset+index*1000003)%65536+1
                y=0
                for coefficient in reversed(coefficients):
                    y=(y*x+coefficient)%65537
                values.extend([x,y])
            parts.append(values_hash(b'gf2-calibration-interpolation-points-v1',[m],values))
        else:
            fail('missing independent operand fixture')
    if not retained:
        domain=b'gf2-extent-operands-bank-role-tuple-v1'
    else:
        domain={5:b'unary-bit',6:b'soa',7:b'binary-bit',8:b'binary-bit',9:b'unary-bit',10:b'unary-fp251',11:b'binary-fp251',12:b'unary-fp251',13:b'unary-fp251',14:b'binary-fp251',15:b'interpolation'}[tag]
        domain=b'gf2-calibration-'+domain+b'-banks-v1'
    return tuple_hash(domain,parts)


def semantic_pair_identity(case: Any, payload: Any) -> tuple[Any, str, str]:
    identity=case['identity']
    field=identity['field']
    if identity['owner']=='gf2-algebra':
        shape=case['shape_index']
        operands=payload['semantics']['operands_sha256']
        result=payload['semantics']['serial_results_sha256']
        coordinate=('algebra',shape)
    elif case['kind']['experiment']=='threshold':
        shape=case['kind']['spec']['size']
        operands=payload['report']['operand_digest']
        result=payload['report']['outcome']['result_digest']
        coordinate=('threshold',field,shape,case['kind']['spec']['variant'])
    else:
        shape=case['kind']['cell']['shape_index']
        operands=payload['operands_sha256']
        result=payload['oracle_sha256']
        coordinate=('extent',field,shape,case['kind']['cell']['site'])
        if field.startswith('m4rm.'):
            coordinate=('m4rm',*extent_dimensions(field,shape))
    retained=identity['owner']=='gf2-core' and case['kind']['experiment']=='threshold'
    require(operands==operand_identity(identity['owner'],field,shape,retained),
            'operand digest differs from independently reconstructed logical bytes')
    require_sha(result,'semantic result')
    require(result!='0'*64, 'zero semantic identity is not a result witness')
    return coordinate,operands,result


def envelope_content(profile_id: str, sections: Any) -> str:
    ordered={key:{'schema_version':section['schema_version'],
                  'measurement':section['measurement'],
                  'selectors':sorted_json(section['selectors'])}
             for key,section in sorted(sections.items())}
    return digest(compact({'profile_format_version':2,'profile_id':profile_id,'sections':ordered}))


def forced_hashes(owner: str, selectors: Any) -> dict[str,str]:
    profile_id='calibration-forced-arm' if owner=='gf2-core' else 'a835-algebra-forced'
    section_id='gf2-core/selectors' if owner=='gf2-core' else 'gf2-algebra/permanent'
    section={'schema_version':1,'measurement':{'kind':'inherited'},'selectors':sorted_json(selectors)}
    content=envelope_content(profile_id,{section_id:section})
    assembly={'kind':'assembled','assembled_at':'1970-01-01T00:00:00Z','source_revision':'0'*40,
              'source_dirty':False,'tool':f'crates/{owner}/benches/tuning_calibration.rs',
              'tool_sha256':'0'*64,'content_sha256':content}
    document=compact({'profile_format_version':2,'profile_id':profile_id,'assembly':assembly,'sections':{section_id:section}})+b'\n'
    return {'section':digest(compact(sorted_json(section))),'content':content,'envelope':digest(document)}


def expected_published_selectors(decisions: Any) -> Any:
    core=decisions['core_decisions']
    selectors=copy.deepcopy(conservative_core_selectors())
    inverse={enum:path for path,enum in THRESHOLD_ENUMS.items()}
    for decision in core['retained_thresholds']:
        if decision['variant']=='standard':
            family,leaf=inverse[decision['field']].split('.')
            selectors[family][leaf]=decision['selection']['value']
    fields={'transpose':'bit_matrix.transpose_macro_tile_blocks', 'soa':'soa_batch.parallel_chunk_len',
            'trsm':'triangular.trsm_panel_rows','dot':'field_vec.dot_chunk_len'}
    for entry in core['extents']:
        if entry['field'] in fields:
            family,leaf=fields[entry['field']].split('.')
            selectors[family][leaf]=entry['decision']['selected']['value']
    selectors['gemm']['row_tile'],selectors['gemm']['col_tile']=core['gemm']['selected']
    selectors['m4rm'].update(core['joint_m4rm']['selected'])
    for path in core['omitted']:
        family,leaf=path.split('.')
        del selectors[family][leaf]
    return selectors


def validate_envelope_hashes(envelope: Any) -> None:
    require_keys(envelope,['profile_format_version','profile_id','assembly','sections'],'format-2 envelope')
    assembly=require_keys(envelope['assembly'],['kind','assembled_at','source_revision','source_dirty','tool','tool_sha256','content_sha256'],'assembly')
    for section in envelope['sections'].values():
        require_keys(section,['schema_version','measurement','selectors'],'section wrapper')
        require(type(section['schema_version']) is int and section['schema_version']==1,'section schema mismatch')
        measurement=section['measurement']
        require_keys(measurement,['kind','measured_at','source_revision','source_dirty','harness','harness_schema','binary_sha256','toolchain','host','cpu_model','cpu_features','os_kernel','governor','receipt'],'calibrated measurement')
        require(measurement['kind']=='calibrated','published owner measurement is not calibrated')
    require(assembly['kind']=='assembled' and assembly['content_sha256']==envelope_content(envelope['profile_id'],envelope['sections']),
            'envelope canonical content digest mismatch')


def validate_published_provenance(envelope: Any, owner: str, request: Any, config: Any) -> None:
    runtime=embedded(config['runtime'],'observed runtime')
    section_id='gf2-core/selectors' if owner=='gf2-core' else 'gf2-algebra/permanent'
    process='core-producer' if owner=='gf2-core' else 'algebra-producer'
    behavior=CORE_BEHAVIOR if owner=='gf2-core' else ALGEBRA_BEHAVIOR
    measurement={'kind':'calibrated','measured_at':request['measurement']['observed_utc'],
                 'source_revision':config['identity']['source_revision'],'source_dirty':False,
                 'harness':f'crates/{owner}/benches/tuning_calibration.rs','harness_schema':behavior,
                 'binary_sha256':config['identity']['executable_sha256'][process],
                 'toolchain':runtime['toolchain'],'host':config['identity']['host_identity'],
                 **{key:runtime[key] for key in ['cpu_model','cpu_features','os_kernel','governor','receipt']}}
    require(envelope['sections'][section_id]['measurement']==measurement,'owner measurement provenance differs from observed request')
    expected_assembly={'kind':'assembled','assembled_at':request['assembly']['observed_utc'],
                       'source_revision':config['identity']['source_revision'],'source_dirty':False,
                       'tool':measurement['harness'],'tool_sha256':measurement['binary_sha256'],
                       'content_sha256':envelope_content(envelope['profile_id'],envelope['sections'])}
    require(envelope['assembly']==expected_assembly,'owner assembly provenance differs from observed request')


def validate_case(case: Any, identity: dict[str, Any], owner: str, stage: Path,
                  resolved: bool) -> None:
    require(case.get("identity") == identity, "case repeats a different identity")
    require(case.get("protocol_sha256") and SHA.fullmatch(case["protocol_sha256"]),
            "case protocol digest invalid")
    if owner == "gf2-algebra":
        require_keys(case, ["schema", "identity", "protocol_sha256", "field", "shape_index",
                            "dimension", "candidate", "pool_threads", "seeds", "channels"],
                     "algebra case")
        shape = case["shape_index"]
        require(case["schema"] == ALGEBRA_PROTOCOL and 0 <= shape < 3
                and case["dimension"] == ALGEBRA_DIMS[shape]
                and case["candidate"] in ALGEBRA_CANDIDATES and case["pool_threads"] == 4,
                "algebra case grid mismatch")
        validate_seed_inventory(case["seeds"], identity["field"], shape)
    else:
        require_keys(case, ["schema", "protocol", "identity", "protocol_sha256", "channels",
                            "kind", "forced_values", "seeds"], "core case")
        require(case.get("schema") == CORE_PROTOCOL and case.get("protocol") ==
                {"executions": 5, "repetitions": 5, "target_ms": 250},
                "core case protocol constants mismatch")
        kind = case.get("kind")
        require(isinstance(kind, dict), "core case kind missing")
        experiment = kind.get("experiment")
        require(experiment in ({"threshold", "extent"} if resolved else
                               {"threshold", "extent", "reserved_m4rm"}),
                "core case experiment mismatch")
        seeds = case.get("seeds", {})
        require_keys(seeds, ["inventory", "seeds"], "core case seeds")
        inventory = seeds.get("seeds") if isinstance(seeds, dict) else None
        if experiment == "threshold":
            require_keys(kind, ["experiment", "spec"], "threshold case kind")
            spec = require_keys(kind["spec"], ["field", "variant", "size", "arm", "task"],
                                "threshold case specification")
            size = spec["size"]
            stratum_variant = identity["stratum"].removeprefix(f"size-{size}-")
            expected_variant = {"standard": "standard", "generic": "generic_interpolation",
                                "two_adic": "two_adic_interpolation"}.get(stratum_variant)
            require(identity["field"] in THRESHOLD_GRIDS
                    and size in THRESHOLD_GRIDS[identity["field"]]
                    and spec["field"] == THRESHOLD_ENUMS[identity["field"]]
                    and spec["variant"] == expected_variant
                    and spec["arm"] == identity["candidate"]
                    and spec["task"] == identity["task"],
                    "threshold case coordinate mismatch")
            require(seeds.get("inventory") == "retained", "threshold seed kind mismatch")
            validate_retained_seeds(inventory, identity["field"], size)
        elif experiment in {"extent", "reserved_m4rm"}:
            require(seeds.get("inventory") == "extent", "extent seed kind mismatch")
            require_keys(kind, ["experiment", "cell"], "extent case kind")
            cell = require_keys(kind["cell"], ["field", "shape_index", "site", "candidate"],
                                "extent cell")
            field_names = dict(zip(EXTENT_GRIDS,
                                   ["transpose", "soa", "m4rm_default_bytes",
                                    "m4rm_mid_bytes", "m4rm_wide_bytes", "m4rm_wide_cap",
                                    "m4rm_small_cap", "trsm", "gemm_tiles", "dot"]))
            field_names["m4rm.joint"] = "m4rm_joint"
            require(cell["field"] == field_names[identity["field"]],
                    "extent cell field mismatch")
            match = re.fullmatch(r"shape-([0-9]+)(?:-(.+))?", identity["stratum"])
            require(match is not None and cell["shape_index"] == int(match.group(1)),
                    "extent cell shape mismatch")
            site = match.group(2)
            expected_site = (re.sub(r"(?<!^)(?=[A-Z])", "_", site).lower()
                             if site is not None else None)
            require(cell["site"] == expected_site, "extent GEMM site mismatch")
            candidate = cell["candidate"]
            if identity["field"] == "gemm.tiles":
                tiles = re.fullmatch(r"row-([0-9]+)-col-([0-9]+)", identity["candidate"])
                require(tiles is not None and candidate == {
                            "kind": "tiles", "row": int(tiles.group(1)),
                            "col": int(tiles.group(2))}, "extent tile candidate mismatch")
            elif identity["field"] == "m4rm.joint":
                require_keys(candidate, ["kind", "proposed", "values"],
                             "joint M4RM candidate")
                require(candidate["kind"] == "vector"
                        and candidate["proposed"] == (identity["candidate"] == "proposed"),
                        "joint M4RM candidate label mismatch")
            else:
                value = identity["candidate"].removeprefix("candidate-")
                require(value.isdigit() and candidate == {"kind": "scalar", "value": int(value)},
                        "extent scalar candidate mismatch")
            validate_seed_inventory(inventory, identity["field"], cell["shape_index"])
    if owner == "gf2-core":
        require(case["forced_values"] == flatten_selectors(expected_forced_selectors(case)),
                "case forced selectors differ from exact candidate and controls")
    channels = case.get("channels")
    if True:
        if owner == "gf2-core":
            require(channels == {"stage": str(stage),
                                 "execution_log": str(stage / "execution.log"),
                                 "checkpoints": str(stage / "checkpoints")},
                    "core case channels are redirected or incomplete")
        else:
            require(channels == {"journal_schema": JOURNAL_SCHEMA,
                                 "checkpoint_schema": CHECKPOINT_SCHEMA,
                                 "raw_sample_schema": "raw-timing-samples-v3",
                                 "execution_log": str(stage / "execution.log"),
                                 "checkpoints": str(stage / "checkpoints")},
                    "algebra case channels are redirected or incomplete")


def validate_sample(sample: Any, execution: int, repetition: int, calls: int | None) -> int:
    require_keys(sample, ["execution", "repetition", "calls", "elapsed_ns"], "timing sample")
    require(sample["execution"] == execution and sample["repetition"] == repetition,
            "timing coordinate mismatch")
    require(type(sample["calls"]) is int and 0 < sample["calls"] <= MAX_CALLS
            and type(sample["elapsed_ns"]) is int and sample["elapsed_ns"] > 0,
            "nonpositive timing sample")
    require(calls is None or calls == sample["calls"], "call count changed within execution")
    return sample["calls"]


def validate_result_payload(result: Any, case: Any, owner: str) -> None:
    payload = embedded(result["payload"], "child payload")
    if owner == "gf2-algebra":
        require_keys(payload, ["schema", "field", "shape_index", "dimension",
                               "requested_chunk_subsets", "observed_requested_chunk_subsets",
                               "effective_partition", "profile", "capability", "semantics"],
                     "algebra payload")
        profile = require_keys(payload["profile"], ["profile_id", "owner_section_id",
                                                     "section_schema_version", "harness_schema",
                                                     "resolution", "active_gray_chunk_subsets",
                                                     "envelope_sha256", "content_sha256",
                                                     "wrapper_sha256"], "algebra profile evidence")
        capability = require_keys(payload["capability"], ["required_features", "pool_threads",
                                                          "parallel_route"],
                                  "algebra capability evidence")
        semantics = require_keys(payload["semantics"], ["fixture_count", "operands_sha256",
                                                         "parallel_results_sha256",
                                                         "serial_results_sha256", "serial_equal"],
                                 "algebra semantic evidence")
        require(payload["schema"] == ALGEBRA_PROTOCOL
                and payload["field"] == "permanent.gray_chunk_subsets"
                and payload["shape_index"] == case["shape_index"]
                and payload["dimension"] == case["dimension"]
                and payload["requested_chunk_subsets"] == case["candidate"]
                and payload["observed_requested_chunk_subsets"] == case["candidate"]
                and profile["profile_id"] == "a835-algebra-forced"
                and profile["owner_section_id"] == "gf2-algebra/permanent"
                and profile["section_schema_version"] == 1
                and profile["harness_schema"] == ALGEBRA_BEHAVIOR
                and profile["resolution"] == "installed"
                and profile["active_gray_chunk_subsets"] == case["candidate"]
                and all(SHA.fullmatch(profile[key]) for key in
                        ["envelope_sha256", "content_sha256", "wrapper_sha256"])
                and capability == {"required_features": FEATURES, "pool_threads": 4,
                                   "parallel_route": "public-permanent-bipedal3-parallel"}
                and semantics["fixture_count"] == 8 and semantics["serial_equal"] is True
                and semantics["parallel_results_sha256"] == semantics["serial_results_sha256"]
                and all(SHA.fullmatch(semantics[key]) for key in
                        ["operands_sha256", "parallel_results_sha256",
                         "serial_results_sha256"]),
                "algebra payload route/profile/semantic evidence mismatch")
        hashes = forced_hashes(owner, {"permanent":{"gray_chunk_subsets":case["candidate"]}})
        require(profile["envelope_sha256"] == hashes["envelope"]
                and profile["content_sha256"] == hashes["content"]
                and profile["wrapper_sha256"] == hashes["section"], "forced algebra envelope hashes mismatch")
        subsets=(1 << case["dimension"])-1
        chunks=(subsets+case["candidate"]-1)//case["candidate"]
        require(payload["effective_partition"] == {"maximum_chunk_len":min(subsets,case["candidate"]),
                "chunk_count":chunks,"last_chunk_len":subsets-(chunks-1)*case["candidate"]},
                "permanent effective partition mismatch")
        return

    companions = {"gemm_row_tile": 32, "gemm_col_tile": 64, "dot_chunk_len": 256}
    experiment = payload.get("experiment")
    if experiment == "threshold":
        require_keys(payload, ["experiment", "schema", "ordinary_companions", "report",
                               "full_active_values"], "threshold payload")
        report = require_keys(payload["report"], ["protocol", "installed", "fixture_shape",
                                                   "seed_inventory", "operand_digest", "outcome"],
                              "threshold report")
        protocol = require_keys(report["protocol"], ["fresh_case_schema",
                                                       "profile_format_version", "section_id",
                                                       "section_schema_version", "harness_schema",
                                                       "raw_sample_schema", "timing"],
                                "threshold protocol identity")
        installed = require_keys(report["installed"], ["profile_id", "section_id", "resolution",
                                                        "measurement", "active_values",
                                                        "section_sha256",
                                                        "envelope_content_sha256"],
                                 "threshold installed evidence")
        outcome = require_keys(report["outcome"], ["status", "requested_route",
                                                    "observed_route", "effective_observation",
                                                    "capability_observation", "result_digest",
                                                    "equivalence_digest", "samples"],
                               "threshold complete outcome")
        require(payload["schema"] == "raw-timing-samples-v3"
                and payload["ordinary_companions"] == companions
                and payload["full_active_values"] == case["forced_values"]
                and protocol["fresh_case_schema"] == "child-v2"
                and protocol["profile_format_version"] == 2
                and protocol["section_id"] == "gf2-core/selectors"
                and protocol["section_schema_version"] == 1
                and protocol["harness_schema"] == CORE_BEHAVIOR
                and protocol["raw_sample_schema"] == "raw-timing-samples-v3"
                and protocol["timing"] == case["protocol"]
                and installed["section_id"] == "gf2-core/selectors"
                and installed["profile_id"] == "calibration-forced-arm"
                and installed["resolution"] == "installed"
                and installed["measurement"] == "inherited"
                and installed["active_values"] == threshold_forcing(case)
                and SHA.fullmatch(installed["section_sha256"])
                and SHA.fullmatch(installed["envelope_content_sha256"])
                and report["seed_inventory"] == case["seeds"]["seeds"]
                and SHA.fullmatch(report["operand_digest"])
                and outcome["status"] == "complete"
                and isinstance(outcome["requested_route"], str) and outcome["requested_route"]
                and isinstance(outcome["observed_route"], str) and outcome["observed_route"]
                and isinstance(outcome["effective_observation"], str)
                and outcome["effective_observation"]
                and isinstance(outcome["capability_observation"], str)
                and outcome["capability_observation"]
                and SHA.fullmatch(outcome["result_digest"])
                and SHA.fullmatch(outcome["equivalence_digest"])
                and outcome["samples"] == result["samples"],
                "threshold payload protocol/profile/route/semantic mismatch")
        require(report["fixture_shape"] == threshold_shape(case["identity"]["field"],
                                                          case["kind"]["spec"]["size"]),
                "threshold fixture shape differs from protocol")
        hashes = forced_hashes(owner, expected_forced_selectors(case))
        require(installed["section_sha256"] == hashes["section"] and installed["envelope_content_sha256"] == hashes["content"],
                "forced threshold profile hashes mismatch")
        validate_threshold_route(case, outcome)
        require(outcome["equivalence_digest"] == tuple_hash(
                    b"gf2-calibration-equivalence-v1",
                    [report["operand_digest"], outcome["result_digest"]]),
                "threshold equivalence digest is not bound to operands and result")
        return

    require(experiment == "extent", "core payload experiment mismatch")
    require_keys(payload, ["experiment", "schema", "ordinary_companions", "installed", "seeds",
                           "dimensions", "operands_sha256", "oracle_sha256", "result_sha256",
                           "observations"], "extent payload")
    installed = require_keys(payload["installed"], ["profile_id", "section_id", "resolution",
                                                    "measurement", "active_values",
                                                    "section_sha256", "envelope_content_sha256"],
                             "extent installed evidence")
    require(payload["schema"] == "raw-timing-samples-v3"
            and payload["ordinary_companions"] == companions
            and installed["profile_id"] == "calibration-forced-arm"
            and installed["section_id"] == "gf2-core/selectors"
            and installed["resolution"] == "installed"
            and installed["measurement"] == "inherited"
            and installed["active_values"] == case["forced_values"]
            and SHA.fullmatch(installed["section_sha256"])
            and SHA.fullmatch(installed["envelope_content_sha256"])
            and payload["seeds"] == case["seeds"]["seeds"]
            and all(SHA.fullmatch(payload[key]) for key in
                    ["operands_sha256", "oracle_sha256", "result_sha256"])
            and payload["oracle_sha256"] == payload["result_sha256"],
            "extent payload profile/fixture/oracle evidence mismatch")
    hashes = forced_hashes(owner, expected_forced_selectors(case))
    require(installed["section_sha256"] == hashes["section"] and installed["envelope_content_sha256"] == hashes["content"],
            "forced extent profile hashes mismatch")
    field = case["identity"]["field"]
    require(payload["dimensions"] == extent_dimensions(field, case["kind"]["cell"]["shape_index"]),
            "extent dimensions differ from protocol")
    route = {"bit_matrix.transpose_macro_tile_blocks": "macro_tiled",
             "soa_batch.parallel_chunk_len": "soa_parallel",
             "triangular.trsm_panel_rows": "trsm_blocked", "gemm.tiles": "gemm_tiles",
             "field_vec.dot_chunk_len": "dot_clmul_barrett"}.get(field, "m4rm")
    observations = payload["observations"]
    require(isinstance(observations, list) and len(observations) == 8,
            "extent lacks eight observations")
    for observation in observations:
        require_keys(observation, ["schedule", "m4rm_consumed", "dedicated_pool_width",
                                   "fp251_whole_gemm", "quiet_timing"],
                     "extent observation")
        require(observation["schedule"].get("route") == route
                and observation["dedicated_pool_width"] == (4 if field ==
                                                             "soa_batch.parallel_chunk_len"
                                                             else None)
                and observation["fp251_whole_gemm"] == (True if field ==
                                                         "triangular.trsm_panel_rows" else None)
                and observation["quiet_timing"] == (field == "triangular.trsm_panel_rows")
                and (observation["m4rm_consumed"] is not None) == field.startswith("m4rm."),
                "extent effective route/capability evidence mismatch")
        schedule = observation["schedule"]
        candidate = case["kind"]["cell"]["candidate"]
        if route == "macro_tiled":
            require(schedule == {"route": route, "blocks": candidate["value"]},
                    "transpose schedule differs from candidate")
        elif route == "soa_parallel":
            require(schedule == {"route": route, "chunk": candidate["value"]},
                    "SoA schedule differs from candidate")
        elif route == "trsm_blocked":
            require(schedule == {"route": route, "panel_rows": candidate["value"]},
                    "TRSM schedule differs from candidate")
        elif route == "gemm_tiles":
            require(schedule == {"route": route, "row": candidate["row"],
                                 "col": candidate["col"],
                                 "site": case["kind"]["cell"]["site"]},
                    "GEMM schedule differs from candidate/site")
        elif route == "dot_clmul_barrett":
            require(schedule == {"route": route, "chunk": candidate["value"]},
                    "dot schedule differs from candidate")
        else:
            _, k, n = extent_dimensions(field, case["kind"]["cell"]["shape_index"])
            expected_schedule, expected_consumed = m4rm_schedule(expected_forced_selectors(case)["m4rm"], k, n)
            require(schedule == expected_schedule and observation["m4rm_consumed"] == expected_consumed,
                    "M4RM executed schedule/consumed selectors differ from production contract")
            require_keys(schedule, ["route", "tier", "panel_width", "c_update"],
                         "M4RM effective schedule")
            require(schedule["tier"] in {"small_n", "wide"}
                    and type(schedule["panel_width"]) is int and schedule["panel_width"] > 0
                    and schedule["c_update"] == "row_wise",
                    "M4RM effective schedule is malformed")
            consumed = require_keys(observation["m4rm_consumed"],
                                    ["band", "table_bytes", "panel_width_cap",
                                     "tiled_stride_admitted"], "M4RM consumed selectors")
            require(consumed["band"] in {"small_n", "default", "mid", "wide"}
                    and (consumed["table_bytes"] is None
                         or type(consumed["table_bytes"]) is int)
                    and type(consumed["panel_width_cap"]) is int
                    and isinstance(consumed["tiled_stride_admitted"], bool),
                    "M4RM consumed selector evidence is malformed")


def validate_bundle(bundle: Any, manifest: Any,
                    checkpoint_files: dict[str, tuple[dict[str, Any], str]]) -> list[dict[str, Any]]:
    require_keys(bundle, ["schema", "manifest_sha256", "accepted"], "accepted bundle")
    require(bundle["schema"] == INPUT_SCHEMA and
            bundle["manifest_sha256"] == manifest["manifest_sha256"],
            "accepted bundle manifest binding mismatch")
    require(len(bundle["accepted"]) == len(manifest["ordered_units"]),
            "accepted bundle is incomplete")
    results = []
    paired_semantics = {}
    for index, (entry, unit) in enumerate(zip(bundle["accepted"], manifest["ordered_units"])):
        require_keys(entry, ["unit", "result", "checkpoint_sha256"], f"accepted {index}")
        require(entry["unit"] == unit, f"accepted unit order mismatch at {index}")
        key = unit["key"]
        require(key in checkpoint_files, f"accepted unit {key} has no checkpoint")
        checkpoint, file_sha = checkpoint_files[key]
        require(entry["checkpoint_sha256"] == file_sha, "accepted checkpoint digest mismatch")
        bound = checkpoint["result"]
        require_keys(bound, ["result", "journal_prefix", "attempt", "spawn_sequence",
                             "validation_sequence"], "bound checkpoint result")
        result = entry["result"]
        require(bound["result"] == result, "bundle result differs from checkpoint")
        require(result.get("schema") == RESULT_SCHEMA and result.get("identity") == unit["identity"]
                and result.get("case_sha256") == digest(unit["case"].encode())
                and result.get("outcome") == "complete", "child result identity/outcome mismatch")
        samples = result.get("samples")
        task = task_index(unit["identity"]["task"])
        require(isinstance(samples, list) and len(samples) == (0 if task == 0 else 5),
                "child window count mismatch")
        if task:
            calls = None
            for repetition, sample in enumerate(samples):
                calls = validate_sample(sample, task - 1, repetition, calls)
        require(isinstance(result.get("payload"), str), "child payload is not canonical JSON")
        validate_result_payload(result, embedded(unit["case"], "accepted owner case"),
                                unit["identity"]["owner"])
        coordinate, operands, semantic = semantic_pair_identity(
            embedded(unit["case"], "accepted case"), embedded(result["payload"], "accepted payload"))
        require(coordinate not in paired_semantics or paired_semantics[coordinate] == (operands, semantic),
                "candidate/probe/execution/joint semantic pairing differs")
        paired_semantics[coordinate] = (operands, semantic)
        require(checkpoint["result_sha256"] == digest(compact(bound)),
                "checkpoint bound-result digest mismatch")
        require(checkpoint["case"] == unit and checkpoint["case_sha256"] == digest(compact(unit)),
                "checkpoint unit binding mismatch")
        results.append(result)
    return results


def validate_checkpoints(stage: Path, campaign: str, identity: Any) -> dict[str, tuple[dict[str, Any], str]]:
    root = stage / "checkpoints"
    manifest = load_json(root / "manifest.json")
    require_keys(manifest, ["schema", "campaign_id", "identity"], "checkpoint manifest")
    require(manifest["schema"] == CHECKPOINT_SCHEMA and manifest["campaign_id"] == campaign
            and manifest["identity"] == identity, "checkpoint manifest identity mismatch")
    identity_sha = digest(compact(identity))
    pending = root / "pending"
    require(pending.is_dir() and not any(pending.iterdir()), "checkpoint pending directory is not empty")
    units_dir = root / "units"
    files = list(units_dir.iterdir())
    require(len(files) == 4302, f"checkpoint unit count is {len(files)}, expected 4302")
    result = {}
    for path in files:
        require(path.is_file() and not path.is_symlink() and path.suffix == ".json",
                f"unexpected checkpoint entry {path}")
        raw = path.read_bytes()
        unit = load_json_bytes(raw, str(path))
        require_keys(unit, ["schema", "campaign_id", "identity_sha256", "key", "case_sha256",
                            "result_sha256", "case", "result"], "checkpoint unit")
        key = unit["key"]
        require(unit["schema"] == CHECKPOINT_UNIT_SCHEMA and unit["campaign_id"] == campaign
                and unit["identity_sha256"] == identity_sha and SHA.fullmatch(key)
                and path.name == digest(key.encode()) + ".json" and key not in result,
                "checkpoint schema/name/identity mismatch")
        require(unit["case_sha256"] == digest(compact(unit["case"]))
                and unit["result_sha256"] == digest(compact(unit["result"])),
                "checkpoint inner digest mismatch")
        result[key] = (unit, digest(raw))
    return result


def parse_journal(stage: Path, campaign: str) -> tuple[list[dict[str, Any]], bytes, list[int]]:
    path = stage / "execution.log"
    require(path.is_file() and not path.is_symlink(), "execution log is missing or linked")
    data = path.read_bytes()
    require(data.endswith(b"\n"), "execution log has a torn tail")
    records = []
    ends = []
    offset = 0
    for sequence, raw in enumerate(data.splitlines(keepends=True)):
        require(raw.endswith(b"\n"), "execution log line lacks newline")
        record = load_json_bytes(raw[:-1], f"execution.log:{sequence + 1}")
        require_keys(record, ["schema", "timestamp_utc", "campaign_id", "session_id",
                              "sequence", "event", "details"] + (["case"] if "case" in record else []),
                     f"journal record {sequence}")
        require(record["schema"] == JOURNAL_SCHEMA and record["campaign_id"] == campaign
                and record["sequence"] == sequence
                and record["event"] in JOURNAL_EVENTS
                and isinstance(record["timestamp_utc"], str)
                and record["timestamp_utc"].endswith("Z")
                and isinstance(record["session_id"], str)
                and TOKEN.fullmatch(record["session_id"]) is not None,
                "journal sequence/schema/campaign/time/session mismatch")
        records.append(record)
        offset += len(raw)
        ends.append(offset)
    require(records and records[0]["event"] == "campaign-start", "journal lacks campaign-start")
    return records, data, ends


def process_outcome(value: Any, where: str) -> bool:
    require(isinstance(value,dict),f'{where} process outcome missing')
    kind=value.get('outcome')
    extra={'exited':['exit_code'],'signaled':['signal'],'timed-out':['kill_grace_exhausted']}
    require(kind in extra,f'{where} unknown process outcome')
    require_keys(value,['outcome','pid','elapsed_ns','all_descendants_reaped']+extra[kind],where)
    require(type(value['pid']) is int and value['pid']>0 and type(value['elapsed_ns']) is int and value['elapsed_ns']>0
            and type(value['all_descendants_reaped']) is bool,f'{where} malformed outcome')
    if kind=='exited':
        require(type(value['exit_code']) is int and 0<=value['exit_code']<=255,f'{where} invalid exit code')
    elif kind=='signaled':
        require(type(value['signal']) is int and value['signal']>0,f'{where} invalid signal')
    else:
        require(type(value['kill_grace_exhausted']) is bool and value['elapsed_ns']>=120_000_000_000
                and (not value['kill_grace_exhausted'] or value['elapsed_ns']>=125_000_000_000),f'{where} invalid timeout/grace')
    require(value['elapsed_ns']<=125_000_000_000 or kind=='timed-out' and value['kill_grace_exhausted'],
            f'{where} exceeds child timeout/grace without exhausted grace evidence')
    return kind=='exited' and value['exit_code']==0 and value['all_descendants_reaped']


def validate_progress_record(item: Any, unit: Any, index: int, calls: int | None) -> int:
    require_keys(item,['schema','identity','case_sha256','progress'],'child progress')
    require(item['schema']=='tuning-campaign-progress-v1' and item['identity']==unit['identity']
            and item['case_sha256']==digest(unit['case'].encode()) and task_index(unit['identity']['task'])>0,
            'progress schema/case/task identity mismatch')
    progress=item['progress']
    require_keys(progress,['event','calls']+([] if index==0 else ['repetition','elapsed_ns']),'progress event')
    count=progress['calls']
    require(type(count) is int and 0<count<=MAX_CALLS and (calls is None or calls==count),'progress call count/calibration mismatch')
    if index==0:
        require(progress['event']=='calibration-complete','first progress is not calibration')
    else:
        require(1<=index<=5 and progress['event']=='window-complete'
                and type(progress['repetition']) is int and progress['repetition']==index-1
                and type(progress['elapsed_ns']) is int and progress['elapsed_ns']>0,
                'progress windows are duplicated, reordered or malformed')
    return count


def replay_acquisition(records: list[Any], units: list[Any]) -> dict[str, int]:
    """Replay attempts against the next unfinished slot, including interrupted retries."""
    by_key={unit['key']:unit for unit in units}
    require(len(by_key)==len(units),'duplicate acquisition unit key')
    accepted=set()
    attempts={}
    last_attempt={}
    phases={}
    active=None
    index=0
    closed_sessions=set()
    for record in records:
        event,details,session=record['event'],record['details'],record['session_id']
        if event in {'complete','failed','paused','budget-exhausted','interrupted'} and 'session_transition' in details:
            closed_sessions.add(session)
            active=None
        elif event=='phase-start':
            require(phases.get(session) is None,'phase opened before previous phase completed')
            require(index<len(units) and details=={'phase':units[index]['identity']['phase'],'owner':units[index]['identity']['owner']},'phase start differs from unfinished acquisition manifest')
            phases[session]=details['phase']
        elif event=='phase-complete':
            phase=phases.get(session)
            require(phase is not None and details=={'phase':phase},'phase completion is unpaired')
            require(not any(unit['key'] not in accepted and unit['identity']['phase']==phase for unit in units),
                    'phase completed with unfinished slots')
            phases[session]=None
        elif event=='cell-start':
            require(index<len(units),'cell started after complete campaign')
            unit=units[index]
            require(details=={'key':unit['key'],'ordinal':unit['ordinal']} and record.get('case')==unit['identity']
                    and phases.get(session)==unit['identity']['phase'],'cell acquisition order/case/phase mismatch')
            prior=last_attempt.get(unit['key'])
            if prior is not None:
                previous=attempts[prior]
                require(previous['session']!=session and previous['session'] in closed_sessions,
                        'retry has no terminal interrupted/failed prior session')
                require(not previous.get('clean',False),'durable complete output was resampled')
            active=f"attempt-{record['sequence']}"
            require(active not in attempts,'duplicate attempt')
            attempts[active]={'unit':unit,'session':session,'start':record['sequence'],'progress':0,'calls':None}
            last_attempt[unit['key']]=active
        elif event=='child-spawn':
            attempt=details.get('attempt')
            require(attempt==active and attempt in attempts,'spawn lacks current cell-start')
            state=attempts[attempt];unit=state['unit']
            require(record['sequence']==state['start']+1 and record.get('case')==unit['identity']
                    and details.get('unit_key')==unit['key'] and details.get('case_sha256')==digest(unit['case'].encode())
                    and type(details.get('pid')) is int and details['pid']>0,'spawn binding mismatch')
            state['pid']=details['pid']
        elif event in {'execution-progress','window-progress'}:
            require_keys(details,['attempt','record'],'journal progress')
            attempt=details['attempt']
            require(attempt in attempts and attempts[attempt]['session']==session,'progress outside its attempt session')
            state=attempts[attempt]
            require('pid' in state and 'exit' not in state and record.get('case')==state['unit']['identity'],
                    'progress outside child lifetime')
            require(event==('execution-progress' if state['progress']==0 else 'window-progress'),'journal progress event mismatch')
            state['calls']=validate_progress_record(details['record'],state['unit'],state['progress'],state['calls'])
            state['progress']+=1
        elif event=='child-exit':
            attempt=details.get('attempt')
            require(attempt in attempts,'exit without spawn')
            state=attempts[attempt]
            require('exit' not in state and state['session']==session and details.get('unit_key')==state['unit']['key'],
                    'duplicate or unrelated exit')
            require(details['outcome']['pid']==state['pid'],'exit PID differs from spawn')
            state['clean']=process_outcome(details['outcome'],'child attempt')
            state['exit']=record['sequence']
        elif event=='checkpoint-accepted':
            key=details['unit_key']
            require(index<len(units) and key==units[index]['key'] and key not in accepted,'checkpoint acceptance reordered or duplicated')
            accepted.add(key)
            index+=1
        elif event=='cell-complete':
            key=details.get('key')
            require(key in accepted and record.get('case')==by_key[key]['identity'],'cell completion lacks accepted checkpoint')
            active=None
    require(index==len(units),'acquisition replay is incomplete')
    return {'attempts':len(attempts),'accepted':len(accepted),
            'failed_or_interrupted_attempts':len(attempts)-len(accepted),
            'attempt_progress':sum(state['progress'] for state in attempts.values())}


def replay_lifecycle(records: list[Any], descriptors: dict[str,Any], preterminal: bool) -> dict[str,int]:
    states={name:{'state':'prepared','work':None,'hold':None,'wrapper':None,'independent':False,'elapsed':0} for name in descriptors}
    require(len({descriptor['lock_path'] for descriptor in descriptors.values()})==1,'campaign sessions use different mutexes')
    held=None
    for record in records:
        session=record['session_id']
        if session not in states:
            require(record['event'] in {'campaign-start','driver-diagnostic','session-start','session-recovery','recovery'},
                    'work or lifecycle record names a session outside descriptors')
            continue
        state=states[session]
        transition=record['details'].get('session_transition',{}).get('transition')
        if transition is None:
            if record['event'] in {'child-spawn','owner-write','composition'} or record['event']=='orchestration-start' and state['state']!='prepared':
                require(state['state']=='held' and held==session,'protected work outside active hold')
            continue
        kind=transition.get('transition')
        before=state['state']
        if kind=='lock-held':
            require(before=='prepared' and held is None,'overlapping or duplicate lock hold')
            evidence=require_keys(transition['evidence'],['lock_path','holder_pid','observation'],'lock evidence')
            require(evidence['lock_path']==descriptors[session]['lock_path'] and type(evidence['holder_pid']) is int and evidence['holder_pid']>0,'lock identity mismatch')
            state['hold']=evidence;state['state']='held';held=session
        elif kind=='work-finished':
            work=require_keys(transition['evidence'],['outcome','all_descendants_reaped','active_elapsed_ns'],'work evidence')
            require(before=='held' and work['all_descendants_reaped'] is True
                    and type(work['active_elapsed_ns']) is int and 0<work['active_elapsed_ns']<=10_800_000_000_000
                    and work['outcome'] in {'complete','failed','paused','budget-exhausted'},'invalid work finish or session budget')
            state['work']=work;state['elapsed']=work['active_elapsed_ns'];state['state']='finished'
        elif kind=='wrapper-returned':
            wrapper=require_keys(transition['evidence'],['exit_code','signal'],'wrapper evidence')
            require(before in {'prepared','held','finished'} and ((type(wrapper['exit_code']) is int and 0<=wrapper['exit_code']<=255 and wrapper['signal'] is None)
                    or (wrapper['exit_code'] is None and type(wrapper['signal']) is int and wrapper['signal']>0)), 'illegal wrapper return')
            state['wrapper']=wrapper;state['state']='returned'
        elif kind=='release-unobserved':
            require(before in {'held','finished','returned'} and state['hold'] is not None,'illegal release-unobserved')
            state['state']='unobserved'
        elif kind=='lock-release':
            require(before in {'held','finished','returned','unobserved'} and held==session,'release without active hold')
            evidence=transition['evidence']
            if evidence.get('kind')=='independent':
                proof=require_keys(evidence['evidence'],['holder_dead','descendants_dead','lock_path','observed_utc','lock_available'],'independent release proof')
                require(proof['holder_dead'] is proof['descendants_dead'] is proof['lock_available'] is True
                        and proof['lock_path']==descriptors[session]['lock_path'] and isinstance(proof['observed_utc'],str) and proof['observed_utc'].endswith('Z'),'invalid release proof')
                state['independent']=True
            else:
                require(evidence=={'kind':'clean-reaped'} and state['work'] is not None and state['wrapper'] is not None,'unproven clean release')
            state['state']='released';held=None
        elif kind=='terminal':
            outcome=transition.get('outcome')
            prelock=before=='returned' and state['hold'] is None and outcome=='failed' and state['wrapper']['exit_code']!=0
            require(before=='released' or prelock,'terminal before proven release')
            require(outcome in {'complete','failed','paused','budget-exhausted'} and (outcome=='failed' or state['work'] is not None and outcome==state['work']['outcome']), 'terminal contradicts work')
            require(outcome!='complete' or state['wrapper']=={'exit_code':0,'signal':None},'complete contradicts wrapper')
            state['state']='terminal'
        elif kind=='interrupted':
            require(before=='released' and state['independent'] and transition.get('active_elapsed_censored') is True,'uncensored/unproven interruption')
            state['state']='interrupted'
        elif kind=='prelock-interrupted':
            require(before=='prepared' and transition.get('evidence',{}).get('active_elapsed_censored') is True,'illegal prelock interruption')
            state['state']='interrupted'
        else:
            fail('unknown lifecycle transition')
    require(held is None,'campaign still holds benchmark mutex')
    require(all(state['state'] in {'terminal','interrupted'} or preterminal and name==records[-1]['session_id'] and state['state']=='released' for name,state in states.items()),'session lifecycle incomplete')
    return {'cumulative_observed_active_ns':sum(state['elapsed'] for state in states.values()),
            'censored_sessions':sum(state['state']=='interrupted' for state in states.values())}


def validate_journal(records: list[dict[str, Any]], log_data: bytes, record_ends: list[int],
                     checkpoint_files: dict[str, Any], campaign_config: Any,
                     preterminal: bool) -> None:
    stage = Path(campaign_config["channels"]["stage"])
    units = [checkpoint_files[unit["key"]][0]["case"] for manifest in campaign_config["manifests"] for unit in manifest["ordered_units"]]
    replay_acquisition(records, units)
    require(records[0]["details"] == expected_bootstrap_inputs(stage, campaign_config),
            "campaign-start does not bind exact bootstrap/staging identity inputs")
    accepted: dict[str, tuple[dict[str, Any], dict[str, Any]]] = {}
    progress_by_attempt: dict[str, list[dict[str, Any]]] = defaultdict(list)
    validated: dict[str, tuple[dict[str, Any], dict[str, Any]]] = {}
    spawn: dict[str, tuple[dict[str, Any], dict[str, Any]]] = {}
    raw_streams: dict[str, tuple[dict[str, Any], dict[str, Any]]] = {}
    child_exits: dict[str, tuple[dict[str, Any], dict[str, Any]]] = {}
    orchestration_starts: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    orchestration_exits: dict[int, list[dict[str, Any]]] = defaultdict(list)
    sessions: dict[str, list[str]] = defaultdict(list)
    for record in records:
        event = record["event"]
        sessions[record["session_id"]].append(event)
        details = record["details"]
        if event == "child-spawn":
            require(details["attempt"] not in spawn, "duplicate child attempt identity")
            spawn[details["attempt"]] = (details, record)
        elif event in {"execution-progress", "window-progress"}:
            progress_by_attempt[details["attempt"]].append(details["record"])
        elif event == "raw-streams":
            raw_attempt = details.get("exit", {}).get("attempt")
            require(raw_attempt is not None and raw_attempt not in raw_streams,
                    "duplicate or malformed raw stream evidence")
            raw_streams[raw_attempt] = (details, record)
        elif event == "child-exit":
            require(details.get("attempt") not in child_exits,
                    "duplicate child exit attempt identity")
            child_exits[details["attempt"]] = (details, record)
        elif event == "orchestration-start":
            request_sha = details.get("request_sha256")
            if isinstance(request_sha, str):
                orchestration_starts[(details.get("process"), request_sha)].append(record)
        elif event == "orchestration-exit" and type(details.get("start_sequence")) is int:
            orchestration_exits[details["start_sequence"]].append(record)
        elif event == "result-validated":
            require(details["unit_key"] not in validated, "duplicate result validation")
            validated[details["unit_key"]] = (details, record)
        elif event == "checkpoint-accepted":
            key = details["unit_key"]
            require(key not in accepted and key in checkpoint_files,
                    "duplicate or unknown checkpoint acceptance")
            require(details["checkpoint_sha256"] == checkpoint_files[key][1],
                    "journal checkpoint digest mismatch")
            accepted[key] = (details, record)
    by_sequence = {record["sequence"]: record for record in records}
    budget_records = [record for record in records
                      if record["event"] == "driver-diagnostic"
                      and record["details"].get("kind") == "session-budget-observation"]
    for record in budget_records:
        details = record["details"]
        validate_budget_diagnostic(record, details.get("process"), details.get("boundary"),
                                   details.get("unit_key"))
    orchestration_records = [record for record in records
                             if record["event"] == "orchestration-start"]
    for start in orchestration_records:
        process_id = start["details"].get("process")
        validate_budget_diagnostic(by_sequence.get(start["sequence"] - 1), process_id,
                                   "before-launch")
        exits = orchestration_exits.get(start["sequence"], [])
        if exits:
            require(len(exits) == 1, "orchestration start has duplicate exits")
            validate_budget_diagnostic(by_sequence.get(exits[0]["sequence"] + 1), process_id,
                                       "after-result")
    unit_processes = {unit["key"]: unit["process"] for manifest in campaign_config["manifests"]
                      for unit in manifest["ordered_units"]}
    cell_starts = [record for record in records if record["event"] == "cell-start"]
    for start in cell_starts:
        key = start["details"].get("key")
        validate_budget_diagnostic(by_sequence.get(start["sequence"] - 1),
                                   unit_processes.get(key), "before-launch", key)
    before_units = [record["details"]["unit_key"] for record in budget_records
                    if record["details"]["boundary"] == "before-launch"
                    and record["details"]["unit_key"] is not None]
    after_units = [record["details"]["unit_key"] for record in budget_records
                   if record["details"]["boundary"] == "after-result"
                   and record["details"]["unit_key"] is not None]
    require(before_units == [record["details"]["key"] for record in cell_starts]
            and after_units == before_units,
            "child launch/result budget observations do not cover exact attempted order")
    empty_sha = digest(b"")
    for owner in ["core", "algebra"]:
        process_id = f"{owner}-producer"
        for mode in ["self-check", "list-grid", "capability-report"]:
            expected_start = {"kind": "orchestration-start", "process": process_id,
                              "request_sha256": empty_sha, "arguments": [f"--{mode}"]}
            starts = [record for record in records
                      if record["event"] == "orchestration-start"
                      and record["details"] == expected_start]
            require(len(starts) == 1,
                    f"staged {owner} --{mode} preflight was not invoked exactly once")
            exits = orchestration_exits.get(starts[0]["sequence"], [])
            require(len(exits) == 1, f"staged {owner} --{mode} preflight lacks one exit")
            response = load_json(stage / f"{owner}-{mode}.json")
            expected_stdout = b"GF2_TUNING_RESULT=" + compact(response)
            details = exits[0]["details"]
            clean_process_outcome(details.get("outcome"),
                                  f"staged {owner} --{mode} preflight outcome")
            require(details.get("kind") == "orchestration-exit"
                    and details.get("process") == process_id
                    and details.get("stdout_sha256") in {
                        digest(expected_stdout), digest(expected_stdout + b"\n")}
                    and details.get("stderr_sha256") == digest(bytes(details.get("stderr", []))),
                    f"staged {owner} --{mode} preflight streams differ from saved response")
    require(set(accepted) == set(checkpoint_files) and len(accepted) == 4302,
            "journal/checkpoint acceptance universe mismatch")
    accepted_attempts = {checkpoint["result"]["attempt"]
                         for checkpoint, _ in checkpoint_files.values()}
    require(len(accepted_attempts) == 4302 and accepted_attempts <= set(raw_streams),
            "completed child attempts differ from exact accepted attempt identities")
    require(set(raw_streams) == set(child_exits),
            "durable raw completion and repaired child-exit attempts differ")
    for attempt, (raw_details, _) in raw_streams.items():
        if attempt not in accepted_attempts:
            process_outcome(raw_details["exit"]["outcome"], "failed attempt")
            byte_artifact(raw_details["stdout"], "failed raw stdout", stage)
            byte_artifact(raw_details["stderr"], "failed raw stderr", stage)
            continue
        require_keys(raw_details, ["exit", "stdout", "stderr", "stream_validation"],
                     "raw stream completion evidence")
        require_keys(raw_details["stream_validation"],
                     ["stderr", "callback_error", "progress_matches_journal"],
                     "raw stream completion validation")
        require(raw_details["stream_validation"] == {
                    "stderr": "valid", "callback_error": None,
                    "progress_matches_journal": True}
                and attempt in child_exits,
                "raw stream completion was rejected or lacks its repaired child exit")
    for key, (checkpoint, _) in checkpoint_files.items():
        bound = checkpoint["result"]
        attempt = bound["attempt"]
        require(key in validated and attempt in spawn, "accepted unit lacks spawn/validation")
        case_sha = digest(checkpoint["case"]["case"].encode())
        result_sha = digest(compact(bound["result"]))
        spawn_details, spawn_record = spawn[attempt]
        validation_details, validation_record = validated[key]
        cell_starts = [record for record in records if record["event"] == "cell-start"
                       and record["details"].get("key") == key
                       and record["sequence"] + 1 == spawn_record["sequence"]]
        require(len(cell_starts) == 1 and attempt == f"attempt-{cell_starts[0]['sequence']}"
                and cell_starts[0].get("case") == checkpoint["case"]["identity"]
                and cell_starts[0]["details"] == {
                    "key": key, "ordinal": checkpoint["case"]["ordinal"]},
                "accepted attempt lacks its deterministic cell-start identity")
        require(spawn_details == {"unit_key": key, "case_sha256": case_sha,
                                   "attempt": attempt, "pid": spawn_details.get("pid")}
                and type(spawn_details["pid"]) is int and spawn_details["pid"] > 0
                and spawn_record.get("case") == checkpoint["case"]["identity"],
                "accepted child spawn identity mismatch")
        require(validation_details == {"unit_key": key, "case_sha256": case_sha,
                                        "attempt": attempt, "result_sha256": result_sha}
                and validation_record.get("case") == checkpoint["case"]["identity"],
                "owner validation evidence mismatch")
        validation_request = {"operation": "validate-result", "unit": checkpoint["case"],
                              "result": bound["result"]}
        request_sha = digest(compact(validation_request))
        starts = orchestration_starts[(checkpoint["case"]["process"], request_sha)]
        matching_pairs = [(start, exit) for start in starts
                          for exit in orchestration_exits[start["sequence"]]
                          if exit["sequence"] + 1 == validation_record["sequence"]]
        require(len(matching_pairs) == 1,
                "owner validation invocation/decision journal order mismatch")
        validation_exit = matching_pairs[0][1]["details"]
        clean_process_outcome(validation_exit.get("outcome"), "owner validation outcome")
        expected_response = compact({"operation": "validate-result", "unit_key": key,
                                     "result_sha256": result_sha})
        framed = b"GF2_TUNING_RESULT=" + expected_response
        require(validation_exit.get("process") == checkpoint["case"]["process"]
                and validation_exit.get("stdout_sha256") in
                    {digest(framed), digest(framed + b"\n")}
                and validation_exit.get("stderr_sha256") == digest(b"")
                and validation_exit.get("stderr") == [],
                "owner validation response stream digest mismatch")
        require(bound["spawn_sequence"] == spawn_record["sequence"]
                and bound["validation_sequence"] == validation_record["sequence"]
                and bound["spawn_sequence"] < bound["validation_sequence"],
                "checkpoint sequence binding is reversed")
        require(attempt in raw_streams and attempt in child_exits,
                "accepted attempt lacks durable raw streams or child exit")
        raw_details, raw_record = raw_streams[attempt]
        exit_details, exit_record = child_exits[attempt]
        require_keys(raw_details, ["exit", "stdout", "stderr", "stream_validation"],
                     "accepted raw stream evidence")
        require_keys(raw_details["stream_validation"],
                     ["stderr", "callback_error", "progress_matches_journal"],
                     "accepted raw stream validation")
        require(raw_details["stream_validation"] == {
                    "stderr": "valid", "callback_error": None,
                    "progress_matches_journal": True},
                "accepted raw streams were not independently accepted")
        stdout_path, stdout = byte_artifact(raw_details["stdout"], "accepted raw stdout", stage)
        stderr_path, stderr = byte_artifact(raw_details["stderr"], "accepted raw stderr",
                                            stage)
        raw_prefix = f"{raw_record['session_id']}-{attempt}"
        require(stdout_path.parent == stderr_path.parent == stage / "raw-attempts"
                and stdout_path.name == raw_prefix + ".stdout"
                and stderr_path.name == raw_prefix + ".stderr"
                and raw_record.get("case") == checkpoint["case"]["identity"],
                "accepted raw stream paths/case mismatch")
        require_keys(exit_details, ["unit_key", "case_sha256", "attempt", "outcome",
                                    "stdout_sha256", "stderr_sha256"],
                     "accepted child exit")
        require(raw_details["exit"] == exit_details
                and exit_details["unit_key"] == key and exit_details["case_sha256"] == case_sha
                and exit_details["attempt"] == attempt
                and exit_details["outcome"].get("pid") == spawn_details["pid"]
                and exit_details["stdout_sha256"] == raw_details["stdout"]["sha256"]
                and exit_details["stderr_sha256"] == raw_details["stderr"]["sha256"]
                and exit_record.get("case") == checkpoint["case"]["identity"],
                "accepted raw stream/exit digest binding mismatch")
        clean_process_outcome(exit_details["outcome"], "accepted child outcome")
        expected_stdout = b"GF2_TUNING_RESULT=" + compact(bound["result"])
        require(stdout in {expected_stdout, expected_stdout + b"\n"},
                "accepted raw stdout differs from the bound result")
        progress = progress_by_attempt[attempt]
        expected_stderr = b"".join(b"GF2_TUNING_PROGRESS=" + compact(item) + b"\n"
                                   for item in progress)
        require(stderr == expected_stderr,
                "accepted raw stderr differs from journaled progress")
        require(spawn_record["sequence"] < raw_record["sequence"] < exit_record["sequence"]
                < validation_record["sequence"],
                "accepted raw/exit/validation evidence order mismatch")
        exits_for_key = [(details, record) for details, record in child_exits.values()
                         if details.get("unit_key") == key]
        require(sum(process_outcome(details["outcome"], "prior outcome") for details, _ in exits_for_key) == 1
                and not any(details.get("unit_key") == key
                            and record["sequence"] > exit_record["sequence"]
                            for details, record in spawn.values()),
                "a completed measurement was respawned instead of recovered")
        prefix = bound["journal_prefix"]
        require_keys(prefix, ["byte_len", "sha256", "record_count"], "bound journal prefix")
        length = prefix["byte_len"]
        count = prefix["record_count"]
        require(type(count) is int and 0 < count <= len(records)
                and length == record_ends[count - 1]
                and digest(log_data[:length]) == prefix["sha256"]
                and bound["validation_sequence"] < count,
                "checkpoint journal prefix is missing or changed")
        acceptance, acceptance_record = accepted[key]
        require(acceptance["journal_prefix"] == prefix
                and acceptance_record["sequence"] >= count,
                "acceptance event changed checkpoint journal prefix")
        task = task_index(checkpoint["case"]["identity"]["task"])
        progress = progress_by_attempt[attempt]
        require(len(progress) == (0 if task == 0 else 6), "accepted attempt progress incomplete")
        if task:
            calls = None
            for progress_index, item in enumerate(progress):
                calls = validate_progress_record(item, checkpoint["case"], progress_index, calls)
            require(progress[0]["progress"]["event"] == "calibration-complete",
                    "accepted timing lacks calibration progress")
            for repetition, item in enumerate(progress[1:]):
                detail = item["progress"]
                sample = checkpoint["result"]["result"]["samples"][repetition]
                require(detail == {"event": "window-complete", "repetition": repetition,
                                   "calls": sample["calls"], "elapsed_ns": sample["elapsed_ns"]},
                        "progress/result window mismatch")
    require(sum(len(progress_by_attempt[checkpoint["result"]["attempt"]])
                for checkpoint, _ in checkpoint_files.values()) == COUNTS["progress_records"],
            "accepted journal progress accounting mismatch")
    terminal = records[-1]["event"]
    if preterminal:
        require(terminal == "lock-release" and "complete" not in [r["event"] for r in records],
                "preterminal validation requires final LockRelease and no terminal")
    else:
        require(terminal == "complete", "complete validation requires final Complete terminal")
    terminal_session = records[-1]["session_id"]
    events = sessions[terminal_session]
    required = ["session-prepared", "lock-hold", "work-finished", "wrapper-returned",
                "lock-release"] + ([] if preterminal else ["complete"])
    require(all(events.count(event) == 1 for event in required),
            "final session lacks unique lifecycle evidence")
    positions = [events.index(event) for event in required]
    require(positions == sorted(positions), "final session lifecycle is out of order")
    transitions = {record["event"]: record["details"].get("session_transition")
                   for record in records if record["session_id"] == terminal_session
                   and record["event"] in required}
    for event in required[1:]:
        transition = transitions[event]
        require(isinstance(transition, dict), f"{event} lacks durable transition projection")
    work = transitions["work-finished"]
    require(work.get("transition", {}).get("transition") == "work-finished"
            and work["transition"].get("evidence", {}).get("outcome") == "complete"
            and work["transition"]["evidence"].get("all_descendants_reaped") is True
            and type(work["transition"]["evidence"].get("active_elapsed_ns")) is int
            and work["transition"]["evidence"]["active_elapsed_ns"] > 0,
            "final WorkFinished evidence is not a complete reaped run")
    wrapper = transitions["wrapper-returned"]
    require(wrapper.get("transition", {}).get("transition") == "wrapper-returned"
            and wrapper["transition"].get("evidence") == {"exit_code": 0, "signal": None},
            "final wrapper return is not a clean zero exit")
    release = transitions["lock-release"]
    release_evidence = release.get("transition", {}).get("evidence", {})
    require(release.get("transition", {}).get("transition") == "lock-release"
            and release_evidence.get("kind") == "independent"
            and release_evidence.get("evidence", {}).get("holder_dead") is True
            and release_evidence["evidence"].get("descendants_dead") is True
            and release_evidence["evidence"].get("lock_available") is True,
            "final lock release lacks independent death/availability evidence")
    if not preterminal:
        complete = transitions["complete"]
        require(complete.get("transition", {}).get("transition") == "terminal"
                and complete["transition"].get("outcome") == "complete",
                "final Complete transition outcome mismatch")


def ns_per_call(sample: dict[str, Any]) -> float:
    value = sample["elapsed_ns"] / sample["calls"]
    require(math.isfinite(value) and value > 0, "invalid ns/call")
    return value


def execution_median(result: dict[str, Any]) -> float:
    return statistics.median(ns_per_call(x) for x in result["samples"])


def summary(values: list[float]) -> dict[str, float]:
    ordered = sorted(values)
    require(len(ordered) == 5 and all(math.isfinite(x) and x > 0 for x in ordered),
            "empirical summary requires five positive values")
    return {"median": ordered[2], "iqr_low": ordered[1], "iqr_high": ordered[3],
            "min": ordered[0], "max": ordered[4]}


def strict_compare(left: list[float], right: list[float]) -> str:
    if all(a / b < 1 for a, b in zip(left, right)):
        return "left-wins"
    if all(b / a < 1 for a, b in zip(left, right)):
        return "right-wins"
    return "unresolved"


def curve(series: list[list[float]]) -> str:
    saw_increase = False
    for left, right in zip(series, series[1:]):
        relation = strict_compare(left, right)
        if relation == "left-wins":
            saw_increase = True
        elif relation == "right-wins" and saw_increase:
            return "non-monotone"
    return "unimodal"


def scores(series: list[dict[str, Any]], default_index: int) -> list[dict[str, Any]]:
    out = []
    strata = len(series[0]["strata"])
    for candidate in series:
        executions = []
        for execution in range(5):
            logs = [math.log(candidate["strata"][s][execution] /
                             series[default_index]["strata"][s][execution])
                    for s in range(strata)]
            executions.append(math.exp(sum(logs) / strata))
        out.append({"candidate": candidate["candidate"], "executions": executions,
                    "summary": summary(executions)})
    return out


def schedule_classes(series: list[dict[str, Any]]) -> tuple[list[list[int]], bool]:
    classes: list[list[int]] = []
    for index, candidate in enumerate(series):
        if classes and series[classes[-1][0]]["schedules"] == candidate["schedules"]:
            classes[-1].append(index)
        else:
            classes.append([index])
    seen = []
    repeat = False
    for group in classes:
        schedule = series[group[0]]["schedules"]
        repeat |= any(old == schedule for old in seen)
        seen.append(schedule)
    return classes, repeat


def analyze_extent(series: list[dict[str, Any]], default: Any) -> dict[str, Any]:
    default_index = next((i for i, c in enumerate(series) if c["candidate"] == default), None)
    require(default_index is not None, "extent default absent")
    score = scores(series, default_index)
    classes, repeat = schedule_classes(series)
    representatives = []
    for group in classes:
        representatives.append(next((i for i in group if series[i]["candidate"] == default), group[0]))
    class_evidence = [
        {"members": [series[i]["candidate"] for i in group],
         "representative": series[representative]["candidate"]}
        for group, representative in zip(classes, representatives)
    ]
    aggregate = curve([score[i]["executions"] for i in representatives])
    strata_curves = [curve([series[i]["strata"][s] for i in representatives])
                     for s in range(len(series[0]["strata"]))]
    selected, reason, boundary = default, None, False
    if repeat:
        reason = "non-monotone-schedule"
    elif aggregate == "non-monotone" or "non-monotone" in strata_curves:
        reason = "non-monotone-curve"
    else:
        winner_class = min(range(len(classes)), key=lambda c: score[representatives[c]]["summary"]["median"])
        winner = representatives[winner_class]
        if not all(i == winner or strict_compare(score[winner]["executions"], score[i]["executions"]) == "left-wins"
                   for i in representatives):
            reason = "unresolved-minimum"
        elif len(classes[winner_class]) != 1:
            reason = "structural-schedule-tie"
        elif any(strict_compare(series[default_index]["strata"][s], series[winner]["strata"][s]) == "left-wins"
                 for s in range(len(series[0]["strata"]))):
            reason = "cross-stratum-conflict"
        else:
            selected = series[winner]["candidate"]
            reason = "measured-default" if winner == default_index else "selected-non-default"
            boundary = winner_class in {0, len(classes) - 1}
    return {"selected": selected, "reason": reason, "grid_boundary_limited": boundary,
            "scores": score, "schedule_classes": class_evidence,
            "aggregate_curve": aggregate, "stratum_curves": strata_curves}


def analyze_gemm(series: list[dict[str, Any]], default: tuple[int, int]) -> dict[str, Any]:
    require([item["candidate"] for item in series] == EXTENT_GRIDS["gemm.tiles"],
            "GEMM candidates are not the exact row-major grid")
    default_index = next(i for i, item in enumerate(series) if item["candidate"] == default)
    score = scores(series, default_index)
    slices = []
    for axis, fixed_values in (("fixed-row", [16, 32, 64]),
                               ("fixed-column", [32, 64, 128])):
        for fixed, value in enumerate(fixed_values):
            indices = ([fixed * 3 + moving for moving in range(3)] if axis == "fixed-row"
                       else [moving * 3 + fixed for moving in range(3)])
            slices.append({"axis": axis, "fixed_value": value, "stratum": None,
                           "shape": curve([score[i]["executions"] for i in indices])})
            for stratum in range(len(series[0]["strata"])):
                slices.append({"axis": axis, "fixed_value": value, "stratum": stratum,
                               "shape": curve([series[i]["strata"][stratum] for i in indices])})
    selected, reason, boundary = default, None, None
    if any(item["shape"] == "non-monotone" for item in slices):
        reason = "non-monotone-curve"
    else:
        winner = min(range(9), key=lambda i: score[i]["summary"]["median"])
        if not all(i == winner or strict_compare(score[winner]["executions"],
                                                 score[i]["executions"]) == "left-wins"
                   for i in range(9)):
            reason = "unresolved-minimum"
        elif any(strict_compare(series[default_index]["strata"][s],
                                series[winner]["strata"][s]) == "left-wins"
                 for s in range(len(series[0]["strata"]))):
            reason = "cross-stratum-conflict"
        else:
            selected = series[winner]["candidate"]
            reason = "measured-default" if winner == default_index else "selected-nondefault"
            boundary = {"row": winner // 3 in {0, 2}, "column": winner % 3 in {0, 2}}
    return {"selected": list(selected), "reason": reason, "grid_boundary_limited": boundary,
            "scores": [{**item, "candidate": list(item["candidate"])} for item in score],
            "slices": slices}


def analyze_joint(conservative: dict[str, Any], proposed: dict[str, Any]) -> dict[str, Any]:
    ratios = []
    regression = False
    for execution in range(5):
        values = []
        for default_stratum, trial_stratum in zip(conservative["strata"], proposed["strata"]):
            ratio = trial_stratum[execution] / default_stratum[execution]
            values.append(math.log(ratio))
            regression |= ratio > 1.0
        ratios.append(math.exp(sum(values) / len(values)))
    if conservative["candidate"] == proposed["candidate"]:
        selected, reason = conservative["candidate"], "conservative-vector"
    elif all(value < 1.0 for value in ratios) and not regression:
        selected, reason = proposed["candidate"], "accepted"
    else:
        selected, reason = conservative["candidate"], "joint-validation-default"
    return {"selected": selected, "reason": reason, "proposed_ratios": ratios,
            "summary": summary(ratios)}


def candidate_wire(candidate: Any) -> Any:
    if isinstance(candidate, int):
        return {"kind": "scalar", "value": candidate}
    if isinstance(candidate, tuple):
        return {"kind": "tiles", "row": candidate[0], "col": candidate[1]}
    return candidate


def extent_wire(decision: dict[str, Any]) -> dict[str, Any]:
    result = copy.deepcopy(decision)
    result["selected"] = candidate_wire(result["selected"])
    for score in result["scores"]:
        score["candidate"] = candidate_wire(score["candidate"])
    for schedule_class in result["schedule_classes"]:
        schedule_class["members"] = [candidate_wire(item)
                                     for item in schedule_class["members"]]
        schedule_class["representative"] = candidate_wire(
            schedule_class["representative"])
    return result


def quantile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    if fraction == 0.5 and len(ordered) % 2 == 0:
        upper = len(ordered) // 2
        return (ordered[upper - 1] + ordered[upper]) / 2
    rank = math.ceil(fraction * len(ordered))
    return ordered[max(1, min(len(ordered), rank)) - 1]


def threshold_selection(points: list[dict[str, Any]], default: int,
                        upper_floor: int | None) -> dict[str, Any]:
    wins = []
    comparisons = []
    for point in points:
        conservative, asymptotic = point["conservative"], point["asymptotic"]
        band = max(conservative["relative_iqr"], asymptotic["relative_iqr"])
        margin = (conservative["median"] - asymptotic["median"]) / conservative["median"]
        comparisons.append((band, margin))
        wins.append(margin > band)
    if not any(wins):
        return {"kind": "kept_default", "value": default,
                "reason": {"kind": "no_grid_point_wins"}}
    first = wins.index(True)
    if False in wins[first + 1:]:
        later = first + 1 + wins[first + 1:].index(False)
        return {"kind": "kept_default", "value": default,
                "reason": {"kind": "non_monotone", "first_win": points[first]["size"],
                           "later_loss": points[later]["size"]}}
    band, margin = comparisons[first]
    value = (points[first]["size"] if upper_floor is None else
             upper_floor if first == 0 else points[first - 1]["size"])
    return {"kind": "crossover", "value": value, "crossover": points[first]["size"],
            "band": band, "margin": margin}


def conservative_core_selectors() -> dict[str, Any]:
    root = Path(__file__).resolve().parents[2]
    envelope = load_json(root / "crates/gf2-core/data/tuning-profiles/conservative.json",
                         canonical=False)
    return envelope["sections"]["gf2-core/selectors"]["selectors"]


def validate_threshold_decisions(core_index: Any, core_decisions: Any) -> None:
    selectors = conservative_core_selectors()
    actual = {(item["field"], item["variant"]): item
              for item in core_decisions["retained_thresholds"]}
    require(len(actual) == 18, "owner threshold decision count mismatch")
    computed: dict[tuple[str, str], dict[str, Any]] = {}
    for field, grid in THRESHOLD_GRIDS.items():
        variants = [("generic", "generic_interpolation"),
                    ("two_adic", "two_adic_interpolation")] if field.endswith(
                        "interpolate_fast_min_points") else [("standard", "standard")]
        family, leaf = field.split(".", 1)
        default = selectors[family][leaf]
        upper_floor = (1 if field == "ple.panel_base_max_cols" else 0
                       if field in {"polynomial.karatsuba_max_out_len",
                                    "bit_matrix.transpose_simple_max_blocks"} else None)
        for stratum_variant, enum_variant in variants:
            points = []
            for size in grid:
                arms = []
                operand = None
                for arm in ["conservative", "asymptotic"]:
                    samples = []
                    for task in range(1, 6):
                        result = core_index[(field, f"size-{size}-{stratum_variant}", arm, task)]
                        samples.extend(ns_per_call(sample) for sample in result["samples"])
                        payload = embedded(result["payload"], "threshold payload")
                        require(payload.get("experiment") == "threshold",
                                "threshold coordinate has extent payload")
                        report = payload.get("report", {})
                        this_operand = report.get("operand_digest")
                        require(isinstance(this_operand, str) and SHA.fullmatch(this_operand),
                                "threshold operand digest missing")
                        require(operand is None or operand == this_operand,
                                "threshold operand identity changes across executions")
                        operand = this_operand
                    ordered = sorted(samples)
                    median = quantile(ordered, 0.5)
                    low, high = quantile(ordered, 0.25), quantile(ordered, 0.75)
                    arms.append({"median": median, "relative_iqr": (high - low) / median})
                left_probe = embedded(
                    core_index[(field, f"size-{size}-{stratum_variant}", "conservative", 0)]["payload"],
                    "threshold conservative probe")
                right_probe = embedded(
                    core_index[(field, f"size-{size}-{stratum_variant}", "asymptotic", 0)]["payload"],
                    "threshold asymptotic probe")
                require(left_probe["report"]["operand_digest"] == right_probe["report"]["operand_digest"],
                        "paired threshold arms use different operands")
                require(left_probe["report"]["operand_digest"] == operand,
                        "threshold probe/timed operands differ")
                points.append({"size": size, "conservative": arms[0], "asymptotic": arms[1]})
            selection = threshold_selection(points, default, upper_floor)
            enum_field = THRESHOLD_ENUMS[field]
            require((enum_field, enum_variant) in actual,
                    f"missing threshold decision {field}/{enum_variant}")
            close_numeric(selection, actual[(enum_field, enum_variant)]["selection"],
                          f"threshold selection {field}/{enum_variant}")
            computed[(enum_field, enum_variant)] = selection
    generic = computed[("interpolate_fast_min_points", "generic_interpolation")]
    two_adic = computed[("interpolate_fast_min_points", "two_adic_interpolation")]
    default = selectors["polynomial"]["interpolate_fast_min_points"]
    if generic["kind"] == two_adic["kind"] == "crossover":
        reconciled = generic if generic["value"] >= two_adic["value"] else two_adic
    elif generic["kind"] == "kept_default":
        reconciled = {"kind": "kept_default", "value": default, "reason": generic["reason"]}
    else:
        reconciled = {"kind": "kept_default", "value": default, "reason": two_adic["reason"]}
    close_numeric(reconciled,
                  actual[("interpolate_fast_min_points", "standard")]["selection"],
                  "reconciled interpolation threshold")


def close_numeric(expected: Any, actual: Any, where: str) -> None:
    if isinstance(expected, float) or isinstance(actual, float):
        require(isinstance(expected, (int, float)) and isinstance(actual, (int, float))
                and math.isclose(float(expected), float(actual), rel_tol=2e-12, abs_tol=0.0),
                f"{where} numeric mismatch: {expected} != {actual}")
    elif isinstance(expected, dict) and isinstance(actual, dict):
        require(set(expected) == set(actual), f"{where} keys differ")
        for key in expected:
            close_numeric(expected[key], actual[key], where + "." + key)
    elif isinstance(expected, list) and isinstance(actual, list):
        require(len(expected) == len(actual), f"{where} list length mismatch")
        for index, (left, right) in enumerate(zip(expected, actual)):
            close_numeric(left, right, f"{where}[{index}]")
    else:
        require(expected == actual, f"{where} mismatch: {expected!r} != {actual!r}")


def result_index(bundle: Any) -> dict[tuple[str, str, str, int], dict[str, Any]]:
    out = {}
    for entry in bundle["accepted"]:
        identity = entry["unit"]["identity"]
        key = (identity["field"], identity["stratum"], identity["candidate"],
               task_index(identity["task"]))
        require(key not in out, "duplicate accepted coordinate")
        out[key] = entry["result"]
    return out


def build_extent_series(index: dict[tuple[str, str, str, int], dict[str, Any]],
                        field: str, candidates: list[Any], strata: list[str]) -> list[dict[str, Any]]:
    result = []
    for candidate in candidates:
        label = (f"row-{candidate[0]}-col-{candidate[1]}" if isinstance(candidate, tuple)
                 else f"candidate-{candidate}")
        schedules, times, pairing = [], [], []
        for stratum in strata:
            probe = index[(field, stratum, label, 0)]
            payload = embedded(probe["payload"], "extent payload")
            require(payload.get("experiment") == "extent", "extent coordinate has threshold payload")
            observations = payload.get("observations")
            require(isinstance(observations, list) and len(observations) == 8
                    and all(x == observations[0] for x in observations),
                    "extent effective schedule is not stable across banks")
            schedules.append(observations[0]["schedule"])
            identity = (payload.get("seeds"), payload.get("dimensions"),
                        payload.get("operands_sha256"), payload.get("oracle_sha256"),
                        payload.get("result_sha256"))
            require(identity[-1] == identity[-2], "extent result differs from oracle")
            pairing.append(identity)
            executions = []
            for task in range(1, 6):
                timed = index[(field, stratum, label, task)]
                require(timed["payload"] == probe["payload"], "timed/probe extent evidence differs")
                executions.append(execution_median(timed))
            times.append(executions)
        result.append({"candidate": candidate, "schedules": schedules, "strata": times,
                       "pairing": pairing})
    for stratum_index in range(len(strata)):
        require(all(item["pairing"][stratum_index] == result[0]["pairing"][stratum_index]
                    for item in result), "candidate-paired operand/oracle identity differs")
    for item in result:
        del item["pairing"]
    return result


def validate_decisions(core_bundle: Any, algebra_bundle: Any,
                       core_response: Any, algebra_response: Any) -> dict[str, Any]:
    require_keys(core_response, ["operation", "artifact", "decisions"], "core owner response")
    require_keys(algebra_response, ["operation", "artifact", "decisions"],
                 "algebra owner response")
    require(core_response["operation"] == "emit-owner" and
            algebra_response["operation"] == "emit-owner", "owner responses are not emit-owner")
    core_decisions = embedded(core_response["decisions"], "core decisions")
    algebra_decisions = embedded(algebra_response["decisions"], "algebra decisions")
    require_keys(core_decisions, ["schema", "retained_thresholds", "extents", "gemm",
                                  "proposed_m4rm", "joint_m4rm", "measured", "omitted",
                                  "counts"], "core decisions")
    require_keys(algebra_decisions, ["schema", "field", "selected", "decision",
                                     "accepted_result_count"], "algebra decisions")
    require(core_decisions["schema"] == CORE_PROTOCOL
            and core_decisions["counts"] == OWNER_COUNTS["gf2-core"],
            "core decision schema/count mismatch")
    require(algebra_decisions["schema"] == "algebra-tuning-analysis-v1"
            and algebra_decisions["field"] == "permanent.gray_chunk_subsets",
            "algebra decision schema/field mismatch")
    measured = set(THRESHOLD_GRIDS) | (set(EXTENT_GRIDS) - {"gemm.tiles"}) | {
        "gemm.row_tile", "gemm.col_tile"}
    selectors = conservative_core_selectors()
    all_leaves = {f"{family}.{leaf}" for family, values in selectors.items()
                  for leaf in values}
    require(core_decisions["measured"] == sorted(measured)
            and len(core_decisions["omitted"]) == 10
            and set(core_decisions["omitted"]) == all_leaves - measured
            and measured | set(core_decisions["omitted"]) == all_leaves,
            "core measured/omitted selector complement mismatch")
    core_index = result_index(core_bundle)
    validate_threshold_decisions(core_index, core_decisions)
    recomputed = {}
    for field, candidates in EXTENT_GRIDS.items():
        if field == "gemm.tiles":
            strata = [f"shape-{shape}-{site}" for shape in range(3) for site in GEMM_SITES]
        else:
            strata = [f"shape-{shape}" for shape in range(EXTENT_STRATA[field])]
        series = build_extent_series(core_index, field, candidates, strata)
        if field != "gemm.tiles":
            recomputed[field] = analyze_extent(series, EXTENT_DEFAULTS[field])
    named = {item["field"]: item["decision"] for item in core_decisions["extents"]}
    enum_names = ["transpose", "soa", "m4rm_default_bytes", "m4rm_mid_bytes",
                  "m4rm_wide_bytes", "m4rm_wide_cap", "m4rm_small_cap", "trsm", "dot"]
    fields = [x for x in EXTENT_GRIDS if x != "gemm.tiles"]
    require(len(named) == len(fields) and len(core_decisions["extents"]) == len(fields),
            "core extent decision inventory mismatch")
    for enum_name, field in zip(enum_names, fields):
        require(enum_name in named, f"missing owner extent decision {enum_name}")
        expected = extent_wire(recomputed[field])
        # Rust's spelling is selected-nondefault (without the second hyphen).
        expected["reason"] = expected["reason"].replace("selected-non-default", "selected-nondefault")
        close_numeric(expected, named[enum_name], f"decision {field}")

    gemm_series = build_extent_series(
        core_index, "gemm.tiles", EXTENT_GRIDS["gemm.tiles"],
        [f"shape-{shape}-{site}" for shape in range(3) for site in GEMM_SITES])
    gemm_expected = analyze_gemm(gemm_series, (32, 64))
    close_numeric(gemm_expected, core_decisions["gemm"], "coupled GEMM decision")

    proposed = {
        "default_table_bytes": recomputed["m4rm.default_table_bytes"]["selected"],
        "mid_table_bytes": recomputed["m4rm.mid_table_bytes"]["selected"],
        "wide_table_bytes": recomputed["m4rm.wide_table_bytes"]["selected"],
        "wide_max_k": recomputed["m4rm.wide_max_k"]["selected"],
        "small_n_max_k": recomputed["m4rm.small_n_max_k"]["selected"],
    }
    require(core_decisions["proposed_m4rm"] == proposed,
            "owner proposed M4RM vector differs from one-factor decisions")
    joint_series = []
    for label in ["conservative", "proposed"]:
        schedules, strata, vector = [], [], None
        for shape in range(12):
            stratum = f"shape-{shape}"
            probe = core_index[("m4rm.joint", stratum, label, 0)]
            payload = embedded(probe["payload"], "joint M4RM payload")
            observations = payload["observations"]
            require(len(observations) == 8 and all(x == observations[0] for x in observations),
                    "joint M4RM schedule is unstable")
            schedules.append(observations[0]["schedule"])
            case = next(embedded(entry["unit"]["case"], "joint M4RM case")
                        for entry in core_bundle["accepted"]
                        if entry["unit"]["identity"] == probe["identity"])
            this_vector = case["kind"]["cell"]["candidate"]["values"]
            require(vector is None or vector == this_vector, "joint vector changed across strata")
            vector = this_vector
            executions = []
            for task in range(1, 6):
                timed = core_index[("m4rm.joint", stratum, label, task)]
                require(timed["payload"] == probe["payload"], "joint timed/probe evidence differs")
                executions.append(execution_median(timed))
            strata.append(executions)
        joint_series.append({"candidate": vector, "schedules": schedules, "strata": strata})
    require(joint_series[0]["candidate"] == {
        "default_table_bytes": 65536, "mid_table_bytes": 131072,
        "wide_table_bytes": 262144, "wide_max_k": 9, "small_n_max_k": 8,
    } and joint_series[1]["candidate"] == proposed, "derived M4RM vectors mismatch")
    joint_expected = analyze_joint(joint_series[0], joint_series[1])
    close_numeric(joint_expected, core_decisions["joint_m4rm"], "joint M4RM decision")

    algebra_index = result_index(algebra_bundle)
    series = []
    for candidate in ALGEBRA_CANDIDATES:
        schedules, strata = [], []
        for dimension in ALGEBRA_DIMS:
            label, stratum = f"q{candidate}", f"n{dimension}"
            probe = algebra_index[("permanent.gray_chunk_subsets", stratum, label, 0)]
            payload = embedded(probe["payload"], "algebra payload")
            total = (1 << dimension) - 1
            chunks = (total + candidate - 1) // candidate
            expected_partition = {"maximum_chunk_len": min(candidate, total), "chunk_count": chunks,
                                  "last_chunk_len": total - (chunks - 1) * candidate}
            require(payload["effective_partition"] == expected_partition
                    and payload["requested_chunk_subsets"] == candidate
                    and payload["observed_requested_chunk_subsets"] == candidate,
                    "algebra effective partition mismatch")
            schedules.append(expected_partition)
            times = []
            for task in range(1, 6):
                timed = algebra_index[("permanent.gray_chunk_subsets", stratum, label, task)]
                require(timed["payload"] == probe["payload"], "algebra timed/probe evidence differs")
                times.append(execution_median(timed))
            strata.append(times)
        series.append({"candidate": candidate, "schedules": schedules, "strata": strata})
    algebra_expected = analyze_extent(series, 65536)
    algebra_expected["reason"] = algebra_expected["reason"].replace("selected-non-default", "selected-nondefault")
    close_numeric(algebra_expected, algebra_decisions["decision"], "algebra decision")
    require(algebra_decisions["selected"] == algebra_expected["selected"]
            and algebra_decisions["accepted_result_count"] == 90,
            "algebra selected/count mismatch")
    return {"core": recomputed, "gemm": gemm_expected, "joint": joint_expected,
            "algebra": algebra_expected,
            "core_decisions": core_decisions, "algebra_decisions": algebra_decisions}


def identity_for(path: Path) -> dict[str, str]:
    return {"path": str(path), "sha256": digest(path.read_bytes())}


def byte_artifact(value: Any, where: str, stage: Path) -> tuple[Path, bytes]:
    require_keys(value, ["path", "sha256"], where)
    path = Path(value["path"])
    require(path.is_absolute() and path.resolve() == path and path.is_relative_to(stage)
            and path.is_file() and not path.is_symlink(),
            f"{where} path is not a canonical regular file inside stage")
    data = path.read_bytes()
    require(value["sha256"] == digest(data), f"{where} digest mismatch")
    return path, data


def validate_preflight_reports(stage: Path, campaign_config: Any) -> None:
    expected_names = [f"{owner}-{mode}" for owner in ["algebra", "core"]
                      for mode in ["capability-report", "list-grid", "self-check"]]
    reports = campaign_config["preflight_reports"]
    require(isinstance(reports, dict) and list(reports) == expected_names,
            "staged CLI preflight report coverage/order mismatch")
    core_inventory = None
    for name in expected_names:
        owner, mode = name.split("-", 1)
        path, response = artifact(reports[name], f"{name} preflight report", stage)
        require(path == stage / f"{name}.json", f"{name} preflight report path mismatch")
        require_keys(response, ["operation", "evidence"], f"{name} preflight response")
        require(response["operation"] == mode, f"{name} returned the wrong CLI report mode")
        evidence = embedded(response["evidence"], f"{name} preflight evidence")
        if owner == "algebra":
            require_keys(evidence, ["schema", "mode", "scope", "owner", "owner_protocol",
                                    "behavior_token", "seed_schema", "raw_sample_schema",
                                    "cells", "accepted_results", "windows", "details"],
                         f"{name} algebra evidence")
            scopes = {"self-check": "complete-declaration-no-measurement",
                      "list-grid": "full-grid-no-measurement",
                      "capability-report": "representative-capabilities-no-grid-execution"}
            require(evidence["schema"] == "algebra-tuning-owner-report-v1"
                    and evidence["mode"] == mode and evidence["scope"] == scopes[mode]
                    and evidence["owner"] == "gf2-algebra"
                    and evidence["owner_protocol"] == ALGEBRA_PROTOCOL
                    and evidence["behavior_token"] == ALGEBRA_BEHAVIOR
                    and evidence["seed_schema"] == "fixture-seeds-v3"
                    and evidence["raw_sample_schema"] == "raw-timing-samples-v3"
                    and {key: evidence[key] for key in
                         ["cells", "accepted_results", "windows"]} ==
                        {key: OWNER_COUNTS["gf2-algebra"][key] for key in
                         ["cells", "accepted_results", "windows"]}
                    and isinstance(evidence["details"], list)
                    and all(isinstance(item, str) for item in evidence["details"]),
                    f"{name} algebra evidence differs from the declared owner protocol")
            if mode == "self-check":
                expected_details = ["codec-inventory=1/1", "profile-installation=not-performed"]
            elif mode == "capability-report":
                expected_details = ["dedicated-pool-threads=4",
                                    f"required-features={FEATURES}",
                                    "full-grid-checked=false",
                                    "profile-installation=not-performed"]
            else:
                orders = []
                for execution in range(5):
                    order = list(ALGEBRA_CANDIDATES)
                    order = order[execution:] + order[:execution]
                    if execution % 2:
                        order.reverse()
                    orders.append(f"execution-{execution}={order}")
                expected_details = [f"dimensions={ALGEBRA_DIMS}",
                                    f"candidates={ALGEBRA_CANDIDATES}", *orders,
                                    "profile-installation=not-performed"]
            require(evidence["details"] == expected_details,
                    f"{name} algebra details differ from the declared report")
        elif mode in {"self-check", "list-grid"}:
            require_keys(evidence, ["owner_protocol", "behavior_token", "raw_schema",
                                    "measured", "counts", "retained", "extents",
                                    "reserved_joint_cells"], f"{name} core inventory")
            measured = sorted(set(THRESHOLD_GRIDS) | {
                "bit_matrix.transpose_macro_tile_blocks", "soa_batch.parallel_chunk_len",
                "m4rm.default_table_bytes", "m4rm.mid_table_bytes",
                "m4rm.wide_table_bytes", "m4rm.wide_max_k", "m4rm.small_n_max_k",
                "triangular.trsm_panel_rows", "gemm.row_tile", "gemm.col_tile",
                "field_vec.dot_chunk_len",
            })
            require(evidence["owner_protocol"] == CORE_PROTOCOL
                    and evidence["behavior_token"] == CORE_BEHAVIOR
                    and evidence["raw_schema"] == "raw-timing-samples-v3"
                    and evidence["measured"] == measured
                    and evidence["counts"] == OWNER_COUNTS["gf2-core"]
                    and evidence["reserved_joint_cells"] == 24
                    and evidence["retained"] == expected_core_retained_grid()
                    and evidence["extents"] == expected_core_extent_grid(),
                    f"{name} core inventory differs from the declared owner protocol")
            if core_inventory is None:
                core_inventory = evidence
            else:
                require(evidence == core_inventory,
                        "core --self-check and --list-grid inventories differ")
        else:
            require_keys(evidence, ["scope", "full_grid_probes", "timed_children",
                                    "fp251_whole_gemm", "simd_backend",
                                    "dedicated_pool_width", "representative_dot_length", "dot",
                                    "ordinary_companions", "cpu_features", "required_features",
                                    "required_threads"], f"{name} core capability evidence")
            require(evidence["scope"] == "representative-prerequisites"
                    and evidence["full_grid_probes"] is False
                    and evidence["timed_children"] == 0
                    and evidence["fp251_whole_gemm"] is True
                    and evidence["simd_backend"] in {"avx2", "avx512", "neon"}
                    and evidence["dedicated_pool_width"] == 4
                    and evidence["representative_dot_length"] ==
                        max(EXTENT_GRIDS["field_vec.dot_chunk_len"]) + 1
                    and evidence["ordinary_companions"] == {
                        "gemm_row_tile": EXTENT_DEFAULTS["gemm.tiles"][0],
                        "gemm_col_tile": EXTENT_DEFAULTS["gemm.tiles"][1],
                        "dot_chunk_len": EXTENT_DEFAULTS["field_vec.dot_chunk_len"],
                    }
                    and isinstance(evidence["cpu_features"], list)
                    and evidence["cpu_features"] == sorted(set(evidence["cpu_features"]))
                    and evidence["required_features"] == FEATURES
                    and evidence["required_threads"] == "4",
                    "core capability report differs from representative-only prerequisites")
            dot = evidence["dot"]
            require(isinstance(dot, list) and len(dot) == 3, "core dot capability count mismatch")
            for row, candidate in zip(dot, EXTENT_GRIDS["field_vec.dot_chunk_len"]):
                require_keys(row, ["candidate_chunk", "effective_chunk",
                                   "batch_clmul_scalar_clmul_and_barrett", "scalar_equal"],
                             "core dot capability")
                require(row == {"candidate_chunk": candidate, "effective_chunk": candidate,
                                "batch_clmul_scalar_clmul_and_barrett": True,
                                "scalar_equal": True},
                        "core dot capability did not exercise the requested chunk")


def validate_staging(stage: Path, campaign_config: Any) -> Any:
    staging = load_json(stage / "staging-manifest.json")
    require_keys(staging, ["schema", "source_before", "source_after", "executables"],
                 "staging manifest")
    expected_ids = ["algebra-producer", "composer", "core-producer", "driver"]
    require(staging["schema"] == "tuning-campaign-staging-v1"
            and list(staging["executables"]) == expected_ids,
            "staging manifest schema/executable inventory mismatch")
    observations = []
    for phase, field in [("before-build", "source_before"),
                         ("after-build", "source_after")]:
        path, observation = artifact(staging[field], f"staging {phase}", stage)
        require(path == stage / "build" / f"source-{phase.split('-')[0]}.json",
                f"staging {phase} artifact path mismatch")
        require_keys(observation, ["schema", "phase", "source_revision", "source_tree",
                                   "porcelain"], f"staging {phase} observation")
        require(observation == {
                    "schema": "tuning-campaign-build-source-v1",
                    "phase": phase,
                    "source_revision": campaign_config["identity"]["source_revision"],
                    "source_tree": campaign_config["source_tree"],
                    "porcelain": "",
                }, f"staging {phase} observation differs from campaign source identity")
        observations.append(observation)
    require(observations[0]["source_revision"] == observations[1]["source_revision"]
            and observations[0]["source_tree"] == observations[1]["source_tree"],
            "build source identity changed across the build")
    processes = {item["id"]: item for item in campaign_config["processes"]}
    for process_id in expected_ids:
        source = require_keys(staging["executables"][process_id], ["path", "sha256"],
                              f"staging source {process_id}")
        require(isinstance(source["path"], str),
                f"staging source {process_id} path must be text")
        source_path = Path(source["path"])
        require(source["path"] == str(source_path)
                and source_path.is_absolute()
                and "." not in source_path.parts and ".." not in source_path.parts
                and not source_path.is_relative_to(stage),
                f"staging source {process_id} path is not canonical external provenance")
        source_sha = require_sha(source["sha256"], f"staging source {process_id} digest")
        staged_path = stage / "bin" / process_id
        require(staged_path.is_file() and not staged_path.is_symlink()
                and digest(staged_path.read_bytes()) == source_sha
                and processes[process_id]["executable"] == str(staged_path)
                and processes[process_id]["executable_sha256"] == source_sha,
                f"staged {process_id} differs from its immutable build artifact")
    return staging


def expected_bootstrap_inputs(stage: Path, campaign_config: Any) -> dict[str, Any]:
    root = Path(__file__).resolve().parents[2]
    revision = campaign_config["identity"]["source_revision"]
    return {
        "revision": revision,
        "tree": git_output(root, "rev-parse", f"{revision}^{{tree}}"),
        "behavior": campaign_config["identity"]["behavior_sha256"],
        "receipt": campaign_config["receipt"],
        "runtime": campaign_config["runtime"],
        "processes": campaign_config["processes"],
        "protocol": campaign_config["protocol"],
        "validator": campaign_config["validator"],
        "affinity": campaign_config["affinity"],
        "staging": identity_for(stage / "staging-manifest.json"),
        "producing_manifest": campaign_config["producing_manifest"],
        "build_inputs": campaign_config["build_inputs"],
        "host_admission_policy": campaign_config["host_admission_policy"],
    }


def clean_process_outcome(value: Any, where: str) -> None:
    require(isinstance(value, dict), f"{where} must be an object")
    outcome = value.get("outcome")
    if outcome == "exited":
        require_keys(value, ["outcome", "pid", "exit_code", "elapsed_ns",
                             "all_descendants_reaped"], where)
        require(type(value["pid"]) is int and value["pid"] > 0
                and value["exit_code"] == 0 and type(value["elapsed_ns"]) is int
                and value["elapsed_ns"] > 0 and value["all_descendants_reaped"] is True,
                f"{where} is not a clean, reaped zero exit")
    else:
        fail(f"{where} is not an exited process")


def validate_budget_diagnostic(record: Any, process: str, boundary: str,
                               unit_key: str | None = None) -> None:
    require(record["event"] == "driver-diagnostic", "missing driver budget diagnostic")
    details = require_keys(record["details"], ["kind", "boundary", "process", "unit_key",
                                                    "active_elapsed_ns", "may_launch_child",
                                                    "session_budget_seconds",
                                                    "child_timeout_seconds",
                                                    "child_kill_grace_seconds"],
                           "driver budget diagnostic")
    active = details["active_elapsed_ns"]
    expected_may_launch = active <= ((SESSION_BUDGET_SECONDS - CHILD_TIMEOUT_SECONDS
                                      - CHILD_KILL_GRACE_SECONDS) * 1_000_000_000)
    require(details == {"kind": "session-budget-observation", "boundary": boundary,
                        "process": process, "unit_key": unit_key,
                        "active_elapsed_ns": active,
                        "may_launch_child": expected_may_launch,
                        "session_budget_seconds": SESSION_BUDGET_SECONDS,
                        "child_timeout_seconds": CHILD_TIMEOUT_SECONDS,
                        "child_kill_grace_seconds": CHILD_KILL_GRACE_SECONDS}
            and type(active) is int and active >= 0
            and (boundary != "before-launch" or expected_may_launch),
            "driver budget diagnostic differs from the fixed launch envelope")


def candidate_directory(path: Path, stage: Path, process: str, record: dict[str, Any],
                        where: str) -> Path:
    root = stage / "candidates"
    directory = path.parent
    require(directory.parent == root and directory.name ==
            f"{process}-{record['session_id']}-{record['sequence']}",
            f"{where} does not bind its process/session/journal sequence")
    return directory


def validate_owner_publication(stage: Path, campaign: str, name: str, owner: str,
                               response: Any, manifest: Any,
                               config: Any, records: list[dict[str, Any]]) -> tuple[Path, Any]:
    producer_id = f"{name}-producer"
    producer = next((item for item in config["processes"] if item["id"] == producer_id), None)
    require(producer is not None, f"missing {producer_id} descriptor")
    candidate_path, candidate = artifact(response["artifact"], f"{name} owner candidate",
                                         stage, canonical=False)
    require(candidate_path.name == "output.json", f"{name} owner candidate name mismatch")
    matching_writes = [record for record in records if record["event"] == "owner-write"
                       and record["details"].get("owner") == owner
                       and candidate_path.parent.name ==
                           f"{producer_id}-{record['session_id']}-{record['sequence']}"]
    require(len(matching_writes) == 1, f"{name} owner lacks one accepted write invocation")
    write = matching_writes[0]
    directory = candidate_directory(candidate_path, stage, producer_id, write,
                                    f"{name} owner candidate")
    request_path = directory / "request.json"
    request_identity = identity_for(request_path)
    require(write["details"] == {"owner": owner, "request": request_identity},
            f"{name} owner-write journal evidence mismatch")
    request = load_json(request_path)
    require_keys(request, ["operation", "request"], f"{name} owner request operation")
    require(request["operation"] == "emit-owner", f"{name} owner request operation mismatch")
    body = require_keys(request["request"], ["campaign_id", "manifest_sha256",
                                              "accepted_results", "measurement", "assembly",
                                              "output"], f"{name} owner request")
    accepted_path = stage / f"{name}-accepted.json"
    require(body["campaign_id"] == campaign
            and body["manifest_sha256"] == manifest["manifest_sha256"]
            and body["accepted_results"] == identity_for(accepted_path)
            and body["output"] == str(candidate_path),
            f"{name} owner request input/output binding mismatch")
    for provenance_name in ["measurement", "assembly"]:
        provenance = require_keys(body[provenance_name], ["identity", "process",
                                                           "observed_utc", "runtime"],
                                  f"{name} {provenance_name} provenance")
        require(provenance["identity"] == config["identity"]
                and provenance["process"] == producer_id
                and isinstance(provenance["observed_utc"], str)
                and provenance["observed_utc"].endswith("Z"),
                f"{name} {provenance_name} provenance identity mismatch")
    require(body["measurement"]["runtime"] == config["runtime"],
            f"{name} measurement runtime differs from observed campaign runtime")
    assembly_runtime = embedded(body["assembly"]["runtime"], f"{name} assembly runtime")
    require_keys(assembly_runtime, ["source_dirty", "tool", "tool_sha256"],
                 f"{name} assembly runtime")
    require(assembly_runtime == {"source_dirty": False,
                                 "tool": f"crates/gf2-{name}/benches/tuning_calibration.rs",
                                 "tool_sha256": producer["executable_sha256"]},
            f"{name} assembly runtime mismatch")
    response_path = directory / "response.json"
    require(response_path.read_bytes() == (stage / f"{name}-owner-response.json").read_bytes(),
            f"{name} durable response differs from accepted root response")
    require(load_json(response_path) == response, f"{name} candidate response mismatch")

    starts = [record for record in records if record["event"] == "orchestration-start"
              and record["details"].get("kind") == "orchestration-start"
              and record["details"].get("process") == producer_id
              and record["details"].get("request_sha256") == request_identity["sha256"]]
    require(len(starts) == 1 and starts[0]["sequence"] == write["sequence"] + 2
            and starts[0]["details"] == {
                "kind": "orchestration-start", "process": producer_id,
                "request_sha256": request_identity["sha256"],
                "arguments": ["--owner-operation"],
            },
            f"{name} owner orchestration-start mismatch")
    validate_budget_diagnostic(records[write["sequence"] + 1], producer_id,
                               "before-launch")
    exits = [record for record in records if record["event"] == "orchestration-exit"
             and record["details"].get("kind") == "orchestration-exit"
             and record["details"].get("process") == producer_id
             and record["details"].get("start_sequence") == starts[0]["sequence"]]
    require(len(exits) == 1, f"{name} owner orchestration-exit mismatch")
    clean_process_outcome(exits[0]["details"].get("outcome"),
                          f"{name} owner orchestration outcome")
    exit_evidence = load_json(directory / "exit.json")
    require_keys(exit_evidence, ["outcome", "stdout", "stderr"], f"{name} owner exit")
    stdout = bytes(exit_evidence["stdout"])
    stderr = bytes(exit_evidence["stderr"])
    expected_stdout = b"GF2_TUNING_RESULT=" + compact(response)
    require(exit_evidence["outcome"] == exits[0]["details"]["outcome"]
            and digest(stdout) == exits[0]["details"]["stdout_sha256"]
            and digest(stderr) == exits[0]["details"]["stderr_sha256"]
            and exits[0]["details"]["stderr"] == exit_evidence["stderr"]
            and stdout in {expected_stdout, expected_stdout + b"\n"},
            f"{name} durable exit streams differ from journal/response")

    canonical_path = stage / f"{name}-owner.json"
    require(canonical_path.is_file() and not canonical_path.is_symlink()
            and canonical_path.read_bytes() == candidate_path.read_bytes(),
            f"{name} canonical owner is not a byte-identical promotion")
    canonical_identity = identity_for(canonical_path)
    reopens = [record for record in records if record["event"] == "owner-reopen"
               and record["details"].get("owner") == owner]
    expected_reopen = {
                "owner": owner, "candidate": response["artifact"],
                "artifact": canonical_identity, "decisions": response["decisions"]}
    require(reopens and all(record["details"] == expected_reopen for record in reopens),
            f"{name} canonical promotion/reopen evidence mismatch")
    validate_envelope_hashes(candidate)
    validate_published_provenance(candidate, owner, body, config)
    return canonical_path, candidate


def validate_envelopes(stage: Path, campaign: str, core_response: Any,
                       algebra_response: Any, decisions: Any, core_manifest: Any,
                       algebra_manifest: Any, config: Any,
                       records: list[dict[str, Any]]) -> None:
    core_path, core = validate_owner_publication(
        stage, campaign, "core", "gf2-core", core_response, core_manifest, config, records)
    algebra_path, algebra = validate_owner_publication(
        stage, campaign, "algebra", "gf2-algebra", algebra_response, algebra_manifest,
        config, records)
    composition = load_json(stage / "composition.json")
    require_keys(composition, ["schema", "args", "source_revision", "source_dirty",
                               "tool_sha256", "core", "algebra", "candidate", "request",
                               "exit", "output"], "composition")
    require(composition["schema"] == "tuning-campaign-composition-v1"
            and composition["source_dirty"] is False
            and composition["source_revision"] == config["identity"]["source_revision"]
            and composition["tool_sha256"] ==
                config["identity"]["executable_sha256"]["composer"],
            "composition provenance mismatch")
    require(composition["core"] == identity_for(core_path)
            and composition["algebra"] == identity_for(algebra_path),
            "composition owner identities differ from canonical promotions")
    complete_path, complete = artifact(composition["output"], "complete", stage, canonical=False)
    require(complete_path == stage / "complete.json", "complete artifact path mismatch")
    candidate_path, candidate_complete = artifact(composition["candidate"],
                                                  "complete candidate", stage,
                                                  canonical=False)
    require(candidate_path.name == "output.json"
            and candidate_path.read_bytes() == complete_path.read_bytes(),
            "complete canonical artifact is not a byte-identical promotion")
    request_path, request = artifact(composition["request"], "composition request", stage)
    exit_path, exit_evidence = artifact(composition["exit"], "composition exit", stage)
    require(request_path.parent == candidate_path.parent == exit_path.parent,
            "composition request/exit/output are from different invocations")
    require_keys(request, ["schema", "process", "args", "core", "algebra"],
                 "composition request")
    composer = next((item for item in config["processes"] if item["id"] == "composer"), None)
    require(composer is not None and request["schema"] ==
            "tuning-campaign-composition-request-v1" and request["process"] == composer
            and request["core"] == composition["core"]
            and request["algebra"] == composition["algebra"],
            "composition request identity mismatch")
    require_keys(exit_evidence, ["outcome", "stdout", "stderr"], "composition exit")
    clean_process_outcome(exit_evidence["outcome"], "composition outcome")
    candidate_record_path = stage / "composition-candidate.json"
    candidate_response_path = candidate_path.parent / "response.json"
    candidate_record = load_json(candidate_record_path)
    expected_candidate_record = {key: value for key, value in composition.items()
                                 if key != "output"}
    require(candidate_record == expected_candidate_record
            and candidate_record_path.read_bytes() == candidate_response_path.read_bytes()
            and load_json(candidate_response_path) == candidate_record,
            "accepted composition response records differ")
    args = composition["args"]
    require(isinstance(args, list) and len(args) == 9 and args[0] == "complete"
            and args[1:4] == [str(core_path), str(algebra_path), str(candidate_path)]
            and args[4] == campaign and isinstance(args[5], str) and args[5].endswith("Z")
            and args[6] == composition["source_revision"]
            and args[7:] == ["false", composition["tool_sha256"]]
            and request["args"] == args,
            "composition command differs from exact owner inputs/provenance")
    composition_records = [record for record in records if record["event"] == "composition"
                           and record["details"].get("request") == composition["request"]]
    require(len(composition_records) == 1, "composition invocation journal evidence mismatch")
    candidate_directory(candidate_path, stage, "composer", composition_records[0],
                        "composition candidate")
    require(composition_records[0]["details"] == {
                "request": composition["request"], "args": args, "process": composer},
            "composition journal request differs from durable invocation")
    starts = [record for record in records if record["event"] == "orchestration-start"
              and record["details"] == {"kind": "orchestration-start",
                                         "process": "composer",
                                         "request": composition["request"]}]
    exits = [record for record in records if record["event"] == "orchestration-exit"
             and record["details"] == {"kind": "orchestration-exit",
                                        "process": "composer", "exit": composition["exit"],
                                        "outcome": exit_evidence["outcome"]}]
    require(len(starts) == len(exits) == 1
            and composition_records[0]["sequence"] < starts[0]["sequence"] < exits[0]["sequence"],
            "composition orchestration journal evidence mismatch")
    validate_budget_diagnostic(records[starts[0]["sequence"] - 1], "composer", "before-launch")
    validate_budget_diagnostic(records[exits[0]["sequence"] + 1], "composer", "after-result")
    reopens = [record for record in records if record["event"] == "composition-reopen"]
    expected_reopen = {"candidate": composition["candidate"],
                       "artifact": composition["output"]}
    require(reopens and all(record["details"] == expected_reopen for record in reopens),
            "composition promotion/reopen evidence mismatch")
    for envelope, ids in [(core, {"gf2-core/selectors"}),
                          (algebra, {"gf2-algebra/permanent"}),
                          (complete, {"gf2-core/selectors", "gf2-algebra/permanent"}),
                          (candidate_complete,
                           {"gf2-core/selectors", "gf2-algebra/permanent"})]:
        require(envelope.get("profile_format_version") == 2 and envelope.get("profile_id") == campaign
                and set(envelope.get("sections", {})) == ids, "profile envelope identity mismatch")
    require(complete["sections"]["gf2-core/selectors"] == core["sections"]["gf2-core/selectors"]
            and complete["sections"]["gf2-algebra/permanent"] == algebra["sections"]["gf2-algebra/permanent"],
            "composer changed canonical owner section wrappers")
    core_section = core["sections"]["gf2-core/selectors"]
    algebra_section = algebra["sections"]["gf2-algebra/permanent"]
    expected_receipt = f"dev/benchmarks/tuning_profiles/{campaign}.md"
    require(core_section["measurement"]["harness_schema"] == CORE_BEHAVIOR
            and core_section["measurement"]["receipt"] == expected_receipt,
            "core calibrated provenance/citation mismatch")
    require(algebra_section["measurement"]["harness_schema"] == ALGEBRA_BEHAVIOR
            and algebra_section["measurement"]["receipt"] == core_section["measurement"]["receipt"],
            "algebra calibrated provenance/citation mismatch")
    for envelope in [core, algebra, complete, candidate_complete]:
        validate_envelope_hashes(envelope)
    require(core_section["selectors"] == expected_published_selectors(decisions),
            "complete core selector section differs from all decisions and omission complement")
    require(algebra_section["selectors"] == {"permanent":{"gray_chunk_subsets":decisions["algebra_decisions"]["selected"]}},
            "complete algebra selector section differs from decision")
    require(complete["assembly"] == {"kind":"assembled","assembled_at":args[5],
                "source_revision":args[6],"source_dirty":False,"tool":"dev/tools/tuning-profile-compose",
                "tool_sha256":args[8],"content_sha256":envelope_content(campaign,complete["sections"])},
            "complete assembly differs from observed composer invocation")
    selected = core_section["selectors"]
    gemm = decisions["core_decisions"]["gemm"]["selected"]
    dot = next(x["decision"]["selected"] for x in decisions["core_decisions"]["extents"]
               if x["field"] == "dot")
    require_keys(dot, ["kind", "value"], "published dot decision")
    require([selected["gemm"]["row_tile"], selected["gemm"]["col_tile"]] == gemm
            and dot["kind"] == "scalar"
            and selected["field_vec"]["dot_chunk_len"] == dot["value"],
            "GEMM/dot owner publication differs from measured decisions")
    require(algebra_section["selectors"]["permanent"]["gray_chunk_subsets"] ==
            decisions["algebra_decisions"]["selected"], "algebra publication differs from decision")


def validate_derivation(stage: Path, original: Any, resolved: Any, decisions: Any,
                        core_bundle: Any) -> None:
    request = load_json(stage / "derived-request.json")
    derived = load_json(stage / "derived-manifest.json")
    require_keys(request, ["campaign_id", "original_manifest_sha256", "reserved_units",
                           "accepted_inputs"], "M4RM derivation request")
    require_keys(derived, ["original_manifest_sha256", "accepted_inputs_sha256", "units",
                           "derivation"], "M4RM derived manifest")
    require(request["original_manifest_sha256"] == original["manifest_sha256"]
            and request["reserved_units"] == original["ordered_units"][-144:],
            "M4RM derivation request changed reserved slots")
    input_path, partial = artifact(request["accepted_inputs"], "M4RM derivation input", stage)
    expected_partial = {"schema": INPUT_SCHEMA,
                        "manifest_sha256": original["manifest_sha256"],
                        "accepted": core_bundle["accepted"][:4068]}
    require(input_path == stage / "core-derivation-input.json" and partial == expected_partial,
            "M4RM derivation input is not the exact one-factor prefix")
    require(derived["original_manifest_sha256"] == original["manifest_sha256"]
            and derived["accepted_inputs_sha256"] == request["accepted_inputs"]["sha256"]
            and derived["units"] == resolved["ordered_units"][-144:],
            "derived M4RM units/bindings differ from resolved manifest")
    derivation = embedded(derived["derivation"], "M4RM derivation evidence")
    require_keys(derivation, ["schema", "decisions", "proposed"],
                 "M4RM derivation evidence")
    require(derivation["schema"] == "core-m4rm-conditional-vector-v1"
            and derivation["decisions"] == decisions["core_decisions"]["extents"]
            and derivation["proposed"] == decisions["core_decisions"]["proposed_m4rm"],
            "M4RM derivation proposal differs from independently recomputed decisions")


def checksum_artifact_boundary(stage: Path) -> set[Path]:
    excluded = {"execution.log", "active-session.json", "active-preparation.json",
                "session-writer.lock", "sessions", "artifact-publications"}
    artifacts: set[Path] = set()

    def collect(directory: Path) -> None:
        for path in directory.iterdir():
            if path.name in excluded:
                continue
            require(not path.is_symlink(), "symlink in staged checksum evidence")
            if path.is_dir():
                collect(path)
            else:
                require(path.is_file(), "non-file in staged checksum evidence")
                artifacts.add(path)

    collect(stage)
    return artifacts


def validate_sessions(stage: Path, log_data: bytes, records: list[dict[str, Any]],
                      record_ends: list[int], campaign_config: Any,
                      preterminal: bool) -> None:
    sessions = stage / "sessions"
    preparations = stage / "preparations"
    require(sessions.is_dir(), "missing sessions directory")
    require(preparations.is_dir(), "missing preparations directory")
    dirs = sorted(x for x in sessions.iterdir() if x.is_dir())
    preparation_dirs = sorted(x for x in preparations.iterdir() if x.is_dir())
    require(all(x.is_dir() and not x.is_symlink() for x in sessions.iterdir()),
            "sessions contains a non-directory entry")
    require(all(x.is_dir() and not x.is_symlink() for x in preparations.iterdir()),
            "preparations contains a non-directory entry")
    session_names = {x.name for x in dirs}
    preparation_by_name = {x.name: x for x in preparation_dirs}
    require(session_names <= set(preparation_by_name),
            "a durable session lacks its committed preparation")
    for name, directory in preparation_by_name.items():
        if name not in session_names:
            require(not any((directory / filename).exists()
                            for filename in ["intent.json", "finish.json", "committed.json"]),
                    f"uncommitted preparation {name} retains canonical state")
    require(dirs, "campaign has no session records")
    require(not (stage / "active-preparation.json").exists(),
            "campaign retains an incomplete active preparation")
    bootstrap = expected_bootstrap_inputs(stage, campaign_config)
    full_checksum = False
    for directory in dirs:
        preparation_directory = preparation_by_name[directory.name]
        descriptor = load_json(directory / "descriptor.json")
        require_keys(descriptor, ["schema", "preparer", "campaign_id", "session_id",
                                  "channels", "identity", "counts", "lock_path"],
                     f"session descriptor {directory.name}")
        preparer = require_keys(descriptor["preparer"], ["pid", "boot_id", "start_time_ticks"],
                                f"session preparer {directory.name}")
        require(descriptor["schema"] == LIFECYCLE_SCHEMA
                and descriptor["campaign_id"] == campaign_config["campaign_id"]
                and descriptor["session_id"] == directory.name
                and descriptor["channels"] == campaign_config["channels"]
                and descriptor["identity"] == campaign_config["identity"]
                and descriptor["counts"] == COUNTS
                and type(preparer["pid"]) is int and preparer["pid"] > 0
                and type(preparer["start_time_ticks"]) is int
                and preparer["start_time_ticks"] > 0
                and re.fullmatch(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}",
                                 preparer["boot_id"]) is not None,
                f"session descriptor {directory.name} identity mismatch")
        intent = load_json(preparation_directory / "intent.json")
        require_keys(intent, ["schema", "campaign_id", "session_id", "preparer", "channels",
                              "lock_path", "identity_inputs", "journal"],
                     f"preparation intent {directory.name}")
        require(intent["schema"] == "tuning-campaign-preparation-v1"
                and intent["campaign_id"] == campaign_config["campaign_id"]
                and intent["session_id"] == directory.name
                and intent["preparer"] == descriptor["preparer"]
                and intent["channels"] == campaign_config["channels"]
                and intent["lock_path"] == descriptor["lock_path"]
                and embedded(intent["identity_inputs"],
                             f"preparation identity inputs {directory.name}") == bootstrap,
                f"preparation intent {directory.name} identity mismatch")
        finish_path = preparation_directory / "finish.json"
        committed_path = preparation_directory / "committed.json"
        require(finish_path.read_bytes() == committed_path.read_bytes(),
                f"preparation {directory.name} was not committed byte-identically")
        finish = load_json(finish_path)
        require_keys(finish, ["descriptor", "config"], f"preparation finish {directory.name}")
        require(finish["descriptor"] == descriptor
                and finish["config"] == identity_for(stage / "campaign.json"),
                f"preparation finish {directory.name} descriptor/config mismatch")
        journal = intent["journal"]
        current_sequences = [record["sequence"] for record in records
                             if record["session_id"] == directory.name]
        require(current_sequences, f"preparation {directory.name} has no journal records")
        if journal.get("kind") == "initial":
            require_keys(journal, ["kind", "first_record"],
                         f"initial preparation journal {directory.name}")
            require(current_sequences[0] == 0 and journal["first_record"] == records[0]
                    and records[0]["event"] == "campaign-start",
                    f"initial preparation {directory.name} did not publish its exact first record")
        elif journal.get("kind") == "replacement":
            require_keys(journal, ["kind", "previous", "checksum"],
                         f"replacement preparation journal {directory.name}")
            first = current_sequences[0]
            previous = journal["previous"]
            require(first > 0 and previous == load_json(
                        sessions / records[first - 1]["session_id"] / "descriptor.json"),
                    f"replacement preparation {directory.name} does not bind the prior journal session")
            checksum_path, _ = artifact(journal["checksum"],
                                        f"prior session checksum {directory.name}", stage)
            require(checksum_path == sessions / previous["session_id"] / "checksum.json"
                    and load_json(sessions / previous["session_id"] / "retired.json")["checksum"]
                        == journal["checksum"],
                    f"replacement preparation {directory.name} lacks prior retirement evidence")
        else:
            fail(f"preparation {directory.name} has an unknown journal kind")
        lock_path = Path(descriptor["lock_path"])
        require(lock_path.is_absolute() and "." not in lock_path.parts
                and ".." not in lock_path.parts,
                f"session {directory.name} lock path is not absolute/normalized")
        prepared = [record for record in records if record["session_id"] == directory.name
                    and record["event"] == "session-prepared"]
        require(len(prepared) == 1
                and prepared[0]["details"] == {"session_descriptor": descriptor},
                f"session {directory.name} descriptor journal projection mismatch")
        affinity_records = [record for record in records
                            if record["session_id"] == directory.name
                            and record["event"] == "driver-diagnostic"
                            and record["details"].get("kind") == "cpu-affinity"]
        preparation_affinity = [record for record in affinity_records
                                if record["details"].get("phase") == "preparation"]
        require(preparation_affinity
                and all(record["details"] == {
                    "kind": "cpu-affinity", "phase": "preparation",
                    "observed": campaign_config["affinity"]}
                    and record["sequence"] < prepared[0]["sequence"]
                    for record in preparation_affinity),
                f"session {directory.name} preparation affinity evidence mismatch")
        transition_records = [record for record in records
                              if record["session_id"] == directory.name
                              and "session_transition" in record["details"]]
        for transition_sequence, record in enumerate(transition_records):
            transition = require_keys(record["details"]["session_transition"],
                                      ["descriptor_sha256", "sequence", "transition",
                                       "recovery"],
                                      f"session transition {directory.name}/{transition_sequence}")
            require(transition["descriptor_sha256"] == digest(compact(descriptor))
                    and transition["sequence"] == transition_sequence
                    and load_json(directory / f"transition-{transition_sequence:06}.json") ==
                        transition,
                    f"session {directory.name} transition file/journal mismatch")
        transition_files = sorted(directory.glob("transition-*.json"))
        require(len(transition_files) == len(transition_records),
                f"session {directory.name} has unprojected transition files")
        hold_records = [record for record in transition_records
                        if record["event"] == "lock-hold"]
        holds = [record["details"]["session_transition"]["transition"]
                 for record in hold_records]
        releases = [record["details"]["session_transition"]["transition"]
                    for record in transition_records if record["event"] == "lock-release"]
        if holds:
            require(len(holds) == 1 and holds[0].get("transition") == "lock-held"
                    and holds[0].get("evidence", {}).get("lock_path") == descriptor["lock_path"]
                    and type(holds[0]["evidence"].get("holder_pid")) is int
                    and holds[0]["evidence"]["holder_pid"] > 0,
                    f"session {directory.name} held-lock evidence mismatch")
            held_affinity = [record for record in affinity_records
                             if record["details"].get("phase") == "held-lock"]
            require(len(held_affinity) == 1
                    and held_affinity[0]["details"] == {
                        "kind": "cpu-affinity", "phase": "held-lock",
                        "observed": campaign_config["affinity"],
                        "expected": campaign_config["affinity"]}
                    and held_affinity[0]["sequence"] == hold_records[0]["sequence"] + 1,
                    f"session {directory.name} held-lock affinity evidence mismatch")
            protected_work = [record for record in records
                              if record["session_id"] == directory.name
                              and record["event"] in {"child-spawn", "orchestration-start"}
                              and record["sequence"] > prepared[0]["sequence"]]
            require(all(record["sequence"] > held_affinity[0]["sequence"]
                        for record in protected_work),
                    f"session {directory.name} launched work before held-lock affinity proof")
        else:
            require(not any(record["session_id"] == directory.name
                            and record["event"] in {"child-spawn", "orchestration-start"}
                            and record["sequence"] > prepared[0]["sequence"]
                            for record in records),
                    f"session {directory.name} launched work without a lock hold")
        require(len(affinity_records) == len(preparation_affinity) + len(holds),
                f"session {directory.name} has extra CPU affinity observations")
        if releases:
            independent = releases[0].get("evidence", {}).get("evidence", {})
            require(len(releases) == 1 and releases[0].get("transition") == "lock-release"
                    and independent.get("lock_path") == descriptor["lock_path"]
                    and independent.get("holder_dead") is True
                    and independent.get("descendants_dead") is True
                    and independent.get("lock_available") is True,
                    f"session {directory.name} release evidence mismatch")

        checksum_path = directory / "checksum.json"
        if not checksum_path.exists():
            require(preterminal and directory.name == records[-1]["session_id"],
                    f"closed session {directory.name} lacks checksum")
            continue
        checksum = load_json(checksum_path)
        require_keys(checksum, ["schema", "descriptor_sha256", "journal_prefix", "artifacts"],
                     f"session checksum {directory.name}")
        require(checksum["schema"] == "tuning-campaign-session-checksum-v1"
                and checksum["descriptor_sha256"] == digest(compact(descriptor)),
                f"session checksum {directory.name} identity mismatch")
        prefix = require_keys(checksum["journal_prefix"],
                              ["byte_len", "sha256", "record_count"],
                              f"session checksum prefix {directory.name}")
        length, count = prefix["byte_len"], prefix["record_count"]
        require(type(length) is int and type(count) is int and 0 < count <= len(records)
                and length == record_ends[count - 1]
                and prefix["sha256"] == digest(log_data[:length]),
                f"session checksum {directory.name} journal prefix mismatch")
        last = records[count - 1]
        require(last["session_id"] == directory.name
                and last["event"] in {"complete", "failed", "paused",
                                      "budget-exhausted", "interrupted"},
                f"session checksum {directory.name} does not end at its terminal")
        seen = set()
        for item in checksum["artifacts"]:
            require_keys(item, ["path", "sha256"], "session checksum artifact")
            path = Path(item["path"])
            require(path not in seen and path.is_absolute() and path.resolve() == path
                    and path.is_relative_to(stage) and path.is_file() and not path.is_symlink()
                    and digest(path.read_bytes()) == item["sha256"],
                    "session checksum artifact digest/path mismatch")
            seen.add(path)
        retired = load_json(directory / "retired.json")
        require_keys(retired, ["descriptor_sha256", "checksum"],
                     f"session retirement {directory.name}")
        require(retired["descriptor_sha256"] == checksum["descriptor_sha256"]
                and retired["checksum"] == identity_for(checksum_path),
                f"session retirement {directory.name} checksum binding mismatch")
        if length == len(log_data):
            require(seen == checksum_artifact_boundary(stage),
                    "final session checksum differs from the recursive immutable boundary")
        full_checksum |= length == len(log_data)
    replay_lifecycle(records, {directory.name:load_json(directory / "descriptor.json") for directory in dirs}, preterminal)
    require(preterminal or full_checksum, "no session checksum pins the final journal")
    active = stage / "active-session.json"
    require((preterminal and active.is_file()) or (not preterminal and not active.exists()),
            "active session claim does not match validation phase")
    if preterminal:
        require(load_json(active) ==
                load_json(sessions / records[-1]["session_id"] / "descriptor.json"),
                "active session claim differs from final session descriptor")


RECEIPT_TEMPLATE = '# Extent calibration {}\n\n## Campaign identity and protocol\n\nProtocol: `{}`; SHA-256 `{}`. Producing commit: `{}`.\n\n## Section-specific provenance and assembly\n\nSee `campaign.json`, owner responses and `composition.json` for runtime observations, executable and behavior identities, and strict codec evidence.\n\n## Grids, controls, and seed allocation\n\nThe immutable owner manifests contain every acquisition slot and opaque owner case. Each accepted payload contains its full seed, fixture, route and semantic witness.\n\n## Coverage, accounting, and resume history\n\nThe execution journal and checkpoint manifest are authoritative for attempts, accepted results, sessions, lock observations, censored intervals, and orchestration.\n\n## Effective routes and semantic witnesses\n\nSee each raw result payload below.\n\n## Raw samples and uncertainty\n\nEvery raw key resolves through `receipt-projection.json` raw_artifacts; five timing windows, calls and elapsed nanoseconds remain in each timed record.\n\n## Argmin and threshold decisions\n\nGEMM row/column decisions are joint; dot chunk decisions cite this campaign. Owner projections preserve ties, schedule plateaus, cross-stratum conflicts, conditional M4RM decisions and fallbacks:\n\n```json\n{}\n```\n\n## Owner and complete validation\n\nOwner responses record strict owner-only reopen. Composition preserves each complete section wrapper. Independent validation recomputes the estimators and evidence accounting.\n\n## Limitations\n\nMeasured choices are conditional on this host, declared grid, controls, and protocol. Unmeasured leaves remain omissions. Timing intervals are empirical measurements, not Monte Carlo probability estimates.\n\n## Raw result index\n\n'

def validate_receipt_text(text: str, projection: Any, config: Any, bundles: list[Any]) -> None:
    """Bind every visible byte, numerical claim and raw row to checked evidence."""
    match=re.search(r'```json\n(.*?)\n```',text,re.S)
    require(match is not None,'receipt decision block missing')
    decisions=load_json_bytes(match.group(1).encode(),'receipt decision block',canonical=False)
    require(decisions==projection['owners'],'receipt decisions differ from independently recomputed owners')
    prefix=RECEIPT_TEMPLATE.format(config['campaign_id'],config['protocol']['path'],
                                   config['protocol']['sha256'],config['identity']['source_revision'],
                                   match.group(1))
    expected=prefix+f"Preparation CPU affinity: `{config['affinity']}`. Held-lock observations are recorded in each session journal and must equal this set.\n\n"
    for manifest in config['manifests']:
        expected+=f"Owner `{manifest['owner']}` uses protocol `{manifest['owner_protocol']}` and behavior `{manifest['behavior_token']}`; executable `{manifest['processes'][0]['executable_sha256']}`.\n\n"
    c=projection['counts']
    expected+=f"Accepted accounting: {c['cells']} cells, {c['probes']} probes, {c['timed_children']} timed children, {c['accepted_results']} accepted results, {c['windows']} raw windows, {c['progress_records']} timing progress records. Observed {projection['attempts']} attempts, {projection['orchestration']} orchestration actions, {projection['sessions']} sessions at the receipt projection journal_sequence. Later finalization and resume events remain in the authoritative journal.\n\n"
    for bundle in bundles:
        for entry in bundle['accepted']:
            unit=entry['unit'];identity=unit['identity'];task=task_index(identity['task'])
            task_text='Probe' if task==0 else f'Measure {{ execution: {task-1} }}'
            expected+=f"- `{unit['key']}`: `{identity['field']}` / `{identity['stratum']}` / `{identity['candidate']}` / `{task_text}`\n"
    require(text==expected,'receipt text/numerical row differs from deterministic evidence projection')


def validate_receipt(stage: Path, campaign: str, core_bundle: Any, algebra_bundle: Any,
                     campaign_config: Any, core_response: Any, algebra_response: Any,
                     records: list[dict[str, Any]], checkpoint_files: dict[str, Any]) -> None:
    receipt = stage / "receipt.md"
    require(receipt.is_file() and not receipt.is_symlink(), "missing staged receipt.md")
    text = receipt.read_text(encoding="utf-8")
    for witness in [campaign, "a83583e0", "GEMM", "dot", "4302", "17925", "21510",
                    CORE_PROTOCOL, ALGEBRA_PROTOCOL, CORE_BEHAVIOR, ALGEBRA_BEHAVIOR]:
        require(witness in text, f"receipt lacks required witness {witness!r}")
    affinity = validate_affinity(campaign_config["affinity"], "campaign affinity")
    require(f"Preparation CPU affinity: `{affinity}`" in text,
            "receipt lacks the exact preparation CPU affinity")
    # Every reported numeric table can resolve to immutable raw keys when the
    # receipt cites unit keys; require both publication-critical families.
    keys = {entry["unit"]["key"] for entry in core_bundle["accepted"]}
    cited = {token for token in re.findall(r"[0-9a-f]{64}", text) if token in keys}
    gemm_keys = {entry["unit"]["key"] for entry in core_bundle["accepted"]
                 if entry["unit"]["identity"]["field"] == "gemm.tiles"}
    dot_keys = {entry["unit"]["key"] for entry in core_bundle["accepted"]
                if entry["unit"]["identity"]["field"] == "field_vec.dot_chunk_len"}
    require(cited & gemm_keys and cited & dot_keys, "receipt lacks resolvable GEMM/dot raw keys")
    projection = load_json(stage / "receipt-projection.json")
    require_keys(projection, ["schema", "campaign_id", "protocol", "identity", "runtime",
                              "affinity",
                              "owners", "counts", "attempts", "orchestration", "sessions",
                              "raw_artifacts", "journal_sequence", "raw_keys"],
                 "receipt projection")
    expected_keys = [entry["unit"]["key"] for bundle in (core_bundle, algebra_bundle)
                     for entry in bundle["accepted"]]
    expected_artifacts = []
    for key in expected_keys:
        checkpoint, checkpoint_sha = checkpoint_files[key]
        expected_artifacts.append({
            "key": key,
            "path": str(stage / "checkpoints/units" / f"{digest(key.encode())}.json"),
            "sha256": checkpoint_sha,
        })
    journal_sequence = projection["journal_sequence"]
    require(type(journal_sequence) is int and 0 <= journal_sequence < len(records),
            "receipt projection journal boundary is invalid")
    require(records[journal_sequence]["event"] == "composition-reopen",
            "receipt projection was not sealed at the composition reopen boundary")
    receipt_records = records[:journal_sequence + 1]
    attempts = sum(record["event"] == "child-spawn" for record in receipt_records)
    orchestration = sum(record["event"] == "orchestration-start" for record in receipt_records)
    sessions = sum(record["event"] == "session-prepared" for record in receipt_records)
    require(projection["schema"] == "tuning-campaign-receipt-projection-v1"
            and projection["campaign_id"] == campaign
            and projection["protocol"] == campaign_config["protocol"]
            and projection["identity"] == campaign_config["identity"]
            and projection["runtime"] == campaign_config["runtime"]
            and projection["affinity"] == affinity
            and projection["owners"] == [core_response, algebra_response]
            and projection["counts"] == COUNTS
            and projection["attempts"] == attempts
            and projection["orchestration"] == orchestration
            and projection["sessions"] == sessions
            and projection["raw_artifacts"] == expected_artifacts
            and projection["raw_keys"] == expected_keys,
            "receipt projection differs from immutable campaign evidence")
    validate_receipt_text(text, projection, campaign_config, [core_bundle, algebra_bundle])


def validate_stage(stage_arg: str, preterminal: bool) -> dict[str, Any]:
    stage = Path(stage_arg)
    require(stage.is_absolute() and stage.exists() and not stage.is_symlink()
            and stage.resolve() == stage and stage.parent == Path("/tmp"),
            "--stage must be an existing canonical /tmp campaign directory")
    campaign_config = load_json(stage / "campaign.json")
    require_keys(campaign_config, ["schema", "campaign_id", "channels", "identity", "manifests",
                                   "processes", "runtime", "protocol", "validator", "receipt",
                                   "affinity", "source_tree", "producing_manifest",
                                   "build_inputs", "preflight_reports",
                                   "host_admission_policy"],
                 "campaign index")
    require(campaign_config.get("schema") == CAMPAIGN_SCHEMA, "campaign index schema mismatch")
    campaign = campaign_config.get("campaign_id")
    require(isinstance(campaign, str) and RUN_ID.fullmatch(campaign) is not None
            and stage.name == campaign, "campaign ID/stage name mismatch")
    identity = campaign_config.get("identity")
    require_keys(identity, ["protocol_digest", "source_revision", "source_sha256",
                            "ordered_work_manifest_sha256", "process_descriptors_sha256",
                            "executable_sha256", "behavior_sha256", "lifecycle_schema",
                            "lifecycle_behavior_sha256", "feature_contract", "thread_contract",
                            "host_identity"], "resume identity")
    require(identity.get("feature_contract") == FEATURES and identity.get("thread_contract") == THREADS
            and identity.get("lifecycle_schema") == LIFECYCLE_SCHEMA,
            "resume feature/thread/lifecycle contract mismatch")
    affinity = validate_affinity(campaign_config["affinity"], "campaign affinity")
    require_sha(identity.get("protocol_digest"), "protocol digest")
    require(isinstance(identity["source_revision"], str)
            and re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", identity["source_revision"])
            and SHA.fullmatch(identity["source_sha256"]) is not None
            and isinstance(identity["host_identity"], str)
            and identity["host_identity"].endswith(
                ";cpus=" + compact(affinity).decode())
            and identity["host_identity"].split(";cpus=", 1)[0],
            "source or host identity is malformed")
    root = Path(__file__).resolve().parents[2]
    tree = git_output(root, "rev-parse", f"{identity['source_revision']}^{{tree}}")
    require(campaign_config["source_tree"] == tree
            and digest(tree.encode()) == identity["source_sha256"],
            "producing revision/tree identity mismatch")
    protocol = Path(__file__).resolve().parents[1] / "active/a83583e0/premeasurement-protocol.md"
    require(digest(protocol.read_bytes()) == identity["protocol_digest"],
            "runtime protocol digest differs from reviewed declaration")
    require(campaign_config["protocol"] == {"path": str(protocol), "sha256": identity["protocol_digest"]},
            "campaign protocol ArtifactIdentity mismatch")
    validator = Path(__file__).resolve()
    require(campaign_config["validator"] == {"path": str(validator),
                                             "sha256": digest(validator.read_bytes())},
            "campaign validator ArtifactIdentity mismatch")
    require(campaign_config["receipt"] == f"dev/benchmarks/tuning_profiles/{campaign}.md",
            "campaign receipt destination mismatch")
    runtime = embedded(campaign_config["runtime"], "campaign runtime")
    require_keys(runtime, ["source_dirty", "toolchain", "cpu_model", "cpu_features",
                           "os_kernel", "governor", "receipt"], "campaign runtime")
    require(runtime["source_dirty"] is False and "rustc 1.95.0" in runtime["toolchain"]
            and isinstance(runtime["cpu_model"], str) and runtime["cpu_model"].strip()
            and isinstance(runtime["cpu_features"], list) and runtime["cpu_features"]
            and runtime["cpu_features"] == sorted(set(runtime["cpu_features"]))
            and isinstance(runtime["os_kernel"], str) and runtime["os_kernel"].strip()
            and runtime["receipt"] == campaign_config["receipt"],
            "campaign runtime lacks clean observed host/toolchain facts")
    governors = embedded(runtime["governor"], "campaign governor observation")
    require(isinstance(governors, dict) and governors
            and all(re.fullmatch(r"cpu[0-9]+", key) and isinstance(value, str) and value
                    for key, value in governors.items()),
            "campaign governor observation is empty or malformed")
    channels = campaign_config["channels"]
    require(channels == {"stage": str(stage), "execution_log": str(stage / "execution.log"),
                         "checkpoints": str(stage / "checkpoints")},
            "campaign channels mismatch")
    inputs = producing_inputs(root, identity["source_revision"])
    producing_path = (root / PRODUCING_MANIFEST).resolve()
    require(campaign_config["producing_manifest"] == {
                "path": str(producing_path), "sha256": digest(producing_path.read_bytes())},
            "campaign producing-input manifest identity mismatch")
    require(identity.get("behavior_sha256") ==
            source_hashes(root, inputs["behavior_sources"]),
            "behavior source inventory/digests differ from the producing manifest")
    require(campaign_config["build_inputs"] == source_hashes(root, inputs["build_inputs"]),
            "build-input inventory/digests differ from the producing manifest")
    require(identity["behavior_sha256"].get("dev/scripts/validate-tuning-extent-campaign.py") ==
            digest(validator.read_bytes()), "behavior manifest omits this validator")
    require(identity["process_descriptors_sha256"] == digest(compact(campaign_config["processes"])),
            "process descriptor digest mismatch")
    expected_processes = [("core-producer", ["--fresh-tuning-process-child"]),
                          ("algebra-producer", ["--fresh-child"]),
                          ("composer", []), ("driver", [])]
    require([(item.get("id"), item.get("arguments")) for item in campaign_config["processes"]]
            == expected_processes, "campaign process order/entry points mismatch")
    for item, (process_id, arguments) in zip(campaign_config["processes"], expected_processes):
        validate_process(item, process_id, arguments, stage)
    validate_staging(stage, campaign_config)
    validate_preflight_reports(stage, campaign_config)
    executable_map = {item["id"]: item["executable_sha256"] for item in campaign_config["processes"]}
    require(identity["executable_sha256"] == executable_map, "executable identity map mismatch")
    require(identity["lifecycle_behavior_sha256"] ==
            digest(compact(source_hashes(root, inputs["lifecycle_sources"]))),
            "lifecycle behavior manifest digest mismatch")
    require(campaign_config["host_admission_policy"] == HOST_ADMISSION_POLICY,
            "host-admission policy differs from the reviewed declaration")

    core_manifest = load_json(stage / "core-manifest.json")
    algebra_manifest = load_json(stage / "algebra-manifest.json")
    core_resolved = load_json(stage / "core-resolved-manifest.json")
    require(campaign_config.get("manifests") == [core_manifest, algebra_manifest],
            "campaign index manifests differ from sealed originals")
    work_projection = [core_manifest["ordered_units"], algebra_manifest["ordered_units"]]
    require(identity["ordered_work_manifest_sha256"] == digest(compact(work_projection)),
            "ordered-work identity digest mismatch")
    validate_manifest(core_manifest, "gf2-core", stage, False)
    validate_manifest(algebra_manifest, "gf2-algebra", stage, True)
    validate_manifest(core_resolved, "gf2-core", stage, True)
    require(core_resolved["ordered_units"][:-144] == core_manifest["ordered_units"][:-144]
            and [u["identity"] for u in core_resolved["ordered_units"][-144:]] ==
                [u["identity"] for u in core_manifest["ordered_units"][-144:]],
            "M4RM derivation changed fixed slots")

    checkpoint_files = validate_checkpoints(stage, campaign, identity)
    core_bundle = load_json(stage / "core-accepted.json")
    algebra_bundle = load_json(stage / "algebra-accepted.json")
    validate_bundle(core_bundle, core_resolved, checkpoint_files)
    validate_bundle(algebra_bundle, algebra_manifest, checkpoint_files)
    require(set(checkpoint_files) == {u["key"] for u in core_resolved["ordered_units"]}
            | {u["key"] for u in algebra_manifest["ordered_units"]},
            "checkpoint universe differs from both resolved manifests")

    records, log_data, record_ends = parse_journal(stage, campaign)
    validate_journal(records, log_data, record_ends, checkpoint_files, campaign_config,
                     preterminal)
    core_response = load_json(stage / "core-owner-response.json")
    algebra_response = load_json(stage / "algebra-owner-response.json")
    decisions = validate_decisions(core_bundle, algebra_bundle, core_response, algebra_response)
    validate_derivation(stage, core_manifest, core_resolved, decisions, core_bundle)
    validate_envelopes(stage, campaign, core_response, algebra_response, decisions,
                       core_resolved, algebra_manifest, campaign_config, records)
    validate_receipt(stage, campaign, core_bundle, algebra_bundle, campaign_config,
                     core_response, algebra_response, records, checkpoint_files)
    validate_sessions(stage, log_data, records, record_ends, campaign_config, preterminal)
    return {"schema": "tuning-extent-campaign-validation-v1", "campaign_id": campaign,
            "status": "preterminal-valid" if preterminal else "complete-valid",
            "cells": 717, "accepted_results": 4302, "windows": 17925,
            "execution_log_sha256": digest((stage / "execution.log").read_bytes()),
            "core_manifest_sha256": core_resolved["manifest_sha256"],
            "algebra_manifest_sha256": algebra_manifest["manifest_sha256"]}


def must_reject(action: Any, where: str) -> None:
    try:
        action()
    except (ValidationError, KeyError, TypeError, ValueError):
        return
    fail(f'{where} mutation was accepted')


def synthetic_execution_fixture(resumed: bool = False) -> tuple[list[Any],list[Any],dict[str,Any]]:
    """Independent small acquisition with real typed framing and all lifecycle boundaries.

    No timing work runs: durations are explicitly synthetic test values. The
    same replay functions validate these and full campaign records.
    """
    units=[]
    for ordinal,task in enumerate([{'kind':'probe'},{'kind':'measure','execution':0}]):
        identity={'protocol':CORE_PROTOCOL,'owner':'gf2-core','campaign_id':'synthetic-test',
                  'phase':'core-extents','field':'field_vec.dot_chunk_len','stratum':'shape-0',
                  'candidate':'candidate-128','task':task}
        case=compact({'identity':identity}).decode()
        units.append({'ordinal':ordinal,'identity':identity,'key':digest(compact(identity)),
                      'process':'core-producer','case':case,'expected_progress':0 if ordinal==0 else 6})
    records=[]
    descriptors={}
    def event(session,kind,details,case=None):
        record={'schema':JOURNAL_SCHEMA,'timestamp_utc':'2026-09-05T00:00:00Z','campaign_id':'synthetic-test',
                'session_id':session,'sequence':len(records),'event':kind,'details':details}
        if case is not None:
            record['case']=case
        records.append(record)
        return record
    def transition(session,kind,payload,event_name=None):
        event(session,event_name or kind,{'session_transition':{'transition':{'transition':kind,**payload}}})
    def start(session):
        descriptors[session]={'lock_path':'/tmp/campaign-test.lock'}
        event(session,'session-prepared',{})
        transition(session,'lock-held',{'evidence':{'lock_path':'/tmp/campaign-test.lock','holder_pid':12,'observation':'inherited'}},'lock-hold')
        event(session,'phase-start',{'phase':'core-extents','owner':'gf2-core'})
    def end(session,outcome):
        transition(session,'work-finished',{'evidence':{'outcome':outcome,'all_descendants_reaped':True,'active_elapsed_ns':2_000_000_000}})
        transition(session,'wrapper-returned',{'evidence':{'exit_code':0,'signal':None}})
        transition(session,'lock-release',{'evidence':{'kind':'independent','evidence':{'holder_dead':True,'descendants_dead':True,
             'lock_path':'/tmp/campaign-test.lock','observed_utc':'2026-09-05T00:00:00Z','lock_available':True}}})
        transition(session,'terminal',{'outcome':outcome},outcome)
    def child(session,unit,failed=False):
        start=event(session,'cell-start',{'key':unit['key'],'ordinal':unit['ordinal']},unit['identity'])
        attempt=f"attempt-{start['sequence']}"
        case_sha=digest(unit['case'].encode())
        event(session,'child-spawn',{'attempt':attempt,'unit_key':unit['key'],'case_sha256':case_sha,'pid':13},unit['identity'])
        if unit['ordinal'] and not failed:
            for progress_index in range(6):
                progress={'event':'calibration-complete','calls':4} if progress_index==0 else {'event':'window-complete','repetition':progress_index-1,'calls':4,'elapsed_ns':250_000_000}
                event(session,'execution-progress' if progress_index==0 else 'window-progress',
                      {'attempt':attempt,'record':{'schema':'tuning-campaign-progress-v1','identity':unit['identity'],
                       'case_sha256':case_sha,'progress':progress}},unit['identity'])
        outcome={'outcome':'exited','pid':13,'exit_code':1 if failed else 0,'elapsed_ns':1_500_000_000,'all_descendants_reaped':True}
        event(session,'child-exit',{'attempt':attempt,'unit_key':unit['key'],'outcome':outcome},unit['identity'])
        if not failed:
            event(session,'checkpoint-accepted',{'unit_key':unit['key']},unit['identity'])
            event(session,'cell-complete',{'key':unit['key']},unit['identity'])
    start('session-1')
    child('session-1',units[0])
    session='session-1'
    if resumed:
        child(session,units[1],True)
        end(session,'failed')
        session='session-2'
        start(session)
    child(session,units[1])
    event(session,'phase-complete',{'phase':'core-extents'})
    end(session,'complete')
    return records,units,descriptors


def reconstruction_self_test() -> None:
    default=conservative_core_selectors()
    require(m4rm_schedule(default['m4rm'],512,1024)[0]['panel_width']==9,'wide panel golden boundary')
    require(m4rm_schedule(default['m4rm'],2048,960)[0]['panel_width']==8,'small-N panel golden boundary')
    require(extent_dimensions('m4rm.joint',11)==[64,2048,960],'joint last shape boundary')
    require(extent_dimensions('m4rm.wide_table_bytes',0)==[64,512,4096],'wide band shape boundary')
    require(extent_dimensions('gemm.tiles',2)==[193,192,385],'GEMM shape boundary')
    core_path=Path(__file__).resolve().parents[2]/'crates/gf2-core/data/tuning-profiles/conservative.json'
    conservative=load_json(core_path,canonical=False)
    require(envelope_content(conservative['profile_id'],conservative['sections'])==conservative['assembly']['content_sha256'],
            'canonical format-2 content golden receipt')
    for field,grid in THRESHOLD_GRIDS.items():
        for size in [grid[0],grid[-1]]:
            for arm in ['conservative','asymptotic']:
                case={'identity':{'field':field},'kind':{'experiment':'threshold','spec':{'size':size,'arm':arm,'variant':'standard'}}}
                forced=expected_forced_selectors(case)
                require(len(flatten_selectors(forced))==37,'complete forced inventory')
                for change in threshold_forcing(case):
                    require(forced[change['family']][change['field']]==change['value'],'partial/full forcing relationship')
    # A compact independently spelled seed/LCG/hash witness, rather than two
    # calls to the same mixer pretending to establish deterministic evidence.
    require(list(itertools.islice(draws(0),2))==[1442695040888963407,1876011003808476466],'LCG golden sequence')
    first=operand_identity('gf2-algebra','permanent.gray_chunk_subsets',0)
    require(first!=operand_identity('gf2-algebra','permanent.gray_chunk_subsets',1),'algebra distinct shape operands')
    require(len(first)==64 and first!='0'*64,'algebra reconstructed input hash')
    require(operand_identity('gf2-core','bit_backend.simd_min_words',1,True)!=
            operand_identity('gf2-core','bit_backend.simd_min_words',2,True),'retained word-boundary operands')
    for resumed in [False,True]:
        records,units,descriptors=synthetic_execution_fixture(resumed)
        accounting=replay_acquisition(records,units)
        require(accounting['accepted']==2 and accounting['attempts']==2+int(resumed),'resumed attempt accounting')
        replay_lifecycle(records,descriptors,False)
        replay_lifecycle(records[:-1],descriptors,True)
        must_reject(lambda:replay_lifecycle(records[:-1],descriptors,False),'missing terminal')
        bad=copy.deepcopy(records)
        spawn=next(record for record in bad if record['event']=='child-spawn')
        spawn['details']['unit_key']=units[1]['key']
        must_reject(lambda:replay_acquisition(bad,units),'out-of-order acquisition')
        for mutation in ['schema','case_sha256','identity','calls','repetition','event']:
            bad=copy.deepcopy(records)
            progress=next(record for record in bad if record['event']=='window-progress')['details']['record']
            if mutation in {'schema','case_sha256'}: progress[mutation]='forged'
            elif mutation=='identity': progress['identity']=units[0]['identity']
            elif mutation=='calls': progress['progress']['calls']=MAX_CALLS+1
            elif mutation=='repetition': progress['progress']['repetition']=True
            else: progress['progress']['event']='calibration-complete'
            must_reject(lambda:replay_acquisition(bad,units),f'progress {mutation}')
        for mutation in ['duration','mutex','release','work-order']:
            bad=copy.deepcopy(records);desc=copy.deepcopy(descriptors)
            if mutation=='duration':
                next(record for record in bad if record['event']=='work-finished')['details']['session_transition']['transition']['evidence']['active_elapsed_ns']=10_800_000_000_001
            elif mutation=='mutex':
                next(record for record in bad if record['event']=='lock-hold')['details']['session_transition']['transition']['evidence']['lock_path']='/tmp/other.lock'
            elif mutation=='release':
                next(record for record in bad if record['event']=='lock-release')['details']['session_transition']['transition']['evidence']['evidence']['lock_available']=False
            else:
                work=next(record for record in bad if record['event']=='work-finished');bad.remove(work);bad.insert(0,work)
            must_reject(lambda:replay_lifecycle(bad,desc,False),f'lifecycle {mutation}')
        if resumed:
            bad=copy.deepcopy(records)
            failed=next(record for record in bad if record['event']=='child-exit' and record['details']['outcome']['exit_code']==1)
            failed['details']['outcome']['exit_code']=0
            must_reject(lambda:replay_acquisition(bad,units),'resampled durable completion')
    sample={'execution':0,'repetition':0,'calls':MAX_CALLS,'elapsed_ns':1}
    validate_sample(sample,0,0,None)
    must_reject(lambda:validate_sample({**sample,'calls':MAX_CALLS+1},0,0,None),'call upper bound')
    must_reject(lambda:validate_sample({**sample,'elapsed_ns':True},0,0,None),'boolean elapsed')


def self_test() -> None:
    reconstruction_self_test()
    require(mix_seed(0x5ECC9BF800000000, 27, 0, 0xC00) ==
            mix_seed(0x5ECC9BF800000000, 27, 0, 0xC00), "seed self-test")
    require(rotated(["a", "b", "c"], 1) == ["a", "c", "b"], "rotation self-test")
    try:
        load_json_bytes(b'{"a":1,"a":2}', "duplicate mutation")
    except ValidationError:
        pass
    else:
        fail("duplicate-key mutation was accepted")
    fast = [[4.0] * 5, [2.0] * 5, [3.0] * 5]
    require(curve(fast) == "unimodal", "U-shaped curve self-test")
    nonmonotone = [[1.0] * 5, [2.0] * 5, [1.0] * 5]
    require(curve(nonmonotone) == "non-monotone", "nonmonotone self-test")
    series = [{"candidate": c, "schedules": [c], "strata": [v]}
              for c, v in zip([1, 2, 3], fast)]
    decision = analyze_extent(series, 1)
    require(decision["selected"] == 2 and decision["reason"] == "selected-non-default",
            "argmin self-test")
    tied = copy.deepcopy(series)
    tied[1]["schedules"] = tied[2]["schedules"] = [9]
    require(analyze_extent(tied, 1)["reason"] == "structural-schedule-tie",
            "schedule plateau self-test")
    try:
        close_numeric({"a": 1}, {"a": 1, "forged": 2}, "unknown-field mutation")
    except ValidationError:
        pass
    else:
        fail("unknown decision field mutation was accepted")
    try:
        clean_process_outcome({"outcome": "exited", "pid": 7, "exit_code": 0,
                               "elapsed_ns": 1, "all_descendants_reaped": False},
                              "unreaped mutation")
    except ValidationError:
        pass
    else:
        fail("unreaped process mutation was accepted")
    try:
        validate_affinity([2, 2], "duplicate affinity mutation")
    except ValidationError:
        pass
    else:
        fail("duplicate CPU affinity mutation was accepted")
    repository = Path(__file__).resolve().parents[2]
    actual_inputs = producing_inputs(repository)
    require(len(actual_inputs["behavior_sources"]) > len(actual_inputs["lifecycle_sources"])
            and len(actual_inputs["build_inputs"]) > len(actual_inputs["behavior_sources"]),
            "real producing-input manifest does not distinguish its three boundaries")
    with tempfile.TemporaryDirectory(prefix="gf2-validator-inputs-", dir="/tmp") as temporary:
        root = Path(temporary)
        manifest_path = root / PRODUCING_MANIFEST
        manifest_path.parent.mkdir(parents=True)
        for relative in ["behavior.rs", "lifecycle.rs", "build.rs"]:
            (root / relative).write_text(relative, encoding="utf-8")
        valid_inputs = {"schema": PRODUCING_INPUTS_SCHEMA,
                        "behavior_sources": ["behavior.rs", "lifecycle.rs"],
                        "lifecycle_sources": ["lifecycle.rs"],
                        "build_inputs": ["behavior.rs", "build.rs", "lifecycle.rs"]}
        manifest_path.write_bytes(compact(valid_inputs))
        producing_inputs(root)
        mutated_inputs = copy.deepcopy(valid_inputs)
        mutated_inputs["behavior_sources"].append("absent.rs")
        manifest_path.write_bytes(compact(mutated_inputs))
        must_reject(lambda: producing_inputs(root), "absent producing input")
        mutated_inputs = copy.deepcopy(valid_inputs)
        mutated_inputs["lifecycle_sources"] = ["build.rs"]
        manifest_path.write_bytes(compact(mutated_inputs))
        must_reject(lambda: producing_inputs(root), "lifecycle subset")
    with tempfile.TemporaryDirectory(prefix="gf2-validator-staging-", dir="/tmp") as temporary:
        root = Path(temporary)
        stage = root / "stage"
        binary_directory = stage / "bin"
        source_directory = root / "build"
        binary_directory.mkdir(parents=True)
        source_directory.mkdir()
        processes = []
        staged = {}
        revision = "1" * 40
        tree = "2" * 40
        build_directory = stage / "build"
        build_directory.mkdir(parents=True)
        source_observations = {}
        for phase, field in [("before-build", "source_before"),
                             ("after-build", "source_after")]:
            source_path = build_directory / f"source-{phase.split('-')[0]}.json"
            source_path.write_bytes(compact({
                "schema": "tuning-campaign-build-source-v1", "phase": phase,
                "source_revision": revision, "source_tree": tree, "porcelain": ""}))
            source_observations[field] = identity_for(source_path)
        for index, process_id in enumerate(
                ["algebra-producer", "composer", "core-producer", "driver"]):
            data = f"immutable-staged-executable-{index}".encode()
            original = (source_directory / process_id).resolve()
            original.write_bytes(data)
            source_sha = digest(data)
            staged[process_id] = {"path": str(original), "sha256": source_sha}
            destination = binary_directory / process_id
            destination.write_bytes(data)
            processes.append({"id": process_id, "executable": str(destination),
                              "executable_sha256": source_sha})
        (stage / "staging-manifest.json").write_bytes(compact({
            "schema": "tuning-campaign-staging-v1", **source_observations,
            "executables": staged}))
        for path in source_directory.iterdir():
            path.unlink()
        source_directory.rmdir()
        staging_config = {"identity": {"source_revision": revision}, "source_tree": tree,
                          "processes": processes}
        validate_staging(stage, staging_config)
        source_after = build_directory / "source-after.json"
        after = load_json(source_after)
        after["source_tree"] = "3" * 40
        source_after.write_bytes(compact(after))
        manifest = load_json(stage / "staging-manifest.json")
        manifest["source_after"] = identity_for(source_after)
        (stage / "staging-manifest.json").write_bytes(compact(manifest))
        must_reject(lambda: validate_staging(stage, staging_config),
                    "changed post-build source")
        after["source_tree"] = tree
        source_after.write_bytes(compact(after))
        manifest["source_after"] = identity_for(source_after)
        (stage / "staging-manifest.json").write_bytes(compact(manifest))
        (binary_directory / "driver").write_bytes(b"mutated")
        try:
            validate_staging(stage, staging_config)
        except ValidationError:
            pass
        else:
            fail("staged executable mutation was accepted")
        nested = stage / "evidence" / "nested.json"
        nested.parent.mkdir()
        nested.write_bytes(b"{}")
        ignored_session = stage / "sessions" / "old" / "descriptor.json"
        ignored_session.parent.mkdir(parents=True)
        ignored_session.write_bytes(b"{}")
        ignored_publication = stage / "artifact-publications" / "diagnostic"
        ignored_publication.parent.mkdir()
        ignored_publication.write_bytes(b"partial")
        boundary = checksum_artifact_boundary(stage)
        require(nested in boundary
                and all(path in boundary for path in binary_directory.iterdir())
                and ignored_session not in boundary and ignored_publication not in boundary,
                "recursive checksum boundary self-test")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--stage", help="canonical absolute campaign stage")
    group.add_argument("--self-test", action="store_true")
    parser.add_argument("--preterminal", action="store_true",
                        help="validate immediately before Complete/checksum publication")
    args = parser.parse_args()
    try:
        if args.self_test:
            require(not args.preterminal, "--preterminal requires --stage")
            self_test()
            output = {"schema": "tuning-extent-campaign-validator-self-test-v1", "status": "pass"}
        else:
            output = validate_stage(args.stage, args.preterminal)
        print("GF2_TUNING_VALIDATION=" + compact(output).decode())
        return 0
    except (ValidationError, OSError, KeyError, TypeError, ValueError) as error:
        print(f"validate-tuning-extent-campaign: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
