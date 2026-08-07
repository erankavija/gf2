# Design: reusable vectorized modular-reduction primitive (Phase 2)

**Issue:** e8a0c47a — Generalize GF(p) reductions and dispatch policy
**Parent epic:** 026fc832
**Predecessor:** 41096af5 (Phase 1 fan-in, closed)
**Plan section:** `dev/active/615db3b9-finite-field-la-sota-plan.md` § Phase 2

## 1. Problem

The 615db3b9 SOTA plan's Phase 2 directive (verbatim, the clause this design
addresses):

> Make vectorized modular reduction a reusable primitive for f32/double cascade
> output and integer-panel outputs.

Pre-Phase-2 state: two functionally-equivalent 32-bit-lane Barrett-reduction
implementations exist in the AVX2 kernel tree:

1. `crates/gf2-kernels-simd/src/x86/fp_small.rs::barrett_reduce_lane32` — the
   SSOT used by:
   - the SpMM row reducer in `fp_small.rs` itself,
   - the route-A f32 cascade output (`fp_small_f32.rs::store_and_reduce_tile_route_a`)
     via the one-line wrapper `barrett_reduce_lane32_local`,
   - the route-C integer panel output (`fp_small_panel.rs` lines 471-482).

2. `crates/gf2-kernels-simd/src/x86/fp_medium.rs::barrett_reduce_u32x8` — a
   separate copy serving the medium-prime u16 lane-wise multiply
   (`fp_medium_batch_mul16`).

Both reduce `x ∈ [0, 2^32)` modulo `p` using `q = ⌊x · μ / 2^32⌋`, `r = x − q·p`,
single conditional subtract. They produce identical results for every
`p ∈ [3, 2^16)`.

The Phase 2 deliverable is a **single shared primitive** for all four call
sites, with no behavior change.

## 2. API of the shared primitive

```rust
/// 32-bit-lane Barrett reduction: returns `x mod p` for each of 8 u32 lanes.
///
/// Inputs:
///   x       : __m256i — 8 u32 lanes, each in [0, 2^32).
///   mu_vec  : __m256i — broadcast Barrett constant μ = ⌊2^32 / p⌋.
///             Either `_mm256_set1_epi64x(μ as i64)` or
///             `_mm256_set1_epi32(μ as i32)` is acceptable; only the low
///             32 bits of each 64-bit lane are read by `_mm256_mul_epu32`.
///   p_vec   : __m256i — broadcast p as 8 u32 lanes (`_mm256_set1_epi32(p as i32)`).
///
/// Returns: __m256i — 8 reduced u32 lanes, each in [0, p).
///
/// # Algorithm (Granlund-Möller, one-step branchless)
///
/// 1. Extract even and odd u32 lanes of `x` (mask / shift-right by 32).
/// 2. `q_even_64 = mul_epu32(x_even, mu_vec)`, `q_odd_64 = mul_epu32(x_odd, mu_vec)`
///    — 4 × u32 × u32 → u64 products per call.
/// 3. Take the high 32 bits of each 64-bit product (`>> 32`); reinterleave
///    even/odd back into 8 u32 lanes → `q`.
/// 4. `r = x − q·p` via `_mm256_sub_epi32(x, _mm256_mullo_epi32(q, p_vec))`.
///    Bound: `r ∈ [0, 2p)`.
/// 5. Single branchless conditional subtract: `_mm256_min_epu32(r, r − p_vec)`.
///    When `r ≥ p`, `r − p < r` (unsigned) and min picks it; when `r < p`,
///    `r − p` underflows to a value `> r` and min keeps `r`.
///
/// # Safety
///
/// Caller must ensure AVX2 is available at runtime.
#[inline]
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn barrett_reduce_lane32(
    x: __m256i,
    mu_vec: __m256i,
    p_vec: __m256i,
) -> __m256i;
```

### 2.1 API changes vs. the pre-Phase-2 primitive

The pre-Phase-2 signature was:

```rust
pub(super) unsafe fn barrett_reduce_lane32(
    x: __m256i,
    mu_vec: __m256i,
    p_vec: __m256i,
    p_vec64: __m256i,  // unused
) -> __m256i
```

Phase 2 changes:

- **Drop the unused `p_vec64` parameter.** The pre-existing implementation
  carried it "for interface symmetry with the calling context" but never
  read it (see the `let _ = p_vec64;` near the end). Removal eliminates dead
  parameter passing at every call site and removes a needless dependency on
  the caller knowing to construct an extra broadcast vector.

- **Promote visibility to `pub(crate)`.** The pre-existing `pub(super)` only
  reached sibling modules under `x86/`. The Phase 2 consolidation will be
  used from `x86/fp_medium.rs` (already a sibling — `pub(super)` would
  suffice for that move) but `pub(crate)` is the discoverable choice for an
  SSOT primitive consumed by multiple sibling kernels and signals "this is
  a shared building block".

- **Switch the conditional-subtract micro-implementation** from
  `_mm256_cmpgt_epi32(0, r2)` + `_mm256_blendv_epi8(r2, r, mask)` to
  `_mm256_min_epu32(r, r − p_vec)`. The min form is one fewer instruction
  (no separate compare; no blend) and is what `fp_medium::barrett_reduce_u32x8`
  already uses. The result is bit-equal for every input in the valid range
  `[0, 2p)`.

These three changes together net **-1 dead parameter, -1 wrapper layer,
-1 duplicated copy** versus the pre-Phase-2 state, and make the primitive
the obvious shared call site for any future 32-bit-lane Barrett work
(e.g. a future medium-prime panel kernel).

### 2.2 Module placement

The primitive lives in `crates/gf2-kernels-simd/src/x86/fp_small.rs` at the
position it already occupies. Moving it to a new `barrett.rs` module would
add an extra translation unit for one function and break the existing
`super::fp_small::barrett_reduce_lane32` references in design docs
(fc182ed5, 68cdf4c8) and code (`fp_small_panel.rs`, `fp_small_f32.rs`)
without adding clarity. The doc-comment block above the function will be
expanded to mark it as the Phase-2 SSOT and to list every consumer.

## 3. Call sites

After the Phase 2 refactor:

| Consumer | File | Site | Source domain |
|---|---|---|---|
| 1. SpMM row reducer | `x86/fp_small.rs::fp_small_spmm_row` | lines 730-731 (already) | sparse-times-dense small-prime kernel |
| 2. Route-A f32 cascade output | `x86/fp_small_f32.rs::store_and_reduce_tile_route_a::write_row` | lines 917-919 (already, via local wrapper) | small-prime float-modular reduction |
| 3. Route-C integer panel output | `x86/fp_small_panel.rs::fp_small_panel_gemm` reduce-and-pack block | lines 471-482 (already) | small-prime integer panel reducer |
| 4. Medium-prime u16 mul | `x86/fp_medium.rs::fp_medium_batch_mul16` | lines 150-151 (NEW after refactor) | medium-prime element-wise multiply |

Site #4 is the **second call site beyond the GF(251) selected route** that
the SC#1 criterion requires. Pre-Phase-2 it used a duplicate
implementation; Phase 2 consolidates it onto the SSOT.

The post-Phase-2 `barrett_reduce_lane32_local` thin wrapper in
`fp_small_f32.rs` is removed; the f32-cascade call site invokes the SSOT
directly. The wrapper existed only to paper over the now-removed
`p_vec64` parameter.

## 4. Dispatch ordering invariant

Phase 2 preserves the existing exact-prime dispatch ordering in
`crates/gf2-core/src/gfp/simd_ops.rs::SimdVecOps for Fp<P>` (per SC#2):

```text
if P == 65537                  → fp65537_try_*_vec
if P == 2^31 - 1   (mul only)  → fpm31_try_mul_vec
if P <= 251                    → fp_small_try_*_vec
if 252 <= P < 65536            → fp_medium_try_*_vec
otherwise                      → fp_generic_try_*_vec
```

The dispatch source ordering (lines 190-243 of `simd_ops.rs`) is **not
touched** by Phase 2. The refactor lives entirely inside the
`gf2-kernels-simd` callee kernels and is invisible to the gf2-core
dispatcher.

The existing in-source comment at `simd_ops.rs:166-189` already states the
"exact-prime tests MUST remain ABOVE the generic fallback" invariant; it
will not need amendment. The Phase-2 evidence doc will cite the line
numbers as proof of preservation.

## 5. Tests

### 5.1 Dispatch-trace test (SC#2)

The existing test
`crates/gf2-core/src/gfp/simd_ops.rs::tests::specialized_primes_do_not_use_generic_montgomery_path`
(line 2959) already asserts that `Fp<65537>`, `Mersenne31`, `Fp<65521>`,
`Fp<257>`, and `Fp<32749>` each route to their specialised SIMD kernel
rather than the generic Montgomery fallback (via the `fp_generic_enabled::<P>`
predicate). That test is the **dispatch-trace test** Phase 2 requires; no
new dispatch test is needed.

The Phase 2 evidence doc will cite it by name and explain why the
structural source-order check in `gfp/simd_ops.rs:190-243` plus the
runtime assertion in that test together prove the invariant.

### 5.2 Multi-prime sweep proptest (SC#3)

A new proptest file
`crates/gf2-core/tests/phase2_prime_sweep_proptests.rs` will sweep the
10 primes named in SC#3 — GF(7), GF(31), GF(127), GF(241), GF(251),
GF(257), GF(32749), GF(65521), Fp<65537>, Mersenne31 — at boundary
lengths `{0, 1, 15, 16, 17, 63, 64, 65}` (excluding `n=0`, which is a
trivial no-op).

For each (prime, n) the proptest asserts
`production_dispatch(a, b) == scalar_reference(a, b)` on `gemm(A, B)` of
shape (n × n). Inputs are drawn via `bench_seed::fp_matrix_from_seed::<P>`
to share the deterministic harness used by every other dispatch proptest.

The new proptest is structurally a sibling of the existing
`proptest_production_dispatch_prime_sweep_boundary_n` in
`route_a_gf251_production_dispatch_proptests.rs` (which covers GF(7), GF(31),
GF(127), GF(241), GF(251)); the new file extends coverage to the medium
primes, Fp<65537>, and Mersenne31 to give the Phase-2 reduction-primitive
refactor a per-prime bit-exact correctness gate that hits the
`fp_medium::fp_medium_batch_mul16` (and therefore the new SSOT call site)
on every medium-prime square GEMM.

`prop_oneof![Just(0usize), Just(1), ...]` form is required per the
52cce970 R1 trap.

### 5.3 Existing kernel-level tests (preservation)

All existing `#[cfg(test)] mod tests` blocks in
`x86/fp_small.rs`, `x86/fp_medium.rs`, `x86/fp_small_panel.rs`, and
`x86/fp_small_f32.rs` already assert per-kernel correctness via scalar
oracles. They remain untouched and must continue to pass after the
refactor — this is the unit-level proof of bit-exactness for the
extracted primitive.

## 6. Non-regression benchmark (SC#4)

5-trial CCX1-pinned (`taskset -c 6-11 nice -n -5`) measurement of the
"currently-PASSing" cells from the post-41096af5 scorecard:
primes `{7, 31, 127, 251}` at `n ∈ {256, 1024}`, plus GF(251)/n=64 for the
control. Driver mirrors `run_41096af5_post_wire_in_bench.sh` byte-for-byte
on the harness (filter pattern, quiet-host check, snapshotting, aggregation).

Acceptance: every cell within ±5% of the
`dev/bench_results/2026-05-25-41096af5-post-wire-in-aggregate.csv`
baseline. **In particular GF(251)/n=1024 must still ratio ≥ 0.667 against
the fflas-ffpack 138.32 Gop/s reference**, the contract from the upstream
615db3b9 / cc5de315 thread.

Output to `dev/bench_results/2026-05-25-e8a0c47a-post-refactor*.csv` and
the evidence doc at `dev/bench_results/2026-05-25-e8a0c47a-phase2-generalization.md`.

## 7. Coordination with 27bb2f75 (SC#5)

`27bb2f75` (closed 2026-05-24) is the small-n overhead-reduction issue
that introduced thread-local pack scratches and Montgomery REDC byte
tables, closing the n ≤ 128 per-call-overhead gap for the small-prime
SIMD path. Its deliverables sit in the `pack`/`unpack` and
`fp_small_pack` layers of `gfp/simd_ops.rs`.

Phase 2's Barrett-reduction consolidation is **a parallel, independent
change**:

- 27bb2f75 touched the pack/unpack scratch layer; Phase 2 touches the
  inner-kernel reduction layer.
- 27bb2f75 affects every small-prime SIMD entry point; Phase 2's
  consolidation only de-duplicates the 32-bit Barrett step, leaving
  pack/unpack untouched.
- No build-order dependency exists in either direction: Phase 2 compiles
  cleanly on top of HEAD-after-27bb2f75 (current state) and would compile
  cleanly without 27bb2f75 had it not landed.

No coordination beyond preserving the existing 27bb2f75 pack-scratch
machinery is required. Phase 2 ships independent of any 27bb2f75 work.

## 8. N_THRESH_PRIME status (SC#6)

The plan's Phase 2 directive on the threshold is explicit:

> Revisit `N_THRESH_PRIME` only with new data; Candidate C currently wins on
> Zen 3 for p ≤ 251.

Phase 2 does **not** change `N_THRESH_PRIME` (currently `251`, set by
41096af5). No new measurement post-41096af5 contradicts the existing
choice; Phase 2's only measurement is the non-regression bench, which by
construction confirms the existing threshold's performance and does not
generate fresh data on a candidate alternative.

The evidence doc will record `N_THRESH_PRIME = 251 (UNCHANGED)` with the
clause above as the cited justification.

## 9. License / provenance

Phase 2 is pure SSOT consolidation of code already in-tree (gf2-authored,
MIT). No fflas-ffpack source is copied, translated, linked, or used as a
template. The Granlund-Möller Barrett-reduction algorithm is textbook
arithmetic (1994 paper); the AVX2 instruction sequence was authored in
the issue-3a37e0f6 SpMM kernel and the issue-9e12659b medium-prime
kernel, both gf2-original.

## 10. Out of scope

Per the issue's "Out of scope" clause:

- No medium-prime cleanup beyond the SSOT refactor.
- No extension-field GEMM (Phase 4 — 873cbec1).
- No sparse-elim / pluq / echelon / invert / solve.

The medium-prime SSOT switch is **not** a medium-prime kernel cleanup —
it's a de-duplication of a shared building block. The medium-prime
kernel's behavior, performance characteristics, and dispatch policy are
unchanged.
