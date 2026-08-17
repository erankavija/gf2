#!/usr/bin/env python3
"""Generate the phase-1 permanent-campaign manifest and freeze documents.

This is a mechanical document generator for the phase-1 draft. It does not
run a benchmark, invoke jit, or modify .jit/.
"""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
from pathlib import Path


REPO = Path(__file__).resolve().parents[3]
HOME = REPO / "dev/active/b8206228-permanent-statistics"
OUT = HOME / "manifest-draft"
MANIFEST = OUT / "manifest.json"
SELECTION = HOME / "backend-selection-draft.md"

CAMPAIGN_ID = "permanent-zero-fraction-20260817"
ROOT_SEED = int("7a81626200000001", 16)
SHARD_SIZE = 50_000
GIT_REVISION = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO, text=True).strip()

N_VALUES = {
    3: lambda n: 20_000_000 if n <= 20 else 222_223,
    5: lambda n: 16_000_000 if n <= 16 else 160_000,
    7: lambda n: 12_244_898 if n <= 16 else 122_449,
}
FRONTIER = {3: 28, 5: 24, 7: 20}
PREMEASUREMENT = {(3, 28): "accelerator_vs_intra", (5, 24): "batch_parallel_only", (7, 20): "accelerator_only"}
# The GPU-study grids are evidence for nomination only in this draft.  The q3
# verdict is no-go, and the owner excludes the q5/q7 retained prototypes from
# freeze selection.  Only the three exact 296a41c9 frontier rows discharge.
RESOLVED = {
    (3, 28): "accelerator",
    (5, 24): "batch_parallel",
    (7, 20): "accelerator",
}

STUDY_RECEIPTS = {
    3: "dev/studies/047b62ed/receipts.md",
    5: "dev/studies/91605d4d/receipts.md",
    7: "dev/studies/6c7fcb38/receipts.md",
}

FRONTIER_RECEIPT_ROWS = {
    3: "`dev/benchmarks/permanent_campaign/backend-ordering.md` §Verdict table rows A/B (q=3,n=28)",
    5: "`dev/benchmarks/permanent_campaign/backend-ordering.md` §Verdict table row C (q=5,n=24)",
    7: "`dev/benchmarks/permanent_campaign/backend-ordering.md` §Verdict table row D (q=7,n=20)",
}


def nomination(q: int, n: int) -> tuple[str, str, bool, str]:
    """Return two nominees, forced status, and the evidence basis."""
    if q == 3:
        if n <= 15:
            candidates = ("batch_parallel", "generic_ryser")
            study_rows = "§4.2 row n=12"
            b488_rows = "rows q=3,n=12,backend=cpu_rayon_batch_scalar and cpu_ryser_generic"
        else:
            candidates = ("intra_matrix_parallel", "accelerator")
            study_rows = "§4.2 rows n=16, n=20, and n=24"
            b488_rows = "rows q=3,n=20 and q=3,n=24 for cpu_rayon_intra_matrix and gpu_hip"
        forced = False
    elif q == 5:
        candidates = ("batch_parallel", "accelerator")
        study_rows = "§4.2 rows n=12, n=16, and n=20"
        b488_rows = "rows q=5,n=12, q=5,n=16, and q=5,n=20 for cpu_rayon_batch_scalar and gpu_hip"
        forced = False
    elif n >= 17:
        candidates = ("accelerator", "generic_ryser")
        study_rows = "§4.2 row n=20"
        b488_rows = "rows q=7,n=20 for gpu_hip and cpu_ryser_generic"
        forced = True
    else:
        candidates = ("batch_parallel", "accelerator")
        study_rows = "§4.2 rows n=12 and n=16"
        b488_rows = "rows q=7,n=12 and q=7,n=16 for cpu_rayon_batch_scalar and gpu_hip"
        forced = False
    basis = (
        f"`{STUDY_RECEIPTS[q]}` {study_rows}; "
        f"`dev/studies/b488f02c/throughput-2026-08-07.csv` {b488_rows}; "
        f"{FRONTIER_RECEIPT_ROWS[q]}. These receipts nominate the pair for measurement; "
        "they do not rank the gap cell or discharge it."
    )
    return candidates[0], candidates[1], forced, basis

RECEIPTS = [
    "dev/benchmarks/permanent_campaign/backend-ordering.md",
    "dev/benchmarks/permanent_campaign/backend-ordering.csv",
    "dev/benchmarks/permanent_campaign/determinant-cost.md",
    "dev/benchmarks/permanent_campaign/determinant-cost.csv",
    "dev/benchmarks/permanent_campaign/exact-anchors.csv",
    "dev/studies/b488f02c/feasibility-study.md",
    "dev/studies/b488f02c/cargo-tree-2026-08-07.txt",
    "dev/studies/b488f02c/envelope-2026-08-07.csv",
    "dev/studies/b488f02c/throughput-2026-08-07.csv",
    "dev/studies/b488f02c/sustained-2026-08-07.csv",
    "dev/studies/b488f02c/equivalence-2026-08-07.csv",
    "dev/studies/b488f02c/zero-fraction-2026-08-07.csv",
    "dev/studies/b488f02c/determinant-anchor-2026-08-08.txt",
    "dev/studies/b488f02c/order3-anchor-2026-08-08.txt",
    "dev/studies/b488f02c/gpu-hang-2026-08-07.log",
]
for study in ("047b62ed", "91605d4d", "6c7fcb38"):
    RECEIPTS.extend(
        [
            f"dev/studies/{study}/receipts.md",
            f"dev/studies/{study}/permanent-campaign-20260814T230032Z-2085453-q{ {'047b62ed': 3, '91605d4d': 5, '6c7fcb38': 7}[study] }-grid.csv",
            f"dev/studies/{study}/permanent-campaign-20260814T230032Z-2085453-q{ {'047b62ed': 3, '91605d4d': 5, '6c7fcb38': 7}[study] }-grid.log",
            f"dev/studies/{study}/permanent-campaign-20260814T230032Z-2085453-q{ {'047b62ed': 3, '91605d4d': 5, '6c7fcb38': 7}[study] }-gray-update.csv",
            f"dev/studies/{study}/permanent-campaign-20260814T230032Z-2085453-q{ {'047b62ed': 3, '91605d4d': 5, '6c7fcb38': 7}[study] }-gray-update.log",
            f"dev/studies/{study}/permanent-campaign-20260814T230032Z-2085453-q{ {'047b62ed': 3, '91605d4d': 5, '6c7fcb38': 7}[study] }-horizontal-product.csv",
            f"dev/studies/{study}/permanent-campaign-20260814T230032Z-2085453-q{ {'047b62ed': 3, '91605d4d': 5, '6c7fcb38': 7}[study] }-horizontal-product.log",
            f"dev/studies/{study}/permanent-campaign-20260814T230032Z-2085453.provenance.txt",
            f"dev/studies/{study}/permanent-campaign-20260814T230032Z-2085453.run-summary.txt",
        ]
    )
RECEIPTS.extend(
    [
        "dev/studies/047b62ed/permanent-campaign-20260814T230032Z-2085453-shared-equivalence.csv",
        "dev/studies/047b62ed/permanent-campaign-20260814T230032Z-2085453-shared-equivalence.log",
        "dev/studies/6c7fcb38/hip-resource-usage-20260814T172506Z-1610002/receipt.txt",
        "dev/studies/6c7fcb38/profiled-20260815T181923Z/provenance.txt",
    ]
)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def cell_evidence(q: int, n: int) -> list[str]:
    evidence = []
    if (q, n) in PREMEASUREMENT:
        evidence.append("296a41c9 four-configuration cohort")
    if n in {12, 16, 20, 24, 28}:
        evidence.append("b488f02c throughput/envelope")
    if n in {8, 12, 16, 20, 24, 28}:
        evidence.append(f"GPU study q{q} grid")
    if (q, n) in {(3, 4), (3, 12), (3, 20), (3, 28), (5, 4), (5, 12), (5, 20), (5, 24), (7, 4), (7, 12), (7, 20)}:
        evidence.append("determinant-cost marginal receipt")
    return evidence or ["none at the cell"]


def comparable_candidates(q: int, n: int) -> str:
    if q == 3 and n in {12, 16, 20, 24}:
        return "q3 GPU-study evidence: batch_parallel, intra_matrix_parallel, accelerator, scalar, generic_ryser, and AVX2 rows; q3 no-go makes these nomination evidence only"
    if q == 3 and n == 28:
        return "296a cohort: accelerator and intra_matrix_parallel; q3 GPU-study rows are not cross-ranked"
    if q == 5 and n in {12, 16, 20}:
        return "q5 GPU-study evidence: batch_parallel, accelerator, scalar, and generic_ryser; retained f5-three-plane is inventory evidence only; intra_matrix_parallel and AVX2 are capability-excluded"
    if q == 5 and n == 24:
        return "296a cohort: batch_parallel only; q5 GPU-study accelerator row is censored"
    if q == 7 and n in {12, 16}:
        return "q7 GPU-study evidence: batch_parallel, generic_ryser, and accelerator; retained f7-three-plane-permanent is inventory evidence only; packed scalar/Rayon/AVX2 rows are capability-excluded"
    if q == 7 and n == 20:
        return "296a cohort: accelerator only; q7 GPU-study generic/accelerator rows are not cross-ranked"
    if n == 8:
        return "equivalence-only rows; no cell-applicable composite timing"
    return "no cell-applicable composite timing in a comparable committed cohort"


def candidate_set(q: int, n: int) -> str:
    if q == 3:
        return "scalar, batch_parallel, batch_parallel_avx2, intra_matrix_parallel, accelerator, generic_ryser, avx2"
    if q == 5:
        return "scalar, batch_parallel, accelerator, generic_ryser (intra-matrix and AVX2 are unsupported)"
    if n <= 16:
        return "scalar, batch_parallel, accelerator, generic_ryser (intra-matrix and AVX2 are unsupported)"
    return "accelerator, generic_ryser (packed scalar and Rayon paths are unsupported above n=16)"


def rule12(q: int, n: int) -> str:
    if (q, n) in RESOLVED:
        if (q, n) == (3, 28):
            return "Rule 1: accelerator and intra-matrix pass shared equivalence; Rule 2: both satisfy host/build/capability/resource conditions."
        return "Rule 1: the measured backend passes the shared equivalence/anchor evidence; Rule 2: it satisfies the recorded host/build/capability/resource conditions."
    if q == 3 and n in {12, 16, 20, 24}:
        return "The GPU-study row and the interior-window finding pass their recorded study checks, but the field verdict is no-go; the row is preserved evidence and does not provide a freeze selection."
    if q in {5, 7} and n in ({12, 16, 20} if q == 5 else {12, 16}):
        return "The study row records equivalence, capability, and resource evidence, but the retained prototype is not selectable at freeze by the owner decision dated 2026-08-17; the row remains inventory evidence."
    if n in {12, 16, 20, 24, 28}:
        return "Rule 1/2 evidence identifies candidate capability and equivalence status, but no cell-applicable composite cohort is dischargeable at this order; the protocol requires same-cohort remeasurement."
    return "Rule 1/2 has no cell-applicable composite candidate row at this order; equivalence-only evidence does not supply timing, so the protocol requires same-cohort remeasurement."


def rule3(q: int, n: int) -> str:
    if (q, n) == (3, 28):
        return "ranked: accelerator mean 19.359758 matrices/s exceeds intra-matrix Rayon mean 17.994302"
    if (q, n) in {(5, 24), (7, 20)}:
        return "only-one-eligible: the 296a cohort contains one eligible configuration"
    return "not applicable: same-cohort timing is absent or unresolved"


def cell_status(q: int, n: int) -> str:
    return "dischargeable now" if (q, n) in RESOLVED else "gap: placeholder only"


def make_manifest(selection_hash: str) -> dict:
    cells = []
    ordinal = 0
    for q in (3, 5, 7):
        for n in range(4, FRONTIER[q] + 1):
            matrix_count = N_VALUES[q](n)
            shard_count = (matrix_count + SHARD_SIZE - 1) // SHARD_SIZE
            shards = [
                {
                    "shard_id": shard_id,
                    "stream_index": (ordinal << 32) | shard_id,
                }
                for shard_id in range(shard_count)
            ]
            cells.append(
                {
                    "q": q,
                    "n": n,
                    "matrix_count": matrix_count,
                    "shard_size": SHARD_SIZE,
                    "shards": shards,
                    "backend": RESOLVED.get((q, n), "generic_ryser"),
                    "backend_receipt": {
                        "path": "dev/active/b8206228-permanent-statistics/backend-selection-draft.md",
                        "sha256": selection_hash,
                    },
                    "determinant_companion": "evaluate",
                }
            )
            ordinal += 1
    return {
        "schema_version": 1,
        "campaign_id": CAMPAIGN_ID,
        "root_seed": ROOT_SEED,
        "stream_purposes": [
            {"name": "validation", "tag": 1},
            {"name": "timing", "tag": 2},
            {"name": "campaign-cells", "tag": 3},
            {"name": "rare-event", "tag": 4},
        ],
        "cells": cells,
        "provenance": {
            "git_revision": GIT_REVISION,
            "compiler_version": "rustc 1.95.0 (59807616e 2026-04-14)",
            "rng_algorithm": "cha_cha20",
            "rng_version": "rand_chacha 0.9.0",
            "invocation": ["permanent_campaign", "--manifest", "manifest.json", "--workers", "1"],
            "accelerator_runtime": {"state": "present", "value": "ROCm 7.2.4"},
            "cpu_model": "AMD Ryzen 9 5900X 12-Core Processor",
            "gpu_model": {"state": "present", "value": "AMD Radeon RX 6950 XT (gfx1030)"},
        },
    }


def inventory_rows() -> list[str]:
    rows = []
    for q in (3, 5, 7):
        for n in range(4, FRONTIER[q] + 1):
            rows.append(
                f"| {q} | {n} | {N_VALUES[q](n):,} | {'; '.join(cell_evidence(q, n))} | {comparable_candidates(q, n)} | {rule12(q, n)} | {rule3(q, n)} | {cell_status(q, n)} |"
            )
    return rows


def gap_rows() -> list[str]:
    rows = []
    for q in (3, 5, 7):
        for n in range(4, FRONTIER[q] + 1):
            if (q, n) in RESOLVED:
                continue
            first, second, forced, basis = nomination(q, n)
            nomination_kind = "forced by capability: at most two eligible candidates" if forced else "evidence-based nomination"
            rows.append(
                f"| {q} | {n} | {N_VALUES[q](n):,} | `{first}`, `{second}` | {nomination_kind} | {basis} | 12 fresh processes per configuration, interleaved under the canonical full-host lock | at least 3 s configuration warm-up, at least 5 timed repetitions and 5 timed seconds, fixed 120 s per-process cap; no result-dependent extension or replacement |"
            )
    return rows


def nomination_totals() -> tuple[int, int, int, int, int]:
    cells = 0
    forced = 0
    for q in (3, 5, 7):
        for n in range(4, FRONTIER[q] + 1):
            if (q, n) not in RESOLVED:
                cells += 1
                forced += nomination(q, n)[2]
    chosen = cells - forced
    return cells, forced, chosen, cells * 2, cells * 2 * 12


def receipt_inventory() -> str:
    lines = [
        "# Draft receipt inventory",
        "",
        "This draft inventories committed artifacts relevant to backend selection. It separates cell-applicable measurements from evidence that cannot be ranked under the backend-freeze cohort rules. Every path below is repository-relative and carries its current SHA-256.",
        "",
        "## Canonical committed receipt files",
        "",
            "| Path | SHA-256 | Role and protocol reading |",
            "|---|---|---|",
    ]
    for rel in RECEIPTS:
        path = REPO / rel
        role = "raw composite or supporting receipt"
        if "backend-ordering" in rel:
            role = "296a41c9 four-configuration replicated composite cohort"
        elif "determinant-cost" in rel:
            role = "8cb4def5 determinant marginal cost; not a selection receipt"
        elif "envelope" in rel or "throughput" in rel or "sustained" in rel:
            role = "b488f02c feasibility/throughput evidence; not the 296a cohort"
        elif "equivalence" in rel or "anchor" in rel or "resource" in rel:
            role = "rule-1 equivalence or rule-2 safety/capability evidence"
        elif "zero-fraction" in rel or "gpu-hang" in rel:
            role = "preserved by-product or failure evidence; never a ranking input"
        elif "receipts.md" in rel:
            role = "study index and protocol/provenance receipt"
        elif "provenance" in rel or "run-summary" in rel:
            role = "study source/build/host and execution-status provenance"
        lines.append(f"| `{rel}` | `{sha256(path)}` | {role} |")
    lines.extend(
        [
            "",
            "The b488f02c study's `throughput`, `sustained`, and `envelope` receipts use the AMD Ryzen 9 5900X host but pin an older source/build and a different timing run from 296a41c9. The three GPU study grids report end-to-end candidate timings in internally matched q3, q5, and q7 study cohorts; those cohorts pin source revision `292320d5`, a different binary set, a different grid schedule, and a different per-cell timing protocol. They remain factual nomination evidence, not freeze selections. The q3 interior window remains the falsification-preserved `fold-gf3` finding in `dev/studies/0dffa759/findings.md` §3.2, while §3 records the q3 no-go. The owner decision dated 2026-08-17 keeps the retained `f5-three-plane` and `f7-three-plane-permanent` prototype rows as inventory evidence only. Their profiling receipts report kernel/resource facts and are not composite selection receipts. The determinant receipt measures evaluation only and therefore supplies no draw-pack-evaluate-count ranking.",
            "",
            "## Per-cell feasibility inventory",
            "",
            "The following table covers every one of the 63 protocol cells. `Comparable candidates` names the backend rows that can enter rule 3 in a matched cohort; capability and equivalence exclusions are stated in the adjacent rule-1/rule-2 column. A gap has no dischargeable rule-3 timing and remains a placeholder in the manifest.",
            "",
            "| q | n | Fixed N | Committed evidence | Comparable candidates | Rule 1 / Rule 2 result | Rule 3 result | Status |",
            "|---:|---:|---:|---|---|---|---|---|",
        ]
    )
    lines.extend(inventory_rows())
    lines.extend(
        [
            "",
            "## Inventory summary",
            "",
            "Of the 63 protocol cells, exactly 3 are dischargeable: q=3 has 1 at n=28, q=5 has 1 at n=24, and q=7 has 1 at n=20. The other 60 cells are gap rows with `generic_ryser` placeholders. The q3 n=16,20,24 study rows remain preserved finding evidence and do not discharge cells; the q5/q7 retained prototype rows remain factual inventory evidence under the owner decision dated 2026-08-17.",
            "",
            "## Retained study-prototype evidence",
            "",
            "The following factual rows remain in the inventory even though they contribute no freeze selection. `dev/studies/91605d4d/receipts.md` §4.2 rows n=12, 16, 20, and 24 report the `f5-three-plane` go prototype; `dev/studies/6c7fcb38/receipts.md` §4.2 rows n=12, 16, 20, 24, and 28 report the `f7-three-plane-permanent` go prototype. The owner decision dated 2026-08-17 rules both retained study-cohort prototypes out of freeze selection; premeasurement settles their gap cells. `dev/studies/0dffa759/findings.md` §3 and §3.2 report the q3 no-go and preserve the `fold-gf3` interior-window finding without retaining an accelerator configuration.",
            "",
            "The CSV rows in `backend-ordering.csv` name 48 scratch raw paths under `dev/research/permanent-sampling-feas/target/`; those scratch paths are not committed dataset artifacts in this worktree. The committed CSV is the canonical receipt and preserves the scratch SHA-256 identities, but the scratch bytes are not independently re-hashable here. That limitation is recorded as unverifiable evidence rather than silently treated as a second committed receipt.",
        ]
    )
    return "\n".join(lines) + "\n"


def selection_receipt() -> str:
    lines = [
        "# Draft backend selection receipt",
        "",
        "Status: draft. This file is the single multi-cell selection receipt bound by the draft manifest. It carries one addressable cell decision per row; unresolved rows carry a placeholder backend and a freeze-blocking status. A final freeze replaces this draft with a committed receipt whose bytes are hashed into `CellSpec.backend_receipt`.",
        "",
        "## Selection contract",
        "",
        "The controlling contract is `dev/simulation_results/permanent-zero-fraction/protocol.md`, section `Backend freeze`. Rule 1 requires shared per-matrix behavioral equivalence and validation-anchor passage. Rule 2 requires host/build safety, capability, launch-duration, and resource conditions. Rule 3 requires a cell-applicable composite draw-pack-evaluate-count receipt; ranking is within one matched cohort only. The exact 296a41c9 cohort uses four configurations, twelve fresh processes per configuration, one initial 90-second machine warm-up, at least 3 seconds of per-configuration warm-up, at least five timed repetitions and five timed seconds, and a fixed 120-second per-process cap.",
        "",
        "## Raw receipts considered",
        "",
        "The committed paths and SHA-256 values are listed in [`receipt-inventory.md`](receipt-inventory.md). The 296a41c9 canonical raw receipt is `dev/benchmarks/permanent_campaign/backend-ordering.csv`; the narrative is `backend-ordering.md`. The determinant companion is `dev/benchmarks/permanent_campaign/determinant-cost.csv`, and the b488f02c and GPU-study artifacts provide older, differently timed, or supporting evidence.",
        "",
        "## Candidate provenance for the dischargeable cohort",
        "",
        "| Candidate | Host / accelerator | Source and compiler | Build, binary, workers | Workload, warm-up, repetitions | Rule-1 / Rule-2 evidence |",
        "|---|---|---|---|---|---|",
        "| `accelerator` at $(q,n)=(3,28)$ | AMD Ryzen 9 5900X; AMD Radeon RX 6950 XT `gfx1030`; ROCm 7.2.4; driver/kernel `7.1.6-arch1-1` | harness `414d31f8`; Rust 1.95.0; HIP build | release + `hip`; binary `6e24533cfbac987a0cec20af02f9dfb0a7bd9ce12c9e80cbdc00cad72150ccad`; one GPU worker; powersave | batch $M=1024$; initial locked 90 s machine warm-up, then per-process configuration warm-up; 3 timed repetitions; 12 fresh processes | q3 shared equivalence rows include n=28 with zero mismatches; backend-ordering records the capability and resource state |",
        "| `intra_matrix_parallel` at $(q,n)=(3,28)$ | AMD Ryzen 9 5900X; 24 logical CPUs; AVX2; no AVX-512F; powersave | harness `414d31f8`; Rust 1.95.0 | release + `hip`; same binary hash; 24 Rayon workers; same lock | cell $M=96$; same locked warm-up discipline; 5 timed repetitions; 12 fresh processes | q3 shared equivalence rows include n=28 with zero mismatches; full-host Rayon and lock conditions are recorded |",
        "| `batch_parallel` at $(q,n)=(5,24)$ | AMD Ryzen 9 5900X; 24 logical CPUs; AVX2; no AVX-512F; powersave | harness `414d31f8`; Rust 1.95.0 | release + `hip`; same binary hash; 24 Rayon workers; same lock | batch $M=96$; same locked warm-up discipline; 5 timed repetitions; 12 fresh processes | q5 shared equivalence evidence and backend-ordering capability record |",
        "| `accelerator` at $(q,n)=(7,20)$ | AMD Ryzen 9 5900X; AMD Radeon RX 6950 XT `gfx1030`; ROCm 7.2.4; driver/kernel `7.1.6-arch1-1` | harness `414d31f8`; Rust 1.95.0; HIP build | release + `hip`; same binary hash; one GPU worker; powersave | batch $M=1024$; same locked warm-up discipline; 5 timed repetitions; 12 fresh processes | q7 n=20 shared equivalence/resource evidence; packed CPU paths above n=16 are excluded by capability |",
        "",
        "## Study evidence excluded from freeze selection",
        "",
        "The q3, q5, and q7 GPU-study grids remain committed factual evidence. Their candidate rows use source revision `292320d5e7d1fefd6b94262d0960e40c8ea35ba1`, Rust 1.95.0 (`rustc 1.95.0 (59807616e 2026-04-14)`), HIP `7.2.53211-9999` / AMD clang `22.0.0git`, release + `hip`, and the AMD Ryzen 9 5900X / Radeon RX 6950 XT `gfx1030` host under `powersave` with ROCm 7.2.4 and kernel `7.1.6-arch1-1`. The grid receipts identify each candidate's $(q,n)$ workload, batch size, worker count, warm-up, timed repetitions, and 120-second process cap. The q3 finding is no-go under `dev/studies/0dffa759/findings.md` §3; its n=16,20,24 interior window is preserved under §3.2 and does not retain an accelerator configuration. The owner decision dated 2026-08-17 rules the retained `f5-three-plane` and `f7-three-plane-permanent` study prototypes out of freeze selection; their factual rows remain in `receipt-inventory.md`.",
        "",
        "| Study rows retained as evidence | Cohort and workload | Factual study result | Freeze status |",
        "|---|---|---|---|",
        "| q3 n=16,20,24 | q3 grid; end-to-end composite draw-pack-evaluate-count; CPU and device rows | The interior-window rates are preserved as the confounded `fold-gf3` finding | `dev/studies/0dffa759/findings.md` §3 is no-go and §3.2 preserves the finding; no freeze selection |",
        "| q5 n=12,16,20,24 | q5 grid; end-to-end composite rows including `f5-three-plane` | `f5-three-plane` is a retained go prototype in the study receipt | Owner decision dated 2026-08-17 excludes the study-cohort prototype from freeze selection; inventory evidence remains |",
        "| q7 n=12,16,20,24,28 | q7 grid; end-to-end composite rows including `f7-three-plane-permanent` | `f7-three-plane-permanent` is a retained go prototype in the study receipt | Owner decision dated 2026-08-17 excludes the study-cohort prototype from freeze selection; inventory evidence remains |",
        "",
        "These study rows are nomination evidence only. The protocol does not permit ranking them against the 296a41c9 measurements because source revision, binary identity, workload schedule, and timing protocol differ. The only dischargeable decisions in this draft are the exact 296a41c9 rows at $(3,28)$, $(5,24)$, and $(7,20)$.",
        "",
        "## Cell decisions",
        "",
        "| q | n | Selected backend in this draft | Rule 3 outcome | Receipt status |",
        "|---:|---:|---|---|---|",
    ]
    for q in (3, 5, 7):
        for n in range(4, FRONTIER[q] + 1):
            selected = RESOLVED.get((q, n), "generic_ryser (placeholder)")
            lines.append(f"| {q} | {n} | `{selected}` | {rule3(q, n)} | {cell_status(q, n)} |")
    lines.extend(
        [
            "",
            "The $(3,28)$ ranking uses the 296a41c9 pooled composite means: 19.359758 matrices/s for the accelerator and 17.994302 matrices/s for intra-matrix Rayon; the receipt reports the independent 12-process comparison and its conservative interval. The study rows are not selections. The $(5,24)$ and $(7,20)$ rows use the protocol's only-one-eligible path, with exclusions recorded in `backend-exclusions-draft.md`; they do not claim a cross-backend ranking.",
            "",
            "The draft does not use permanent or determinant zero counts, intervals, or test outcomes as selection inputs. The determinant companion is evaluated on every cell and its marginal timing remains separate from backend selection, as required by the protocol.",
        ]
    )
    return "\n".join(lines) + "\n"


def exclusions() -> str:
    return """# Draft backend exclusion records

Status: draft. These records explain why a candidate does not enter rule 3 for each field family. They do not turn a missing timing cohort into a selection.

## Rule 1: behavioral and equivalence exclusions

The committed shared-equivalence receipts report per-matrix agreement against the field reference for the candidates that execute at their listed orders. A candidate with no cell row in the committed equivalence receipt has no rule-1 passage for that cell. The GPU study receipt records q3, q5, and q7 equivalence rows at its measured orders; the b488f02c equivalence receipt is an older supporting check. These receipts do not replace the missing same-cohort timing receipt.

## Rule 2: safety and capability exclusions

- $\\mathbb{F}_5$: AVX2 and intra-matrix Rayon are unsupported in the in-tree candidate registry; the study receipt records those candidates as unsupported by library capability. This is a capability exclusion, not a GPU resource falsification.
- $\\mathbb{F}_7$, $n>16$: packed scalar, packed Rayon, and AVX2 paths are excluded by the documented 16-lane limit; the generic Ryser and accelerator paths remain the candidate family where their rule-1 evidence exists.
- GPU candidates: the study resource receipts record the AMD Radeon RX 6950 XT `gfx1030`, ROCm 7.2.4, driver/kernel identity, and launch/resource observations. A GPU study row remains non-comparable to the 296a cohort when its source, binary, workload shape, or timing protocol differs.
- Any candidate absent from a cell-applicable committed equivalence row is excluded by rule 1 for that cell until a matching validation/equivalence receipt is committed.

## Rule 3 exclusions

The b488f02c rows are not ranked against either the 296a41c9 cohort or the GPU-study cohorts. They differ in source/build identity and timing protocol, and the envelope includes projected rows without measured composite rates. The determinant-cost receipt measures evaluation-only marginal cost. These facts exclude those rows from rule-3 selection; they do not establish that a backend is mathematically or mechanically invalid.

The GPU-study grids are nomination evidence only. Their source revision, binary set, schedule, and timing protocol differ from 296a41c9, so their rows do not discharge or cross-rank any cell. The q3 no-go in `dev/studies/0dffa759/findings.md` §3 excludes retention of q3 n=16,20,24 device rows; §3.2 preserves the confounded `fold-gf3` finding. The owner decision dated 2026-08-17 excludes the retained `f5-three-plane` and `f7-three-plane-permanent` study prototypes from freeze selection while keeping their factual inventory rows. The 296a41c9 cohort discharges only q3 n=28, q5 n=24, and q7 n=20.

The q3 n=16,20,24 rows therefore use `generic_ryser` placeholders in the manifest, selection receipt, and inventory, and they are gap rows. They do not carry a retained accelerator configuration from the GPU study.

The only-one-eligible path applies to $(q,n)=(5,24)$ and $(7,20)$ in the current 296a cohort. The ranked path applies only to $(3,28)$. For every one of the 60 gap cells, exactly two candidates are nominated in `gap-list.md`; every eligible but non-nominated candidate is recorded as a rule-3 exclusion for that cell. The three forced nominations are q7 n=17,18,19, where capability leaves only `accelerator` and `generic_ryser`.

## Per-gap rule-3 exclusion ledger

| Gap cells | Eligible candidate count | Nominated count | Non-nominated rule-3 exclusions |
|---|---:|---:|---:|
| q3 n=4..15 | 7 | 2 | 60 |
| q3 n=16..27, including n=16,20,24 | 7 | 2 | 60 |
| q5 n=4..23 | 4 | 2 | 40 |
| q7 n=4..16 | 4 | 2 | 26 |
| q7 n=17..19 (capability-forced pair) | 2 | 2 | 0 |
| **Total** |  |  | **186** |
"""


def gaps() -> str:
    cells, forced, chosen, configurations, processes = nomination_totals()
    lines = [
        "# Draft gap list",
        "",
        f"A gap is a core cell for which no backend selection is dischargeable from committed evidence under the protocol's cohort rules. This draft has {cells} gaps: exactly two candidates are nominated per gap for premeasurement, with {forced} forced nominations and {chosen} evidence-based nominations. The nominations choose what to measure; they are not a ranking and do not discharge a cell. The protocol therefore requires a same-cohort remeasurement before freeze. No measurement runs in this phase.",
        "",
        "## Minimal closing premeasurement",
        "",
        f"The run has {cells} cells, {configurations} nominated configurations, and {processes} fresh processes: q3 has 24 cells, 48 configurations, and 576 processes; q5 has 20 cells, 40 configurations, and 480 processes; q7 has 16 cells, 32 configurations, and 384 processes. For each row, the minimal run is one 296a41c9-pattern cohort at that exact $(q,n)$: one configuration for each of the two nominees, twelve fresh processes per configuration, interleaved under the canonical full-host lock. The first locked process performs the 90-second machine warm-up once; every process performs at least 3 seconds of configuration warm-up and at least 5 timed repetitions and 5 timed seconds; every process has the fixed 120-second cap. Sampling count and stopping rule are fixed before timing. Failures are recorded without replacement, and no result-dependent extension or replacement is allowed. The measurement is composite draw-pack-evaluate-count throughput. It does not run now.",
        "",
        "| q | n | Fixed N | Exactly two nominated candidates | Nomination kind | Nomination basis (path + section/row) | Process count / interleave | Stopping rule |",
        "|---:|---:|---:|---|---|---|---|---|",
    ]
    lines.extend(gap_rows())
    lines.extend(
        [
            "",
            "The three resolved cells are not gaps: $(3,28)$ has a ranked two-candidate 296a cohort, $(5,24)$ has one eligible batch-parallel configuration, and $(7,20)$ has one eligible accelerator configuration. The 60 `generic_ryser` placeholders remain freeze-blocking; a placeholder never licenses campaign draws. Every non-nominated candidate in a gap is rule-3-excluded in `backend-exclusions-draft.md` and must not be substituted after timing begins.",
        ]
    )
    return "\n".join(lines) + "\n"


def overview(inventory_hash: str, manifest_hash: str, selection_hash: str) -> str:
    cells, forced, chosen, configurations, processes = nomination_totals()
    return f"""# Phase-1 freeze-feasibility inventory and draft manifest

Status: draft. This artifact set supports feasibility review for JIT issue `7a816262`; it is not a final freeze and it does not authorize campaign draws.

## Outcome

The draft enumerates all 63 protocol cells, uses the two fixed N tiers, evaluates the determinant companion on every cell, and parses through the real `CampaignManifest` reader. Exactly three cells are dischargeable now: q=3 has 1, q=5 has 1, and q=7 has 1. The remaining {cells} cells carry `generic_ryser` only as a schema-valid placeholder and are listed in the gap document. The protocol's same-cohort remeasurement clause therefore still blocks final freeze.

The selected cells are q3 n=28, q5 n=24, and q7 n=20 from the exact 296a41c9 cohort. The q3 GPU-study n=16,20,24 interior window remains the confounded `fold-gf3` finding in `dev/studies/0dffa759/findings.md` §3.2; §3 records the q3 no-go, so no q3 accelerator configuration is retained. Owner decision (2026-08-17), recorded by the owner: retained `f5-three-plane` and `f7-three-plane-permanent` study-cohort prototypes are not selectable at freeze, and their cells remain gaps settled empirically by premeasurement. Their factual receipt rows remain in the inventory. The selection receipt records the candidate provenance and within-cohort ranking for the three selected groups; exclusion records distinguish rule 1, rule 2, and rule 3 exclusions.

## Draft identity and addressing

The draft campaign id is `{CAMPAIGN_ID}`. It is date-qualified, valid under `CampaignId`, and names the dataset directory the final campaign uses if the protocol and cell universe remain unchanged. The root seed is `0x{ROOT_SEED:016x}`. Purpose tags use the sampler's reserved namespace: validation=1, timing=2, campaign-cells=3, rare-event=4. Every shard uses the campaign-cell purpose tag 3.

The stream index is allocated as `((cell_ordinal as u64) << 32) | shard_id`, with cells ordered lexicographically by `(q,n)` and `shard_id` starting at zero. The low-56-bit schema limit and the scheduler's shard-id limit hold. Distinct purpose tags occupy separate seed-address domains; distinct cells have distinct `(q,n)` coordinates and distinct high-order stream-index slots; distinct shards have distinct low-order slots. This is the construction claimed for REQ-08, not an after-the-fact collision inspection.

## N, error budget, and determinant plan

The cell counts are the protocol's exact values: q=3 uses 20,000,000 for n=4..20 and 222,223 for n=21..28; q=5 uses 16,000,000 for n=4..16 and 160,000 for n=17..24; q=7 uses 12,244,898 for n=4..16 and 122,449 for n=17..20. The multiplicity is K=63. The permanent-floor and determinant families each use family budget 0.025 and per-cell level 1/2520. The manifest schema has no error-budget fields, so the freeze document binds these protocol values by citation rather than adding unknown JSON fields.

`determinant_companion` is `evaluate` for all 63 cells. The determinant-cost receipt supplies marginal-cost evidence but never acts as a backend ranking receipt.
`dev/benchmarks/permanent_campaign/determinant-cost.md` §Verdict and §Twelve-hour budget projection record no marginal-cost failures; the largest measured addition is 0.014288 h at (q,n)=(3,20), with 0.000328 h at (3,28), 0.000187 h at (5,24), and 0.000098 h at (7,20). The receipt's §Measured ratio and §Twelve-hour budget projection tables record the other measured anchor cells and the fixed-N calculation.

## Integrity and reproducibility

The draft uses the implemented integrity contract: `manifest.json` carries no self-hash field; `provenance::manifest_content_hash` recomputes SHA-256 from the manifest bytes, and `recorded_manifest_hash` reads the `manifest.json` entry from `checksums.sha256`. The draft sidecar records that digest. A final freeze generates the complete raw-data integrity file through `permanent_dataset checksums`, and verification compares the recomputed manifest hash with the sidecar entry. No new hash mechanism is introduced.

The manifest records `rng_algorithm` as the schema token `cha_cha20` (ChaCha20), `rng_version` as `rand_chacha 0.9.0`, and an argument-token invocation. The exact patch-version derivation is `dev/studies/0dffa759/rng-provenance-addendum.md` §4 and `@/citation/RustRandom2025`.

REQ-07 is a freeze rule: adding a cell after this draft requires a new campaign id and a restated multiplicity adjustment; the added cell does not reuse this campaign identity or its 63-cell correction.

## Gap premeasurement totals

The gap list nominates exactly two candidates per gap: {configurations} configurations and {processes} fresh processes. q7 n=17,18,19 are forced because capability leaves only `accelerator` and `generic_ryser`; the other {chosen} cells use evidence-based nominations. The b488f02c throughput rows, the q3/q5/q7 GPU-study §4.2 rows, and the 296a41c9 frontier table rows provide nomination basis only. Non-nominated candidates are rule-3-excluded with recorded exclusions; no measurement runs in this draft.

## Validation record

The draft reader validation is recorded in [`validation.md`](validation.md). It invokes the real `read_manifest` through the `gf2-sim` binary test harness and returns success. It does not run a campaign arm, create shards, or run a benchmark.

## Artifact hashes

| Artifact | SHA-256 | Role |
|---|---|---|
| [`manifest.json`](manifest-draft/manifest.json) | `{manifest_hash}` | schema-valid 63-cell draft |
| [`backend-selection-draft.md`](backend-selection-draft.md) | `{selection_hash}` | manifest-bound draft selection receipt |
| [`receipt-inventory.md`](receipt-inventory.md) | `{inventory_hash}` | committed receipt inventory |

See [`receipt-inventory.md`](receipt-inventory.md), [`backend-selection-draft.md`](backend-selection-draft.md), [`backend-exclusions-draft.md`](backend-exclusions-draft.md), and [`gap-list.md`](gap-list.md) for the evidence records. Unverifiable items are the uncommitted scratch raw paths named inside `backend-ordering.csv`, plus any final selection receipt hashes that depend on post-draft edits.
"""


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    SELECTION.write_text(selection_receipt(), encoding="utf-8")
    selection_hash = sha256(SELECTION)
    manifest = make_manifest(selection_hash)
    MANIFEST.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    manifest_hash = sha256(MANIFEST)
    (OUT / "checksums.sha256").write_text(f"{manifest_hash}  manifest.json\n", encoding="utf-8")
    inventory = receipt_inventory()
    (HOME / "receipt-inventory.md").write_text(inventory, encoding="utf-8")
    inventory_hash = sha256(HOME / "receipt-inventory.md")
    (HOME / "backend-exclusions-draft.md").write_text(exclusions(), encoding="utf-8")
    (HOME / "gap-list.md").write_text(gaps(), encoding="utf-8")
    (HOME / "freeze-feasibility-draft.md").write_text(overview(inventory_hash, manifest_hash, selection_hash), encoding="utf-8")
    print(f"manifest={MANIFEST}")
    print(f"manifest_sha256={manifest_hash}")
    print(f"selection_sha256={selection_hash}")
    print(f"inventory_sha256={inventory_hash}")
    print(f"cells=63 resolved={len(RESOLVED)} gaps={63 - len(RESOLVED)}")


if __name__ == "__main__":
    main()
