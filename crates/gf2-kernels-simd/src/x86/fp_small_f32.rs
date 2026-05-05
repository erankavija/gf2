//! AVX2 + FMA3 f32-cascade GEMM micro-kernel for small `Fp<P>` with
//! `P <= 251` (Candidate F per
//! `dev/plans/small_prime_kernel_strategy.md` § 4.5 / § 5.5 / § 6.1).
//!
//! Inputs and outputs are canonical bytes (`u8`, value `< P`); all
//! arithmetic happens through f32 lanes via `_mm256_fmadd_ps`. The
//! kernel is structured as a BLIS-class register-blocked sgemm
//! micro-kernel:
//!
//! - **Pack pass.** Convert `a: &[u8]` (m × k row-major) into
//!   `a_packed: Vec<f32>` of the same shape, and `bt: &[u8]` (n × k
//!   row-major) into a column-major *N-major panel* `b_packed`
//!   suitable for the inner FMA tile (each `n_R = 24`-wide panel is
//!   `k` rows × 24 cols, contiguous).
//! - **Inner micro-kernel.** A `4 × 24` tile (`m_R = 4`, `n_R = 24`)
//!   uses 12 accumulator AVX2 registers, 1 broadcast register for
//!   the `a` lane, and 3 register slots for the `b` row tiles —
//!   exhausting the 16-register file by design. Each inner-`k` step
//!   issues 12 `_mm256_fmadd_ps` instructions; on Zen-3 they pipeline
//!   across the two FMA execution ports at 0.5-cycle reciprocal
//!   throughput (Agner Fog's Zen-3 instruction tables).
//! - **Reduction.** At each `k_chunk` boundary the f32 accumulator
//!   tile is rounded to nearest integer, converted to `i32` SIMD
//!   lanes, and added into a 12-vector i32 running sum kept across
//!   all chunks. Only the final tile-end pass runs the scalar `% p`
//!   per output cell. The chunk size is
//!   `k_chunk = min(k, k_max(p), K_CHUNK_CAP)` where
//!   `k_max(p) = floor(2^24 / (p-1)²)` keeps the running f32 sum
//!   inside the exact-integer range, and `K_CHUNK_CAP = 256` keeps
//!   each B-panel slice (`k_chunk · N_R · 4 = 24 KB`) inside
//!   Zen-3's 32 KB L1d.
//!
//! # Safety
//!
//! All public functions here are `unsafe` — callers must ensure
//! AVX2 + FMA3 are both available at runtime. Safe, dispatched entry
//! points live in `fp_small_f32.rs` via the `SmallPrimeF32Fns` table
//! returned by `detect`.

#![allow(clippy::missing_safety_doc)]

use core::arch::x86_64::*;

/// Inner `m × n` register-tile dimensions.
const M_R: usize = 4;
const N_R: usize = 24;

/// Whole-gemm AVX2 + FMA3 f32-cascade kernel for small primes.
///
/// Computes `c[i*n + j] = (∑_t a[i*k + t] · bt[j*k + t]) mod p` for
/// every `(i, j) ∈ [0, m) × [0, n)`. `bt` is the row-major transpose
/// of the right operand (length `n * k`, so row `j` holds column `j`
/// of B).
///
/// # Safety
///
/// Caller must ensure AVX2 and FMA3 are both available at runtime,
/// `p ∈ [3, 251]` is an odd prime, and every input byte is canonical
/// (`< p`).
///
/// # Panics
///
/// Panics if any slice length disagrees with `m`, `k`, `n`.
#[target_feature(enable = "avx2,fma")]
pub unsafe fn fp_small_f32_gemm(
    a: &[u8],
    bt: &[u8],
    m: usize,
    k: usize,
    n: usize,
    p: u8,
    c: &mut [u8],
) {
    assert_eq!(a.len(), m * k, "fp_small_f32_gemm: a shape");
    assert_eq!(bt.len(), n * k, "fp_small_f32_gemm: bt shape");
    assert_eq!(c.len(), m * n, "fp_small_f32_gemm: c shape");

    if m == 0 || k == 0 || n == 0 {
        // c is already the m×n zero matrix because the caller
        // passes a zero-initialised buffer (per the gemm contract);
        // nothing to do.
        return;
    }

    // ── Pack A (m × k) into row-major f32. ────────────────────────
    let a_packed: Vec<f32> = a.iter().map(|&v| v as f32).collect();

    // ── Pack B-transpose (n × k row-major: row j == column j of B)
    //    into N-major panels of width N_R, each `k × N_R` row-major.
    //    For each n-panel `j_blk = 0, N_R, 2*N_R, ...`, we need
    //    `b_packed[panel_offset + t*N_R + j_off] = B[t, j_blk + j_off]
    //                                            = bt[(j_blk + j_off)*k + t]`.
    //    For the partial trailing panel (`n % N_R != 0`), unused
    //    lanes are filled with 0.0 so the FMA accumulates a zero
    //    (semantically harmless; the unused output cells are not
    //    read at unpack time).
    let n_panels = n.div_ceil(N_R);
    let panel_stride = k * N_R;
    let mut b_packed: Vec<f32> = vec![0.0f32; n_panels * panel_stride];
    // Outer loop over t so the inner write is the contiguous N_R-wide
    // row of the panel; this keeps writes streaming and avoids the
    // 96-byte stride that would otherwise be on the inner axis.
    for panel_idx in 0..n_panels {
        let j_blk = panel_idx * N_R;
        let j_end = (j_blk + N_R).min(n);
        let n_eff = j_end - j_blk;
        let panel_off = panel_idx * panel_stride;
        for t in 0..k {
            let dst_row_off = panel_off + t * N_R;
            for j_off in 0..n_eff {
                // Source: bt[(j_blk + j_off) * k + t] — strided read,
                // but cache lines are touched in column-major order
                // so the prefetcher can stream them.
                b_packed[dst_row_off + j_off] = bt[(j_blk + j_off) * k + t] as f32;
            }
        }
    }

    let p_i32 = p as i32;

    // ── Per-prime k_max ───────────────────────────────────────────
    //
    // The largest number of `(p-1)²`-magnitude FMA accumulations a
    // single f32 register can absorb without leaving the exact-integer
    // range `[0, 2^24]`.
    //
    //   k_max(p) = floor(2^24 / (p-1)²)
    //
    // For `p = 7`: k_max = 466 033, but we cap at 4096 (the issue's
    // `K_CHUNK` ceiling).
    // For `p = 31`: k_max = 18 631 (capped at 4096).
    // For `p = 251`: k_max = 268.
    let k_max = compute_k_max(p);
    let k_chunk = k_max.min(K_CHUNK_CAP);

    // ── Inner GEMM loop. ──────────────────────────────────────────
    let mut i_blk = 0usize;
    while i_blk < m {
        let i_end = (i_blk + M_R).min(m);
        let m_eff = i_end - i_blk;

        for panel_idx in 0..n_panels {
            let j_blk = panel_idx * N_R;
            let j_end = (j_blk + N_R).min(n);
            let n_eff = j_end - j_blk;
            let panel_off = panel_idx * panel_stride;

            // i32 SIMD accumulators (12 vectors covering the 4 × 24 tile).
            // Each lane sums the rounded f32 chunk contributions across
            // all `k / k_chunk` chunks; the i32 range absorbs
            // `k · (p-1)² ≤ 4096 · 250² = 256M < 2^31`.
            let mut sum00 = _mm256_setzero_si256();
            let mut sum01 = _mm256_setzero_si256();
            let mut sum02 = _mm256_setzero_si256();
            let mut sum10 = _mm256_setzero_si256();
            let mut sum11 = _mm256_setzero_si256();
            let mut sum12 = _mm256_setzero_si256();
            let mut sum20 = _mm256_setzero_si256();
            let mut sum21 = _mm256_setzero_si256();
            let mut sum22 = _mm256_setzero_si256();
            let mut sum30 = _mm256_setzero_si256();
            let mut sum31 = _mm256_setzero_si256();
            let mut sum32 = _mm256_setzero_si256();

            // k-chunked FMA loop: each chunk holds 12 f32 accumulators
            // alive in the register file. At chunk end we round-and-cast
            // to i32 and add into the 12 i32 SIMD accumulators above, so
            // the costly per-lane scalar % p is paid only ONCE at the
            // very end of the k axis (not once per chunk).
            let mut t_blk = 0usize;
            while t_blk < k {
                let t_end = (t_blk + k_chunk).min(k);

                let mut acc00 = _mm256_setzero_ps();
                let mut acc01 = _mm256_setzero_ps();
                let mut acc02 = _mm256_setzero_ps();
                let mut acc10 = _mm256_setzero_ps();
                let mut acc11 = _mm256_setzero_ps();
                let mut acc12 = _mm256_setzero_ps();
                let mut acc20 = _mm256_setzero_ps();
                let mut acc21 = _mm256_setzero_ps();
                let mut acc22 = _mm256_setzero_ps();
                let mut acc30 = _mm256_setzero_ps();
                let mut acc31 = _mm256_setzero_ps();
                let mut acc32 = _mm256_setzero_ps();

                for t in t_blk..t_end {
                    let b_ptr = b_packed.as_ptr().add(panel_off + t * N_R);
                    let b0 = _mm256_loadu_ps(b_ptr);
                    let b1 = _mm256_loadu_ps(b_ptr.add(8));
                    let b2 = _mm256_loadu_ps(b_ptr.add(16));

                    // 12 FMAs per inner iteration; on Zen-3 the two FMA
                    // ports each retire one per cycle, so the inner
                    // body issues at 6 cycles per (m_R, t) step at peak.
                    if m_eff >= 1 {
                        let a0 = _mm256_set1_ps(*a_packed.get_unchecked(i_blk * k + t));
                        acc00 = _mm256_fmadd_ps(b0, a0, acc00);
                        acc01 = _mm256_fmadd_ps(b1, a0, acc01);
                        acc02 = _mm256_fmadd_ps(b2, a0, acc02);
                    }
                    if m_eff >= 2 {
                        let a1 = _mm256_set1_ps(*a_packed.get_unchecked((i_blk + 1) * k + t));
                        acc10 = _mm256_fmadd_ps(b0, a1, acc10);
                        acc11 = _mm256_fmadd_ps(b1, a1, acc11);
                        acc12 = _mm256_fmadd_ps(b2, a1, acc12);
                    }
                    if m_eff >= 3 {
                        let a2 = _mm256_set1_ps(*a_packed.get_unchecked((i_blk + 2) * k + t));
                        acc20 = _mm256_fmadd_ps(b0, a2, acc20);
                        acc21 = _mm256_fmadd_ps(b1, a2, acc21);
                        acc22 = _mm256_fmadd_ps(b2, a2, acc22);
                    }
                    if m_eff >= 4 {
                        let a3 = _mm256_set1_ps(*a_packed.get_unchecked((i_blk + 3) * k + t));
                        acc30 = _mm256_fmadd_ps(b0, a3, acc30);
                        acc31 = _mm256_fmadd_ps(b1, a3, acc31);
                        acc32 = _mm256_fmadd_ps(b2, a3, acc32);
                    }
                }

                // Round-and-cast all live f32 accumulators to i32 SIMD
                // and add into the running i32 SIMD sums. The round +
                // cvtps2dq pair is ~6 cycles on Zen-3 per accumulator,
                // and the `_mm256_add_epi32` is a 1-cycle instruction.
                sum00 = _mm256_add_epi32(sum00, round_ps_to_epi32(acc00));
                sum01 = _mm256_add_epi32(sum01, round_ps_to_epi32(acc01));
                sum02 = _mm256_add_epi32(sum02, round_ps_to_epi32(acc02));
                if m_eff >= 2 {
                    sum10 = _mm256_add_epi32(sum10, round_ps_to_epi32(acc10));
                    sum11 = _mm256_add_epi32(sum11, round_ps_to_epi32(acc11));
                    sum12 = _mm256_add_epi32(sum12, round_ps_to_epi32(acc12));
                }
                if m_eff >= 3 {
                    sum20 = _mm256_add_epi32(sum20, round_ps_to_epi32(acc20));
                    sum21 = _mm256_add_epi32(sum21, round_ps_to_epi32(acc21));
                    sum22 = _mm256_add_epi32(sum22, round_ps_to_epi32(acc22));
                }
                if m_eff >= 4 {
                    sum30 = _mm256_add_epi32(sum30, round_ps_to_epi32(acc30));
                    sum31 = _mm256_add_epi32(sum31, round_ps_to_epi32(acc31));
                    sum32 = _mm256_add_epi32(sum32, round_ps_to_epi32(acc32));
                }

                t_blk = t_end;
            }

            // Final reduction: store the 12 i32 SIMD sums to a stack
            // buffer and reduce modulo p once per output cell.
            store_and_reduce_tile(
                sum00, sum01, sum02, sum10, sum11, sum12, sum20, sum21, sum22, sum30, sum31, sum32,
                m_eff, n_eff, p_i32, i_blk, j_blk, n, c,
            );
        }

        i_blk += M_R;
    }
}

/// L1d-resident k-chunk cap. The k-chunk size is `min(k, k_max(p),
/// K_CHUNK_CAP)`.
///
/// Choice of 256: for the 4 × 24 tile, the B-panel slice consumed per
/// chunk is `K_CHUNK_CAP · N_R · 4 = 256 · 24 · 4 = 24 KB`, well
/// within Zen-3's 32 KB L1d. Larger chunks (e.g. 1024) would spill
/// the B-panel out of L1, causing per-tile read misses to L2 — the
/// dominant overhead at `n ≥ 1024` per `perf stat` profiling. Smaller
/// chunks issue more round-and-cast reductions (one per accumulator
/// per chunk boundary) but each is ~1 cycle on Zen-3 and amortises
/// across the inner kernel.
const K_CHUNK_CAP: usize = 256;

/// Computes the per-prime k_max — the largest number of `(p-1)²`
/// products an f32 accumulator can absorb without leaving the
/// exact-integer range `[0, 2^24]`.
#[inline]
fn compute_k_max(p: u8) -> usize {
    let p_minus_1 = (p as u32).saturating_sub(1);
    if p_minus_1 == 0 {
        return 1;
    }
    let max_product = (p_minus_1 as u64) * (p_minus_1 as u64);
    if max_product == 0 {
        return 1;
    }
    let k = (1u64 << 24) / max_product;
    k.max(1) as usize
}

/// Round-to-nearest f32 → i32 SIMD cast. Inputs in `[0, 2^24]` produce
/// exact integer i32 lanes.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn round_ps_to_epi32(v: __m256) -> __m256i {
    let rounded = _mm256_round_ps::<{ _MM_FROUND_TO_NEAREST_INT | _MM_FROUND_NO_EXC }>(v);
    _mm256_cvtps_epi32(rounded)
}

/// Store the 12 i32 SIMD accumulators of the `4 × 24` tile to scratch,
/// reduce modulo `p`, and write the canonical bytes into `c`.
#[allow(clippy::too_many_arguments)]
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn store_and_reduce_tile(
    sum00: __m256i,
    sum01: __m256i,
    sum02: __m256i,
    sum10: __m256i,
    sum11: __m256i,
    sum12: __m256i,
    sum20: __m256i,
    sum21: __m256i,
    sum22: __m256i,
    sum30: __m256i,
    sum31: __m256i,
    sum32: __m256i,
    m_eff: usize,
    n_eff: usize,
    p_i32: i32,
    i_blk: usize,
    j_blk: usize,
    n: usize,
    c: &mut [u8],
) {
    // Flat row-major scratch: 4 × 24 = 96 i32 lanes.
    let mut tile = [0i32; M_R * N_R];
    _mm256_storeu_si256(tile.as_mut_ptr() as *mut __m256i, sum00);
    _mm256_storeu_si256(tile.as_mut_ptr().add(8) as *mut __m256i, sum01);
    _mm256_storeu_si256(tile.as_mut_ptr().add(16) as *mut __m256i, sum02);
    if m_eff >= 2 {
        _mm256_storeu_si256(tile.as_mut_ptr().add(N_R) as *mut __m256i, sum10);
        _mm256_storeu_si256(tile.as_mut_ptr().add(N_R + 8) as *mut __m256i, sum11);
        _mm256_storeu_si256(tile.as_mut_ptr().add(N_R + 16) as *mut __m256i, sum12);
    }
    if m_eff >= 3 {
        _mm256_storeu_si256(tile.as_mut_ptr().add(2 * N_R) as *mut __m256i, sum20);
        _mm256_storeu_si256(tile.as_mut_ptr().add(2 * N_R + 8) as *mut __m256i, sum21);
        _mm256_storeu_si256(tile.as_mut_ptr().add(2 * N_R + 16) as *mut __m256i, sum22);
    }
    if m_eff >= 4 {
        _mm256_storeu_si256(tile.as_mut_ptr().add(3 * N_R) as *mut __m256i, sum30);
        _mm256_storeu_si256(tile.as_mut_ptr().add(3 * N_R + 8) as *mut __m256i, sum31);
        _mm256_storeu_si256(tile.as_mut_ptr().add(3 * N_R + 16) as *mut __m256i, sum32);
    }

    for i_off in 0..m_eff {
        for j_off in 0..n_eff {
            let v = tile[i_off * N_R + j_off].rem_euclid(p_i32);
            c[(i_blk + i_off) * n + (j_blk + j_off)] = v as u8;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn run_for_primes(test: impl Fn(u8)) {
        if !std::arch::is_x86_feature_detected!("avx2")
            || !std::arch::is_x86_feature_detected!("fma")
        {
            return;
        }
        for &p in &[3u8, 5, 7, 11, 13, 17, 31, 127, 251] {
            test(p);
        }
    }

    fn scalar_gemm(a: &[u8], bt: &[u8], m: usize, k: usize, n: usize, p: u8) -> Vec<u8> {
        let mut out = vec![0u8; m * n];
        for i in 0..m {
            for j in 0..n {
                let mut acc: u64 = 0;
                for t in 0..k {
                    acc += a[i * k + t] as u64 * bt[j * k + t] as u64;
                }
                out[i * n + j] = (acc % p as u64) as u8;
            }
        }
        out
    }

    #[test]
    fn gemm_matches_scalar_small_shapes() {
        run_for_primes(|p| {
            for &(m, k, n) in &[
                (1usize, 1usize, 1usize),
                (1, 1, 8),
                (1, 1, 24),
                (1, 1, 25),
                (4, 1, 24),
                (5, 1, 24),
                (1, 2, 24),
                (4, 64, 24),
                (8, 64, 48),
                (5, 65, 25),
                (4, 67, 17),
            ] {
                let a: Vec<u8> = (0..(m * k) as u32)
                    .map(|i| ((i * 17 + 1) % p as u32) as u8)
                    .collect();
                let bt: Vec<u8> = (0..(n * k) as u32)
                    .map(|i| ((i * 23 + 5) % p as u32) as u8)
                    .collect();
                let mut got = vec![0u8; m * n];
                unsafe { fp_small_f32_gemm(&a, &bt, m, k, n, p, &mut got) };
                let want = scalar_gemm(&a, &bt, m, k, n, p);
                assert_eq!(got, want, "p={p} m={m} k={k} n={n}");
            }
        });
    }

    #[test]
    fn gemm_matches_scalar_k_chunk_boundary() {
        // k near `K_CHUNK = 64`: exercise the chunk join.
        run_for_primes(|p| {
            for &k in &[63usize, 64, 65, 127, 128, 129, 134, 268, 512] {
                let m = 4;
                let n = 24;
                let a: Vec<u8> = (0..(m * k) as u32)
                    .map(|i| ((i * 17 + 1) % p as u32) as u8)
                    .collect();
                let bt: Vec<u8> = (0..(n * k) as u32)
                    .map(|i| ((i * 23 + 5) % p as u32) as u8)
                    .collect();
                let mut got = vec![0u8; m * n];
                unsafe { fp_small_f32_gemm(&a, &bt, m, k, n, p, &mut got) };
                let want = scalar_gemm(&a, &bt, m, k, n, p);
                assert_eq!(got, want, "p={p} k={k}");
            }
        });
    }

    #[test]
    fn gemm_matches_scalar_n_panel_boundary() {
        // n that is not a multiple of N_R = 24 → trailing partial panel.
        run_for_primes(|p| {
            for &n in &[1usize, 8, 23, 24, 25, 47, 48, 49, 95, 96, 97] {
                let m = 4;
                let k = 32;
                let a: Vec<u8> = (0..(m * k) as u32)
                    .map(|i| ((i * 17 + 1) % p as u32) as u8)
                    .collect();
                let bt: Vec<u8> = (0..(n * k) as u32)
                    .map(|i| ((i * 23 + 5) % p as u32) as u8)
                    .collect();
                let mut got = vec![0u8; m * n];
                unsafe { fp_small_f32_gemm(&a, &bt, m, k, n, p, &mut got) };
                let want = scalar_gemm(&a, &bt, m, k, n, p);
                assert_eq!(got, want, "p={p} n={n}");
            }
        });
    }

    #[test]
    fn gemm_matches_scalar_m_partial() {
        // m that is not a multiple of M_R = 4 → trailing partial row tile.
        run_for_primes(|p| {
            for &m in &[1usize, 2, 3, 5, 6, 7, 9] {
                let k = 32;
                let n = 24;
                let a: Vec<u8> = (0..(m * k) as u32)
                    .map(|i| ((i * 17 + 1) % p as u32) as u8)
                    .collect();
                let bt: Vec<u8> = (0..(n * k) as u32)
                    .map(|i| ((i * 23 + 5) % p as u32) as u8)
                    .collect();
                let mut got = vec![0u8; m * n];
                unsafe { fp_small_f32_gemm(&a, &bt, m, k, n, p, &mut got) };
                let want = scalar_gemm(&a, &bt, m, k, n, p);
                assert_eq!(got, want, "p={p} m={m}");
            }
        });
    }

    #[test]
    fn gemm_matches_scalar_zero_dims() {
        let avail = std::arch::is_x86_feature_detected!("avx2")
            && std::arch::is_x86_feature_detected!("fma");
        if !avail {
            return;
        }
        let mut out: Vec<u8> = vec![];
        unsafe { fp_small_f32_gemm(&[], &[], 0, 0, 0, 7, &mut out) };
        assert!(out.is_empty());
    }
}
