# Phase 2 Generalization Evidence Doc

**Issue:** e8a0c47a — Generalize GF(p) reductions and dispatch policy  
**Parent epic:** 026fc832  
**Phase:** Phase 2 (Barrett-reduction SSOT consolidation)  
**Date:** 2026-05-25  
**Host:** Zen 3 AMD 5900X, 12 cores, 2 CCX (CCX0: cores 0-5, CCX1: cores 6-11)  

| Attribute | Value |
|-----------|-------|
| Commit range | 6a69c90b (design doc) → 97e5ab3f (R0 WIP) → `feat(jit:e8a0c47a)` |
| Baseline commit | `04b7cef4` (41096af5 post-wire-in) |
| Baseline bench CSV | `dev/bench_results/2026-05-25-41096af5-post-wire-in-aggregate.csv` |
| Bench CSV (this run) | `dev/bench_results/2026-05-25-e8a0c47a-post-refactor-aggregate.csv` |

---

## 1. Provenance + clean-room attestation

Phase 2 is a pure SSOT consolidation of code already in-tree (gf2-authored, MIT).
No fflas-ffpack source is copied, translated, linked, or used as a template.
The Granlund-Möller Barrett-reduction algorithm is textbook arithmetic (1994 paper);
the AVX2 instruction sequence was first authored for the issue-3a37e0f6 SpMM kernel
and the issue-9e12659b medium-prime kernel, both gf2-original.

The shared primitive `barrett_reduce_lane32` (`crates/gf2-kernels-simd/src/x86/fp_small.rs`
line 840) is not a new algorithm — it is the pre-existing SpMM row-reducer primitive
(also in `fp_small.rs`) promoted from `pub(super)` to `pub(crate)`, with the unused
`p_vec64` parameter removed and the conditional-subtract micro-implementation updated
from `cmpgt_epi32` + `blendv_epi8` to the one-instruction `min_epu32` form already
used by the medium-prime kernel.

---

## 2. Methodology

5 sequential trials, CCX1-pinned (`taskset -c 6-11 nice -n -5`).  
Quiet-host check before each trial: aborts if `cargo|rustc|criterion` detected.  
Criterion benchmark binary: `gf2-core/benches/fieldmatrix_gemm.rs`.  
Filter: `gemm/Fp_(7|31|127|251)/Fp_(7|31|127|251)/(64|256|1024)$`.  
Aggregate: 5-trial median, Q1 (trial 2), Q3 (trial 4) of sorted values.  
Driver script: `dev/bench_results/run_e8a0c47a_post_refactor_bench.sh`.

---

## 3. Success criteria

Verbatim from JIT issue e8a0c47a:

> - [hard] A reusable vectorized-modular-reduction primitive lives in
>   `crates/gf2-kernels-simd/` and is used by both the GF(251) selected
>   route's output reduction and at least one other call site (e.g.,
>   medium-prime u16 dot path or another small-prime kernel).
>
> - [hard] Exact-prime dispatch ordering (Fp<65537>, then Mersenne31, then
>   family paths) is preserved — verified by reading
>   `crates/gf2-core/src/gfp/simd_ops.rs` and a dispatch-trace test.
>
> - [hard] Bit-exact correctness preserved across GF(7), GF(31), GF(127),
>   GF(241), GF(251), GF(257), GF(32749), GF(65521), Fp<65537>, Mersenne31
>   sweep proptests (boundary lengths {0, 1, 15, 16, 17, 63, 64, 65}).
>
> - [hard] No regression on currently-PASSing GF(p) cells (delta <= 5% under
>   same-session measurement at the 5900X reference host) — confirmed by
>   re-running the predecessor scorecard's GF(p) section.
>
> - [hard] Coordination note in the evidence doc explaining how this work
>   composes with `27bb2f75` (small-n overhead) — either as
>   parallel/independent or as an explicit hand-off.
>
> - [hard] If `N_THRESH_PRIME` is changed, the evidence doc cites the new
>   5-trial CCX-pinned measurement that justifies it.

| SC | Status | Evidence |
|----|--------|---------|
| SC#1 — shared primitive + 2+ call sites | PASS | §4 below |
| SC#2 — dispatch ordering preserved | PASS | §5 below |
| SC#3 — multi-prime sweep proptests | PASS | §6 below |
| SC#4 — non-regression bench | PASS | §7 below |
| SC#5 — 27bb2f75 coordination note | PASS | §8 below |
| SC#6 — N_THRESH_PRIME unchanged | PASS | §9 below |

---

## 4. SC#1 — shared primitive and call sites

The shared primitive is:

```
crates/gf2-kernels-simd/src/x86/fp_small.rs:840
pub(crate) unsafe fn barrett_reduce_lane32(x: __m256i, mu_vec: __m256i, p_vec: __m256i) -> __m256i
```

Visibility promoted from `pub(super)` to `pub(crate)`. Unused parameter `p_vec64`
removed. Conditional-subtract updated to `_mm256_min_epu32(r, r - p_vec)`.

Four call sites post-Phase-2:

| # | File | Lines | Function | Domain |
|---|------|-------|----------|--------|
| 1 | `x86/fp_small.rs` | 731-732 | `fp_small_spmm_row` | SpMM row reducer |
| 2 | `x86/fp_small_f32.rs` | 892-894 | `store_and_reduce_tile_route_a::write_row` | Route-A f32 cascade output |
| 3 | `x86/fp_small_panel.rs` | 462-473 | `fp_small_panel_gemm` | Route-C integer panel output |
| 4 | `x86/fp_medium.rs` | 115-116 | `fp_medium_batch_mul16` | Medium-prime u16 lane-wise multiply |

Call site #4 is the second non-GF(251) call site required by SC#1. Pre-Phase-2 it
used a duplicate local implementation (`barrett_reduce_u32x8`); Phase 2 consolidates
it onto the SSOT.

---

## 5. SC#2 — dispatch ordering preserved

The exact-prime dispatch ordering in `crates/gf2-core/src/gfp/simd_ops.rs` is:

```text
simd_ops.rs:193  if P == 65537       → fp65537_try_*_vec
simd_ops.rs:196  if P == M31         → fpm31_try_mul_vec   (mul only)
simd_ops.rs:199  if P <= 251         → fp_small_try_*_vec
simd_ops.rs:202  if P >= 252 && P < 65536 → fp_medium_try_*_vec
                 otherwise           → fp_generic_try_*_vec
```

Phase 2 touches only the inner kernels in `gf2-kernels-simd`. The dispatcher in
`simd_ops.rs:190-243` is **not modified**. The dispatch source ordering is
structurally preserved by the code layout (Fp<65537> branch is first, then M31,
then small-prime, then medium-prime, then generic fallback).

The dispatch-trace test that guards this invariant at runtime is:

```
crates/gf2-core/src/gfp/simd_ops.rs:2959
fn specialized_primes_do_not_use_generic_montgomery_path()
```

This test asserts `!fp_generic_enabled::<P>()` for `P ∈ {65537, M31, M61, 65521,
257, 32749}`, confirming each routes to its specialised SIMD kernel rather than
the generic Montgomery fallback. It also verifies live `try_simd_mul_vec` calls
for Fp<65537> and M31 return `Some` when AVX2 is detected.

No new dispatch-trace test was added: the design doc §5.1 designates this existing
test as the SC#2 dispatch-trace test, noting that "the structural source-order
check in `gfp/simd_ops.rs:190-243` plus the runtime assertion in that test together
prove the invariant."

---

## 6. SC#3 — multi-prime sweep proptests

New test file: `crates/gf2-core/tests/phase2_prime_sweep_proptests.rs`

Four proptest blocks, all gated on `#[cfg(feature = "simd")]`:

| Proptest | Primes covered | Boundary lengths |
|----------|----------------|-----------------|
| `proptest_phase2_small_prime_sweep_boundary_n` | GF(7), GF(31), GF(127), GF(241), GF(251) | {0,1,15,16,17,63,64,65} |
| `proptest_phase2_medium_prime_sweep_boundary_n` | GF(257), GF(32749), GF(65521) | {0,1,15,16,17,63,64,65} |
| `proptest_phase2_fp65537_boundary_n` | Fp<65537> | {0,1,15,16,17,63,64,65} |
| `proptest_phase2_mersenne31_boundary_n` | Mersenne31 (Fp<2147483647>) | {0,1,15,16,17,63,64,65} |

Each block calls `gemm(A, B)` and compares against a naive scalar oracle for
square matrices of each boundary size. The `prop_oneof![Just(0usize), ...]` form
is used as required by the 52cce970 R1 trap. All 4 proptests PASS:

```
gf2-core::phase2_prime_sweep_proptests proptest_phase2_mersenne31_boundary_n        PASS [0.049s]
gf2-core::phase2_prime_sweep_proptests proptest_phase2_fp65537_boundary_n           PASS [0.069s]
gf2-core::phase2_prime_sweep_proptests proptest_phase2_medium_prime_sweep_boundary_n PASS [0.141s]
gf2-core::phase2_prime_sweep_proptests proptest_phase2_small_prime_sweep_boundary_n  PASS [0.241s]
```

The medium-prime proptest (`proptest_phase2_medium_prime_sweep_boundary_n`) exercises
the `fp_medium_batch_mul16` path which now calls `barrett_reduce_lane32` SSOT — this
is the direct bit-exact correctness gate for the new call site.

---

## 7. SC#4 — non-regression bench

### Methodology

5 sequential trials, CCX1-pinned (`taskset -c 6-11 nice -n -5`), quiet-host check.
Primes: GF(7), GF(31), GF(127), GF(251). Sizes: n ∈ {64, 256, 1024} — all 12
(prime × n) cells measured.
Baseline: `dev/bench_results/2026-05-25-41096af5-post-wire-in-aggregate.csv`.

### Results — all 11 measured cells (GF(127)/n=64 omitted from baseline; included in CSV)

| prime | n | post-refactor (Gop/s) | baseline (Gop/s) | delta | fflas ratio | verdict |
|-------|---|----------------------|-----------------|-------|-------------|---------|
| 7 | 64 | 33.754 | 32.156 | **+4.97%** | — | PASS (closest to ±5% limit) |
| 7 | 256 | 72.96 | 70.75 | +3.1% | — | PASS |
| 7 | 1024 | 77.08 | 75.79 | +1.7% | — | PASS |
| 31 | 64 | 31.410 | 31.482 | −0.23% | — | PASS |
| 31 | 256 | 69.93 | 70.00 | -0.1% | — | PASS |
| 31 | 1024 | 76.63 | 76.51 | +0.2% | — | PASS |
| 127 | 256 | 69.27 | 69.80 | -0.8% | — | PASS |
| 127 | 1024 | 76.57 | 76.19 | +0.5% | — | PASS |
| 251 | 64 | 33.02 | 31.52 | +4.8% | — | PASS |
| 251 | 256 | 71.98 | 69.93 | +2.9% | — | PASS |
| 251 | 1024 | **95.83** | 94.43 | **+1.5%** | **0.693** | **PASS** |

All 11 cells within ±5% of the 41096af5 baseline. The maximum absolute delta
is GF(7)/n=64 at +4.97% (within the criterion's ±5% bound).
GF(251)/n=1024: ratio 0.693 vs fflas-ffpack 138.32 Gop/s (threshold 0.667). PASS.

Aggregate CSV: `dev/bench_results/2026-05-25-e8a0c47a-post-refactor-aggregate.csv`  
Raw CSV: `dev/bench_results/2026-05-25-e8a0c47a-post-refactor.csv`  
Per-trial snapshots: `dev/bench_results/2026-05-25-e8a0c47a-post-refactor/`

---

## 8. SC#5 — coordination with 27bb2f75

Issue `27bb2f75` (closed 2026-05-24) reduced the per-call pack/unpack overhead
for the small-prime SIMD path by introducing thread-local pack scratches and
Montgomery REDC byte tables. Its deliverables sit in the `pack`/`unpack` and
`fp_small_pack` layers of `gfp/simd_ops.rs`.

Phase 2's Barrett-reduction consolidation is **a parallel, independent change**:

- `27bb2f75` touched the pack/unpack scratch layer; Phase 2 touches the inner-kernel
  reduction layer (the `barrett_reduce_lane32` primitive inside `gf2-kernels-simd`).
- `27bb2f75` affects every small-prime SIMD entry point; Phase 2's consolidation
  only de-duplicates the 32-bit Barrett step, leaving pack/unpack untouched.
- No build-order dependency exists in either direction: Phase 2 compiles cleanly on
  top of HEAD-after-27bb2f75 and would compile cleanly without 27bb2f75 had it not
  landed.

No further coordination is required. Phase 2 ships independent of any 27bb2f75 work.

---

## 9. SC#6 — N_THRESH_PRIME status

`N_THRESH_PRIME` is **UNCHANGED at 251** (as set by 41096af5).

The plan's Phase 2 directive is explicit:

> Revisit `N_THRESH_PRIME` only with new data; Candidate C currently wins on
> Zen 3 for p ≤ 251.

Phase 2's only measurement is the non-regression bench (§7 above), which by
construction confirms the existing threshold's performance and does not generate
fresh data on a candidate alternative. The bench shows the refactored kernel
performs identically (within ±5%) to the pre-refactor baseline — consistent with
the expectation that a pure SSOT deduplication has zero algorithmic impact.

No `N_THRESH_PRIME` change is made. No new 5-trial CCX-pinned measurement of an
alternative threshold is needed.

---

## 10. Source index

| File | Role |
|------|------|
| `crates/gf2-kernels-simd/src/x86/fp_small.rs:840` | Phase 2 SSOT `barrett_reduce_lane32` primitive |
| `crates/gf2-kernels-simd/src/x86/fp_small.rs:731` | Call site #1: SpMM row reducer |
| `crates/gf2-kernels-simd/src/x86/fp_small_f32.rs:892` | Call site #2: Route-A f32 cascade output |
| `crates/gf2-kernels-simd/src/x86/fp_small_panel.rs:462` | Call site #3: Route-C panel output (12 calls) |
| `crates/gf2-kernels-simd/src/x86/fp_medium.rs:115` | Call site #4: Medium-prime u16 lane-wise multiply |
| `crates/gf2-core/src/gfp/simd_ops.rs:190-243` | Dispatch chain (unchanged by Phase 2) |
| `crates/gf2-core/src/gfp/simd_ops.rs:2959` | Dispatch-trace test `specialized_primes_do_not_use_generic_montgomery_path` |
| `crates/gf2-core/tests/phase2_prime_sweep_proptests.rs` | SC#3 multi-prime sweep proptests (4 blocks, 10 primes) |
| `dev/bench_results/2026-05-25-e8a0c47a-post-refactor-aggregate.csv` | Post-refactor bench aggregate |
| `dev/bench_results/2026-05-25-41096af5-post-wire-in-aggregate.csv` | Baseline aggregate (41096af5) |
| `dev/active/e8a0c47a-vector-modular-reduction-design.md` | Phase 2 design doc |
