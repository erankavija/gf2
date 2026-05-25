//! AVX2 pure-integer Goto/BLIS-style panelized GEMM kernel for small
//! `Fp<P>` with `P <= 251`.
//!
//! This is **Route C** from the jit:615db3b9 Phase 1 plan
//! (`dev/active/615db3b9-finite-field-la-sota-plan.md` § Phase 1, item 3)
//! and the design note `dev/active/fc182ed5-route-c-design.md`.
//!
//! The kernel processes a whole-GEMM call as a Goto/BLIS-style
//! 3-loop structure (outer-N, KC blocking, MR/NR register-blocked
//! micro-kernel) with **explicit A/B panel packing** that Candidate C
//! does not do. The inner kernel uses `_mm256_madd_epi16` over u16
//! lane pairs, accumulating into u32 SIMD lanes — same lane-pair MAC
//! shape as Candidate C, but with contiguous pre-packed operand
//! loads instead of strided per-row reloads.
//!
//! All unsafe intrinsics are isolated here; the safe wrapper lives in
//! `crate::fp_small_panel` via the `SmallPrimePanelFns` table returned
//! by `detect`.
//!
//! # Algorithm
//!
//! Panel dimensions (see `dev/active/fc182ed5-route-c-design.md` § 2 for
//! the derivation from Goto-vandeGeijn 2008, BLIS 2015, and the AMD
//! Zen 3 Software Optimization Guide):
//!
//! - `MR = 4` rows of A per inner tile (one A pack per `m / MR` outer-M loops)
//! - `NR = 24` columns of output per inner tile (3 × 8-lane i32 sub-tiles)
//! - `KC = 256` k-axis cache blocking (L1d-fit; well below u32 overflow)
//!
//! ## A-pack layout
//!
//! For a 4-row block starting at row `i_blk`, the A pack is a
//! `Vec<u16>` of length `MR · KC` (or `Vec<u32>` of `MR · KC / 2`,
//! equivalent), holding pair-broadcasts:
//!
//! ```text
//! a_pack32[(t / 2) * MR + i] = ((a[i_blk + i, t + 1] as u32) << 16)
//!                            |  (a[i_blk + i, t]     as u32)
//! ```
//!
//! The inner kernel reads `a_pack32[t/2 * MR + i]` as a single u32 and
//! broadcasts it via `_mm256_set1_epi32` to obtain a ymm holding the
//! pair `[a[i,t], a[i,t+1]]` repeated 8 times across 16 u16 lanes.
//!
//! ## B-pack layout
//!
//! For one n-panel of `NR = 24` output columns starting at column
//! `j_blk`, the B pack is a `Vec<u8>` slice of `KC × NR` bytes,
//! laid out as pair-of-rows per `j_off`:
//!
//! ```text
//! b_pack[panel_off + (t / 2) * NR * 2 + j_off * 2 + 0] = b[t,     j_blk + j_off]
//! b_pack[panel_off + (t / 2) * NR * 2 + j_off * 2 + 1] = b[t + 1, j_blk + j_off]
//! ```
//!
//! Equivalently, viewed in u16-lane units: each pair-of-rows for one
//! `(t/2, j_blk)` is 48 contiguous bytes, organised as 3 × 16-byte
//! sub-tiles that load via `_mm256_cvtepu8_epi16` into 3 ymm holding
//! 16 u16 lanes each.
//!
//! ## Inner micro-kernel (per t-pair)
//!
//! 4 a-broadcasts × 3 b-pair loads × 12 `_mm256_madd_epi16` /
//! `_mm256_add_epi32` pairs. 12 u32 SIMD accumulators are held in
//! registers across the entire kc chunk; only at chunk boundary
//! (or at panel boundary for `k ≤ KC`) do they get reduced mod p
//! and packed to u8 bytes.
//!
//! ## Reduction at panel boundary
//!
//! Each of the 12 u32 ymm accumulators is reduced via
//! [`crate::x86::fp_small::barrett_reduce_lane32`] (the SSOT 32-bit-lane
//! Barrett reducer used by Candidate C's SpMM row reducer and by
//! route A). 8 reduced i32 lanes per ymm are packed to 8 u8 lanes
//! via `_mm256_packus_epi32` → `_mm256_permute4x64_epi64` →
//! `_mm256_packus_epi16`, the same SSOT 3-step pack route A uses
//! (`pack_i32x8_to_u8` in `fp_small_f32.rs`).
//!
//! # Safety
//!
//! All public functions here are `unsafe` — callers must ensure AVX2
//! is available at runtime, `p` is an odd prime in `[3, 251]`, and
//! all input bytes are canonical (`< p`). The safe wrapper in
//! `crate::fp_small_panel` enforces this at the function-pointer
//! dispatch boundary.

#![allow(clippy::missing_safety_doc)]
#![allow(clippy::too_many_arguments)]

use core::arch::x86_64::*;

/// Inner register tile: rows of A.
pub(crate) const MR: usize = 4;

/// Inner register tile: columns of output (3 × 8-lane i32 sub-tiles).
pub(crate) const NR: usize = 24;

/// Cache-blocking factor along the k-axis. See module docs and
/// `dev/active/fc182ed5-route-c-design.md` § 2.2 for the L1d-fit
/// derivation; the u32 overflow bound (`KC ≤ 68 719` at p = 251) is
/// orders of magnitude larger and not binding.
pub(crate) const KC: usize = 256;

/// Whole-GEMM panelized integer kernel for canonical-byte `Fp<P>`
/// operands with `P <= 251` (route C, issue fc182ed5).
///
/// Computes `c[i*n + j] = (∑_t a[i*k + t] * bt[j*k + t]) mod p` for
/// every `(i, j) ∈ [0, m) × [0, n)`. Inputs `a` and `bt` carry
/// canonical bytes (value `< p`); `bt` is the row-major transpose
/// of B (length `n * k`, so row `j` holds column `j` of B).
///
/// # Safety
///
/// Caller must ensure AVX2 is available at runtime, `p ∈ [3, 251]`
/// is an odd prime, and all input bytes are canonical (`< p`).
///
/// # Panics
///
/// Panics if any slice length disagrees with `m`, `k`, `n`.
#[target_feature(enable = "avx2")]
pub unsafe fn fp_small_panel_gemm(
    a: &[u8],
    bt: &[u8],
    m: usize,
    k: usize,
    n: usize,
    p: u8,
    c: &mut [u8],
) {
    assert_eq!(a.len(), m * k, "fp_small_panel_gemm: a shape");
    assert_eq!(bt.len(), n * k, "fp_small_panel_gemm: bt shape");
    assert_eq!(c.len(), m * n, "fp_small_panel_gemm: c shape");

    if m == 0 || k == 0 || n == 0 {
        return;
    }

    // Pre-compute the per-prime 32-bit Barrett constant
    // mu32 = ⌊2³² / p⌋ (passed to `barrett_reduce_lane32` once per
    // tile reduction).
    let p_u32 = p as u32;
    let mu32 = ((1u64 << 32) / p_u32 as u64) as u32;
    let mu_vec = _mm256_set1_epi64x(mu32 as i64);
    let p_vec32 = _mm256_set1_epi32(p_u32 as i32);

    // n-panel count (each panel covers NR output columns).
    let n_panels = n.div_ceil(NR);

    // Pre-pack B into NR-major panels. Layout per panel: a flat byte
    // strip of length `kc_total · NR` bytes, where `kc_total` is the
    // padded k length to an even pair (handled below). For each
    // t-pair `(t, t+1)` within the panel and each j_off in
    // `0..n_eff`, two adjacent bytes hold b[t, j_blk+j_off] and
    // b[t+1, j_blk+j_off]. The last pair (if k is odd) zero-pads
    // the upper byte so the inner kernel never reads past k.
    let k_padded = if k.is_multiple_of(2) { k } else { k + 1 };
    let panel_bytes = k_padded * NR;
    let mut b_packed: Vec<u8> = vec![0u8; n_panels * panel_bytes];
    for panel_idx in 0..n_panels {
        let j_blk = panel_idx * NR;
        let j_end = (j_blk + NR).min(n);
        let n_eff = j_end - j_blk;
        let panel_off = panel_idx * panel_bytes;
        // Iterate t-pairs over the full k axis.
        let mut t = 0usize;
        while t < k {
            let t_pair_idx = t / 2;
            let dst_pair_off = panel_off + t_pair_idx * NR * 2;
            for j_off in 0..n_eff {
                let b_at_t = bt[(j_blk + j_off) * k + t];
                b_packed[dst_pair_off + j_off * 2] = b_at_t;
            }
            if t + 1 < k {
                for j_off in 0..n_eff {
                    let b_at_t1 = bt[(j_blk + j_off) * k + t + 1];
                    b_packed[dst_pair_off + j_off * 2 + 1] = b_at_t1;
                }
            }
            // Slack columns (j_off ≥ n_eff): left zero by the
            // initial `vec![0u8; …]`, contributing zero to the inner
            // MAC — output cells in the slack columns are written
            // but the caller only reads the first `n_eff` slots.
            t += 2;
        }
    }

    // Outer-M loop: process MR-row blocks of A.
    let m_full = m - (m % MR);
    let mut i_blk = 0usize;
    while i_blk < m_full {
        run_one_m_block::<4>(
            a,
            &b_packed,
            i_blk,
            k,
            n,
            panel_bytes,
            n_panels,
            k_padded,
            p,
            mu_vec,
            p_vec32,
            c,
        );
        i_blk += MR;
    }
    if i_blk < m {
        let m_eff = m - i_blk;
        match m_eff {
            1 => run_one_m_block::<1>(
                a,
                &b_packed,
                i_blk,
                k,
                n,
                panel_bytes,
                n_panels,
                k_padded,
                p,
                mu_vec,
                p_vec32,
                c,
            ),
            2 => run_one_m_block::<2>(
                a,
                &b_packed,
                i_blk,
                k,
                n,
                panel_bytes,
                n_panels,
                k_padded,
                p,
                mu_vec,
                p_vec32,
                c,
            ),
            3 => run_one_m_block::<3>(
                a,
                &b_packed,
                i_blk,
                k,
                n,
                panel_bytes,
                n_panels,
                k_padded,
                p,
                mu_vec,
                p_vec32,
                c,
            ),
            _ => unreachable!(),
        }
    }
}

/// Pack one MR-row block of A into the pair-broadcast u32 format.
///
/// `dst` is a `Vec<u32>` of length `MR * (k_padded / 2)`; the layout
/// matches the inner-kernel access pattern
/// `a_pack32[(t/2) * MR + i] = (a[i,t+1] << 16) | a[i,t]`.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn pack_a_block<const M_EFF: usize>(
    a: &[u8],
    i_blk: usize,
    k: usize,
    k_padded: usize,
    dst: &mut [u32],
) {
    debug_assert!(M_EFF <= MR);
    debug_assert_eq!(dst.len(), MR * (k_padded / 2));
    // Zero-init: slack pair-positions (t ≥ k) and slack rows
    // (i ≥ M_EFF) must read as zero in the inner kernel.
    for slot in dst.iter_mut() {
        *slot = 0;
    }
    let mut t = 0usize;
    while t < k {
        let t_pair = t / 2;
        let dst_base = t_pair * MR;
        let a_base = i_blk * k + t;
        // i = 0..M_EFF rows; remaining MR - M_EFF rows stay zero.
        // Use a const-loop unrolled by M_EFF.
        if M_EFF >= 1 {
            let lo = a[a_base] as u32;
            let hi = if t + 1 < k { a[a_base + 1] as u32 } else { 0 };
            dst[dst_base] = lo | (hi << 16);
        }
        if M_EFF >= 2 {
            let lo = a[a_base + k] as u32;
            let hi = if t + 1 < k {
                a[a_base + k + 1] as u32
            } else {
                0
            };
            dst[dst_base + 1] = lo | (hi << 16);
        }
        if M_EFF >= 3 {
            let lo = a[a_base + 2 * k] as u32;
            let hi = if t + 1 < k {
                a[a_base + 2 * k + 1] as u32
            } else {
                0
            };
            dst[dst_base + 2] = lo | (hi << 16);
        }
        if M_EFF >= 4 {
            let lo = a[a_base + 3 * k] as u32;
            let hi = if t + 1 < k {
                a[a_base + 3 * k + 1] as u32
            } else {
                0
            };
            dst[dst_base + 3] = lo | (hi << 16);
        }
        t += 2;
    }
}

/// Drives all n-panels for one MR-row block of A.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn run_one_m_block<const M_EFF: usize>(
    a: &[u8],
    b_packed: &[u8],
    i_blk: usize,
    k: usize,
    n: usize,
    panel_bytes: usize,
    n_panels: usize,
    k_padded: usize,
    p: u8,
    mu_vec: __m256i,
    p_vec32: __m256i,
    c: &mut [u8],
) {
    // Pack the A block once and reuse across every n-panel.
    let mut a_pack32: Vec<u32> = vec![0u32; MR * (k_padded / 2)];
    pack_a_block::<M_EFF>(a, i_blk, k, k_padded, &mut a_pack32);

    for panel_idx in 0..n_panels {
        let j_blk = panel_idx * NR;
        let j_end = (j_blk + NR).min(n);
        let n_eff = j_end - j_blk;
        let panel_off = panel_idx * panel_bytes;
        run_one_panel::<M_EFF>(
            &a_pack32, b_packed, panel_off, i_blk, k, n, j_blk, n_eff, k_padded, p, mu_vec,
            p_vec32, c,
        );
    }
}

/// Compute one MR×NR output tile (for one MR-row block × one n-panel).
///
/// Steps through the k-axis in pair-of-rows increments, blocked by
/// `KC / 2` pair-steps per cache chunk. After the last chunk for
/// this panel, the 12 u32 SIMD accumulators are reduced mod p and
/// packed to u8 output bytes.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn run_one_panel<const M_EFF: usize>(
    a_pack32: &[u32],
    b_packed: &[u8],
    panel_off: usize,
    i_blk: usize,
    _k: usize,
    n: usize,
    j_blk: usize,
    n_eff: usize,
    k_padded: usize,
    _p: u8,
    mu_vec: __m256i,
    p_vec32: __m256i,
    c: &mut [u8],
) {
    // 12 u32 SIMD accumulators (one per output sub-tile cell).
    let mut acc00 = _mm256_setzero_si256();
    let mut acc01 = _mm256_setzero_si256();
    let mut acc02 = _mm256_setzero_si256();
    let mut acc10 = _mm256_setzero_si256();
    let mut acc11 = _mm256_setzero_si256();
    let mut acc12 = _mm256_setzero_si256();
    let mut acc20 = _mm256_setzero_si256();
    let mut acc21 = _mm256_setzero_si256();
    let mut acc22 = _mm256_setzero_si256();
    let mut acc30 = _mm256_setzero_si256();
    let mut acc31 = _mm256_setzero_si256();
    let mut acc32 = _mm256_setzero_si256();

    let total_pairs = k_padded / 2;
    let kc_pairs = KC / 2;
    let a_pack_base = a_pack32.as_ptr();
    let b_pack_base = b_packed.as_ptr().add(panel_off);

    // Prefetch distance in t-pairs ahead of the live row.
    // Each t-pair reads `NR · 2 = 48` bytes from b_pack; 4 t-pairs
    // ahead = 192 B = 3 cache lines, well within the L1d prefetch
    // queue (16 lines deep on Zen 3 per AMD Family 19h SOG § 2.13).
    const PREFETCH_DIST_PAIRS: usize = 4;

    let mut t_pair = 0usize;
    while t_pair < total_pairs {
        let chunk_end = (t_pair + kc_pairs).min(total_pairs);
        let prefetch_end = chunk_end.saturating_sub(PREFETCH_DIST_PAIRS);

        for tp in t_pair..chunk_end {
            // Prefetch hint for the b-pack rows PREFETCH_DIST_PAIRS
            // pairs ahead.
            if tp < prefetch_end {
                let pf_ptr = b_pack_base.add((tp + PREFETCH_DIST_PAIRS) * NR * 2) as *const i8;
                _mm_prefetch::<{ _MM_HINT_T0 }>(pf_ptr);
            }

            // Load 3 b-pair sub-tiles (each 16 bytes → 16 u16 lanes).
            // Layout per pair: 48 bytes total starting at offset tp * 48.
            let b_pair_ptr = b_pack_base.add(tp * NR * 2);
            let b0 = _mm256_cvtepu8_epi16(_mm_loadu_si128(b_pair_ptr as *const __m128i));
            let b1 = _mm256_cvtepu8_epi16(_mm_loadu_si128(b_pair_ptr.add(16) as *const __m128i));
            let b2 = _mm256_cvtepu8_epi16(_mm_loadu_si128(b_pair_ptr.add(32) as *const __m128i));

            // Broadcast each row's pair-of-bytes as 16 u16 lanes.
            // a_pack32[tp * MR + i] holds (a[i, t+1] as u32) << 16 | a[i, t]
            // — interpreting as 16 u16 lanes via _mm256_set1_epi32 puts
            // the pair `[lo, hi]` into u16 lanes [0, 1], repeated 8 times.
            let a_row_ptr = a_pack_base.add(tp * MR);

            if M_EFF >= 1 {
                let a0 = _mm256_set1_epi32(*a_row_ptr as i32);
                acc00 = _mm256_add_epi32(acc00, _mm256_madd_epi16(a0, b0));
                acc01 = _mm256_add_epi32(acc01, _mm256_madd_epi16(a0, b1));
                acc02 = _mm256_add_epi32(acc02, _mm256_madd_epi16(a0, b2));
            }
            if M_EFF >= 2 {
                let a1 = _mm256_set1_epi32(*a_row_ptr.add(1) as i32);
                acc10 = _mm256_add_epi32(acc10, _mm256_madd_epi16(a1, b0));
                acc11 = _mm256_add_epi32(acc11, _mm256_madd_epi16(a1, b1));
                acc12 = _mm256_add_epi32(acc12, _mm256_madd_epi16(a1, b2));
            }
            if M_EFF >= 3 {
                let a2 = _mm256_set1_epi32(*a_row_ptr.add(2) as i32);
                acc20 = _mm256_add_epi32(acc20, _mm256_madd_epi16(a2, b0));
                acc21 = _mm256_add_epi32(acc21, _mm256_madd_epi16(a2, b1));
                acc22 = _mm256_add_epi32(acc22, _mm256_madd_epi16(a2, b2));
            }
            if M_EFF >= 4 {
                let a3 = _mm256_set1_epi32(*a_row_ptr.add(3) as i32);
                acc30 = _mm256_add_epi32(acc30, _mm256_madd_epi16(a3, b0));
                acc31 = _mm256_add_epi32(acc31, _mm256_madd_epi16(a3, b1));
                acc32 = _mm256_add_epi32(acc32, _mm256_madd_epi16(a3, b2));
            }
        }
        t_pair = chunk_end;
    }

    // Reduce and pack to bytes. Per-lane bound after the full k axis:
    // `(k_padded / 2) · 2 · (p−1)² = k_padded · (p−1)²`. For
    // p = 251 and k ≤ 2^15 the bound stays well below 2^32 (the
    // u32 overflow cap is k ≤ 2^32 / 62500 ≈ 68 719). We use the
    // Phase-2 SSOT 32-bit-lane Barrett primitive.
    let reduced = [
        super::fp_small::barrett_reduce_lane32(acc00, mu_vec, p_vec32),
        super::fp_small::barrett_reduce_lane32(acc01, mu_vec, p_vec32),
        super::fp_small::barrett_reduce_lane32(acc02, mu_vec, p_vec32),
        super::fp_small::barrett_reduce_lane32(acc10, mu_vec, p_vec32),
        super::fp_small::barrett_reduce_lane32(acc11, mu_vec, p_vec32),
        super::fp_small::barrett_reduce_lane32(acc12, mu_vec, p_vec32),
        super::fp_small::barrett_reduce_lane32(acc20, mu_vec, p_vec32),
        super::fp_small::barrett_reduce_lane32(acc21, mu_vec, p_vec32),
        super::fp_small::barrett_reduce_lane32(acc22, mu_vec, p_vec32),
        super::fp_small::barrett_reduce_lane32(acc30, mu_vec, p_vec32),
        super::fp_small::barrett_reduce_lane32(acc31, mu_vec, p_vec32),
        super::fp_small::barrett_reduce_lane32(acc32, mu_vec, p_vec32),
    ];

    // For each output row in the tile, pack the 3 × 8-i32 sub-tiles
    // into 24 contiguous u8 bytes and write into `c`. Slack
    // columns (`j ≥ n_eff`) are not written.
    for i_off in 0..M_EFF {
        let row_acc0 = reduced[i_off * 3];
        let row_acc1 = reduced[i_off * 3 + 1];
        let row_acc2 = reduced[i_off * 3 + 2];
        let bytes0 = pack_i32x8_to_u8_local(row_acc0);
        let bytes1 = pack_i32x8_to_u8_local(row_acc1);
        let bytes2 = pack_i32x8_to_u8_local(row_acc2);
        let c_row_base = (i_blk + i_off) * n + j_blk;
        for j_off in 0..n_eff {
            let src = if j_off < 8 {
                bytes0[j_off]
            } else if j_off < 16 {
                bytes1[j_off - 8]
            } else {
                bytes2[j_off - 16]
            };
            *c.get_unchecked_mut(c_row_base + j_off) = src;
        }
    }
}

/// Pack 8 reduced i32 lanes (each in `[0, p)`) into an 8-byte u8 array.
///
/// Same 3-step SSOT pack route A uses (`pack_i32x8_to_u8` in
/// `crates/gf2-kernels-simd/src/x86/fp_small_f32.rs`); the algebra is
/// identical so we don't introduce new behaviour. We keep a local copy
/// here (rather than delegate cross-module) only to avoid a circular
/// `pub(super)` re-export plus to allow inlining without leaking the
/// route-A internal name into the panel kernel's call graph; the
/// generated SIMD sequence is byte-identical to route A's
/// `pack_i32x8_to_u8`.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn pack_i32x8_to_u8_local(reduced: __m256i) -> [u8; 8] {
    let packed16 = _mm256_packus_epi32(reduced, reduced);
    let permuted = _mm256_permute4x64_epi64::<0xD8>(packed16);
    let packed8 = _mm256_packus_epi16(permuted, permuted);
    let lower = _mm256_castsi256_si128(packed8);
    let mut out = [0u8; 16];
    _mm_storeu_si128(out.as_mut_ptr() as *mut __m128i, lower);
    [
        out[0], out[1], out[2], out[3], out[4], out[5], out[6], out[7],
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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

    fn run_for_primes(test: impl Fn(u8)) {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        for &p in &[3u8, 5, 7, 11, 13, 17, 31, 127, 251] {
            test(p);
        }
    }

    #[test]
    fn panel_gemm_matches_scalar_at_boundary_shapes() {
        run_for_primes(|p| {
            // Cover MR/NR/KC boundaries plus the criterion sizes.
            let cases: &[(usize, usize, usize)] = &[
                (1, 1, 1),
                (1, 4, 4),
                (3, 5, 7),
                (4, 64, 24),
                (8, 64, 32),
                (4, 65, 25),   // odd k → pair tail
                (4, 255, 24),  // KC-1
                (4, 256, 24),  // exact KC
                (4, 257, 24),  // KC+1 → 2 KC chunks
                (4, 512, 48),  // 2 KC chunks × 2 panels
                (1, 256, 256), // m=1 partial row
                (2, 256, 256), // m=2 partial row
                (3, 256, 256), // m=3 partial row
                (5, 256, 256), // 4 + 1
                (9, 64, 32),
                (16, 134, 16),
                (16, 134, 24),
                (4, 256, 256),
                (4, 1024, 1024),
            ];
            for &(m, k, n) in cases {
                let a: Vec<u8> = (0..(m * k) as u32)
                    .map(|i| ((i * 17 + 1) % p as u32) as u8)
                    .collect();
                let bt: Vec<u8> = (0..(n * k) as u32)
                    .map(|i| ((i * 23 + 5) % p as u32) as u8)
                    .collect();
                let mut got = vec![0u8; m * n];
                unsafe { fp_small_panel_gemm(&a, &bt, m, k, n, p, &mut got) };
                let expected = scalar_gemm(&a, &bt, m, k, n, p);
                assert_eq!(got, expected, "p={p} m={m} k={k} n={n}");
            }
        });
    }

    #[test]
    fn panel_gemm_n_boundary_sweep() {
        run_for_primes(|p| {
            // Per-issue boundary n values for criterion 2.
            let m = 4;
            let k = 64;
            for &n in &[
                1usize, 8, 15, 16, 17, 23, 24, 25, 47, 48, 49, 63, 64, 65, 95, 96, 97, 121,
            ] {
                let a: Vec<u8> = (0..(m * k) as u32)
                    .map(|i| ((i * 11 + 7) % p as u32) as u8)
                    .collect();
                let bt: Vec<u8> = (0..(n * k) as u32)
                    .map(|i| ((i * 19 + 3) % p as u32) as u8)
                    .collect();
                let mut got = vec![0u8; m * n];
                unsafe { fp_small_panel_gemm(&a, &bt, m, k, n, p, &mut got) };
                let expected = scalar_gemm(&a, &bt, m, k, n, p);
                assert_eq!(got, expected, "p={p} n={n}");
            }
        });
    }

    #[test]
    fn panel_gemm_handles_zero_dims() {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        let a: Vec<u8> = vec![];
        let bt: Vec<u8> = vec![];
        let mut out: Vec<u8> = vec![];
        unsafe { fp_small_panel_gemm(&a, &bt, 0, 0, 0, 7, &mut out) };
        assert!(out.is_empty());
    }
}
