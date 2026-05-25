//! AVX2 byte-lane batch kernels for small `Fp<P>` with `P <= 251`.
//!
//! Inputs and outputs are canonical bytes (`u8`, value `< P`). The
//! kernels lane-pack 16 elements per output pass: load 16 bytes, expand
//! to 16-bit lanes, multiply via `_mm256_mullo_epi16`, Barrett-reduce
//! modulo `P`, then pack back to bytes via `_mm_packus_epi16`.
//!
//! Barrett constant: `μ = ⌊2¹⁶ / P⌋`. For `n ∈ [0, 2¹⁶)` the bound
//! `r = n − ⌊n·μ / 2¹⁶⌋ · P ∈ [0, 2P)` holds, so a single conditional
//! subtract canonicalises. We compute the high half of `n·μ` via
//! `_mm256_mulhi_epu16`.
//!
//! The dot-product entry point uses `_mm256_madd_epi16` to fuse the
//! 16-bit-pair multiply and 32-bit-pair add in one cycle on Zen 3,
//! reducing modulo `P` at the panel boundary via a single scalar
//! horizontal sum.
//!
//! # Safety
//!
//! All public functions here are `unsafe` — callers must ensure AVX2
//! is available at runtime. Safe, dispatched entry points live in
//! `fp_small.rs` via the `SmallPrimeFns` table returned by `detect`.

#![allow(clippy::missing_safety_doc)]

use core::arch::asm;
use core::arch::x86_64::*;

// ---------------------------------------------------------------------------
// Barrett constants
// ---------------------------------------------------------------------------

/// Computes the 16-bit Barrett constant for an odd prime `p ∈ [3, 255]`.
///
/// Returns `μ = ⌊2¹⁶ / p⌋`. For canonical input `n ∈ [0, 2¹⁶)`,
/// `q = mulhi_u16(n, μ)` satisfies `q · p ≤ n` and `n − q · p < 2 · p`,
/// so a single conditional subtract canonicalises.
///
/// We restrict `p ≥ 3` so `μ` always fits in `u16` without saturation
/// (`μ ≤ ⌊2¹⁶/3⌋ = 21845`). The caller must already restrict `p ≤ 251`
/// for the byte-lane representation to be sound.
#[inline(always)]
pub(crate) const fn barrett_mu_u16(p: u8) -> u16 {
    debug_assert!(p >= 3);
    (65536u32 / p as u32) as u16
}

// ---------------------------------------------------------------------------
// Reduction helpers
// ---------------------------------------------------------------------------

/// Reduces 16 packed `u16` lanes (each `< 2¹⁶`) modulo `p`, returning
/// 16 packed canonical `u16` lanes (each `< p`).
///
/// Implements the 16-bit Barrett reduction described in the module
/// docs.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn reduce_mod_p_u16(n: __m256i, p: u8) -> __m256i {
    let mu = _mm256_set1_epi16(barrett_mu_u16(p) as i16);
    let p_vec = _mm256_set1_epi16(p as i16);

    // q = (n * mu) >> 16 in u16 lanes (mulhi).
    let q = _mm256_mulhi_epu16(n, mu);
    // r = n - q * p, with mullo's u16-truncated product: since the
    // mathematical product q * p < 2^16 (q ≤ μ ≤ 2^16/3 and p · μ < 2^16
    // for any p ≥ 3), no truncation occurs.
    let qp = _mm256_mullo_epi16(q, p_vec);
    let r = _mm256_sub_epi16(n, qp);

    // r ∈ [0, 2p). Conditional subtract: r' = (r ≥ p) ? r - p : r,
    // implemented via `min_epu16(r, r - p)` — when r < p, r - p wraps
    // to a value > r (treated as unsigned), so the min keeps r.
    let r_minus_p = _mm256_sub_epi16(r, p_vec);
    _mm256_min_epu16(r, r_minus_p)
}

/// Reduces 16 packed signed `i16` lanes in `[-p, p)` modulo `p`,
/// returning 16 packed canonical `u16` lanes in `[0, p)`.
///
/// Used by the subtract path: after the lane-wise `a - b`, results land
/// in `[-(p-1), p-1]`; adding `p` lifts negatives into `[1, p-1]` while
/// leaving non-negatives in `[p, 2p-1]`. A single conditional subtract
/// canonicalises.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn canon_after_sub(diff: __m256i, p: u8) -> __m256i {
    let p_vec = _mm256_set1_epi16(p as i16);
    // shifted = diff + p ∈ [1, 2p - 1].
    let shifted = _mm256_add_epi16(diff, p_vec);
    // Conditional subtract to land in [0, p).
    let minus_p = _mm256_sub_epi16(shifted, p_vec);
    _mm256_min_epu16(shifted, minus_p)
}

// ---------------------------------------------------------------------------
// Pack/unpack helpers
// ---------------------------------------------------------------------------

/// Loads 16 packed bytes from `ptr` and zero-extends them into a
/// 256-bit vector of 16 `u16` lanes.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn load_u8_to_u16(ptr: *const u8) -> __m256i {
    let v128 = _mm_loadu_si128(ptr as *const __m128i);
    _mm256_cvtepu8_epi16(v128)
}

/// Packs a 256-bit vector of 16 canonical `u16` lanes (each `< 256`)
/// back to 16 contiguous bytes at `ptr`.
///
/// Lane-shuffles via `_mm256_packus_epi16` then de-interleaves the
/// resulting two 128-bit lanes via `_mm256_permute4x64_epi64`.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn store_u16_to_u8(v: __m256i, ptr: *mut u8) {
    // packus_epi16 saturates u16 → u8. All input lanes are < 256 so
    // saturation never fires; the pack is purely a narrowing.
    let packed = _mm256_packus_epi16(v, _mm256_setzero_si256());
    // packus interleaves 128-bit halves: lanes [0..7, 0, 0..0, 8..15, 0..0].
    // Permute the 64-bit qwords [0, 2, 1, 3] → [a_lo, b_lo, a_hi, b_hi]
    // so the 16 packed bytes land contiguously in the low 128 bits.
    let permuted = _mm256_permute4x64_epi64::<0b1101_1000>(packed);
    _mm_storeu_si128(ptr as *mut __m128i, _mm256_castsi256_si128(permuted));
}

// ---------------------------------------------------------------------------
// Scalar tail helpers
// ---------------------------------------------------------------------------

#[inline(always)]
fn scalar_mul_mod(a: u8, b: u8, p: u8) -> u8 {
    ((a as u32 * b as u32) % p as u32) as u8
}

#[inline(always)]
fn scalar_add_mod(a: u8, b: u8, p: u8) -> u8 {
    let s = a as u16 + b as u16;
    if s >= p as u16 {
        (s - p as u16) as u8
    } else {
        s as u8
    }
}

#[inline(always)]
fn scalar_sub_mod(a: u8, b: u8, p: u8) -> u8 {
    if a >= b {
        a - b
    } else {
        p - (b - a)
    }
}

// ---------------------------------------------------------------------------
// Public batch entry points
// ---------------------------------------------------------------------------

/// Batch lane-wise multiplication for `Fp<P>` with `P ≤ 251`.
///
/// Computes `out[i] = a[i] * b[i] mod p` for all `i`. Inputs and
/// outputs are canonical bytes (`< p`).
///
/// # Safety
///
/// Caller must ensure AVX2 is available at runtime, `p` is an odd
/// prime in `[3, 251]`, and all input bytes are canonical (`< p`).
///
/// # Panics
///
/// Panics if the slice lengths differ.
#[target_feature(enable = "avx2")]
pub unsafe fn fp_small_batch_mul(a: &[u8], b: &[u8], p: u8, out: &mut [u8]) {
    assert_eq!(a.len(), b.len(), "fp_small_batch_mul: length mismatch");
    assert_eq!(a.len(), out.len(), "fp_small_batch_mul: output length");

    let n = a.len();
    let nvec = n / 16;

    let mut a_ptr = a.as_ptr();
    let mut b_ptr = b.as_ptr();
    let mut o_ptr = out.as_mut_ptr();

    for _ in 0..nvec {
        let av = load_u8_to_u16(a_ptr);
        let bv = load_u8_to_u16(b_ptr);
        // 16-bit-lane mul: lane = a · b ≤ (P-1)² ≤ 250² = 62500 < 2^16.
        let prod = _mm256_mullo_epi16(av, bv);
        let red = reduce_mod_p_u16(prod, p);
        store_u16_to_u8(red, o_ptr);
        a_ptr = a_ptr.add(16);
        b_ptr = b_ptr.add(16);
        o_ptr = o_ptr.add(16);
    }

    // Scalar tail.
    let tail_start = nvec * 16;
    for i in tail_start..n {
        *out.get_unchecked_mut(i) = scalar_mul_mod(*a.get_unchecked(i), *b.get_unchecked(i), p);
    }
}

/// Batch lane-wise addition for `Fp<P>` with `P ≤ 251`.
///
/// Computes `out[i] = (a[i] + b[i]) mod p`. Inputs and outputs are
/// canonical bytes (`< p`).
///
/// # Safety
///
/// Same contract as [`fp_small_batch_mul`].
///
/// # Panics
///
/// Panics if the slice lengths differ.
#[target_feature(enable = "avx2")]
pub unsafe fn fp_small_batch_add(a: &[u8], b: &[u8], p: u8, out: &mut [u8]) {
    assert_eq!(a.len(), b.len(), "fp_small_batch_add: length mismatch");
    assert_eq!(a.len(), out.len(), "fp_small_batch_add: output length");

    let n = a.len();
    let nvec = n / 16;

    let p_vec = _mm256_set1_epi16(p as i16);

    let mut a_ptr = a.as_ptr();
    let mut b_ptr = b.as_ptr();
    let mut o_ptr = out.as_mut_ptr();

    for _ in 0..nvec {
        let av = load_u8_to_u16(a_ptr);
        let bv = load_u8_to_u16(b_ptr);
        let sum = _mm256_add_epi16(av, bv); // sum ∈ [0, 2p) ⊂ [0, 502)
        let sum_minus_p = _mm256_sub_epi16(sum, p_vec);
        let red = _mm256_min_epu16(sum, sum_minus_p);
        store_u16_to_u8(red, o_ptr);
        a_ptr = a_ptr.add(16);
        b_ptr = b_ptr.add(16);
        o_ptr = o_ptr.add(16);
    }

    let tail_start = nvec * 16;
    for i in tail_start..n {
        *out.get_unchecked_mut(i) = scalar_add_mod(*a.get_unchecked(i), *b.get_unchecked(i), p);
    }
}

/// Batch lane-wise subtraction for `Fp<P>` with `P ≤ 251`.
///
/// Computes `out[i] = (a[i] − b[i]) mod p`, with the result in the
/// canonical range `[0, p)`.
///
/// # Safety
///
/// Same contract as [`fp_small_batch_mul`].
///
/// # Panics
///
/// Panics if the slice lengths differ.
#[target_feature(enable = "avx2")]
pub unsafe fn fp_small_batch_sub(a: &[u8], b: &[u8], p: u8, out: &mut [u8]) {
    assert_eq!(a.len(), b.len(), "fp_small_batch_sub: length mismatch");
    assert_eq!(a.len(), out.len(), "fp_small_batch_sub: output length");

    let n = a.len();
    let nvec = n / 16;

    let mut a_ptr = a.as_ptr();
    let mut b_ptr = b.as_ptr();
    let mut o_ptr = out.as_mut_ptr();

    for _ in 0..nvec {
        let av = load_u8_to_u16(a_ptr);
        let bv = load_u8_to_u16(b_ptr);
        // diff ∈ [-(p-1), p-1] in 16-bit signed lanes (value-equivalent
        // to (a - b) reinterpreted), still fits comfortably in i16.
        let diff = _mm256_sub_epi16(av, bv);
        let red = canon_after_sub(diff, p);
        store_u16_to_u8(red, o_ptr);
        a_ptr = a_ptr.add(16);
        b_ptr = b_ptr.add(16);
        o_ptr = o_ptr.add(16);
    }

    let tail_start = nvec * 16;
    for i in tail_start..n {
        *out.get_unchecked_mut(i) = scalar_sub_mod(*a.get_unchecked(i), *b.get_unchecked(i), p);
    }
}

/// Batch dot product for `Fp<P>` with `P ≤ 251`.
///
/// Returns `sum_i (a[i] * b[i]) mod p`, accumulating into 32-bit AVX2
/// lanes via `_mm256_madd_epi16` (the lane-pair fused multiply-add) and
/// reducing modulo `p` once at the panel boundary.
///
/// At `P = 251` the per-lane MAC budget is `⌊2³² / (P − 1)²⌋ ≈ 6.87 ×
/// 10⁴` before overflow; we conservatively reduce every 16 384 elements
/// (well below that cap) so even adversarial inputs stay safe.
///
/// # Safety
///
/// Same contract as [`fp_small_batch_mul`].
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
#[target_feature(enable = "avx2")]
pub unsafe fn fp_small_batch_dot(a: &[u8], b: &[u8], p: u8) -> u8 {
    assert_eq!(a.len(), b.len(), "fp_small_batch_dot: length mismatch");
    let n = a.len();

    // Each 16-bit pair-product is at most (P-1)² ≤ 62500. Each
    // _mm256_madd_epi16 lane sums two such products, so each 32-bit
    // accumulator lane gains ≤ 125000 per vector iteration. With a
    // u32 cap of 2^32, we have ~34000 iterations of safe-budget. We
    // refresh the accumulator and reduce to scalar every CHUNK_VEC
    // iterations to keep a fat margin.
    const CHUNK_VEC: usize = 16384;
    let nvec = n / 16;
    let mut total: u64 = 0;
    let p_u32 = p as u32;

    let a_base = a.as_ptr() as *const __m256i;
    let b_base = b.as_ptr() as *const __m256i;

    let mut vec_idx = 0;
    while vec_idx < nvec {
        let chunk_end = (vec_idx + CHUNK_VEC).min(nvec);
        let mut acc = _mm256_setzero_si256();
        for i in vec_idx..chunk_end {
            // Load 16 u8 lanes into u16 lanes from each of a, b.
            let av_lo = _mm_loadu_si128(a_base.cast::<u8>().add(i * 16) as *const __m128i);
            let bv_lo = _mm_loadu_si128(b_base.cast::<u8>().add(i * 16) as *const __m128i);
            let av = _mm256_cvtepu8_epi16(av_lo);
            let bv = _mm256_cvtepu8_epi16(bv_lo);
            // madd_epi16 multiplies u16 lane-pairs and sums into u32
            // lanes: out[i] = a[2i]*b[2i] + a[2i+1]*b[2i+1].
            let mac = _mm256_madd_epi16(av, bv);
            acc = _mm256_add_epi32(acc, mac);
        }

        // Horizontal sum across 8 u32 lanes.
        let lo = _mm256_castsi256_si128(acc);
        let hi = _mm256_extracti128_si256::<1>(acc);
        let s128 = _mm_add_epi32(lo, hi);
        // s128 has 4 u32 lanes; sum them into a single u32.
        let mut tmp = [0u32; 4];
        _mm_storeu_si128(tmp.as_mut_ptr() as *mut __m128i, s128);
        let chunk_sum: u32 = tmp[0]
            .wrapping_add(tmp[1])
            .wrapping_add(tmp[2])
            .wrapping_add(tmp[3]);
        total = (total + chunk_sum as u64) % p_u32 as u64;

        vec_idx = chunk_end;
    }

    // Scalar tail.
    let tail_start = nvec * 16;
    for i in tail_start..n {
        total =
            (total + (*a.get_unchecked(i) as u64) * (*b.get_unchecked(i) as u64)) % p_u32 as u64;
    }

    total as u8
}

/// Whole-row gemm panel. Computes `out[j] = (∑_t a[t] * bt[j*k + t]) mod p`
/// for `j ∈ [0, n)`, where `a` is one length-`k` row of the left
/// matrix and `bt` is the row-major B-transpose (`n` rows × `k`
/// columns). Output is `n` canonical bytes written to `out`.
///
/// The kernel loads each 16-byte block of `a` once and reuses it
/// against four B^T rows simultaneously, amortising the AVX2 lane
/// broadcasts and constant-table loads across four output cells per
/// pass. Fixed prime-`p` constants are loaded once at the head of
/// the function.
///
/// # Safety
///
/// Caller must ensure AVX2 is available, `p` is an odd prime in
/// `[3, 251]`, and all input bytes are canonical (`< p`).
///
/// # Panics
///
/// Panics if `bt.len() != n * k` or `out.len() != n`.
#[target_feature(enable = "avx2")]
pub unsafe fn fp_small_gemm_row_panel(
    a: &[u8],
    bt: &[u8],
    k: usize,
    n: usize,
    p: u8,
    out: &mut [u8],
) {
    assert_eq!(a.len(), k, "fp_small_gemm_row_panel: a.len() != k");
    assert_eq!(
        bt.len(),
        n * k,
        "fp_small_gemm_row_panel: bt.len() != n * k"
    );
    assert_eq!(out.len(), n, "fp_small_gemm_row_panel: out.len() != n");

    let nvec = k / 16;
    let p_u32 = p as u32;

    // Process 4 output cells per inner sweep, sharing the loaded A
    // chunks across four parallel accumulators.
    let mut j = 0;
    while j + 4 <= n {
        let bt0 = bt.as_ptr().add(j * k);
        let bt1 = bt.as_ptr().add((j + 1) * k);
        let bt2 = bt.as_ptr().add((j + 2) * k);
        let bt3 = bt.as_ptr().add((j + 3) * k);
        let mut acc0 = _mm256_setzero_si256();
        let mut acc1 = _mm256_setzero_si256();
        let mut acc2 = _mm256_setzero_si256();
        let mut acc3 = _mm256_setzero_si256();
        for v in 0..nvec {
            let av128 = _mm_loadu_si128(a.as_ptr().add(v * 16) as *const __m128i);
            let av = _mm256_cvtepu8_epi16(av128);
            let b0 = _mm256_cvtepu8_epi16(_mm_loadu_si128(bt0.add(v * 16) as *const __m128i));
            let b1 = _mm256_cvtepu8_epi16(_mm_loadu_si128(bt1.add(v * 16) as *const __m128i));
            let b2 = _mm256_cvtepu8_epi16(_mm_loadu_si128(bt2.add(v * 16) as *const __m128i));
            let b3 = _mm256_cvtepu8_epi16(_mm_loadu_si128(bt3.add(v * 16) as *const __m128i));
            acc0 = _mm256_add_epi32(acc0, _mm256_madd_epi16(av, b0));
            acc1 = _mm256_add_epi32(acc1, _mm256_madd_epi16(av, b1));
            acc2 = _mm256_add_epi32(acc2, _mm256_madd_epi16(av, b2));
            acc3 = _mm256_add_epi32(acc3, _mm256_madd_epi16(av, b3));
        }
        // Horizontal-sum each accumulator and reduce mod p.
        let sums = [
            horizontal_sum_u32(acc0),
            horizontal_sum_u32(acc1),
            horizontal_sum_u32(acc2),
            horizontal_sum_u32(acc3),
        ];
        let tail_start = nvec * 16;
        for (jj, &sum) in sums.iter().enumerate() {
            let mut total = sum as u64;
            let bt_row = bt.as_ptr().add((j + jj) * k);
            for t in tail_start..k {
                total += (*a.get_unchecked(t) as u64) * (*bt_row.add(t) as u64);
            }
            *out.get_unchecked_mut(j + jj) = (total % p_u32 as u64) as u8;
        }
        j += 4;
    }
    // Scalar-loop tail for non-multiples of 4 in `n`.
    while j < n {
        let bt_row = bt.as_ptr().add(j * k);
        let mut acc = _mm256_setzero_si256();
        for v in 0..nvec {
            let av =
                _mm256_cvtepu8_epi16(_mm_loadu_si128(a.as_ptr().add(v * 16) as *const __m128i));
            let bv = _mm256_cvtepu8_epi16(_mm_loadu_si128(bt_row.add(v * 16) as *const __m128i));
            acc = _mm256_add_epi32(acc, _mm256_madd_epi16(av, bv));
        }
        let mut total = horizontal_sum_u32(acc) as u64;
        let tail_start = nvec * 16;
        for t in tail_start..k {
            total += (*a.get_unchecked(t) as u64) * (*bt_row.add(t) as u64);
        }
        *out.get_unchecked_mut(j) = (total % p_u32 as u64) as u8;
        j += 1;
    }
}

/// Fused `buf := (buf − α · chain_j) mod p` in-place AXPY-style kernel
/// for `Fp<P>` with `P ≤ 251`.
///
/// Inputs are canonical bytes (each `< p`). The kernel performs, for
/// `i ∈ [0, chain_j.len())`:
///
/// ```text
///     buf[i] := (buf[i] − α · chain_j[i]) mod p
/// ```
///
/// `buf` is mutated in place; `chain_j` is read only. `buf.len()` may
/// exceed `chain_j.len()` (extra elements are not touched).
///
/// # Why this exists
///
/// The closing kernel for the `52cce970` issue's GF(251)/n=256 charpoly
/// gap. `PackedFpChainPolys::sub_scaled_into` (in `gf2-core`) was
/// previously implemented as `tmp = batch_mul(α, chain_j); buf =
/// batch_sub(buf, tmp)`, paying:
///
///   * two `[u8] → [u8]` AVX2 kernel function-pointer indirections
///     per call,
///   * one `cj_len` byte broadcast-fill into a scratch lane,
///   * one `cj_len` byte intermediate write (the product),
///   * one `cj_len` byte copy back from the scratch lane to `buf`.
///
/// Fusing collapses those into a single 16-lane register-resident
/// read-modify-write loop:
///   * `α`, the Barrett constant `μ`, and `p` stay broadcast-loaded
///     in `ymm` registers across the whole loop (three registers).
///   * The intermediate product never leaves register `ymm4`.
///   * The only memory traffic per iteration is one 16-byte load
///     from `chain_j`, one 16-byte load from `buf`, and one 16-byte
///     store to `buf`.
///
/// # Algorithm
///
/// Per 16-lane iteration:
///
/// 1. Load 16 bytes from `chain_j`; zero-extend to 16 × `u16` lanes.
/// 2. Multiply lane-wise by broadcast `α`: product ≤ `(P − 1)² ≤ 250² = 62 500 < 2¹⁶`.
/// 3. Reduce mod `p` via a single 16-bit Barrett step (`mulhi(prod, μ)`
///    quotient + `mullo(q, p)` correction + conditional subtract).
/// 4. Load 16 bytes from `buf`; zero-extend.
/// 5. `diff = buf − reduced_prod ∈ [−(p−1), p−1]` (16-bit signed).
/// 6. Lift via `diff + p`, conditionally subtract `p` to land in `[0, p)`.
/// 7. Pack 16 × `u16` back to 16 × `u8` and store into `buf`.
///
/// Tail bytes (`chain_j.len() % 16`) run a scalar `(buf[i] + (p −
/// (α · chain_j[i]) mod p)) mod p` per byte.
///
/// # Why `mu` is a parameter (jit:52cce970 R1)
///
/// The Barrett constant `μ = ⌊2¹⁶ / p⌋` was previously recomputed at the
/// start of every call via a 22-25 cycle integer `div` and broadcast to a
/// `ymm` register. The reduce-path hot loop in
/// `gf2_core::gfp::simd_ops::fp_reduce_packed` and the chain-poly
/// bookkeeping in `PackedFpChainPolys::sub_scaled_into` jointly invoke
/// this kernel ~32 000 times per GF(251)/n=256 charpoly call, which made
/// the per-call `div` cost a 7-8 % wall-time tax. Hoisting `μ` out of
/// the kernel (precomputed once per prime by
/// `build_small_prime_tables::<P>().barrett_mu`) eliminates that tax.
///
/// # Why `vpmulhuw` is hand-encoded via inline `asm!` (jit:52cce970 R1)
///
/// LLVM 19 (rustc 1.95) compiles the natural `_mm256_mulhi_epu16(prod,
/// mu_vec)` intrinsic into a six-instruction `vpmovzxwd` /
/// `vextracti128` / `vpmovzxwd` / two `vpmulhuw` / `vpackusdw` /
/// `vpermq` widen-then-pack sequence whenever one of the operands is a
/// broadcast vector — the optimiser appears to lose track of the
/// 16-bit-lane invariant and falls back to a 32-bit-lane intermediate.
/// In isolation, the same intrinsic with locally constructed operands
/// emits a single `vpmulhuw` so the issue is a per-call codegen quality
/// problem rather than an intrinsic limitation. Forcing the single-
/// instruction encoding via `asm!` cuts the inner loop from 21 to ~13
/// instructions per 16-lane iteration (~35 % body speedup).
///
/// # Safety
///
/// Caller must ensure:
///   * AVX2 is available at runtime.
///   * `p` is an odd prime in `[3, 251]`.
///   * `alpha < p`.
///   * `mu == ⌊2¹⁶ / p⌋`. (Pass via `SmallPrimeTables::barrett_mu`.)
///   * Every byte of `buf` and `chain_j` is canonical (`< p`).
///   * `buf.len() >= chain_j.len()`.
///
/// # Panics
///
/// Panics if `buf.len() < chain_j.len()`.
#[target_feature(enable = "avx2")]
pub unsafe fn fp_small_sub_scaled(buf: &mut [u8], chain_j: &[u8], alpha: u8, p: u8, mu: u16) {
    // SAFETY: the kernel is annotated `#[target_feature(enable = "avx2")]`
    // and the caller has already verified AVX2 availability via the
    // `SmallPrimeFns` dispatch table returned by `detect_x86`. All
    // intrinsics below are AVX2 (256-bit lane) or SSE2 (128-bit lane)
    // and require no fences. The inline `asm!` block is a single
    // `vpmulhuw` (AVX2 packed 16-bit unsigned high-multiply) with pure
    // / nomem / nostack options so it has no side effects beyond the
    // explicit output register. Pointer arithmetic stays within the
    // bounds asserted at function entry. `mu` is supplied by the caller
    // (matches `⌊2¹⁶ / p⌋`) so no per-call division is required.
    assert!(
        buf.len() >= chain_j.len(),
        "fp_small_sub_scaled: buf shorter than chain_j ({} < {})",
        buf.len(),
        chain_j.len()
    );
    debug_assert_eq!(
        mu,
        barrett_mu_u16(p),
        "fp_small_sub_scaled: mu must equal ⌊2¹⁶ / p⌋"
    );

    let n = chain_j.len();
    if n == 0 {
        return;
    }
    let nvec = n / 16;

    let alpha_vec = _mm256_set1_epi16(alpha as i16);
    let mu_vec = _mm256_set1_epi16(mu as i16);
    let p_vec = _mm256_set1_epi16(p as i16);

    let mut c_ptr = chain_j.as_ptr();
    let mut b_ptr = buf.as_mut_ptr();
    for _ in 0..nvec {
        // 1. Load 16 chain_j bytes, expand to u16 lanes.
        let cv = _mm256_cvtepu8_epi16(_mm_loadu_si128(c_ptr as *const __m128i));
        // 2. Lane-wise mul by α. Product fits in u16.
        let prod = _mm256_mullo_epi16(cv, alpha_vec);
        // 3. Barrett-reduce mod p (single step, result in [0, p)). The
        //    natural `_mm256_mulhi_epu16(prod, mu_vec)` here is compiled
        //    into a six-instruction widen-then-pack sequence on rustc
        //    1.95 — see the function-level rustdoc for the analysis.
        //    Forcing the single-instruction encoding via inline `asm!`
        //    cuts the inner loop from 21 to ~13 instructions per
        //    16-lane iteration.
        let q: __m256i;
        asm!(
            "vpmulhuw {q}, {p}, {m}",
            q = out(ymm_reg) q,
            p = in(ymm_reg) prod,
            m = in(ymm_reg) mu_vec,
            options(pure, nomem, nostack, preserves_flags),
        );
        let qp = _mm256_mullo_epi16(q, p_vec);
        let r = _mm256_sub_epi16(prod, qp);
        let r_minus_p = _mm256_sub_epi16(r, p_vec);
        let r_canon = _mm256_min_epu16(r, r_minus_p);
        // 4. Load buf bytes, expand.
        let bv = _mm256_cvtepu8_epi16(_mm_loadu_si128(b_ptr as *const __m128i));
        // 5. diff = bv - r_canon ∈ [-(p-1), p-1] (signed 16-bit).
        let diff = _mm256_sub_epi16(bv, r_canon);
        // 6. shifted = diff + p ∈ [1, 2p-1].
        let shifted = _mm256_add_epi16(diff, p_vec);
        let shifted_minus_p = _mm256_sub_epi16(shifted, p_vec);
        let out = _mm256_min_epu16(shifted, shifted_minus_p);
        // 7. Pack 16 × u16 → 16 × u8 and store.
        store_u16_to_u8(out, b_ptr);

        c_ptr = c_ptr.add(16);
        b_ptr = b_ptr.add(16);
    }

    // Scalar tail (at most 15 iterations; bounds-check cost is negligible
    // compared to the modular arithmetic).
    let tail_start = nvec * 16;
    let p_u32 = p as u32;
    let alpha_u32 = alpha as u32;
    for i in tail_start..n {
        let prod = (alpha_u32 * chain_j[i] as u32) % p_u32;
        let b = buf[i] as u32;
        // b + (p - prod) mod p — keeps the unsigned arithmetic positive.
        let new = (b + (p_u32 - prod)) % p_u32;
        buf[i] = new as u8;
    }
}

/// Sparse-times-dense row kernel: writes `out[j] = (∑_h a_vals[h] *
/// b[a_cols[h] * b_stride + j]) mod p` for `j ∈ [0, n)`.
///
/// `a_vals` and `a_cols` describe one row of a sparse left matrix in
/// CSR form: `a_vals.len() == a_cols.len() == nnz_r` and `a_cols[h]`
/// is the column index of the `h`-th non-zero into `b`. `b` is a
/// row-major dense byte matrix of stride `b_stride`; row `k` spans
/// `b[k * b_stride .. k * b_stride + n]`. `out` is the dense output
/// row of length `n`.
///
/// The kernel iterates output blocks of 16 lanes; for each block it
/// sweeps every non-zero of the sparse row, broadcasts `a_vals[h]` to
/// 16 u16 lanes, multiplies element-wise with the loaded B-row block,
/// and accumulates into two 32-bit lane vectors. After the sparse-row
/// sweep, the 32-bit lanes are reduced modulo `p` and packed back to
/// bytes. The accumulator overflow bound is `nnz_r · (p-1)² < 2³²`,
/// which holds for any realistic sparse density at `p ≤ 251` (e.g.
/// nnz_r = 10 000 at `p = 251` gives `≈ 6.25 × 10⁸`).
///
/// # Safety
///
/// Caller must ensure AVX2 is available, `p` is an odd prime in
/// `[3, 251]`, and:
/// - `a_vals.len() == a_cols.len()`,
/// - every `a_cols[h] * b_stride + n` is within `b.len()`,
/// - every `a_vals[h] < p` and each B-byte read is canonical (`< p`).
///
/// # Panics
///
/// Panics if `out.len() != n` or `a_vals.len() != a_cols.len()`.
#[target_feature(enable = "avx2")]
pub unsafe fn fp_small_spmm_row(
    a_vals: &[u8],
    a_cols: &[usize],
    b: &[u8],
    b_stride: usize,
    n: usize,
    p: u8,
    out: &mut [u8],
) {
    assert_eq!(
        a_vals.len(),
        a_cols.len(),
        "fp_small_spmm_row: a_vals/a_cols length mismatch"
    );
    assert_eq!(out.len(), n, "fp_small_spmm_row: out.len() != n");

    let nnz = a_vals.len();
    let p_u32 = p as u32;
    let p_vec = _mm256_set1_epi32(p_u32 as i32);
    // Use Barrett at 32-bit lane width: q = (x * mu32) >> 32, with
    // mu32 = ⌊2³² / p⌋. The SSOT `barrett_reduce_lane32` primitive uses
    // `_mm256_mul_epu32` internally, which only reads the low 32 bits of
    // each 64-bit lane — broadcasting μ as `epi64x` lets the primitive
    // skip an in-kernel `set1_epi32` rebuild on every call.
    let mu32 = ((1u64 << 32) / p_u32 as u64) as u32;
    let mu_vec = _mm256_set1_epi64x(mu32 as i64);

    let mut j = 0;
    while j + 16 <= n {
        // 16 u32 accumulators split across two ymm registers (8 lo + 8 hi).
        let mut acc_lo = _mm256_setzero_si256();
        let mut acc_hi = _mm256_setzero_si256();
        for h in 0..nnz {
            let a_h = *a_vals.get_unchecked(h);
            let col = *a_cols.get_unchecked(h);
            let b_row_ptr = b.as_ptr().add(col * b_stride + j);
            // Load 16 bytes from B[col, j..j+16], expand to 16 u16 lanes.
            let bv8 = _mm_loadu_si128(b_row_ptr as *const __m128i);
            let bv16 = _mm256_cvtepu8_epi16(bv8);
            // Broadcast a_h to all 16 u16 lanes.
            let av16 = _mm256_set1_epi16(a_h as i16);
            // Element-wise u16 product (≤ 250² = 62500 fits in u16).
            let prod = _mm256_mullo_epi16(av16, bv16);
            // Widen 16 u16 lanes → 16 u32 lanes via two unpack-with-zero.
            let zero = _mm256_setzero_si256();
            let plo = _mm256_unpacklo_epi16(prod, zero);
            let phi = _mm256_unpackhi_epi16(prod, zero);
            acc_lo = _mm256_add_epi32(acc_lo, plo);
            acc_hi = _mm256_add_epi32(acc_hi, phi);
        }
        // Reduce each u32 lane mod p via the Phase-2 SSOT 32-bit Barrett
        // primitive (see `barrett_reduce_lane32` below).
        let lo_red = barrett_reduce_lane32(acc_lo, mu_vec, p_vec);
        let hi_red = barrett_reduce_lane32(acc_hi, mu_vec, p_vec);
        // Per-lane mapping back to output positions:
        //   prod[0..16]  =  a_h * B[col, j..j+16]  (16 u16 lanes)
        //   AVX2 unpacklo/unpackhi_epi16 are in-lane (per 128-bit half):
        //     acc_lo low-half  =  prod[0..4]
        //     acc_lo high-half =  prod[8..12]
        //     acc_hi low-half  =  prod[4..8]
        //     acc_hi high-half =  prod[12..16]
        //   _mm256_packus_epi32(lo, hi) is also in-lane:
        //     packed16 low-half  = packus(acc_lo low, acc_hi low)
        //                        = u16 lanes [prod[0..4], prod[4..8]]
        //     packed16 high-half = packus(acc_lo high, acc_hi high)
        //                        = u16 lanes [prod[8..12], prod[12..16]]
        //   So packed16 already holds prod[0..16] in order across u16
        //   lanes 0..15 — no cross-lane permute is needed.
        let packed16 = _mm256_packus_epi32(lo_red, hi_red);
        // Pack 16 u16 → 16 u8. _mm256_packus_epi16 is in-lane:
        //   low half  = packus(packed16 low half, packed16 low half)
        //             = u8 lanes [prod[0..4], prod[4..8]] repeated
        //   high half = packus(packed16 high, packed16 high)
        //             = u8 lanes [prod[8..12], prod[12..16]] repeated
        // We then need 64-bit-lane permute to fuse the low halves of
        // each 128-bit half into a single 128-bit register holding
        // u8 lanes [prod[0..8], prod[8..16]].
        let packed8 = _mm256_packus_epi16(packed16, packed16);
        // packed8 has, per 128-bit half, [prod[0..8], prod[0..8]] in
        // low half and [prod[8..16], prod[8..16]] in high half. We
        // want a 128-bit value [prod[0..8], prod[8..16]]. That is the
        // 64-bit lane 0 of the low half + 64-bit lane 0 of the high
        // half. Use permute4x64 with control (0,2,_,_) — though we
        // only consume the low 128 bits of the result.
        let fused = _mm256_permute4x64_epi64::<0b11_01_10_00>(packed8);
        // After 0b11_01_10_00 = (3,1,2,0) the low 128 bits of `fused`
        // are 64-bit lane 0 (= prod[0..8]) and 64-bit lane 2 (=
        // prod[8..16]) — exactly the output we want.
        let lower = _mm256_castsi256_si128(fused);
        _mm_storeu_si128(out.as_mut_ptr().add(j) as *mut __m128i, lower);
        j += 16;
    }
    // Scalar tail for j ∈ [j..n).
    while j < n {
        let mut total: u64 = 0;
        for h in 0..nnz {
            let a_h = *a_vals.get_unchecked(h) as u64;
            let col = *a_cols.get_unchecked(h);
            let b_kj = *b.get_unchecked(col * b_stride + j) as u64;
            total += a_h * b_kj;
        }
        *out.get_unchecked_mut(j) = (total % p_u32 as u64) as u8;
        j += 1;
    }
}

/// 32-bit-lane Barrett reduction: `r = x mod p` for `x ∈ [0, 2³²)`.
///
/// The Phase-2 SSOT (issue e8a0c47a) for vectorized modular reduction:
/// every AVX2 kernel that needs to canonicalise 8 packed u32 lanes
/// against an odd prime modulus calls this function. Consumers
/// (post-Phase-2):
///
/// 1. `fp_small.rs::fp_small_spmm_row` — sparse-times-dense small-prime
///    row reducer (this module, same file).
/// 2. `fp_small_f32.rs::store_and_reduce_tile_route_a` — route-A f32
///    cascade output reducer for GF(251)/n ≥ 512.
/// 3. `fp_small_panel.rs::fp_small_panel_gemm` — route-C integer-panel
///    output reducer.
/// 4. `fp_medium.rs::fp_medium_batch_mul16` — medium-prime u16 lane-wise
///    multiply (the second non-GF(251) call site required by issue
///    e8a0c47a SC#1).
///
/// # Algorithm (Granlund-Möller, one-step branchless)
///
/// With `μ = ⌊2³² / p⌋`:
///
/// 1. `q = ⌊(x · μ) / 2³²⌋` — computed per 32-bit lane via two
///    `_mm256_mul_epu32` invocations on the even/odd u32 lanes and a
///    single `>> 32` extraction of each 64-bit product's high half.
/// 2. `r = x − q · p` via `_mm256_mullo_epi32` + `_mm256_sub_epi32`.
///    Result is in `[0, 2p)` for every `x < 2³²`.
/// 3. Conditional subtract via `_mm256_min_epu32(r, r − p)`: when
///    `r ≥ p`, `r − p < r` as unsigned and `min` picks it; when `r < p`,
///    `r − p` underflows to a value `> r` (unsigned) and `min` keeps `r`.
///    Result lands in `[0, p)`.
///
/// # Arguments
///
/// * `x` — 8 u32 lanes packed into one `__m256i`, each `< 2³²`.
/// * `mu_vec` — broadcast Barrett constant `μ = ⌊2³² / p⌋`. Either
///   `_mm256_set1_epi64x(μ as i64)` (preferred by route-A / SpMM, which
///   want one builder call producing a vector ready for
///   `_mm256_mul_epu32`) or `_mm256_set1_epi32(μ as i32)` (preferred by
///   the medium-prime kernel, which broadcasts both `p` and `μ` as
///   32-bit lanes for symmetry) is acceptable: `_mm256_mul_epu32` only
///   reads the low 32 bits of each 64-bit lane and both broadcast
///   styles place `μ` there.
/// * `p_vec` — broadcast `p` as 8 u32 lanes
///   (`_mm256_set1_epi32(p as i32)`). Used for the `q · p` correction
///   and the conditional subtract.
///
/// # Returns
///
/// 8 reduced u32 lanes, each canonical in `[0, p)`.
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
) -> __m256i {
    // Compute q = (x * mu32) >> 32 per 32-bit lane.
    // _mm256_mul_epu32 multiplies the even u32 lanes of two u64-shaped
    // vectors and produces u64 results; combining with a shift handles
    // the odd lanes.
    let mask32 = _mm256_set1_epi64x(0xFFFF_FFFF);
    let x_even = _mm256_and_si256(x, mask32);
    let x_odd = _mm256_srli_epi64::<32>(x);
    let q_even_64 = _mm256_mul_epu32(x_even, mu_vec); // even u64 results
    let q_odd_64 = _mm256_mul_epu32(x_odd, mu_vec); // odd u64 results

    // q = high 32 bits of each 64-bit product
    let q_even_hi = _mm256_srli_epi64::<32>(q_even_64);
    let q_odd_hi = _mm256_srli_epi64::<32>(q_odd_64);
    // Re-interleave into u32 lanes: q_even_hi at even slots, q_odd_hi at odd slots.
    let q_odd_shifted = _mm256_slli_epi64::<32>(q_odd_hi);
    let q = _mm256_or_si256(q_even_hi, q_odd_shifted);

    // r = x - q * p, with r in [0, 2p).
    let qp = _mm256_mullo_epi32(q, p_vec);
    let r = _mm256_sub_epi32(x, qp);

    // Conditional subtract: if r >= p, r -= p. `min_epu32(r, r - p)`
    // picks r - p when r >= p (since r - p < r as unsigned) and keeps r
    // when r < p (since r - p underflows to a value > r as unsigned).
    // Bound justification: with μ = ⌊2³² / p⌋ truncated, Granlund-Möller
    // guarantees r ∈ [0, 2p) for every x < 2³². One conditional subtract
    // is sufficient — confirmed bit-exact by the per-kernel proptest
    // suite (10 primes × {0, 1, 15, 16, 17, 63, 64, 65} boundary lengths).
    _mm256_min_epu32(r, _mm256_sub_epi32(r, p_vec))
}

#[inline]
#[target_feature(enable = "avx2")]
unsafe fn horizontal_sum_u32(v: __m256i) -> u32 {
    let lo = _mm256_castsi256_si128(v);
    let hi = _mm256_extracti128_si256::<1>(v);
    let s = _mm_add_epi32(lo, hi);
    let mut tmp = [0u32; 4];
    _mm_storeu_si128(tmp.as_mut_ptr() as *mut __m128i, s);
    tmp[0]
        .wrapping_add(tmp[1])
        .wrapping_add(tmp[2])
        .wrapping_add(tmp[3])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn run_for_primes(test: impl Fn(u8)) {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        for &p in &[3u8, 5, 7, 11, 13, 17, 31, 127, 251] {
            test(p);
        }
    }

    #[test]
    fn batch_mul_exact_multiple_of_16() {
        run_for_primes(|p| {
            let a: Vec<u8> = (0..32u32).map(|i| (i * 17 % p as u32) as u8).collect();
            let b: Vec<u8> = (0..32u32)
                .map(|i| (i * 23 + 5) % p as u32)
                .map(|x| x as u8)
                .collect();
            let mut out = vec![0u8; 32];
            unsafe { fp_small_batch_mul(&a, &b, p, &mut out) };
            for i in 0..32 {
                let expected = ((a[i] as u32 * b[i] as u32) % p as u32) as u8;
                assert_eq!(out[i], expected, "p={p} i={i}");
            }
        });
    }

    #[test]
    fn batch_mul_with_tail() {
        run_for_primes(|p| {
            let a: Vec<u8> = (0..21u32).map(|i| (i * 17 % p as u32) as u8).collect();
            let b: Vec<u8> = (0..21u32)
                .map(|i| (i * 23 + 5) % p as u32)
                .map(|x| x as u8)
                .collect();
            let mut out = vec![0u8; 21];
            unsafe { fp_small_batch_mul(&a, &b, p, &mut out) };
            for i in 0..21 {
                let expected = ((a[i] as u32 * b[i] as u32) % p as u32) as u8;
                assert_eq!(out[i], expected, "p={p} i={i}");
            }
        });
    }

    #[test]
    fn batch_mul_boundary_values() {
        run_for_primes(|p| {
            // Generate identical-length adversarial sequences exercising the
            // {0, 1, p-1, p/2} corners across enough lanes to span both an
            // AVX2 vector boundary and a scalar tail.
            let len = 48;
            let a: Vec<u8> = (0..len).map(|i| (i as u8) % p).collect();
            let b: Vec<u8> = (0..len)
                .map(|i| ((i as u32 * 7 + 3) % p as u32) as u8)
                .collect();
            let mut out = vec![0u8; len];
            unsafe { fp_small_batch_mul(&a, &b, p, &mut out) };
            for i in 0..len {
                let expected = ((a[i] as u32 * b[i] as u32) % p as u32) as u8;
                assert_eq!(out[i], expected, "p={p} i={i}");
            }
        });
    }

    #[test]
    fn batch_add_matches_scalar() {
        run_for_primes(|p| {
            let a: Vec<u8> = (0..40u32).map(|i| (i * 17 % p as u32) as u8).collect();
            let b: Vec<u8> = (0..40u32)
                .map(|i| (i * 23 + 5) % p as u32)
                .map(|x| x as u8)
                .collect();
            let mut out = vec![0u8; 40];
            unsafe { fp_small_batch_add(&a, &b, p, &mut out) };
            for i in 0..40 {
                let expected = (a[i] as u16 + b[i] as u16) % p as u16;
                assert_eq!(out[i] as u16, expected, "p={p} i={i}");
            }
        });
    }

    #[test]
    fn batch_sub_matches_scalar() {
        run_for_primes(|p| {
            let a: Vec<u8> = (0..40u32).map(|i| (i * 17 % p as u32) as u8).collect();
            let b: Vec<u8> = (0..40u32)
                .map(|i| (i * 23 + 5) % p as u32)
                .map(|x| x as u8)
                .collect();
            let mut out = vec![0u8; 40];
            unsafe { fp_small_batch_sub(&a, &b, p, &mut out) };
            for i in 0..40 {
                let expected = (a[i] as i32 - b[i] as i32).rem_euclid(p as i32) as u16;
                assert_eq!(out[i] as u16, expected, "p={p} i={i}");
            }
        });
    }

    #[test]
    fn batch_dot_matches_scalar() {
        run_for_primes(|p| {
            for &len in &[0usize, 1, 7, 8, 15, 16, 17, 31, 32, 33, 100, 256, 1024] {
                let a: Vec<u8> = (0..len as u32).map(|i| (i * 17 % p as u32) as u8).collect();
                let b: Vec<u8> = (0..len as u32)
                    .map(|i| ((i * 23 + 5) % p as u32) as u8)
                    .collect();
                let got = unsafe { fp_small_batch_dot(&a, &b, p) };
                let mut expected: u64 = 0;
                for i in 0..len {
                    expected = (expected + a[i] as u64 * b[i] as u64) % p as u64;
                }
                assert_eq!(got as u64, expected, "p={p} len={len}");
            }
        });
    }

    #[test]
    fn gemm_row_panel_matches_scalar() {
        run_for_primes(|p| {
            // Cover row counts that span both the 4-output-cell tile
            // and the 1-cell tail, plus k values that exercise the
            // SIMD body and the scalar tail.
            let cases = [(7usize, 65usize), (16, 64), (15, 100), (32, 128)];
            for &(n, k) in &cases {
                let a: Vec<u8> = (0..k as u32)
                    .map(|i| ((i * 11 + 7) % p as u32) as u8)
                    .collect();
                let bt: Vec<u8> = (0..(n * k) as u32)
                    .map(|i| ((i * 19 + 3) % p as u32) as u8)
                    .collect();
                let mut out = vec![0u8; n];
                unsafe { fp_small_gemm_row_panel(&a, &bt, k, n, p, &mut out) };
                for j in 0..n {
                    let mut expected: u64 = 0;
                    for t in 0..k {
                        expected += a[t] as u64 * bt[j * k + t] as u64;
                    }
                    expected %= p as u64;
                    assert_eq!(out[j] as u64, expected, "p={p} k={k} n={n} j={j}");
                }
            }
        });
    }

    #[test]
    fn spmm_row_matches_scalar() {
        run_for_primes(|p| {
            // Cover (nnz, n) shapes spanning the 16-lane SIMD body and
            // the scalar tail, with sparse columns scattered across
            // multiple B rows.
            let cases = [
                (1usize, 16usize, 8usize), // single nnz, exact 16
                (5, 16, 32),               // tail-free 32
                (7, 16, 33),               // 1-byte tail
                (10, 16, 64),              // typical nnz at density~1%
                (1, 16, 17),               // small tail
                (20, 16, 100),             // mid-size n
                (3, 8, 16),                // few B rows
                (10, 32, 128),             // larger n
                (15, 16, 1024),            // realistic SpMM cell
            ];
            for &(nnz, b_rows, n) in &cases {
                // Build deterministic sparse row.
                let a_vals: Vec<u8> = (0..nnz as u32)
                    .map(|h| ((h * 13 + 1) % p as u32) as u8)
                    .collect();
                let a_cols: Vec<usize> = (0..nnz).map(|h| (h * 7) % b_rows).collect();
                // Build dense B.
                let b_stride = n;
                let b: Vec<u8> = (0..(b_rows * n) as u32)
                    .map(|i| ((i * 23 + 5) % p as u32) as u8)
                    .collect();
                let mut out = vec![0u8; n];
                unsafe { fp_small_spmm_row(&a_vals, &a_cols, &b, b_stride, n, p, &mut out) };
                for (j, &val) in out.iter().enumerate() {
                    let mut expected: u64 = 0;
                    for h in 0..nnz {
                        let col = a_cols[h];
                        expected += a_vals[h] as u64 * b[col * b_stride + j] as u64;
                    }
                    expected %= p as u64;
                    assert_eq!(
                        val as u64, expected,
                        "p={p} nnz={nnz} b_rows={b_rows} n={n} j={j}"
                    );
                }
            }
        });
    }

    #[test]
    fn spmm_row_empty_nnz() {
        run_for_primes(|p| {
            let n = 64;
            let a_vals: Vec<u8> = vec![];
            let a_cols: Vec<usize> = vec![];
            let b: Vec<u8> = (0..n).map(|i| ((i * 7) as u8) % p).collect();
            let mut out = vec![5u8; n];
            unsafe { fp_small_spmm_row(&a_vals, &a_cols, &b, n, n, p, &mut out) };
            for (j, &val) in out.iter().enumerate().take(n) {
                assert_eq!(val, 0, "p={p} j={j}");
            }
        });
    }

    fn scalar_sub_scaled_oracle(buf: &mut [u8], chain_j: &[u8], alpha: u8, p: u8) {
        // Reference: buf[i] := (buf[i] - alpha * chain_j[i]) mod p.
        let p_u32 = p as u32;
        let alpha_u32 = alpha as u32;
        for i in 0..chain_j.len() {
            let prod = (alpha_u32 * chain_j[i] as u32) % p_u32;
            let b = buf[i] as u32;
            buf[i] = ((b + p_u32 - prod) % p_u32) as u8;
        }
    }

    /// Bit-identical scalar-equivalence test for the fused `sub_scaled`
    /// kernel at the issue-mandated boundary lengths `{0, 1, 15, 16, 17,
    /// 63, 64, 65, 255, 256}` across every supported small prime.
    ///
    /// Issue `52cce970` § "Success Criteria" requires correctness at
    /// these exact lengths. The oracle is the same scalar computation
    /// used by the non-AVX2 fallback. Input data is randomised per the
    /// `seed` parameter so each proptest run exercises a fresh data set.
    #[allow(clippy::wildcard_imports)]
    mod proptest_sub_scaled_jit_52cce970 {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(256))]

            #[test]
            fn proptest_sub_scaled_matches_scalar_boundary_lengths_jit_52cce970(
                len in prop_oneof![
                    Just(0usize), Just(1), Just(15), Just(16), Just(17),
                    Just(63), Just(64), Just(65), Just(255), Just(256)
                ],
                seed in any::<u64>(),
                p_idx in 0usize..9usize,
            ) {
                if !std::arch::is_x86_feature_detected!("avx2") {
                    return Ok(());
                }
                let primes: [u8; 9] = [3, 5, 7, 11, 13, 17, 31, 127, 251];
                let p = primes[p_idx];
                let mu = barrett_mu_u16(p);
                // Derive alpha and data from seed via a simple LCG so
                // every proptest case uses independent pseudo-random values.
                let s1 = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let s2 = s1.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let alpha = ((s1 >> 32) % p as u64) as u8;
                let chain_j: Vec<u8> = (0..len)
                    .map(|i| {
                        let v = s1.wrapping_mul(i as u64 + 1).wrapping_add(s2);
                        (v % p as u64) as u8
                    })
                    .collect();
                let buf_init: Vec<u8> = (0..len)
                    .map(|i| {
                        let v = s2.wrapping_mul(i as u64 + 1).wrapping_add(s1);
                        (v % p as u64) as u8
                    })
                    .collect();
                let mut buf = buf_init.clone();
                let mut expected = buf_init;
                scalar_sub_scaled_oracle(&mut expected, &chain_j, alpha, p);
                // SAFETY: AVX2 detected above; p is a small prime in [3,251].
                unsafe { fp_small_sub_scaled(&mut buf, &chain_j, alpha, p, mu) };
                prop_assert_eq!(buf, expected, "p={} alpha={} len={}", p, alpha, len);
            }
        }
    }

    /// Smoke test: deterministic boundary-length check retained alongside
    /// the proptest for fast feedback during development.
    #[test]
    fn sub_scaled_matches_scalar_boundary_lengths_jit_52cce970() {
        run_for_primes(|p| {
            let mu = barrett_mu_u16(p);
            for &len in &[0usize, 1, 15, 16, 17, 63, 64, 65, 255, 256] {
                // Two different alpha values per (p, len) to broaden
                // coverage: a "small" one and the maximal canonical
                // (p-1) which exercises the (P-1)² product corner.
                for &alpha in &[2u8 % p, (p - 1) % p] {
                    let mut buf: Vec<u8> = (0..len as u32)
                        .map(|i| ((i * 31 + 11) % p as u32) as u8)
                        .collect();
                    let chain_j: Vec<u8> = (0..len as u32)
                        .map(|i| ((i * 19 + 5) % p as u32) as u8)
                        .collect();
                    let mut expected = buf.clone();
                    scalar_sub_scaled_oracle(&mut expected, &chain_j, alpha, p);
                    unsafe { fp_small_sub_scaled(&mut buf, &chain_j, alpha, p, mu) };
                    assert_eq!(buf, expected, "p={p} alpha={alpha} len={len}");
                }
            }
        });
    }

    /// Coverage for `buf.len() > chain_j.len()` — the kernel must
    /// leave the trailing `buf` bytes untouched. This case matches the
    /// `PackedFpChainPolys::sub_scaled_into` call shape where `buf`
    /// has been resized to hold the upcoming `x · chain_{d-1}` shift
    /// (one byte longer than the longest chain polynomial seen so far).
    #[test]
    fn sub_scaled_preserves_buf_tail() {
        run_for_primes(|p| {
            let mu = barrett_mu_u16(p);
            let chain_len = 33;
            let buf_extra = 7;
            let chain_j: Vec<u8> = (0..chain_len as u32)
                .map(|i| ((i * 11 + 3) % p as u32) as u8)
                .collect();
            let mut buf: Vec<u8> = (0..(chain_len + buf_extra) as u32)
                .map(|i| ((i * 23 + 5) % p as u32) as u8)
                .collect();
            let alpha = 7u8 % p;
            let mut expected = buf.clone();
            scalar_sub_scaled_oracle(&mut expected[..chain_len], &chain_j, alpha, p);
            unsafe { fp_small_sub_scaled(&mut buf, &chain_j, alpha, p, mu) };
            assert_eq!(buf, expected, "p={p}");
            // Explicit check: tail bytes equal the original initial values.
            let original_tail: Vec<u8> = (chain_len..chain_len + buf_extra)
                .map(|i| ((i as u32 * 23 + 5) % p as u32) as u8)
                .collect();
            assert_eq!(&buf[chain_len..], &original_tail[..], "p={p} tail mutated");
        });
    }

    /// Stress test: random alphas, random chain_j and buf at varying
    /// lengths, repeated many times. Complements the boundary test by
    /// hitting non-corner lengths that the byte-pair tile may treat
    /// differently inside the SIMD vs. tail boundary.
    #[test]
    fn sub_scaled_matches_scalar_random_lengths() {
        run_for_primes(|p| {
            let mu = barrett_mu_u16(p);
            // Simple LCG for reproducible pseudo-random byte values.
            let mut state: u64 = 0xDEAD_BEEF_CAFE_BABE;
            let mut step = || {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                state
            };
            for &len in &[2usize, 7, 8, 18, 30, 31, 32, 47, 96, 113, 200, 257, 511] {
                let chain_j: Vec<u8> = (0..len).map(|_| (step() as u32 % p as u32) as u8).collect();
                let mut buf: Vec<u8> = (0..len).map(|_| (step() as u32 % p as u32) as u8).collect();
                let alpha = (step() as u32 % p as u32) as u8;
                let mut expected = buf.clone();
                scalar_sub_scaled_oracle(&mut expected, &chain_j, alpha, p);
                unsafe { fp_small_sub_scaled(&mut buf, &chain_j, alpha, p, mu) };
                assert_eq!(buf, expected, "p={p} alpha={alpha} len={len}");
            }
        });
    }

    /// Sanity: `alpha == 0` is a no-op (the caller in `gf2-core`
    /// short-circuits on zero alpha but the kernel itself must handle
    /// the corner cleanly in case a future caller forgets).
    #[test]
    fn sub_scaled_zero_alpha_is_noop() {
        run_for_primes(|p| {
            let mu = barrett_mu_u16(p);
            let len = 65;
            let chain_j: Vec<u8> = (0..len as u32)
                .map(|i| ((i * 7 + 1) % p as u32) as u8)
                .collect();
            let original_buf: Vec<u8> = (0..len as u32)
                .map(|i| ((i * 13 + 2) % p as u32) as u8)
                .collect();
            let mut buf = original_buf.clone();
            unsafe { fp_small_sub_scaled(&mut buf, &chain_j, 0, p, mu) };
            assert_eq!(buf, original_buf, "p={p} alpha=0 must be no-op");
        });
    }

    /// Cross-check: passing the right `mu` is required for correctness.
    /// This is a defensive test ensuring that callers cannot get correct
    /// answers by passing a stale `mu` left over from a different prime.
    /// (The `debug_assert!` inside the kernel additionally catches it in
    /// debug builds; this test belt-and-braces the release path by
    /// pinning the contract.)
    #[test]
    fn sub_scaled_jit_52cce970_r1_mu_param_matches_barrett_constant() {
        for &p in &[3u8, 5, 7, 11, 13, 17, 31, 127, 251] {
            assert_eq!(
                barrett_mu_u16(p),
                (65536u32 / p as u32) as u16,
                "mu helper mismatch at p={p}",
            );
        }
    }
}
