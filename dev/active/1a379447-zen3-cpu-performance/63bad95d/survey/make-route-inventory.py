#!/usr/bin/env python3
"""Write the dispatch route inventory of the core kernels (jit:63bad95d).

The inventory joins five kinds of committed input and writes into the issue
directory `route-inventory.json` and `route-inventory.md`, and the per-cell
routes of every measured workload as `workload-routes.jsonl` (one receipt per
line) with the table `workload-routes.md`:

- the production sources, for the selectors the tuning sections own, the baked
  constants, each selector's read sites and the threshold-like constants no
  section owns;
- `source-evidence.json`, whose claim identifiers every declared entry point
  and selector row cites;
- `route-witness/*.tsv`, the answers of the shared fallback contract on each
  build configuration, and `route-witness/dependency-features.tsv`;
- every committed protocol receipt, for the path each arm of each measured
  cell reported and for the routes a family's cells contrast;
- the outcome documents of the closed families, located by name and opening.

The inventory tracks the current tree. A declared row that names an unknown
claim, selector, family or witness entry fails the script, as does a scanned
constant without a classification.

Usage: make-route-inventory.py, with no arguments
"""

import collections
import hashlib
import json
import re

from locate import HERE, ROOT, package, repository_files, tracked

ISSUE = HERE.parent
CORE, SIMD, CODING, ALGEBRA = "gf2-core", "gf2-kernels-simd", "gf2-coding", "gf2-algebra"

# Public entry points whose route a kernel family of this epic measures.
# `witness` names the contract entry that reports the route, when one exists.
ENTRY_POINTS = [
    {
        "id": "logical-word-ops",
        "package": CORE,
        "entries": [
            "kernels::ops::xor_inplace", "kernels::ops::and_inplace", "kernels::ops::or_inplace",
            "kernels::ops::not_inplace", "kernels::ops::resolve_xor_inplace",
            "BitMatrix::row_xor",
        ],
        "routes": [
            ("scalar backend", "width below the threshold, no `simd` feature, or no bundle"),
            ("AVX2 `LogicalFns` bundle", "`simd`, AVX2 detected, width at or above the threshold"),
        ],
        "cargo_features": ["simd"],
        "runtime_checks": ["avx2"],
        "selectors": ["bit_backend.simd_min_words"],
        "private": ["xor-unroll-configurations"],
        "witness": "kernels::select_backend_for_size",
        "claims": ["core-simd-optional", "bit-threshold-read", "bit-threshold-default-alias",
                   "bit-threshold-baked-alias", "ops-xor-resolver", "logical-bundle-detect",
                   "core-logical-bundle-once", "xor-unroll-cfg-declared"],
    },
    {
        "id": "population-count",
        "package": CORE,
        "entries": ["kernels::ops::popcount", "kernels::ops::resolve_popcount",
                    "BitVec::count_ones"],
        "routes": [
            ("scalar count", "width below the threshold, no `simd` feature, or no bundle"),
            ("AVX2 nibble lookup", "`simd`, AVX2 detected, width at or above the threshold"),
        ],
        "cargo_features": ["simd"],
        "runtime_checks": ["avx2"],
        "selectors": ["bit_backend.simd_min_words"],
        "private": ["popcount-comparator-kernels"],
        "witness": "kernels::ops::popcount",
        "claims": ["bit-threshold-read", "ops-popcount-resolver", "ops-popcount-route",
                   "popcount-csa-comparator", "logical-bundle-detect"],
    },
    {
        "id": "fused-and-population-count",
        "package": CORE,
        "entries": ["kernels::ops::and_popcount", "kernels::ops::resolve_and_popcount"],
        "routes": [
            ("scalar fused count", "width below the threshold, no `simd` feature, or no bundle"),
            ("AVX2 fused nibble lookup",
             "`simd`, AVX2 detected, width at or above the threshold"),
        ],
        "cargo_features": ["simd"],
        "runtime_checks": ["avx2"],
        "selectors": ["bit_backend.simd_min_words"],
        "private": ["popcount-comparator-kernels"],
        "witness": "kernels::ops::and_popcount",
        "claims": ["bit-threshold-read", "ops-and-popcount-resolver", "popcount-csa-comparator"],
    },
    {
        "id": "dense-matvec",
        "package": CORE,
        "entries": ["BitMatrix::matvec"],
        "routes": [
            ("scalar row parity", "stride below the threshold, no `simd` feature, or no bundle"),
            ("AVX2 fused AND-popcount per row",
             "`simd`, AVX2 detected, stride at or above the threshold"),
        ],
        "cargo_features": ["simd"],
        "runtime_checks": ["avx2"],
        "selectors": ["bit_matrix.matvec_simd_min_words"],
        "private": [],
        "witness": "BitMatrix::matvec",
        "claims": ["matvec-threshold-conservative", "matvec-threshold-baked-alias",
                   "matvec-threshold-read", "matvec-bundle-check", "matvec-fused-kernel"],
    },
    {
        "id": "bit-transpose",
        "package": CORE,
        "entries": ["BitMatrix::transpose"],
        "routes": [
            ("portable 64x64 block kernel", "no `simd` feature or no AVX2"),
            ("first available lane of the production preference", "`simd` and AVX2 detected"),
            ("simple or macro-tiled outer loop", "block counts against two profile fields"),
        ],
        "cargo_features": ["simd"],
        "runtime_checks": ["avx2"],
        "selectors": ["bit_matrix.transpose_simple_max_blocks",
                      "bit_matrix.transpose_macro_tile_blocks"],
        "private": ["PRODUCTION_PREFERENCE"],
        "witness": "BitMatrix::transpose",
        "claims": ["transpose-lane-selection", "transpose-lane-reporter",
                   "transpose-outer-route", "transpose-preference",
                   "transpose-preference-value", "transpose-lane-feature-gate",
                   "core-transpose-bundle-once"],
    },
    {
        "id": "residual-shift",
        "package": CORE,
        "entries": ["BitVec::shift_left", "BitVec::shift_right"],
        "routes": [
            ("portable funnel", "no `simd` feature or no BMI2"),
            ("BMI2 funnel kernel", "`simd` and BMI2 detected"),
        ],
        "cargo_features": ["simd"],
        "runtime_checks": ["bmi2"],
        "selectors": [],
        "private": ["residual-shift-capability"],
        "witness": "BitVec::shift_left",
        "claims": ["shift-route-selection", "shift-feature-gate", "core-shift-bundle-once"],
    },
    {
        "id": "wide-carry-less-product",
        "package": CORE,
        "entries": ["gf2m::wide::clmul_wide", "gf2m::wide::clmul_wide_slice",
                    "Gf2mWide::mul_ref"],
        "routes": [
            ("portable schoolbook", "another width, no `simd` feature, or no kernel"),
            ("YMM VPCLMULQDQ kernel", "`simd`, four or nine words, AVX2+VPCLMULQDQ+SSE4.1"),
            ("XMM PCLMULQDQ kernel", "`simd`, four or nine words, PCLMULQDQ+SSE4.1 only"),
        ],
        "cargo_features": ["simd"],
        "runtime_checks": ["avx2", "vpclmulqdq", "pclmulqdq", "sse4.1"],
        "selectors": [],
        "private": ["wide-kernel-widths"],
        "witness": "gf2m::wide::clmul_wide",
        "claims": ["clmul-wide-width-4", "clmul-wide-width-9", "clmul-wide-ymm-predicate",
                   "core-wide-bundle-once"],
    },
    {
        "id": "gf2m-batch-products",
        "package": CORE,
        "entries": ["gf2m::batch::batch_mul", "FieldVec::dot_product",
                    "FieldVec::simd_dot_product"],
        "routes": [
            ("per-element products", "no `simd` feature or no PCLMULQDQ+SSE4.1"),
            ("sequential PCLMULQDQ batch lane", "`simd` and PCLMULQDQ+SSE4.1 detected"),
            ("YMM batch lane", "an explicit preference only; no production caller passes it"),
        ],
        "cargo_features": ["simd"],
        "runtime_checks": ["pclmulqdq", "sse4.1"],
        "selectors": ["field_vec.dot_chunk_len"],
        "private": ["raw-batch-default-lane", "gf2m-batch-capability"],
        "witness": None,
        "claims": ["clmul-batch-default-lane", "clmul-batch-feature-gate", "dot-batch-kernel",
                   "dot-chunk-read", "core-gf2m-bundle-once"],
    },
    {
        "id": "gf256-product-table",
        "package": CORE,
        "entries": ["FieldVec::axpy", "field::matrix::gemm"],
        "routes": [
            ("cached product table", "degree 8, single-word value, shared field context"),
            ("the route without the table", "every other field or representation"),
        ],
        "cargo_features": [],
        "runtime_checks": [],
        "selectors": [],
        "private": ["gf256-table-predicate"],
        "witness": None,
        "claims": ["gf256-table-predicate"],
    },
]

# Selectors and selector-like choices that are no scanned constant.
DECLARED_SELECTORS = [
    ("xor-unroll-configurations", SIMD,
     "Two private compiler configurations select the XOR unroll bodies.",
     ["xor-unroll-cfg-declared"]),
    ("popcount-comparator-kernels", SIMD,
     "The scalar POPCNT and carry-save kernels are bundle fields that no resolver selects.",
     ["popcount-csa-comparator", "ops-popcount-resolver"]),
    ("wide-kernel-widths", CORE,
     "The wide carry-less dispatch selects a kernel at four and nine words.",
     ["clmul-wide-width-4", "clmul-wide-width-9"]),
    ("raw-batch-default-lane", SIMD,
     "The default GF(2^m) bundle prefers the sequential PCLMULQDQ batch lane.",
     ["clmul-batch-default-lane"]),
    ("gf256-table-predicate", CORE,
     "The product-table lane is selected by field degree and representation.",
     ["gf256-table-predicate"]),
    ("residual-shift-capability", CORE,
     "The residual shift takes the BMI2 funnel whenever it is detected; no size selector exists.",
     ["shift-route-selection", "shift-feature-gate"]),
    ("gf2m-batch-capability", CORE,
     "The GF(2^m) dot product takes the batch kernel at every length; no length selector exists.",
     ["dot-batch-kernel"]),
]

# Scan rule of the private-constant list: an integer or lane constant of a
# production source file whose name carries one of these tokens.
SCAN_TOKENS = (
    "THRESH", "MIN", "MAX", "CUTOFF", "CROSSOVER", "TILE", "CHUNK", "BLOCK", "PANEL", "BASE",
    "PREFERENCE", "LIMIT", "KC", "BATCH", "STRIDE", "WIDTH", "WORDS", "UNROLL",
)
SCAN = re.compile(
    r"^\s*(?:pub(?:\([a-z]+\))? )?const ([A-Z0-9_]*(?:" + "|".join(SCAN_TOKENS)
    + r")[A-Z0-9_]*)\s*:\s*([^=]+?)\s*=\s*(.+?);?\s*$"
)
SCANNED_PACKAGES = (CORE, SIMD, ALGEBRA)

# Class of each scanned constant no tuning section owns.
#   selector: chooses between routes or bounds one for performance
#   kernel-geometry: a structural extent of one kernel or data layout
#   algorithm-limit: a retry, seed or recursion bound without a route choice
CLASSES = {
    "N_MAX_MULTIWORD": "kernel-geometry",
    "MAX_MATRIX_BYTES_FOR_L1": "selector",
    "B2_GRAY_TILE_WORDS": "kernel-geometry",
    "B2_GRAY_MAX_TILES": "kernel-geometry",
    "M4RM_DEFAULT_MAX_K": "selector",
    "M4RM_TILE_ROWS": "kernel-geometry",
    "M4RM_TILE_WORDS": "kernel-geometry",
    "KG_MAX_RETRIES": "algorithm-limit",
    "WIEDEMANN_DETERMINISTIC_VERIFY_N": "algorithm-limit",
    "UNROLL": "kernel-geometry",
    "XOR_UNROLL": "selector",
    "WIEDEMANN_MAX_RETRIES": "algorithm-limit",
    "MAX_DECODE_DEPTH": "algorithm-limit",
    "MAX_SEEDS": "algorithm-limit",
    "MAX_ITERATIONS": "algorithm-limit",
    "MAX_RETRIES": "algorithm-limit",
    "DEFAULT_BLOCK_ROWS": "selector",
    "AXPY_UNROLL": "kernel-geometry",
    "PANEL_SCRATCH_COLS": "kernel-geometry",
    "POPCOUNT_CSA_BLOCK_WORDS": "kernel-geometry",
    "PRODUCTION_PREFERENCE": "selector",
    "LANE_BLOCK": "kernel-geometry",
    "FP_MEDIUM_PANEL_MR": "kernel-geometry",
    "FP_MEDIUM_PANEL_NR": "kernel-geometry",
    "K_CHUNK_CAP": "selector",
    "CHUNK_VEC": "selector",
    "I_TILE": "selector",
    "WORDS_PER_VECTOR": "kernel-geometry",
    "CSA_BLOCK_VECTORS": "kernel-geometry",
    "CSA_BLOCK_WORDS": "kernel-geometry",
}

# Track of each issue whose receipts the repository holds. `core-kernel` is the
# calibration scope of this issue; the others are listed for their routes only.
TRACKS = {
    "00dd43c3": "core-kernel", "04b85d10": "core-kernel", "18a87159": "core-kernel",
    "19513245": "core-kernel", "1c602857": "core-kernel", "1d0da41f": "core-kernel",
    "1d4fd63d": "core-kernel", "26465e6c": "core-kernel", "4c1e441f": "core-kernel",
    "53c5a8c0": "core-kernel", "5cbb6545": "core-kernel", "65c0e13d": "core-kernel",
    "6c6b09b1": "core-kernel", "6fb89a3c": "core-kernel", "85fc5ff4": "core-kernel",
    "ad2a6a58": "core-kernel", "c7113c5a": "core-kernel", "e1f9a78f": "core-kernel",
    "12fdeb5b": "coding-consumer", "9fb40c83": "coding-consumer", "eda07788": "coding-consumer",
    "07ca8585": "decoder", "3be770d5": "decoder", "c077a88b": "decoder", "f63a2464": "decoder",
    "f547c394": "protocol",
}

# Outcome documents by file name and opening line.
OUTCOMES = {
    "public-clmul": ("findings.md",
                     "# Public wide carry-less product: dispatch routing and its receipt"),
    "clmul-crossover": ("findings.md",
                        "# Carry-less multiplication crossovers and wide polynomial "
                        "competitiveness"),
    "ymm-dispatch": ("findings.md", "# YMM carry-less batch dispatch on Zen 3"),
    "polynomial-baselines": ("findings.md", "# Equivalent polynomial multiplication baselines"),
    "popcount": ("findings.md",
                 "# Population count and fused bit reductions on the Ryzen 9 5900X"),
    "popcount-baselines": ("findings.md", "# Population-count and fused-reduction baselines"),
    "bit-storage": ("findings.md", "# Bit-storage costs in production consumers"),
    "transpose": ("findings.md",
                  "# The 64x64 transpose lane family and the BCH bitslice conversion"),
    "shifts": ("publication-findings.md", "# Shift and permutation dispositions"),
    "shift-confirmation": ("confirmation-outcome.md",
                           "# Residual-shift BMI2 route confirmation (jit:00dd43c3)"),
    "byte-field": ("findings.md", "# Byte-field acceleration in the vector and matrix consumers"),
    "gf256-axpy": ("outcome.md", "# Shipped GF(2^8) axpy lane: confirmation outcome"),
    "gf256-product": ("outcome.md",
                      "# Outcome: the shipped GF(2^8) dense product against the matrix route"),
    "mid-range": ("mid-range-findings.md", "# Mid-range buffer evidence synthesis"),
    "dense-parity": ("no-change-outcome.md", "# Dense-parity production outcome"),
    "logical-buffer": ("no-change-outcome.md", "# Logical-buffer production outcome"),
    "tuning-campaign": ("gf2-dbd8787d-*.md", "# Extent calibration gf2-dbd8787d-"),
}

# For each selector, the measurement families whose cells contrast its two
# sides, and the outcome documents that state the disposition. The evidence
# class is derived from the families' receipts, never declared.
EVIDENCE = [
    ("bit_backend.simd_min_words",
     ["bit-storage-logical-consumers", "bit-storage-count-consumers", "popcount-baselines"],
     ["bit-storage", "popcount-baselines", "popcount", "mid-range", "logical-buffer",
      "tuning-campaign"]),
    ("bit_matrix.matvec_simd_min_words",
     ["2037941f-dense-allocated-matvec"], ["mid-range", "dense-parity"]),
    ("bit_matrix.transpose_simple_max_blocks", [], ["tuning-campaign"]),
    ("bit_matrix.transpose_macro_tile_blocks", [], ["tuning-campaign"]),
    ("field_vec.dot_chunk_len", [], ["tuning-campaign"]),
    ("PRODUCTION_PREFERENCE",
     ["transpose-lane-selection", "bit-storage-layout-consumers"], ["transpose", "bit-storage"]),
    ("popcount-comparator-kernels",
     ["popcount-route-selection", "fused-count-consumers"], ["popcount"]),
    ("xor-unroll-configurations",
     ["2037941f-logical-isolated-xor", "2037941f-logical-public-row-xor"],
     ["mid-range", "logical-buffer"]),
    ("wide-kernel-widths", ["public-clmul-wide-dispatch"], ["public-clmul"]),
    ("raw-batch-default-lane", ["ymm-clmul-dispatch"], ["ymm-dispatch"]),
    ("gf2m-batch-capability", ["gf2m-clmul-crossover"], ["clmul-crossover"]),
    ("gf256-table-predicate",
     ["gf256-shipped-axpy-lane", "gf256-shipped-dense-product", "bytefield-consumer-vector",
      "bytefield-consumer-matrix"],
     ["byte-field", "gf256-axpy", "gf256-product"]),
    ("residual-shift-capability",
     ["bitvec-residual-bmi2-route"], ["shifts", "shift-confirmation"]),
]

WITNESS_CONFIGURATIONS = [
    ("core-default", "standalone `gf2-core`, default features"),
    ("core-simd", "standalone `gf2-core`, `simd`"),
    ("core-all-features", "standalone `gf2-core`, `--all-features`"),
    ("core-baked", "standalone `gf2-core`, `simd`, `--cfg gf2_tuning_baked`"),
    ("coding-default", "`gf2-coding` dependency build, default features"),
    ("coding-no-default-features",
     "`gf2-coding` test build, `--no-default-features`; its self dev-dependency keeps the "
     "default features"),
    ("sim-default", "`gf2-sim` dependency build, default features"),
]


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def read(path):
    return (ROOT / path).read_text()


def production_sources(name):
    """Root-relative `src/**/*.rs` paths of a package, and each file's code lines.

    The code of a file ends where its unit-test module begins.
    """
    sources = {}
    for path in tracked(f"{package(name)}/src/**/*.rs"):
        lines = read(path).splitlines()
        end = len(lines)
        for index in range(len(lines) - 1):
            if lines[index].startswith("#[cfg(test)]") and lines[index + 1].startswith("mod "):
                end = index
                break
        sources[path] = lines[:end]
    return sources


def is_comment(line):
    return line.lstrip().startswith("//")


def tuning_sections(claims):
    """Selectors the tuning sections own, with their values and read sites."""
    core = package(CORE)
    conservative = json.loads(read(f"{core}/data/tuning-profiles/conservative.json"))
    section = conservative["sections"]["gf2-core/selectors"]["selectors"]

    baked_text = read(f"{core}/src/tuning/baked.rs")
    baked, pending = {}, None
    for line in baked_text.splitlines():
        named = re.match(r"/// `([a-z_0-9]+\.[a-z_0-9]+)`", line)
        if named:
            pending = named.group(1)
        constant = re.match(r"pub\(crate\) const ([A-Z0-9_]+): \w+ = (\d+);", line)
        if constant and pending:
            baked[pending] = {"constant": constant.group(1), "value": int(constant.group(2))}
            pending = None
        if line.startswith("#[cfg(test)]"):
            break

    owner_name = re.search(r"data/tuning-profiles/([a-z0-9-]+\.json)", baked_text).group(1)
    owner_digest = re.search(r"SHA-256 `([0-9a-f]{64})`", baked_text).group(1)
    owner_path = f"{core}/data/tuning-profiles/{owner_name}"
    owner_bytes = (ROOT / owner_path).read_bytes()
    if sha256(owner_bytes) != owner_digest:
        raise SystemExit(f"{owner_path} does not hold the digest the baked constants pin")
    owner = json.loads(owner_bytes)["sections"]["gf2-core/selectors"]
    measured = owner["selectors"]

    sources = production_sources(CORE)
    rows = []
    for family in sorted(section):
        for field in sorted(section[family]):
            selector = f"{family}.{field}"
            compile_time = selector in baked
            needle = (
                re.compile(rf"baked::{baked[selector]['constant']}\b") if compile_time
                else re.compile(rf"\.{field}\(\)")
            )
            sites = [
                f"{path}:{number}"
                for path, lines in sources.items()
                if "/src/tuning/" not in path and not path.endswith("dispatch_contract.rs")
                for number, line in enumerate(lines, 1)
                if needle.search(line) and not is_comment(line)
            ]
            rows.append(
                {
                    "selector": selector,
                    "conservative": section[family][field],
                    "selection": "compile-time" if compile_time else "runtime-profile",
                    "baked": baked[selector]["value"] if compile_time else None,
                    "measured_owner": measured.get(family, {}).get(field),
                    "read_sites": sites,
                }
            )
    if len(baked) != sum(row["selection"] == "compile-time" for row in rows):
        raise SystemExit("a baked constant names no selector of the conservative table")

    others = []
    algebra = json.loads(
        read(f"{package(ALGEBRA)}/data/tuning-profiles/conservative.json")
    )["sections"]
    for identifier, body in sorted(algebra.items()):
        others.append(
            {
                "section": identifier,
                "owner": ALGEBRA,
                "selectors": sorted(
                    f"{family}.{field}"
                    for family, fields in body["selectors"].items() for field in fields
                ),
            }
        )
    coding = read(f"{package(CODING)}/src/tuning.rs")
    others.append(
        {
            "section": re.search(r"carried by `([a-z0-9/-]+)`", coding).group(1),
            "owner": CODING,
            "selectors": sorted(
                "encode." + name
                for name in re.findall(r"pub const fn (\w+)\(&self\) -> usize", coding)
            ),
            "claims": ["coding-tuning-section"],
        }
    )
    assert "coding-tuning-section" in claims
    return {
        "core_section": "gf2-core/selectors",
        "conservative_profile": f"{core}/data/tuning-profiles/conservative.json",
        "measured_owner": {
            "path": owner_path,
            "sha256": owner_digest,
            "harness": owner["measurement"]["harness"],
            "harness_schema": owner["measurement"]["harness_schema"],
            "binary_sha256": owner["measurement"]["binary_sha256"],
            "recorded_receipt": owner["measurement"]["receipt"],
        },
        "selectors": rows,
        "other_sections": others,
    }


def owned_constant_names(sections):
    """Constant names the core conservative table or the baked module defines or reads."""
    core = package(CORE)
    text = read(f"{core}/src/tuning/mod.rs")
    start = text.index("    pub const CONSERVATIVE: Self = Self {")
    initializer = text[start:text.index("\n    };\n", start)]
    names = set(re.findall(r"::([A-Z][A-Z0-9_]+)\b", initializer))
    names |= set(re.findall(r"const ([A-Z0-9_]+):", read(f"{core}/src/tuning/baked.rs")))
    algebra = read(f"{package(ALGEBRA)}/src/tuning.rs")
    names |= set(re.findall(r"::([A-Z][A-Z0-9_]+)\b", algebra))
    return names


def private_constants(sections):
    """Scanned constants no tuning section owns, with their read sites."""
    owned = owned_constant_names(sections)
    rows, unclassified, owned_scanned = [], [], 0
    for name in SCANNED_PACKAGES:
        sources = production_sources(name)
        for path, lines in sources.items():
            if "/src/tuning/" in path or path.endswith("/src/tuning.rs"):
                continue
            for number, line in enumerate(lines, 1):
                if line.rstrip().endswith("=") and number < len(lines):
                    line = f"{line.rstrip()} {lines[number].strip()}"
                found = SCAN.match(line)
                if not found or is_comment(line):
                    continue
                constant, kind, value = found.groups()
                if kind.strip() == "&str":
                    continue
                alias = re.fullmatch(r"(?:[a-z_:]+::)?([A-Z][A-Z0-9_]*)", value.strip())
                if constant in owned or (alias and alias.group(1) in owned):
                    owned.add(constant)
                    owned_scanned += 1
                    continue
                if constant not in CLASSES:
                    unclassified.append(f"{path}:{number}: {constant}")
                    continue
                word = re.compile(rf"\b{constant}\b")
                sites = [
                    f"{other}:{index}"
                    for other, body in sources.items()
                    for index, text in enumerate(body, 1)
                    if word.search(text) and not is_comment(text)
                    and not (other == path and index == number)
                ]
                rows.append(
                    {
                        "id": constant,
                        "package": name,
                        "defined": f"{path}:{number}",
                        "definition": line.strip(),
                        "class": CLASSES[constant],
                        "read_sites": sites,
                    }
                )
    if unclassified:
        raise SystemExit("scanned constants without a class:\n" + "\n".join(unclassified))
    return rows, owned_scanned


def witnesses():
    """Per configuration, the contract's answers; per entry, the routes by configuration."""
    directory = HERE / "route-witness"
    configurations, by_entry = [], collections.defaultdict(dict)
    for name, description in WITNESS_CONFIGURATIONS:
        data = (directory / f"{name}.tsv").read_bytes()
        rows = [line.split("\t") for line in data.decode().splitlines()]
        facts = {entry: route for entry, _, route, _ in rows if entry.startswith("build.")}
        resolution = [route for entry, _, route, _ in rows if entry == "tuning.active"]
        for entry, argument, route, _ in rows:
            if not entry.startswith("build.") and entry != "tuning.active":
                by_entry[(entry, argument)][name] = route
        configurations.append(
            {
                "configuration": name,
                "description": description,
                "record": str((directory / f"{name}.tsv").relative_to(ROOT)),
                "record_sha256": sha256(data),
                "simd_feature": facts["build.simd-feature"],
                "baked_selectors": facts["build.baked-selectors"],
                "core_section_resolution": resolution[0],
                "rows": len(rows),
                "kernel_rows": sum(kind == "kernel" for _, _, _, kind in rows[2:]),
            }
        )
    features = [
        dict(zip(("selection", "package", "features"), line.split("\t")))
        for line in (directory / "dependency-features.tsv").read_text().splitlines()
    ]
    matrix = [
        {"entry": entry, "input": argument, "routes": routes}
        for (entry, argument), routes in by_entry.items()
    ]
    return configurations, matrix, features


def receipts(selector_sites):
    """Every committed protocol receipt: its arms' reported paths and its contrasts."""
    current = {}
    campaigns, families = [], {}
    for path in tracked("**/receipt.json"):
        record = json.loads(read(path))
        if record.get("schema") != "zen3-benchmark-receipt-v1":
            continue
        issue = record["issue"]
        if issue not in TRACKS:
            raise SystemExit(f"{path}: issue {issue} has no declared track")
        directory = str((ROOT / path).parent.relative_to(ROOT))
        summary = json.loads(read(f"{directory}/acceptance-summary.json"))
        family = record.get("family_id") or summary["family"]["family_id"]
        arms = {}
        cells = []
        for cell in record["cells"]:
            sides = {}
            for side in ("baseline", "candidate"):
                arm = cell[f"{side}_arm"]
                paths = sorted({pair[side]["selected_path"] for pair in cell["pairs"]})
                arms.setdefault(arm, set()).update(paths)
                sides[side] = {
                    "arm": arm,
                    "build": record["arms"][arm]["build"],
                    "paths": paths,
                    "executable": record["arms"][arm]["executable_sha256"],
                }
            builds = {sides["baseline"]["build"], sides["candidate"]["build"]}
            if not cell["pairs"]:
                contrast = "unavailable"
            elif "external" in builds:
                contrast = "external-comparator"
            elif sides["baseline"]["paths"] != sides["candidate"]["paths"]:
                contrast = "route-contrast"
            elif sides["baseline"]["executable"] != sides["candidate"]["executable"]:
                contrast = "build-contrast"
            else:
                contrast = "same-route"
            cells.append(
                {
                    "cell_id": cell["cell_id"],
                    "role": cell["role"],
                    "contrast": contrast,
                    "baseline": {key: sides["baseline"][key] for key in ("arm", "paths")},
                    "candidate": {key: sides["candidate"][key] for key in ("arm", "paths")},
                }
            )
        pinned = (record.get("source") or {}).get("producing", {}).get("behavior_sha256")
        drift = None
        if pinned:
            changed = []
            for source, digest in pinned.items():
                if source not in current:
                    target = ROOT / source
                    current[source] = sha256(target.read_bytes()) if target.is_file() else None
                if current[source] != digest:
                    changed.append(source)
            drift = {
                "pinned_files": len(pinned),
                "changed_in_tree": len(changed),
                "changed_selector_sites": sorted(set(changed) & selector_sites),
            }
        campaigns.append(
            {
                "issue": issue,
                "track": TRACKS[issue],
                "family": family,
                "campaign_id": record["campaign_id"],
                "label": record["label"],
                "receipt": path,
                "receipt_sha256": sha256((ROOT / path).read_bytes()),
                "verdict": summary["verdict"],
                "qualifies": summary["qualifies"],
                "arms": [
                    {
                        "arm": arm,
                        "build": record["arms"][arm]["build"],
                        "rustflags": record["arms"][arm].get("rustflags"),
                        "tuning_profile": record["arms"][arm].get("tuning_profile"),
                        "executable_sha256": record["arms"][arm]["executable_sha256"],
                        "paths": sorted(paths),
                    }
                    for arm, paths in sorted(arms.items())
                ],
                "cells": cells,
                "producing_sources": drift,
            }
        )
        entry = families.setdefault(
            family,
            {"family": family, "issue": issue, "track": TRACKS[issue], "campaigns": [],
             "contrast_cells": collections.Counter(), "contrast_roles": set(),
             "contrasted_routes": set(), "builds": set()},
        )
        entry["campaigns"].append(
            {"campaign_id": record["campaign_id"], "label": record["label"],
             "verdict": summary["verdict"], "qualifies": summary["qualifies"]}
        )
        for cell in cells:
            entry["contrast_cells"][f"{cell['role']}/{cell['contrast']}"] += 1
            if cell["contrast"] in ("route-contrast", "build-contrast"):
                entry["contrast_roles"].add(cell["role"])
            if cell["contrast"] == "route-contrast":
                for before in cell["baseline"]["paths"]:
                    for after in cell["candidate"]["paths"]:
                        entry["contrasted_routes"].add((cell["role"], before, after))
        entry["builds"].update(arm["build"] for arm in campaigns[-1]["arms"])
    campaigns.sort(key=lambda row: (row["track"], row["issue"], row["family"], row["campaign_id"]))
    rows = []
    for entry in sorted(families.values(), key=lambda row: (row["track"], row["issue"],
                                                           row["family"])):
        entry["campaigns"].sort(key=lambda row: row["campaign_id"])
        entry["contrast_cells"] = dict(sorted(entry["contrast_cells"].items()))
        entry["contrasted_routes"] = [
            {"role": role, "baseline": before, "candidate": after}
            for role, before, after in sorted(entry["contrasted_routes"])
        ]
        entry["contrast_roles"] = sorted(entry["contrast_roles"])
        entry["builds"] = sorted(entry["builds"])
        rows.append(entry)
    return campaigns, rows


def evidence(families, selectors, private):
    """Per selector, the families that contrast it and the class their receipts carry."""
    known = {row["family"]: row for row in families}
    names = {row["selector"] for row in selectors} | {row["id"] for row in private}
    stated = {row["selector"]: row["measured_owner"] is not None for row in selectors}
    documents = {}
    for key, (name, opening) in OUTCOMES.items():
        documents[key] = repository_files.document(ROOT, name, opening.encode())
    rows = []
    for selector, family_names, outcome_keys in EVIDENCE:
        if selector not in names:
            raise SystemExit(f"evidence row names unknown selector {selector}")
        cited, roles = [], set()
        for name in family_names:
            family = known.get(name)
            if family is None:
                raise SystemExit(f"evidence row of {selector} names unknown family {name}")
            contrasted = family["contrast_roles"]
            if not contrasted:
                raise SystemExit(f"{name} contrasts no two gf2 routes or builds")
            roles.update(contrasted)
            cited.append(
                {
                    "family": name,
                    "issue": family["issue"],
                    "route_contrast_roles": contrasted,
                    "campaigns": [
                        campaign for campaign in family["campaigns"]
                        if campaign["label"] in ("confirmation", "holdout")
                    ],
                }
            )
        if roles & {"confirmatory", "holdout"}:
            status = "confirmatory"
        elif roles:
            status = "exploratory"
        else:
            status = "none"
        rows.append(
            {
                "selector": selector,
                "measured_owner_states_it": stated.get(selector, False),
                "protocol_evidence": status,
                "families": cited,
                "outcome_documents": [documents[key] for key in outcome_keys],
            }
        )
    return rows


def interned(campaign):
    """A receipt row whose arms and cells name each path by its index in `paths`."""
    paths = sorted({path for arm in campaign["arms"] for path in arm["paths"]})
    index = {path: number for number, path in enumerate(paths)}
    return {
        **campaign,
        "paths": paths,
        "arms": [
            {**arm, "paths": [index[path] for path in arm["paths"]]} for arm in campaign["arms"]
        ],
        "cells": [
            {
                **cell,
                "baseline": {"arm": cell["baseline"]["arm"],
                             "paths": [index[path] for path in cell["baseline"]["paths"]]},
                "candidate": {"arm": cell["candidate"]["arm"],
                              "paths": [index[path] for path in cell["candidate"]["paths"]]},
            }
            for cell in campaign["cells"]
        ],
    }


def table(header, rows):
    lines = ["| " + " | ".join(header) + " |", "|" + "---|" * len(header)]
    lines += ["| " + " | ".join(str(cell) for cell in row) + " |" for row in rows]
    return "\n".join(lines)


def code(items):
    return ", ".join(f"`{item}`" for item in items) if items else "—"


def render(inventory):
    out = [
        "# Dispatch route inventory of the core kernels",
        "",
        "> **Diátaxis Type:** Reference",
        "",
        "Generated by `survey/make-route-inventory.py` from the current tree and the "
        "committed records it names; [`route-inventory.json`](route-inventory.json) holds "
        "every row in full, and claim identifiers resolve in "
        "[`survey/source-evidence.json`](survey/source-evidence.json).",
        "",
        "## Counts",
        "",
        table(["Quantity", "Count"], inventory["counts"].items()),
        "",
        "## Entry points and routes",
        "",
    ]
    for entry in inventory["entry_points"]:
        out += [
            f"### `{entry['id']}`",
            "",
            f"Entries: {code(entry['entries'])}.",
            "",
            table(["Route", "Selected when"], entry["routes"]),
            "",
            table(
                ["Decider", "Value"],
                [
                    ["Cargo features of `gf2-core`", code(entry["cargo_features"])],
                    ["Runtime processor checks", code(entry["runtime_checks"])],
                    ["Selectors of the tuning sections", code(entry["selectors"])],
                    ["Private selectors", code(entry["private"])],
                    ["Route reporter in the contract", code([entry["witness"]])
                     if entry["witness"] else "none"],
                    ["Source claims", code(entry["claims"])],
                ],
            ),
            "",
        ]
    out += [
        "## Route witnesses",
        "",
        "Each column is one build configuration on which the shared fallback contract ran; "
        "a cell is the route its reporter named.",
        "",
    ]
    names = [row["configuration"] for row in inventory["witness_configurations"]]
    out += [
        table(
            ["Configuration", "Build", "`simd`", "Baked", "Core section", "Kernel rows", "Record"],
            [
                [f"`{row['configuration']}`", row["description"], row["simd_feature"],
                 row["baked_selectors"], row["core_section_resolution"],
                 f"{row['kernel_rows']} of {row['rows']}",
                 f"[tsv](survey/route-witness/{row['configuration']}.tsv)"]
                for row in inventory["witness_configurations"]
            ],
        ),
        "",
        table(
            ["Entry", "Input"] + [f"`{name}`" for name in names],
            [
                [f"`{row['entry']}`", row["input"]] + [row["routes"][name] for name in names]
                for row in inventory["route_witnesses"]
            ],
        ),
        "",
        "## Features of the kernel packages per dependency build",
        "",
        table(
            ["Cargo selection", "Package", "Features"],
            [[f"`-p {row['selection']}`", f"`{row['package']}`", code(row["features"].split(","))]
             for row in inventory["dependency_features"]],
        ),
        "",
        "## Selectors the tuning sections own",
        "",
        f"Core section `{inventory['tuning']['core_section']}`; measured owner "
        f"`{inventory['tuning']['measured_owner']['path']}`. A blank measured value is a field "
        "the measured owner omits.",
        "",
        table(
            ["Selector", "Selection", "Conservative", "Baked", "Measured owner", "Read sites"],
            [
                [f"`{row['selector']}`", row["selection"], row["conservative"],
                 "" if row["baked"] is None else row["baked"],
                 "" if row["measured_owner"] is None else row["measured_owner"],
                 len(row["read_sites"])]
                for row in inventory["tuning"]["selectors"]
            ],
        ),
        "",
        table(
            ["Other section", "Owner", "Selectors"],
            [[f"`{row['section']}`", f"`{row['owner']}`", code(row["selectors"])]
             for row in inventory["tuning"]["other_sections"]],
        ),
        "",
        "## Thresholds and selectors outside the tuning sections",
        "",
        "Scan rule: a constant of a production source file of "
        + code(SCANNED_PACKAGES)
        + " whose name carries one of "
        + code(SCAN_TOKENS)
        + ", outside unit-test modules and string constants.",
        "",
        table(
            ["Constant", "Class", "Defined", "Definition", "Read sites"],
            [
                [f"`{row['id']}`", row["class"], f"`{row['defined']}`", f"`{row['definition']}`",
                 code(row["read_sites"][:3])
                 + (f" and {len(row['read_sites']) - 3} more" if len(row["read_sites"]) > 3
                    else "")]
                for row in inventory["private_constants"]
            ],
        ),
        "",
        table(
            ["Declared selector", "Package", "What it selects", "Source claims"],
            [[f"`{row['id']}`", f"`{row['package']}`", row["what"], code(row["claims"])]
             for row in inventory["declared_selectors"]],
        ),
        "",
        "## Evidence in hand per selector",
        "",
        "`confirmatory` means a cited family holds a confirmatory or holdout cell whose two "
        "arms are gf2 arms that report different routes or run different executables; "
        "`exploratory` means only exploratory cells do; `none` means no committed protocol "
        "receipt contrasts the selector's sides. `Calibrated` states whether the measured "
        "owner of the offline tuning system states the selector. The outcome documents "
        "state each disposition.",
        "",
        table(
            ["Selector", "Protocol evidence", "Calibrated", "Families and confirmations",
             "Outcome documents"],
            [
                [
                    f"`{row['selector']}`",
                    row["protocol_evidence"],
                    "yes" if row["measured_owner_states_it"] else "no",
                    "<br>".join(
                        f"`{family['family']}` ({family['issue']}; "
                        + ", ".join(family["route_contrast_roles"]) + "): "
                        + (", ".join(
                            f"`{campaign['campaign_id']}` {campaign['verdict']}, "
                            f"qualifies {str(campaign['qualifies']).lower()}"
                            for campaign in family["campaigns"]) or "no confirmation")
                        for family in row["families"]
                    ) or "—",
                    "<br>".join(f"`{path}`" for path in row["outcome_documents"]),
                ]
                for row in inventory["selector_evidence"]
            ],
        ),
        "",
    ]
    return "\n".join(out)


def render_workloads(workloads):
    out = [
        "# Routes of the measured workloads",
        "",
        "> **Diátaxis Type:** Reference",
        "",
        "Generated by `survey/make-route-inventory.py`; "
        "[`workload-routes.jsonl`](workload-routes.jsonl) holds the paths of every cell.",
        "",
        "One row per committed receipt: the path each arm reported in every pair of its "
        "cells, on the executable the receipt pins. `Sources` counts the producing source "
        "files whose bytes differ in the current tree, of those the receipt pins, and how "
        "many of the differing files hold a selector read site; the record names them. A "
        "differing digest alone does not state a different selection.",
        "",
    ]
    for track in ("core-kernel", "coding-consumer", "decoder", "protocol"):
        rows = [row for row in workloads if row["track"] == track]
        out += [
            f"### Track `{track}`",
            "",
            table(
                ["Issue", "Family", "Campaign", "Label", "Cells", "Arm: build: reported paths",
                 "Sources"],
                [
                    [
                        row["issue"], f"`{row['family']}`", f"`{row['campaign_id']}`",
                        row["label"], len(row["cells"]),
                        "<br>".join(
                            f"`{arm['arm']}`: {arm['build']}: "
                            + (code(arm["paths"]) if len(arm["paths"]) <= 4
                               else f"{len(arm['paths'])} paths")
                            for arm in row["arms"]
                        ),
                        "not pinned per file" if row["producing_sources"] is None else (
                            f"{row['producing_sources']['changed_in_tree']} of "
                            f"{row['producing_sources']['pinned_files']}"
                            + f"; {len(row['producing_sources']['changed_selector_sites'])} "
                            "hold a selector read site"
                        ),
                    ]
                    for row in rows
                ],
            ),
            "",
        ]
    return "\n".join(out)


def main():
    ledger = json.loads((HERE / "source-evidence.json").read_text())
    claims = {claim["id"] for claim in ledger["claims"]}
    tuning = tuning_sections(claims)
    private, owned_scanned = private_constants(tuning)
    declared = [
        {"id": identifier, "package": name, "what": what, "claims": cited}
        for identifier, name, what, cited in DECLARED_SELECTORS
    ]
    configurations, matrix, features = witnesses()
    selector_sites = {
        site.rsplit(":", 1)[0]
        for row in tuning["selectors"] for site in row["read_sites"]
    } | {row["defined"].rsplit(":", 1)[0] for row in private if row["class"] == "selector"}
    campaigns, families = receipts(selector_sites)

    owned = {row["selector"] for row in tuning["selectors"]}
    private_ids = {row["id"] for row in private} | {row["id"] for row in declared}
    witnessed = {row["entry"] for row in matrix}
    for entry in ENTRY_POINTS:
        missing = [
            *(claim for claim in entry["claims"] if claim not in claims),
            *(selector for selector in entry["selectors"] if selector not in owned),
            *(selector for selector in entry["private"] if selector not in private_ids),
        ]
        if entry["witness"] and entry["witness"] not in witnessed:
            missing.append(entry["witness"])
        if missing:
            raise SystemExit(f"{entry['id']} names unknown {missing}")
    for row in declared:
        if set(row["claims"]) - claims:
            raise SystemExit(f"{row['id']} cites an unknown claim")

    core_families = [row for row in families if row["track"] == "core-kernel"]
    workload_lines = "".join(
        json.dumps(interned(row), separators=(",", ":")) + "\n" for row in campaigns
    )
    inventory = {
        "schema": "dispatch-route-inventory-v1",
        "issue": "63bad95d",
        "source_evidence": {
            "path": str((HERE / "source-evidence.json").relative_to(ROOT)),
            "sha256": sha256((HERE / "source-evidence.json").read_bytes()),
        },
        "counts": {
            "entry point groups": len(ENTRY_POINTS),
            "public entry points": sum(len(entry["entries"]) for entry in ENTRY_POINTS),
            "routes": sum(len(entry["routes"]) for entry in ENTRY_POINTS),
            "selectors of the core tuning section": len(tuning["selectors"]),
            "core selectors selected at compile time": sum(
                row["selection"] == "compile-time" for row in tuning["selectors"]),
            "core selectors the measured owner states": sum(
                row["measured_owner"] is not None for row in tuning["selectors"]),
            "scanned constants a tuning section owns": owned_scanned,
            "scanned constants outside the tuning sections": len(private),
            "of those, class selector": sum(row["class"] == "selector" for row in private),
            "declared selectors outside the tuning sections": len(declared),
            "witness configurations": len(configurations),
            "committed protocol receipts": len(campaigns),
            "core-kernel receipts": sum(row["track"] == "core-kernel" for row in campaigns),
            "core-kernel measurement families": len(core_families),
        },
        "entry_points": [
            {**entry, "routes": [list(route) for route in entry["routes"]]}
            for entry in ENTRY_POINTS
        ],
        "witness_configurations": configurations,
        "route_witnesses": matrix,
        "dependency_features": features,
        "tuning": tuning,
        "private_constants": private,
        "declared_selectors": declared,
        "selector_evidence": evidence(families, tuning["selectors"], private + declared),
        "measurement_families": families,
        "workload_routes": {
            "path": str((ISSUE / "workload-routes.jsonl").relative_to(ROOT)),
            "sha256": sha256(workload_lines.encode()),
        },
    }
    (ISSUE / "workload-routes.jsonl").write_text(workload_lines)
    (ISSUE / "workload-routes.md").write_text(render_workloads(campaigns) + "\n")
    (ISSUE / "route-inventory.json").write_text(json.dumps(inventory, indent=1) + "\n")
    (ISSUE / "route-inventory.md").write_text(render(inventory) + "\n")
    for key, value in inventory["counts"].items():
        print(f"{value:5}  {key}")


if __name__ == "__main__":
    main()
