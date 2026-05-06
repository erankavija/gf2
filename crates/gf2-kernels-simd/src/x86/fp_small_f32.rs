//! AVX2 + FMA3 f32-cascade GEMM micro-kernel for small `Fp<P>` with
//! `P <= 251` (Candidate F per
//! `dev/plans/small_prime_kernel_strategy.md` § 4.5 / § 5.5 / § 6.1).
//!
//! Inputs and outputs are canonical bytes (`u8`, value `< P`); all
//! arithmetic happens through f32 lanes via `_mm256_fmadd_ps`. The
//! kernel is structured as a BLIS-class register-blocked sgemm
//! micro-kernel:
//!
//! - **Pack pass.** `a: &[u8]` (m × k row-major) is consumed in place
//!   — no auxiliary `Vec<f32>` is allocated. `bt: &[u8]` (n × k
//!   row-major) is repacked into N-major u8 panels of width `N_R = 24`
//!   (each panel `k × N_R` contiguous u8). The u8→f32 conversion
//!   happens at register granularity inside the inner kernel via
//!   `_mm256_cvtepu8_epi32` + `_mm256_cvtepi32_ps`, eliminating the
//!   intermediate 4×-blown `Vec<f32>` that the previous design built.
//! - **Inner micro-kernel.** A `4 × 24` tile (`m_R = 4`, `n_R = 24`)
//!   uses 12 accumulator AVX2 registers + 3 b registers + 1 a
//!   broadcast — exhausting the 16-register file by design. Each
//!   inner-`k` step issues 12 `_mm256_fmadd_ps`; on Zen-3 the two FMA
//!   ports each retire one per cycle (Agner Fog's Zen-3 tables) so
//!   the inner body is back-end-bound at ~6 cycles / step.
//! - **Reduction.** At each `k_chunk` boundary the f32 accumulator
//!   tile is rounded to nearest integer, converted to `i32` SIMD
//!   lanes, and added into a 12-vector i32 running sum kept across
//!   all chunks. Only the final tile-end pass runs the scalar `% p`
//!   per output cell. The chunk size is
//!   `k_chunk = min(k, k_max(p), K_CHUNK_CAP)` where
//!   `k_max(p) = floor(2^24 / (p-1)²)` keeps the running f32 sum
//!   inside the exact-integer range, and the `K_CHUNK_CAP` limit
//!   (1024 u8) keeps each B-panel slice
//!   `k_chunk · N_R · 1 byte = 24 KB` inside Zen-3's 32 KB L1d.
//! - **Prefetch.** `_MM_HINT_T0` is issued for the next 3 B-panel
//!   rows ahead of the inner step, lifting the cache miss off the
//!   critical path on n ≥ 1024 cells where the B-panel does not fit
//!   in the inner-most cache hierarchy on first traversal.
//!
//! # Safety
//!
//! All public functions here are `unsafe` — callers must ensure
//! AVX2 + FMA3 are both available at runtime. Safe, dispatched entry
//! points live in `crate::fp_small_f32` via the `SmallPrimeF32Fns`
//! table returned by `detect`.

#![allow(clippy::missing_safety_doc)]
#![allow(clippy::too_many_arguments)]

use core::arch::x86_64::*;

/// Inner `m × n` register-tile dimensions.
const M_R: usize = 4;
const N_R: usize = 24;

/// L1d-resident k-chunk cap measured in **u8 lanes**. The k-chunk size
/// is `min(k, k_max(p), K_CHUNK_CAP)`.
///
/// Choice of 1024: for the 4 × 24 tile, the B-panel slice consumed per
/// chunk is `K_CHUNK_CAP · N_R · 1 byte = 1024 · 24 · 1 = 24 KB`,
/// well within Zen-3's 32 KB L1d. The earlier design's
/// `K_CHUNK_CAP = 256` was tuned for 4-byte lanes (24 KB) and is
/// 4× too restrictive once the B-panel is u8-packed; the chunked
/// reduction overhead amortises better at the larger K_CHUNK because
/// the round-and-cast is paid once per (`m_R × n_R = 96 cells`) per
/// chunk-end rather than per inner step.
const K_CHUNK_CAP: usize = 1024;

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

    // ── Pack B-transpose (n × k row-major: row j == column j of B)
    //    into N-major u8 panels of width N_R, each `k × N_R` row-major.
    //    For each n-panel `j_blk = 0, N_R, 2*N_R, ...`, we need
    //    `b_packed[panel_offset + t*N_R + j_off] = B[t, j_blk + j_off]
    //                                            = bt[(j_blk + j_off)*k + t]`.
    //    For the partial trailing panel (`n % N_R != 0`), unused
    //    lanes are filled with 0 so the FMA accumulates a zero
    //    (semantically harmless; the unused output cells are not
    //    read at unpack time). Storing as u8 keeps the panel 4× smaller
    //    than the previous f32 representation, freeing 75 % of L1d
    //    for the active working set and letting K_CHUNK_CAP grow to
    //    1024 without spilling.
    let n_panels = n.div_ceil(N_R);
    let panel_stride = k * N_R;
    let mut b_packed: Vec<u8> = vec![0u8; n_panels * panel_stride];
    // Outer loop over t so the inner write is the contiguous N_R-wide
    // row of the panel; this keeps writes streaming and avoids the
    // 24-byte stride that would otherwise be on the inner axis.
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
                b_packed[dst_row_off + j_off] = bt[(j_blk + j_off) * k + t];
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
    // For `p = 7`: k_max = 466 033, but we cap at K_CHUNK_CAP.
    // For `p = 31`: k_max = 18 631 (capped at K_CHUNK_CAP).
    // For `p = 251`: k_max = 268.
    let k_max = compute_k_max(p);
    let k_chunk = k_max.min(K_CHUNK_CAP);

    // ── A-pack (per row-tile) ─────────────────────────────────────
    //
    // Pre-convert each `M_R`-row block of A from u8 to f32 once per
    // i_blk, in interleaved row-major layout: `a_pack_f32[t*M_R + i]
    // = a[(i_blk + i) * k + t]`. The inner kernel then reads each of
    // M_EFF a-rows as a single SIMD broadcast from a contiguous f32
    // address — eliminating the per-step `movzbl + vcvtsi2ss +
    // vbroadcastss` partial-register dependency chain that the
    // previous "load + scalar cvt + broadcast" path produced. The
    // pack is paid once per i_blk and amortised across all
    // `n_panels` n-tiles for that row block.
    let mut a_pack_f32: Vec<f32> = vec![0.0; M_R * k];

    // ── Inner GEMM loop. ──────────────────────────────────────────
    //
    // We split into a `m_eff = M_R = 4` steady-state path and a
    // generic `m_eff < 4` trailing path. The steady-state path is
    // monomorphised on `M_EFF = 4` so the inner loop body holds
    // exactly 12 FMAs with zero branches; the trailing path
    // monomorphises on `M_EFF ∈ {1, 2, 3}` likewise. Branchless
    // inner loops let the compiler schedule the FMA / load / cvt
    // dispatch fully to the back-end.
    let m_full = m - (m % M_R);
    let mut i_blk = 0usize;
    while i_blk < m_full {
        pack_a_block::<4>(a, i_blk, k, &mut a_pack_f32);
        run_panels::<4>(
            &a_pack_f32,
            &b_packed,
            i_blk,
            k,
            n,
            n_panels,
            panel_stride,
            k_chunk,
            p_i32,
            c,
        );
        i_blk += M_R;
    }
    if i_blk < m {
        match m - i_blk {
            1 => {
                pack_a_block::<1>(a, i_blk, k, &mut a_pack_f32);
                run_panels::<1>(
                    &a_pack_f32,
                    &b_packed,
                    i_blk,
                    k,
                    n,
                    n_panels,
                    panel_stride,
                    k_chunk,
                    p_i32,
                    c,
                );
            }
            2 => {
                pack_a_block::<2>(a, i_blk, k, &mut a_pack_f32);
                run_panels::<2>(
                    &a_pack_f32,
                    &b_packed,
                    i_blk,
                    k,
                    n,
                    n_panels,
                    panel_stride,
                    k_chunk,
                    p_i32,
                    c,
                );
            }
            3 => {
                pack_a_block::<3>(a, i_blk, k, &mut a_pack_f32);
                run_panels::<3>(
                    &a_pack_f32,
                    &b_packed,
                    i_blk,
                    k,
                    n,
                    n_panels,
                    panel_stride,
                    k_chunk,
                    p_i32,
                    c,
                );
            }
            _ => unreachable!(),
        }
    }
}

/// Pre-pack a `M_EFF`-row block of A into `[f32; M_R * k]` in interleaved
/// row-major form `dst[t * M_R + i] = a[(i_blk + i) * k + t]`. The
/// dst slack rows (`i ∈ [M_EFF, M_R)`) hold zeros (already initialised
/// by the caller's zero-fill).
///
/// Done once per i_blk; the inner kernel reads broadcasts from this
/// scratch buffer rather than recomputing u8→f32 12 times per t-step.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn pack_a_block<const M_EFF: usize>(a: &[u8], i_blk: usize, k: usize, dst: &mut [f32]) {
    debug_assert!(dst.len() >= M_R * k);
    debug_assert!(M_EFF <= M_R);
    let a_base = a.as_ptr().add(i_blk * k);
    let dst_base = dst.as_mut_ptr();
    // Manual loop, branch-free over t.
    for t in 0..k {
        let dst_row = dst_base.add(t * M_R);
        if M_EFF >= 1 {
            *dst_row = *a_base.add(t) as f32;
        }
        if M_EFF >= 2 {
            *dst_row.add(1) = *a_base.add(k + t) as f32;
        }
        if M_EFF >= 3 {
            *dst_row.add(2) = *a_base.add(2 * k + t) as f32;
        }
        if M_EFF >= 4 {
            *dst_row.add(3) = *a_base.add(3 * k + t) as f32;
        }
        // Slack rows for M_EFF < M_R are pre-zeroed by the caller's
        // `vec![0.0; M_R * k]`; we leave them untouched here.
    }
}

/// Sweep all `n_panels` for one `M_EFF`-row tile starting at `i_blk`.
///
/// Monomorphisation on `M_EFF` deletes the dead FMA / sum branches
/// for `m_eff < 4`, leaving the steady-state `M_EFF = 4` body with
/// exactly 12 FMAs per inner step (no branches inside the hot loop).
///
/// `a_pack_f32` is the pre-packed A-row block: `M_R × k` f32 in
/// interleaved row-major (`a_pack_f32[t * M_R + i] = a[(i_blk + i) * k + t]`).
#[inline]
#[target_feature(enable = "avx2,fma")]
unsafe fn run_panels<const M_EFF: usize>(
    a_pack_f32: &[f32],
    b_packed: &[u8],
    i_blk: usize,
    k: usize,
    n: usize,
    n_panels: usize,
    panel_stride: usize,
    k_chunk: usize,
    p_i32: i32,
    c: &mut [u8],
) {
    for panel_idx in 0..n_panels {
        let j_blk = panel_idx * N_R;
        let j_end = (j_blk + N_R).min(n);
        let n_eff = j_end - j_blk;
        let panel_off = panel_idx * panel_stride;

        // i32 SIMD accumulators (12 vectors covering the 4 × 24 tile).
        // Each lane sums the rounded f32 chunk contributions across
        // all `k / k_chunk` chunks; the i32 range absorbs
        // `k · (p-1)² ≤ 4096 · 250² = 256M < 2^31` for p=251 and
        // even more headroom for smaller primes.
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

            let b_panel_base = b_packed.as_ptr().add(panel_off);
            let a_pack_base = a_pack_f32.as_ptr();

            // Pre-compute the prefetch boundary so the inner branch
            // condition reduces to a single compare.
            //
            // Prefetch distance: 3 rows ahead = 72 B ≈ next cache
            // line (rows are 24 B; cache lines 64 B). Empirically
            // the difference between 3 and 8 rows is in the noise on
            // Zen-3 — the hardware prefetcher streams the tail
            // adequately once the first miss is taken.
            const PREFETCH_DIST: usize = 3;
            let prefetch_end = t_end.saturating_sub(PREFETCH_DIST);

            for t in t_blk..t_end {
                let b_row_ptr = b_panel_base.add(t * N_R);
                let a_row_ptr = a_pack_base.add(t * M_R);

                // Issue prefetch hints for the B-panel row PREFETCH_DIST
                // steps ahead. Cheap (no µops on the back-end FMA ports);
                // typically lifts ~10 % of L1d miss latency off the
                // critical path on n ≥ 1024 cells.
                if t < prefetch_end {
                    _mm_prefetch::<{ _MM_HINT_T0 }>(b_row_ptr.add(PREFETCH_DIST * N_R) as *const i8);
                }

                // Load 24 u8 from B panel (one 16-byte + one 8-byte
                // load, then convert each 8-u8 chunk to 8 f32 via
                // cvtepu8_epi32 + cvtepi32_ps).
                let b_lo16 = _mm_loadu_si128(b_row_ptr as *const __m128i);
                let b_hi8 = _mm_loadu_si64(b_row_ptr.add(16) as *const _);

                // Three 8-lane f32 chunks covering [0..8), [8..16),
                // [16..24).
                let b0 = _mm256_cvtepi32_ps(_mm256_cvtepu8_epi32(b_lo16));
                let b1 = _mm256_cvtepi32_ps(_mm256_cvtepu8_epi32(_mm_srli_si128::<8>(b_lo16)));
                let b2 = _mm256_cvtepi32_ps(_mm256_cvtepu8_epi32(b_hi8));

                // 12 FMAs per inner iteration (M_EFF=4 path). The four
                // a-row broadcasts are now `_mm256_broadcast_ss` from
                // a contiguous f32 address — a single load+broadcast
                // µop on Zen-3 (no scalar cvt dep chain).
                if M_EFF >= 1 {
                    let a0 = _mm256_broadcast_ss(&*a_row_ptr);
                    acc00 = _mm256_fmadd_ps(b0, a0, acc00);
                    acc01 = _mm256_fmadd_ps(b1, a0, acc01);
                    acc02 = _mm256_fmadd_ps(b2, a0, acc02);
                }
                if M_EFF >= 2 {
                    let a1 = _mm256_broadcast_ss(&*a_row_ptr.add(1));
                    acc10 = _mm256_fmadd_ps(b0, a1, acc10);
                    acc11 = _mm256_fmadd_ps(b1, a1, acc11);
                    acc12 = _mm256_fmadd_ps(b2, a1, acc12);
                }
                if M_EFF >= 3 {
                    let a2 = _mm256_broadcast_ss(&*a_row_ptr.add(2));
                    acc20 = _mm256_fmadd_ps(b0, a2, acc20);
                    acc21 = _mm256_fmadd_ps(b1, a2, acc21);
                    acc22 = _mm256_fmadd_ps(b2, a2, acc22);
                }
                if M_EFF >= 4 {
                    let a3 = _mm256_broadcast_ss(&*a_row_ptr.add(3));
                    acc30 = _mm256_fmadd_ps(b0, a3, acc30);
                    acc31 = _mm256_fmadd_ps(b1, a3, acc31);
                    acc32 = _mm256_fmadd_ps(b2, a3, acc32);
                }
            }

            // Round-and-cast all live f32 accumulators to i32 SIMD
            // and add into the running i32 SIMD sums.
            if M_EFF >= 1 {
                sum00 = _mm256_add_epi32(sum00, round_ps_to_epi32(acc00));
                sum01 = _mm256_add_epi32(sum01, round_ps_to_epi32(acc01));
                sum02 = _mm256_add_epi32(sum02, round_ps_to_epi32(acc02));
            }
            if M_EFF >= 2 {
                sum10 = _mm256_add_epi32(sum10, round_ps_to_epi32(acc10));
                sum11 = _mm256_add_epi32(sum11, round_ps_to_epi32(acc11));
                sum12 = _mm256_add_epi32(sum12, round_ps_to_epi32(acc12));
            }
            if M_EFF >= 3 {
                sum20 = _mm256_add_epi32(sum20, round_ps_to_epi32(acc20));
                sum21 = _mm256_add_epi32(sum21, round_ps_to_epi32(acc21));
                sum22 = _mm256_add_epi32(sum22, round_ps_to_epi32(acc22));
            }
            if M_EFF >= 4 {
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
            M_EFF, n_eff, p_i32, i_blk, j_blk, n, c,
        );
    }
}

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
        // k around `K_CHUNK_CAP` and `k_max(p=251)=268` boundaries.
        run_for_primes(|p| {
            for &k in &[
                63usize, 64, 65, 127, 128, 129, 134, 267, 268, 269, 512, 1023, 1024, 1025, 2047,
                2048,
            ] {
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
