//! AVX2 panelized PLE base-case kernel for medium `Fp<P>` with
//! `P ∈ (251, 65536)`, issue `68db401b`, design `2e8c5a29` § 9.
//!
//! Implements the u16-lane analogue of the byte-lane panel-base
//! kernel in [`crate::x86::fp_small_ple`]. Performs an in-place
//! rank-revealing PLE decomposition on a column-window panel of
//! canonical u16 storage, with a row-major axpy-style Schur update
//! that uses the SSOT [`crate::x86::fp_small::barrett_reduce_lane32`]
//! reducer at panel tile boundaries.
//!
//! # Algorithm
//!
//! For each column `col` in `[0, win)` of the panel:
//! 1. Pivot search: linear scan rows `rank..m` for the first non-zero
//!    entry at column `col`.
//! 2. Row swap: if pivot row != `rank`, swap **both** the scratch
//!    window's rows and the kernel-local `row_perm` tracker. The
//!    caller is responsible for propagating the final `row_perm` to
//!    the parent matrix's cells **outside** the window.
//! 3. Fused scale + Schur update (row-major axpy form): compute `inv =
//!    pivot^{-1} mod p` once. For each row `k in (rank+1)..m`,
//!    (a) compute the L-multiplier `mult = window[k, col] * inv mod p`
//!    (scalar mod-mul, written back to `window[k, col]`);
//!    (b) update `window[k, col+1..win] -= mult * window[rank,
//!    col+1..win]` (mod p) via AVX2: 8-lane u32 MUL + SSOT
//!    `barrett_reduce_lane32` + branchless cond-sub pack.
//! 4. Record the pivot column offset.
//!
//! The row-major form of the Schur update (one row of `y` per outer
//! iteration, against a fixed `x` = pivot row slice and scalar `mult`
//! = L-multiplier in row `k`) is algorithmically equivalent to the
//! column-major form used by `ple_base_direct` (Dumas-Pernet §2.2 Alg.
//! 2.5): same writes, just re-ordered loop nesting. No in-place
//! aliasing changes because each row `k > rank` is written
//! independently.
//!
//! # Lane width
//!
//! The kernel operates on **canonical u16** lanes (not Montgomery
//! storage). One AVX2 ymm holds 16 u16 lanes; for the Schur update we
//! widen 8 of these to u32 per tile (via `_mm256_cvtepu16_epi32`),
//! multiply against the broadcast multiplier (also 8 u32 lanes), call
//! `barrett_reduce_lane32`, and pack 8 u32 → 8 u16 via
//! `_mm256_packus_epi32` (clamping is a no-op since reduced values are
//! in `[0, p) ⊂ [0, 2^16)`).
//!
//! Choosing an 8-lane tile (rather than 16) matches the SSOT
//! `barrett_reduce_lane32` primitive's 8-u32-lane shape; processing 16
//! u16 lanes per outer iteration would need two reduces and is no
//! faster on Zen 3 because the multiply throughput is the same.
//!
//! # Safety
//!
//! All public functions are `unsafe`. Caller must ensure AVX2 is
//! available at runtime, `p` is an odd prime in `(251, 65536)`, and
//! all input u16 lanes are canonical (`< p`).
//!
//! # SSOT reuse
//!
//! - Barrett reduction at panel-step boundaries reuses
//!   [`crate::x86::fp_small::barrett_reduce_lane32`] (SSOT issued by
//!   `e8a0c47a`). No new reducer.
//! - Lane shape matches `crate::x86::fp_medium::fp_medium_batch_mul16`
//!   (the medium-prime u32 widening + Barrett pack pattern, issue
//!   `9e12659b`).

#![allow(clippy::missing_safety_doc)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::needless_range_loop)]

use core::arch::x86_64::*;

/// Inner SIMD lane width (8 × u32 lanes per ymm).
const LANE_U32: usize = 8;

/// Panelized PLE base-case elimination on canonical u16 storage.
///
/// See module docs for algorithm. Performs in-place rank-revealing
/// PLE on the `m × win` canonical u16 panel `window` (row-major,
/// `window[r * win + c]` is the cell at row `r`, column `c`).
///
/// # Returns
///
/// Number of pivots found (rank contribution from this panel).
///
/// # Arguments
///
/// * `window` — `m * win` canonical u16 panel storage.
/// * `m` — row count.
/// * `win` — column count of the panel.
/// * `p` — odd prime, `252 <= p <= 65535`.
/// * `inv_table` — length `p`; `inv_table[v]` is the modular inverse
///   of `v` for `v ∈ [1, p)`. `inv_table[0]` is unused.
/// * `row_perm` — length `m`, mutated to track row swaps performed by
///   the kernel. The caller is responsible for propagating the final
///   `row_perm` to the parent matrix's cells outside the column window.
/// * `pivot_cols_local` — pivot column offsets within `[0, win)` are
///   pushed in left-to-right order.
///
/// # Safety
///
/// AVX2 must be available at runtime. All lanes in `window` must be
/// canonical (`< p`). `p` must be an odd prime in `(251, 65536)`.
/// `row_perm.len() == m`, `inv_table.len() == p as usize`,
/// `window.len() == m * win`.
#[target_feature(enable = "avx2")]
pub unsafe fn ple_panel_base_canonical_u16(
    window: &mut [u16],
    m: usize,
    win: usize,
    p: u16,
    inv_table: &[u16],
    row_perm: &mut [usize],
    pivot_cols_local: &mut Vec<usize>,
) -> usize {
    debug_assert_eq!(
        window.len(),
        m * win,
        "ple_panel_base_canonical_u16: window shape"
    );
    debug_assert_eq!(
        row_perm.len(),
        m,
        "ple_panel_base_canonical_u16: row_perm length"
    );
    debug_assert_eq!(
        inv_table.len(),
        p as usize,
        "ple_panel_base_canonical_u16: inv_table length"
    );
    debug_assert!(p > 251, "ple_panel_base_canonical_u16: p must be > 251");

    if m == 0 || win == 0 {
        return 0;
    }

    let p_u32 = p as u32;
    let mu32 = ((1u64 << 32) / p_u32 as u64) as u32;
    // The SSOT `barrett_reduce_lane32` reads only the low 32 bits of
    // each 64-bit lane internally, so `_mm256_set1_epi64x(mu32 as i64)`
    // is the canonical broadcast shape (matches `fp_small.rs:840`).
    let mu_vec = _mm256_set1_epi64x(mu32 as i64);
    let p_vec32 = _mm256_set1_epi32(p_u32 as i32);

    let mut rank = 0usize;

    for col in 0..win {
        if rank >= m {
            break;
        }

        // Step 1: pivot search (rows [rank..m] of column `col`).
        let mut pivot_row: Option<usize> = None;
        for i in rank..m {
            if *window.get_unchecked(i * win + col) != 0 {
                pivot_row = Some(i);
                break;
            }
        }
        let Some(piv) = pivot_row else { continue };

        // Step 2: swap row `piv` into row `rank` (whole row of the
        // window panel; the caller handles outside-window cells via
        // `row_perm`).
        if piv != rank {
            swap_panel_rows_u16(window, win, rank, piv);
            row_perm.swap(rank, piv);
        }

        // Step 3+4: fused scale + Schur update.
        //
        // For each row `k in (rank+1)..m`:
        //   - Compute multiplier `mult = window[k, col] * inv mod p`
        //     and write back to `window[k, col]` (L-multiplier).
        //   - Update tail `window[k, col+1..win] -= mult *
        //     window[rank, col+1..win]` (mod p) via AVX2 axpy.
        //
        // Fusing eliminates the separate column-strided scalar scale
        // pass and lets us keep the multiplier `mult` in a register
        // across the SIMD tail update.
        let pivot_val = *window.get_unchecked(rank * win + col);
        debug_assert!(pivot_val != 0, "panel base: zero pivot post-search");
        let inv = *inv_table.get_unchecked(pivot_val as usize) as u32;

        if rank + 1 < m {
            fused_scale_and_schur_u16(window, win, col, rank, m, p, inv, mu_vec, p_vec32);
        }

        // Step 5: record this pivot's column offset (local within
        // window) and advance the rank.
        pivot_cols_local.push(col);
        rank += 1;
    }

    rank
}

/// Swap two rows of the row-major window panel.
///
/// Uses 16-byte (8-u16) and 32-byte (16-u16) AVX2 chunks where
/// possible; the panel rows are at most `KC = 128` u16 wide
/// (256 bytes), so the swap is cheap.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn swap_panel_rows_u16(window: &mut [u16], win: usize, r1: usize, r2: usize) {
    if r1 == r2 {
        return;
    }
    let base1 = r1 * win;
    let base2 = r2 * win;
    // SIMD-aligned 16-u16 chunks (32 bytes) where possible.
    let mut off = 0usize;
    while off + 16 <= win {
        let a_ptr = window.as_mut_ptr().add(base1 + off) as *mut __m256i;
        let b_ptr = window.as_mut_ptr().add(base2 + off) as *mut __m256i;
        let av = _mm256_loadu_si256(a_ptr);
        let bv = _mm256_loadu_si256(b_ptr);
        _mm256_storeu_si256(a_ptr, bv);
        _mm256_storeu_si256(b_ptr, av);
        off += 16;
    }
    while off < win {
        let tmp = *window.get_unchecked(base1 + off);
        *window.get_unchecked_mut(base1 + off) = *window.get_unchecked(base2 + off);
        *window.get_unchecked_mut(base2 + off) = tmp;
        off += 1;
    }
}

/// Fused scale + Schur-complement update for one pivot column.
///
/// For each row `k in (rank+1)..m`:
///   1. Compute the L-multiplier `mult = window[k, col] * inv mod p`
///      and write it back into `window[k, col]`. This replaces the
///      separate column-strided scale loop.
///   2. Update the tail `window[k, col+1..win] -= mult *
///      window[rank, col+1..win]` (mod p) via AVX2 axpy:
///      - widen 8 pivot u16 lanes to u32 (`_mm256_cvtepu16_epi32`),
///      - multiply by `mult` (8 × u32 mul; exact since
///        `(p-1)^2 < 2^32`),
///      - SSOT Barrett reduce 8 u32 → 8 u32 in `[0, p)`,
///      - subtract from y (widened to u32), conditional-add p,
///      - pack 8 u32 → 8 u16 via `_mm256_packus_epi32`.
///
/// Fusing keeps `mult` in a register and removes the separate scalar
/// pass over the pivot column below the pivot row, cutting the
/// per-pivot scalar work from `O(m)` mod-muls to one mod-mul per row
/// (still required for the multiplier computation).
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn fused_scale_and_schur_u16(
    window: &mut [u16],
    win: usize,
    col: usize,
    rank: usize,
    m: usize,
    p: u16,
    inv: u32,
    mu_vec: __m256i,
    p_vec32: __m256i,
) {
    let tail_start = col + 1;
    let tail_len = win - tail_start;

    // Snapshot the pivot row's tail into a stack buffer so the inner
    // loop can broadcast contiguous lanes without aliasing the
    // mutable `window` slice. The tail length is at most `win <= 128`
    // (PLE_PANEL_COLS_U16 = 128), bounded here at 256 for safety.
    let mut pivot_buf = [0u16; 256];
    debug_assert!(
        tail_len <= 256,
        "fused_scale_and_schur_u16: tail_len > 256 (win exceeds PLE_PANEL_COLS_U16 bound)"
    );
    if tail_len > 0 {
        let pivot_base = rank * win + tail_start;
        for c in 0..tail_len {
            pivot_buf[c] = *window.get_unchecked(pivot_base + c);
        }
    }
    let pivot_slice: &[u16] = &pivot_buf[..tail_len];

    let p_u32 = p as u32;

    // For each row k in (rank+1..m).
    for k in (rank + 1)..m {
        // Step 1: compute multiplier and write back as L-multiplier.
        let v = *window.get_unchecked(k * win + col) as u32;
        let mult = (v * inv) % p_u32;
        *window.get_unchecked_mut(k * win + col) = mult as u16;
        if mult == 0 || tail_len == 0 {
            continue;
        }
        let mult_vec = _mm256_set1_epi32(mult as i32);

        let y_base = k * win + tail_start;

        // 8-lane batches (8 u16 in, 8 u16 out per iteration).
        let mut off = 0usize;
        while off + LANE_U32 <= tail_len {
            // Load 8 canonical u16 from pivot row → 8 u32 lanes.
            let pivot_ptr = pivot_slice.as_ptr().add(off);
            let pivot_lo = _mm_loadu_si128(pivot_ptr as *const __m128i);
            let pivot_u32 = _mm256_cvtepu16_epi32(pivot_lo);

            // Compute mult * pivot per lane: 8 × u32 multiply. Exact
            // because `(p-1)^2 < 2^32` for any `p < 2^16`.
            let prod = _mm256_mullo_epi32(pivot_u32, mult_vec);

            // Reduce mod p via SSOT lane32 Barrett reducer.
            let reduced = super::fp_small::barrett_reduce_lane32(prod, mu_vec, p_vec32);

            // Load 8 canonical u16 from row k → u32 lanes.
            let y_ptr = window.as_mut_ptr().add(y_base + off);
            let y_lo = _mm_loadu_si128(y_ptr as *const __m128i);
            let y_u32 = _mm256_cvtepu16_epi32(y_lo);

            // y_new = (y - reduced + p) mod p
            // Compute y + p first, then subtract reduced, then
            // conditionally subtract p if the result is >= p.
            let y_plus_p = _mm256_add_epi32(y_u32, p_vec32);
            let diff = _mm256_sub_epi32(y_plus_p, reduced);
            // `diff` is now in `[1, 2p-1]` (since y < p and reduced < p
            // so y + p - reduced ∈ [1, 2p-1]). One conditional subtract
            // via _mm256_min_epu32 returns the canonical value:
            // min(diff, diff - p) — if diff >= p, diff - p < diff so
            // min picks diff - p; otherwise diff - p underflows huge so
            // min picks diff.
            let canon = _mm256_min_epu32(diff, _mm256_sub_epi32(diff, p_vec32));

            // Pack 8 i32 lanes → 8 u16. `_mm256_packus_epi32` saturates
            // negative inputs to zero, but our `canon` is already in
            // `[0, p) ⊂ [0, 2^16)` so saturation never engages. The
            // packus interleaves the two 128-bit halves; for a single
            // 8-lane source vector we self-pack and extract the low 8
            // u16 lanes.
            let packed16 = _mm256_packus_epi32(canon, canon);
            // After packus, lanes 0..3 are canon lanes 0..3 (low half),
            // lanes 4..7 are canon lanes 0..3 again (low-half dup),
            // lanes 8..11 are canon lanes 4..7 (high half), lanes 12..15
            // are canon lanes 4..7 dup. We need lanes 0..3 from the low
            // 128 and 8..11 from the high 128 → permute4x64<0xD8>
            // swizzles 64-bit lanes [0,1,2,3] → [0,2,1,3].
            let permuted = _mm256_permute4x64_epi64::<0xD8>(packed16);
            // Now the low 128 of `permuted` holds 8 contiguous u16 lanes
            // in canonical order. Store via the low half.
            let lower = _mm256_castsi256_si128(permuted);
            let dst_ptr = window.as_mut_ptr().add(y_base + off) as *mut __m128i;
            _mm_storeu_si128(dst_ptr, lower);

            off += LANE_U32;
        }
        // Scalar tail.
        while off < tail_len {
            let yc = *window.get_unchecked(y_base + off) as u32;
            let xc = pivot_slice[off] as u32;
            let prod = (mult * xc) % p_u32;
            // y - prod (mod p) with one conditional add.
            let raw = if yc >= prod {
                yc - prod
            } else {
                yc + p_u32 - prod
            };
            *window.get_unchecked_mut(y_base + off) = raw as u16;
            off += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn build_inv_table(p: u16) -> Vec<u16> {
        let p_u64 = p as u64;
        let mut table = vec![0u16; p_u64 as usize];
        // inv_table[v] = v^{p-2} mod p (Fermat) for v ∈ [1, p).
        for v in 1..p_u64 {
            let mut result: u64 = 1;
            let mut base: u64 = v;
            let mut e: u64 = p_u64 - 2;
            while e > 0 {
                if e & 1 == 1 {
                    result = (result * base) % p_u64;
                }
                e >>= 1;
                if e > 0 {
                    base = (base * base) % p_u64;
                }
            }
            table[v as usize] = result as u16;
        }
        table
    }

    fn scalar_ple_oracle_u16(
        window: &mut [u16],
        m: usize,
        win: usize,
        p: u16,
        row_perm: &mut [usize],
        pivot_cols_local: &mut Vec<usize>,
    ) -> usize {
        let p_u32 = p as u32;
        let mut rank = 0usize;
        let inv_table = build_inv_table(p);

        for col in 0..win {
            if rank >= m {
                break;
            }
            let mut pivot_row: Option<usize> = None;
            for i in rank..m {
                if window[i * win + col] != 0 {
                    pivot_row = Some(i);
                    break;
                }
            }
            let Some(piv) = pivot_row else { continue };
            if piv != rank {
                for c in 0..win {
                    window.swap(rank * win + c, piv * win + c);
                }
                row_perm.swap(rank, piv);
            }
            let pivot_val = window[rank * win + col] as u32;
            let inv = inv_table[pivot_val as usize] as u32;
            for k in (rank + 1)..m {
                let v = window[k * win + col] as u32;
                window[k * win + col] = ((v * inv) % p_u32) as u16;
            }
            for c in (col + 1)..win {
                let pivot_c = window[rank * win + c] as u32;
                if pivot_c == 0 {
                    continue;
                }
                for k in (rank + 1)..m {
                    let mult = window[k * win + col] as u32;
                    let prod = (mult * pivot_c) % p_u32;
                    let yc = window[k * win + c] as u32;
                    let raw = if yc >= prod {
                        yc - prod
                    } else {
                        yc + p_u32 - prod
                    };
                    window[k * win + c] = raw as u16;
                }
            }
            pivot_cols_local.push(col);
            rank += 1;
        }
        rank
    }

    fn run_for_primes(test: impl Fn(u16)) {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        // Cover small medium primes and the reference prime 65521.
        // 257 = smallest medium prime above the byte-lane cutoff.
        // 65521 = reference (largest prime under 2^16).
        for &p in &[257u16, 1009, 8191, 32771, 65521] {
            test(p);
        }
    }

    #[test]
    fn ple_panel_base_u16_matches_scalar_oracle_full_rank() {
        run_for_primes(|p| {
            let inv_table = build_inv_table(p);
            let cases: &[(usize, usize)] = &[
                (1, 1),
                (4, 4),
                (8, 8),
                (16, 16),
                (32, 32),
                (4, 8),
                (8, 4),
                (3, 5),
                (5, 7),
                (17, 16),
                (16, 17),
                (64, 16),
                (16, 64),
                (64, 64),
                (15, 17),
                (4, 128),
                (8, 128),
                (128, 64),
            ];
            for &(m, win) in cases {
                // Deterministic pseudo-random window.
                let mut window: Vec<u16> = (0..(m * win) as u32)
                    .map(|i| ((i.wrapping_mul(17) + 1) % p as u32) as u16)
                    .collect();
                let mut window_oracle = window.clone();
                let mut row_perm: Vec<usize> = (0..m).collect();
                let mut pivot_cols: Vec<usize> = Vec::new();
                let mut row_perm_oracle: Vec<usize> = (0..m).collect();
                let mut pivot_cols_oracle: Vec<usize> = Vec::new();

                let rank = unsafe {
                    ple_panel_base_canonical_u16(
                        &mut window,
                        m,
                        win,
                        p,
                        &inv_table,
                        &mut row_perm,
                        &mut pivot_cols,
                    )
                };
                let rank_oracle = scalar_ple_oracle_u16(
                    &mut window_oracle,
                    m,
                    win,
                    p,
                    &mut row_perm_oracle,
                    &mut pivot_cols_oracle,
                );
                assert_eq!(rank, rank_oracle, "rank mismatch p={p} m={m} win={win}");
                assert_eq!(
                    pivot_cols, pivot_cols_oracle,
                    "pivot_cols mismatch p={p} m={m} win={win}"
                );
                assert_eq!(
                    row_perm, row_perm_oracle,
                    "row_perm mismatch p={p} m={m} win={win}"
                );
                assert_eq!(
                    window, window_oracle,
                    "window mismatch p={p} m={m} win={win}"
                );
            }
        });
    }

    #[test]
    fn ple_panel_base_u16_rank_deficient_zero_matrix() {
        run_for_primes(|p| {
            let inv_table = build_inv_table(p);
            let m = 8;
            let win = 8;
            let mut window = vec![0u16; m * win];
            let mut row_perm: Vec<usize> = (0..m).collect();
            let mut pivot_cols: Vec<usize> = Vec::new();
            let rank = unsafe {
                ple_panel_base_canonical_u16(
                    &mut window,
                    m,
                    win,
                    p,
                    &inv_table,
                    &mut row_perm,
                    &mut pivot_cols,
                )
            };
            assert_eq!(rank, 0, "zero matrix should have rank 0 (p={p})");
            assert!(pivot_cols.is_empty(), "no pivots on zero matrix (p={p})");
        });
    }

    #[test]
    fn ple_panel_base_u16_rank_deficient_scattered_pivots() {
        run_for_primes(|p| {
            let inv_table = build_inv_table(p);
            // 4×8 matrix where columns 1, 3, 6 are pivot columns and
            // columns 0, 2, 4, 5, 7 are zero. Rank should be 3 and
            // pivot_cols should be [1, 3, 6].
            let m = 4;
            let win = 8;
            let mut window = vec![0u16; m * win];
            // row 0 has a 1 at col 1.
            window[1] = 1;
            // row 1 has a 1 at col 3.
            window[win + 3] = 1;
            // row 2 has a 1 at col 6.
            window[2 * win + 6] = 1;
            // row 3 is all zero.

            let mut window_oracle = window.clone();
            let mut row_perm: Vec<usize> = (0..m).collect();
            let mut pivot_cols: Vec<usize> = Vec::new();
            let mut row_perm_oracle: Vec<usize> = (0..m).collect();
            let mut pivot_cols_oracle: Vec<usize> = Vec::new();

            let rank = unsafe {
                ple_panel_base_canonical_u16(
                    &mut window,
                    m,
                    win,
                    p,
                    &inv_table,
                    &mut row_perm,
                    &mut pivot_cols,
                )
            };
            let rank_oracle = scalar_ple_oracle_u16(
                &mut window_oracle,
                m,
                win,
                p,
                &mut row_perm_oracle,
                &mut pivot_cols_oracle,
            );
            assert_eq!(rank, rank_oracle, "rank mismatch (scattered) p={p}");
            assert_eq!(rank, 3, "expected rank 3 p={p}");
            assert_eq!(pivot_cols, vec![1, 3, 6], "pivot_cols p={p}");
            assert_eq!(pivot_cols, pivot_cols_oracle, "pivot_cols vs oracle p={p}");
            assert_eq!(window, window_oracle, "window vs oracle p={p}");
        });
    }
}
