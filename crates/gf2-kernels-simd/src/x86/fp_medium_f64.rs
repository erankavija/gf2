//! AVX2 + FMA3 (`_mm256_fmadd_pd`) f64-cascade GEMM kernel for medium
//! `Fp<P>` with `P ∈ (251, 65536)` — the "Phase 6e f64 cascade" route
//! filed under jit issue `0749dbad`.
//!
//! Mirrors the structure of `crate::x86::fp_small_f32` (Route A's f32
//! cascade for `P ≤ 251`) at f64 lane density, sized for medium primes
//! where the canonical residue range `[0, p)` no longer fits in a byte
//! lane. The design rationale and post-mortem evidence live at
//! `dev/bench_results/2026-05-26-695350fd-fp-medium-blis.md` § 9.
//!
//! # Algorithm
//!
//! For canonical residues `a, b ∈ [0, p)` arriving as **`f64` lanes**
//! (one residue per lane), each gemm call:
//!
//! 1. **Pack pass.** `bt: &[f64]` (n × k row-major) is repacked into
//!    N-major f64 panels of width `N_R = 12` (each panel `k × N_R`
//!    f64). `a` is consumed in place — no auxiliary `Vec<f64>` is
//!    allocated for A pre-pack beyond the existing scratch. Inputs
//!    arrive as f64 from the caller's outer pre-pack of `&[Fp<P>]` →
//!    `Vec<f64>`; the kernel's inner loop performs **no cvt
//!    instructions**, mirroring the fflas-ffpack `Modular<double>`
//!    micro-kernel structure.
//!
//! 2. **Inner micro-kernel.** A BLIS-class register-blocked dgemm
//!    micro-kernel with tile shape `m_R × n_R = 4 × 12` (12
//!    accumulator AVX2 registers + 3 B-tile registers + 1 broadcast =
//!    16/16 register file). Each `_mm256_fmadd_pd(b_tile_j,
//!    a_broadcast_i, acc_ij)` issues at 0.5-cycle reciprocal
//!    throughput on Zen-3's two FMA execution ports. Prefetch hints
//!    (`_MM_HINT_T0`) on B-panel rows four steps ahead lift the
//!    L1d-miss latency off the critical path on `n ≥ 1024` cells.
//!
//!    With `k ≤ 4096` the entire k-axis sum fits within one
//!    exact-integer chunk: the largest possible partial sum is
//!    `k · (p-1)² ≤ 4096 · 65520² ≈ 2^44`, well inside the f64
//!    exact-integer range `[0, 2^53]`. We therefore use **one
//!    k-chunk** for any `k ≤ K_CHUNK_CAP = 4096`; longer rows
//!    trigger a vectorised f64 Barrett reduction between chunks.
//!
//! 3. **Reduction.** At the very end of the panel's k sweep the 12
//!    f64 accumulators carry an exact integer in `[0, 2^53]`. We
//!    apply a vectorised f64 Barrett reduction
//!    (`r = x - p · round(x · (1/p))`, then a single conditional
//!    fix-up) to bring each lane into `[0, p)`. Finally we write the
//!    reduced lanes back to the caller's `&mut [u16]` output as
//!    canonical u16 cells.
//!
//! 4. **Unpack pass.** Output canonical u16 cells are written directly
//!    to the caller's `&mut [u16]` storage; the caller is responsible
//!    for converting back to `Fp<P>` via `Fp::new`.
//!
//! # Throughput envelope
//!
//! Per Zen-3 micro-architecture: two FMA ports each retiring 4 f64
//! lanes per cycle = 16 ops/cycle in the bench's `2 m k n` op-count
//! metric. At a 4.4 GHz boost on the 5900X reference host the peak is
//! **70.4 Gop/s**, exactly matching the fflas-ffpack `Modular<double>`
//! peak (69.72 Gop/s observed at GF(65521)/n=4096). With the pre-pack-
//! once + pure-f64-inner-loop structure (no cvt instructions competing
//! with the FMA back-end), the inner kernel approaches the OpenBLAS
//! dgemm throughput on this exact host.
//!
//! # Soundness for `p ∈ (251, 65536)`
//!
//! Inputs `a, b` are canonical in `[0, p)` with `p ≤ 65535`, so each
//! lane product is `(p-1)² ≤ 65534² < 2³²` — exactly representable in
//! f64. The running sum across `k ≤ 2^21` MACs stays ≤ `2^21 · 2^32 =
//! 2^53`, still exactly representable. The k-loop therefore commits no
//! rounding error in the inner loop.
//!
//! The Barrett reduction `r = x - p · round(x · (1/p))` uses one f64
//! multiply (`x * p_inv_f64`, with `p_inv_f64 = (1/p) f64`) plus one
//! `_mm256_round_pd<TO_NEAREST>` plus one f64 FMA (`x - q · p`). The
//! result is in `(-p, p)`; one conditional add of `p` brings it into
//! `[0, p)`. We never need a second iteration because `p_inv_f64` has
//! 53 bits of precision and `x ≤ 2^53`, so `round(x · p_inv_f64) - x/p`
//! is bounded by `1 + 2 · ε` (where ε is f64 machine epsilon ≈ 2⁻⁵²),
//! giving `|r - x mod p| ≤ p · (1 + 2 · ε)`; a single fix-up suffices.
//!
//! # Safety
//!
//! All public functions here are `unsafe` — callers must ensure
//! AVX2 + FMA3 are both available at runtime. Safe, dispatched entry
//! points live in `crate::fp_medium_f64` via the `FpMediumF64Fns`
//! table returned by `detect`.

#![allow(clippy::missing_safety_doc)]
#![allow(clippy::too_many_arguments)]

use core::arch::x86_64::*;

/// Inner `m × n` register-tile dimensions. With f64 at 4 lanes per ymm,
/// `M_R = 4` rows × `N_R = 12` columns is 12 acc + 3 b loads + 1 a
/// broadcast = 16/16 register file. The Zen 3 FMA back-end retires
/// 2 FMAs per cycle so 12 FMAs per inner step → 6 cycles/step steady
/// state, 8 MACs/cycle = 70.4 Gop/s at 4.4 GHz boost.
const M_R: usize = 4;
const N_R: usize = 12;

/// k-axis chunk cap. With `(p-1)² ≤ 2^32` and `k · (p-1)² ≤ 2^53`
/// (f64 exact-integer ceiling), we can absorb up to `2^53 / 2^32 =
/// 2^21 ≈ 2 097 152` MACs per chunk without leaving the exact-integer
/// range. Capping at 4096 keeps each B-panel slice `k · N_R · 8 byte ≤
/// 384 KB` comfortably within Zen 3's 512 KB L2 cache, and at this size
/// the entire k-axis of every reference bench cell (largest k=4096)
/// fits in one chunk — so a single end-of-k Barrett reduction handles
/// the reduction work.
const K_CHUNK_CAP: usize = 4096;

/// Outer-N panel grouping for the cache-blocked loop nest. Picks an
/// `n_c_panels` value that keeps the active B-panel slice resident in
/// the CCX-shared L3 (Zen 3: 32 MB per CCX) while sharing one A-pack
/// across every panel in the group.
///
/// Same calibration as Route A's `n_c_panels_outer` in
/// `crates/gf2-kernels-simd/src/x86/fp_small_f32.rs` and the u16 medium
/// kernel's `fp_medium_nc_panels_outer` in
/// `crates/gf2-kernels-simd/src/x86/fp_medium.rs`: an L3 budget of
/// 16 MB (half the 32 MB CCX-shared L3) bounds the active B slab while
/// leaving headroom for the A-pack scratch + criterion harness state.
#[inline]
fn n_c_panels_outer(n_panels: usize, k: usize) -> usize {
    // 16 MB — half of Zen 3's 32 MB CCX-shared L3, mirroring Route A.
    const L3_BUDGET_BYTES: usize = 16 * 1024 * 1024;
    let panel_bytes = k.saturating_mul(N_R).saturating_mul(8);
    if panel_bytes == 0 {
        return n_panels.max(1);
    }
    let blocked = L3_BUDGET_BYTES / panel_bytes;
    blocked.max(1).min(n_panels.max(1))
}

/// Whole-gemm AVX2 + FMA3 f64-cascade kernel for medium primes.
///
/// Computes `c[i*n + j] = (∑_t a[i*k + t] · bt[j*k + t]) mod p` for
/// every `(i, j) ∈ [0, m) × [0, n)`. Inputs `a` and `bt` carry
/// canonical residues (value `< p`) one per `f64` lane; `bt` is the
/// row-major transpose of the right operand (length `n * k`, so row
/// `j` holds column `j` of B).
///
/// # Safety
///
/// Caller must ensure AVX2 and FMA3 are both available at runtime,
/// `p ∈ (251, 65535]` is an odd prime, and every input lane holds a
/// non-negative integer canonical residue in `[0, p)`.
///
/// # Panics
///
/// Panics if any slice length disagrees with `m`, `k`, `n`.
#[target_feature(enable = "avx2,fma")]
pub unsafe fn fp_medium_f64_gemm(
    a: &[f64],
    bt: &[f64],
    m: usize,
    k: usize,
    n: usize,
    p: u16,
    c: &mut [u16],
) {
    assert_eq!(a.len(), m * k, "fp_medium_f64_gemm: a shape");
    assert_eq!(bt.len(), n * k, "fp_medium_f64_gemm: bt shape");
    assert_eq!(c.len(), m * n, "fp_medium_f64_gemm: c shape");

    if m == 0 || k == 0 || n == 0 {
        // Output is the m×n zero matrix; nothing to do (caller passes
        // a zero-initialised buffer per the gemm contract).
        return;
    }

    // ── Pack B-transpose into N-major f64 panels of width N_R ──────
    //
    // For each n-panel `j_blk = 0, N_R, 2*N_R, ...` we need
    // `b_packed[panel_offset + t*N_R + j_off] = B[t, j_blk + j_off]
    //                                         = bt[(j_blk + j_off)*k + t]`.
    // Slack lanes in a partial trailing panel are filled with 0.0 so
    // the FMA accumulates zero (semantically harmless; we skip the
    // slack lanes at unpack time).
    let n_panels = n.div_ceil(N_R);
    let panel_stride = k * N_R;
    let mut b_packed: Vec<f64> = vec![0.0f64; n_panels * panel_stride];
    for panel_idx in 0..n_panels {
        let j_blk = panel_idx * N_R;
        let j_end = (j_blk + N_R).min(n);
        let n_eff = j_end - j_blk;
        let panel_off = panel_idx * panel_stride;
        for t in 0..k {
            let dst_row_off = panel_off + t * N_R;
            for j_off in 0..n_eff {
                b_packed[dst_row_off + j_off] = bt[(j_blk + j_off) * k + t];
            }
        }
    }

    // ── A-pack scratch (one M_R-row block) ────────────────────────
    //
    // `a_pack_f64[t * M_R + i] = a[(i_blk + i) * k + t]`. The inner
    // kernel then reads each of M_EFF a-rows as a single
    // `_mm256_broadcast_sd` from a contiguous f64 address.
    let mut a_pack_f64: Vec<f64> = vec![0.0f64; M_R * k];

    // ── Outer-N cache-block size ─────────────────────────────────
    let n_c_panels = n_c_panels_outer(n_panels, k);

    // ── k-chunk ────────────────────────────────────────────────────
    //
    // Determine the chunk size. With p ≤ 65535 we have
    // `k_chunk_max = floor(2^53 / (p-1)²)` which exceeds 2^21 for every
    // medium prime; we therefore cap at K_CHUNK_CAP for L2 working-set
    // hygiene. For k ≤ K_CHUNK_CAP the entire k-axis is one chunk.
    let k_chunk = K_CHUNK_CAP.min(k);

    let p_f64 = p as f64;
    let p_inv_f64 = 1.0_f64 / p_f64;

    // ── Inner GEMM loop (outer-N cache-blocked) ───────────────────
    let m_full = m - (m % M_R);
    let mut n_outer = 0usize;
    while n_outer < n_panels {
        let n_outer_end = (n_outer + n_c_panels).min(n_panels);

        let mut i_blk = 0usize;
        while i_blk < m_full {
            pack_a_block::<4>(a, i_blk, k, &mut a_pack_f64);
            for panel_idx in n_outer..n_outer_end {
                let j_blk = panel_idx * N_R;
                let j_end = (j_blk + N_R).min(n);
                let n_eff = j_end - j_blk;
                let panel_off = panel_idx * panel_stride;
                run_one_panel::<4>(
                    &a_pack_f64,
                    &b_packed,
                    i_blk,
                    k,
                    n,
                    panel_off,
                    j_blk,
                    n_eff,
                    k_chunk,
                    p_f64,
                    p_inv_f64,
                    c,
                );
            }
            i_blk += M_R;
        }
        if i_blk < m {
            let m_eff = m - i_blk;
            macro_rules! run_partial {
                ($me:literal) => {{
                    pack_a_block::<$me>(a, i_blk, k, &mut a_pack_f64);
                    for panel_idx in n_outer..n_outer_end {
                        let j_blk = panel_idx * N_R;
                        let j_end = (j_blk + N_R).min(n);
                        let n_eff = j_end - j_blk;
                        let panel_off = panel_idx * panel_stride;
                        run_one_panel::<$me>(
                            &a_pack_f64,
                            &b_packed,
                            i_blk,
                            k,
                            n,
                            panel_off,
                            j_blk,
                            n_eff,
                            k_chunk,
                            p_f64,
                            p_inv_f64,
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

/// Pre-pack a `M_EFF`-row block of A into `[f64; M_R * k]` in interleaved
/// row-major form `dst[t * M_R + i] = a[(i_blk + i) * k + t]`. The
/// dst slack rows (`i ∈ [M_EFF, M_R)`) hold zeros (already initialised
/// by the caller's zero-fill).
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn pack_a_block<const M_EFF: usize>(a: &[f64], i_blk: usize, k: usize, dst: &mut [f64]) {
    debug_assert!(dst.len() >= M_R * k);
    debug_assert!(M_EFF <= M_R);
    let a_base = a.as_ptr().add(i_blk * k);
    let dst_base = dst.as_mut_ptr();
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
    }
}

/// Compute one `M_EFF × n_eff` output tile against one B-panel.
///
/// Monomorphisation on `M_EFF` deletes the dead FMA branches for
/// `m_eff < 4`, leaving the steady-state `M_EFF = 4` body with exactly
/// 12 FMAs per inner step (no branches inside the hot loop).
///
/// `a_pack_f64` is the pre-packed A-row block: `M_R × k` f64 in
/// interleaved row-major (`a_pack_f64[t * M_R + i] = a[(i_blk + i) * k + t]`).
/// `b_packed` is the N-major B-panel buffer: `n_panels × k × N_R` f64;
/// `panel_off` and `n_eff` select the active panel slice.
#[inline]
#[target_feature(enable = "avx2,fma")]
unsafe fn run_one_panel<const M_EFF: usize>(
    a_pack_f64: &[f64],
    b_packed: &[f64],
    i_blk: usize,
    k: usize,
    n: usize,
    panel_off: usize,
    j_blk: usize,
    n_eff: usize,
    k_chunk: usize,
    p_f64: f64,
    p_inv_f64: f64,
    c: &mut [u16],
) {
    // 12 f64 SIMD accumulators (4 × 3 — 4 rows × 3 ymm-of-4-lanes per
    // row = 12 cells per row). At chunk end each lane carries the exact
    // integer dot product `Σ a · b` for that cell. With k ≤ K_CHUNK_CAP
    // = 4096 and (p-1)² ≤ 2^32, the lane value is ≤ 2^44, exactly
    // representable in f64.
    let mut acc00 = _mm256_setzero_pd();
    let mut acc01 = _mm256_setzero_pd();
    let mut acc02 = _mm256_setzero_pd();
    let mut acc10 = _mm256_setzero_pd();
    let mut acc11 = _mm256_setzero_pd();
    let mut acc12 = _mm256_setzero_pd();
    let mut acc20 = _mm256_setzero_pd();
    let mut acc21 = _mm256_setzero_pd();
    let mut acc22 = _mm256_setzero_pd();
    let mut acc30 = _mm256_setzero_pd();
    let mut acc31 = _mm256_setzero_pd();
    let mut acc32 = _mm256_setzero_pd();

    // For k ≤ K_CHUNK_CAP the entire k-axis is one chunk (no
    // intermediate reductions). For k > K_CHUNK_CAP we apply a
    // vectorised f64 Barrett reduction between chunks and re-add the
    // canonical-range remainder into a fresh accumulator. This keeps
    // every intermediate lane ≤ k_chunk · (p-1)² ≤ 2^53.
    let mut t_blk = 0usize;
    while t_blk < k {
        let t_end = (t_blk + k_chunk).min(k);

        // If we are entering the second or later chunk, the
        // accumulators carry a value from the previous chunk that has
        // already been Barrett-reduced to `[0, p)`. We re-accumulate on
        // top — the new chunk adds at most k_chunk · (p-1)², and the
        // current value is < p, so the post-chunk lane is bounded by
        // p + k_chunk · (p-1)² ≤ 2^53.

        let b_panel_base = b_packed.as_ptr().add(panel_off);
        let a_pack_base = a_pack_f64.as_ptr();

        const PREFETCH_DIST: usize = 4;
        let prefetch_end = t_end.saturating_sub(PREFETCH_DIST);

        for t in t_blk..t_end {
            let b_row_ptr = b_panel_base.add(t * N_R);
            let a_row_ptr = a_pack_base.add(t * M_R);

            // Prefetch the B-panel rows PREFETCH_DIST steps ahead.
            // Each B row is N_R · 8 = 96 bytes = 1.5 cache lines; nudge
            // both lines onto the L1d miss queue.
            if t < prefetch_end {
                let pf_ptr = b_row_ptr.add(PREFETCH_DIST * N_R) as *const i8;
                _mm_prefetch::<{ _MM_HINT_T0 }>(pf_ptr);
                _mm_prefetch::<{ _MM_HINT_T0 }>(pf_ptr.add(64));
            }

            // Three pure 4-lane f64 loads from the B-panel — no cvt
            // instructions, matching the OpenBLAS dgemm micro-kernel
            // structure. The B-panel is already f64; loads are
            // contiguous within a single 96-byte row.
            let b0 = _mm256_loadu_pd(b_row_ptr);
            let b1 = _mm256_loadu_pd(b_row_ptr.add(4));
            let b2 = _mm256_loadu_pd(b_row_ptr.add(8));

            // 12 FMAs per inner step (M_EFF=4 path). Each `a_i` is a
            // 4-lane broadcast of the contiguous a_pack entry; on Zen 3
            // `_mm256_broadcast_sd` issues a single load+broadcast µop
            // (no scalar cvt dep chain).
            if M_EFF >= 1 {
                let a0 = _mm256_broadcast_sd(&*a_row_ptr);
                acc00 = _mm256_fmadd_pd(b0, a0, acc00);
                acc01 = _mm256_fmadd_pd(b1, a0, acc01);
                acc02 = _mm256_fmadd_pd(b2, a0, acc02);
            }
            if M_EFF >= 2 {
                let a1 = _mm256_broadcast_sd(&*a_row_ptr.add(1));
                acc10 = _mm256_fmadd_pd(b0, a1, acc10);
                acc11 = _mm256_fmadd_pd(b1, a1, acc11);
                acc12 = _mm256_fmadd_pd(b2, a1, acc12);
            }
            if M_EFF >= 3 {
                let a2 = _mm256_broadcast_sd(&*a_row_ptr.add(2));
                acc20 = _mm256_fmadd_pd(b0, a2, acc20);
                acc21 = _mm256_fmadd_pd(b1, a2, acc21);
                acc22 = _mm256_fmadd_pd(b2, a2, acc22);
            }
            if M_EFF >= 4 {
                let a3 = _mm256_broadcast_sd(&*a_row_ptr.add(3));
                acc30 = _mm256_fmadd_pd(b0, a3, acc30);
                acc31 = _mm256_fmadd_pd(b1, a3, acc31);
                acc32 = _mm256_fmadd_pd(b2, a3, acc32);
            }
        }

        // If more chunks remain, apply an in-place Barrett reduction
        // to bring each accumulator lane back into `[0, p)` before the
        // next chunk's additions push us past 2^53. For k ≤ K_CHUNK_CAP
        // (the common case) this branch is never taken — the
        // accumulators carry their exact integer dot product straight
        // to the final Barrett-and-pack step below.
        if t_end < k {
            acc00 = barrett_reduce_pd(acc00, p_f64, p_inv_f64);
            acc01 = barrett_reduce_pd(acc01, p_f64, p_inv_f64);
            acc02 = barrett_reduce_pd(acc02, p_f64, p_inv_f64);
            if M_EFF >= 2 {
                acc10 = barrett_reduce_pd(acc10, p_f64, p_inv_f64);
                acc11 = barrett_reduce_pd(acc11, p_f64, p_inv_f64);
                acc12 = barrett_reduce_pd(acc12, p_f64, p_inv_f64);
            }
            if M_EFF >= 3 {
                acc20 = barrett_reduce_pd(acc20, p_f64, p_inv_f64);
                acc21 = barrett_reduce_pd(acc21, p_f64, p_inv_f64);
                acc22 = barrett_reduce_pd(acc22, p_f64, p_inv_f64);
            }
            if M_EFF >= 4 {
                acc30 = barrett_reduce_pd(acc30, p_f64, p_inv_f64);
                acc31 = barrett_reduce_pd(acc31, p_f64, p_inv_f64);
                acc32 = barrett_reduce_pd(acc32, p_f64, p_inv_f64);
            }
        }

        t_blk = t_end;
    }

    // Final Barrett reduction + canonical-u16 pack.
    store_and_reduce_tile::<M_EFF>(
        acc00, acc01, acc02, acc10, acc11, acc12, acc20, acc21, acc22, acc30, acc31, acc32, n_eff,
        p_f64, p_inv_f64, i_blk, j_blk, n, c,
    );
}

/// Vectorised f64 Barrett reduction on a single 4-lane ymm.
///
/// Computes `r = x - p · round(x · (1/p))` followed by a single
/// conditional add of `p`. The output is in `[0, p)` provided the input
/// lane is a non-negative exact integer ≤ 2^53.
///
/// # Soundness
///
/// `x ≤ 2^53` is exactly representable in f64. `1/p` has 53 bits of
/// precision so `x · p_inv_f64` is accurate to within 0.5 ulp. After
/// rounding to nearest integer (via `_mm256_round_pd`) the quotient
/// `q` satisfies `|q - x/p| ≤ 1`. The remainder `r = x - q · p` is
/// therefore in `(-p, p)`; a single `r += p` if `r < 0` brings it into
/// `[0, p)`. We never need a second iteration because the quotient
/// error is bounded by 1, not 2.
#[inline]
#[target_feature(enable = "avx2,fma")]
unsafe fn barrett_reduce_pd(x: __m256d, p_f64: f64, p_inv_f64: f64) -> __m256d {
    let p_vec = _mm256_set1_pd(p_f64);
    let p_inv_vec = _mm256_set1_pd(p_inv_f64);
    let zero_vec = _mm256_setzero_pd();
    // q = round(x · (1/p))  with round-to-nearest-int semantics
    let q_approx = _mm256_mul_pd(x, p_inv_vec);
    let q = _mm256_round_pd::<{ _MM_FROUND_TO_NEAREST_INT | _MM_FROUND_NO_EXC }>(q_approx);
    // r = x - q · p, via one FMA: r = -(q · p) + x.
    let r = _mm256_fnmadd_pd(q, p_vec, x);
    // If r < 0, add p once.
    let neg_mask = _mm256_cmp_pd::<{ _CMP_LT_OQ }>(r, zero_vec);
    let r_plus_p = _mm256_add_pd(r, p_vec);
    _mm256_blendv_pd(r, r_plus_p, neg_mask)
}

/// Store the 12 f64 accumulators of the `4 × 12` tile to scratch,
/// apply f64 Barrett reduction, and write the canonical u16 cells into `c`.
#[inline]
#[target_feature(enable = "avx2,fma")]
#[allow(clippy::too_many_arguments)]
unsafe fn store_and_reduce_tile<const M_EFF: usize>(
    acc00: __m256d,
    acc01: __m256d,
    acc02: __m256d,
    acc10: __m256d,
    acc11: __m256d,
    acc12: __m256d,
    acc20: __m256d,
    acc21: __m256d,
    acc22: __m256d,
    acc30: __m256d,
    acc31: __m256d,
    acc32: __m256d,
    n_eff: usize,
    p_f64: f64,
    p_inv_f64: f64,
    i_blk: usize,
    j_blk: usize,
    n: usize,
    c: &mut [u16],
) {
    // Reduce every live accumulator.
    let r00 = barrett_reduce_pd(acc00, p_f64, p_inv_f64);
    let r01 = barrett_reduce_pd(acc01, p_f64, p_inv_f64);
    let r02 = barrett_reduce_pd(acc02, p_f64, p_inv_f64);
    let r10 = if M_EFF >= 2 {
        barrett_reduce_pd(acc10, p_f64, p_inv_f64)
    } else {
        _mm256_setzero_pd()
    };
    let r11 = if M_EFF >= 2 {
        barrett_reduce_pd(acc11, p_f64, p_inv_f64)
    } else {
        _mm256_setzero_pd()
    };
    let r12 = if M_EFF >= 2 {
        barrett_reduce_pd(acc12, p_f64, p_inv_f64)
    } else {
        _mm256_setzero_pd()
    };
    let r20 = if M_EFF >= 3 {
        barrett_reduce_pd(acc20, p_f64, p_inv_f64)
    } else {
        _mm256_setzero_pd()
    };
    let r21 = if M_EFF >= 3 {
        barrett_reduce_pd(acc21, p_f64, p_inv_f64)
    } else {
        _mm256_setzero_pd()
    };
    let r22 = if M_EFF >= 3 {
        barrett_reduce_pd(acc22, p_f64, p_inv_f64)
    } else {
        _mm256_setzero_pd()
    };
    let r30 = if M_EFF >= 4 {
        barrett_reduce_pd(acc30, p_f64, p_inv_f64)
    } else {
        _mm256_setzero_pd()
    };
    let r31 = if M_EFF >= 4 {
        barrett_reduce_pd(acc31, p_f64, p_inv_f64)
    } else {
        _mm256_setzero_pd()
    };
    let r32 = if M_EFF >= 4 {
        barrett_reduce_pd(acc32, p_f64, p_inv_f64)
    } else {
        _mm256_setzero_pd()
    };

    // Store the 12 reduced f64 lanes per row to a stack scratch, then
    // convert lane-by-lane to canonical u16 cells. Each lane is a
    // non-negative integer in `[0, p)` with `p ≤ 65535`, so the f64 →
    // u16 cast is exact.
    let mut tile = [0.0f64; M_R * N_R];
    _mm256_storeu_pd(tile.as_mut_ptr(), r00);
    _mm256_storeu_pd(tile.as_mut_ptr().add(4), r01);
    _mm256_storeu_pd(tile.as_mut_ptr().add(8), r02);
    if M_EFF >= 2 {
        _mm256_storeu_pd(tile.as_mut_ptr().add(N_R), r10);
        _mm256_storeu_pd(tile.as_mut_ptr().add(N_R + 4), r11);
        _mm256_storeu_pd(tile.as_mut_ptr().add(N_R + 8), r12);
    }
    if M_EFF >= 3 {
        _mm256_storeu_pd(tile.as_mut_ptr().add(2 * N_R), r20);
        _mm256_storeu_pd(tile.as_mut_ptr().add(2 * N_R + 4), r21);
        _mm256_storeu_pd(tile.as_mut_ptr().add(2 * N_R + 8), r22);
    }
    if M_EFF >= 4 {
        _mm256_storeu_pd(tile.as_mut_ptr().add(3 * N_R), r30);
        _mm256_storeu_pd(tile.as_mut_ptr().add(3 * N_R + 4), r31);
        _mm256_storeu_pd(tile.as_mut_ptr().add(3 * N_R + 8), r32);
    }

    for i_off in 0..M_EFF {
        for j_off in 0..n_eff {
            let v = tile[i_off * N_R + j_off];
            // v ∈ [0, p) is a non-negative exact integer; the f64→u16
            // cast is well-defined (Rust spec: saturating cast).
            c[(i_blk + i_off) * n + (j_blk + j_off)] = v as u16;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn run_for_primes(test: impl Fn(u16)) {
        if !std::arch::is_x86_feature_detected!("avx2")
            || !std::arch::is_x86_feature_detected!("fma")
        {
            return;
        }
        // Sample primes spanning the medium range (251 < p ≤ 65535).
        for &p in &[257u16, 521, 1031, 4099, 16381, 32771, 65521] {
            test(p);
        }
    }

    fn scalar_gemm(a: &[u16], bt: &[u16], m: usize, k: usize, n: usize, p: u16) -> Vec<u16> {
        let mut out = vec![0u16; m * n];
        for i in 0..m {
            for j in 0..n {
                let mut acc: u64 = 0;
                for t in 0..k {
                    acc += a[i * k + t] as u64 * bt[j * k + t] as u64;
                }
                out[i * n + j] = (acc % p as u64) as u16;
            }
        }
        out
    }

    fn u16_to_f64(xs: &[u16]) -> Vec<f64> {
        xs.iter().map(|&x| x as f64).collect()
    }

    #[test]
    fn gemm_matches_scalar_small_shapes() {
        run_for_primes(|p| {
            for &(m, k, n) in &[
                (1usize, 1usize, 1usize),
                (1, 1, 8),
                (1, 1, 12),
                (1, 1, 13),
                (4, 1, 12),
                (5, 1, 12),
                (1, 2, 12),
                (4, 64, 12),
                (8, 64, 24),
                (5, 65, 13),
                (4, 67, 17),
            ] {
                let a: Vec<u16> = (0..(m * k) as u32)
                    .map(|i| ((i * 17 + 1) % p as u32) as u16)
                    .collect();
                let bt: Vec<u16> = (0..(n * k) as u32)
                    .map(|i| ((i * 23 + 5) % p as u32) as u16)
                    .collect();
                let a_f = u16_to_f64(&a);
                let bt_f = u16_to_f64(&bt);
                let mut got = vec![0u16; m * n];
                unsafe { fp_medium_f64_gemm(&a_f, &bt_f, m, k, n, p, &mut got) };
                let want = scalar_gemm(&a, &bt, m, k, n, p);
                assert_eq!(got, want, "p={p} m={m} k={k} n={n}");
            }
        });
    }

    #[test]
    fn gemm_matches_scalar_k_chunk_boundary() {
        // k around `K_CHUNK_CAP = 4096` boundary to exercise the
        // multi-chunk Barrett path.
        run_for_primes(|p| {
            for &k in &[
                63usize, 64, 65, 127, 128, 129, 512, 1023, 1024, 1025, 4095, 4096, 4097, 8191, 8192,
            ] {
                let m = 4;
                let n = 12;
                let a: Vec<u16> = (0..(m * k) as u32)
                    .map(|i| ((i * 17 + 1) % p as u32) as u16)
                    .collect();
                let bt: Vec<u16> = (0..(n * k) as u32)
                    .map(|i| ((i * 23 + 5) % p as u32) as u16)
                    .collect();
                let a_f = u16_to_f64(&a);
                let bt_f = u16_to_f64(&bt);
                let mut got = vec![0u16; m * n];
                unsafe { fp_medium_f64_gemm(&a_f, &bt_f, m, k, n, p, &mut got) };
                let want = scalar_gemm(&a, &bt, m, k, n, p);
                assert_eq!(got, want, "p={p} k={k}");
            }
        });
    }

    #[test]
    fn gemm_matches_scalar_n_panel_boundary() {
        // n that is not a multiple of N_R = 12 → trailing partial panel.
        run_for_primes(|p| {
            for &n in &[1usize, 8, 11, 12, 13, 23, 24, 25, 35, 36, 37] {
                let m = 4;
                let k = 32;
                let a: Vec<u16> = (0..(m * k) as u32)
                    .map(|i| ((i * 17 + 1) % p as u32) as u16)
                    .collect();
                let bt: Vec<u16> = (0..(n * k) as u32)
                    .map(|i| ((i * 23 + 5) % p as u32) as u16)
                    .collect();
                let a_f = u16_to_f64(&a);
                let bt_f = u16_to_f64(&bt);
                let mut got = vec![0u16; m * n];
                unsafe { fp_medium_f64_gemm(&a_f, &bt_f, m, k, n, p, &mut got) };
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
                let n = 12;
                let a: Vec<u16> = (0..(m * k) as u32)
                    .map(|i| ((i * 17 + 1) % p as u32) as u16)
                    .collect();
                let bt: Vec<u16> = (0..(n * k) as u32)
                    .map(|i| ((i * 23 + 5) % p as u32) as u16)
                    .collect();
                let a_f = u16_to_f64(&a);
                let bt_f = u16_to_f64(&bt);
                let mut got = vec![0u16; m * n];
                unsafe { fp_medium_f64_gemm(&a_f, &bt_f, m, k, n, p, &mut got) };
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
        let mut out: Vec<u16> = vec![];
        unsafe { fp_medium_f64_gemm(&[], &[], 0, 0, 0, 65521, &mut out) };
        assert!(out.is_empty());
    }

    #[test]
    fn gemm_matches_scalar_boundary_lengths() {
        // SC#3 (issue 0749dbad): proptest-style boundary-length sweep at
        // {0, 1, 15, 16, 17, 63, 64, 65}. Exercises the deterministic
        // panel-boundary cases requested by the success criteria.
        let avail = std::arch::is_x86_feature_detected!("avx2")
            && std::arch::is_x86_feature_detected!("fma");
        if !avail {
            return;
        }
        let primes = [257u16, 65521];
        let lens = [0usize, 1, 15, 16, 17, 63, 64, 65];
        for &p in &primes {
            for &m in &lens {
                for &k in &lens {
                    for &n in &lens {
                        if m == 0 || k == 0 || n == 0 {
                            // zero-dim cases short-circuit on either
                            // the kernel or the scalar oracle; the
                            // kernel returns early without touching `c`.
                            continue;
                        }
                        let a: Vec<u16> = (0..(m * k) as u32)
                            .map(|i| ((i * 17 + 1) % p as u32) as u16)
                            .collect();
                        let bt: Vec<u16> = (0..(n * k) as u32)
                            .map(|i| ((i * 23 + 5) % p as u32) as u16)
                            .collect();
                        let a_f = u16_to_f64(&a);
                        let bt_f = u16_to_f64(&bt);
                        let mut got = vec![0u16; m * n];
                        unsafe { fp_medium_f64_gemm(&a_f, &bt_f, m, k, n, p, &mut got) };
                        let want = scalar_gemm(&a, &bt, m, k, n, p);
                        assert_eq!(got, want, "p={p} m={m} k={k} n={n}");
                    }
                }
            }
        }
    }

    #[test]
    fn barrett_reduce_pd_matches_scalar_mod() {
        let avail = std::arch::is_x86_feature_detected!("avx2")
            && std::arch::is_x86_feature_detected!("fma");
        if !avail {
            return;
        }
        let primes = [257u64, 521, 1031, 4099, 16381, 32771, 65521];
        for &p in &primes {
            let p_f = p as f64;
            let p_inv = 1.0_f64 / p_f;
            // Sample exact-integer values across the f64-exact range.
            let samples: Vec<u64> = (0u64..32)
                .chain((0..16).map(|i| (i + 1) * p - 1))
                .chain([
                    0,
                    p - 1,
                    p,
                    p + 1,
                    p * p,
                    p * p + 7,
                    (1u64 << 32) - 1,
                    (1u64 << 44),
                    (1u64 << 50),
                    (1u64 << 53) - 1, // largest exact integer in f64
                ])
                .collect();
            for chunk in samples.chunks(4) {
                let mut buf = [0.0f64; 4];
                for (i, &v) in chunk.iter().enumerate() {
                    buf[i] = v as f64;
                }
                let v = unsafe { _mm256_loadu_pd(buf.as_ptr()) };
                let r = unsafe { barrett_reduce_pd(v, p_f, p_inv) };
                let mut out = [0.0f64; 4];
                unsafe { _mm256_storeu_pd(out.as_mut_ptr(), r) };
                for (i, &v) in chunk.iter().enumerate() {
                    let expected = (v % p) as f64;
                    assert_eq!(
                        out[i], expected,
                        "barrett_reduce_pd: p={p} v={v} got={} expected={}",
                        out[i], expected
                    );
                }
            }
        }
    }
}
