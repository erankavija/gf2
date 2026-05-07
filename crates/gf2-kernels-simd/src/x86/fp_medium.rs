//! AVX2 batch kernels for medium primes `Fp<P>` with `P < 2^16`.
//!
//! This module targets the `word-fits-in-u16` family of prime fields,
//! whose canonical residues fit in a single 16-bit lane. The reference
//! prime is `P = 65521`, the largest prime below `2^16`; the kernels
//! also accept any odd prime `P ∈ (251, 65535]` (the dispatch upper
//! bound enforced by `gf2-core::gfp::simd_ops`). Primes `P ≤ 251` are
//! served by the dedicated 8-bit small-prime kernel built in sibling
//! issue `662f7a15`; primes `P ≥ 65536` are served by the generic
//! 64-bit Montgomery kernel in `fp_generic.rs`.
//!
//! # Input contract per kernel
//!
//! All kernels accept u16 lanes in `[0, P) ⊆ [0, 2^16)`. The kernels
//! differ in how they interpret those lanes:
//!
//! * **`fp_medium_batch_add` / `fp_medium_batch_sub`** — accept any
//!   in-range u16, **canonical residue or Montgomery raw storage**. The
//!   modular arithmetic is identical for both interpretations because
//!   addition and subtraction are linear in the Montgomery domain
//!   (`aR + bR = (a+b)R mod P`). The caller in
//!   `gf2-core/src/gfp/simd_ops.rs::fp_medium_try_add_vec` exploits this
//!   to feed Montgomery raw storage via `fp_medium_pack_raw` (a pure
//!   `u64 → u16` truncation, no REDC), which is the throughput win.
//! * **`fp_medium_batch_mul`** — requires **canonical** residues. The
//!   per-cell output is written back in the input domain with no
//!   post-correction, so feeding `aR, bR` would silently produce
//!   `abR² mod P` instead of `ab mod P`. The mul caller
//!   (`fp_medium_try_mul_vec` in `gf2-core/src/gfp/simd_ops.rs`) packs
//!   canonical via `fp_medium_pack_canonical` accordingly.
//! * **`fp_medium_batch_dot`** — domain-agnostic at the kernel level:
//!   the kernel computes the unsigned 16-bit MAC sum
//!   `(Σ a[i] * b[i]) mod P` whether the lanes are canonical or
//!   Montgomery storage; only the *meaning* of the result differs by an
//!   `R²` factor. Standalone callers feeding canonical lanes get the
//!   canonical dot product. The GEMM caller in
//!   `gf2-core/src/gfp/simd_ops.rs::fp_medium_try_dot_packed` (with
//!   operands packed by `fp_medium_try_pack_u16`)
//!   feeds **Montgomery raw storage** truncated `u64 → u16`; the
//!   kernel returns `R² · Σ aᵢbᵢ mod P`, and the caller then applies one
//!   Montgomery REDC to recover the canonical Montgomery storage of the
//!   dot product. The pack-as-Montgomery path is the GEMM throughput
//!   win — it skips a per-cell `Fp::value()` call (one REDC per lane)
//!   in favour of a pure `u64 → u16` truncation.
//!
//! # Algorithm
//!
//! Reduction is via Barrett's algorithm with a compile-time-derived
//! magic constant `m = floor(2^32 / P)`:
//!
//! ```text
//!   q = (x * m) >> 32        // approximation of floor(x / P)
//!   r = x - q * P            // r ∈ [0, 2P) for x ∈ [0, P²)
//!   if r >= P { r -= P }     // single conditional subtract canonicalises
//! ```
//!
//! Multiplication uses `_mm256_unpacklo_epi16`/`unpackhi_epi16` to widen
//! 16-bit operands into 32-bit lanes, then `_mm256_mullo_epi32` for the
//! 16×16→32 product (exact, since `(P-1)² < 2^32` for `P ≤ 65535`),
//! followed by Barrett. The inner reduction stays entirely in 32-bit
//! lanes so we get **8 reduced u32 results per 256-bit half-vector**,
//! repacked to u16 via `_mm256_packus_epi32`.
//!
//! Dot products use `_mm256_madd_epi16` (multiply pairs of 16-bit lanes,
//! accumulate adjacent pairs into 32-bit lanes — one fused MAC per
//! lane-pair). The 32-bit lane outputs are widened to 64-bit (via
//! `_mm256_unpacklo_epi32`/`unpackhi_epi32`) and accumulated, giving
//! `k_max = 2^64 / (P-1)² ≈ 4.3 × 10^9` for `P = 65521` — far larger
//! than any realistic panel size.
//!
//! # Safety
//!
//! All public functions here are `unsafe` — callers must ensure AVX2 is
//! available at runtime. The safe, dispatched entry points live in the
//! parent `fp_medium.rs` module via the `MediumPrimeFns` table returned
//! by `crate::fp_medium::detect`.

#![allow(clippy::missing_safety_doc)]

use core::arch::x86_64::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// 32-bit-lane Barrett reduction: `x mod P` for `x ∈ [0, P²) ⊆ [0, 2^32)`.
///
/// Returns reduced values still in 32-bit lanes (the caller repacks to
/// u16 with `_mm256_packus_epi32`).
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn barrett_reduce_u32x8(x: __m256i, p: __m256i, m: __m256i) -> __m256i {
    // q = (x * m) >> 32. We need a u32×u32→u64 mul to recover the high 32
    // bits. `_mm256_mul_epu32` operates on the even u32 lanes of each 64-bit
    // lane; we shift and rerun for the odd lanes, then OR the high halves
    // back together at the right positions.
    let mask32 = _mm256_set1_epi64x(0xFFFF_FFFF);

    // Even u32 lanes: low 32 bits of each 64-bit lane.
    let x_even = _mm256_and_si256(x, mask32);
    let m_even = _mm256_and_si256(m, mask32);
    // Four 64-bit products. Take the high 32 bits of each → q for even lanes.
    let prod_even = _mm256_mul_epu32(x_even, m_even);
    let q_even = _mm256_srli_epi64(prod_even, 32);

    // Odd u32 lanes: shift each 64-bit lane right by 32.
    let x_odd = _mm256_srli_epi64(x, 32);
    let m_odd = _mm256_srli_epi64(m, 32);
    let prod_odd = _mm256_mul_epu32(x_odd, m_odd);
    let q_odd = _mm256_srli_epi64(prod_odd, 32);

    // Reassemble: even lanes go in u32 lanes {0, 2, 4, 6}; odd lanes go in
    // {1, 3, 5, 7}. Both `q_even` and `q_odd` currently sit in the low half
    // of each 64-bit lane.
    let q = _mm256_or_si256(q_even, _mm256_slli_epi64(q_odd, 32));

    // r = x - q * P (low 32 bits suffice; q * P ≤ x < 2^32).
    let qp = _mm256_mullo_epi32(q, p);
    let r = _mm256_sub_epi32(x, qp);

    // Conditional subtract of P when r >= P. Branchless:
    // `min_epu32(r, r - P)` selects `r - P` when r >= P (since `r - P` is
    // smaller as a u32) and `r` otherwise (since `r - P` underflows to a
    // very large u32).
    _mm256_min_epu32(r, _mm256_sub_epi32(r, p))
}

/// Lane-wise modular multiplication for 16 u16 values per 256-bit vector.
///
/// Inputs are 16 canonical u16 values per vector (`a, b < P`); output is
/// 16 canonical u16 values. Internally widens to 32-bit, multiplies, and
/// Barrett-reduces.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn fp_medium_batch_mul16(a: __m256i, b: __m256i, p32: __m256i, m32: __m256i) -> __m256i {
    // Unpack 16-bit lanes into 32-bit lanes. `unpacklo` interleaves the low
    // 128-bit half of each 256-bit input; `unpackhi` does the high half.
    // After unpacking, lanes are zero-extended (u16 → u32).
    let zero = _mm256_setzero_si256();
    let a_lo = _mm256_unpacklo_epi16(a, zero);
    let a_hi = _mm256_unpackhi_epi16(a, zero);
    let b_lo = _mm256_unpacklo_epi16(b, zero);
    let b_hi = _mm256_unpackhi_epi16(b, zero);

    // 32-bit lane multiply: (P-1)² < 2^32 so mullo is exact.
    let prod_lo = _mm256_mullo_epi32(a_lo, b_lo);
    let prod_hi = _mm256_mullo_epi32(a_hi, b_hi);

    // Barrett-reduce each 32-bit lane.
    let red_lo = barrett_reduce_u32x8(prod_lo, p32, m32);
    let red_hi = barrett_reduce_u32x8(prod_hi, p32, m32);

    // Repack 32-bit results to 16-bit. `packus_epi32` saturates negative
    // inputs to zero — but our reduced values are already in `[0, P)`, so
    // saturation never engages.
    //
    // `packus_epi32(lo, hi)` interleaves the 128-bit halves:
    //   result lanes 0..3   ← lo lanes 0..3  (lo's low half)
    //   result lanes 4..7   ← hi lanes 0..3  (hi's low half)
    //   result lanes 8..11  ← lo lanes 4..7  (lo's high half)
    //   result lanes 12..15 ← hi lanes 4..7  (hi's high half)
    //
    // This reverses the unpack convention used above (which interleaves
    // low/high halves the same way), so a single packus restores the
    // original lane order.
    _mm256_packus_epi32(red_lo, red_hi)
}

/// Lane-wise modular addition for 16 u16 lanes.
///
/// Sum `s = a + b` fits in 17 bits (`P ≤ 2^16 - 15`, so `s ≤ 2P - 2 <
/// 2^17`); we use a 16-bit add with branchless cond-sub of `P`.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn fp_medium_add16(a: __m256i, b: __m256i, p: __m256i) -> __m256i {
    // 16-bit add wraps modulo 2^16. Since `a + b < 2P < 2^17`, the wrap
    // happens iff `a + b ≥ 2^16`, in which case `(a+b) mod 2^16 = a+b-2^16`
    // — and we need to add `P - 2^16` (which is negative). Easier: do the
    // add in 16-bit with saturation considerations bypassed by computing in
    // 32-bit lanes for the cond-sub.
    //
    // Simpler approach: widen to 32 bits, add, conditional-sub P, narrow.
    let zero = _mm256_setzero_si256();
    let a_lo = _mm256_unpacklo_epi16(a, zero);
    let a_hi = _mm256_unpackhi_epi16(a, zero);
    let b_lo = _mm256_unpacklo_epi16(b, zero);
    let b_hi = _mm256_unpackhi_epi16(b, zero);

    let s_lo = _mm256_add_epi32(a_lo, b_lo);
    let s_hi = _mm256_add_epi32(a_hi, b_hi);

    let r_lo = _mm256_min_epu32(s_lo, _mm256_sub_epi32(s_lo, p));
    let r_hi = _mm256_min_epu32(s_hi, _mm256_sub_epi32(s_hi, p));

    _mm256_packus_epi32(r_lo, r_hi)
}

/// Lane-wise modular subtraction for 16 u16 lanes.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn fp_medium_sub16(a: __m256i, b: __m256i, p: __m256i) -> __m256i {
    let zero = _mm256_setzero_si256();
    let a_lo = _mm256_unpacklo_epi16(a, zero);
    let a_hi = _mm256_unpackhi_epi16(a, zero);
    let b_lo = _mm256_unpacklo_epi16(b, zero);
    let b_hi = _mm256_unpackhi_epi16(b, zero);

    // Compute (a + P) - b. Since `a, b < P`, `a + P < 2P < 2^17` so the
    // 32-bit add is exact, and `(a + P) - b ∈ [1, 2P - 1]`. Then a single
    // branchless cond-sub of P canonicalises.
    let ap_lo = _mm256_add_epi32(a_lo, p);
    let ap_hi = _mm256_add_epi32(a_hi, p);
    let d_lo = _mm256_sub_epi32(ap_lo, b_lo);
    let d_hi = _mm256_sub_epi32(ap_hi, b_hi);

    let r_lo = _mm256_min_epu32(d_lo, _mm256_sub_epi32(d_lo, p));
    let r_hi = _mm256_min_epu32(d_hi, _mm256_sub_epi32(d_hi, p));

    _mm256_packus_epi32(r_lo, r_hi)
}

// ---------------------------------------------------------------------------
// Public batch entry points
// ---------------------------------------------------------------------------

/// Batch lane-wise multiplication for `Fp<P>` with `P < 2^16`.
///
/// Computes `out[i] = (a[i] * b[i]) mod P` for all `i`, using 16-lane
/// AVX2 vectorisation with Barrett reduction.
///
/// # Arguments
///
/// * `a`, `b` — input slices of **canonical** residues in `[0, P)`; same
///   length. Unlike the add/sub kernels and unlike `fp_medium_batch_dot`,
///   the *per-cell* `batch_mul` writes back canonical residues, so the
///   caller must pre-pack canonical (no post-REDC step). Modular
///   multiplication is not linear in the Montgomery domain (`aR · bR mod
///   P = abR² mod P`, not `abR mod P`), so feeding Montgomery raw
///   storage would silently produce wrong-domain output without any
///   subsequent REDC fix-up. The `gf2-core` caller
///   `fp_medium_try_mul_vec` (in `crates/gf2-core/src/gfp/simd_ops.rs`)
///   packs canonical via `fp_medium_pack_canonical` accordingly.
/// * `p` — the prime modulus; must be in `(1, 2^16)`.
/// * `barrett_m` — `floor(2^32 / p)`, the Barrett magic constant.
/// * `out` — output slice of canonical results in `[0, P)` (same length).
///
/// # Safety
///
/// Caller must ensure AVX2 is available at runtime, all input values are
/// `< p`, and `barrett_m == floor(2^32 / p)`. Behaviour is undefined
/// otherwise. Inputs in Montgomery raw storage are *not* an unsoundness
/// hazard but produce a wrong-domain result — see the module-level
/// "Input contract per kernel" section.
///
/// # Panics
///
/// Panics if slice lengths differ.
///
/// # Complexity
///
/// O(n) with a 16-u16-lane vectorisation factor.
#[target_feature(enable = "avx2")]
pub unsafe fn fp_medium_batch_mul(a: &[u16], b: &[u16], p: u16, barrett_m: u32, out: &mut [u16]) {
    assert_eq!(a.len(), b.len(), "fp_medium_batch_mul: length mismatch");
    assert_eq!(a.len(), out.len(), "fp_medium_batch_mul: output length");

    let n = a.len();
    let nvec = n / 16;

    let p32 = _mm256_set1_epi32(p as i32);
    let m32 = _mm256_set1_epi32(barrett_m as i32);

    let a_ptr = a.as_ptr() as *const __m256i;
    let b_ptr = b.as_ptr() as *const __m256i;
    let o_ptr = out.as_mut_ptr() as *mut __m256i;

    for i in 0..nvec {
        let av = _mm256_loadu_si256(a_ptr.add(i));
        let bv = _mm256_loadu_si256(b_ptr.add(i));
        let rv = fp_medium_batch_mul16(av, bv, p32, m32);
        _mm256_storeu_si256(o_ptr.add(i), rv);
    }

    // Scalar tail.
    let tail_start = nvec * 16;
    for i in tail_start..n {
        let prod = (*a.get_unchecked(i) as u32) * (*b.get_unchecked(i) as u32);
        *out.get_unchecked_mut(i) = (prod % p as u32) as u16;
    }
}

/// Batch lane-wise addition for `Fp<P>` with `P < 2^16`.
///
/// # Arguments
///
/// * `a`, `b` — input slices of u16 lanes in `[0, P)`. May be canonical
///   residues **or** Montgomery raw storage; the result is in the same
///   domain as the inputs (addition is linear, so
///   `aR + bR = (a+b)R mod P`).
/// * `p` — the prime modulus; must be in `(1, 2^16)`.
/// * `out` — output slice (same length).
///
/// # Safety
///
/// Caller must ensure AVX2 is available at runtime and all input values
/// are `< p`. Behaviour is undefined otherwise.
#[target_feature(enable = "avx2")]
pub unsafe fn fp_medium_batch_add(a: &[u16], b: &[u16], p: u16, out: &mut [u16]) {
    assert_eq!(a.len(), b.len(), "fp_medium_batch_add: length mismatch");
    assert_eq!(a.len(), out.len(), "fp_medium_batch_add: output length");

    let n = a.len();
    let nvec = n / 16;

    let p_vec = _mm256_set1_epi32(p as i32);

    let a_ptr = a.as_ptr() as *const __m256i;
    let b_ptr = b.as_ptr() as *const __m256i;
    let o_ptr = out.as_mut_ptr() as *mut __m256i;

    for i in 0..nvec {
        let av = _mm256_loadu_si256(a_ptr.add(i));
        let bv = _mm256_loadu_si256(b_ptr.add(i));
        let rv = fp_medium_add16(av, bv, p_vec);
        _mm256_storeu_si256(o_ptr.add(i), rv);
    }

    let tail_start = nvec * 16;
    for i in tail_start..n {
        let s = *a.get_unchecked(i) as u32 + *b.get_unchecked(i) as u32;
        *out.get_unchecked_mut(i) = (if s >= p as u32 { s - p as u32 } else { s }) as u16;
    }
}

/// Batch lane-wise subtraction for `Fp<P>` with `P < 2^16`.
///
/// # Arguments
///
/// * `a`, `b` — input slices of u16 lanes in `[0, P)`. May be canonical
///   residues **or** Montgomery raw storage; the result is in the same
///   domain as the inputs (subtraction is linear, so
///   `aR - bR = (a-b)R mod P`).
/// * `p` — the prime modulus; must be in `(1, 2^16)`.
/// * `out` — output slice (same length).
///
/// # Safety
///
/// Caller must ensure AVX2 is available at runtime and all input values
/// are `< p`. Behaviour is undefined otherwise.
#[target_feature(enable = "avx2")]
pub unsafe fn fp_medium_batch_sub(a: &[u16], b: &[u16], p: u16, out: &mut [u16]) {
    assert_eq!(a.len(), b.len(), "fp_medium_batch_sub: length mismatch");
    assert_eq!(a.len(), out.len(), "fp_medium_batch_sub: output length");

    let n = a.len();
    let nvec = n / 16;

    let p_vec = _mm256_set1_epi32(p as i32);

    let a_ptr = a.as_ptr() as *const __m256i;
    let b_ptr = b.as_ptr() as *const __m256i;
    let o_ptr = out.as_mut_ptr() as *mut __m256i;

    for i in 0..nvec {
        let av = _mm256_loadu_si256(a_ptr.add(i));
        let bv = _mm256_loadu_si256(b_ptr.add(i));
        let rv = fp_medium_sub16(av, bv, p_vec);
        _mm256_storeu_si256(o_ptr.add(i), rv);
    }

    let tail_start = nvec * 16;
    for i in tail_start..n {
        let ai = *a.get_unchecked(i) as u32;
        let bi = *b.get_unchecked(i) as u32;
        let d = ai + p as u32 - bi;
        *out.get_unchecked_mut(i) = (if d >= p as u32 { d - p as u32 } else { d }) as u16;
    }
}

/// Batch dot product for `Fp<P>` with `P < 2^16`, returning the
/// reduced sum `(Σ a[i] * b[i]) mod p` in the same domain interpretation
/// as the inputs.
///
/// # Domain semantics
///
/// The kernel computes the unsigned 16-bit dot product
/// `(Σ a[i] * b[i]) mod p`. The result's *meaning* depends on the
/// caller's input domain:
///
/// * Standalone callers feeding **canonical** lanes (e.g. via
///   `fp_medium_pack_canonical`) get the canonical dot product
///   `(Σ aᵢbᵢ) mod p`.
/// * The GEMM caller (`gf2-core/src/gfp/simd_ops.rs::fp_medium_try_dot_packed`,
///   with operands packed by `fp_medium_try_pack_u16`) feeds
///   **Montgomery raw storage** `aR mod p` truncated to
///   u16. The kernel's output is then `(R² · Σ aᵢbᵢ) mod p`, and the
///   caller applies one Montgomery REDC to recover the canonical
///   Montgomery storage `R · Σ aᵢbᵢ mod p`. The kernel itself is
///   domain-agnostic — the same MAC primitive serves both interpretations
///   (see module-level "Input contract per kernel" section).
///
/// # Algorithm
///
/// Two implementation paths share a public entry point, dispatched on
/// `p`:
///
/// * **`p ≤ 32767`** (signed-`madd_epi16` path) — `_mm256_madd_epi16`
///   fuses two adjacent u16 × u16 products into a single signed-i32 lane
///   sum, giving 8 paired MACs per 256-bit vector iteration. Signed
///   interpretation is safe because canonical lanes are in `[0, p) ⊆
///   [0, 2^15)`, so signed and unsigned interpretations coincide. The
///   per-pair MAC bound is `2 · (p-1)² < 2^31`, so a u32 lane absorbs
///   `K_PANEL_PAIRS = floor(2^32 / (2 · (p-1)²))` pair-MACs before
///   needing to drain to u64. For GF(8191) this gives K ≈ 32 pairs per
///   panel = 4 vector chunks per panel; for GF(257) ≈ 32k pairs (no
///   draining ever required at the gemm cell sizes in scope).
/// * **`p > 32767`** (mulhi+mullo path) — `_mm256_madd_epi16` would
///   misinterpret canonical lanes ≥ 2^15 as negative. The fallback uses
///   `_mm256_mullo_epi16` + `_mm256_mulhi_epu16` to reconstruct the full
///   u32 product, then widens to u64 every iteration (no panel
///   accumulation is possible — a single full u32 product can already
///   approach 2^32 for P near 2^16). This is the original `9e12659b`
///   implementation; performance is the same for the reference prime
///   GF(65521).
///
/// # Arguments
///
/// * `a`, `b` — input slices of u16 lanes in `[0, p)`; same length. The
///   kernel computes `Σ a[i] * b[i] mod p` regardless of whether the
///   lanes are canonical residues or Montgomery raw storage; the result
///   *value* differs by an `R²` factor between the two domains, and the
///   GEMM caller applies a post-REDC to land in Montgomery storage. See
///   the module-level "Input contract per kernel" section.
/// * `p` — the prime modulus. Selects the algorithm path internally.
///
/// # Returns
///
/// The reduced dot product `(Σ a[i] * b[i]) mod p` in `[0, p)`.
///
/// # Safety
///
/// Caller must ensure AVX2 is available and all input values are `< p`.
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
///
/// # Complexity
///
/// O(n) with 16-u16-lane vectorisation. The fast path (`p ≤ 32767`)
/// runs at one `madd_epi16` per 16 inputs plus one u32 add per panel
/// stride; the fallback path runs four ops per 16 inputs (mullo+mulhi+
/// 2× unpack) plus four u64 adds per chunk.
#[target_feature(enable = "avx2")]
pub unsafe fn fp_medium_batch_dot(a: &[u16], b: &[u16], p: u16) -> u32 {
    assert_eq!(a.len(), b.len(), "fp_medium_batch_dot: length mismatch");

    if p <= 32_767 {
        fp_medium_batch_dot_madd(a, b, p)
    } else {
        fp_medium_batch_dot_mulhi(a, b, p)
    }
}

/// Fast path for `p ≤ 32767`: signed `_mm256_madd_epi16` with per-prime
/// panel-size accumulation in u32 lanes before draining to u64.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn fp_medium_batch_dot_madd(a: &[u16], b: &[u16], p: u16) -> u32 {
    debug_assert!(p > 0 && p <= 32_767);

    let n = a.len();
    let nvec = n / 16;

    let a_ptr = a.as_ptr() as *const __m256i;
    let b_ptr = b.as_ptr() as *const __m256i;

    // Each madd_epi16 chunk produces 8 i32 lanes, each holding the sum
    // of 2 unsigned products `a[2i]*b[2i] + a[2i+1]*b[2i+1]`. Per-lane
    // bound is `2 * (p-1)²`. The u32 panel capacity is therefore
    // `K_PANEL_CHUNKS = floor(2^32 / (16 * (p-1)²))` 16-u16 chunks
    // (each chunk contributes 2*(p-1)² per u32 lane × 8 lanes per chunk;
    // we only need to bound the per-lane sum, so the divisor is
    // `2 * (p-1)²` per chunk). Using a saturating-floor and clamping to
    // 1 below gives a safe per-prime panel size; for p=8191 this is 32,
    // for p=257 it overflows usize (treated as nvec). After each panel,
    // u32 lanes are widened to u64 and accumulated into a single
    // u64-lane vector accumulator that absorbs the full sweep.
    let pair_bound = 2u64 * (p as u64 - 1) * (p as u64 - 1);
    let panel_chunks: usize = match (1u64 << 32).checked_div(pair_bound) {
        // raw is "max chunks per u32 lane such that no overflow". One
        // chunk contributes one MAC pair per lane, so panel_chunks = raw.
        // Clamp to ≥ 1 (guaranteed when p > 1; pair_bound ≤ 2*32767² <
        // 2^31).
        Some(raw) => raw.max(1) as usize,
        None => usize::MAX,
    };

    let zero = _mm256_setzero_si256();
    let mut acc_u64 = _mm256_setzero_si256(); // 4 × u64 accumulators

    let mut chunk = 0usize;
    while chunk < nvec {
        let panel_end = (chunk + panel_chunks).min(nvec);
        let mut acc_u32 = _mm256_setzero_si256(); // 8 × u32 lane sums

        while chunk < panel_end {
            let av = _mm256_loadu_si256(a_ptr.add(chunk));
            let bv = _mm256_loadu_si256(b_ptr.add(chunk));
            // Each i32 lane = a[2i]*b[2i] + a[2i+1]*b[2i+1], unsigned-safe
            // because a, b < p ≤ 2^15 keeps both factors and the sum in
            // i32 positive range.
            let m = _mm256_madd_epi16(av, bv);
            acc_u32 = _mm256_add_epi32(acc_u32, m);
            chunk += 1;
        }

        // Drain u32 panel sum into the u64 accumulator. Widen 8 × u32
        // → 8 × u64 split across two vectors.
        let lo = _mm256_unpacklo_epi32(acc_u32, zero);
        let hi = _mm256_unpackhi_epi32(acc_u32, zero);
        acc_u64 = _mm256_add_epi64(acc_u64, lo);
        acc_u64 = _mm256_add_epi64(acc_u64, hi);
    }

    // Horizontal sum across the u64 accumulator's four lanes.
    let mut tmp = [0u64; 4];
    _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, acc_u64);
    let mut total: u64 = tmp[0]
        .wrapping_add(tmp[1])
        .wrapping_add(tmp[2])
        .wrapping_add(tmp[3]);

    // Scalar tail.
    let tail_start = nvec * 16;
    for i in tail_start..n {
        total = total.wrapping_add((*a.get_unchecked(i) as u64) * (*b.get_unchecked(i) as u64));
    }

    (total % p as u64) as u32
}

/// Fallback path for `p > 32767`: full u32 products via mullo + mulhi,
/// widened to u64 per chunk. No panel batching possible (a single u32
/// product can approach 2^32).
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn fp_medium_batch_dot_mulhi(a: &[u16], b: &[u16], p: u16) -> u32 {
    let n = a.len();
    let nvec = n / 16;

    let a_ptr = a.as_ptr() as *const __m256i;
    let b_ptr = b.as_ptr() as *const __m256i;

    // Two parallel u64-lane accumulators (widened from u32 mullo outputs).
    let mut acc_lo = _mm256_setzero_si256();
    let mut acc_hi = _mm256_setzero_si256();
    let zero = _mm256_setzero_si256();

    for i in 0..nvec {
        let av = _mm256_loadu_si256(a_ptr.add(i));
        let bv = _mm256_loadu_si256(b_ptr.add(i));

        // Compute the full 16-lane u16 × u16 → u32 product using the
        // mullo+mulhi pair. Both ops have 1-cycle throughput on Zen-3,
        // so the multiply step costs two µops vs the four needed by the
        // u16→u32 widen + `_mm256_mullo_epi32` path. `mullo_epi16` is
        // signed but the low 16 bits of a signed product equal the low
        // 16 bits of the unsigned product; `mulhi_epu16` returns the
        // unsigned high half. Re-interleaving via `unpack{lo,hi}_epi16`
        // reconstructs eight packed u32 products per 256-bit half.
        let prod_lo16 = _mm256_mullo_epi16(av, bv);
        let prod_hi16 = _mm256_mulhi_epu16(av, bv);
        let prod_full_lo = _mm256_unpacklo_epi16(prod_lo16, prod_hi16);
        let prod_full_hi = _mm256_unpackhi_epi16(prod_lo16, prod_hi16);

        // Widen 32-bit-lane products to 64-bit lanes (zero-extension).
        let p_lo_l = _mm256_unpacklo_epi32(prod_full_lo, zero);
        let p_lo_h = _mm256_unpackhi_epi32(prod_full_lo, zero);
        let p_hi_l = _mm256_unpacklo_epi32(prod_full_hi, zero);
        let p_hi_h = _mm256_unpackhi_epi32(prod_full_hi, zero);

        // Accumulate into the two parallel acc lanes.
        acc_lo = _mm256_add_epi64(acc_lo, _mm256_add_epi64(p_lo_l, p_hi_l));
        acc_hi = _mm256_add_epi64(acc_hi, _mm256_add_epi64(p_lo_h, p_hi_h));
    }

    // Horizontal sum of `acc_lo + acc_hi`. Each holds four u64 lanes; max
    // per lane is `(n/16) * (P-1)² ≈ n/16 * 2^32 ≈ 2^60` for `n = 2^32`.
    let acc = _mm256_add_epi64(acc_lo, acc_hi);

    let mut tmp = [0u64; 4];
    _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, acc);
    let mut total: u64 = tmp[0]
        .wrapping_add(tmp[1])
        .wrapping_add(tmp[2])
        .wrapping_add(tmp[3]);

    // Scalar tail.
    let tail_start = nvec * 16;
    for i in tail_start..n {
        total = total.wrapping_add((*a.get_unchecked(i) as u64) * (*b.get_unchecked(i) as u64));
    }

    // Final reduction. `total` fits in u64; for very long inputs the
    // accumulator never wraps (k_max ≈ 4.3e9 for P = 65521).
    (total % p as u64) as u32
}

/// Sparse-times-dense row kernel for medium-prime `Fp<P>` with
/// `P ∈ (251, 65535]`.
///
/// Writes `out[j] = (∑_h a_vals[h] * b[a_cols[h] * b_stride + j]) mod p`
/// for `j ∈ [0, n)`. The sparse left row is given as `(a_vals, a_cols)`
/// with canonical u16 lanes; `b` is a row-major dense u16 matrix with
/// row stride `b_stride`. `out` is the dense output row of length `n`.
///
/// The kernel iterates output blocks of 16 u16 lanes; for each block
/// it sweeps every non-zero of the sparse row, broadcasts `a_vals[h]`,
/// computes the full 16×u32 product via `mullo_epi16 + mulhi_epu16`,
/// widens each u32 lane to u64, and accumulates into four u64-lane
/// vectors. After the sparse-row sweep, each u64 lane is reduced
/// modulo `p` scalarly and written back to the output as a u16. The
/// u64 accumulator capacity `2^64 / (p-1)² ≈ 4.3 × 10^9` (at `p =
/// 65521`) is orders of magnitude larger than any realistic
/// nnz-per-row, so no chunked reduction is needed.
///
/// # Safety
///
/// Caller must ensure AVX2 is available, `p` is an odd prime in
/// `(251, 65535]`, every input lane is canonical (`< p`), and:
/// - `a_vals.len() == a_cols.len()`,
/// - every `a_cols[h] * b_stride + n <= b.len()`,
/// - `out.len() == n`.
///
/// # Panics
///
/// Panics if `a_vals.len() != a_cols.len()` or `out.len() != n`.
#[target_feature(enable = "avx2")]
pub unsafe fn fp_medium_spmm_row(
    a_vals: &[u16],
    a_cols: &[usize],
    b: &[u16],
    b_stride: usize,
    n: usize,
    p: u16,
    out: &mut [u16],
) {
    assert_eq!(
        a_vals.len(),
        a_cols.len(),
        "fp_medium_spmm_row: a_vals/a_cols length mismatch"
    );
    assert_eq!(out.len(), n, "fp_medium_spmm_row: out.len() != n");

    let nnz = a_vals.len();
    let p_u64 = p as u64;
    let zero = _mm256_setzero_si256();

    let mut j = 0;
    while j + 16 <= n {
        // 16 u32 products → widened to 16 u64 lanes split into 4 ymm.
        let mut acc0 = _mm256_setzero_si256(); // u64 lanes 0..3 (B-cols j..j+3)
        let mut acc1 = _mm256_setzero_si256(); // u64 lanes 4..7 (B-cols j+4..j+7)
        let mut acc2 = _mm256_setzero_si256(); // u64 lanes 8..11
        let mut acc3 = _mm256_setzero_si256(); // u64 lanes 12..15
        for h in 0..nnz {
            let a_h = *a_vals.get_unchecked(h);
            let col = *a_cols.get_unchecked(h);
            // Load 16 u16 lanes from B[col, j..j+16] (32 bytes).
            let b_row_ptr = b.as_ptr().add(col * b_stride + j) as *const __m256i;
            let bv = _mm256_loadu_si256(b_row_ptr);
            // Broadcast a_h to all 16 u16 lanes.
            let av = _mm256_set1_epi16(a_h as i16);
            // 16-lane u16 × u16 → u32 product via mullo + mulhi.
            // unpack{lo,hi}_epi16 reconstructs eight u32 products per
            // 256-bit half.
            let prod_lo16 = _mm256_mullo_epi16(av, bv);
            let prod_hi16 = _mm256_mulhi_epu16(av, bv);
            let prod_full_lo = _mm256_unpacklo_epi16(prod_lo16, prod_hi16);
            let prod_full_hi = _mm256_unpackhi_epi16(prod_lo16, prod_hi16);
            // Widen 32-bit-lane products → 64-bit lanes (zero-extend).
            // unpacklo/unpackhi_epi32 are in-lane (per 128-bit half).
            //
            // Lane mapping (per 128-bit half of bv):
            //   bv low half  = lanes 0..7 of B[col, j..j+8]
            //   bv high half = lanes 8..15 of B[col, j+8..j+16]
            //   prod_full_lo low  = u32 lanes [0,1,2,3]      (B-cols j+0..j+3)
            //   prod_full_lo high = u32 lanes [8,9,10,11]    (B-cols j+8..j+11)
            //   prod_full_hi low  = u32 lanes [4,5,6,7]      (B-cols j+4..j+7)
            //   prod_full_hi high = u32 lanes [12,13,14,15]  (B-cols j+12..j+15)
            let p_lo_l = _mm256_unpacklo_epi32(prod_full_lo, zero);
            let p_lo_h = _mm256_unpackhi_epi32(prod_full_lo, zero);
            let p_hi_l = _mm256_unpacklo_epi32(prod_full_hi, zero);
            let p_hi_h = _mm256_unpackhi_epi32(prod_full_hi, zero);
            // Map each widened u64 vector to the right output cells.
            // Each unpack-{lo,hi}_epi32 is per-128-bit-half:
            //   p_lo_l low  = u64 lanes [B-col j+0, B-col j+1]
            //   p_lo_l high = u64 lanes [B-col j+8, B-col j+9]
            //   p_lo_h low  = u64 lanes [B-col j+2, B-col j+3]
            //   p_lo_h high = u64 lanes [B-col j+10, B-col j+11]
            //   p_hi_l low  = u64 lanes [B-col j+4, B-col j+5]
            //   p_hi_l high = u64 lanes [B-col j+12, B-col j+13]
            //   p_hi_h low  = u64 lanes [B-col j+6, B-col j+7]
            //   p_hi_h high = u64 lanes [B-col j+14, B-col j+15]
            //
            // Group into 4 accumulators of 4 u64 lanes each so each
            // accumulator vector covers 4 contiguous output cells:
            //   acc0 → B-cols j+0..j+3 from { p_lo_l_lo (j+0,j+1) ; p_lo_h_lo (j+2,j+3) }
            //   acc1 → B-cols j+4..j+7 from { p_hi_l_lo (j+4,j+5) ; p_hi_h_lo (j+6,j+7) }
            //   acc2 → B-cols j+8..j+11 from { p_lo_l_hi ; p_lo_h_hi }
            //   acc3 → B-cols j+12..j+15 from { p_hi_l_hi ; p_hi_h_hi }
            //
            // Compose: take the low 128 of one and the low 128 of
            // another into a single ymm using inserti128.
            let lo01 = _mm256_castsi256_si128(p_lo_l);
            let lo23 = _mm256_castsi256_si128(p_lo_h);
            let lo45 = _mm256_castsi256_si128(p_hi_l);
            let lo67 = _mm256_castsi256_si128(p_hi_h);
            let hi89 = _mm256_extracti128_si256::<1>(p_lo_l);
            let hi1011 = _mm256_extracti128_si256::<1>(p_lo_h);
            let hi1213 = _mm256_extracti128_si256::<1>(p_hi_l);
            let hi1415 = _mm256_extracti128_si256::<1>(p_hi_h);
            let v0 = _mm256_inserti128_si256::<1>(_mm256_castsi128_si256(lo01), lo23);
            let v1 = _mm256_inserti128_si256::<1>(_mm256_castsi128_si256(lo45), lo67);
            let v2 = _mm256_inserti128_si256::<1>(_mm256_castsi128_si256(hi89), hi1011);
            let v3 = _mm256_inserti128_si256::<1>(_mm256_castsi128_si256(hi1213), hi1415);
            acc0 = _mm256_add_epi64(acc0, v0);
            acc1 = _mm256_add_epi64(acc1, v1);
            acc2 = _mm256_add_epi64(acc2, v2);
            acc3 = _mm256_add_epi64(acc3, v3);
        }
        // Reduce each u64 lane mod p scalarly (4 u64 lanes per
        // accumulator vector). Stores 16 reduced lanes back to `out`.
        let mut tmp = [0u64; 4];
        for (acc, base) in [acc0, acc1, acc2, acc3]
            .iter()
            .enumerate()
            .map(|(i, v)| (v, j + 4 * i))
        {
            _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, *acc);
            for (k, &t) in tmp.iter().enumerate() {
                *out.get_unchecked_mut(base + k) = (t % p_u64) as u16;
            }
        }
        j += 16;
    }
    // Scalar tail for j ∈ [j..n).
    while j < n {
        let mut total: u64 = 0;
        for h in 0..nnz {
            let a_h = *a_vals.get_unchecked(h) as u64;
            let col = *a_cols.get_unchecked(h);
            let b_kj = *b.get_unchecked(col * b_stride + j) as u64;
            total = total.wrapping_add(a_h * b_kj);
        }
        *out.get_unchecked_mut(j) = (total % p_u64) as u16;
        j += 1;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use crate::fp_medium::barrett_m32;

    const P_65521: u16 = 65521;
    const M_65521: u32 = barrett_m32(P_65521);

    fn scalar_mul(a: u16, b: u16, p: u16) -> u16 {
        ((a as u32 * b as u32) % p as u32) as u16
    }

    fn scalar_add(a: u16, b: u16, p: u16) -> u16 {
        let s = a as u32 + b as u32;
        (if s >= p as u32 { s - p as u32 } else { s }) as u16
    }

    fn scalar_sub(a: u16, b: u16, p: u16) -> u16 {
        let d = a as u32 + p as u32 - b as u32;
        (if d >= p as u32 { d - p as u32 } else { d }) as u16
    }

    #[test]
    fn batch_mul_matches_scalar_65521() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        let a: Vec<u16> = (0..50u16).map(|i| (i * 137) % P_65521).collect();
        let b: Vec<u16> = (0..50u16).map(|i| (i * 211 + 17) % P_65521).collect();
        let mut out = vec![0u16; 50];
        unsafe { fp_medium_batch_mul(&a, &b, P_65521, M_65521, &mut out) };
        for i in 0..50 {
            assert_eq!(out[i], scalar_mul(a[i], b[i], P_65521), "i={i}");
        }
    }

    #[test]
    fn batch_mul_boundary_values_65521() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        // 65520 = P - 1 stresses the (P-1)² ≈ 2^32 overflow boundary.
        let a = vec![
            0u16, 1, 65520, 32760, 1, 65520, 0, 65519, 65520, 32761, 1, 100, 0, 65520, 32760, 1,
        ];
        let b = vec![
            65520u16, 0, 65520, 2, 32760, 1, 0, 3, 65519, 32761, 65520, 100, 0, 65520, 2, 32760,
        ];
        let mut out = vec![0u16; 16];
        unsafe { fp_medium_batch_mul(&a, &b, P_65521, M_65521, &mut out) };
        for i in 0..16 {
            assert_eq!(out[i], scalar_mul(a[i], b[i], P_65521), "i={i}");
        }
    }

    #[test]
    fn batch_mul_with_tail_65521() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        for &len in &[0usize, 1, 7, 15, 16, 17, 31, 32, 33, 100, 1024, 4097] {
            let a: Vec<u16> = (0..len)
                .map(|i| ((i as u32 * 137) % P_65521 as u32) as u16)
                .collect();
            let b: Vec<u16> = (0..len)
                .map(|i| ((i as u32 * 211 + 7) % P_65521 as u32) as u16)
                .collect();
            let mut out = vec![0u16; len];
            unsafe { fp_medium_batch_mul(&a, &b, P_65521, M_65521, &mut out) };
            for i in 0..len {
                assert_eq!(out[i], scalar_mul(a[i], b[i], P_65521), "len={len} i={i}");
            }
        }
    }

    #[test]
    fn batch_add_matches_scalar_65521() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        for &len in &[0usize, 1, 15, 16, 17, 100] {
            let a: Vec<u16> = (0..len)
                .map(|i| ((i as u32 * 4093) % P_65521 as u32) as u16)
                .collect();
            let b: Vec<u16> = (0..len)
                .map(|i| ((i as u32 * 9973) % P_65521 as u32) as u16)
                .collect();
            let mut out = vec![0u16; len];
            unsafe { fp_medium_batch_add(&a, &b, P_65521, &mut out) };
            for i in 0..len {
                assert_eq!(out[i], scalar_add(a[i], b[i], P_65521), "len={len} i={i}");
            }
        }
    }

    #[test]
    fn batch_sub_matches_scalar_65521() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        for &len in &[0usize, 1, 15, 16, 17, 100] {
            let a: Vec<u16> = (0..len)
                .map(|i| ((i as u32 * 4093) % P_65521 as u32) as u16)
                .collect();
            let b: Vec<u16> = (0..len)
                .map(|i| ((i as u32 * 9973) % P_65521 as u32) as u16)
                .collect();
            let mut out = vec![0u16; len];
            unsafe { fp_medium_batch_sub(&a, &b, P_65521, &mut out) };
            for i in 0..len {
                assert_eq!(out[i], scalar_sub(a[i], b[i], P_65521), "len={len} i={i}");
            }
        }
    }

    #[test]
    fn batch_dot_matches_scalar_65521() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        for &len in &[0usize, 1, 15, 16, 17, 100, 256, 1024] {
            let a: Vec<u16> = (0..len)
                .map(|i| ((i as u32 * 17) % P_65521 as u32) as u16)
                .collect();
            let b: Vec<u16> = (0..len)
                .map(|i| ((i as u32 * 23 + 5) % P_65521 as u32) as u16)
                .collect();
            let got = unsafe { fp_medium_batch_dot(&a, &b, P_65521) };
            let mut expected: u64 = 0;
            for i in 0..len {
                expected += (a[i] as u64) * (b[i] as u64);
            }
            assert_eq!(got as u64, expected % P_65521 as u64, "len={len}");
        }
    }

    #[test]
    fn batch_dot_boundary_values_65521() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        // Worst-case lane saturation: every product is (P-1)² ≈ 2^32.
        let a = vec![65520u16; 1024];
        let b = vec![65520u16; 1024];
        let got = unsafe { fp_medium_batch_dot(&a, &b, P_65521) };
        let expected = (1024u64 * 65520u64 * 65520u64) % P_65521 as u64;
        assert_eq!(got as u64, expected);
    }

    #[test]
    fn smaller_medium_primes_match_scalar() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        for &p in &[257u16, 509, 1009, 8191, 32749] {
            let m = barrett_m32(p);
            let a: Vec<u16> = (0..200)
                .map(|i| ((i as u32 * 17) % p as u32) as u16)
                .collect();
            let b: Vec<u16> = (0..200)
                .map(|i| ((i as u32 * 23 + 5) % p as u32) as u16)
                .collect();
            let mut out = vec![0u16; 200];
            unsafe { fp_medium_batch_mul(&a, &b, p, m, &mut out) };
            for i in 0..200 {
                assert_eq!(out[i], scalar_mul(a[i], b[i], p), "p={p} i={i}");
            }
            let got = unsafe { fp_medium_batch_dot(&a, &b, p) };
            let mut expected: u64 = 0;
            for i in 0..200 {
                expected += (a[i] as u64) * (b[i] as u64);
            }
            assert_eq!(got as u64, expected % p as u64, "dot p={p}");
        }
    }

    #[test]
    fn spmm_row_matches_scalar() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        // Cover P at the small/large medium boundary plus the
        // reference 65521 prime; sweep (nnz, b_rows, n) shapes that
        // hit the SIMD body and the scalar tail.
        for &p in &[257u16, 65521, 8191, 1009] {
            let cases = [
                (1usize, 16usize, 8usize),
                (5, 16, 32),
                (7, 16, 33),
                (10, 16, 64),
                (1, 16, 17),
                (20, 16, 100),
                (3, 8, 16),
                (10, 32, 128),
                (15, 16, 1024),
            ];
            for &(nnz, b_rows, n) in &cases {
                let a_vals: Vec<u16> = (0..nnz as u32)
                    .map(|h| ((h * 13 + 1) % p as u32) as u16)
                    .collect();
                let a_cols: Vec<usize> = (0..nnz).map(|h| (h * 7) % b_rows).collect();
                let b: Vec<u16> = (0..(b_rows * n) as u32)
                    .map(|i| ((i * 23 + 5) % p as u32) as u16)
                    .collect();
                let mut out = vec![0u16; n];
                unsafe { fp_medium_spmm_row(&a_vals, &a_cols, &b, n, n, p, &mut out) };
                for (j, &val) in out.iter().enumerate() {
                    let mut expected: u64 = 0;
                    for h in 0..nnz {
                        let col = a_cols[h];
                        expected = expected.wrapping_add(a_vals[h] as u64 * b[col * n + j] as u64);
                    }
                    expected %= p as u64;
                    assert_eq!(
                        val as u64, expected,
                        "p={p} nnz={nnz} b_rows={b_rows} n={n} j={j}"
                    );
                }
            }
        }
    }

    #[test]
    fn spmm_row_empty_nnz() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        let p = 65521u16;
        let n = 64;
        let a_vals: Vec<u16> = vec![];
        let a_cols: Vec<usize> = vec![];
        let b: Vec<u16> = (0..n).map(|i| ((i as u32 * 7) % p as u32) as u16).collect();
        let mut out = vec![5u16; n];
        unsafe { fp_medium_spmm_row(&a_vals, &a_cols, &b, n, n, p, &mut out) };
        for (j, &val) in out.iter().enumerate().take(n) {
            assert_eq!(val, 0, "j={j}");
        }
    }
}
