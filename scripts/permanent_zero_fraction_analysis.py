#!/usr/bin/env python3
"""Render schema-3 permanent campaign comparisons from committed reference data.

The experiment's permanent_dataset verifier checks pooled evidence before this
consumer renders Wilson intervals, literature claims, and target comparisons.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Iterable


SCHEMA_VERSION = 3
Z_95 = 1.959963984540054
CAMPAIGN_ID_RE = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)*\Z")
CHECKSUM_RE = re.compile(r"([0-9a-f]{64}) (?: |\*)(.+)\Z")
SUMMARY_FIELDS = (
    "schema_version",
    "q",
    "n",
    "matrix_count",
    "permanent_zero_count",
    "permanent_point_estimate",
    "permanent_interval_lower",
    "permanent_interval_upper",
    "permanent_verdict",
    "determinant_state",
    "determinant_sample_count",
    "determinant_zero_count",
    "determinant_point_estimate",
    "determinant_interval_lower",
    "determinant_interval_upper",
    "determinant_verdict",
    "terminal_state",
    "halt_reason",
    "permanent_log_p_value",
    "determinant_log_p_value",
)
OUTPUT_FIELDS = (
    "campaign_id",
    "manifest_sha256",
    "q",
    "n",
    "terminal_state",
    "pooled_sample_count",
    "pooled_permanent_zero_count",
    "halt_reason",
    "dataset_permanent_verdict",
    "estimate",
    "wilson_95_lower",
    "wilson_95_upper",
    "prior_source_table",
    "prior_source_evidence",
    "prior_sample_count",
    "prior_zero_count",
    "prior_point_estimate",
    "prior_interval_kind",
    "prior_interval_level",
    "prior_interval_lower",
    "prior_interval_upper",
    "precision_classification",
    "prior_interval_relation",
    "interval_excludes_published",
)
HALT_REASONS = {"acceptance_failure", "backend_unavailable", "execution_failure"}
VERDICTS = {"accepted", "rejected"}
SUPPORTED_FIELDS = {3, 5, 7}


class AnalysisError(ValueError):
    """Raised when a dataset cannot be safely analysed."""


@dataclass(frozen=True)
class PriorRow:
    source_table: str
    source_evidence: str
    zero_count: int
    sample_count: int

    @property
    def point_estimate(self) -> float:
        return self.zero_count / self.sample_count

    @property
    def is_exact(self) -> bool:
        return self.source_evidence == "exact_enumeration"


@dataclass(frozen=True)
class CompletedCell:
    campaign_id: str
    manifest_sha256: str
    q: int
    n: int
    sample_count: int
    zero_count: int
    verdict: str
    estimate: float
    lower: float
    upper: float
    prior: PriorRow | None


def _error(message: str) -> AnalysisError:
    return AnalysisError(message)


def _read_json(path: Path) -> object:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except OSError as exc:
        raise _error(f"cannot read {path}: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise _error(f"invalid JSON in {path}: {exc}") from exc


def _integer(value: object, field: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise _error(f"{field} must be an integer")
    return value


def _safe_relative_path(value: str) -> str:
    path = PurePosixPath(value)
    if not value or path.is_absolute() or any(part in {"", ".", ".."} for part in path.parts):
        raise _error(f"unsafe checksum path {value!r}")
    return path.as_posix()


def _dataset_root(dataset: Path) -> Path:
    try:
        root = dataset.resolve(strict=True)
    except OSError as exc:
        raise _error(f"cannot resolve dataset root {dataset}: {exc}") from exc
    if not root.is_dir():
        raise _error("dataset root is not a directory")
    return root


def _declared_path(root: Path, relative_path: str) -> Path:
    return root / _safe_relative_path(relative_path)


def _read_declared_file(root: Path, relative_path: str) -> bytes:
    path = _declared_path(root, relative_path)
    if not path.is_file():
        raise _error(f"checksummed path is missing or unsafe: {relative_path}")
    try:
        return path.read_bytes()
    except OSError as exc:
        raise _error(f"cannot read checksummed path {relative_path}: {exc}") from exc


def _manifest_raw_paths(manifest: object) -> tuple[str, set[str]]:
    if not isinstance(manifest, dict) or manifest.get("schema_version") != SCHEMA_VERSION:
        raise _error("manifest schema_version is not supported")
    campaign_id = manifest.get("campaign_id")
    if not isinstance(campaign_id, str) or not CAMPAIGN_ID_RE.fullmatch(campaign_id):
        raise _error("manifest campaign_id is invalid")
    cells = manifest.get("cells")
    if not isinstance(cells, list) or not cells:
        raise _error("manifest cells must be a non-empty array")
    paths = {"manifest.json", "summary.csv", "execution.jsonl", "runs.json"}
    seen: set[tuple[int, int]] = set()
    for index, cell in enumerate(cells):
        if not isinstance(cell, dict):
            raise _error(f"manifest cell {index} must be an object")
        q = _integer(cell.get("q"), "manifest q")
        n = _integer(cell.get("n"), "manifest n")
        if q not in SUPPORTED_FIELDS or n <= 0 or (q, n) in seen:
            raise _error("invalid manifest cell")
        seen.add((q, n))
        paths.add(f"shards/q{q}-n{n:02}.jsonl")
    return campaign_id, paths


def verify_checksums(
    dataset: Path,
) -> tuple[str, str, object]:
    """Verify the manifest-defined raw integrity set and return its manifest."""

    root = _dataset_root(dataset)
    manifest_path = _declared_path(root, "manifest.json")
    manifest = _read_json(manifest_path)
    campaign_id, base_paths = _manifest_raw_paths(manifest)

    checksum_path = _declared_path(root, "checksums.sha256")
    try:
        checksum_lines = checksum_path.read_text(encoding="utf-8").splitlines()
    except OSError as exc:
        raise _error(f"cannot read {checksum_path}: {exc}") from exc
    if not checksum_lines:
        raise _error("checksums.sha256 is empty")

    recorded: dict[str, str] = {}
    for line_number, line in enumerate(checksum_lines, start=1):
        match = CHECKSUM_RE.fullmatch(line)
        if match is None:
            raise _error(f"invalid checksum entry on line {line_number}")
        digest, raw_path = match.groups()
        path = _safe_relative_path(raw_path)
        if path in recorded:
            raise _error(f"duplicate checksum entry for {path}")
        recorded[path] = digest

    missing_base = sorted(base_paths.difference(recorded))
    if missing_base:
        raise _error(f"checksums.sha256 is missing required entry {missing_base[0]}")
    for relative_path in base_paths:
        expected_digest = recorded[relative_path]
        actual_digest = hashlib.sha256(_read_declared_file(root, relative_path)).hexdigest()
        if actual_digest != expected_digest:
            raise _error(f"checksum mismatch for {relative_path}")

    required_paths = base_paths
    missing = sorted(required_paths.difference(recorded))
    unexpected = sorted(set(recorded).difference(required_paths))
    if missing:
        raise _error(f"checksums.sha256 is missing required entry {missing[0]}")
    if unexpected:
        raise _error(f"checksums.sha256 covers non-raw path {unexpected[0]}")

    for relative_path, expected_digest in recorded.items():
        actual_digest = hashlib.sha256(_read_declared_file(root, relative_path)).hexdigest()
        if actual_digest != expected_digest:
            raise _error(f"checksum mismatch for {relative_path}")
    return campaign_id, recorded["manifest.json"], manifest


def wilson_95(zero_count: int, sample_count: int, z: float = Z_95) -> tuple[float, float, float]:
    """Return p-hat and the protocol's two-sided 95% Wilson score interval."""

    if sample_count <= 0 or zero_count < 0 or zero_count > sample_count:
        raise _error("Wilson counts must satisfy 0 <= zero_count <= sample_count and N > 0")
    estimate = zero_count / sample_count
    z_squared = z * z
    denominator = 1.0 + z_squared / sample_count
    centre = (estimate + z_squared / (2.0 * sample_count)) / denominator
    half_width = (
        z
        * math.sqrt(
            estimate * (1.0 - estimate) / sample_count
            + z_squared / (4.0 * sample_count * sample_count)
        )
        / denominator
    )
    return estimate, max(0.0, centre - half_width), min(1.0, centre + half_width)


def _parse_nonnegative_int(row: dict[str, str], name: str, row_number: int) -> int:
    value = row.get(name, "")
    if not isinstance(value, str):
        raise _error(f"summary.csv row {row_number} {name} must be an integer")
    try:
        parsed = int(value)
    except ValueError as exc:
        raise _error(f"summary.csv row {row_number} {name} must be an integer") from exc
    if parsed < 0:
        raise _error(f"summary.csv row {row_number} {name} must be non-negative")
    return parsed


def _require_empty(row: dict[str, str], names: Iterable[str], row_number: int) -> None:
    for name in names:
        if row[name] != "":
            raise _error(f"summary.csv halted row {row_number} must leave {name} empty")


def read_summary(
    dataset: Path,
    campaign_id: str,
    manifest_sha256: str,
    manifest: object,
    prior_rows: dict[tuple[int, int], PriorRow],
) -> list[CompletedCell | dict[str, str]]:
    """Read every terminal pooled row after the raw data has authenticated."""

    cells = manifest["cells"]
    manifest_cells = {(_integer(cell["q"], "manifest q"), _integer(cell["n"], "manifest n")) for cell in cells}
    summary_path = dataset / "summary.csv"
    try:
        with summary_path.open("r", encoding="utf-8", newline="") as handle:
            reader = csv.DictReader(handle)
            if tuple(reader.fieldnames or ()) != SUMMARY_FIELDS:
                raise _error("summary.csv does not use the canonical header")
            rows = list(reader)
    except OSError as exc:
        raise _error(f"cannot read {summary_path}: {exc}") from exc
    if not rows:
        raise _error("summary.csv must contain at least one terminal row")

    result: list[CompletedCell | dict[str, str]] = []
    seen_cells: set[tuple[int, int]] = set()
    completed_fields = (
        "permanent_point_estimate",
        "permanent_interval_lower",
        "permanent_interval_upper",
        "permanent_verdict",
    )
    for row_number, row in enumerate(rows, start=2):
        if None in row:
            raise _error(f"summary.csv row {row_number} has too many columns")
        schema_version = _parse_nonnegative_int(row, "schema_version", row_number)
        q = _parse_nonnegative_int(row, "q", row_number)
        n = _parse_nonnegative_int(row, "n", row_number)
        sample_count = _parse_nonnegative_int(row, "matrix_count", row_number)
        zero_count = _parse_nonnegative_int(row, "permanent_zero_count", row_number)
        if schema_version != SCHEMA_VERSION or (q, n) not in manifest_cells or (q, n) in seen_cells:
            raise _error(f"summary.csv row {row_number} has invalid cell identity")
        if zero_count > sample_count:
            raise _error(f"summary.csv row {row_number} zero count exceeds the sample count")
        seen_cells.add((q, n))
        terminal_state = row["terminal_state"]
        if terminal_state == "completed":
            if not all(row[name] for name in completed_fields) or row["halt_reason"]:
                raise _error(f"summary.csv completed row {row_number} lacks terminal metrics")
            if row["permanent_verdict"] not in VERDICTS:
                raise _error(f"summary.csv completed row {row_number} has an invalid verdict")
            estimate, lower, upper = wilson_95(zero_count, sample_count, manifest["acceptance"]["wilson_z"])
            result.append(
                CompletedCell(
                    campaign_id=campaign_id,
                    manifest_sha256=manifest_sha256,
                    q=q,
                    n=n,
                    sample_count=sample_count,
                    zero_count=zero_count,
                    verdict=row["permanent_verdict"],
                    estimate=estimate,
                    lower=lower,
                    upper=upper,
                    prior=prior_rows.get((q, n)),
                )
            )
        elif terminal_state == "halted":
            _require_empty(row, completed_fields, row_number)
            if row["halt_reason"] not in HALT_REASONS:
                raise _error(f"summary.csv halted row {row_number} has an invalid halt reason")
            result.append(
                {
                    "campaign_id": campaign_id,
                    "manifest_sha256": manifest_sha256,
                    "q": str(q),
                    "n": str(n),
                    "terminal_state": terminal_state,
                    "pooled_sample_count": str(sample_count),
                    "pooled_permanent_zero_count": str(zero_count),
                    "halt_reason": row["halt_reason"],
                }
            )
        else:
            raise _error(f"summary.csv row {row_number} lacks a valid terminal_state")
    if seen_cells != manifest_cells:
        raise _error("summary.csv does not give every manifest cell one terminal state")
    return result


def _repository_root() -> Path:
    result = subprocess.run(
        ["git", "-C", str(Path(__file__).parent), "rev-parse", "--show-toplevel"],
        text=True, capture_output=True, check=True,
    )
    return Path(result.stdout.strip())


def _require_canonical_conformance(dataset: Path) -> None:
    """Run the schema owner after checksum verification and before analysis."""

    try:
        result = subprocess.run(
            [
                "cargo",
                "+1.95.0",
                "run",
                "--quiet",
                "-p",
                "permanent-statistics",
                "--release",
                "--bin",
                "permanent_dataset",
                "--",
                "verify",
                str(dataset),
            ],
            cwd=_repository_root(),
            text=True,
            capture_output=True,
            check=False,
        )
    except OSError as exc:
        raise _error(f"cannot invoke canonical dataset conformance: {exc}") from exc
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip()
        raise _error(f"canonical dataset conformance refused: {detail}")


def load_prior_rows(path: Path) -> dict[tuple[int, int], PriorRow]:
    """Load versioned q=3 source counts without embedding a duplicate table."""

    source_path = path
    try:
        lines = source_path.read_text(encoding="utf-8").splitlines()
    except OSError as exc:
        raise _error(f"cannot read prior source table {source_path}: {exc}") from exc
    reader = csv.DictReader(line for line in lines if not line.startswith("#"))
    required = {"q", "n", "source_table", "source_evidence", "source_zero_count", "source_n"}
    if reader.fieldnames is None or set(reader.fieldnames) != required.union(
        {
            "source_reported_p_hat",
            "p_hat_from_source_counts",
            "source_reported_precision",
            "reference_precision_kind",
            "reference_precision_value",
            "reference_interval_kind",
            "reference_interval_level",
            "reference_interval_lower",
            "reference_interval_upper",
        }
    ):
        raise _error("prior source table has an unexpected header")
    prior_rows: dict[tuple[int, int], PriorRow] = {}
    for row_number, row in enumerate(reader, start=2):
        try:
            q = int(row["q"])
            n = int(row["n"])
            zero_count = int(row["source_zero_count"])
            sample_count = int(row["source_n"])
        except ValueError as exc:
            raise _error(f"prior source table row {row_number} has invalid counts") from exc
        if q != 3 or n <= 0 or sample_count <= 0 or not 0 <= zero_count <= sample_count:
            raise _error(f"prior source table row {row_number} is invalid")
        key = (q, n)
        if key in prior_rows:
            raise _error(f"prior source table repeats q={q}, n={n}")
        prior_rows[key] = PriorRow(
            source_table=row["source_table"],
            source_evidence=row["source_evidence"],
            zero_count=zero_count,
            sample_count=sample_count,
        )
    return prior_rows


def _decimal(value: float) -> str:
    return f"{value:.12f}"


def _blank_output_row(cell: dict[str, str]) -> dict[str, str]:
    row = {field: "" for field in OUTPUT_FIELDS}
    row.update(cell)
    return row


def _completed_output_row(cell: CompletedCell) -> dict[str, str]:
    row = {field: "" for field in OUTPUT_FIELDS}
    row.update(
        {
            "campaign_id": cell.campaign_id,
            "manifest_sha256": cell.manifest_sha256,
            "q": str(cell.q),
            "n": str(cell.n),
            "terminal_state": "completed",
            "pooled_sample_count": str(cell.sample_count),
            "pooled_permanent_zero_count": str(cell.zero_count),
            "dataset_permanent_verdict": cell.verdict,
            "estimate": _decimal(cell.estimate),
            "wilson_95_lower": _decimal(cell.lower),
            "wilson_95_upper": _decimal(cell.upper),
        }
    )
    if cell.prior is None:
        return row
    prior = cell.prior
    if prior.is_exact:
        prior_lower = prior_upper = prior.point_estimate
        interval_kind = "exact_no_sampling_uncertainty"
        interval_level = "not_applicable"
        precision = "prior_exact"
    else:
        _, prior_lower, prior_upper = wilson_95(prior.zero_count, prior.sample_count)
        interval_kind = "derived_wilson_score"
        interval_level = "0.95"
        campaign_se = math.sqrt(cell.estimate * (1.0 - cell.estimate) / cell.sample_count)
        prior_se = math.sqrt(prior.point_estimate * (1.0 - prior.point_estimate) / prior.sample_count)
        if campaign_se < 0.9 * prior_se:
            precision = "exceeds_prior_precision"
        elif campaign_se <= 1.1 * prior_se:
            precision = "matches_prior_precision"
        else:
            precision = "below_prior_precision"
    overlaps = cell.lower <= prior_upper and prior_lower <= cell.upper
    row.update(
        {
            "prior_source_table": prior.source_table,
            "prior_source_evidence": prior.source_evidence,
            "prior_sample_count": str(prior.sample_count),
            "prior_zero_count": str(prior.zero_count),
            "prior_point_estimate": _decimal(prior.point_estimate),
            "prior_interval_kind": interval_kind,
            "prior_interval_level": interval_level,
            "prior_interval_lower": _decimal(prior_lower),
            "prior_interval_upper": _decimal(prior_upper),
            "precision_classification": precision,
            "prior_interval_relation": "overlap" if overlaps else "disjoint",
            "interval_excludes_published": "true"
            if not cell.lower <= prior.point_estimate <= cell.upper
            else "false",
        }
    )
    return row


def render_csv(cells: list[CompletedCell | dict[str, str]]) -> str:
    """Render every terminal cell in the stable analysis-table order."""

    rows = [
        _completed_output_row(cell) if isinstance(cell, CompletedCell) else _blank_output_row(cell)
        for cell in cells
    ]
    rows.sort(key=lambda row: (int(row["q"]), int(row["n"])))
    output: list[str] = []
    writer = csv.DictWriter(output := _CsvLines(), fieldnames=OUTPUT_FIELDS, lineterminator="\n")
    writer.writeheader()
    writer.writerows(rows)
    return "".join(output)


class _CsvLines(list[str]):
    def write(self, value: str) -> int:
        self.append(value)
        return len(value)


def _svg_number(value: float) -> str:
    return f"{value:.6f}"


def render_curve(campaign_id: str, manifest_sha256: str, q: int, cells: list[CompletedCell]) -> str:
    """Render one fixed-geometry SVG curve with Wilson error bars."""

    width, height = 800, 500
    left, right, top, bottom = 90, 40, 35, 65
    points = sorted(cells, key=lambda cell: cell.n)
    if not points:
        raise _error(f"cannot render q={q} without completed cells")
    min_n, max_n = points[0].n, points[-1].n
    x_min, x_max = (min_n - 0.5, max_n + 0.5) if min_n == max_n else (min_n, max_n)
    y_min = max(0.0, min(point.lower for point in points))
    y_max = min(1.0, max(point.upper for point in points))
    if y_min == y_max:
        y_min, y_max = max(0.0, y_min - 0.05), min(1.0, y_max + 0.05)
    horizontal = width - left - right
    vertical = height - top - bottom

    def x(value: int) -> float:
        return left + (value - x_min) * horizontal / (x_max - x_min)

    def y(value: float) -> float:
        return top + (y_max - value) * vertical / (y_max - y_min)

    path_data = " ".join(
        ("M" if index == 0 else "L") + _svg_number(x(point.n)) + " " + _svg_number(y(point.estimate))
        for index, point in enumerate(points)
    )
    lines = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" data-campaign-id="{campaign_id}" data-manifest-sha256="{manifest_sha256}" data-q="{q}">',
        f'  <title>campaign_id={campaign_id}; q={q}; 95% Wilson error bars</title>',
        '  <rect width="100%" height="100%" fill="white"/>',
        f'  <line x1="{left}" y1="{height - bottom}" x2="{width - right}" y2="{height - bottom}" stroke="black"/>',
        f'  <line x1="{left}" y1="{top}" x2="{left}" y2="{height - bottom}" stroke="black"/>',
        f'  <path d="{path_data}" fill="none" stroke="#1f4e79" stroke-width="2"/>',
        f'  <text x="{width / 2:.1f}" y="{height - 20}" text-anchor="middle" font-family="sans-serif" font-size="14">n</text>',
        f'  <text x="20" y="{height / 2:.1f}" transform="rotate(-90 20 {height / 2:.1f})" text-anchor="middle" font-family="sans-serif" font-size="14">p_hat</text>',
    ]
    for point in points:
        x_value = x(point.n)
        lower_y, estimate_y, upper_y = y(point.lower), y(point.estimate), y(point.upper)
        title = (
            f"n={point.n}; N={point.sample_count}; p_hat={_decimal(point.estimate)}; "
            f"wilson_95=[{_decimal(point.lower)},{_decimal(point.upper)}]"
        )
        lines.extend(
            [
                f'  <g data-n="{point.n}" data-sample-count="{point.sample_count}" data-estimate="{_decimal(point.estimate)}" data-wilson-95-lower="{_decimal(point.lower)}" data-wilson-95-upper="{_decimal(point.upper)}">',
                f"    <title>{title}</title>",
                f'    <line x1="{_svg_number(x_value)}" y1="{_svg_number(upper_y)}" x2="{_svg_number(x_value)}" y2="{_svg_number(lower_y)}" stroke="#1f4e79"/>',
                f'    <line x1="{_svg_number(x_value - 4)}" y1="{_svg_number(upper_y)}" x2="{_svg_number(x_value + 4)}" y2="{_svg_number(upper_y)}" stroke="#1f4e79"/>',
                f'    <line x1="{_svg_number(x_value - 4)}" y1="{_svg_number(lower_y)}" x2="{_svg_number(x_value + 4)}" y2="{_svg_number(lower_y)}" stroke="#1f4e79"/>',
                f'    <circle cx="{_svg_number(x_value)}" cy="{_svg_number(estimate_y)}" r="3" fill="#1f4e79"/>',
                "  </g>",
            ]
        )
    lines.append("</svg>")
    return "\n".join(lines) + "\n"


def write_outputs(dataset: Path, cells: list[CompletedCell | dict[str, str]]) -> Path:
    """Write all derived artefacts under the campaign-keyed derived directory."""

    campaign_id = next(
        cell.campaign_id if isinstance(cell, CompletedCell) else cell["campaign_id"] for cell in cells
    )
    manifest_sha256 = next(
        cell.manifest_sha256 if isinstance(cell, CompletedCell) else cell["manifest_sha256"] for cell in cells
    )
    output_dir = dataset / "derived" / campaign_id / "zero-fraction-analysis"
    payloads: dict[Path, str] = {output_dir / "cells.csv": render_csv(cells)}
    completed_by_q: dict[int, list[CompletedCell]] = {}
    for cell in cells:
        if isinstance(cell, CompletedCell):
            completed_by_q.setdefault(cell.q, []).append(cell)
    for q, completed_cells in sorted(completed_by_q.items()):
        payloads[output_dir / f"curve-q{q}.svg"] = render_curve(
            campaign_id, manifest_sha256, q, completed_cells
        )
    output_dir.mkdir(parents=True, exist_ok=True)
    for path, payload in payloads.items():
        path.write_bytes(payload.encode("utf-8"))
    return output_dir


def analyse(dataset: Path) -> Path:
    """Verify a dataset, derive its analysis artefacts, and return their directory."""

    root = _dataset_root(dataset)
    campaign_id, manifest_sha256, manifest = verify_checksums(root)
    _require_canonical_conformance(root)
    cells = read_summary(
        root,
        campaign_id,
        manifest_sha256,
        manifest,
        load_prior_rows(_repository_root() / _safe_relative_path(manifest["reference_data"]["reproduction_targets"])),
    )
    output = write_outputs(root, cells)
    claim = _read_json(_repository_root() / _safe_relative_path(manifest["reference_data"]["literature_claim"]))
    (output / "literature-claim.json").write_text(json.dumps(claim, indent=2) + "\n", encoding="utf-8")
    return output


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Render deterministic permanent-zero-fraction analysis")
    parser.add_argument("dataset", type=Path, help="published campaign directory")
    args = parser.parse_args(argv)
    try:
        output_dir = analyse(args.dataset)
    except AnalysisError as exc:
        print(f"permanent-zero-fraction analysis refused: {exc}", file=sys.stderr)
        return 1
    print(output_dir)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
