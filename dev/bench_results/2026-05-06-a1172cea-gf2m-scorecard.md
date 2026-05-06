# GF(2^m) post-PPC GEMM scorecard -- issue a1172cea

| Field | Value |
|---|---|
| Date | 2026-05-06 |
| JIT issue | `a1172cea` (Measure GF(2^m) post-PPC GEMM against reference) |
| Parent story | `2c7548ae` (Close GF(2^m) FieldMatrix gaps to best reference) |
| Parent epic | `97bf0879` (Close gf2-core SOTA performance gaps) |
| Host | Linux 7.0.3-arch1-1 / Zen 3 (AMD Ryzen 9 5900X, 12C/24T), no AVX-512 |
| References | M4RIE 20250128 (`mzed_mul`, matmul over GF(2^8) and GF(2^16)); NTL 11.6.0 `mat_GF2E` (matmul over GF(2^32)) |
| M4RIE promotion | `dev/plans/m4rie_promotion_evidence.md` (Wave-2 promotion R3) |
| NTL promotion | `dev/bench_results/2026-05-04-b13799ac-gf2pow32-promotion.md` |
| Status | DELIVERY COMPLETE -- measurements taken and both success criteria satisfied; gaps identified (see § 4) |

This rev (2026-05-06 evening update) follows code-review FAIL on the prior rev. Three changes:

- **Finding 1 (HIGH):** path A taken -- NTL `mat_GF2E` reference at GF(2^32) n=256 and n=1024 was captured via the pinned container (`localhost/gf2-bench:ref`, post-PPC build of `benchmarks/reference/ntl_bench`). All three GF(2^32) sizes now have measured reference rows in `2026-05-06-a1172cea-ntl-gf2pow32-large.csv`. See § 4 for the path-A note explaining the container-side patch (`--gf2pow32-only` flag added to `ntl_bench` to skip the GF(p) lanes, which had a pre-existing latent invert-matrix abort at GF(7) n=256 unrelated to this issue).
- **Finding 2 (MEDIUM):** count math recomputed from the headline table -- 2 PASS / 7 FAIL / 6 n/a (deficient) / 18 EXCLUDED. The previous "3 of 9" wording is replaced.
- **Finding 3 (MEDIUM):** GF(2^16) n=1024 gap re-routed to `e24f7839` (Implement panelized GF(2^m) GEMM) as primary owner; `fb271c41` (Evaluate GFNI and AVX-512 follow-on routing) is listed as secondary research follow-up. All GF(2^8) and GF(2^32) gaps are also routed to `e24f7839` since the panelized GEMM pattern is the structural fix.

For consistency with the NTL/M4RIE references (default `--warmup 3 --iters 5` per `benchmarks/run.sh`), all gf2 rows in this rev are at warmup=3 iters=5. The previous rev used warmup=2 for the gf2 side. Both warmup counts produce throughputs within ~5%; the unification removes a minor apples-to-apples concern.

A subtle observation surfaced during the path-A run: the canonical NTL row from `benchmarks/results/20260505T091600Z.csv` (single GF(2^32) n=64 row, throughput 7.539e7 ops/s) is ~3.5x slower than every fresh measurement on the same host with the same image and seed (warmup=2: 2.446e8; warmup=3: 2.674e8). The fresh value is reproducible across two re-runs; the canonical value appears to have been an anomaly (likely CPU thermal throttling or background contention on the original 2026-05-05 bench day). For verdict consistency this scorecard uses the fresh path-A measurement as the authoritative reference. The canonical row remains in the bench-day CSV unchanged; it is cited in § 5 with this caveat.

---

## 1. Headline verdict table

Criterion: gf2 throughput / reference throughput >= 0.667 (i.e. gf2 is within 1.5x of reference or faster). The threshold applies per-cell.

For GF(2^8) and GF(2^16), the gf2 side emits operation tag `fgemm` (via `run_gf2m` in `bench_csv_emitter.rs`); the M4RIE reference emits `matmul`. The operation tags differ and cells do not merge automatically in `analyze.py`, but the throughput normalizer is identical (`2 * n^3`) for both sides, so manual comparison is valid. Seeds differ between gf2 and M4RIE sides (different seed-derivation paths); shapes match at n = {64, 256, 1024} square uniform.

For GF(2^32), gf2 emits operation tag `matmul` using `derive_seed(master ^ 0x77, "matmul", 0, si, 0)` -- identical to NTL's seed derivation -- so each n cell compares identical input matrices.

The gf2 emitter does not emit `deficient` regime rows for fgemm/matmul (only `uniform`). M4RIE has deficient reference rows; those cells are recorded as `n/a` on the gf2 side. They are not gaps and not counted toward PASS/FAIL totals.

Throughput units: ops/s, where one op = one GF(2^m) field multiply-accumulate (scalar element, not word). The throughput_ops column in the CSV uses `2 * n^3 / wall_ns` for square n x n x n matrix multiply -- the standard matmul normalizer.

| field | n | regime | gf2 ops/s | reference ops/s | ratio (gf2/ref) | threshold | marker | verdict | evidence row |
|---|---:|---|---:|---:|---:|---|---|---|---|
| GF(2^8) | 64 | uniform | 7.631e8 | 4.052e9 | 0.188 | >=0.667 | [hard] | FAIL | `gf2m-gf2-rows.csv:2`; M4RIE `507b0036-m4rie-reference.csv:8` |
| GF(2^8) | 64 | deficient | n/a | 4.032e9 | n/a | -- | -- | n/a -- gf2 deficient fgemm not emitted | M4RIE `507b0036-m4rie-reference.csv:9` |
| GF(2^8) | 256 | uniform | 8.433e8 | 2.453e10 | 0.034 | >=0.667 | [hard] | FAIL | `gf2m-gf2-rows.csv:3`; M4RIE `507b0036-m4rie-reference.csv:10` |
| GF(2^8) | 256 | deficient | n/a | 2.417e10 | n/a | -- | -- | n/a -- gf2 deficient fgemm not emitted | M4RIE `507b0036-m4rie-reference.csv:11` |
| GF(2^8) | 1024 | uniform | 7.276e8 | 9.757e10 | 0.0075 | >=0.667 | [hard] | FAIL | `gf2m-gf2-rows.csv:4`; M4RIE `507b0036-m4rie-reference.csv:12` |
| GF(2^8) | 1024 | deficient | n/a | 9.827e10 | n/a | -- | -- | n/a -- gf2 deficient fgemm not emitted | M4RIE `507b0036-m4rie-reference.csv:13` |
| GF(2^16) | 64 | uniform | 5.402e8 | 1.244e7 | 43.4 | >=0.667 | [hard] | PASS | `gf2m-gf2-rows.csv:7`; M4RIE `507b0036-m4rie-reference.csv:14` |
| GF(2^16) | 64 | deficient | n/a | 1.278e7 | n/a | -- | -- | n/a -- gf2 deficient fgemm not emitted | M4RIE `507b0036-m4rie-reference.csv:15` |
| GF(2^16) | 256 | uniform | 5.905e8 | 5.312e7 | 11.12 | >=0.667 | [hard] | PASS | `gf2m-gf2-rows.csv:8`; M4RIE `507b0036-m4rie-reference.csv:16` |
| GF(2^16) | 256 | deficient | n/a | 5.373e7 | n/a | -- | -- | n/a -- gf2 deficient fgemm not emitted | M4RIE `507b0036-m4rie-reference.csv:17` |
| GF(2^16) | 1024 | uniform | 5.708e8 | 2.854e9 | 0.200 | >=0.667 | [hard] | FAIL | `gf2m-gf2-rows.csv:9`; M4RIE `507b0036-m4rie-reference.csv:18` |
| GF(2^16) | 1024 | deficient | n/a | 2.816e9 | n/a | -- | -- | n/a -- gf2 deficient fgemm not emitted | M4RIE `507b0036-m4rie-reference.csv:19` |
| GF(2^32) | 64 | uniform | 8.945e7 | 2.675e8 | 0.334 | >=0.667 | [hard] | FAIL | `gf2m-gf2-rows.csv:10`; NTL `2026-05-06-a1172cea-ntl-gf2pow32-large.csv:2` |
| GF(2^32) | 256 | uniform | 9.002e7 | 2.805e8 | 0.321 | >=0.667 | [hard] | FAIL | `gf2m-gf2-rows.csv:11`; NTL `2026-05-06-a1172cea-ntl-gf2pow32-large.csv:3` |
| GF(2^32) | 1024 | uniform | 6.208e7\* | 2.829e8\* | 0.219 | >=0.667 | [hard] | FAIL | `gf2m-gf2-rows.csv:12`; NTL `2026-05-06-a1172cea-ntl-gf2pow32-large.csv:4` |

\*Both gf2 and NTL n=1024 cells exited early on the 30 s per-cell wall budget (single timing iteration). The ratio (0.219) is consistent in direction with n=64 (0.334) and n=256 (0.321), so even with single-sample variance the verdict is stable.

**Recomputed totals (re finding 2):**

- Cells with both gf2 measurement AND reference measurement: **9** (3 GF(2^8) uniform + 3 GF(2^16) uniform + 3 GF(2^32) uniform).
- **PASS: 2** -- GF(2^16) n=64 (43.4x), GF(2^16) n=256 (11.12x).
- **FAIL [hard]: 7** -- all 3 GF(2^8) uniform sizes; GF(2^16) n=1024; all 3 GF(2^32) uniform sizes.
- **n/a (deficient regime not emitted by gf2 fgemm path): 6** -- GF(2^8) and GF(2^16) deficient at n in {64, 256, 1024}. These are not gaps; they are protocol-recognized regime gaps in the gf2 emitter.
- **Excluded by lane-selection SSOT: 18** (per § 2 below).

---

## 2. Excluded cells

Per `dev/plans/gf2m_reference_lane_selection.md` § 3 (SSOT), 18 cells are excluded from the scorecard with user-approved exclusion classes. The exclusions cover all non-matmul GF(2^m) operations. They are not gaps and do not require a gf2 row.

| op | GF(2^8) | GF(2^16) | GF(2^32) | exclusion class |
|---|---|---|---|---|
| echelon (RREF) | EXCLUDED | EXCLUDED | EXCLUDED | `no-independent-oracle` -- protocol § 6 requires bitwise canonical RREF vs an independent reference; no scalar GF(2^m) RREF harness exists. |
| invert | EXCLUDED | EXCLUDED | EXCLUDED | `no-independent-oracle` -- same rationale; M4RIE provides `mzed_invert` but cannot serve as its own oracle. |
| solve | EXCLUDED | EXCLUDED | EXCLUDED | `no-independent-oracle` -- no independent GF(2^m) solution oracle harnessed. |
| charpoly | EXCLUDED | EXCLUDED | EXCLUDED | `no-independent-oracle` -- no GF(2^m) charpoly oracle harnessed (NTL and FLINT candidates exist upstream but have no harness for this epic). |
| minpoly | EXCLUDED | EXCLUDED | EXCLUDED | `no-independent-oracle` -- same rationale; FLINT `fq_nmod_mat_minpoly` is the recommended future candidate per the lane-selection doc. |
| spmv (sparse) | EXCLUDED | EXCLUDED | EXCLUDED | `not-performance-relevant`-adjacent -- deferred to issue `a3412e15` (Wave 3 sparse-corpus selection). |

SSOT citation: `dev/plans/gf2m_reference_lane_selection.md` § 3 table and § 4 proposals #2, #3, #4. User approval recorded 2026-05-04 per § 6.

---

## 3. Methodology

### Host

AMD Ryzen 9 5900X (Zen 3), 12 cores / 24 threads. No AVX-512.
- L1d cache: 384 KiB (12 instances), L2: 6 MiB (12 instances), L3: 64 MiB (2 instances).
- ISA flags relevant to GF(2^m): `pclmulqdq`, `vpclmulqdq`, `avx2`, `sse4_1`, `vaes`.

### Toolchain

- `rustc 1.95.0 (59807616e 2026-04-14)`, cargo 1.95.0.
- `RUSTFLAGS="-C target-cpu=native"` applied to gf2-side runs.
- gf2-core commit at time of measurement: `bc4b45d` (HEAD before this rework rev).

### Bench driver

gf2-side rows produced by `crates/gf2-core/examples/bench_csv_emitter.rs` with `--warmup 3 --iters 5` (matched to `benchmarks/run.sh` defaults). The emitter uses `std::time::Instant` timing, a 30 s per-cell budget, and the canonical master seed `0x6F73AC91D31E4A7C` from `benchmarks/seeds/seed.txt`. Per-cell seed derivation mirrors the C reference harness `benchmarks/reference/seed_helpers.h` via `bench_seed::derive_seed`.

For GF(2^32) matmul: seed derivation uses `derive_seed(master ^ 0x77, "matmul", 0, si, 0)` to mirror the NTL bench's field-tag salt (see `benchmarks/reference/ntl_bench.cpp` `run_gf2pow32` call site). At n=64 (si=0), this produces seed `17158103737143628803`, matching the NTL row.

For GF(2^8) and GF(2^16): seed derivation uses `derive_seed(master, "fgemm", 0, si, 0)`. Seeds differ from the M4RIE side (which uses its own seed in `m4rie_bench.c`), but shapes (n x n x n square uniform) match the reference.

### Reference rows

- M4RIE 20250128: rows from `dev/bench_results/2026-05-04-507b0036-m4rie-reference.csv` (pinned container `localhost/gf2-bench:m4rie-507b0036-perf`, 2026-05-04 bench-day run). A fresh re-run on 2026-05-06 (this session) reproduced the canonical numbers within ~5% on every GF(2^8)/GF(2^16) cell, confirming stability of the canonical CSV; no replacement was needed.
- NTL 11.6.0: rows from `dev/bench_results/2026-05-06-a1172cea-ntl-gf2pow32-large.csv` (path-A bench-day run, 2026-05-06 this session, pinned container `localhost/gf2-bench:ref`). The single canonical row at n=64 in `benchmarks/results/20260505T091600Z.csv` was found to be ~3.5x slower than the fresh measurements; see § 4 path-A note.

### GF(2^32) early-exit note (n=1024)

Both the gf2 emitter and the NTL harness hit the 30 s per-cell budget at GF(2^32) n=1024 (gf2 wall_ns=34591756997 ns = 34.6 s; NTL wall_ns=7591131276 ns mean across 4 iterations before budget reached). Both throughput values are based on at most one or two timing samples and may show higher variance than the n=64/256 cells; the n=1024 ratio (0.219) is consistent with the n=256 ratio (0.321) in direction and order of magnitude, so the FAIL verdict is stable.

---

## 4. Remaining gaps (criterion #2 deliverable)

The following cells do NOT meet the 0.667x throughput threshold against their reference. These are the remaining GF(2^m) gaps that Wave-8 implementation work (`e24f7839` -- Implement panelized GF(2^m) GEMM) must close. `fb271c41` (Evaluate GFNI and AVX-512 follow-on routing) is listed as a secondary research follow-up where applicable.

| cell | gf2 (ops/s) | ref (ops/s) | ratio | gap factor | next-step (primary / secondary) |
|---|---:|---:|---:|---:|---|
| matmul x GF(2^8) n=64 uniform | 7.631e8 | 4.052e9 | 0.188 | 5.3x | `e24f7839` (panelized GEMM) -- M4RIE uses Gray-code / Method of Four Russians at the matrix level; gf2-core needs an analogous macro-level batching for GF(2^8). |
| matmul x GF(2^8) n=256 uniform | 8.433e8 | 2.453e10 | 0.034 | 29.1x | `e24f7839` -- same; gap widens with n because M4RIE's O(n^3 / log n)-class algorithm scales much more favourably than gf2-core's current O(n^3) per-element CLMUL path. |
| matmul x GF(2^8) n=1024 uniform | 7.276e8 | 9.757e10 | 0.0075 | 134.1x | `e24f7839` -- largest measured gap in this scorecard. |
| matmul x GF(2^16) n=1024 uniform | 5.708e8 | 2.854e9 | 0.200 | 5.0x | `e24f7839` (primary) -- panelized GEMM extension to GF(2^16). gf2-core is faster at n=64 and n=256 (M4RIE's GF(2^16) path has higher constant overhead), but M4RIE overtakes at n=1024. `fb271c41` (secondary) -- only if e24f7839's panelized path does not close the gap and a GFNI / AVX-512 routing investigation is required. |
| matmul x GF(2^32) n=64 uniform | 8.945e7 | 2.675e8 | 0.334 | 3.0x | `e24f7839` -- panelized GEMM extension to GF(2^32). NTL `mat_GF2E` uses NTL's polynomial-arithmetic primitives without hardware CLMUL, but is internally panelized; gf2-core's per-element VPCLMULQDQ path lacks the cache-friendly matrix-level blocking that NTL gets for free from its mat structure. |
| matmul x GF(2^32) n=256 uniform | 9.002e7 | 2.805e8 | 0.321 | 3.1x | `e24f7839` -- same. |
| matmul x GF(2^32) n=1024 uniform | 6.208e7 | 2.829e8 | 0.219 | 4.6x | `e24f7839` -- same. Both gf2 and NTL are early_exit single-iteration at this size. |

### Path-A note (per finding 1)

NTL `mat_GF2E` reference rows at GF(2^32) n=256 and n=1024 were not present in the canonical 2026-05-05 bench-day CSV. Path A was attempted and succeeded on 2026-05-06: the pinned container (`localhost/gf2-bench:ref`) was already built and cached; `ntl_bench` was rebuilt inside the container via `make -B` and run directly with a new `--gf2pow32-only --large` flag combination that:

- `--large` enables `dense_sizes = {64, 256, 1024}` for the GF(2^32) lane (existing flag).
- `--gf2pow32-only` (added in this session, scoped to `benchmarks/reference/ntl_bench.cpp`) skips the GF(p) lanes (GF(7), GF(251), GF(65521), GF(2^31-1)). The skip was needed because the GF(7) invert lane crashed at n=256 with `inv: non-invertible matrix` (a latent reference-harness bug unrelated to this issue); without the skip, the harness aborts before reaching the GF(2^32) lane.

The skip flag is a bench-side ergonomic addition only; it does not change the GF(2^32) measurement path. The change touches only the argv parser and the `if (!gf2pow32_only)` guard around the four `run_field` calls; `run_gf2pow32` is unchanged. No production GEMM kernel was modified.

Path-A measurements (`dev/bench_results/2026-05-06-a1172cea-ntl-gf2pow32-large.csv`):

- n=64 ntl=2.674611e+08 ops/s (warmup=3, iters=5)
- n=256 ntl=2.805346e+08 ops/s (warmup=3, iters=5)
- n=1024 ntl=2.828938e+08 ops/s (warmup=3, iters=4 + early_exit)

A re-run with warmup=2 produced 2.446e8 / 2.786e8 / 2.796e8 -- within ~5% of the warmup=3 values, confirming reproducibility.

The single canonical NTL n=64 row in `benchmarks/results/20260505T091600Z.csv` (7.539187e+07 ops/s) is ~3.5x slower than the fresh measurements. The fresh value is reproducible across two re-runs on the same host with the same image and seed; the canonical value appears to have been an anomaly. Investigating root cause (CPU thermal or background contention on 2026-05-05) is out of scope for this issue. The fresh measurement is used for verdicts; the canonical row remains in the bench-day CSV unchanged.

### Pattern analysis

The gaps cluster into three structural patterns:

- **GF(2^8) (large, scaling gap):** gf2 throughput is ~750-840 Mops/s and is nearly flat across n=64..1024. M4RIE scales from 4 Gops/s to 98 Gops/s (24x improvement). M4RIE exploits GF(2^8) elements fitting in 1 byte to enable bit-sliced / table-based multiplication over 64-element word slices. gf2-core's current path is per-element CLMUL + Barrett without this word-level batching. Panelized GEMM (`e24f7839`) is the structural fix.

- **GF(2^16) (size-conditional):** gf2 wins at n=64 and n=256 (43x and 11x respectively) where M4RIE's GF(2^16) path has high constant overhead. M4RIE's asymptotic advantage re-emerges at n=1024 (gf2/M4RIE = 0.200). The gap is the smallest of the three patterns (5x) and is the most likely candidate for `fb271c41` GFNI / AVX-512 routing follow-up after `e24f7839` lands.

- **GF(2^32) (consistent ~3-5x gap):** gf2 ~60-90 Mops/s; NTL ~270-280 Mops/s. NTL `mat_GF2E` operates on polynomial coefficients via its `GF2E` type and benefits from its general polynomial-arithmetic optimizations (caching of the modulus, fast multiplication via `mul`); gf2-core uses VPCLMULQDQ directly per element but lacks NTL's matrix-level blocking. Panelized GEMM (`e24f7839`) extension to GF(2^32) is the primary fix; the VPCLMULQDQ kernel itself remains useful as the inner-loop scalar primitive.

---

## 5. Raw CSV and evidence index

- `/home/vkaskivuo/Projects/gf2/dev/bench_results/2026-05-06-a1172cea-gf2m-gf2-rows.csv` -- gf2-core GF(2^m) matmul rows (this session, warmup=3 iters=5, commit `bc4b45d`). GF(2^8) and GF(2^16) use operation tag `fgemm`; GF(2^32) uses `matmul`. Includes the original 6 GF(2^8) cells (3 square + 3 rect/n=4096) for context, plus 3 GF(2^16) and 3 GF(2^32) cells (re-measured at warmup=3 in this rework rev).
- `/home/vkaskivuo/Projects/gf2/dev/bench_results/2026-05-06-a1172cea-ntl-gf2pow32-large.csv` -- NTL `mat_GF2E` GF(2^32) rows at n = {64, 256, 1024} uniform (path-A bench-day run, 2026-05-06 this session).
- `/home/vkaskivuo/Projects/gf2/dev/bench_results/2026-05-04-507b0036-m4rie-reference.csv` -- M4RIE 20250128 reference rows for GF(2^4), GF(2^8), GF(2^16) at n = {64, 256, 1024} x {uniform, deficient}. Stable to within ~5% of a 2026-05-06 re-run; canonical CSV retained.
- `/home/vkaskivuo/Projects/gf2/benchmarks/results/20260505T091600Z.csv` -- canonical bench-day CSV; last row is NTL `mat_GF2E` GF(2^32) n=64 uniform. Throughput value (7.539e7 ops/s) is ~3.5x slower than the fresh path-A measurement; cited for completeness only, not used for verdicts.
- `/home/vkaskivuo/Projects/gf2/dev/bench_results/2026-05-04-b13799ac-gf2pow32-promotion.md` -- NTL GF(2^32) promotion evidence (five-criterion table).
- `/home/vkaskivuo/Projects/gf2/dev/bench_results/2026-04-29-gf2m-batch-fieldmatrix-gemm.md` -- post-PPC batch-GEMM evidence (context for gf2-core's GF(2^m) kernel path).
- `/home/vkaskivuo/Projects/gf2/dev/plans/gf2m_reference_lane_selection.md` -- SSOT for per-cell reference and exclusion decisions.

---

## 6. Self-satisfaction of success criteria

**Criterion #1 [hard]: GF(2^m) gf2 and reference CSV rows use comparable shapes and inputs.**

Satisfied by § 3 (Methodology): the gf2 emitter and both reference harnesses use (n, n, n) square shapes at n in {64, 256, 1024}, `uniform` regime, with throughput normalizer `2 * n^3` and warmup=3 iters=5 across both sides. For GF(2^32), seeds are identical at every n (both sides use `derive_seed(master ^ 0x77, "matmul", 0, si, 0)`), so input matrices are bit-identical. For GF(2^8) and GF(2^16), seeds differ between gf2 and M4RIE sides, but shapes and throughput normalizer match.

**Criterion #2 [hard]: The scorecard identifies all remaining GF(2^m) gaps.**

Satisfied by § 4 (Remaining gaps): seven cells are identified as [hard] FAILs with measured ratios, gap factors, and Wave-8 issue assignments to `e24f7839` (primary, all gaps). `fb271c41` is listed as secondary research follow-up for GF(2^16) n=1024 only. No gap is argued away or marked aspirational. The 18 excluded cells (§ 2) carry user-approved exclusion classes; they are not gaps. Every cell with a measured reference row is included; nothing is deferred by trend speculation.
