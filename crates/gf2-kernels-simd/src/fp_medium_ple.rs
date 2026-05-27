//! AVX2 panelized PLE base-case kernel for medium `Fp<P>` (`P ∈
//! (251, 65536)`), issue `68db401b`, design `2e8c5a29` § 9.
//!
//! This is the safe wrapper layer; the unsafe AVX2 intrinsics live in
//! [`crate::x86::fp_medium_ple`]. The kernel is the u16-lane analogue
//! of the byte-lane [`crate::fp_small_ple`] panel-base kernel — same
//! column-by-column Gaussian elimination as the scalar
//! `ple_base_direct`, but with a row-major axpy-style Schur update
//! that processes 8 × u32 lanes per inner step via SSOT-reused
//! `_mm256_mullo_epi32` + Barrett reduction
//! ([`crate::x86::fp_small::barrett_reduce_lane32`], SSOT issued by
//! `e8a0c47a`).
//!
//! # Algorithm summary
//!
//! See `dev/active/2e8c5a29-panelized-ple-design.md` § 9 for the
//! design context (the original design declined a u16 base case;
//! issue `68db401b` reverses that decision based on the R0 evidence
//! that GF(65521) PLE cells did not close to ≤ 1.5× via the Schur
//! path alone).
//!
//! The base-case kernel processes an `m × win` column window of
//! canonical u16 storage in-place, performing:
//!   1. Linear-scan pivot search (rank-revealing, preserves the
//!      bd9c6e13 scattered-column behaviour).
//!   2. Full-row swap on the **panel window only** (caller propagates
//!      to cells outside the window via the returned `row_perm`).
//!   3. L-multiplier scale fused with the Schur update (scalar mod-mul
//!      for the multiplier, SIMD axpy for the tail).
//!   4. AVX2 row-major axpy Schur update with 8-lane Barrett reduction.
//!
//! # Coverage scope
//!
//! Activates exclusively for `Fp<P>` with `P ∈ (251, 65536)` on AVX2
//! hosts. For `P ≤ 251` the dedicated byte-lane kernel
//! ([`crate::fp_small_ple`]) is used. For `P ≥ 65536` the generic
//! 64-bit Montgomery path remains.

/// Whole panelized PLE base-case signature (u16-lane variant).
///
/// `window` is the canonical u16 panel storage (row-major,
/// `window[r * win + c]` is the cell at row `r`, column offset `c`).
/// `inv_table[v]` is the modular inverse of `v` for `v ∈ [1, p)`; the
/// caller builds this table once per prime.
///
/// Returns the number of pivots found in this panel. `row_perm` is
/// mutated to reflect the kernel's row swaps; `pivot_cols_local`
/// receives the panel-relative pivot column offsets in left-to-right
/// order.
///
/// # Safety contract
///
/// All function pointers in this struct are **safe `fn`** —
/// internally they dispatch to AVX2 intrinsics under an `unsafe`
/// block only after [`detect`] has confirmed AVX2 is available at
/// runtime, exactly mirroring the [`crate::fp_small_ple`] safe
/// wrapper pattern. Callers must still ensure `p ∈ (251, 65536)`,
/// `window.len() == m * win`, `inv_table.len() == p as usize`,
/// `row_perm.len() == m`, and every lane in `window` is canonical
/// (`< p`). These preconditions are debug-asserted by the kernel.
pub type MediumPrimePlePanelBaseFn = fn(
    window: &mut [u16],
    m: usize,
    win: usize,
    p: u16,
    inv_table: &[u16],
    row_perm: &mut [usize],
    pivot_cols_local: &mut Vec<usize>,
) -> usize;

/// Bundle of medium-prime panelized PLE operations (issue `68db401b`).
///
/// Populated at runtime by [`detect`] when AVX2 is available. The
/// function pointer takes the prime `p` as a runtime argument so one
/// dispatch struct covers every medium-prime consumer (`252 ≤ P ≤
/// 65535`).
#[derive(Copy, Clone)]
pub struct MediumPrimePlePanelFns {
    /// Panelized PLE base-case kernel for canonical u16 `Fp<P>` with
    /// `P ∈ (251, 65536)`.
    pub ple_panel_base_fn: MediumPrimePlePanelBaseFn,
}

/// Detect and return the best available medium-prime panelized PLE
/// base-case kernel.
///
/// Returns `None` on non-x86 targets or when the runtime CPU lacks
/// AVX2. Callers receive `None` and must fall back to the scalar
/// `ple_base_direct` path.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::fp_medium_ple;
///
/// if let Some(fns) = fp_medium_ple::detect() {
///     // p = 257 (smallest medium prime); identity 2x2 panel.
///     // Inverse table: v^{-1} mod 257 for v ∈ [1, 257).
///     let p = 257u16;
///     let mut inv_table = vec![0u16; p as usize];
///     for v in 1..p as u32 {
///         // Fermat: v^{p-2} mod p.
///         let mut r: u32 = 1;
///         let mut b: u32 = v;
///         let mut e: u32 = p as u32 - 2;
///         while e > 0 {
///             if e & 1 == 1 { r = (r * b) % p as u32; }
///             e >>= 1;
///             if e > 0 { b = (b * b) % p as u32; }
///         }
///         inv_table[v as usize] = r as u16;
///     }
///     let mut window = [1u16, 0, 0, 1];
///     let mut row_perm = [0usize, 1];
///     let mut pivot_cols: Vec<usize> = Vec::new();
///     let rank = (fns.ple_panel_base_fn)(
///         &mut window, 2, 2, p, &inv_table, &mut row_perm, &mut pivot_cols,
///     );
///     assert_eq!(rank, 2);
///     assert_eq!(pivot_cols, vec![0, 1]);
///     assert_eq!(row_perm, [0, 1]);
/// }
/// ```
pub fn detect() -> Option<MediumPrimePlePanelFns> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return detect_x86();
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_x86() -> Option<MediumPrimePlePanelFns> {
    use std::arch::is_x86_feature_detected;
    if is_x86_feature_detected!("avx2") {
        Some(MediumPrimePlePanelFns {
            ple_panel_base_fn: ple_panel_base_safe,
        })
    } else {
        None
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn ple_panel_base_safe(
    window: &mut [u16],
    m: usize,
    win: usize,
    p: u16,
    inv_table: &[u16],
    row_perm: &mut [usize],
    pivot_cols_local: &mut Vec<usize>,
) -> usize {
    // Safety: `detect_x86` only published this pointer when AVX2 is
    // available at runtime. The unsafe pre-conditions (canonical u16
    // lanes, prime in (251, 65536), slice lengths) are documented on
    // the outer `ple_panel_base_fn` contract and enforced by the
    // gf2-core dispatch site (which packs canonical u16 via the
    // medium-prime `Fp::value()` round-trip) plus the kernel's own
    // debug_asserts.
    unsafe {
        crate::x86::fp_medium_ple::ple_panel_base_canonical_u16(
            window,
            m,
            win,
            p,
            inv_table,
            row_perm,
            pivot_cols_local,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_inv_table(p: u16) -> Vec<u16> {
        let p_u64 = p as u64;
        let mut table = vec![0u16; p_u64 as usize];
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

    #[test]
    fn detect_returns_some_on_avx2() {
        let fns = detect();
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            use std::arch::is_x86_feature_detected;
            if is_x86_feature_detected!("avx2") {
                assert!(fns.is_some());
            }
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            assert!(fns.is_none());
        }
    }

    #[test]
    fn safe_wrapper_matches_scalar_oracle() {
        let fns = match detect() {
            Some(f) => f,
            None => return,
        };
        for &p in &[257u16, 8191, 65521] {
            let inv_table = build_inv_table(p);
            for &(m, win) in &[(4usize, 4usize), (8, 8), (16, 16), (15, 17), (32, 16)] {
                let mut window: Vec<u16> = (0..(m * win) as u32)
                    .map(|i| ((i.wrapping_mul(11) + 7) % p as u32) as u16)
                    .collect();
                let mut window_oracle = window.clone();
                let mut row_perm: Vec<usize> = (0..m).collect();
                let mut pivot_cols: Vec<usize> = Vec::new();
                let mut row_perm_oracle: Vec<usize> = (0..m).collect();
                let mut pivot_cols_oracle: Vec<usize> = Vec::new();

                let rank = (fns.ple_panel_base_fn)(
                    &mut window,
                    m,
                    win,
                    p,
                    &inv_table,
                    &mut row_perm,
                    &mut pivot_cols,
                );
                let rank_oracle = scalar_oracle(
                    &mut window_oracle,
                    m,
                    win,
                    p,
                    &mut row_perm_oracle,
                    &mut pivot_cols_oracle,
                );
                assert_eq!(rank, rank_oracle, "p={p} m={m} win={win}");
                assert_eq!(window, window_oracle, "p={p} m={m} win={win}");
                assert_eq!(pivot_cols, pivot_cols_oracle, "p={p} m={m} win={win}");
                assert_eq!(row_perm, row_perm_oracle, "p={p} m={m} win={win}");
            }
        }
    }

    fn scalar_oracle(
        window: &mut [u16],
        m: usize,
        win: usize,
        p: u16,
        row_perm: &mut [usize],
        pivot_cols_local: &mut Vec<usize>,
    ) -> usize {
        let p_u32 = p as u32;
        let inv_table = build_inv_table(p);
        let mut rank = 0usize;

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
}
