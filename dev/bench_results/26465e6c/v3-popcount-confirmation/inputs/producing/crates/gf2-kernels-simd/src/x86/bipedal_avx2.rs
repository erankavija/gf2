//! Generic AVX2 batch entry points for the bipedal-like
//! [`crate::bipedal::framework::BatchedBipedalLike`] framework.
//!
//! Each `run_*_batch::<C>` function is `#[target_feature(enable = "avx2")]`
//! and generic over the per-prime [`BipedalLikeConfig`] impl `C`. The F_3
//! instantiation `C = Config3` is the only one wired here; F_5 and F_7
//! ship via dedicated, non-generic AVX2 entry points in
//! [`crate::x86::bipedal_avx2_packed5`] and
//! [`crate::x86::bipedal_avx2_packed7`] because their R1 Candidate D
//! (3-plane) and R2 Candidate A (LUT) encodings do not fit the 2-stream
//! `(MagLane, SgnLane)` framework shape (see JIT issue `1f769232`'s
//! `## Amendment 2026-05-14`). R4 §4.1 documents the 12-34x regression
//! that occurs without the `#[target_feature]` discipline.
//!
//! All `pub unsafe fn` here carry a top-of-function `// SAFETY:` comment.
//! AVX2 availability is the dynamic precondition every caller must
//! runtime-detect via `is_x86_feature_detected!("avx2")` before invoking.
//!
//! These are the only files in the bipedal stack that actually emit AVX2
//! instructions; everything in `crate::bipedal::*` is plumbing that
//! inlines into them.
//!
//! [`run_permanent4`] is the F_3 consumer that keeps four matrices in those
//! lanes for an entire Ryser/Gray walk. Its packed-column ABI is concrete
//! rather than generic because only [`crate::bipedal::Bipedal3x4`] currently
//! has a single-word permanent representation.

#[cfg(target_arch = "x86")]
use core::arch::x86::_mm256_srli_epi64;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::_mm256_srli_epi64;

use crate::bipedal::framework::{BatchedBipedalLike, BipedalLikeConfig};
use crate::bipedal::lanes::{Avx2Lane, BipedalLogicalLanes};
use crate::bipedal::Bipedal3x4;

// The `where C: BipedalLikeConfig<MagLane = Avx2Lane, SgnLane = Avx2Lane>`
// bound on each entry point spells out the lane-shape contract: both lane
// types of the per-prime config must resolve to `Avx2Lane`. F_3's
// `Config3` satisfies this; F_5 / F_7 do not (their encodings need a
// different shape — see this module's top-of-file note) and ship through
// `bipedal_avx2_packed5` / `bipedal_avx2_packed7` instead.

/// Apply a bipedal-like add over canonical `(mag, sgn)` u64-word streams via AVX2.
///
/// Generic over the per-prime [`BipedalLikeConfig`] `C`. One AVX2 lane
/// consumes 4 × `u64` (256 bits = 256 logical lanes). All six slices
/// must be the same length and a multiple of 4. An empty input
/// (length 0) is allowed and is a no-op.
///
/// # Type parameters
///
/// * `C` — per-prime arithmetic recipe. Today only [`crate::bipedal::Config3`]
///   is instantiated through this generic entry point; F_5 / F_7 use
///   dedicated entry points in [`crate::x86::bipedal_avx2_packed5`] /
///   [`crate::x86::bipedal_avx2_packed7`].
///
/// # Arguments
///
/// * `mag1`, `sgn1` — first operand `(mag, sgn)` streams.
/// * `mag2`, `sgn2` — second operand `(mag, sgn)` streams.
/// * `out_mag`, `out_sgn` — output buffers.
///
/// # Safety
///
/// AVX2 must be available at runtime (verify via
/// `is_x86_feature_detected!("avx2")`). All six slices share length
/// divisible by 4; behaviour is undefined otherwise.
///
/// # Examples
///
/// The generic entry point itself is crate-internal (the parent `x86` module
/// is private to the crate); F_3 callers reach it through the
/// `Config3`-monomorphised public re-export at
/// [`crate::bipedal::avx2::run_add_batch`]:
///
/// ```no_run
/// use gf2_kernels_simd::bipedal::avx2::run_add_batch;
/// if is_x86_feature_detected!("avx2") {
///     let v = vec![0u64; 4];
///     let mut out_m = vec![0u64; 4];
///     let mut out_s = vec![0u64; 4];
///     // SAFETY: AVX2 verified, slices length 4 (= one AVX2 lane).
///     unsafe { run_add_batch(&v, &v, &v, &v, &mut out_m, &mut out_s); }
/// }
/// ```
///
/// # Complexity
///
/// `O(n / 4)` AVX2 ops, where `n = mag1.len()`.
#[inline]
#[target_feature(enable = "avx2")]
pub unsafe fn run_add_batch<C>(
    mag1: &[u64],
    sgn1: &[u64],
    mag2: &[u64],
    sgn2: &[u64],
    out_mag: &mut [u64],
    out_sgn: &mut [u64],
) where
    C: BipedalLikeConfig<MagLane = Avx2Lane, SgnLane = Avx2Lane>,
{
    debug_assert_eq!(mag1.len() % 4, 0);
    debug_assert_eq!(mag1.len(), sgn1.len());
    debug_assert_eq!(mag1.len(), mag2.len());
    debug_assert_eq!(mag1.len(), sgn2.len());
    debug_assert_eq!(mag1.len(), out_mag.len());
    debug_assert_eq!(mag1.len(), out_sgn.len());
    let n = mag1.len();
    // SAFETY: AVX2 + bounds + multiple-of-4 are caller's preconditions.
    unsafe {
        let mut i = 0usize;
        while i < n {
            let v_m1 = Avx2Lane::loadu(mag1, i);
            let v_s1 = Avx2Lane::loadu(sgn1, i);
            let v_m2 = Avx2Lane::loadu(mag2, i);
            let v_s2 = Avx2Lane::loadu(sgn2, i);
            let (m, s) = BatchedBipedalLike::<C>::add(v_m1, v_s1, v_m2, v_s2);
            Avx2Lane::storeu(out_mag, i, m);
            Avx2Lane::storeu(out_sgn, i, s);
            i += 4;
        }
    }
}

/// Apply a bipedal-like sub over canonical `(mag, sgn)` u64-word streams via AVX2.
///
/// Generic over [`BipedalLikeConfig`] `C`. See [`run_add_batch`] for the
/// slice-shape contract.
///
/// # Type parameters
///
/// * `C` — per-prime arithmetic recipe.
///
/// # Arguments
///
/// Same shape as [`run_add_batch`]; `(mag1, sgn1) - (mag2, sgn2)`.
///
/// # Safety
///
/// AVX2 must be available at runtime. All six slices share length
/// divisible by 4.
///
/// # Examples
///
/// Reach the AVX2 sub batch via the `Config3`-monomorphised public
/// re-export at [`crate::bipedal::avx2::run_sub_batch`]:
///
/// ```no_run
/// use gf2_kernels_simd::bipedal::avx2::run_sub_batch;
/// if is_x86_feature_detected!("avx2") {
///     let v = vec![0u64; 4];
///     let mut out_m = vec![0u64; 4];
///     let mut out_s = vec![0u64; 4];
///     // SAFETY: AVX2 verified, slices length 4.
///     unsafe { run_sub_batch(&v, &v, &v, &v, &mut out_m, &mut out_s); }
/// }
/// ```
///
/// # Complexity
///
/// `O(n / 4)` AVX2 ops.
#[inline]
#[target_feature(enable = "avx2")]
pub unsafe fn run_sub_batch<C>(
    mag1: &[u64],
    sgn1: &[u64],
    mag2: &[u64],
    sgn2: &[u64],
    out_mag: &mut [u64],
    out_sgn: &mut [u64],
) where
    C: BipedalLikeConfig<MagLane = Avx2Lane, SgnLane = Avx2Lane>,
{
    debug_assert_eq!(mag1.len() % 4, 0);
    debug_assert_eq!(mag1.len(), sgn1.len());
    debug_assert_eq!(mag1.len(), mag2.len());
    debug_assert_eq!(mag1.len(), sgn2.len());
    debug_assert_eq!(mag1.len(), out_mag.len());
    debug_assert_eq!(mag1.len(), out_sgn.len());
    let n = mag1.len();
    // SAFETY: AVX2 + bounds + multiple-of-4 are caller's preconditions.
    unsafe {
        let mut i = 0usize;
        while i < n {
            let v_m1 = Avx2Lane::loadu(mag1, i);
            let v_s1 = Avx2Lane::loadu(sgn1, i);
            let v_m2 = Avx2Lane::loadu(mag2, i);
            let v_s2 = Avx2Lane::loadu(sgn2, i);
            let (m, s) = BatchedBipedalLike::<C>::sub(v_m1, v_s1, v_m2, v_s2);
            Avx2Lane::storeu(out_mag, i, m);
            Avx2Lane::storeu(out_sgn, i, s);
            i += 4;
        }
    }
}

/// Apply a bipedal-like mul over canonical `(mag, sgn)` u64-word streams via AVX2.
///
/// Generic over [`BipedalLikeConfig`] `C`. See [`run_add_batch`] for the
/// slice-shape contract.
///
/// # Type parameters
///
/// * `C` — per-prime arithmetic recipe.
///
/// # Arguments
///
/// Same shape as [`run_add_batch`]; `(mag1, sgn1) * (mag2, sgn2)`.
///
/// # Safety
///
/// AVX2 must be available at runtime. All six slices share length
/// divisible by 4.
///
/// # Examples
///
/// Reach the AVX2 mul batch via the `Config3`-monomorphised public
/// re-export at [`crate::bipedal::avx2::run_mul_batch`]:
///
/// ```no_run
/// use gf2_kernels_simd::bipedal::avx2::run_mul_batch;
/// if is_x86_feature_detected!("avx2") {
///     let v = vec![0u64; 4];
///     let mut out_m = vec![0u64; 4];
///     let mut out_s = vec![0u64; 4];
///     // SAFETY: AVX2 verified, slices length 4.
///     unsafe { run_mul_batch(&v, &v, &v, &v, &mut out_m, &mut out_s); }
/// }
/// ```
///
/// # Complexity
///
/// `O(n / 4)` AVX2 ops.
#[inline]
#[target_feature(enable = "avx2")]
pub unsafe fn run_mul_batch<C>(
    mag1: &[u64],
    sgn1: &[u64],
    mag2: &[u64],
    sgn2: &[u64],
    out_mag: &mut [u64],
    out_sgn: &mut [u64],
) where
    C: BipedalLikeConfig<MagLane = Avx2Lane, SgnLane = Avx2Lane>,
{
    debug_assert_eq!(mag1.len() % 4, 0);
    debug_assert_eq!(mag1.len(), sgn1.len());
    debug_assert_eq!(mag1.len(), mag2.len());
    debug_assert_eq!(mag1.len(), sgn2.len());
    debug_assert_eq!(mag1.len(), out_mag.len());
    debug_assert_eq!(mag1.len(), out_sgn.len());
    let n = mag1.len();
    // SAFETY: AVX2 + bounds + multiple-of-4 are caller's preconditions.
    unsafe {
        let mut i = 0usize;
        while i < n {
            let v_m1 = Avx2Lane::loadu(mag1, i);
            let v_s1 = Avx2Lane::loadu(sgn1, i);
            let v_m2 = Avx2Lane::loadu(mag2, i);
            let v_s2 = Avx2Lane::loadu(sgn2, i);
            let (m, s) = BatchedBipedalLike::<C>::mul(v_m1, v_s1, v_m2, v_s2);
            Avx2Lane::storeu(out_mag, i, m);
            Avx2Lane::storeu(out_sgn, i, s);
            i += 4;
        }
    }
}

/// Apply a bipedal-like neg over canonical `(mag, sgn)` u64-word streams via AVX2.
///
/// Generic over [`BipedalLikeConfig`] `C`. Two input slices and two
/// output slices, all the same length and a multiple of 4.
///
/// # Type parameters
///
/// * `C` — per-prime arithmetic recipe.
///
/// # Arguments
///
/// * `mag`, `sgn` — input `(mag, sgn)` streams.
/// * `out_mag`, `out_sgn` — output buffers.
///
/// # Safety
///
/// AVX2 must be available at runtime. All four slices share length
/// divisible by 4.
///
/// # Examples
///
/// Reach the AVX2 neg batch via the `Config3`-monomorphised public
/// re-export at [`crate::bipedal::avx2::run_neg_batch`]:
///
/// ```no_run
/// use gf2_kernels_simd::bipedal::avx2::run_neg_batch;
/// if is_x86_feature_detected!("avx2") {
///     let v = vec![0u64; 4];
///     let mut out_m = vec![0u64; 4];
///     let mut out_s = vec![0u64; 4];
///     // SAFETY: AVX2 verified, slices length 4.
///     unsafe { run_neg_batch(&v, &v, &mut out_m, &mut out_s); }
/// }
/// ```
///
/// # Complexity
///
/// `O(n / 4)` AVX2 ops.
#[inline]
#[target_feature(enable = "avx2")]
pub unsafe fn run_neg_batch<C>(mag: &[u64], sgn: &[u64], out_mag: &mut [u64], out_sgn: &mut [u64])
where
    C: BipedalLikeConfig<MagLane = Avx2Lane, SgnLane = Avx2Lane>,
{
    debug_assert_eq!(mag.len() % 4, 0);
    debug_assert_eq!(mag.len(), sgn.len());
    debug_assert_eq!(mag.len(), out_mag.len());
    debug_assert_eq!(mag.len(), out_sgn.len());
    let n = mag.len();
    // SAFETY: AVX2 + bounds + multiple-of-4 are caller's preconditions.
    unsafe {
        let mut i = 0usize;
        while i < n {
            let v_m = Avx2Lane::loadu(mag, i);
            let v_s = Avx2Lane::loadu(sgn, i);
            let (m, s) = BatchedBipedalLike::<C>::neg(v_m, v_s);
            Avx2Lane::storeu(out_mag, i, m);
            Avx2Lane::storeu(out_sgn, i, s);
            i += 4;
        }
    }
}

#[inline(always)]
unsafe fn fold_step<const SHIFT: i32>(mag: Avx2Lane, sgn: Avx2Lane) -> (Avx2Lane, Avx2Lane) {
    // SAFETY: the enclosing permanent kernel establishes AVX2. The const
    // shifts are all in 1..64, and Bipedal3x4's multiplication has the same
    // target-feature precondition.
    unsafe {
        let shifted_mag = Avx2Lane(_mm256_srli_epi64::<SHIFT>(mag.0));
        let shifted_sgn = Avx2Lane(_mm256_srli_epi64::<SHIFT>(sgn.0));
        Bipedal3x4::mul(mag, sgn, shifted_mag, shifted_sgn)
    }
}

/// Evaluate four packed single-word F_3 permanents in one AVX2 Gray walk.
///
/// `columns_mag[j][lane]` and `columns_sgn[j][lane]` encode column `j` of
/// matrix `lane` as one canonical bipedal word. Each AVX2 64-bit lane is
/// therefore one independent matrix; the bit positions inside that lane are
/// its rows. The two column slices have length `n`, where `n <= 63`.
///
/// The Gray walk, bipedal column-sum updates, six-step product fold, and Ryser
/// accumulation all remain YMM-resident through [`Bipedal3x4`]. The returned
/// array contains canonical residues in `0..3`, in lane order. Callers pad a
/// partial batch with zero column words and ignore its unused output lanes.
///
/// # Safety
///
/// AVX2 must be available at runtime. Callers must establish this with
/// `is_x86_feature_detected!("avx2")` or invoke the safe function pointer
/// returned by [`crate::bipedal::detect_avx2`]. Slice length and `n <= 63`
/// are checked before any load, so invalid shapes panic rather than causing an
/// out-of-bounds SIMD access.
///
/// # Panics
///
/// Panics if the magnitude and sign slices differ in length or contain more
/// than 63 columns.
///
/// # Complexity
///
/// `O(n * 2^n)` AVX2 operations for all four matrices together and `O(1)`
/// auxiliary storage beyond the packed columns supplied by the caller.
#[target_feature(enable = "avx2")]
pub unsafe fn run_permanent4(columns_mag: &[[u64; 4]], columns_sgn: &[[u64; 4]]) -> [u64; 4] {
    assert_eq!(
        columns_mag.len(),
        columns_sgn.len(),
        "run_permanent4: magnitude/sign column counts must match"
    );
    let n = columns_mag.len();
    assert!(n <= 63, "run_permanent4: n must be <= 63; got {n}");
    if n == 0 {
        return [1; 4];
    }

    let zeros = [0u64; 4];
    // SAFETY: all arrays loaded below contain exactly four u64 words and this
    // function's target_feature attribute establishes AVX2.
    unsafe {
        let zero = Avx2Lane::loadu(&zeros, 0);
        let mut col_sum_mag = zero;
        let mut col_sum_sgn = zero;
        let mut total_mag = zero;
        let mut total_sgn = zero;
        let mut subset_size = 0usize;
        let upper = 1u64 << n;

        for k in 1..upper {
            let flip = k.trailing_zeros() as usize;
            let gray = k ^ (k >> 1);
            let column_mag = Avx2Lane::loadu(&columns_mag[flip], 0);
            let column_sgn = Avx2Lane::loadu(&columns_sgn[flip], 0);
            if ((gray >> flip) & 1) == 1 {
                subset_size += 1;
                (col_sum_mag, col_sum_sgn) =
                    Bipedal3x4::add(col_sum_mag, col_sum_sgn, column_mag, column_sgn);
            } else {
                subset_size -= 1;
                (col_sum_mag, col_sum_sgn) =
                    Bipedal3x4::sub(col_sum_mag, col_sum_sgn, column_mag, column_sgn);
            }

            let used = (1u64 << n) - 1;
            let used_words = [used; 4];
            let unused_words = [!used; 4];
            let used_lane = Avx2Lane::loadu(&used_words, 0);
            let unused_lane = Avx2Lane::loadu(&unused_words, 0);
            let mut term_mag = Avx2Lane::or(col_sum_mag, unused_lane);
            let mut term_sgn = Avx2Lane::and(col_sum_sgn, used_lane);
            (term_mag, term_sgn) = fold_step::<32>(term_mag, term_sgn);
            (term_mag, term_sgn) = fold_step::<16>(term_mag, term_sgn);
            (term_mag, term_sgn) = fold_step::<8>(term_mag, term_sgn);
            (term_mag, term_sgn) = fold_step::<4>(term_mag, term_sgn);
            (term_mag, term_sgn) = fold_step::<2>(term_mag, term_sgn);
            (term_mag, term_sgn) = fold_step::<1>(term_mag, term_sgn);

            if subset_size % 2 == 1 {
                (total_mag, total_sgn) = Bipedal3x4::sub(total_mag, total_sgn, term_mag, term_sgn);
            } else {
                (total_mag, total_sgn) = Bipedal3x4::add(total_mag, total_sgn, term_mag, term_sgn);
            }
        }

        if n % 2 == 1 {
            (total_mag, total_sgn) = Bipedal3x4::neg(total_mag, total_sgn);
        }

        let mut result_mag = [0u64; 4];
        let mut result_sgn = [0u64; 4];
        Avx2Lane::storeu(&mut result_mag, 0, total_mag);
        Avx2Lane::storeu(&mut result_sgn, 0, total_sgn);
        core::array::from_fn(|lane| {
            if result_mag[lane] & 1 == 0 {
                0
            } else {
                1 + (result_sgn[lane] & 1)
            }
        })
    }
}
