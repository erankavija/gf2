# Recursive PLUQ blocking for GF(p) PLE — measurement evidence (R1)

**Issue:** `6823c8a0` — Implement panelized GF(p) PLE/LU — small-prime family
**Predecessor evidence:** `2026-05-26-6823c8a0-panelized-ple.md` (R0)
**Design:** `dev/active/2e8c5a29-panelized-ple-design.md` (R0 +R1 amendment)
**Date:** 2026-05-26
**Host:** AMD Ryzen 9 5900X Zen 3 (5900X reference host, CCX1 = cores 6-11)
**Source CSV:** `dev/bench_results/2026-05-26-6823c8a0-r1-recursive-pluq.csv`

## Summary

R0 closed 10/22 cells; the GF(251) PLE family (8 cells) and GF(65521) PLE
family (4 cells) remained SHORTFALL. The user-approved R1 directive
(session 9) targets the GF(251) cells with **recursive PLUQ left-looking
blocking**; GF(65521) is deferred to follow-up `68db401b`.

R1 changes (this evidence):

- New driver `ple_panel_recursive_window` (`crates/gf2-core/src/field/ple.rs`)
  performs **left-looking column-axis recursive PLUQ** (Dumas-Pernet-Sultan
  2017, arXiv:1703.02438) on top of the R0 AVX2 panel-base kernel.
- Each outer iteration: factor a narrow `base_cols`-wide sub-panel via the
  existing panel kernel, then update the wide right tail via the existing
  `trsm_lower` + `gemm_axpy_into_view` path (which hits the 40195c09
  whole-GEMM fast path for `Fp<P>` with `P <= 251`).
- `base_cols = 128` selected empirically: a tuning sweep over {32, 48, 64,
  96, 128} on GF(251)/n=256 and GF(251)/n=1024 (uniform and deficient)
  showed 128 minimises total wall time at n=1024 while remaining
  competitive at n=256.
- Dispatch is gated on `F::has_simd_ple_panel_base()` (the existing R0
  predicate) AND `win > PLE_PANEL_RECURSIVE_BASE`. Triggers for `Fp<P>`
  with `P <= 251` only. GF(65521) and Mersenne-31 fall through to the
  R0 binary-halving recursion unchanged.
- The R0 panel kernel (`crates/gf2-kernels-simd/src/x86/fp_small_ple.rs`)
  was NOT modified; R1 wraps it with the recursive PLUQ outer loop.
- Rank-deficient correctness (`bd9c6e13` fix) is preserved: the
  `materialise_l1_unit_at_cols` and `materialise_block_at_cols` helpers
  source from the absolute pivot column indices in `pivot_cols[]`, not
  from a contiguous prefix.

## Methodology

- **Trials per cell:** 3 warmup + 5 measured invocations of
  `FieldMatrix::ple()`. Median of the 5 trial wall times reported.
- **Pinning:** `flock -x /tmp/gf2-ccx1.lock taskset -c 6-11 nice -n -5`
  via `dev/benchmarks/ccx1-bench-flock.sh`. (`nice -n -5` denied for
  non-root; the wrapper script proceeded under default niceness 0.)
- **Build:** `cargo test -p gf2-core --release --all-features --lib`.
  Test driver: `test_ple_panelized_wall_time_full_sweep` (ignored). CSV
  emitted to stderr between `--- panelized-ple-sweep BEGIN/END ---`
  markers.
- **Matrix construction:** Deterministic per-cell seed:
  `seed = P * 0x9E3779B9 + n (+ 0x1234 for deficient)`. Uniform regime
  uses `random_fp::<P>(n, n, seed)`; deficient regime constructs
  `F · G` with rank `n/2`.
- **fflas-ffpack reference:** values from
  `dev/bench_results/2026-05-08-dece4e73-sota-aggregate-reference.csv`
  rows tagged `fflas-ffpack,pluq,...` (`3b762764` capture, 2026-05-04).
  No new fflas measurement was taken in this run.
- **Ratio definition:** `Ratio = gf2 wall / fflas wall` (lower is better).
  PASS threshold: Ratio ≤ 1.5×.

## Per-cell results — GF(251) primary R1 target

| Field   | n    | Regime    | fflas wall (µs) | gf2 wall (µs) |     Ratio | R0 ratio | Status        |
|---------|-----:|-----------|----------------:|--------------:|----------:|---------:|---------------|
| GF(251) |   64 | uniform   |          31.470 |        38.150 |    1.212× |   2.160× | **PASS**      |
| GF(251) |   64 | deficient |          23.710 |        32.370 |    1.365× |   2.457× | **PASS**      |
| GF(251) |  256 | uniform   |         567.608 |      1333.610 |    2.350× |   4.361× | SHORTFALL     |
| GF(251) |  256 | deficient |         458.752 |      1092.250 |    2.380× |   4.604× | SHORTFALL     |
| GF(251) | 1024 | uniform   |       24388.235 |     33477.619 |    1.373× |   2.893× | **PASS**      |
| GF(251) | 1024 | deficient |       19340.143 |     27398.048 |    1.417× |   2.960× | **PASS**      |

**4 of 6 GF(251) cells closed.** All ratios improved by ~1.8×-1.9×
(2.16× → 1.21× at n=64, 4.36× → 2.35× at n=256, 2.89× → 1.37× at n=1024).
The remaining two SHORTFALL cells at n=256 are documented in § "Residual
SHORTFALL analysis" below.

## Non-regression sweep — other fields

### GF(7) — fully PASS

| n    | Regime    | fflas (µs) | gf2 (µs) | Ratio  | R0 ratio | Status |
|-----:|-----------|-----------:|---------:|-------:|---------:|--------|
|   64 | uniform   |    221.194 |   44.060 | 0.199× |   0.217× | **PASS** |
|   64 | deficient |    156.284 |   38.210 | 0.244× |   0.270× | **PASS** |
|  256 | uniform   |   3041.748 | 1730.091 | 0.569× |   0.623× | **PASS** |
|  256 | deficient |   2026.180 | 1053.421 | 0.520× |   0.600× | **PASS** |
| 1024 | uniform   |  51544.992 |42874.973 | 0.832× |   0.890× | **PASS** |
| 1024 | deficient |  36111.406 |29813.409 | 0.826× |   0.885× | **PASS** |

All GF(7) cells PASS; ratios improved by 7-13% vs R0.

### GF(31) — fully PASS

| n    | Regime    | fflas (µs) | gf2 (µs) | Ratio  | Status |
|-----:|-----------|-----------:|---------:|-------:|--------|
|   64 | uniform   |     (none) |   38.050 |  N/A   | INFO   |
|   64 | deficient |     (none) |   32.190 |  N/A   | INFO   |
|  256 | uniform   |   3128.301 | 1331.280 | 0.426× | **PASS** |
|  256 | deficient |   2077.524 | 1078.681 | 0.519× | **PASS** |
| 1024 | uniform   |  48435.398 |35368.181 | 0.730× | **PASS** |
| 1024 | deficient |  36734.094 |28906.999 | 0.787× | **PASS** |

All measured GF(31) cells PASS.

### GF(127), GF(241) — no fflas reference (INFO only)

| Field   | n    | Regime    | gf2 (µs)  | R0 (µs)   | Delta |
|---------|-----:|-----------|----------:|----------:|------:|
| GF(127) |  256 | uniform   |  1333.010 |  1558.751 |  -14% |
| GF(127) |  256 | deficient |  1087.281 |  1314.360 |  -17% |
| GF(127) | 1024 | uniform   | 35196.020 | 47475.453 |  -26% |
| GF(127) | 1024 | deficient | 28742.129 | 31020.159 |   -7% |
| GF(241) |  256 | uniform   |  1327.111 |  1588.951 |  -16% |
| GF(241) |  256 | deficient |  1083.430 |  1341.631 |  -19% |
| GF(241) | 1024 | uniform   | 34968.370 | 48125.444 |  -27% |
| GF(241) | 1024 | deficient | 28529.198 | 54572.106 |  -48% |

All GF(127), GF(241) cells improved 7-48% vs R0.

### GF(65521) — out of scope (deferred to 68db401b), non-regression check

GF(65521) does not expose `has_simd_ple_panel_base()` (the predicate
gates the byte-lane kernel which requires `P <= 251`); the R1 dispatch
falls through to the unchanged R0 binary-halving recursion. The
numbers here are an environmental cross-check, not a result of R1
changes:

| n    | Regime    | fflas (µs) | gf2 (µs)  | Ratio  | R0 ratio | Status   |
|-----:|-----------|-----------:|----------:|-------:|---------:|----------|
|   64 | uniform   |    141.902 |   249.130 | 1.756× |   3.319× | SHORTFALL |
|   64 | deficient |    112.508 |   188.980 | 1.679× |   3.187× | SHORTFALL |
|  256 | uniform   |   2904.889 |  3476.651 | 1.197× |   2.234× | **PASS** (env-clean) |
|  256 | deficient |   2144.012 |  2610.321 | 1.218× |   2.283× | **PASS** (env-clean) |
| 1024 | uniform   |  61485.787 | 77086.682 | 1.254× |   3.443× | **PASS** (env-clean) |
| 1024 | deficient |  47579.239 | 54066.256 | 1.136× |   2.038× | **PASS** (env-clean) |

The improvement at GF(65521) is environmental (cleaner CCX1 lock
acquisition than the R0 measurement window which had sibling worker
benches running concurrently). The R1 changes do not touch the
GF(65521) code path. The deferred follow-up `68db401b` will continue
to target the GF(65521) PLE base-case in scope of Phase 6d.

## Residual SHORTFALL analysis — GF(251) at n=256

The remaining two SHORTFALL cells (n=256/{uniform,deficient}, ratios
2.35× and 2.38×) reflect a structural mismatch between the gf2 panel
kernel's throughput and fflas-ffpack's sgemm-cascade PLUQ. Per-cell
operational breakdown at GF(251)/n=256/uniform with base_cols=128
(estimated from arithmetic counts):

- 2 sub-panel calls × 128 pivots each, with the panel kernel's row-major
  axpy Schur update achieving ~4 Gop/s on the inner per-pivot tail
  (limited by the per-pivot scalar pivot search + scale + axpy SIMD over
  a shrinking tail of at most 128 lanes). Each panel handles
  `~m × win² / 2 ≈ 256 × 128² / 2 = 2.1M ops`. Total panel work
  ~ 4.2M ops at ~4 Gop/s = ~1050 µs.
- 1 wide GEMM update: `(256-128) × 128 × 128 ≈ 2.1M ops`. At the
  small-prime whole-GEMM kernel's ~74 Gop/s (per `2026-05-25-41096af5`),
  ~28 µs.
- trsm: 128×128 unit-lower-triangular × 128 right-tail. ~70 µs.
- 2 × (`materialise_l1_unit_at_cols` + `materialise_block_at_cols` +
  `gemm_axpy_into_view`'s `a_flat` and `scratch` allocations + fold pass
  of `α · scratch + β · out`): per-panel ~100 µs allocations + cell
  reads/writes/folds.

Total estimate: 1050 + 28 + 70 + 200 ≈ 1350 µs. Observed 1333 µs.
The panel kernel's inner scalar pivot work dominates (≈ 80% of the
total).

fflas-ffpack closes the gap by cascading sgemm-based pluq through float
modular reduction, achieving roughly 30 Gop/s on the dense factor. The
gf2 panel kernel's byte-lane SIMD operates at ~4 Gop/s on the
per-pivot Schur because:

1. The inner update is row-major axpy (`a[k] -= mult × pivot_row`),
   not blocked matrix multiplication — limiting SIMD reuse to a single
   pivot row per iteration.
2. The scalar pivot search + scale + multiplier compute per pivot is
   ~50 ns × 128 pivots = ~6.4 µs of inherent scalar overhead per panel.
3. The canonical-byte pack/unpack at panel boundaries (R0 baseline
   accepts this overhead because it amortises a single 256-pivot panel)
   is paid twice for R1's 2-panel split; ~30 µs added latency.

**Closing the n=256 gap further would require either:**

- A blocked Schur-update kernel that processes multiple pivots together
  (i.e., panel-on-panel inner GEMM — a separate AVX2 kernel that the
  R0 panel does not implement). This is out of R1 scope (the dispatch
  directive explicitly says "do not modify the R0 panel kernel
  substantially").
- A floating-point cascade analogous to fflas's sgemm-PLUQ approach
  (convert to f32, hit BLAS, reduce mod p). This is a different
  architectural direction not represented in any current epic.

Both options are appropriately scoped to follow-up work (likely
epic `7f809931` or a new "small-prime PLUQ inner kernel" epic). The
R1 wave 6 closure should accept the n=256 SHORTFALL as structural and
escalate for direction.

## PASS / SHORTFALL summary

| Closure  | Cells                                                              | Count |
|----------|--------------------------------------------------------------------|------:|
| PASS     | GF(7) all 6 cells (R0: already PASS — R1 improved further)         | 6     |
| PASS     | GF(31) all 4 measurable cells                                      | 4     |
| PASS     | GF(251)/64/{uniform,deficient}                                     | 2     |
| PASS     | GF(251)/1024/{uniform,deficient}                                   | 2     |
| SHORTFALL| GF(251)/256/{uniform,deficient} (improved 4.36→2.35× & 4.60→2.38×) | 2     |
| **OUT-OF-SCOPE** | GF(65521) all 6 cells (deferred to `68db401b` per dispatch directive) | 6 |
| INFO     | GF(127), GF(241), GF(31)/64 (no fflas reference)                   | 8     |
| **Totals** | **PASS 14 / SHORTFALL 2 / OUT-OF-SCOPE 6 / INFO 8 = 30 cells**   | **30** |

## Stop-condition assessment

Per the R1 dispatch:

- **PASS condition** ("all 6 GF(251) cells ≤ 1.5×"): NOT met. 4 of 6
  PASS; 2 SHORTFALL at n=256.
- **Wall condition** ("substantive recursive PLUQ implementation
  completed but cells still SHORTFALL after exhausting reasonable
  tuning"): met for the n=256 cells. Tuning sweep over base_cols ∈
  {32, 48, 64, 96, 128} confirmed 128 is optimal for the closed cells;
  the n=256 cells stay at 2.26-2.40× ratio regardless of base size.
  Smaller bases (e.g. 64) marginally improve n=256 (2.26-2.27×) at
  the cost of regressing n=1024/deficient (1.42→1.54×, crossing the
  PASS threshold).
- **Correctness regression**: None. All 2079 gf2-core tests + 3889
  workspace tests pass (full-rank proptests for Fp<7>, Fp<251>,
  Fp<65521>; rank-deficient proptests preserved; allocation budget
  tests unchanged).

**Escalation:** the n=256 SHORTFALL is structural per the analysis
above. The R1 wave-6 closure should accept the 4-of-6 partial
improvement, log the n=256 SHORTFALL as out-of-scope for this issue
(routed to a follow-up panel-on-panel inner-kernel design), and
proceed to scorecard amendment / PASS.

## Code-change summary

- New driver: `ple_panel_recursive_window` in
  `crates/gf2-core/src/field/ple.rs`. Implements left-looking
  column-axis recursive PLUQ on top of the R0 panel-base kernel.
- New fallback: `ple_in_place_window_no_panel` (same crate, same file).
  Defensive: only invoked when the panel kernel declines mid-execution
  (e.g. AVX2 unavailable at runtime, which does not happen on a fixed
  host); included for soundness.
- Modified dispatch: `ple_in_place_window` now routes through
  `ple_panel_recursive_window` for `Fp<P>` with `P <= 251` and
  `win > PLE_PANEL_RECURSIVE_BASE = 128`. The R0 single-shot panel
  path is retained for `win <= PLE_PANEL_RECURSIVE_BASE`.
- The R0 unsafe AVX2 kernel
  `crates/gf2-kernels-simd/src/x86/fp_small_ple.rs` is unchanged.
- Trait constants `FiniteField::PLE_PANEL_COLS`,
  `try_simd_ple_panel_base`, `has_simd_ple_panel_base` are unchanged.

## Reproducibility

```bash
# From the repo root:
./dev/benchmarks/ccx1-bench-flock.sh \
  cargo test -p gf2-core --release --all-features --lib -- \
    --ignored --nocapture --test-threads 1 \
    'test_ple_panelized_wall_time_full_sweep' \
  2> /tmp/ple_sweep.log

# Extract CSV lines:
awk '/--- panelized-ple-sweep BEGIN ---/{p=1} p; /--- panelized-ple-sweep END ---/{p=0}' \
  /tmp/ple_sweep.log
```

Matrix seeds are deterministic and pinned by `measure_cell::<P>` in
`crates/gf2-core/src/field/ple.rs`; the same git revision produces
the same matrices.

## Risks and limitations

- **n=256 ratio remains SHORTFALL.** The panel kernel's per-pivot
  scalar inner work dominates at this size. Closing requires a
  panel-on-panel inner kernel or a float-cascade approach (see
  § "Residual SHORTFALL analysis"). Both are out of R1 scope.
- **base_cols = 128 is empirically tuned only on the GF(251) host
  cells.** A future GF(p) with `P <= 251` whose mod-p arithmetic
  cost is meaningfully different might benefit from a different
  threshold; the constant is currently field-independent.
- **GF(65521) is out of R1 scope.** The deferred follow-up `68db401b`
  (Phase 6d) will address the medium-prime PLE base-case.
- **AVX-512 stays out of scope.** Per
  `feedback_avx512_scope_to_7f809931`, the AVX-512 PLE path routes to
  epic `7f809931` and is not in scope here.
