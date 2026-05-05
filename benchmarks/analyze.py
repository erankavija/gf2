#!/usr/bin/env python3
"""benchmarks/analyze.py — merge gf2-side and reference-side CSVs and
render side-by-side markdown tables with throughput ratios.

Both inputs share the schema documented in `benchmarks/README.md`:

    lib,operation,field,m,k,n,rank_regime,seed,wall_ns,throughput_ops

Where `lib` is one of {"gf2", "fflas-ffpack", "m4ri", "m4rie",
"linbox", "ntl", "flint"}. The first two are the canonical references
selected by `reference_lib_for()` for GF(p) (fflas-ffpack), GF(2)
(m4ri), and GF(2^m) for m≥2 (m4rie). The remaining libs (linbox, ntl,
flint) are secondary references — their rows merge into the CSV
without changing canonical selection, available for explicit per-cell
designations by the target-matrix story.

Usage
-----
    # Merge both sides:
    python3 benchmarks/analyze.py \\
        --gf2 dev/bench_results/2026-04-26-gf2.csv \\
        --reference dev/bench_results/2026-04-26-reference.csv \\
        --out dev/bench_results/2026-04-26-tables.md

    # gf2-only (reference cells will be marked PENDING):
    python3 benchmarks/analyze.py \\
        --gf2 dev/bench_results/2026-04-26-gf2.csv \\
        --out dev/bench_results/2026-04-26-tables.md

    # Self-test on a tiny synthetic CSV:
    python3 benchmarks/analyze.py --smoke

The script is intentionally stdlib-only (csv, argparse, pathlib,
collections, math) so it is reproducible from any Python 3.8+ host
without a venv.
"""

from __future__ import annotations

import argparse
import csv
import io
import sys
from collections import OrderedDict, defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Tuple

CSV_COLUMNS = [
    "lib",
    "operation",
    "field",
    "m",
    "k",
    "n",
    "rank_regime",
    "seed",
    "wall_ns",
    "throughput_ops",
]

# Stable display order for tables. Operations listed here are emitted
# in this sequence; anything unknown is appended in alphabetical order
# at the end so the script doesn't silently swallow new ops.
OPERATION_ORDER = [
    "fgemm",
    "matmul",  # m4ri spelling for fgemm on GF(2)
    "pluq",
    "echelon",
    "invert",
    "solve",
    "charpoly",
    "minpoly",
    "spmv",
    # Sparse ops added by protocol Amendment 2 (2026-05-04, jit:a3412e15).
    # Validator support is owned by 47698404, but presentation order is
    # set here so the rendered tables put related ops next to each other.
    "sparse-matmul",
    "sparse×dense",
    "sparse-elim",
]

# Field families. Mapping from the CSV `field` value to the family the
# table groups by. We keep `field` as the table heading verbatim — the
# family is only used to bucket fflas/m4ri counterparts together.
FIELD_FAMILY = {
    "GF(2)": "gf2",
    "GF(7)": "gfp_small",
    "GF(251)": "gfp_small",
    "GF(65521)": "gfp_medium",
    "GF(2^31-1)": "gfp_medium",
    "GF(2^4)": "gf2m",
    "GF(2^8)": "gf2m",
    "GF(2^16)": "gf2m",
    "GF(2^32)": "gf2m",
}


@dataclass(frozen=True)
class Cell:
    """Identity of a single bench cell — what we group rows by."""

    operation: str
    field: str
    m: int
    k: int
    n: int
    rank_regime: str

    def label(self) -> str:
        """Human-readable label used for the table row's leftmost cell."""
        if self.m == self.k == self.n:
            shape = f"n={self.n}"
        else:
            shape = f"{self.m}×{self.k}×{self.n}"
        if self.rank_regime in ("uniform", "uniform_rect"):
            return shape
        return f"{shape} [{self.rank_regime}]"


@dataclass
class Measurement:
    wall_ns: float
    throughput_ops: float
    seed: int


@dataclass
class CellRow:
    cell: Cell
    by_lib: Dict[str, Measurement] = field(default_factory=dict)


def _parse_int(s: str) -> int:
    return int(s)


def _parse_float(s: str) -> float:
    return float(s)


def read_csv(path: Path) -> List[CellRow]:
    """Read a CSV and return one `CellRow` per (cell, lib) merge.

    Multiple rows for the same `(lib, cell)` are averaged on `wall_ns`
    and `throughput_ops` (`seed` is taken from the last occurrence).
    """
    rows: Dict[Tuple[Cell, str], List[Tuple[float, float, int]]] = defaultdict(list)
    with path.open("r", newline="") as fh:
        reader = csv.DictReader(fh)
        missing = [c for c in CSV_COLUMNS if c not in (reader.fieldnames or [])]
        if missing:
            raise ValueError(
                f"{path}: missing CSV columns: {missing}; got {reader.fieldnames}"
            )
        for raw in reader:
            cell = Cell(
                operation=raw["operation"],
                field=raw["field"],
                m=_parse_int(raw["m"]),
                k=_parse_int(raw["k"]),
                n=_parse_int(raw["n"]),
                rank_regime=raw["rank_regime"],
            )
            lib = raw["lib"]
            wall_ns = _parse_float(raw["wall_ns"])
            tput = _parse_float(raw["throughput_ops"])
            seed = int(raw["seed"])
            rows[(cell, lib)].append((wall_ns, tput, seed))

    by_cell: "OrderedDict[Cell, CellRow]" = OrderedDict()
    for (cell, lib), measurements in rows.items():
        wall_avg = sum(m[0] for m in measurements) / len(measurements)
        tput_avg = sum(m[1] for m in measurements) / len(measurements)
        seed = measurements[-1][2]
        cr = by_cell.setdefault(cell, CellRow(cell=cell))
        cr.by_lib[lib] = Measurement(
            wall_ns=wall_avg, throughput_ops=tput_avg, seed=seed
        )
    return list(by_cell.values())


def merge(gf2_rows: List[CellRow], ref_rows: List[CellRow]) -> List[CellRow]:
    """Merge gf2-side and reference-side CellRows by their Cell key."""
    out: "OrderedDict[Cell, CellRow]" = OrderedDict()
    for r in gf2_rows:
        out[r.cell] = CellRow(cell=r.cell, by_lib=dict(r.by_lib))
    for r in ref_rows:
        existing = out.setdefault(r.cell, CellRow(cell=r.cell))
        for lib, meas in r.by_lib.items():
            existing.by_lib[lib] = meas
    return list(out.values())


def _operation_sort_key(op: str) -> Tuple[int, str]:
    if op in OPERATION_ORDER:
        return (OPERATION_ORDER.index(op), op)
    return (len(OPERATION_ORDER), op)


# Field ordering inside each operation block: small Fp → large Fp → small
# GF(2^m) → large GF(2^m) → GF(2). Falls back to alphabetical for any
# unrecognised field value so the renderer stays defensive.
FIELD_ORDER: List[str] = [
    "GF(7)",
    "GF(251)",
    "GF(65521)",
    "GF(2^31-1)",
    "GF(2^8)",
    "GF(2^16)",
    "GF(2^32)",
    "GF(2)",
]


def _field_sort_key(field: str) -> Tuple[int, str]:
    if field in FIELD_ORDER:
        return (FIELD_ORDER.index(field), field)
    return (len(FIELD_ORDER), field)


def group_by_op_field(rows: Iterable[CellRow]) -> "OrderedDict[Tuple[str, str], List[CellRow]]":
    """Bucket rows by `(operation, field)`, preserving a stable sort."""
    groups: Dict[Tuple[str, str], List[CellRow]] = defaultdict(list)
    for r in rows:
        groups[(r.cell.operation, r.cell.field)].append(r)

    def grp_key(
        item: Tuple[Tuple[str, str], List[CellRow]],
    ) -> Tuple[Tuple[int, str], Tuple[int, str]]:
        (op, field), _ = item
        return (_operation_sort_key(op), _field_sort_key(field))

    sorted_items = sorted(groups.items(), key=grp_key)
    out: "OrderedDict[Tuple[str, str], List[CellRow]]" = OrderedDict()
    for k, v in sorted_items:
        # Within a group, sort by (m, k, n, rank_regime) for stable tables.
        v.sort(
            key=lambda r: (r.cell.m, r.cell.k, r.cell.n, r.cell.rank_regime)
        )
        out[k] = v
    return out


def reference_lib_for(field_value: str, operation: Optional[str] = None) -> str:
    """Pick the reference lib name we expect for a given (operation, field) cell.

    GF(2) goes to M4RI, GF(2^m) for m ≥ 2 goes to M4RIE, everything
    else (GF(p) families) goes to fflas-ffpack. This is a
    presentation-only decision — multiple libs may legitimately
    co-exist in the same CSV, and `render_table` will surface
    whichever the canonical reference is for that field.

    Per-cell overrides are applied first when `operation` is supplied:
    the (matmul, GF(2^32)) cell is promoted to NTL by issue
    `b13799ac` (M4RIE caps at m ≤ 16). Calls without `operation`
    keep the field-only routing for backwards compatibility — they
    do not trigger the per-cell override path.

    Per-cell routing decisions for charpoly + minpoly (issue c3e79272):

    * charpoly × GF(p): canonical = fflas-ffpack. fflas-ffpack
      promotes charpoly via FFPACK::CharPoly with the n^3 normalizer,
      already published in 2026-04-26-reference.csv across all four
      reference primes at n ∈ {64, 256}. LinBox + FLINT + NTL rows
      exist as secondary-reference cross-checks (issues 79388011,
      73ab8eef) — they merge into the reference CSV stream but
      analyze.py renders only the fflas-ffpack column to keep the
      side-by-side narrow.

    * minpoly × GF(p): canonical = fflas-ffpack. fflas-ffpack
      promotes minpoly via FFPACK::MinPoly (added in 2026-05-04
      issue 5dea7457; rows in 2026-05-04-5dea7457-reference-extension.csv
      across all four primes at n ∈ {64, 256, 1024}). Before 5dea7457
      no fflas-ffpack minpoly rows existed; LinBox + FLINT minpoly
      rows continue to exist as secondary-reference cross-checks
      (LinBox 79388011, FLINT 73ab8eef). NTL is excluded from minpoly
      because it does not expose a user-facing matrix-minpoly API.

    A future per-cell override (e.g. designating LinBox as canonical
    for a specific charpoly cell where it is materially faster than
    fflas-ffpack) is not made here; that designation is owned by the
    SOTA target-matrix story (4c0d0202).
    """
    # Per-cell overrides — only applied when the caller supplied an
    # operation. Each entry corresponds to a single promoted
    # (operation, field) cell from a JIT issue's evidence doc.
    if operation == "matmul" and field_value == "GF(2^32)":
        # Promoted 2026-05-04 (jit:b13799ac). M4RIE caps at m ≤ 16, so
        # the (matmul, GF(2^32)) cell cannot share the GF(2^m)
        # reference and is routed to NTL `mat_GF2E` instead. See
        # `dev/bench_results/2026-05-04-b13799ac-gf2pow32-promotion.md`.
        # Other GF(2^32) operations (invert, solve, charpoly, …) fall
        # through to the field-default rule below; they have no
        # promoted reference yet.
        return "ntl"
    if operation == "sparse-matmul":
        # Per jit:a3412e15 § 4: no public sparse × sparse matmul exists
        # in fflas-ffpack, LinBox, M4RI/M4RIE, NTL, or FLINT. gf2-core's
        # SpBitMatrix::matmul (jit:2403c054) and SparseFieldMatrix::matmul
        # (jit:eb57f944) are the canonical references for every field.
        return "gf2"
    if operation == "spmv" and FIELD_FAMILY.get(field_value) == "gf2m":
        # Per jit:a3412e15 § 4: fflas-ffpack / LinBox sparse over GF(2^m)
        # ride GivaroExtension polynomial multiplication and are not
        # performance-comparable to gf2-core's PCLMULQDQ-backed
        # Gf2mWide. M4RIE has no public sparse type. gf2-core's
        # SparseFieldMatrix<Gf2mWide<…>>::matvec is the canonical
        # reference.
        return "gf2"
    if operation == "sparse×dense":
        # Per jit:a3412e15 § 4: GF(p) routes to fflas-ffpack `fspmm`
        # (canonical). GF(2^m) cells fall back to gf2-core's
        # SparseFieldMatrix::matmat (no comparable external path —
        # GivaroExtension is `semantics-mismatch` per protocol § 9). GF(2)
        # routes to LinBox `applyLeft × Modular<int64_t>(2)` per the
        # design doc's hard-reference designation; gf2-core's
        # SpBitMatrix::matmat (jit:521390db) is the candidate, LinBox is
        # canonical (note throughput-unit normalisation: gf2-core
        # measures bit ops in u64 packed words; LinBox measures
        # int-modular ops).
        if FIELD_FAMILY.get(field_value) == "gf2m":
            return "gf2"
        # GF(p) and GF(2) cells fall through to the field-default rule
        # (which routes GF(2) → m4ri by default; we override to linbox
        # below since m4ri has no sparse type).
        if field_value == "GF(2)":
            return "linbox"
        # GF(p) cells fall through to the field-default rule.
    if operation == "sparse-elim":
        # Per jit:a3412e15 § 4: LinBox `Method::SparseElimination` is
        # canonical for sparse-elim × {GF(2), GF(p)} (LinBox 1.7.1, Wave-2
        # promotion). GF(2^m) falls back to gf2-core's
        # SparseFieldMatrix::rref (jit:eb57f944) because LinBox over
        # GivaroExtension is not performance-comparable.
        if field_value == "GF(2)":
            return "linbox"
        if FIELD_FAMILY.get(field_value) == "gf2m":
            return "gf2"
        return "linbox"
    if field_value == "GF(2)":
        return "m4ri"
    if FIELD_FAMILY.get(field_value) == "gf2m":
        return "m4rie"
    return "fflas-ffpack"


def _fmt_ns(x: Optional[float]) -> str:
    if x is None:
        return "PENDING"
    if x >= 1e9:
        return f"{x / 1e9:.3f} s"
    if x >= 1e6:
        return f"{x / 1e6:.3f} ms"
    if x >= 1e3:
        return f"{x / 1e3:.3f} µs"
    return f"{x:.0f} ns"


def _fmt_tput(x: Optional[float]) -> str:
    if x is None:
        return "PENDING"
    if x >= 1e9:
        return f"{x / 1e9:.3f} Gops/s"
    if x >= 1e6:
        return f"{x / 1e6:.3f} Mops/s"
    if x >= 1e3:
        return f"{x / 1e3:.3f} kops/s"
    return f"{x:.3f} ops/s"


def _fmt_ratio(gf2: Optional[float], ref: Optional[float]) -> str:
    """Throughput ratio gf2 / ref. >1.0 means gf2 is faster."""
    if gf2 is None or ref is None or ref == 0.0:
        return "PENDING"
    r = gf2 / ref
    return f"{r:.2f}×"


def render_table(op: str, field_value: str, rows: List[CellRow]) -> str:
    ref_lib = reference_lib_for(field_value, operation=op)
    buf = io.StringIO()
    buf.write(f"### {op} × {field_value}\n\n")
    buf.write(
        "| shape | gf2 wall | gf2 throughput | "
        f"{ref_lib} wall | {ref_lib} throughput | ratio (gf2/{ref_lib}) |\n"
    )
    buf.write("|---|---|---|---|---|---|\n")
    for r in rows:
        gf2 = r.by_lib.get("gf2")
        ref = r.by_lib.get(ref_lib)
        gf2_wall = gf2.wall_ns if gf2 else None
        gf2_tput = gf2.throughput_ops if gf2 else None
        ref_wall = ref.wall_ns if ref else None
        ref_tput = ref.throughput_ops if ref else None
        buf.write(
            "| {label} | {gw} | {gt} | {rw} | {rt} | {ratio} |\n".format(
                label=r.cell.label(),
                gw=_fmt_ns(gf2_wall),
                gt=_fmt_tput(gf2_tput),
                rw=_fmt_ns(ref_wall),
                rt=_fmt_tput(ref_tput),
                ratio=_fmt_ratio(gf2_tput, ref_tput),
            )
        )
    buf.write("\n")
    return buf.getvalue()


def render_markdown(rows: List[CellRow]) -> str:
    groups = group_by_op_field(rows)
    buf = io.StringIO()
    if not groups:
        buf.write("_No measurements present in the merged CSVs._\n")
        return buf.getvalue()
    for (op, field_value), group_rows in groups.items():
        buf.write(render_table(op, field_value, group_rows))
    return buf.getvalue()


# ── Smoke / self-test ─────────────────────────────────────────────────

SMOKE_CSV = """lib,operation,field,m,k,n,rank_regime,seed,wall_ns,throughput_ops
gf2,fgemm,GF(2^31-1),256,256,256,uniform,1,123456,2.72e8
fflas-ffpack,fgemm,GF(2^31-1),256,256,256,uniform,1,98765,3.40e8
gf2,fgemm,GF(2^31-1),1024,1024,1024,uniform,2,99999999,2.15e7
gf2,charpoly,GF(2^31-1),64,64,64,uniform,3,2000000,1.31e5
gf2,matmul,GF(2),256,256,256,uniform,4,55000,6.10e8
m4ri,matmul,GF(2),256,256,256,uniform,4,40000,8.40e8
"""


def _run_smoke() -> int:
    """Parse and render the embedded SMOKE_CSV, asserting structural
    invariants. Returns 0 on success, non-zero on failure."""
    parsed = []
    reader = csv.DictReader(io.StringIO(SMOKE_CSV))
    rows: Dict[Tuple[Cell, str], List[Tuple[float, float, int]]] = defaultdict(list)
    for raw in reader:
        cell = Cell(
            operation=raw["operation"],
            field=raw["field"],
            m=int(raw["m"]),
            k=int(raw["k"]),
            n=int(raw["n"]),
            rank_regime=raw["rank_regime"],
        )
        rows[(cell, raw["lib"])].append(
            (float(raw["wall_ns"]), float(raw["throughput_ops"]), int(raw["seed"]))
        )
    by_cell: "OrderedDict[Cell, CellRow]" = OrderedDict()
    for (cell, lib), ms in rows.items():
        cr = by_cell.setdefault(cell, CellRow(cell=cell))
        cr.by_lib[lib] = Measurement(ms[0][0], ms[0][1], ms[0][2])
    parsed = list(by_cell.values())

    md = render_markdown(parsed)
    print(md)

    # Sanity assertions.
    assert "fgemm × GF(2^31-1)" in md, "fgemm/Fp table missing"
    assert "matmul × GF(2)" in md, "matmul/GF(2) table missing"
    assert "charpoly × GF(2^31-1)" in md, "charpoly/Fp table missing"
    assert "PENDING" in md, "expected PENDING marker for unmeasured ref cell"
    assert "0.80×" in md, "expected ratio 0.80× for gf2/fflas at n=256 (got md=\n" + md + ")"
    print("[smoke] OK", file=sys.stderr)
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Merge gf2 + reference bench CSVs and render markdown tables.",
    )
    parser.add_argument(
        "--gf2",
        type=Path,
        help="Path to gf2-side CSV (one row per cell, lib=gf2)",
    )
    parser.add_argument(
        "--reference",
        type=Path,
        help="Path to reference-side CSV (lib=fflas-ffpack/m4ri/m4rie + optional linbox/ntl/flint secondary rows)",
    )
    parser.add_argument(
        "--out",
        type=Path,
        help="Where to write the rendered markdown. Defaults to stdout.",
    )
    parser.add_argument(
        "--smoke",
        action="store_true",
        help="Run an embedded self-test against a tiny synthetic CSV.",
    )
    args = parser.parse_args()

    if args.smoke:
        return _run_smoke()

    if not args.gf2 and not args.reference:
        parser.error("at least one of --gf2 / --reference is required (or pass --smoke)")

    gf2_rows = read_csv(args.gf2) if args.gf2 else []
    ref_rows = read_csv(args.reference) if args.reference else []
    merged = merge(gf2_rows, ref_rows)
    md = render_markdown(merged)

    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(md)
        print(f"wrote {args.out} ({len(merged)} cells)", file=sys.stderr)
    else:
        sys.stdout.write(md)
    return 0


if __name__ == "__main__":
    sys.exit(main())
