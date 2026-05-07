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
    // mu32 = ⌊2³² / p⌋. We use _mm256_mul_epu32 to handle 32-bit-lane
    // multiplication via 64-bit-lane intermediate.
    let mu32 = ((1u64 << 32) / p_u32 as u64) as u32;
    let mu_vec = _mm256_set1_epi64x(mu32 as i64);
    let p_vec64 = _mm256_set1_epi64x(p_u32 as i64);

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
        // Reduce each u32 lane mod p via 32-bit Barrett:
        //   q = (x * mu32) >> 32; r = x - q*p; if (r >= p) r -= p.
        let lo_red = barrett_reduce_lane32(acc_lo, mu_vec, p_vec, p_vec64);
        let hi_red = barrett_reduce_lane32(acc_hi, mu_vec, p_vec, p_vec64);
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

/// 32-bit-lane Barrett reduction: `r = x mod p` for `x ∈ [0, 2³²)` and
/// `p ≤ 251`. Returns reduced 32-bit lanes still in u32 form.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn barrett_reduce_lane32(
    x: __m256i,
    mu_vec: __m256i,
    p_vec: __m256i,
    p_vec64: __m256i,
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

    // r = x - q * p
    let qp = _mm256_mullo_epi32(q, p_vec);
    let mut r = _mm256_sub_epi32(x, qp);
    // Conditional subtract: if r >= p, r -= p. We use a single subtract
    // followed by add-back with a mask. r2 = r - p; if r2 < 0, take r;
    // else take r2.
    let r2 = _mm256_sub_epi32(r, p_vec);
    // r2 < 0 (signed) means r < p; we want r in that case.
    // mask = (r2 < 0) ? -1 : 0
    let mask_lt = _mm256_cmpgt_epi32(_mm256_setzero_si256(), r2);
    // Final: blend r2 (when r >= p) with r (when r < p).
    r = _mm256_blendv_epi8(r2, r, mask_lt);
    // Some bounds may still leave r ≥ p if accumulator was very large
    // (Barrett one-step bounds: r < 2p strictly only for x < p². For
    // accumulator with nnz_r * (p-1)² up to 2³² we need a second
    // step.)
    // Worst case: x < 2³², q = ⌊x * mu / 2³²⌋ ≤ ⌊x / p⌋, x - q*p ∈ [0, 2p).
    // Reference: Granlund-Möller: with mu = ⌊2³² / p⌋, x = q*p + r, r ∈ [0, p).
    // The shift+subtract gives r ∈ [0, 2p). One conditional subtract suffices.
    // But because mu has been truncated, r can be in [0, p + small slack); the
    // standard form r ∈ [0, 2p) is safe.
    let _ = p_vec64; // unused but kept for symmetry
    r
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
}
