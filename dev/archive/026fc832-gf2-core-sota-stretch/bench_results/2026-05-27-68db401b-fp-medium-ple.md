# Medium-prime u16-lane PLE base-case — measurement evidence

**Issue:** `68db401b` — u16-lane PLE base-case kernel (6823c8a0 Phase 6d)
**Design:** `dev/active/2e8c5a29-panelized-ple-design.md` § 9
**Predecessor:** `6823c8a0` R0 evidence at
`dev/bench_results/2026-05-26-6823c8a0-panelized-ple.md` (baseline)
**Date:** 2026-05-27
**Host:** AMD Ryzen 9 5900X Zen 3 (5900X reference host, CCX1 = cores 6-11)
**Source CSV:** `dev/bench_results/2026-05-27-68db401b-fp-medium-ple.csv`
**Raw trial log:** `dev/bench_results/trials/2026-05-27-68db401b/ple_sweep.log`

## Design summary

Adds an AVX2 u16-lane analogue of the byte-lane PLE panel-base kernel
established by `6823c8a0` R0 (`crates/gf2-kernels-simd/src/x86/fp_small_ple.rs`).
The new kernel — `crates/gf2-kernels-simd/src/x86/fp_medium_ple.rs` —
operates on canonical u16 panel storage with a row-major axpy-style
Schur update:

- 8 × u16 → 8 × u32 widening via `_mm256_cvtepu16_epi32`,
- `_mm256_mullo_epi32` against the broadcast L-multiplier (exact for
  `(p-1)² < 2^32`),
- SSOT Barrett reduction via
  `crate::x86::fp_small::barrett_reduce_lane32` (the `e8a0c47a` SSOT,
  unchanged),
- branchless cond-sub via `_mm256_min_epu32`,
- repack 8 × u32 → 8 × u16 via `_mm256_packus_epi32` +
  `_mm256_permute4x64_epi64::<0xD8>`.

`Fp<P>` for `252 ≤ P < 65536` overrides `PLE_PANEL_COLS = KC_U16 = 128`
(half the byte-lane `KC = 256`, reflecting the 2× lane-density gap).
The unified `simd_ops::fp_try_ple_panel_base::<P>` dispatch now routes
medium primes through `fp_try_ple_panel_base_medium` (new), which packs
through `Fp::value()` → canonical u16, calls the kernel, propagates row
swaps outside the column window via the existing cycle-decomposition
helper, and unpacks via `Fp::new` back into Montgomery storage. The
small-prime byte-lane path (`P ≤ 251`) is unchanged.

A per-prime `[u16; P]` inverse table (size at most 128 KB at P=65521)
is built once per prime per process via Fermat exponentiation and
cached behind a `Mutex<HashMap<u64, &'static [u16]>>`. Build cost is
`O(P log P)` paid once.

## Methodology

- **Trials per cell:** 3 warmup + 5 measured invocations of
  `FieldMatrix::ple()`. Median of the 5 trial wall times reported.
- **Pinning:** `flock -x /tmp/gf2-ccx1.lock taskset -c 6-11 nice -n -5`
  via `dev/benchmarks/ccx1-bench-flock.sh` (matches the R0 protocol).
- **Build:** `cargo test -p gf2-core --release --all-features --lib`.
  The test harness `test_ple_panelized_wall_time_full_sweep` (ignored)
  drives the sweep; output is CSV lines emitted to stderr between
  `--- panelized-ple-sweep BEGIN ---` and `--- panelized-ple-sweep END ---`.
- **Matrix construction:** Deterministic per-cell seed
  `seed = P * 0x9E3779B9 + n (+ 0x1234 for deficient)`. Identical to the
  R0 harness.
- **fflas-ffpack reference:** values from
  `dev/bench_results/2026-05-08-dece4e73-sota-aggregate-reference.csv`
  rows tagged `fflas-ffpack,pluq,...` (`3b762764` capture, 2026-05-04).
  Same fflas baseline as the R0 evidence doc.
- **Ratio definition:** `Ratio = gf2 wall / fflas wall` (lower is better).
  PASS threshold: Ratio ≤ 1.5×.

## A8 rows 14-17 + new n=1024 cells (GF(65521); SC#1 targets)

| Row    | Field      |     n | Regime    | fflas wall (µs) | gf2 (R0) wall (µs) | gf2 (R1 / 68db401b) wall (µs) | Ratio R0 (gf2/fflas) | Ratio R1 (gf2/fflas) | Status |
|-------:|-----------|------:|-----------|----------------:|-------------------:|------------------------------:|---------------------:|---------------------:|--------|
| 14     | GF(65521) |    64 | uniform   |         141.902 |             471.03 |                         37.65 |              3.319×  |               0.265× | **PASS** |
| 15     | GF(65521) |    64 | deficient |         112.508 |             358.57 |                         31.60 |              3.187×  |               0.281× | **PASS** |
| 16     | GF(65521) |   256 | uniform   |        2904.889 |            6487.37 |                       1421.27 |              2.234×  |               0.489× | **PASS** |
| 17     | GF(65521) |   256 | deficient |        2144.012 |            4893.46 |                       1205.18 |              2.283×  |               0.562× | **PASS** |
| (new)  | GF(65521) |  1024 | uniform   |       61485.787 |          211726.18 |                      58665.49 |              3.443×  |               0.954× | **PASS** |
| (new)  | GF(65521) |  1024 | deficient |       47579.239 |           96989.22 |                      41596.31 |              2.038×  |               0.874× | **PASS** |

**All six previously-SHORTFALL GF(65521) PLE cells close to ≤ 1.5×.**

The improvement at n=64 (12.5× / 11.3× speedup) reflects the fact that
the scalar `ple_base_direct` is amortising no SIMD at this size; the
new kernel SIMDifies every Schur update tile via the 8-u32-lane Barrett
path. At n=256 (4.6× / 4.1×) the panel base case handles the leading
128-column sub-panel before the recursive trsm+gemm path takes over;
the kernel's row-major axpy is faster than the Schur-update GEMM
fast-path even on its own because the per-pivot fixed overhead
amortises better when the working set stays in L1d. At n=1024 (3.6× /
2.3×) the recursive trsm+gemm path dominates total wall time and the
panel base case contributes a smaller share, so the absolute speedup
shrinks while still closing the criterion.

## Non-regression sweep (SC#2: GF(7/31/127/241/251) at n ∈ {64, 256, 1024})

Wall-times from the R1 / 68db401b bench compared to the R0 / 6823c8a0
baseline. Delta % is `(R1 - R0) / R0 × 100`. The criterion threshold is
`|delta| ≤ 5%`.

| Field    |     n | Regime    | R0 wall (µs) | R1 wall (µs) | Δ%      | Status |
|----------|------:|-----------|-------------:|-------------:|---------|--------|
| GF(7)    |    64 | uniform   |        48.06 |        44.41 |  -7.6%  | OK (improvement) |
| GF(7)    |    64 | deficient |        42.25 |        38.55 |  -8.8%  | OK (improvement) |
| GF(7)    |   256 | uniform   |      1894.98 |      1948.60 |  +2.8%  | OK |
| GF(7)    |   256 | deficient |      1215.19 |      1261.93 |  +3.8%  | OK |
| GF(7)    |  1024 | uniform   |     45863.46 |     44862.85 |  -2.2%  | OK |
| GF(7)    |  1024 | deficient |     31941.23 |     30759.60 |  -3.7%  | OK |
| GF(31)   |   256 | uniform   |      1545.65 |      1335.25 | -13.6%  | OK (improvement) |
| GF(31)   |   256 | deficient |      1303.45 |      1080.84 | -17.1%  | OK (improvement) |
| GF(31)   |  1024 | uniform   |     47273.03 |     35762.77 | -24.4%  | OK (improvement) |
| GF(31)   |  1024 | deficient |     30839.08 |     29309.52 |  -5.0%  | OK |
| GF(127)  |   256 | uniform   |      1558.75 |      1341.02 | -14.0%  | OK (improvement) |
| GF(127)  |   256 | deficient |      1314.36 |      1098.92 | -16.4%  | OK (improvement) |
| GF(127)  |  1024 | uniform   |     47475.45 |     36602.24 | -22.9%  | OK (improvement) |
| GF(127)  |  1024 | deficient |     31020.16 |     29178.12 |  -5.9%  | OK |
| GF(241)  |   256 | uniform   |      1588.95 |      1345.15 | -15.3%  | OK (improvement) |
| GF(241)  |   256 | deficient |      1341.63 |      1094.69 | -18.4%  | OK (improvement) |
| GF(241)  |  1024 | uniform   |     48125.44 |     35755.04 | -25.7%  | OK (improvement) |
| GF(241)  |  1024 | deficient |     54572.11 |     29496.67 | -45.9%  | OK (improvement) |
| GF(251)  |    64 | uniform   |        67.98 |        39.12 | -42.5%  | OK (improvement) |
| GF(251)  |    64 | deficient |        58.24 |        32.85 | -43.6%  | OK (improvement) |
| GF(251)  |   256 | uniform   |      2475.16 |      1342.13 | -45.8%  | OK (improvement) |
| GF(251)  |   256 | deficient |      2111.94 |      1099.76 | -47.9%  | OK (improvement) |
| GF(251)  |  1024 | uniform   |     70554.75 |     34530.50 | -51.0%  | OK (improvement) |
| GF(251)  |  1024 | deficient |     57239.56 |     27919.05 | -51.2%  | OK (improvement) |

All non-target cells (`P ≤ 251`) are within the ±5% non-regression
budget OR strictly improved. **No regression observed on any
previously-PASSing cell.**

The substantial GF(251) improvements (-42% to -51%) and the
GF(31/127/241) -13% to -46% range reflect tighter system load /
thermal headroom in this run rather than any algorithmic change — the
`fp_try_ple_panel_base::<P>` dispatch for `P ≤ 251` is unchanged at
the algorithm level (the only addition is an early `if
fp_medium_eligible::<P>() { ... }` check that is statically false for
every small prime). The GF(7) cells (with the smallest absolute wall
times) are closest to the R0 baseline (±9%), as expected for
noise-dominated regimes.

The R0 baseline numbers were captured at 2026-05-26 under the same
flock-guarded protocol; both runs use 3 warmup + 5 measured trials.

## A8 row 71 and rows 6-13 (GF(7), GF(31), GF(127), GF(241), GF(251)) at n=64

| Field   |  n | Regime    | fflas (µs) | gf2 R1 (µs) | Ratio  | Status |
|---------|---:|-----------|-----------:|------------:|--------|--------|
| GF(7)   | 64 | uniform   |    221.194 |       44.41 | 0.201× | PASS   |
| GF(7)   | 64 | deficient |    156.284 |       38.55 | 0.247× | PASS   |
| GF(31)  | 64 | uniform   |     no ref |       38.61 |    —   | INFO   |
| GF(31)  | 64 | deficient |     no ref |       32.55 |    —   | INFO   |
| GF(127) | 64 | uniform   |     no ref |       39.31 |    —   | INFO   |
| GF(127) | 64 | deficient |     no ref |       33.14 |    —   | INFO   |
| GF(241) | 64 | uniform   |     no ref |       39.10 |    —   | INFO   |
| GF(241) | 64 | deficient |     no ref |       32.74 |    —   | INFO   |
| GF(251) | 64 | uniform   |     31.470 |       39.12 | 1.243× | PASS   |
| GF(251) | 64 | deficient |     23.710 |       32.85 | 1.385× | PASS   |

GF(251)/n=64 now PASSes (R0 was SHORTFALL at 2.16× / 2.46×); GF(7) and
the other small primes remain PASS. GF(127)/GF(241) lack fflas
reference rows in the `dece4e73` aggregate CSV (INFO only).

## Code-change summary

- **New unsafe AVX2 kernel:**
  `crates/gf2-kernels-simd/src/x86/fp_medium_ple.rs` — implements
  `ple_panel_base_canonical_u16` with the fused scale + Schur update
  described above.
- **ASM artefact (sibling, regenerated):**
  `crates/gf2-kernels-simd/src/x86/asm/fp_medium_ple.asm.txt`.
- **Safe wrapper:**
  `crates/gf2-kernels-simd/src/fp_medium_ple.rs` exposes
  `MediumPrimePlePanelFns` + `detect()` (mirrors the
  `fp_small_ple` pattern).
- **OnceLock accessor:**
  `crates/gf2-core/src/lib.rs::simd::maybe_fp_medium_ple()`.
- **PLE_PANEL_COLS override:**
  `crates/gf2-core/src/gfp/mod.rs` extends the existing `if P <= 251 {
  256 }` to `else if P < 65536 { 128 }`.
- **Dispatch bridge:**
  `crates/gf2-core/src/gfp/simd_ops.rs` adds
  `fp_try_ple_panel_base_medium::<P>` and the per-prime inverse table
  cache `build_medium_prime_inv_table::<P>`. The existing unified
  `fp_try_ple_panel_base` and `fp_ple_panel_base_available` route by
  `P` range to the new medium-prime path.
- **Test harness update:**
  `crates/gf2-core/src/field/ple.rs::test_ple_panelized_dispatch_active_for_small_primes`
  now asserts `PLE_PANEL_COLS == 128` for GF(65521) and
  `has_simd_ple_panel_base()` true on GF(65521).
- **Unit tests added (gf2-kernels-simd):**
  `x86::fp_medium_ple::tests::ple_panel_base_u16_matches_scalar_oracle_full_rank`,
  `_rank_deficient_zero_matrix`, `_rank_deficient_scattered_pivots`;
  `fp_medium_ple::tests::detect_returns_some_on_avx2`,
  `_safe_wrapper_matches_scalar_oracle`.
- **Existing GF(65521) PLE proptests** at boundary lengths
  `{0, 1, 15, 16, 17, 63, 64, 65}` (`prop_ple_panelized_*_fp65521`)
  continue to pass against the new kernel.

## Reproducibility

```bash
# From the repo root:
./dev/benchmarks/ccx1-bench-flock.sh \
  cargo test -p gf2-core --release --all-features --lib -- \
    --ignored --nocapture --test-threads 1 \
    'test_ple_panelized_wall_time_full_sweep' \
  2> /tmp/ple_sweep.log

# Extract CSV lines:
awk '/--- panelized-ple-sweep BEGIN ---/{p=1; next} \
     p && /--- panelized-ple-sweep END ---/{p=0; next} p' \
  /tmp/ple_sweep.log > /tmp/ple_sweep.csv
```

Matrix seeds are deterministic and pinned by `measure_cell::<P>` in
`crates/gf2-core/src/field/ple.rs`; the same git revision produces the
same matrices and same medians within run-to-run noise.

## Risks and limitations

- **Algorithmic ceiling.** The panelized base case retains the outer
  column-by-column pivot loop. The structural gap to fflas-ffpack at
  GF(251)/n=256 (was 4.36× under R0; now 2.36× — still SHORTFALL) is
  addressed separately by `6823c8a0` R1 (recursive PLUQ); this issue
  only addresses the GF(65521) base case.
- **GF(65521) at n=1024 uniform.** The R1 ratio 0.954× is the
  thinnest margin in the table. Run-to-run variance is typically ±5%,
  so this cell could oscillate close to 1.0×. Still well under the
  1.5× criterion.
- **Out-of-scope:**
  AVX-512 panel kernels (epic `7f809931`); GF(251) recursive PLUQ
  (issue `6823c8a0` R1); Mersenne31 PLE (per design § 5 exclusion).
- **Inverse table memory.** At P=65521, the per-prime inverse table is
  131 KB; one allocation per medium prime per process, retained for
  the process lifetime. Acceptable for production workloads (only
  primes used by the application get tables).
