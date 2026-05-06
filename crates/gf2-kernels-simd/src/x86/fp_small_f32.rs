//! AVX2 + FMA3 f32-cascade GEMM micro-kernel for small `Fp<P>` with
//! `P <= 251` (Candidate F per
//! `dev/plans/small_prime_kernel_strategy.md` § 4.5 / § 5.5 / § 6.1).
//!
//! Inputs and outputs are canonical residues. Inputs arrive **already
//! pre-packed to `f32`** (one canonical residue per lane, value in
//! `[0, p)`); outputs are written as canonical bytes (`u8`, value
//! `< P`). All inner-loop arithmetic happens through f32 lanes via
//! `_mm256_fmadd_ps`. The kernel is structured as a BLIS-class
//! register-blocked sgemm micro-kernel:
//!
//! - **Pack pass.** `a: &[f32]` (m × k row-major) is consumed in place
//!   — no auxiliary `Vec<f32>` is allocated for A. `bt: &[f32]` (n × k
//!   row-major, row j = column j of B) is repacked into N-major f32
//!   panels of width `N_R = 24` (each panel `k × N_R` contiguous f32).
//!   The packed B-panel is consumed by 3 × 8-lane `_mm256_loadu_ps`
//!   loads per inner step — no cvt instructions in the inner loop,
//!   matching the OpenBLAS / fflas-ffpack `Modular<float>` micro-kernel
//!   structure.
//! - **Inner micro-kernel.** A `4 × 24` tile (`m_R = 4`, `n_R = 24`)
//!   uses 12 accumulator AVX2 registers + 3 b registers + 1 a
//!   broadcast — exhausting the 16-register file by design. Each
//!   inner-`k` step issues 12 `_mm256_fmadd_ps`; on Zen-3 the two FMA
//!   ports each retire one per cycle so the inner body is back-end-
//!   bound at ~6 cycles / step. With pure f32 loads (no
//!   `vpmovzxbd + vcvtdq2ps` chains competing for back-end ports)
//!   the FMAs hit their issue-rate ceiling.
//! - **Reduction.** At each `k_chunk` boundary the f32 accumulator
//!   tile is rounded to nearest integer, converted to `i32` SIMD
//!   lanes, and added into a 12-vector i32 running sum kept across
//!   all chunks. Only the final tile-end pass runs the scalar `% p`
//!   per output cell. The chunk size is
//!   `k_chunk = min(k, k_max(p), K_CHUNK_CAP)` where
//!   `k_max(p) = floor(2^24 / (p-1)²)` keeps the running f32 sum
//!   inside the exact-integer range, and the `K_CHUNK_CAP` limit
//!   (1024 f32) keeps each B-panel slice
//!   `k_chunk · N_R · 4 byte = 96 KB` close to the L1d/L2 boundary
//!   on the Zen-3 reference host (3072 cells × 24 lanes worth of
//!   working set).
//! - **Prefetch.** `_MM_HINT_T0` is issued for the next 4 B-panel
//!   rows ahead of the inner step (rows are 96 B → 1.5 cache lines;
//!   4 rows ahead lifts the next 6 lines onto the L1d miss queue).
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

/// L2-resident k-chunk cap measured in **f32 lanes**. The k-chunk size
/// is `min(k, k_max(p), K_CHUNK_CAP)`.
///
/// Choice of 1024: for the 4 × 24 tile, the B-panel slice consumed per
/// chunk is `K_CHUNK_CAP · N_R · 4 byte = 1024 · 24 · 4 = 96 KB`,
/// fitting in Zen-3's 512 KB L2 with room for A-pack and the running
/// accumulator state. Picking 256 (24 KB, L1d-resident) penalises
/// `p ≤ 31` by 3 extra round-and-cast passes per panel at `k = 1024`
/// (k_chunk = 256 → 4 chunks vs 1 chunk at K_CHUNK_CAP = 1024). The
/// per-chunk reduction (12 `vroundps + vcvtps2dq + vpaddd`) is small
/// vs the inner FMAs but adds up over many panel sweeps.
///
/// For `p = 251` the per-prime `k_max = 268` cap binds first, so the
/// cap value only affects `p ≤ 31` and large `k`; the outer-N
/// cache-blocking (below) handles the L1d/L2 working-set sizing
/// independently of the chunk cap.
const K_CHUNK_CAP: usize = 1024;

/// Whole-gemm AVX2 + FMA3 f32-cascade kernel for small primes.
///
/// Computes `c[i*n + j] = (∑_t a[i*k + t] · bt[j*k + t]) mod p` for
/// every `(i, j) ∈ [0, m) × [0, n)`. Inputs `a` and `bt` carry
/// canonical residues (value `< p`) one per `f32` lane; `bt` is the
/// row-major transpose of the right operand (length `n * k`, so row
/// `j` holds column `j` of B).
///
/// # Safety
///
/// Caller must ensure AVX2 and FMA3 are both available at runtime,
/// `p ∈ [3, 251]` is an odd prime, and every input lane holds a
/// non-negative integer canonical residue in `[0, p)`.
///
/// # Panics
///
/// Panics if any slice length disagrees with `m`, `k`, `n`.
#[target_feature(enable = "avx2,fma")]
pub unsafe fn fp_small_f32_gemm(
    a: &[f32],
    bt: &[f32],
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
    //    into N-major f32 panels of width N_R, each `k × N_R` row-major.
    //    For each n-panel `j_blk = 0, N_R, 2*N_R, ...`, we need
    //    `b_packed[panel_offset + t*N_R + j_off] = B[t, j_blk + j_off]
    //                                            = bt[(j_blk + j_off)*k + t]`.
    //    For the partial trailing panel (`n % N_R != 0`), unused
    //    lanes are filled with 0.0 so the FMA accumulates a zero
    //    (semantically harmless; the unused output cells are not
    //    read at unpack time). Storing as f32 lets the inner kernel
    //    issue 3 × `_mm256_loadu_ps` per step with NO cvt micro-ops
    //    competing with the FMAs for back-end ports.
    let n_panels = n.div_ceil(N_R);
    let panel_stride = k * N_R;
    let mut b_packed: Vec<f32> = vec![0.0f32; n_panels * panel_stride];
    // Outer loop over t so the inner write is the contiguous N_R-wide
    // row of the panel; this keeps writes streaming and avoids the
    // 24-lane stride that would otherwise be on the inner axis.
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
    // Re-pack each `M_R`-row block of A into a stack-resident
    // interleaved row-major buffer: `a_pack_f32[t*M_R + i]
    // = a[(i_blk + i) * k + t]`. The inner kernel then reads each of
    // M_EFF a-rows as a single `_mm256_broadcast_ss` from a
    // contiguous f32 address. The interleave step keeps the
    // broadcasts cache-line-aligned to the t-axis (one cache line
    // covers M_R = 4 lanes = 16 B), so the broadcast load issues
    // from the same cache line as the prior step's broadcast and the
    // prefetcher stays on the t-axis stride.
    let mut a_pack_f32: Vec<f32> = vec![0.0; M_R * k];

    // ── Outer-N cache-block size ──────────────────────────────────
    //
    // Two regimes based on whether the full `b_packed` fits in
    // Zen-3's 32 MB CCX-shared L3:
    //
    // - **Small/medium B (≤ 16 MB)**: use a single outer block
    //   covering all panels. The kernel's loop nest collapses to
    //   `for i_blk: for panel:`, packing A once per i_blk. The
    //   full `b_packed` lives in L3 across all `m / M_R` sweeps;
    //   per-i_blk B traffic comes from L3 at ~30 GB/s. This is the
    //   best path at `n = 1024` where the FMA back-end is the
    //   binding constraint.
    //
    // - **Large B (> 16 MB)**: split panels into outer blocks of
    //   size `n_c_panels` so each block's B-data fits in L2
    //   (256 KB target, half of Zen-3's 512 KB L2). Then within
    //   each outer block, sweep all i_blks before moving on. This
    //   re-uses each block's B-data across all row-tiles, bounding
    //   inner-loop B traffic to one streaming pass per outer block
    //   instead of `m / M_R` passes per panel. The cost is
    //   `n_outer_blocks × m / M_R` A-pack calls instead of `m / M_R`,
    //   but with `n_outer_blocks` ≪ `m / M_R` the trade is favorable
    //   (and `n = 4096` was hard-bound on B-bandwidth without it).
    //
    // The 16 MB threshold is half of L3; below it we keep the
    // simple loop nest, above it we cache-block. The 256 KB target
    // for the outer block is empirically tuned (smaller blocks add
    // A-pack overhead; larger blocks lose the L2 re-use win).
    let l3_threshold_bytes: usize = 16 * 1024 * 1024;
    let total_b_bytes = n_panels * k * N_R * 4;
    let n_c_panels = if total_b_bytes <= l3_threshold_bytes {
        // Single outer block — replicates original `for i_blk: for panel:`
        // loop nest with one A pre-pack per i_blk.
        n_panels.max(1)
    } else {
        let l2_budget_bytes: usize = 256 * 1024;
        let panel_bytes = k * N_R * 4;
        let blocked = l2_budget_bytes
            .checked_div(panel_bytes)
            .unwrap_or(n_panels)
            .max(1);
        blocked.min(n_panels.max(1))
    };

    // ── Inner GEMM loop (outer-N cache-blocked) ───────────────────
    //
    // Loop nesting:
    //
    //   for n_outer (group of n_c_panels panels):
    //     for i_blk:
    //       pack_a(i_blk)
    //       for panel in n_outer:
    //         run_one_panel(panel, i_blk)
    //
    // The panel slice (active B-data for one outer block) stays
    // resident in L2 for the duration of the i_blk sweep; cold
    // panel data only crosses memory once per outer block.
    //
    // Within `run_one_panel` we still split into a `m_eff = M_R = 4`
    // steady-state path and a generic `m_eff < 4` trailing path,
    // monomorphised on `M_EFF` so the inner FMA body has zero
    // branches.
    let m_full = m - (m % M_R);
    let mut n_outer = 0usize;
    while n_outer < n_panels {
        let n_outer_end = (n_outer + n_c_panels).min(n_panels);

        let mut i_blk = 0usize;
        while i_blk < m_full {
            pack_a_block::<4>(a, i_blk, k, &mut a_pack_f32);
            for panel_idx in n_outer..n_outer_end {
                let j_blk = panel_idx * N_R;
                let j_end = (j_blk + N_R).min(n);
                let n_eff = j_end - j_blk;
                let panel_off = panel_idx * panel_stride;
                run_one_panel::<4>(
                    &a_pack_f32,
                    &b_packed,
                    i_blk,
                    k,
                    n,
                    panel_off,
                    j_blk,
                    n_eff,
                    k_chunk,
                    p_i32,
                    c,
                );
            }
            i_blk += M_R;
        }
        if i_blk < m {
            let m_eff = m - i_blk;
            macro_rules! run_partial {
                ($me:literal) => {{
                    pack_a_block::<$me>(a, i_blk, k, &mut a_pack_f32);
                    for panel_idx in n_outer..n_outer_end {
                        let j_blk = panel_idx * N_R;
                        let j_end = (j_blk + N_R).min(n);
                        let n_eff = j_end - j_blk;
                        let panel_off = panel_idx * panel_stride;
                        run_one_panel::<$me>(
                            &a_pack_f32,
                            &b_packed,
                            i_blk,
                            k,
                            n,
                            panel_off,
                            j_blk,
                            n_eff,
                            k_chunk,
                            p_i32,
                            c,
                        );
                    }
                }};
            }
            match m_eff {
                1 => run_partial!(1),
                2 => run_partial!(2),
                3 => run_partial!(3),
                _ => unreachable!(),
            }
        }

        n_outer = n_outer_end;
    }
}

/// Pre-pack a `M_EFF`-row block of A into `[f32; M_R * k]` in interleaved
/// row-major form `dst[t * M_R + i] = a[(i_blk + i) * k + t]`. The
/// dst slack rows (`i ∈ [M_EFF, M_R)`) hold zeros (already initialised
/// by the caller's zero-fill).
///
/// Done once per i_blk; the inner kernel reads broadcasts from this
/// scratch buffer rather than non-contiguous strided f32 lanes.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn pack_a_block<const M_EFF: usize>(a: &[f32], i_blk: usize, k: usize, dst: &mut [f32]) {
    debug_assert!(dst.len() >= M_R * k);
    debug_assert!(M_EFF <= M_R);
    let a_base = a.as_ptr().add(i_blk * k);
    let dst_base = dst.as_mut_ptr();
    // Manual loop, branch-free over t.
    for t in 0..k {
        let dst_row = dst_base.add(t * M_R);
        if M_EFF >= 1 {
            *dst_row = *a_base.add(t);
        }
        if M_EFF >= 2 {
            *dst_row.add(1) = *a_base.add(k + t);
        }
        if M_EFF >= 3 {
            *dst_row.add(2) = *a_base.add(2 * k + t);
        }
        if M_EFF >= 4 {
            *dst_row.add(3) = *a_base.add(3 * k + t);
        }
        // Slack rows for M_EFF < M_R are pre-zeroed by the caller's
        // `vec![0.0; M_R * k]`; we leave them untouched here.
    }
}

/// Compute one `M_EFF × n_eff` output tile against one B-panel.
///
/// Monomorphisation on `M_EFF` deletes the dead FMA / sum branches
/// for `m_eff < 4`, leaving the steady-state `M_EFF = 4` body with
/// exactly 12 FMAs per inner step (no branches inside the hot loop).
///
/// `a_pack_f32` is the pre-packed A-row block: `M_R × k` f32 in
/// interleaved row-major (`a_pack_f32[t * M_R + i] = a[(i_blk + i) * k + t]`).
/// `b_packed` is the N-major B-panel buffer: `n_panels × k × N_R` f32;
/// `panel_off` and `n_eff` select the active panel slice.
#[inline]
#[target_feature(enable = "avx2,fma")]
unsafe fn run_one_panel<const M_EFF: usize>(
    a_pack_f32: &[f32],
    b_packed: &[f32],
    i_blk: usize,
    k: usize,
    n: usize,
    panel_off: usize,
    j_blk: usize,
    n_eff: usize,
    k_chunk: usize,
    p_i32: i32,
    c: &mut [u8],
) {
    {
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
            // Prefetch distance: 4 rows ahead = 4 × 96 B = 384 B (6
            // cache lines). With the inner step at ~6 cycles on Zen-3
            // and L1d miss latency ~12 cycles + L2 ~12 cycles, 4 rows
            // ahead lifts the demand misses off the FMA critical path.
            const PREFETCH_DIST: usize = 4;
            let prefetch_end = t_end.saturating_sub(PREFETCH_DIST);

            for t in t_blk..t_end {
                let b_row_ptr = b_panel_base.add(t * N_R);
                let a_row_ptr = a_pack_base.add(t * M_R);

                // Issue prefetch hints for the B-panel rows
                // PREFETCH_DIST steps ahead. Cheap (no µops on the
                // back-end FMA ports); typically lifts L1d miss
                // latency off the critical path on n ≥ 1024 cells.
                if t < prefetch_end {
                    let pf_ptr = b_row_ptr.add(PREFETCH_DIST * N_R) as *const i8;
                    _mm_prefetch::<{ _MM_HINT_T0 }>(pf_ptr);
                    // 96 B = 1.5 cache lines: nudge the second line.
                    _mm_prefetch::<{ _MM_HINT_T0 }>(pf_ptr.add(64));
                }

                // Three pure 8-lane f32 loads — no cvt instructions in
                // the inner loop (matches OpenBLAS sgemm micro-kernel
                // structure). The B-panel is already f32; loads are
                // contiguous within a single 96-byte row.
                let b0 = _mm256_loadu_ps(b_row_ptr);
                let b1 = _mm256_loadu_ps(b_row_ptr.add(8));
                let b2 = _mm256_loadu_ps(b_row_ptr.add(16));

                // 12 FMAs per inner iteration (M_EFF=4 path). The four
                // a-row broadcasts are `_mm256_broadcast_ss` from a
                // contiguous f32 address — single load+broadcast µop
                // on Zen-3 (no scalar cvt dep chain).
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

    /// Convert a `u8` canonical-residue slice to f32 lanes for input
    /// to the kernel.
    fn u8_to_f32(xs: &[u8]) -> Vec<f32> {
        xs.iter().map(|&b| b as f32).collect()
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
                let a_f = u8_to_f32(&a);
                let bt_f = u8_to_f32(&bt);
                let mut got = vec![0u8; m * n];
                unsafe { fp_small_f32_gemm(&a_f, &bt_f, m, k, n, p, &mut got) };
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
                let a_f = u8_to_f32(&a);
                let bt_f = u8_to_f32(&bt);
                let mut got = vec![0u8; m * n];
                unsafe { fp_small_f32_gemm(&a_f, &bt_f, m, k, n, p, &mut got) };
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
                let a_f = u8_to_f32(&a);
                let bt_f = u8_to_f32(&bt);
                let mut got = vec![0u8; m * n];
                unsafe { fp_small_f32_gemm(&a_f, &bt_f, m, k, n, p, &mut got) };
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
                let a_f = u8_to_f32(&a);
                let bt_f = u8_to_f32(&bt);
                let mut got = vec![0u8; m * n];
                unsafe { fp_small_f32_gemm(&a_f, &bt_f, m, k, n, p, &mut got) };
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
