//! AVX2 panelized PLE base-case kernel for small `Fp<P>` (`P <= 251`).
//!
//! Implements the panel-base path of the recursive PLE algorithm
//! described in design `dev/active/2e8c5a29-panelized-ple-design.md`
//! and issue `6823c8a0`. Performs an in-place rank-revealing PLE
//! decomposition on a column-window panel of canonical-byte storage,
//! with a row-major axpy-style Schur update that uses the SSOT
//! [`crate::x86::fp_small::barrett_reduce_lane32`] reducer at panel
//! tile boundaries.
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
//! 3. Scale: compute `inv = pivot^{-1} mod p` via the precomputed
//!    `inv_table` (one L1 table load per pivot). Then for each row
//!    `k in (rank+1)..m`, replace `scratch[k, col]` with
//!    `(scratch[k, col] * inv) mod p` — these are the L-multipliers.
//! 4. Schur update (row-major axpy form): for each row `k in
//!    (rank+1)..m`, perform
//!    `scratch[k, col+1..win] -= scratch[k, col] * scratch[rank, col+1..win]`
//!    via an AVX2 axpy lane: 8 × u32 lanes per inner step,
//!    accumulate `_mm256_madd_epi16` products, reduce mod p via
//!    `barrett_reduce_lane32`, then subtract (with conditional add
//!    mod p) and write back as canonical u8.
//! 5. Record the pivot column offset.
//!
//! The row-major form of the Schur update (one row of `y` per outer
//! iteration, against a fixed `x` = pivot row slice and scalar `mult`
//! = L-multiplier in row `k`) is algorithmically equivalent to the
//! column-major form used by `ple_base_direct` (Dumas-Pernet §2.2
//! Alg. 2.5) — same writes, just re-ordered loop nesting; no in-place
//! aliasing changes because each row `k > rank` is written independently.
//!
//! # Safety
//!
//! All public functions are `unsafe`. Caller must ensure AVX2 is
//! available at runtime, `p` is an odd prime in `[3, 251]`, and all
//! input bytes are canonical (`< p`).
//!
//! # SSOT reuse
//!
//! - Barrett reduction at panel-step boundaries reuses
//!   [`crate::x86::fp_small::barrett_reduce_lane32`] (SSOT issued by
//!   `e8a0c47a`). No new reducer.
//! - Lane width and `_mm256_madd_epi16` MAC shape match
//!   `crate::x86::fp_small_panel`'s inner kernel (issue `fc182ed5`).

#![allow(clippy::missing_safety_doc)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::needless_range_loop)]

use core::arch::x86_64::*;

/// Inner SIMD lane width (8 × u32 lanes per ymm).
const LANE_U32: usize = 8;

/// Panelized PLE base-case elimination on canonical-byte storage.
///
/// See module docs for algorithm. Performs in-place rank-revealing
/// PLE on the `m × win` canonical-byte panel `window` (row-major,
/// `window[r * win + c]` is the cell at row `r`, column `c`).
///
/// # Returns
///
/// Number of pivots found (rank contribution from this panel).
///
/// # Arguments
///
/// * `window` — `m * win` canonical-byte panel storage.
/// * `m` — row count.
/// * `win` — column count of the panel.
/// * `p` — odd prime, `3 <= p <= 251`.
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
/// AVX2 must be available at runtime. All bytes in `window` must be
/// canonical (`< p`). `p` must be an odd prime in `[3, 251]`.
/// `row_perm.len() == m`, `inv_table.len() == p as usize`,
/// `window.len() == m * win`.
#[target_feature(enable = "avx2")]
pub unsafe fn ple_panel_base_canonical(
    window: &mut [u8],
    m: usize,
    win: usize,
    p: u8,
    inv_table: &[u8],
    row_perm: &mut [usize],
    pivot_cols_local: &mut Vec<usize>,
) -> usize {
    debug_assert_eq!(
        window.len(),
        m * win,
        "ple_panel_base_canonical: window shape"
    );
    debug_assert_eq!(
        row_perm.len(),
        m,
        "ple_panel_base_canonical: row_perm length"
    );
    debug_assert_eq!(
        inv_table.len(),
        p as usize,
        "ple_panel_base_canonical: inv_table length"
    );

    if m == 0 || win == 0 {
        return 0;
    }

    let p_u32 = p as u32;
    let mu32 = ((1u64 << 32) / p_u32 as u64) as u32;
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
            swap_panel_rows(window, win, rank, piv);
            row_perm.swap(rank, piv);
        }

        // Step 3: scale. Read the pivot value, look up its inverse,
        // then update column `col` of rows (rank+1..m) to be the
        // L-multipliers `a[k, col] * inv mod p`.
        let pivot_val = *window.get_unchecked(rank * win + col);
        debug_assert!(pivot_val != 0, "panel base: zero pivot post-search");
        let inv = *inv_table.get_unchecked(pivot_val as usize) as u32;

        if rank + 1 < m {
            scale_column_below_pivot(window, win, col, rank, m, p, inv, mu_vec, p_vec32);
        }

        // Step 4: Schur update (row-major axpy form). For each row
        // `k in (rank+1)..m`, update
        //   window[k, col+1..win] -= window[k, col] * window[rank, col+1..win]  (mod p)
        if rank + 1 < m && col + 1 < win {
            schur_update_panel(window, win, col, rank, m, p, mu_vec, p_vec32);
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
/// Uses byte-wise memcpy through a stack temporary; the panel rows
/// are at most `KC = 256` bytes wide, so the swap is cheap.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn swap_panel_rows(window: &mut [u8], win: usize, r1: usize, r2: usize) {
    if r1 == r2 {
        return;
    }
    let base1 = r1 * win;
    let base2 = r2 * win;
    // SIMD-aligned 32-byte chunks where possible.
    let mut off = 0usize;
    while off + 32 <= win {
        let a_ptr = window.as_mut_ptr().add(base1 + off) as *mut __m256i;
        let b_ptr = window.as_mut_ptr().add(base2 + off) as *mut __m256i;
        let av = _mm256_loadu_si256(a_ptr);
        let bv = _mm256_loadu_si256(b_ptr);
        _mm256_storeu_si256(a_ptr, bv);
        _mm256_storeu_si256(b_ptr, av);
        off += 32;
    }
    while off < win {
        let tmp = *window.get_unchecked(base1 + off);
        *window.get_unchecked_mut(base1 + off) = *window.get_unchecked(base2 + off);
        *window.get_unchecked_mut(base2 + off) = tmp;
        off += 1;
    }
}

/// Scale column `col` below row `rank` by `inv` (mod p).
///
/// Replaces `window[k, col]` with `(window[k, col] * inv) mod p` for
/// `k in (rank+1)..m`. Uses 8-lane u32 batches with SSOT
/// `barrett_reduce_lane32` reduction.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn scale_column_below_pivot(
    window: &mut [u8],
    win: usize,
    col: usize,
    rank: usize,
    m: usize,
    p: u8,
    inv: u32,
    mu_vec: __m256i,
    p_vec32: __m256i,
) {
    let start = rank + 1;
    if start >= m {
        return;
    }
    // Column-strided access (stride = win), so we do per-element
    // scalar updates here — the scale step is O(m) per pivot column;
    // it is dominated by the O(m * win) Schur update, so scalar is fine.
    // Future optimisation: AVX2-gather batch if `m * num_pivots` becomes
    // a hotspot in profiling.
    let _ = (mu_vec, p_vec32); // unused for the scalar-stride scale path
    let p_u32 = p as u32;
    for k in start..m {
        let v = *window.get_unchecked(k * win + col) as u32;
        let mul = v * inv;
        let red = mul % p_u32;
        *window.get_unchecked_mut(k * win + col) = red as u8;
    }
}

/// Row-major Schur-complement update for one pivot column.
///
/// For each row `k in (rank+1)..m`, computes
/// `window[k, col+1..win] -= mult_k * window[rank, col+1..win]` mod p
/// where `mult_k = window[k, col]` (the L-multiplier just written in
/// the scale step).
///
/// Each row's axpy runs in 8-lane u32 SIMD blocks: load 8 canonical
/// bytes from the pivot row and from row `k`, multiply by the
/// scalar broadcast multiplier, accumulate the lane-pair MAC, reduce
/// mod p via `barrett_reduce_lane32`, then subtract the canonical
/// reduced product from row `k`'s slice with a conditional add
/// (the conditional add restores canonical range `[0, p)`).
///
/// Tail elements (fewer than 8 columns remaining) are processed scalar.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn schur_update_panel(
    window: &mut [u8],
    win: usize,
    col: usize,
    rank: usize,
    m: usize,
    p: u8,
    mu_vec: __m256i,
    p_vec32: __m256i,
) {
    let tail_start = col + 1;
    if tail_start >= win {
        return;
    }
    let tail_len = win - tail_start;

    // Snapshot the pivot row's slice into a temporary buffer so the
    // inner loop can broadcast contiguous lanes without aliasing the
    // mutable `window` slice.
    //
    // Length is at most `win <= 256`, so a 256-byte stack buffer
    // suffices.
    let mut pivot_buf = [0u8; 256];
    debug_assert!(tail_len <= 256);
    let pivot_base = rank * win + tail_start;
    for c in 0..tail_len {
        pivot_buf[c] = *window.get_unchecked(pivot_base + c);
    }
    let pivot_slice: &[u8] = &pivot_buf[..tail_len];

    let p_u32 = p as u32;
    let p_vec_sub = p_vec32; // alias for clarity below

    // For each row k in (rank+1..m): row-axpy on `window[k, tail_start..win]`.
    for k in (rank + 1)..m {
        let mult = *window.get_unchecked(k * win + col) as u32;
        if mult == 0 {
            continue; // no update needed
        }
        let mult_vec = _mm256_set1_epi32(mult as i32);

        let y_base = k * win + tail_start;

        // 8-lane batches.
        let mut off = 0usize;
        while off + LANE_U32 <= tail_len {
            // Load 8 canonical bytes from pivot row → u32 lanes.
            let pivot_ptr = pivot_slice.as_ptr().add(off);
            let pivot_lo = _mm_loadl_epi64(pivot_ptr as *const __m128i);
            let pivot_u32 = _mm256_cvtepu8_epi32(pivot_lo);

            // Compute mult * pivot per lane: 8 × u32 multiply.
            let prod = _mm256_mullo_epi32(pivot_u32, mult_vec);

            // Reduce mod p via SSOT lane32 Barrett reducer.
            let reduced = super::fp_small::barrett_reduce_lane32(prod, mu_vec, p_vec_sub);

            // Load 8 canonical bytes from row k → u32 lanes.
            let y_ptr = window.as_mut_ptr().add(y_base + off);
            let y_lo = _mm_loadl_epi64(y_ptr as *const __m128i);
            let y_u32 = _mm256_cvtepu8_epi32(y_lo);

            // y_new = (y - reduced + p) mod p
            // Compute y + p first, then subtract reduced, then conditionally
            // subtract p if the result is >= p.
            let y_plus_p = _mm256_add_epi32(y_u32, p_vec_sub);
            let diff = _mm256_sub_epi32(y_plus_p, reduced);
            // `diff` is now in `[0, 2p)` (since y < p and reduced < p so
            // y + p - reduced ∈ (0, 2p)). One conditional subtract via
            // _mm256_min_epu32 returns the canonical value:
            // min(diff, diff - p) — if diff >= p, diff - p < diff so min
            // picks diff - p; otherwise diff - p underflows huge so min
            // picks diff.
            let canon = _mm256_min_epu32(diff, _mm256_sub_epi32(diff, p_vec_sub));

            // Pack 8 i32 lanes → 8 u8 bytes (same SSOT 3-step pack used
            // by route C: packus_epi32 → permute4x64 → packus_epi16,
            // then extract the low 8 bytes).
            let packed16 = _mm256_packus_epi32(canon, canon);
            let permuted = _mm256_permute4x64_epi64::<0xD8>(packed16);
            let packed8 = _mm256_packus_epi16(permuted, permuted);
            let lower = _mm256_castsi256_si128(packed8);
            // Extract the low 8 bytes and store.
            let mut tmp = [0u8; 16];
            _mm_storeu_si128(tmp.as_mut_ptr() as *mut __m128i, lower);
            let dst_ptr = window.as_mut_ptr().add(y_base + off);
            core::ptr::copy_nonoverlapping(tmp.as_ptr(), dst_ptr, 8);

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
            *window.get_unchecked_mut(y_base + off) = raw as u8;
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

    fn build_inv_table(p: u8) -> Vec<u8> {
        let p_u32 = p as u32;
        let mut table = vec![0u8; p_u32 as usize];
        // inv_table[v] = v^{p-2} mod p (Fermat) for v ∈ [1, p).
        for v in 1..p_u32 {
            let mut result: u32 = 1;
            let mut base: u32 = v;
            let mut e: u32 = p_u32 - 2;
            while e > 0 {
                if e & 1 == 1 {
                    result = (result * base) % p_u32;
                }
                e >>= 1;
                if e > 0 {
                    base = (base * base) % p_u32;
                }
            }
            table[v as usize] = result as u8;
        }
        table
    }

    fn scalar_ple_oracle(
        window: &mut [u8],
        m: usize,
        win: usize,
        p: u8,
        row_perm: &mut [usize],
        pivot_cols_local: &mut Vec<usize>,
    ) -> usize {
        let p_u32 = p as u32;
        let mut rank = 0usize;
        // Build inverse table on the fly (Fermat).
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
                // Swap rows in window.
                for c in 0..win {
                    window.swap(rank * win + c, piv * win + c);
                }
                row_perm.swap(rank, piv);
            }
            let pivot_val = window[rank * win + col] as u32;
            let inv = inv_table[pivot_val as usize] as u32;
            for k in (rank + 1)..m {
                let v = window[k * win + col] as u32;
                window[k * win + col] = ((v * inv) % p_u32) as u8;
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
                    window[k * win + c] = raw as u8;
                }
            }
            pivot_cols_local.push(col);
            rank += 1;
        }
        rank
    }

    fn run_for_primes(test: impl Fn(u8)) {
        if !std::arch::is_x86_feature_detected!("avx2") {
            return;
        }
        for &p in &[3u8, 5, 7, 11, 13, 17, 31, 127, 241, 251] {
            test(p);
        }
    }

    #[test]
    fn ple_panel_base_matches_scalar_oracle_full_rank() {
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
                (4, 256),
                (8, 256),
                (256, 64),
            ];
            for &(m, win) in cases {
                // Deterministic pseudo-random window.
                let mut window: Vec<u8> = (0..(m * win) as u32)
                    .map(|i| ((i * 17 + 1) % p as u32) as u8)
                    .collect();
                let mut window_oracle = window.clone();
                let mut row_perm: Vec<usize> = (0..m).collect();
                let mut pivot_cols: Vec<usize> = Vec::new();
                let mut row_perm_oracle: Vec<usize> = (0..m).collect();
                let mut pivot_cols_oracle: Vec<usize> = Vec::new();

                let rank = unsafe {
                    ple_panel_base_canonical(
                        &mut window,
                        m,
                        win,
                        p,
                        &inv_table,
                        &mut row_perm,
                        &mut pivot_cols,
                    )
                };
                let rank_oracle = scalar_ple_oracle(
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
    fn ple_panel_base_rank_deficient_zero_matrix() {
        run_for_primes(|p| {
            let inv_table = build_inv_table(p);
            let m = 8;
            let win = 8;
            let mut window = vec![0u8; m * win];
            let mut row_perm: Vec<usize> = (0..m).collect();
            let mut pivot_cols: Vec<usize> = Vec::new();
            let rank = unsafe {
                ple_panel_base_canonical(
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
    fn ple_panel_base_rank_deficient_scattered_pivots() {
        run_for_primes(|p| {
            let inv_table = build_inv_table(p);
            // 4×8 matrix where columns 1, 3, 6 are pivot columns and
            // columns 0, 2, 4, 5, 7 are zero. Rank should be 3 and
            // pivot_cols should be [1, 3, 6].
            let m = 4;
            let win = 8;
            let mut window = vec![0u8; m * win];
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
                ple_panel_base_canonical(
                    &mut window,
                    m,
                    win,
                    p,
                    &inv_table,
                    &mut row_perm,
                    &mut pivot_cols,
                )
            };
            let rank_oracle = scalar_ple_oracle(
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
