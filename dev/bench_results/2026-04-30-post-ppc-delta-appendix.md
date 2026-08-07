# Post-PPC Benchmark Delta Appendix

| Field | Value |
|---|---|
| Date | 2026-04-30 |
| JIT issue | `b8189dbf` |
| Parent story | `b0434149` |
| Parent epic | `97bf0879` |

## Scope

This appendix is the post-PPC supplement to the published baseline at
[`2026-04-26.md`](2026-04-26.md). It composes deltas — pre-PPC vs.
post-PPC throughput per `(operation, field, shape)` cell, with ratios
against the original baseline and (where available) against the same
external reference (`fflas-ffpack` or `M4RI`) used in
[`2026-04-26.md`](2026-04-26.md).

Raw methodology — host description, container pins, harness commands,
Criterion configuration, perf-stat methodology, asm-extraction
procedure, and the seed scheme — is **not** repeated here. Each post-PPC
row links to its source-of-truth evidence document:

- [`2026-04-29-3abb755e-benchmark-gap-closure.md`](2026-04-29-3abb755e-benchmark-gap-closure.md)
  — story-level summary mapping each `64c88ae4` algorithmic gap to the
  PPC child that closed (or scoped) it.
- [`2026-04-29-strassen-matmul-crossover.md`](2026-04-29-strassen-matmul-crossover.md)
  — GF(2) `BitMatrix` square-matmul: the Strassen scaffold + post-Tier-A/B
  M4RM crossover sweep against the pinned baseline.
- [`2026-04-29-2598b981-fieldmatrix-gemm-fflas-sweep.md`](2026-04-29-2598b981-fieldmatrix-gemm-fflas-sweep.md)
  — GF(p) `FieldMatrix::gemm` reference-host sweep with delayed-reduction
  Mersenne / Solinas wins and the `fflas-ffpack` 256³/1024³ comparison.
- [`2026-04-29-gf2m-batch-fieldmatrix-gemm.md`](2026-04-29-gf2m-batch-fieldmatrix-gemm.md)
  — GF(2^m) batch-GEMM (`scalar_eager` vs. `batch_gemm` via `VPCLMULQDQ`).
- [`2026-04-26-uncompetitiveness-profile.md`](2026-04-26-uncompetitiveness-profile.md)
  — root-cause profile of the original `LogicalFns` dispatch overhead
  and missing `[profile.release]` block, motivating the PPC spiral.
- [`c7791a20/2026-04-26-profile-release-delta.md`](c7791a20/2026-04-26-profile-release-delta.md)
  — workspace-`[profile.release]` (thin LTO + `codegen-units = 1`)
  micro-delta on GF(2) matmul.
- [`2026-04-27-asm-audit.md`](2026-04-27-asm-audit.md)
  — asm-level audit of all five `gf2-kernels-simd::x86` modules at the
  point the PPC epic closed.
- [`2026-04-29-7c954fb5-criterion.txt`](2026-04-29-7c954fb5-criterion.txt)
  — criterion summary for the GF(2^m) wide-clmul kernel
  (`gf2m_wide9_m571`); 16.99× full mul+Barrett, 117.26× raw clmul.
- [`../active/babcf05e-gf2-core-ppc-spiral/babcf05e-handoff-5.md`](../active/babcf05e-gf2-core-ppc-spiral/babcf05e-handoff-5.md)
  — final session handoff from the PPC epic listing C1/B1/C3/B2 outcomes.

## Status legend

This appendix uses the same five-token cell-status vocabulary that
[`2026-04-26.md`](2026-04-26.md) is updated to publish under JIT
`02ace293`: **measured**, **N/A**, **slow-or-nightly**, **harness-scope
gap**, **optimization gap**. See the legend section in
[`2026-04-26.md`](2026-04-26.md) for the canonical definitions; this
document does not redefine them.

## Reading the tables

- The "pre" column is the gf2-side number from
  [`2026-04-26.md`](2026-04-26.md) / `dev/bench_results/2026-04-26-gf2.csv`
  — it is the number a reviewer would have seen at the original
  baseline publication.
- The "post-PPC (gf2-core)" column is the same operation × shape
  re-measured after the PPC epic closed. Throughput units match the
  "pre" column (Mops/s for GF(p) / GF(2^m), Gops/s for GF(2)).
- "delta ratio (post / pre)" is the gf2-side speedup the PPC epic
  delivered for that cell. >1× means faster.
- "reference" reproduces the same external-library number from
  [`2026-04-26.md`](2026-04-26.md) (M4RI for GF(2), fflas-ffpack for
  GF(p)) so the gf2-core / reference column is directly comparable
  with the pre-PPC ratio in the original report.
- "cell status" applies the legend to the post-PPC cell, not the
  pre-PPC cell.

Every row carries a footnote `[E#]` keyed to the **Sources** section
at the end. No row's delta or post-PPC number is sourced from
anywhere except those evidence documents.

## GF(2) deltas (`BitMatrix` square matmul)

Reference: `M4RI` (same column as in
[`2026-04-26.md`](2026-04-26.md) § `matmul × GF(2)`).

| operation | shape | pre (gf2-core) | post-PPC (gf2-core) | delta ratio (post / pre) | M4RI reference | gf2-core / M4RI (post) | cell status | source |
|---|---|---:|---:|---:|---:|---:|---|---|
| matmul | n=1024 (uniform) | 387.462 Gops/s | 1,283 Gops/s | 3.31× | 3,020.762 Gops/s | 0.42× | optimization gap | [E2] |
| matmul | n=4096 (uniform) | 1,092.904 Gops/s | 1,927 Gops/s | 1.76× | 6,272.592 Gops/s | 0.31× | optimization gap | [E2] |
| matmul | n=2048 (auto dispatch) | harness-scope gap (size not in [`2026-04-26.md`](2026-04-26.md)) | 1,622 Gops/s | n/a | not measured at n=2048 in [`2026-04-26.md`](2026-04-26.md) | n/a | measured | [E2] |
| matmul | n=8192 (auto dispatch) | harness-scope gap (size not in [`2026-04-26.md`](2026-04-26.md)) | 2,134 Gops/s | n/a | not measured at n=8192 in [`2026-04-26.md`](2026-04-26.md) | n/a | measured | [E2] |
| transpose | n=4096 | harness-scope gap (no transpose row in [`2026-04-26.md`](2026-04-26.md)) | ≥10× criterion-gate met (worker-reported ~65× over `ppc-v0-2026-04-27`) | ≥65× over the PPC-baseline transpose, no entry in pre-PPC row | N/A (no M4RI transpose comparison populated) | n/a | measured | [E8] |

Notes:

- `[E2]` records that the safe-Rust Strassen-family scaffold landed but
  did **not** beat M4RM through n=8192 after the view/scratch rework;
  production dispatch therefore stays on M4RM at every measured n.
  The gf2-side delta for the n=1024 / n=4096 cells is therefore
  attributable to the M4RM-leaf improvements (workspace `[profile.release]`
  thin LTO + `codegen-units = 1` per [`c7791a20/2026-04-26-profile-release-delta.md`](c7791a20/2026-04-26-profile-release-delta.md)
  plus the upstream Tier-A/B kernel work landed under `babcf05e`),
  not to a Strassen layer.
- `M4RI reference` is the throughput value already published in
  [`2026-04-26.md`](2026-04-26.md) § `matmul × GF(2)`; it has not
  been re-measured for this appendix (no PPC change touches the
  M4RI side).
- `transpose` was not on the original `64c88ae4` operation matrix.
  The PPC `B1` task introduced the operation and its bench together,
  so there is no pre-PPC gf2-side number to ratio against. The
  speedup is reported against the PPC tier-0 transpose baseline
  (`ppc-v0-2026-04-27`), per `[E8]`.
- The `optimization gap` classification on the n=1024 / n=4096 rows
  reflects the parent story: gf2-core is now within ~3× / ~3.2× of
  M4RI on the headline cells (vs. ~8× / ~6× pre-PPC), but the 1.5×
  epic target (`97bf0879`) is not yet met, and there is no follow-up
  child currently in scope to close it.

## GF(p) deltas (`FieldMatrix::gemm` square)

Reference: `fflas-ffpack` (same column as in
[`2026-04-26.md`](2026-04-26.md) § `fgemm × GF(p)`). The post-PPC
GF(p) speedups all come from the `e7ab802d` delayed-reduction
landing — the post-PPC throughput is shape-stable across small and
large primes because the inner-loop is a Goldilocks / Mersenne /
Solinas modular trick, not a per-prime kernel.

Throughput in Gops/s (post-PPC numbers from `[E3]` are reported as
Gop/s = `2·m·k·n / wall`, matching the conventional GEMM convention
used in [`2026-04-26.md`](2026-04-26.md)).

| operation | shape | field | pre (gf2-core) | post-PPC (gf2-core) | delta ratio (post / pre) | fflas-ffpack reference (pinned [`2026-04-26.md`](2026-04-26.md)) | gf2-core / fflas (post) | cell status | source |
|---|---|---|---:|---:|---:|---:|---:|---|---|
| fgemm | 256³ | GF(7) | 0.528 Gops/s | 3.708 Gops/s | 7.02× | 50.752 Gops/s | 0.073× | optimization gap | [E3] |
| fgemm | 256³ | GF(251) | 0.593 Gops/s | 3.704 Gops/s | 6.25× | 128.480 Gops/s | 0.029× | optimization gap | [E3] |
| fgemm | 256³ | GF(65521) | 0.590 Gops/s | 3.695 Gops/s | 6.27× | 31.615 Gops/s | 0.117× | optimization gap | [E3] |
| fgemm | 256³ | GF(2^31-1) | 0.595 Gops/s | 3.696 Gops/s | 6.21× | 2.126 Gops/s | 1.739× | measured | [E3] |
| fgemm | 1024³ | GF(7) | 0.527 Gops/s | 3.838 Gops/s | 7.29× | 96.233 Gops/s | 0.040× | optimization gap | [E3] |
| fgemm | 1024³ | GF(251) | 0.590 Gops/s | 3.838 Gops/s | 6.50× | 138.317 Gops/s | 0.028× | optimization gap | [E3] |
| fgemm | 1024³ | GF(65521) | 0.557 Gops/s | 3.842 Gops/s | 6.89× | 43.381 Gops/s | 0.089× | optimization gap | [E3] |
| fgemm | 1024³ | GF(2^31-1) | 0.590 Gops/s | 3.822 Gops/s | 6.48× | 2.341 Gops/s | 1.633× | measured | [E3] |
| fgemm | 1024×1024×8 | GF(2^31-1) | 0.644 Gops/s | 3.860 Gops/s | 6.00× | harness-scope gap (rectangular not covered by `fflas_bench.cpp`) | n/a | measured | [E3] |
| fgemm | 1024×1024×32 | GF(2^31-1) | 0.627 Gops/s | 3.867 Gops/s | 6.17× | harness-scope gap (rectangular not covered by `fflas_bench.cpp`) | n/a | measured | [E3] |
| fgemm | 4096³ | GF(7) / GF(251) / GF(65521) / GF(2^31-1) | PENDING in [`2026-04-26.md`](2026-04-26.md) (deferred at baseline) | slow-or-nightly: not re-measured for this appendix | n/a | varies (see [`2026-04-26.md`](2026-04-26.md)) | n/a | slow-or-nightly | [E3] |

Notes:

- The "Mersenne post-PPC strong row" the issue brief calls out is
  `fgemm/GF(2^31-1)/256³` and `1024³`: post-PPC gf2-core is **faster**
  than the pinned-container fflas-ffpack (`1.74×` and `1.63×` of fflas
  respectively), and `[E3]` records this as the only field where the
  aspirational "within-10× of fflas at n ≥ 256" target is met across
  every measured cell at every measured n.
- The "small-prime row that still trails fflas" the issue brief calls
  out is `fgemm/GF(7)/256³` and `1024³` and `fgemm/GF(251)/256³`
  and `1024³`: even with the 6×–7× post-PPC speedup, gf2-core is at
  `0.028×–0.073×` of pinned fflas-ffpack — the small-prime fflas-ffpack
  path is roughly two orders of magnitude faster than the gf2-core
  Goldilocks-style path, because fflas-ffpack runs a double-precision
  modular trick that turns the inner loop into BLAS3. `[E3]` flags this
  as future work for `97bf0879` (no in-epic child currently scoped).
- The host fflas-ffpack run measured in `[E3]` is **not** the pinned
  container; ratios in this appendix use the **pinned** fflas-ffpack
  numbers from [`2026-04-26.md`](2026-04-26.md) for like-for-like
  comparison with the original baseline, even though `[E3]` also
  reports a host-fflas column.
- The `fgemm/4096³` row is `slow-or-nightly` per `[E3]`'s Deferred-cells
  section — Criterion would need ≥ 36 s per iteration on this host.

## GF(2^m) deltas (`FieldMatrix::gemm` and batch wide-clmul)

GF(2^m) had no `fflas-ffpack` reference at the baseline (the fflas
driver enumerates `Fp` only) and has no comparable hard reference yet.
The deltas below are therefore composed against:

- the pre-PPC gf2-side baseline at the original published shapes
  (square `n=64` from [`2026-04-26.md`](2026-04-26.md) § `fgemm × GF(2^m)`);
- the C1 batch-vs-loop baseline `pclmulqdq_barrett_loop_v0` from
  `[E5]` and the canonical handoff `[E8]`;
- where the bench harness shape differs (post-PPC tests use
  `64×64×64` / `128×8×128` / `128×32×128` because the full square
  `n=1024` cell is too expensive for a `scalar_eager` reference),
  the row is annotated and the size mismatch called out.

| operation | shape | field | pre (gf2-core) | post-PPC (gf2-core) | delta ratio (post / pre) | external reference | gf2-core / reference | cell status | source |
|---|---|---|---:|---:|---:|---:|---:|---|---|
| fgemm | n=64 | GF(2^8) | 36.455 Mops/s | 182.48 Mops/s (`64×64×64` `batch_gemm`) | 5.01× | no comparable hard reference yet (fflas does not enumerate GF(2^m)) | n/a | measured | [E4] |
| fgemm | n=64 | GF(2^16) | 32.548 Mops/s | 186.88 Mops/s (`64×64×64` `batch_gemm`) | 5.74× | no comparable hard reference yet | n/a | measured | [E4] |
| fgemm | `128×32×128` | GF(2^8) | harness-scope gap (no `128×32×128` cell at baseline) | 164.72 Mops/s | n/a | reference column: `scalar_eager` 12.326 Mops/s, batch / scalar = **13.36×** | n/a | measured | [E4] |
| fgemm | `128×32×128` | GF(2^16) | harness-scope gap (no `128×32×128` cell at baseline) | 297.05 Mops/s | n/a | reference column: `scalar_eager` 19.523 Mops/s, batch / scalar = **15.22×** | n/a | measured | [E4] |
| fgemm | `128×32×128` | GF(2^32) | harness-scope gap (`GF(2^32)` is one of the two field-coverages explicitly handed off from `64c88ae4` to `97bf0879`) | 275.80 Mops/s | n/a | reference column: `scalar_eager` 17.897 Mops/s, batch / scalar = **15.41×** | n/a | measured | [E4] |
| fgemm | n=1024 / n=256 / `1024×1024×{8,32}` | GF(2^8) / GF(2^16) | 36.075–36.429 Mops/s ([`2026-04-26.md`](2026-04-26.md) § `fgemm × GF(2^8)`) | slow-or-nightly: a `scalar_eager` reference at `n=1024` is too expensive for the fast agent lane; correctness covered by `test_gf2m_batch_gemm_covers_64c88ae4_rectangular_shapes` per `[E4]` | n/a | no comparable hard reference yet | n/a | slow-or-nightly | [E4] |
| C1 batch wide-clmul mul/square (geomean) | m ∈ {8, 16, 32} | GF(2^m) (kernel-level, not `FieldMatrix`) | `pclmulqdq_barrett_loop_v0` baseline (criterion-1.5x gate) | `gf2m_batch_unroll4` | **5.131× geomean** over `pclmulqdq_barrett_loop_v0` (criterion-1.5x gate) | reference column: `pclmulqdq_barrett_loop_v0` is the same kernel path before PPC | n/a | measured | [E8] |
| C2-style wide-clmul full mul+Barrett | `gf2m_wide9_m571` | GF(2^571) | 2.7775 µs (scalar) | 163.52 ns (`avx2+vpclmulqdq-ymm`) | **16.99×** | reference column: `gf2m_wide9_m571_scalar_clmul_barrett` is the pre-dispatch path | n/a | measured | [E7] |
| C2-style raw clmul only | `gf2m_wide9_m571` | GF(2^571) | 2.6191 µs (scalar) | 22.335 ns (`avx2+vpclmulqdq-ymm`) | **117.26×** | reference column: `gf2m_wide9_m571_scalar_clmul_only` is the pre-dispatch path | n/a | measured | [E7] |

Notes:

- The first two rows directly reproduce the issue brief's request to
  cite "the C1 batch geomean" against `pclmulqdq_barrett_loop_v0` —
  see `[E8]`'s amendment: "the criterion gate compares
  `gf2m_batch_unroll4` against `pclmulqdq_barrett_loop_v0` over
  m = 8,16,32, geomean 5.131x." That is the canonical post-PPC
  number for the batch kernel and matches the `577b9e7f` evidence
  in `[E4]` (5×–17× per individual cell).
- "no comparable hard reference yet" is taken verbatim from the
  issue brief's instruction for the GF(2^m) reference column. M4RIE
  is **not** cited because no PPC evidence document used it.
- The `n=1024` / rectangular-`{8,32}` cells from [`2026-04-26.md`](2026-04-26.md)
  § `fgemm × GF(2^m)` are classified `slow-or-nightly` rather than
  invented: `[E4]` explicitly states "they are not used for the
  fast-budget scalar-eager benchmark table because a dense scalar
  triple-loop reference at those shapes is too expensive for the
  normal agent lane." The production path through the same shapes
  is covered by a release-mode correctness test
  (`test_gf2m_batch_gemm_covers_64c88ae4_rectangular_shapes`) in
  the same evidence document.
- `GF(2^32)` was explicitly named as a `64c88ae4`-to-`97bf0879`
  handoff field in [`2026-04-26.md`](2026-04-26.md) § Handoff. The
  `128×32×128 / GF(2^32)` row in `[E4]` is the first published
  post-PPC measurement on that field; the row is annotated as a
  PPC-only shape because no pre-PPC GF(2^32) gf2-side cell was
  measured.

## Sources

- `[E1]` [`2026-04-29-3abb755e-benchmark-gap-closure.md`](2026-04-29-3abb755e-benchmark-gap-closure.md)
  — story-level summary; cited for the structural mapping
  ("which child closes which gap") rather than for individual
  numbers.
- `[E2]` [`2026-04-29-strassen-matmul-crossover.md`](2026-04-29-strassen-matmul-crossover.md)
  — GF(2) `BitMatrix` square-matmul post-PPC sweep at n ∈ {1024,
  2048, 4096, 8192}; pinned-baseline ratio table at `n=1024` /
  `n=4096`; final no-Strassen-dispatch policy.
- `[E3]` [`2026-04-29-2598b981-fieldmatrix-gemm-fflas-sweep.md`](2026-04-29-2598b981-fieldmatrix-gemm-fflas-sweep.md)
  — GF(p) `FieldMatrix::gemm` post-`e7ab802d` sweep at 256³ and 1024³
  for GF(7) / GF(251) / GF(65521) / GF(2^31-1), plus rectangular
  `1024×1024×{8,32}` cells; comparison vs. host fflas and pinned-
  container fflas; explicit `4096³` deferral.
- `[E4]` [`2026-04-29-gf2m-batch-fieldmatrix-gemm.md`](2026-04-29-gf2m-batch-fieldmatrix-gemm.md)
  — GF(2^m) `FieldMatrix::gemm` post-PPC sweep at `64×64×64`,
  `128×8×128`, `128×32×128` for GF(2^8) / GF(2^16) / GF(2^32);
  ratio against `64c88ae4` published `n=64` square baseline.
- `[E5]` [`2026-04-26-uncompetitiveness-profile.md`](2026-04-26-uncompetitiveness-profile.md)
  — root-cause profile motivating the PPC spiral; cited
  for the `LogicalFns` / missing-`[profile.release]` analysis
  that the post-PPC numbers improve on.
- `[E6]` [`c7791a20/2026-04-26-profile-release-delta.md`](c7791a20/2026-04-26-profile-release-delta.md)
  — `[profile.release]` thin-LTO + `codegen-units = 1` micro-delta;
  cited as one component of the GF(2) post-PPC delta.
- `[E7]` [`2026-04-29-7c954fb5-criterion.txt`](2026-04-29-7c954fb5-criterion.txt)
  — kernel-level GF(2^571) clmul criterion summary; full-multiply
  16.99× and raw-clmul 117.26× scalar-vs-`avx2+vpclmulqdq-ymm`
  speedups.
- `[E8]` [`../active/babcf05e-gf2-core-ppc-spiral/babcf05e-handoff-5.md`](../active/babcf05e-gf2-core-ppc-spiral/babcf05e-handoff-5.md)
  — final PPC-epic session handoff; cited for the C1 batch geomean
  `5.131× over pclmulqdq_barrett_loop_v0`, the B1 transpose ≥10×
  criterion-gate result and worker-reported ~65× at n=4096, and the
  list of integrated PPC commits per task.
- The pre-PPC numbers in every row are sourced from the published
  baseline at [`2026-04-26.md`](2026-04-26.md) and the underlying
  CSV at `dev/bench_results/2026-04-26-gf2.csv`; the M4RI / fflas
  reference numbers are sourced from the same baseline's reference
  CSV at `dev/bench_results/2026-04-26-reference.csv`. Per the
  issue contract, this appendix does not edit
  [`2026-04-26.md`](2026-04-26.md) — companion task `02ace293`
  owns the cell-status legend cleanup there.
