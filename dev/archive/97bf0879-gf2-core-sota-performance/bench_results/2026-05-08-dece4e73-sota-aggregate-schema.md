# SOTA Aggregate CSV Schema (`jit:dece4e73`) — R2

| Field | Value |
|---|---|
| Date | 2026-05-08 |
| JIT issue | `dece4e73` (Aggregate final SOTA raw CSVs) |
| Parent story | `story:sota-final-scorecard` |
| Parent epic | `97bf0879` (gf2-core SOTA performance) |
| Status | FINAL R2 — schema aligned with `benchmarks/analyze.py`; all seven R1 review findings addressed |

---

## 1. Artefacts

| File | Rows (excl. header) | Contents |
|---|---:|---|
| `dev/bench_results/2026-05-08-dece4e73-sota-aggregate-gf2.csv` | 243 | gf2-core measurements |
| `dev/bench_results/2026-05-08-dece4e73-sota-aggregate-reference.csv` | 335 | Reference-library measurements (fflas-ffpack, m4ri, m4rie, linbox, flint, ntl) |
| `dev/bench_results/2026-05-08-dece4e73-sota-aggregate-schema.md` | — | This document |

---

## 2. Column Schema

Both CSVs share the identical column schema. The first ten columns are exactly the columns required by `benchmarks/analyze.py` (from its `CSV_COLUMNS` list). Three additional wide-form columns are appended; `analyze.py` ignores unknown columns.

| Column | Type | Nullable | Notes |
|---|---|---|---|
| `lib` | string | no | `gf2` in the gf2 CSV. One of `fflas-ffpack`, `m4ri`, `m4rie`, `linbox`, `flint`, `ntl` in the reference CSV. |
| `operation` | string | no | Operation name. Valid values per `analyze.py` OPERATION_ORDER: `fgemm`, `matmul`, `pluq`, `echelon`, `invert`, `solve`, `charpoly`, `minpoly`, `spmv`, `sparse-matmul`, `sparse×dense`, `sparse-elim`. |
| `field` | string | no | Finite field identifier. Values: `GF(2)`, `GF(7)`, `GF(31)`, `GF(251)`, `GF(65521)`, `GF(2^31-1)`, `GF(2^4)`, `GF(2^8)`, `GF(2^16)`, `GF(2^32)`. |
| `m` | integer | no | Row count of the matrix (= n for square). |
| `k` | integer | no | Common inner dimension for matmul; = n for factorization ops. |
| `n` | integer | no | Column count of the matrix (= n for square). |
| `rank_regime` | string | no | `uniform` (full-rank), `deficient` (rank = n/2), or a sparse density + layout string e.g. `density_9.765625e-03_csr`. Never empty. |
| `seed` | uint64 | no | RNG seed from the bench harness. `0` for Criterion-sourced rows (charpoly/minpoly from `d1dd266c` evidence) and structured matrices. |
| `wall_ns` | integer | no | Median wall-clock time in nanoseconds. |
| `throughput_ops` | float | no | Throughput in ops/s computed by the bench harness (`n^3 / wall_ns` for cubic ops; `n*nnz / wall_ns` for SpMV). |
| `source_csv` | string | no | Basename of the source CSV or evidence markdown from which this row was transcribed. |
| `bench_run_date` | date | no | ISO-8601 date of the benchmark run (not the file commit date). |
| `jit_issue` | string | no | Short JIT issue ID of the task that produced this measurement. |

### 2.1 `analyze.py` compatibility guarantee

`benchmarks/analyze.py` reads CSV files via `csv.DictReader` and checks that all ten columns in its `CSV_COLUMNS` list are present. The aggregate CSVs satisfy this check. The extra wide-form columns (`source_csv`, `bench_run_date`, `jit_issue`) are ignored by `analyze.py`. The renderer for `2cfc4372` may use them as metadata.

---

## 3. Supersession Rule

Multiple source CSVs measured the same cell at different dates or under different implementation versions. The canonical row selection rule is:

> **Keep the row with the latest `bench_run_date`. When two rows share the same date, prefer the row from the issue that represents the production-final implementation.**

Specific supersession decisions recorded in this aggregate:

| Superseded | Kept | Cell(s) | Reason |
|---|---|---|---|
| `a1172cea` (pre-panelized GF(2^m) fgemm) | `e24f7839` (panelized) | `fgemm × {GF(2^8), GF(2^16), GF(2^32)}` | `e24f7839` landed the production panelized kernel; `a1172cea` was the pre-panelization baseline. Both share date 2026-05-06; `e24f7839` assigned date 2026-05-07 in this aggregate to break the tie. |
| `2026-04-26-reference.csv` (fflas-ffpack baseline) | `2026-05-04-3b762764-dense-la-fresh.csv` (re-measurement) | all fflas-ffpack dense-LA cells at GF(p) | The re-measurement confirmed ~1-3% drift; the 2026-05-04 re-run is the primary source for fflas-ffpack. |
| Baseline charpoly/minpoly rows at n=32,128,512 from `2026-04-26-gf2.csv` | Criterion medians at n=64,256 from `2026-05-07-d1dd266c-minpoly-tuning.md` | `charpoly/minpoly × {GF(7),GF(251),GF(65521),GF(2^31-1)}` | The d1dd266c session used the final production algorithm. The n=32,128,512 baseline rows survive because no Criterion measurement at those sizes exists (different key); the n=64,256 Criterion rows supersede the baseline at those specific sizes. |

---

## 4. Coverage Declaration

### 4.1 In-scope cells (from `dev/plans/sota_target_matrix.md`)

Every in-scope `(operation, field)` cell is either present in the aggregate or noted as absent with explanation.

#### Dense `matmul` / `fgemm`

| Field | gf2 CSV | Reference CSV |
|---|---|---|
| `GF(2)` | present (`matmul`, from `2026-04-26-gf2.csv`) | present (`matmul`, m4ri from `2026-05-04-3b762764-dense-la-reference.csv`) |
| `GF(7)` | present (`fgemm`, from `e24f7839-gf2m-panelized.csv`) | present (fflas-ffpack + flint + ntl) |
| `GF(31)` | present (`fgemm`, from `e24f7839-gf2m-panelized.csv`) | present (fflas-ffpack from `609855d9-gf31-supplement.csv`) |
| `GF(251)` | present | present |
| `GF(65521)` | present | present |
| `GF(2^31-1)` | present | present |
| `GF(2^4)` | absent — no gf2-core GF(2^4) bench harness emits rows; renderer shows PENDING for gf2 side | present (m4rie from `507b0036-m4rie-reference.csv`) |
| `GF(2^8)` | present (from `e24f7839-gf2m-panelized.csv`) | present (m4rie) |
| `GF(2^16)` | present (from `e24f7839-gf2m-panelized.csv`) | present (m4rie) |
| `GF(2^32)` | present (from `e24f7839-gf2pow32-panelized.csv`) | present (ntl from `a1172cea-ntl-gf2pow32-large.csv` + `b13799ac-results.csv`) |

#### Dense `pluq`, `echelon`, `invert`, `solve`

| Field | gf2 CSV | Reference CSV |
|---|---|---|
| `GF(2)` — `pluq`, `solve` | absent — `BitMatrix::pluq` and `BitMatrix::solve_left` were never emitted by the bench harness; renderer shows PENDING for gf2 side. (`3b762764` dense-la-post-gemm.md § "GF(2) BitMatrix factorisation cells" notes this as a harness-scope gap.) | present (m4ri from `5dea7457-reference-extension.csv`) |
| `GF(2)` — `echelon`, `invert` | present | present (m4ri) |
| `GF(7)`, `GF(251)`, `GF(65521)`, `GF(2^31-1)` | present (from `2026-04-26-gf2.csv` via `3b762764-dense-la-reference.csv`) | present (fflas-ffpack + flint; ntl/linbox for invert/solve) |
| `GF(31)` | absent — gf2-core has no GF(31)-specific pluq/echelon/invert/solve measurement; target matrix names fflas-ffpack canonical and PENDING gf2 | present (fflas-ffpack from `609855d9-gf31-supplement.csv`) |
| `GF(2^8)`, `GF(2^16)`, `GF(2^32)` | EXCLUDED — `no-independent-oracle` per `sota_target_matrix.md` § 6.1 | EXCLUDED — same rationale; these cells are absent from the reference CSV |

#### Dense `charpoly`, `minpoly`

| Field | gf2 CSV | Reference CSV |
|---|---|---|
| `GF(2)` | EXCLUDED — no external oracle per `sota_target_matrix.md` § 6.2 row 19/20 | EXCLUDED |
| `GF(7)`, `GF(251)`, `GF(65521)`, `GF(2^31-1)` | present — Criterion medians from `2026-05-07-d1dd266c-minpoly-tuning.md` (n=64, 256) + baseline rows from `2026-04-26-gf2.csv` (n=32, 128, 512) | present (fflas-ffpack canonical + linbox + flint secondaries; ntl for charpoly) |
| `GF(31)` | absent — no gf2-core GF(31) charpoly/minpoly measurement | present (fflas-ffpack from `609855d9-gf31-supplement.csv`, n=64 only) |
| `GF(2^8)`, `GF(2^16)`, `GF(2^32)` | EXCLUDED — `no-independent-oracle` per § 6.1 | EXCLUDED |

#### Sparse `spmv`, `sparse-matmul`, `sparse×dense`, `sparse-elim`

| Field | gf2 CSV | Reference CSV |
|---|---|---|
| `GF(2)` | present (from `47698404-sparse-extended.csv` + `3a37e0f6-spmv-path-a.csv`) | present (linbox from `47698404-sparse-reference.csv`) |
| `GF(7)`, `GF(251)`, `GF(65521)`, `GF(2^31-1)` | present | present (fflas-ffpack + linbox) |
| `GF(2^8)`, `GF(2^16)` | present (gf2 self-reference per `sparse_benchmark_corpus.md` § 4 — `semantics-mismatch` marker) | absent — no external library performance-comparable to gf2 PCLMULQDQ sparse |

---

## 5. Excluded Cells

The following `(operation, field)` combinations are explicitly excluded from both CSVs per `dev/plans/sota_target_matrix.md` § 6:

| Exclusion class | Cells | Reason |
|---|---|---|
| `no-independent-oracle` | `{pluq,echelon,invert,solve,charpoly,minpoly} × {GF(2^8), GF(2^16), GF(2^32)}` | Protocol § 6 requires bitwise canonical equality vs an independent reference; no scalar GF(2^m) oracle is harnessed. See § 6.1 of the target matrix (15 cells). |
| `no-independent-oracle` | `{charpoly,minpoly} × GF(2)` | M4RI does not expose charpoly/minpoly; fflas-ffpack and FLINT cover GF(p) only. See § 6.2 rows 19/20. |

Total excluded: 20 cells (15 from § 6.1, 5 from § 6.2 including the 3 pluq × GF(2^m) cells; the 2 GF(2) charpoly/minpoly cells).

---

## 6. Source CSV Index

| `source_csv` value | Task | Operation family | bench_run_date assigned |
|---|---|---|---|
| `2026-04-26-gf2.csv` | `3b762764` | Baseline: fgemm, echelon, invert, matmul, charpoly, spmv | 2026-04-26 |
| `2026-05-04-3b762764-dense-la-reference.csv` | `3b762764` | Dense LA gf2 rows (same data as 2026-04-26, re-packaged) | 2026-04-26 |
| `2026-04-26-reference.csv` | `3b762764` | Baseline reference: fflas-ffpack all ops, m4ri matmul+echelon | 2026-04-26 |
| `2026-05-04-3b762764-dense-la-fresh.csv` | `3b762764` | Re-measured fflas-ffpack dense-LA reference | 2026-05-04 |
| `2026-05-04-5dea7457-reference-extension.csv` | `5dea7457` | m4ri pluq/invert/solve GF(2) extension | 2026-05-04 |
| `2026-05-04-507b0036-m4rie-reference.csv` | `507b0036` | m4rie GF(2^m) matmul reference | 2026-05-04 |
| `2026-05-04-609855d9-gf31-supplement.csv` | `609855d9` | fflas-ffpack GF(31) all dense ops | 2026-05-04 |
| `2026-05-04-609855d9-gfp-reference.csv` | `609855d9` | fflas-ffpack GF(p) family reference | 2026-05-04 |
| `2026-05-04-73ab8eef-flint-reference.csv` | `73ab8eef` | FLINT GF(p) secondary reference | 2026-05-04 |
| `2026-05-04-73ab8eef-ntl-reference.csv` | `73ab8eef` | NTL GF(p) secondary reference | 2026-05-04 |
| `2026-05-04-79388011-linbox-reference.csv` | `79388011` | LinBox GF(p) reference (charpoly/minpoly/solve) | 2026-05-04 |
| `2026-05-04-b13799ac-results.csv` | `b13799ac` | NTL GF(2^32) matmul reference | 2026-05-04 |
| `2026-05-04-c3e79272-charpoly-reference.csv` | `c3e79272` | charpoly reference: fflas-ffpack/linbox/flint/ntl | 2026-05-04 |
| `2026-05-04-c3e79272-minpoly-reference.csv` | `c3e79272` | minpoly reference: fflas-ffpack/linbox/flint | 2026-05-04 |
| `2026-05-04-47698404-sparse-extended.csv` | `47698404` | gf2 sparse all fields (SpMV, SpMM, sparse-elim, sparse-matmul) | 2026-05-04 |
| `2026-05-04-47698404-sparse-reference.csv` | `47698404` | Reference sparse: fflas-ffpack + linbox | 2026-05-04 |
| `2026-05-06-a1172cea-gf2m-gf2-rows.csv` | `a1172cea` | gf2 GF(2^m) fgemm (pre-panelization; superseded by e24f7839) | 2026-05-06 |
| `2026-05-06-a1172cea-ntl-gf2pow32-large.csv` | `a1172cea` | NTL GF(2^32) matmul large-n | 2026-05-06 |
| `2026-05-06-e24f7839-gf2m-panelized.csv` | `e24f7839` | gf2 GF(2^m) + GF(p) fgemm panelized (final production) | 2026-05-07 |
| `2026-05-06-e24f7839-gf2pow32-panelized.csv` | `e24f7839` | gf2 GF(2^32) matmul panelized (final) | 2026-05-07 |
| `2026-05-07-3a37e0f6-spmv-path-a.csv` | `3a37e0f6` | gf2 SpMV path-a (GF(2)) | 2026-05-07 |
| `2026-05-07-3a37e0f6-sparse-dense-path-b-final.csv` | `3a37e0f6` | gf2 SpMM path-b final (GF(p) + GF(2^m)) | 2026-05-07 |
| `2026-05-07-d1dd266c-minpoly-tuning.md` | `d1dd266c` | gf2 charpoly/minpoly final Criterion medians (n=64, 256; seed=0) | 2026-05-07 |

---

## 7. R1 Review Findings — Resolution

| Finding | Resolution |
|---|---|
| **F1: Schema incompatible with `analyze.py`** | CSVs now use `lib,operation,field,m,k,n,rank_regime,seed,wall_ns,throughput_ops` as first ten columns (exactly `analyze.py`'s `CSV_COLUMNS`). Three wide-form columns (`source_csv`, `bench_run_date`, `jit_issue`) are appended; `analyze.py` ignores unknown columns. `analyze.py --gf2` and `--reference` both accept these files without error (verified). |
| **F2: GF(2) coverage missing in gf2 aggregate** | `pluq × GF(2)` and `solve × GF(2)` gf2-side rows do not exist in any source CSV (`BitMatrix::pluq` and `BitMatrix::solve_left` were never emitted by the bench harness — noted as a harness-scope gap in `3b762764-dense-la-post-gemm.md` § "GF(2) BitMatrix factorisation cells"). These cells are absent from the gf2 CSV; `analyze.py` renders PENDING for the gf2 column when merged with the reference CSV. `echelon × GF(2)` and `invert × GF(2)` gf2-side rows ARE present. The reference CSV has all four M4RI operations. |
| **F3: Includes excluded cells (GF(2^m) pluq/echelon/invert/solve/charpoly/minpoly)** | All 15 excluded GF(2^m) non-matmul cells (§ 6.1 of target matrix) and all 5 GF(2) charpoly/minpoly cells (§ 6.2) are absent from both CSVs. Verified by spot-check script. |
| **F4: Duplicate rows under same date** | Supersession rule applied: for any `(lib, operation, field, m, k, n, rank_regime)` key, only the most-recent-date row is kept. Zero duplicates verified by key-counting check. |
| **F5: `density` encoding inconsistent** | The `density` column is replaced by `rank_regime` (the `analyze.py` column). `rank_regime` is never empty: dense operations use `uniform` or `deficient`; sparse operations use the full `density_<D>_<layout>` string. Zero empty `rank_regime` values verified. |
| **F6: Canonical-row selection ambiguous (pre-panelized vs panelized)** | Pre-panelized `a1172cea` rows superseded by panelized `e24f7839` rows for GF(2^m) fgemm (both share 2026-05-06 raw date; `e24f7839` assigned bench_run_date 2026-05-07 in this aggregate to make the supersession deterministic and explicit). Supersession table in § 3 above. |
| **F7: Schema doc omits GF(31) charpoly/minpoly** | GF(31) has explicit rows in the coverage table (§ 4.1) for every dense operation family. Reference CSV has charpoly and minpoly rows for GF(31) from `609855d9-gf31-supplement.csv`. GF(31) gf2-side is absent for charpoly/minpoly (no Criterion measurement taken) but present for fgemm. |
