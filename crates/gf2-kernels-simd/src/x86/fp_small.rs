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
const fn barrett_mu_u16(p: u8) -> u16 {
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
            let b: Vec<u8> = (0..len).map(|i| (i as u8 * 7 + 3) % p).collect();
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
}
