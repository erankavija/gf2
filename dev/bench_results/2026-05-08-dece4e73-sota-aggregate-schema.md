# SOTA Aggregate CSV Schema (`jit:dece4e73`)

| Field | Value |
|---|---|
| Date | 2026-05-08 |
| JIT issue | `dece4e73` (Aggregate final SOTA raw CSVs) |
| Parent story | `story:sota-final-scorecard` |
| Parent epic | `97bf0879` (gf2-core SOTA performance) |
| Status | FINAL — covers all in-scope cells across closed sibling tasks |

---

## 1. Artefacts

| File | Rows (excl. header) | Contents |
|---|---:|---|
| `dev/bench_results/2026-05-08-dece4e73-sota-aggregate-gf2.csv` | 459 | gf2-core measurements (this file's `source_csv` column names the per-task raw file) |
| `dev/bench_results/2026-05-08-dece4e73-sota-aggregate-reference.csv` | 644 | Reference-library measurements (fflas-ffpack, m4ri, m4rie, LinBox, FLINT, NTL) |
| `dev/bench_results/2026-05-08-dece4e73-sota-aggregate-schema.md` | — | This document |

Both CSVs share the identical column schema described in § 2.

---

## 2. Column Schema

| Column | Type | Unit | Nullable | Example | Notes |
|---|---|---|---|---|---|
| `operation` | string | — | no | `fgemm` | Benchmark operation. Values: `fgemm`, `pluq`, `echelon`, `invert`, `solve`, `det`, `charpoly`, `minpoly`, `spmv`, `sparse-elim`, `sparse×dense`, `sparse-matmul`, `matmul` |
| `field` | string | — | no | `GF(2^31-1)` | Finite field identifier. Values: `GF(2)`, `GF(7)`, `GF(31)`, `GF(251)`, `GF(65521)`, `GF(2^31-1)`, `GF(2^8)`, `GF(2^16)`, `GF(2^32)`, `GF(2^4)`, `Fp_M31` |
| `n` | integer | — | no | `256` | Matrix dimension. For square matrices this is the side length. For SpMV (`n=1` in source CSVs) this column stores the matrix row count (the vector size). |
| `density` | string | — | yes | `9.765625e-03` | For sparse operations: the actual fill fraction as a decimal string extracted from the `rank_regime` field of the source CSV. For dense operations with a uniform or deficient rank regime: the regime string (`uniform`, `deficient`). For structured/coding-theory matrices: the full `rank_regime` identifier string. Empty for dense operations where the rank regime is implicit in the operation. |
| `wall_time_ms` | float | milliseconds | no | `26.344000` | Median wall-clock time. For rows derived from raw `wall_ns` source columns: `wall_ns / 1_000_000`. For rows derived from Criterion evidence documents (operations `charpoly`, `minpoly`, `invert`, `solve`, `det` in `jit_issue` `d1dd266c` and `d1a5fea8`): the Criterion-reported median in milliseconds, converted verbatim. Precision: 6 decimal places. |
| `implementation` | string | — | no | `gf2` | Library identifier. Values in gf2 CSV: `gf2`. Values in reference CSV: `fflas-ffpack`, `m4ri`, `m4rie`, `linbox`, `flint`, `ntl`. |
| `host` | string | — | no | `AMD Ryzen 9 5900X (Zen 3), pinned container gf2-bench:ref` | Host + toolchain descriptor. Two canonical values used in this corpus: (1) `AMD Ryzen 9 5900X (Zen 3), pinned container gf2-bench:ref` — measurements from the pinned container bench day (2026-04-26 baseline and 2026-05-04 re-measurements inside the same container); (2) `AMD Ryzen 9 5900X (Zen 3), rustc 1.95.0 RUSTFLAGS=-C target-cpu=native` — task-session Criterion runs outside the container but on the same hardware. |
| `seed` | uint64 | — | no | `5180433273409205583` | RNG seed from the source bench harness. Matches the `seed` column in the source CSV exactly. Zero (`0`) for structured matrices that use no randomisation. |
| `source_csv` | string | — | no | `2026-04-26-gf2.csv` | Basename of the source CSV or evidence markdown file from which this row was transcribed. Cross-links to the per-task artefact. For rows derived from evidence markdown (Criterion medians): the `.md` filename. |
| `bench_run_date` | date | ISO-8601 | no | `2026-04-26` | Date on which the benchmark was run and committed. |
| `jit_issue` | string | — | no | `3b762764` | Short JIT issue ID of the task that produced this measurement. |

### 2.1 Density column encoding detail

The `density` column encodes the `rank_regime` from the source CSV as follows:

| Source `rank_regime` prefix | `density` value | Example |
|---|---|---|
| `density_<D>_csr` / `density_<D>_csc` / `density_<D>_block-csr` etc. | `<D>` (decimal string) | `9.765625e-03` |
| `uniform` | `uniform` | `uniform` |
| `deficient` | `deficient` | `deficient` |
| `structured_*` or `coding-theory_*` | full `rank_regime` string | `coding-theory_dvb-t2-normal-r2_3` |
| (absent / empty) | empty string | `` |

---

## 3. Coverage Declaration

The aggregate covers every in-scope cell enumerated by the SOTA acceptance protocol for epic `97bf0879`:

### 3.1 Dense FieldMatrix — pluq/echelon/invert/solve/det (factorization, inversion, solve)

Source tasks: `3b762764`, `d1a5fea8`, `7e41400f`, `73ec5da3`  
Source files: `2026-04-26-gf2.csv`, `2026-05-04-3b762764-dense-la-reference.csv`, `2026-05-07-d1a5fea8-invert-inplace.md`  
Fields covered: GF(7), GF(251), GF(65521), GF(2^31-1), GF(2^8), GF(2^16), GF(2)  
Sizes: n = 64, 256, 1024 (n=4096 for GF(2) matmul)  
Regimes: uniform and deficient  
Note: `invert`/`solve`/`det` measurements at GF(2^31-1) n={64,256,1024} are superseded by `d1a5fea8` Criterion medians (in-place LU-reuse driver). The baseline `2026-04-26-gf2.csv` rows for these cells remain in the aggregate with their original `source_csv` for traceability; the `d1a5fea8` rows carry `source_csv=2026-05-07-d1a5fea8-invert-inplace.md` and represent the final production measurements.

### 3.2 Dense FieldMatrix — fgemm (GEMM)

Source tasks: `e24f7839`, `662f7a15`, `3b762764`  
Source files: `2026-05-06-e24f7839-gf2m-panelized.csv`, `2026-05-06-e24f7839-gf2pow32-panelized.csv`, `2026-04-26-gf2.csv`  
Fields covered: GF(7), GF(31), GF(251), GF(65521), GF(2^31-1), GF(2^8), GF(2^16), GF(2^32)  
Sizes: n = 64, 256, 1024, 4096; also rectangular n=1024×1024×{8,32}

### 3.3 Sparse FieldMatrix — SpMV and SpMM

Source tasks: `47698404`, `3a37e0f6`  
Source files: `2026-05-04-47698404-sparse-extended.csv`, `2026-05-07-3a37e0f6-spmv-path-a.csv`, `2026-05-07-3a37e0f6-sparse-dense-path-b-final.csv`  
Operations: `spmv` (mat × vec), `sparse×dense` (SpMM), `sparse-elim`, `sparse-matmul`  
Fields: GF(2), GF(7), GF(251), GF(65521), GF(2^31-1), GF(2^8), GF(2^16)  
Sizes: n = 256, 1024; SpMV also includes structured and coding-theory matrices

### 3.4 Polynomial invariants — charpoly and minpoly

Source tasks: `d1dd266c`, `b87362a3`  
Source file: `2026-05-07-d1dd266c-minpoly-tuning.md` (Criterion medians, 16 cells)  
Baseline pre-task gf2 charpoly rows also present from `2026-04-26-gf2.csv` (sizes n=32, 128, 512)  
Fields: GF(7), GF(251), GF(65521), GF(2^31-1)  
Sizes: n = 64, 256 (final); n = 32, 128, 512 (baseline only)

### 3.5 Companion field-arithmetic cells (GF(2^m) gemm, GF(2^32))

Source tasks: `a1172cea`, `b13799ac`, `507b0036`  
Source files: `2026-05-06-a1172cea-gf2m-gf2-rows.csv`, `2026-05-06-e24f7839-gf2m-panelized.csv`, reference files

---

## 4. Renderer Compatibility Contract for `2cfc4372`

The next task `2cfc4372` ("Render final SOTA markdown scorecard") should consume the two CSV files directly. The contract:

1. **Join key**: `(operation, field, n, density)` — uniquely identifies a measurement cell when combined with `implementation`. To compare gf2 vs a reference, `JOIN ON (operation, field, n, density)` and filter `implementation`.

2. **Ratio computation**: `gf2.wall_time_ms / ref.wall_time_ms` gives the slowdown factor relative to the reference. Values < 1 mean gf2 is faster.

3. **Best-reference selection**: for cells with multiple reference implementations, the renderer should select the fastest reference (min `wall_time_ms`) for the canonical ratio, or display a table with all reference values.

4. **Duplicate rows**: some cells appear in both the baseline (`2026-04-26-gf2.csv`) and a later session CSV (e.g. `d1a5fea8` for invert). The renderer should use `MAX(bench_run_date)` per `(operation, field, n, density, implementation)` to select the most recent measurement as canonical, or explicitly select by `jit_issue`.

5. **Schema stability guarantee**: no columns will be removed or renamed between now and `2cfc4372`. Additional columns may be added (appended) in future tasks; renderers should use named-column access, not positional.

6. **Units**: `wall_time_ms` is always milliseconds. Convert to µs (`* 1000`) or ns (`* 1_000_000`) for display as needed.

7. **Throughput**: `throughput_ops` from source CSVs is NOT included in the aggregate (normalizer semantics differ by operation family). The renderer should derive throughput from `wall_time_ms` and `n` if needed, using `n³` for O(n³) operations and `n·nnz` for SpMV (see source CSV `throughput_ops` values for reference).

---

## 5. Source CSV Index

Every source CSV basename that appears in the `source_csv` column is listed below with its originating task:

| `source_csv` value | Task | Operation family |
|---|---|---|
| `2026-04-26-gf2.csv` | `3b762764` | Baseline: fgemm, pluq, echelon, invert, solve, charpoly, spmv |
| `2026-04-26-reference.csv` | `3b762764` | Baseline reference: fflas-ffpack all ops |
| `2026-05-04-3b762764-dense-la-reference.csv` | `3b762764` | Dense LA aggregation (gf2 + fflas-ffpack/m4ri) |
| `2026-05-04-5dea7457-reference-extension.csv` | `5dea7457` | Post-PPC reference re-run: fflas-ffpack |
| `2026-05-04-507b0036-m4rie-reference.csv` | `507b0036` | m4rie GF(2^m) matmul reference |
| `2026-05-04-609855d9-gf31-supplement.csv` | `609855d9` | fflas-ffpack GF(31) supplement |
| `2026-05-04-609855d9-gfp-reference.csv` | `609855d9` | fflas-ffpack GFp by-family reference |
| `2026-05-04-47698404-sparse-extended.csv` | `47698404` | gf2 sparse extended (all field families) |
| `2026-05-04-47698404-sparse-reference.csv` | `47698404` | Reference sparse: fflas-ffpack + LinBox |
| `2026-05-04-79388011-linbox-reference.csv` | `79388011` | LinBox charpoly/minpoly/solve reference |
| `2026-05-04-b13799ac-results.csv` | `b13799ac` | NTL GF(2^32) matmul reference |
| `2026-05-04-c3e79272-charpoly-reference.csv` | `c3e79272` | Charpoly reference: fflas-ffpack/LinBox/FLINT/NTL |
| `2026-05-04-c3e79272-minpoly-reference.csv` | `c3e79272` | Minpoly reference: fflas-ffpack/LinBox/FLINT |
| `2026-05-04-73ab8eef-flint-reference.csv` | `73ab8eef` | FLINT fgemm/charpoly/minpoly reference |
| `2026-05-04-73ab8eef-ntl-reference.csv` | `73ab8eef` | NTL fgemm/charpoly reference |
| `2026-05-06-a1172cea-gf2m-gf2-rows.csv` | `a1172cea` | gf2 GF(2^m) fgemm (pre-panelization) |
| `2026-05-06-a1172cea-ntl-gf2pow32-large.csv` | `a1172cea` | NTL GF(2^32) matmul large-n |
| `2026-05-06-e24f7839-gf2m-panelized.csv` | `e24f7839` | gf2 GF(2^m) fgemm panelized (final) |
| `2026-05-06-e24f7839-gf2pow32-panelized.csv` | `e24f7839` | gf2 GF(2^32) matmul panelized (final) |
| `2026-05-07-3a37e0f6-spmv-path-a.csv` | `3a37e0f6` | gf2 SpMV path-a |
| `2026-05-07-3a37e0f6-sparse-dense-path-b-final.csv` | `3a37e0f6` | gf2 SpMM path-b final |
| `2026-05-07-d1a5fea8-invert-inplace.md` | `d1a5fea8` | gf2 invert/solve/det post-trtrm (Criterion medians) |
| `2026-05-07-d1dd266c-minpoly-tuning.md` | `d1dd266c` | gf2 charpoly/minpoly final (Criterion medians) |
