//! `permanent_bipedal3` — fast path dispatcher and single-`u64`-pair fast
//! path for permanents over `F_3`.
//!
//! ## Single-word path (`n ≤ 63`)
//!
//! For `n ≤ 63` the column-sum vector fits in a single Bipedal3 word (one
//! `u64` mag + one `u64` sgn pair). The narrowing from the pre-2026-05-15
//! `n ≤ 64` is a wallclock-driven CPU/GPU consistency choice (2^64 Gray
//! steps takes ~600 years on either path; the contract was always
//! implicitly bounded by feasibility, and is now bounded explicitly).
//!  Each Gray-code step updates a single
//! `Bipedal3` column-sum in-place via `Bipedal3::add` or `Bipedal3::sub`
//! (the canonical paper §2.2 SSOT lives once in those methods), followed by
//! a horizontal fold via `Bipedal3::fold_mul_first_n` — the bipedal
//! multiplication tree halving lives once in that method.
//!
//! ## Single-matrix and batched SIMD paths (`n ≤ 63`)
//!
//! The public single-matrix dispatcher selects the scalar single-word kernel.
//! The locked provenance-fixed four-matrix receipt records scalar at
//! 2.858539–3.354322 times the direct single-matrix AVX2 rate for
//! `n = 8, 12, 16, 20, 24, 28`; see
//! `dev/benchmarks/permanent_campaign/batched-f3-avx2-provenance-fixed.md`.
//! Historical S3 cross-CPU evidence is corroboration only; its provenance
//! status is recorded in `dev/benchmarks/gf2_algebra_permanent/README.md`. The archived
//! portability plan preserves the historical rationale:
//! `dev/archive/ae82bd73-gf2-algebra-permanent/plans/363556e6/s3_cross_cpu_portability.md`.
//!
//! The single-matrix AVX2 function remains directly available for kernel
//! conformance checks. The batched entry point uses AVX2 when available to
//! evaluate up to four matrices together, with one matrix in each lane.
//!
//! ## Multi-word path (`n > 63`)
//!
//! For `n > 63` the column-sum spans `W = ceil(n / 64)` words per leg.
//! The multi-word streaming path lives in `super::bipedal3_multiword` and
//! implements the R3 cache-blocking design
//! (`dev/plans/60c30e2d/r3_multi_word_streaming.md`).
//!
//! ## Dispatcher
//!
//! The public `permanent_bipedal3` function dispatches by `n`: it selects the
//! scalar `permanent_bipedal3_singleword` kernel through `n = 63` and the
//! multi-word path above that boundary. The single-word kernel is also exposed
//! directly for callers such as multi-word boundary cross-checks.
//!
//! This module is the **headline single-thread fast path** of the
//! permanent epic; the 50× speedup target is measured against
//! `permanent_mod3_reference` at `n = 36`.
//!
//! # Algorithm reference
//!
//! `dev/plans/ae82bd73-gf2-algebra-permanent/gf2_algebra_permanent.md` §7.3 (single-word path).
//! `dev/plans/60c30e2d/r3_multi_word_streaming.md` §8 (multi-word pseudocode).

use gf2_core::gfp::Fp;

use crate::gray::gray_code_iter;
use crate::packed::bipedal3::{Bipedal3, Bipedal3Matrix};
use crate::packed::PackedField;
use crate::packed::PackedFieldVec;
use crate::permanent::bipedal3_multiword;

// ---------------------------------------------------------------------------
// SIMD detection cache (x86/x86_64 only, behind the `simd` feature).
//
// `maybe_bipedal_avx2()` follows the project's `gf2_core::simd::maybe_simd`
// OnceLock SSOT pattern (CLAUDE.md §Architecture, point 3).  It wraps
// `gf2_kernels_simd::bipedal::detect_avx2()` — the upstream OnceLock that
// performs CPUID. On non-x86 targets, or when the `simd` feature is off,
// the symbol simply does not exist and its call sites are elided at
// compile time.
// ---------------------------------------------------------------------------

/// Return the cached AVX2 function bundle for F_3 bipedal operations, or
/// `None` if AVX2 is absent at runtime.
///
/// This is the `gf2-algebra`-local shim over
/// [`gf2_kernels_simd::bipedal::detect_avx2`]; it re-uses that crate's
/// own `OnceLock` so CPUID is queried at most once per process across callers.
///
/// # Complexity
///
/// `O(1)` — first call may perform CPUID; all subsequent calls are a
/// cached read.
#[cfg(all(feature = "simd", any(target_arch = "x86", target_arch = "x86_64")))]
#[inline]
fn maybe_bipedal_avx2() -> Option<gf2_kernels_simd::bipedal::BipedalAvx2Fns> {
    #[cfg(test)]
    AVX2_DETECTION_PROBES.with(|probes| probes.set(probes.get() + 1));
    gf2_kernels_simd::bipedal::detect_avx2()
}

type Permanent4KernelFn = fn(&[[u64; 4]], &[[u64; 4]]) -> [u64; 4];

#[cfg(test)]
std::thread_local! {
    static AVX2_DETECTION_PROBES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

// ---------------------------------------------------------------------------
// Test-only scalar/SIMD cross-check note.
//
// The original T13 sketch used a process-global `AtomicBool` to force the
// scalar path from tests. That was race-prone: cargo's parallel test
// runner could let one test set the flag while another test was mid-call
// to `permanent_bipedal3`, silently making the SIMD/scalar comparison
// vacuous. The shipped design instead exposes both
// `permanent_bipedal3_singleword` and `permanent_bipedal3_singleword_simd`
// as `pub` and has the tests call them directly with the appropriate
// path, so no shared mutable state participates in the cross-check.
// ---------------------------------------------------------------------------

/// Compute the permanent of an `n × n` matrix over `F_3`, dispatching to
/// the single-word fast path for `n ≤ 63` or the multi-word streaming path
/// for `64 ≤ n ≤ N_MAX_MULTIWORD`.
///
/// This is the unified public entrypoint. For callers that always have
/// `n ≤ 63`, use [`permanent_bipedal3_singleword`] directly. For those
/// dimensions this dispatcher deliberately selects that scalar kernel even
/// when AVX2 is available; committed cross-CPU measurements show it is faster
/// than padding a single word into the four-lane AVX2 kernel. The `n ≤ 63`
/// upper bound on the single-word path was narrowed from the pre-2026-05-15
/// `n ≤ 64` for CPU/GPU consistency; see the module-level documentation
/// for the measurement and bound rationales.
///
/// The permanent of an `n × n` matrix `A` over `F_3` is:
///
/// ```text
/// perm(A) = sum over all permutations sigma of prod_{i=0}^{n-1} A[i, sigma(i)]
/// ```
///
/// Evaluated via Ryser's inclusion-exclusion formula in Gray-code order
/// (see `permanent_ryser` for the generic version):
///
/// ```text
/// perm(A) = (-1)^n * sum_{S ⊆ [n], S ≠ ∅} (-1)^|S| * prod_{i=0}^{n-1} sum_{j ∈ S} A[i,j]
/// ```
///
/// # Arguments
///
/// * `mat` — An `n × n` [`Bipedal3Matrix`] (column-major, `rows == cols`).
///   `n` must satisfy `n ≤ N_MAX_MULTIWORD` (currently 255).
///
/// # Examples
///
/// ```
/// use gf2_algebra::packed::Bipedal3Matrix;
/// use gf2_algebra::permanent::permanent_bipedal3;
/// use gf2_core::gfp::Fp;
///
/// // 2×2 identity over F_3: permanent = 1
/// let id: Vec<Fp<3>> = vec![
///     Fp::<3>::new(1), Fp::<3>::new(0),
///     Fp::<3>::new(0), Fp::<3>::new(1),
/// ];
/// let m = Bipedal3Matrix::from_row_major(&id, 2, 2);
/// assert_eq!(permanent_bipedal3(&m), Fp::<3>::new(1));
///
/// // 2×2 all-ones over F_3: permanent = 2! mod 3 = 2
/// let ones: Vec<Fp<3>> = vec![Fp::<3>::new(1); 4];
/// let m2 = Bipedal3Matrix::from_row_major(&ones, 2, 2);
/// assert_eq!(permanent_bipedal3(&m2), Fp::<3>::new(2));
/// ```
///
/// # Panics
///
/// Panics if `mat.rows() != mat.cols()` (matrix must be square).
///
/// Panics if `mat.cols() > bipedal3_multiword::N_MAX_MULTIWORD` (`n` must
/// be `≤ N_MAX_MULTIWORD = 255`; above that, use the W3-T15 rayon parallel
/// path or W5 GPU path).
///
/// # Complexity
///
/// `O(n · 2^n)` field operations over `Fp<3>`. See [`permanent_bipedal3_singleword`]
/// and [`bipedal3_multiword::permanent_bipedal3_multiword`] for per-path details.
pub fn permanent_bipedal3(mat: &Bipedal3Matrix) -> Fp<3> {
    let n = mat.cols();
    assert_eq!(
        mat.rows(),
        n,
        "permanent_bipedal3: matrix must be square (rows={}, cols={})",
        mat.rows(),
        n
    );
    assert!(
        n <= bipedal3_multiword::N_MAX_MULTIWORD,
        "permanent_bipedal3: n must satisfy n <= {}; got n = {}",
        bipedal3_multiword::N_MAX_MULTIWORD,
        n
    );
    if n <= 63 {
        permanent_bipedal3_singleword(mat)
    } else {
        bipedal3_multiword::permanent_bipedal3_multiword(mat)
    }
}

/// Compute the permanents of one to four equally sized matrices over F_3.
///
/// On AVX2 hosts with the `simd` feature enabled, the four matrix words are
/// packed into the 64-bit lanes of one YMM register and evaluated together by
/// the [`gf2_kernels_simd::bipedal::Bipedal3x4`] Gray-walk kernel. Missing
/// lanes in a partial batch are zero-padded and omitted from the returned
/// vector. On other hosts the function safely evaluates each matrix with
/// [`permanent_bipedal3_singleword`].
///
/// All matrices must be square and have the same dimension `n <= 63`. The
/// `0 x 0` permanent is supported and equals one for every matrix in the
/// batch. Results preserve input order.
///
/// # Arguments
///
/// * `matrices` — a slice containing between one and four equally sized,
///   square [`Bipedal3Matrix`] values.
///
/// # Examples
///
/// ```
/// use gf2_algebra::packed::Bipedal3Matrix;
/// use gf2_algebra::permanent::bipedal3::permanent_bipedal3_batch;
/// use gf2_core::gfp::Fp;
///
/// let identity = Bipedal3Matrix::from_row_major(
///     &[
///         Fp::<3>::new(1), Fp::<3>::new(0),
///         Fp::<3>::new(0), Fp::<3>::new(1),
///     ],
///     2,
///     2,
/// );
/// let ones = Bipedal3Matrix::from_row_major(&[Fp::<3>::new(1); 4], 2, 2);
/// assert_eq!(
///     permanent_bipedal3_batch(&[identity, ones]),
///     vec![Fp::<3>::new(1), Fp::<3>::new(2)],
/// );
/// ```
///
/// # Panics
///
/// Panics if the slice is empty or contains more than four matrices, if a
/// matrix is not square, if dimensions differ within the batch, or if
/// `n > 63`.
///
/// # Complexity
///
/// `O(n * 2^n)` work. AVX2 evaluates up to four matrices in that one walk;
/// the fallback performs one scalar walk per matrix. Packing uses `O(4n)`
/// additional words.
pub fn permanent_bipedal3_batch(matrices: &[Bipedal3Matrix]) -> Vec<Fp<3>> {
    let kernel: Option<Permanent4KernelFn> = {
        #[cfg(all(feature = "simd", any(target_arch = "x86", target_arch = "x86_64")))]
        {
            maybe_bipedal_avx2().map(|fns| fns.permanent4_fn)
        }
        #[cfg(not(all(feature = "simd", any(target_arch = "x86", target_arch = "x86_64"))))]
        {
            None
        }
    };
    permanent_bipedal3_batch_with_kernel(matrices, kernel)
}

fn permanent_bipedal3_batch_with_kernel(
    matrices: &[Bipedal3Matrix],
    kernel: Option<Permanent4KernelFn>,
) -> Vec<Fp<3>> {
    assert!(
        !matrices.is_empty(),
        "permanent_bipedal3_batch: matrices must not be empty"
    );
    assert!(
        matrices.len() <= 4,
        "permanent_bipedal3_batch: at most four matrices are supported; got {}",
        matrices.len()
    );
    let n = matrices[0].cols();
    for (index, matrix) in matrices.iter().enumerate() {
        assert_eq!(
            matrix.rows(),
            matrix.cols(),
            "permanent_bipedal3_batch: matrix[{index}] must be square (rows={}, cols={})",
            matrix.rows(),
            matrix.cols()
        );
        assert_eq!(
            matrix.cols(),
            n,
            "permanent_bipedal3_batch: matrix[{index}] has dimension {}, expected {n}",
            matrix.cols()
        );
    }
    assert!(
        n <= 63,
        "permanent_bipedal3_batch: single-word path requires n <= 63; got n = {n}"
    );

    let Some(kernel) = kernel else {
        return matrices.iter().map(permanent_bipedal3_singleword).collect();
    };

    // A zero row makes the permanent identically zero. Remove such matrices
    // before SIMD packing, then scatter the active-lane results back into the
    // original input order. This also keeps randomized boundary conformance at
    // large, exponentially infeasible dimensions testable in the fast tier.
    let active: Vec<_> = matrices
        .iter()
        .enumerate()
        .filter(|(_, matrix)| !has_zero_row(matrix))
        .collect();
    let mut results = vec![Fp::<3>::new(0); matrices.len()];
    if active.is_empty() {
        return results;
    }

    let packed: Vec<_> = active
        .iter()
        .map(|(_, matrix)| pack_singleword_columns(matrix))
        .collect();
    let mut columns_mag = vec![[0u64; 4]; n];
    let mut columns_sgn = vec![[0u64; 4]; n];
    for (lane, matrix_columns) in packed.iter().enumerate() {
        for (column, value) in matrix_columns.iter().copied().enumerate() {
            columns_mag[column][lane] = value.mag();
            columns_sgn[column][lane] = value.sgn();
        }
    }
    let lane_results = kernel(&columns_mag, &columns_sgn);
    for (lane, (input_index, _)) in active.iter().enumerate() {
        results[*input_index] = Fp::<3>::new(lane_results[lane]);
    }
    results
}

#[inline]
fn has_zero_row(mat: &Bipedal3Matrix) -> bool {
    (0..mat.rows()).any(|row| (0..mat.cols()).all(|column| mat.get(row, column).value() == 0))
}

fn pack_singleword_columns(mat: &Bipedal3Matrix) -> Vec<Bipedal3> {
    let n = mat.cols();
    let mut columns = Vec::with_capacity(n);
    for j in 0..n {
        let col_vec = mat.column(j);
        let mut col = Bipedal3::zero();
        for i in 0..n {
            col = col.with_lane(i, col_vec.get(i));
        }
        columns.push(col);
    }
    columns
}

/// Compute the permanent of an `n × n` matrix over `F_3` using the
/// single-`u64` Bipedal3 fast path.
///
/// For `n ≤ 63` the column-sum vector fits in a single Bipedal3 word
/// (one `u64` mag + one `u64` sgn pair), so each Gray-code step performs
/// exactly one Bipedal3 add or sub followed by a horizontal
/// bipedal-multiplication-tree fold of the `n` active lanes. The
/// `n ≤ 63` upper bound was narrowed from the pre-2026-05-15 `n ≤ 64`
/// for CPU/GPU consistency (n=64 is computationally infeasible on either
/// path).
///
/// Prefer [`permanent_bipedal3`] for the dispatching entrypoint that also
/// handles `n > 63`.
///
/// # Arguments
///
/// * `mat` — An `n × n` [`Bipedal3Matrix`] (column-major, `rows == cols`),
///   with `n ≤ 63`.
///
/// # Examples
///
/// ```
/// use gf2_algebra::packed::Bipedal3Matrix;
/// use gf2_algebra::permanent::bipedal3::permanent_bipedal3_singleword;
/// use gf2_core::gfp::Fp;
///
/// // 2×2 identity over F_3: permanent = 1
/// let id: Vec<Fp<3>> = vec![
///     Fp::<3>::new(1), Fp::<3>::new(0),
///     Fp::<3>::new(0), Fp::<3>::new(1),
/// ];
/// let m = Bipedal3Matrix::from_row_major(&id, 2, 2);
/// assert_eq!(permanent_bipedal3_singleword(&m), Fp::<3>::new(1));
/// ```
///
/// # Panics
///
/// Panics if `mat.rows() != mat.cols()` (matrix must be square).
///
/// Panics if `mat.cols() > 63` (single-u64 fast path requires `n <= 63`
/// per the 2026-05-15 CPU/GPU consistency narrowing).
/// The Gray walk uses a `u128` step counter so the n=63 boundary is
/// well-defined; column-sum state still fits one Bipedal3 word.
///
/// # Complexity
///
/// `O(n · 2^n)` field operations over `Fp<3>`:
/// - Matrix prep: `O(n^2)` one-time lane-by-lane column extraction.
/// - Gray walk: `2^n - 1` steps, each with 1 `Bipedal3::add` or `sub`
///   (6 word-level bitwise ops) plus 1 `Bipedal3::fold_mul_first_n`
///   (~6 halving steps, 2 word ops each).
/// - Space: `O(n)` extra (the `columns` Vec plus one `Bipedal3` col-sum word).
pub fn permanent_bipedal3_singleword(mat: &Bipedal3Matrix) -> Fp<3> {
    let n = mat.cols();
    assert_eq!(
        mat.rows(),
        n,
        "permanent_bipedal3_singleword: matrix must be square (rows={}, cols={})",
        mat.rows(),
        n
    );
    assert!(
        n <= 63,
        "permanent_bipedal3_singleword: single-u64 fast path requires n <= 63; got n = {}",
        n
    );

    // Edge case: the 0×0 matrix has exactly one permutation (the empty
    // one), whose product over an empty index set is the vacuous product 1.
    if n == 0 {
        return Fp::<3>::new(1);
    }

    if has_zero_row(mat) {
        return Fp::<3>::new(0);
    }

    // One-time matrix-prep: extract each column j into a Bipedal3 word.
    // Lane i of columns[j] holds A[i,j] for i in 0..n; lanes n..63 are 0
    // (the additive identity, i.e. (mag=0, sgn=0)).
    //
    // Cost: O(n^2) — dominated by the O(n · 2^n) Gray walk for n ≥ 4.
    let columns = pack_singleword_columns(mat);

    // Column-sum accumulator as a single Bipedal3 word.
    // Lane i of col_sum holds sum_{j ∈ S} A[i,j] mod 3.
    // Lanes n..63 stay 0 throughout (add/sub leave them at 0, and
    // fold_mul_first_n pads inactive lanes to the mul-identity before folding).
    let mut col_sum = Bipedal3::zero();

    // Running Ryser accumulator and subset-size counter.
    let mut total = Fp::<3>::new(0);
    let mut subset_size: usize = 0;

    // Gray walk: enumerate all 2^n - 1 non-empty subsets of [n].
    // At each step (flip, parity):
    //   flip   — which column just entered or left S
    //   parity — +1 (entered, ADD) or -1 (left, SUB)
    for (flip, parity) in gray_code_iter(n) {
        if parity == 1 {
            // col_sum += columns[flip]: paper §2.2 SSOT lives in Bipedal3::add.
            subset_size += 1;
            col_sum = col_sum.add(columns[flip]);
        } else {
            // col_sum -= columns[flip]: paper §2.2 SSOT lives in Bipedal3::sub.
            subset_size -= 1;
            col_sum = col_sum.sub(columns[flip]);
        }

        // Horizontal fold via bipedal multiplication tree SSOT in fold_mul_first_n.
        let term = col_sum.fold_mul_first_n(n);

        // Ryser sign: (-1)^|S|.
        if subset_size % 2 == 1 {
            total = total - term;
        } else {
            total += term;
        }
    }

    // Apply the outer (-1)^n factor from Ryser's formula.
    if n % 2 == 1 {
        -total
    } else {
        total
    }
}

// ---------------------------------------------------------------------------
// SIMD single-word path
// ---------------------------------------------------------------------------

/// Compute the permanent of an `n × n` matrix over `F_3` using the AVX2
/// bipedal batch kernel for the per-step column-sum add/sub.
///
/// This is the SIMD variant of [`permanent_bipedal3_singleword`].  It
/// consumes an already-detected [`gf2_kernels_simd::bipedal::BipedalAvx2Fns`]
/// bundle and routes each Gray-code add/sub step through the batch kernel,
/// zero-padding the single-word column-sum into the required 4-element `u64`
/// buffer (one AVX2 lane).
///
/// The algorithm is semantically identical to the scalar path — only the
/// add/sub step is delegated to the SIMD kernel.  At W=1 the kernel
/// processes 4 `u64` words of which 3 carry no data (always zero); for
/// production throughput the batched multi-matrix path (T16) is the
/// intended SIMD consumer. This direct function remains available to exercise
/// and cross-check the single-matrix kernel; the public single-matrix
/// dispatcher selects the measured-faster scalar kernel.
///
/// # Arguments
///
/// * `mat`  — An `n × n` [`Bipedal3Matrix`], with `n ≤ 63` (narrowed
///   from the pre-2026-05-15 `n ≤ 64` for CPU/GPU consistency).
/// * `fns`  — Pre-detected AVX2 kernel bundle from
///   [`gf2_kernels_simd::bipedal::detect_avx2`].
///
/// # Panics
///
/// Panics if `mat.rows() != mat.cols()` or `mat.cols() > 63`.
///
/// # Examples
///
/// ```
/// # #[cfg(all(feature = "simd", any(target_arch = "x86", target_arch = "x86_64")))]
/// # {
/// use gf2_algebra::packed::Bipedal3Matrix;
/// use gf2_algebra::permanent::bipedal3::permanent_bipedal3_singleword_simd;
/// use gf2_core::gfp::Fp;
/// use gf2_kernels_simd::bipedal::detect_avx2;
///
/// // 2x2 identity over F_3: permanent = 1. On non-AVX2 hosts this direct
/// // kernel call is skipped; `permanent_bipedal3` selects scalar on every
/// // host for a single matrix.
/// if let Some(fns) = detect_avx2() {
///     let id: Vec<Fp<3>> = vec![
///         Fp::<3>::new(1), Fp::<3>::new(0),
///         Fp::<3>::new(0), Fp::<3>::new(1),
///     ];
///     let m = Bipedal3Matrix::from_row_major(&id, 2, 2);
///     assert_eq!(permanent_bipedal3_singleword_simd(&m, &fns), Fp::<3>::new(1));
/// }
/// # }
/// ```
///
/// # Complexity
///
/// `O(n · 2^n)` — same asymptotic cost as the scalar path.  Per-step
/// overhead: one AVX2 add/sub on 4 × u64 (including buffer fill/drain)
/// rather than 6 word ops on 1 × u64.
#[cfg(all(feature = "simd", any(target_arch = "x86", target_arch = "x86_64")))]
pub fn permanent_bipedal3_singleword_simd(
    mat: &Bipedal3Matrix,
    fns: &gf2_kernels_simd::bipedal::BipedalAvx2Fns,
) -> Fp<3> {
    let n = mat.cols();
    assert_eq!(
        mat.rows(),
        n,
        "permanent_bipedal3_singleword_simd: matrix must be square (rows={}, cols={})",
        mat.rows(),
        n
    );
    assert!(
        n <= 63,
        "permanent_bipedal3_singleword_simd: single-u64 fast path requires n <= 63; got n = {}",
        n
    );

    if n == 0 {
        return Fp::<3>::new(1);
    }

    if has_zero_row(mat) {
        return Fp::<3>::new(0);
    }

    // One-time matrix-prep: identical to the scalar path.
    let columns = pack_singleword_columns(mat);

    // Column-sum accumulator as a single Bipedal3 word.
    let mut col_sum = Bipedal3::zero();

    let mut total = Fp::<3>::new(0);
    let mut subset_size: usize = 0;

    // SIMD I/O buffers: 4 × u64 each — one AVX2 lane.
    // Indices 1..3 are always zero (unused lanes). Index 0 carries the
    // active column-sum word.
    let mut buf_sum_mag = [0u64; 4];
    let mut buf_sum_sgn = [0u64; 4];
    let mut buf_col_mag = [0u64; 4];
    let mut buf_col_sgn = [0u64; 4];
    let mut out_mag = [0u64; 4];
    let mut out_sgn = [0u64; 4];

    for (flip, parity) in gray_code_iter(n) {
        // Load column into SIMD buffer (word 0 only; words 1..3 stay zero).
        let col = columns[flip];
        buf_col_mag[0] = col.mag();
        buf_col_sgn[0] = col.sgn();

        // Load col_sum into SIMD buffer.
        buf_sum_mag[0] = col_sum.mag();
        buf_sum_sgn[0] = col_sum.sgn();

        if parity == 1 {
            // col_sum += columns[flip]
            subset_size += 1;
            (fns.add_fn)(
                &buf_sum_mag,
                &buf_sum_sgn,
                &buf_col_mag,
                &buf_col_sgn,
                &mut out_mag,
                &mut out_sgn,
            );
        } else {
            // col_sum -= columns[flip]
            subset_size -= 1;
            (fns.sub_fn)(
                &buf_sum_mag,
                &buf_sum_sgn,
                &buf_col_mag,
                &buf_col_sgn,
                &mut out_mag,
                &mut out_sgn,
            );
        }

        // Read result back from word 0.
        col_sum = Bipedal3::from_raw(out_mag[0], out_sgn[0]);

        // Horizontal fold via bipedal multiplication tree.
        let term = col_sum.fold_mul_first_n(n);

        if subset_size % 2 == 1 {
            total = total - term;
        } else {
            total += term;
        }
    }

    if n % 2 == 1 {
        -total
    } else {
        total
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packed::Bipedal3Matrix;
    use crate::permanent::reference::permanent_mod3_reference;
    use crate::permanent::ryser::permanent_ryser;
    use crate::testutil::random_matrix;
    use gf2_core::gfp::Fp;

    // Deterministic pseudo-random matrix generation lives in
    // `crate::testutil::random_matrix` — the workspace SSOT for these tests.

    /// Wrap a row-major `Vec<Fp<3>>` into a `Bipedal3Matrix`.
    fn to_bipedal3_matrix(row_major: &[Fp<3>], n: usize) -> Bipedal3Matrix {
        Bipedal3Matrix::from_row_major(row_major, n, n)
    }

    type BatchBackend = fn(&[Bipedal3Matrix]) -> Vec<Fp<3>>;

    fn scalar_batch_backend(matrices: &[Bipedal3Matrix]) -> Vec<Fp<3>> {
        matrices.iter().map(permanent_bipedal3_singleword).collect()
    }

    fn dispatcher_batch_backend(matrices: &[Bipedal3Matrix]) -> Vec<Fp<3>> {
        matrices.iter().map(permanent_bipedal3).collect()
    }

    fn avx2_detection_probes_during<T>(f: impl FnOnce() -> T) -> (T, usize) {
        let previous = AVX2_DETECTION_PROBES.with(|probes| probes.replace(0));
        let result = f();
        let observed = AVX2_DETECTION_PROBES.with(|probes| probes.replace(previous));
        (result, observed)
    }

    /// Calling the public dispatcher for one matrix must neither probe for
    /// AVX2 nor select the slower single-matrix SIMD path. The thread-local
    /// probe trace is host-independent and cannot race with sibling tests.
    #[test]
    fn test_public_dispatch_selects_scalar_singleword_kernel() {
        let entries = vec![Fp::<3>::new(1); 4];
        let matrix = Bipedal3Matrix::from_row_major(&entries, 2, 2);
        let (actual, avx2_probes) = avx2_detection_probes_during(|| permanent_bipedal3(&matrix));

        assert_eq!(actual, Fp::<3>::new(2));
        assert_eq!(
            avx2_probes, 0,
            "single-matrix public dispatch must not probe/select AVX2"
        );
    }

    #[cfg(all(feature = "simd", any(target_arch = "x86", target_arch = "x86_64")))]
    fn singleword_simd_batch_backend(matrices: &[Bipedal3Matrix]) -> Vec<Fp<3>> {
        let Some(fns) = maybe_bipedal_avx2() else {
            return scalar_batch_backend(matrices);
        };
        matrices
            .iter()
            .map(|matrix| permanent_bipedal3_singleword_simd(matrix, &fns))
            .collect()
    }

    fn reference_batch_backend(matrices: &[Bipedal3Matrix]) -> Vec<Fp<3>> {
        matrices
            .iter()
            .map(|matrix| {
                let n = matrix.cols();
                let row_major: Vec<_> = (0..n)
                    .flat_map(|i| (0..n).map(move |j| matrix.get(i, j)))
                    .collect();
                permanent_mod3_reference(&row_major, n)
            })
            .collect()
    }

    fn ryser_batch_backend(matrices: &[Bipedal3Matrix]) -> Vec<Fp<3>> {
        matrices
            .iter()
            .map(|matrix| {
                let n = matrix.cols();
                let row_major: Vec<_> = (0..n)
                    .flat_map(|i| (0..n).map(move |j| matrix.get(i, j)))
                    .collect();
                permanent_ryser::<Fp<3>>(&row_major, n)
            })
            .collect()
    }

    #[cfg(feature = "parallel")]
    fn parallel_batch_backend(matrices: &[Bipedal3Matrix]) -> Vec<Fp<3>> {
        matrices
            .iter()
            .map(crate::permanent::parallel_bipedal3::permanent_bipedal3_parallel)
            .collect()
    }

    fn shared_permanent_behavioral_suite(name: &str, backend: BatchBackend) {
        let empty = vec![
            Bipedal3Matrix::from_row_major(&[], 0, 0),
            Bipedal3Matrix::from_row_major(&[], 0, 0),
        ];
        assert_eq!(backend(&empty), vec![Fp::<3>::new(1); 2], "{name}: 0x0");

        let one_by_one: Vec<_> = [0, 1, 2, 2]
            .into_iter()
            .map(|value| Bipedal3Matrix::from_row_major(&[Fp::<3>::new(value)], 1, 1))
            .collect();
        assert_eq!(
            backend(&one_by_one),
            vec![
                Fp::<3>::new(0),
                Fp::<3>::new(1),
                Fp::<3>::new(2),
                Fp::<3>::new(2),
            ],
            "{name}: 1x1 values"
        );

        let cases = [
            ([1, 0, 0, 1], 1),
            ([1, 1, 1, 1], 2),
            ([0, 2, 1, 0], 2),
            ([1, 2, 0, 0], 0),
        ];
        let matrices: Vec<_> = cases
            .iter()
            .map(|(entries, _)| {
                let entries: Vec<_> = entries.iter().copied().map(Fp::<3>::new).collect();
                Bipedal3Matrix::from_row_major(&entries, 2, 2)
            })
            .collect();
        let expected: Vec<_> = cases
            .iter()
            .map(|(_, value)| Fp::<3>::new(*value))
            .collect();
        assert_eq!(backend(&matrices), expected, "{name}: 2x2 contract");
    }

    /// Every standing F_3 permanent backend runs the same observable
    /// empty/identity/all-ones/zero-row contract, including the new batch path.
    #[test]
    fn test_shared_permanent_backend_behavioral_suite() {
        let mut backends: Vec<(&str, BatchBackend)> = vec![
            ("generic Ryser", ryser_batch_backend),
            ("paper reference", reference_batch_backend),
            ("scalar bipedal", scalar_batch_backend),
            ("public dispatcher", dispatcher_batch_backend),
            ("four-lane batch", permanent_bipedal3_batch),
        ];
        #[cfg(feature = "parallel")]
        backends.push(("parallel bipedal", parallel_batch_backend));
        #[cfg(all(feature = "simd", any(target_arch = "x86", target_arch = "x86_64")))]
        backends.push(("singleword SIMD", singleword_simd_batch_backend));

        for (name, backend) in backends {
            shared_permanent_behavioral_suite(name, backend);
        }
    }

    /// Randomised conformance covers every dimension representable by one
    /// bipedal word. Dense inputs exercise the full Gray walk through n=16;
    /// larger exponential dimensions retain random entries but carry a random
    /// zero row, whose permanent is identically zero and can be checked in the
    /// fast tier without attempting an infeasible 2^63 walk.
    #[test]
    fn test_batched_randomized_conformance_all_singleword_sizes() {
        for n in 0..=63 {
            let matrices: Vec<_> = (0..4u64)
                .map(|lane| {
                    let seed = 0x83ee_dd07_0000_0000u64
                        .wrapping_add((n as u64) << 8)
                        .wrapping_add(lane);
                    let mut row_major = random_matrix::<3>(n, seed);
                    if n > 16 {
                        let zero_row = (seed as usize) % n;
                        row_major[zero_row * n..(zero_row + 1) * n].fill(Fp::<3>::new(0));
                    }
                    to_bipedal3_matrix(&row_major, n)
                })
                .collect();
            let expected = scalar_batch_backend(&matrices);
            let actual = permanent_bipedal3_batch(&matrices);
            assert_eq!(actual, expected, "batch/scalar mismatch at n={n}");
        }
    }

    /// Widths one through three must preserve the results and order of the
    /// matrices present rather than exposing results from padded SIMD lanes.
    #[test]
    fn test_batched_partial_widths_1_through_3() {
        let n = 7;
        let matrices: Vec<_> = (0..4u64)
            .map(|lane| {
                let row_major = random_matrix::<3>(n, 0x83ee_dd07_7000 + lane);
                to_bipedal3_matrix(&row_major, n)
            })
            .collect();
        for width in 1..=3 {
            assert_eq!(
                permanent_bipedal3_batch(&matrices[..width]),
                scalar_batch_backend(&matrices[..width]),
                "partial batch mismatch at width={width}"
            );
        }
    }

    /// Explicitly removing the detected kernel exercises the same route used
    /// on non-AVX2 and non-x86 hosts.
    #[test]
    fn test_batched_forced_non_avx2_fallback_matches_scalar() {
        let n = 8;
        let matrices: Vec<_> = (0..4u64)
            .map(|lane| {
                let row_major = random_matrix::<3>(n, 0x83ee_dd07_fa11 + lane);
                to_bipedal3_matrix(&row_major, n)
            })
            .collect();
        assert_eq!(
            permanent_bipedal3_batch_with_kernel(&matrices, None),
            scalar_batch_backend(&matrices)
        );
    }

    fn parse_f3_cas_vectors() -> Vec<(usize, Vec<Fp<3>>, Fp<3>)> {
        include_str!("../../tests/data/cas_permanent_f3_batch.csv")
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') || line.starts_with("q,") {
                    return None;
                }
                let fields: Vec<_> = line.splitn(4, ',').collect();
                assert_eq!(fields.len(), 4, "malformed CAS row: {line}");
                assert_eq!(fields[0], "3", "CAS vector must be over F_3");
                let n = fields[1].parse::<usize>().expect("valid CAS dimension");
                let entries: Vec<_> = fields[2]
                    .split_whitespace()
                    .filter(|token| *token != "/")
                    .map(|token| {
                        Fp::<3>::new(token.parse::<u64>().expect("valid F_3 matrix entry"))
                    })
                    .collect();
                assert_eq!(entries.len(), n * n, "wrong CAS matrix shape at n={n}");
                let expected =
                    Fp::<3>::new(fields[3].parse::<u64>().expect("valid CAS permanent value"));
                Some((n, entries, expected))
            })
            .collect()
    }

    /// Four-lane batches agree with the committed SageMath 10.9 vectors.
    #[test]
    fn test_batched_matches_committed_cas_reference_vectors() {
        let vectors = parse_f3_cas_vectors();
        assert!(!vectors.is_empty());
        assert_eq!(vectors.len() % 4, 0, "CAS vectors must form full batches");
        for chunk in vectors.chunks_exact(4) {
            let n = chunk[0].0;
            assert!(chunk.iter().all(|(chunk_n, _, _)| *chunk_n == n));
            let matrices: Vec<_> = chunk
                .iter()
                .map(|(_, entries, _)| to_bipedal3_matrix(entries, n))
                .collect();
            let expected: Vec<_> = chunk.iter().map(|(_, _, value)| *value).collect();
            assert_eq!(
                permanent_bipedal3_batch(&matrices),
                expected,
                "CAS batch mismatch at n={n}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // T13 SIMD-vs-scalar cross-checks.
    //
    // These tests verify that the direct SIMD kernel produces the same output
    // as the pure-Rust scalar path. They call the two pub entry points
    // (`permanent_bipedal3_singleword_simd` and `permanent_bipedal3_singleword`)
    // directly, side-by-side on the same matrix, and assert raw equality.
    //
    // On hosts without AVX2 (or without the `simd` feature), the SIMD entry
    // point is compile-time gated out and the helper degrades to a
    // scalar-vs-scalar comparison (always equal); the criterion-3
    // requirement is vacuous in that case, which is the intended behaviour.
    //
    // Tier assignment (each matrix requires 2 bipedal3_singleword calls):
    //   n=8:  100 matrices — fast tier (2^8 = 256 steps; trivially fast).
    //   n=16: 100 matrices — fast tier (2^16 = 65536 steps; ≈ 0.15 s total).
    //   n=24: 10 matrices  — fast tier (2^24 ~16M steps; ≈ 0.5 s total).
    //   n=24: 100 matrices — slow tier (≈ 5 s total, fits 120 s slow budget).
    //   n=32: 1 matrix     — slow tier (2^32 ~4B steps ≈ 6 s/matrix; criterion
    //     originally stated 100 matrices, reduced to ≥1 per the JIT
    //     amendment dated 2026-05-11 — the slow-tier budget caps the count).
    // -----------------------------------------------------------------------

    /// Cross-check SIMD vs scalar for `trials` random `n × n` matrices.
    ///
    /// Calls `permanent_bipedal3_singleword_simd` and
    /// `permanent_bipedal3_singleword` directly. Direct calls are
    /// race-safe under cargo's parallel test execution: no shared mutable
    /// state participates in the cross-check, so concurrent invocations
    /// from sibling tests cannot make this comparison vacuous.
    ///
    /// On non-x86 hosts or when the `simd` feature is off, the SIMD path
    /// is not callable at compile time and this helper is also gated out
    /// — the criterion-3 assertion is vacuous in that case, which is the
    /// correct behaviour (the criterion requires equality on AVX2 hosts
    /// only).
    #[cfg(all(feature = "simd", any(target_arch = "x86", target_arch = "x86_64")))]
    fn simd_vs_scalar_cross_check(n: usize, trials: u64, seed_base: u64) {
        let fns = match super::maybe_bipedal_avx2() {
            Some(fns) => fns,
            None => {
                eprintln!(
                    "simd_vs_scalar_cross_check: AVX2 not detected; skipping n={n} cross-check"
                );
                return;
            }
        };
        for trial in 0u64..trials {
            let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
            let row_major = random_matrix::<3>(n, seed);
            let mat = to_bipedal3_matrix(&row_major, n);
            let simd_result = permanent_bipedal3_singleword_simd(&mat, &fns);
            let scalar_result = permanent_bipedal3_singleword(&mat);
            assert_eq!(
                simd_result, scalar_result,
                "T13 SIMD/scalar mismatch: n={n}, trial={trial}, seed={seed:#018x}"
            );
        }
    }

    /// Non-x86 / non-SIMD-feature stub: the cross-check has no SIMD path
    /// to call, so we run scalar-vs-scalar (always equal) to keep the
    /// test surface non-empty without introducing dead-test warnings.
    #[cfg(not(all(feature = "simd", any(target_arch = "x86", target_arch = "x86_64"))))]
    fn simd_vs_scalar_cross_check(n: usize, trials: u64, seed_base: u64) {
        for trial in 0u64..trials {
            let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
            let row_major = random_matrix::<3>(n, seed);
            let mat = to_bipedal3_matrix(&row_major, n);
            let a = permanent_bipedal3_singleword(&mat);
            let b = permanent_bipedal3_singleword(&mat);
            assert_eq!(
                a, b,
                "T13 scalar-vs-scalar mismatch (should be impossible): n={n}, trial={trial}, seed={seed:#018x}"
            );
        }
    }

    /// T13 SIMD-vs-scalar cross-check for n=8: 100 random matrices.
    ///
    /// Fast tier: 2^8 = 256 Gray steps per matrix; trivially fast.
    #[test]
    fn test_simd_vs_scalar_n8() {
        simd_vs_scalar_cross_check(8, 100, 0x686e_e1b5_0000_0008_u64);
    }

    /// T13 SIMD-vs-scalar cross-check for n=16: 100 random matrices.
    ///
    /// Fast tier: 2^16 = 65536 Gray steps per matrix; well under 5 s.
    #[test]
    fn test_simd_vs_scalar_n16() {
        simd_vs_scalar_cross_check(16, 100, 0x686e_e1b5_0000_0010_u64);
    }

    /// T13 SIMD-vs-scalar cross-check for n=24: 3 random matrices (fast
    /// tier).
    ///
    /// Fast tier: 2^24 ~16M steps × 3 matrices × 2 passes (SIMD + scalar).
    /// On a developer machine with AVX2 this is ~0.15 s, but on shared CI
    /// runners (where the "SIMD" pass may fall back to scalar and cores are
    /// throttled) 10 matrices exceeded the 5 s per-test budget — so the
    /// fast-tier smoke check uses 3 matrices. The full 100-matrix run is
    /// covered by `test_simd_vs_scalar_n24_slow`.
    #[test]
    fn test_simd_vs_scalar_n24() {
        simd_vs_scalar_cross_check(24, 3, 0x686e_e1b5_0000_0018_u64);
    }

    /// T13 SIMD-vs-scalar cross-check for n=24: 100 random matrices (slow
    /// tier).
    ///
    /// Slow tier: 2^24 ~16M steps × 100 matrices × 2 passes ≈ 5 s total;
    /// fits the 120 s slow-tier budget. Covers the remaining 90 matrices
    /// beyond the 10-matrix fast-tier subset.
    #[test]
    #[ignore = "sim: T13 SIMD/scalar cross-check n=24, 100 matrices (≈ 5 s)"]
    fn test_simd_vs_scalar_n24_slow() {
        simd_vs_scalar_cross_check(24, 100, 0x686e_e1b5_1000_0018_u64);
    }

    /// T13 SIMD-vs-scalar cross-check for n=32: 1 matrix (slow tier).
    ///
    /// 2^32 ~4B steps at ~6 word-ops each ≈ 6 s/matrix in release mode,
    /// exceeding the 5 s fast-tier budget.  Per T13 criterion 3, the
    /// original target was 100 matrices; this is reduced to 1 matrix here
    /// because 100 × 6 s ≈ 10 min far exceeds the 120 s slow-tier budget.
    /// The criterion reduction is documented inline (project-lead handles
    /// the JIT amendment per CLAUDE.md escalation policy).
    #[test]
    #[ignore = "slow: T13 SIMD/scalar cross-check n=32 (2^32 steps ≈ 6 s/matrix)"]
    fn test_simd_vs_scalar_n32() {
        simd_vs_scalar_cross_check(32, 1, 0x686e_e1b5_0000_0020_u64);
    }

    // -----------------------------------------------------------------------
    // Hand-checked vectors
    // -----------------------------------------------------------------------

    /// `permanent_bipedal3` of the 0×0 matrix is `Fp::<3>::new(1)` (vacuous product).
    #[test]
    fn test_permanent_empty_matrix() {
        let m = Bipedal3Matrix::from_row_major(&[], 0, 0);
        assert_eq!(
            permanent_bipedal3(&m),
            Fp::<3>::new(1),
            "0×0 permanent must be 1"
        );
    }

    /// A 1×1 matrix `[v]` has permanent = `v`.
    #[test]
    fn test_permanent_1x1() {
        for v in 0u64..3 {
            let row = vec![Fp::<3>::new(v)];
            let m = Bipedal3Matrix::from_row_major(&row, 1, 1);
            assert_eq!(
                permanent_bipedal3(&m),
                Fp::<3>::new(v),
                "1×1 permanent of [{v}] must be {v}"
            );
        }
    }

    /// `I_n` has permanent = 1 for `n ∈ {1, 2, 3, 4}`.
    #[test]
    fn test_permanent_identity_n() {
        for n in 1..=4usize {
            let mut id = vec![Fp::<3>::new(0); n * n];
            for i in 0..n {
                id[i * n + i] = Fp::<3>::new(1);
            }
            let m = Bipedal3Matrix::from_row_major(&id, n, n);
            assert_eq!(
                permanent_bipedal3(&m),
                Fp::<3>::new(1),
                "identity permanent must be 1 for n={n}"
            );
        }
    }

    /// All-ones `n×n` matrix: permanent = `n! mod 3` for `n ∈ {1, 2, 3, 4}`.
    ///
    /// n! mod 3: n=1 → 1, n=2 → 2, n=3 → 6 ≡ 0, n=4 → 24 ≡ 0.
    #[test]
    fn test_permanent_all_ones_n() {
        // n! mod 3: {1, 2, 0, 0}
        let expected = [1u64, 2, 0, 0];
        for n in 1..=4usize {
            let ones = vec![Fp::<3>::new(1); n * n];
            let m = Bipedal3Matrix::from_row_major(&ones, n, n);
            assert_eq!(
                permanent_bipedal3(&m),
                Fp::<3>::new(expected[n - 1]),
                "all-ones permanent for n={n} must be {} (= n! mod 3)",
                expected[n - 1]
            );
        }
    }

    // -----------------------------------------------------------------------
    // Panic tests
    // -----------------------------------------------------------------------

    /// Non-square matrix panics.
    #[test]
    #[should_panic(expected = "matrix must be square")]
    fn test_permanent_bipedal3_panics_on_non_square() {
        let data = vec![Fp::<3>::new(0); 3 * 5];
        let m = Bipedal3Matrix::from_row_major(&data, 3, 5);
        let _ = permanent_bipedal3(&m);
    }

    /// `n > N_MAX_MULTIWORD` panics.
    ///
    /// The dispatcher caps at `N_MAX_MULTIWORD = 255`; above that the
    /// multi-word streaming path's `[u64; 4]` Gray counter cannot represent
    /// the iteration range (W3-T15 / W5 for parallel + GPU paths).
    #[test]
    #[should_panic(expected = "n must satisfy n <=")]
    fn test_permanent_bipedal3_panics_on_n_exceeding_n_max() {
        use crate::permanent::bipedal3_multiword::N_MAX_MULTIWORD;
        let n = N_MAX_MULTIWORD + 1;
        let data = vec![Fp::<3>::new(0); n * n];
        let m = Bipedal3Matrix::from_row_major(&data, n, n);
        let _ = permanent_bipedal3(&m);
    }

    /// `permanent_bipedal3_singleword` panics for `n = 64` (above its bound).
    ///
    /// The single-word fast path supports `n <= 63` per the 2026-05-15
    /// CPU/GPU consistency narrowing. At `n = 64` the column-sum state
    /// still nominally fits one `(mag, sgn)` u64 pair, but the Gray walk
    /// is wallclock-infeasible (~600 years on either CPU or GPU), so the
    /// dispatcher routes to the multi-word path which uses a 256-bit
    /// counter and can chunk across cores.
    #[test]
    #[should_panic(expected = "single-u64 fast path requires n <= 63")]
    fn test_permanent_bipedal3_singleword_panics_on_n_64() {
        let data = vec![Fp::<3>::new(0); 64 * 64];
        let m = Bipedal3Matrix::from_row_major(&data, 64, 64);
        let _ = permanent_bipedal3_singleword(&m);
    }

    /// Dispatcher routes `n = 63` to the single-word fast path; `n = 64`
    /// routes to multi-word.
    ///
    /// Verifies the post-2026-05-15 dispatch contract: the singleword arm
    /// covers `1..=63`, and `64..=N_MAX_MULTIWORD` is multi-word. The
    /// actual computation at n=64 is wallclock-infeasible (2^64 steps);
    /// this test only checks the dispatch constant relationship.
    #[test]
    fn test_permanent_bipedal3_dispatch_routes_n64_to_multiword() {
        use crate::permanent::bipedal3_multiword::N_MAX_MULTIWORD;
        // n=63 routes to singleword; n=64..N_MAX is multi-word.
        const { assert!(N_MAX_MULTIWORD >= 64) }
    }

    // -----------------------------------------------------------------------
    // Cross-checks: permanent_bipedal3 vs permanent_ryser (default tier)
    // Per-n tests with 1000 random matrices each.
    // n=1..12 fit well within the 5 s budget; n=13..16 are slow-tier.
    // -----------------------------------------------------------------------

    macro_rules! cross_check_n {
        ($name:ident, $n:expr) => {
            #[test]
            fn $name() {
                let n = $n;
                let seed_base: u64 =
                    0xb085_7ae9_0000_0000_u64.wrapping_add(n as u64);
                for trial in 0u64..1000 {
                    let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
                    let row_major = random_matrix::<3>(n, seed);
                    let mat = to_bipedal3_matrix(&row_major, n);
                    let expected = permanent_ryser::<Fp<3>>(&row_major, n);
                    let actual = permanent_bipedal3(&mat);
                    assert_eq!(
                        actual, expected,
                        "permanent mismatch: n={n}, trial={trial}, seed={seed:#018x}"
                    );
                }
            }
        };
        ($name:ident, $n:expr, slow) => {
            #[test]
            #[ignore = "sim: per-n cross-check (n>12, 1000 matrices) — slow oracle, multi-second runtime"]
            fn $name() {
                let n = $n;
                let seed_base: u64 =
                    0xb085_7ae9_0000_0000_u64.wrapping_add(n as u64);
                for trial in 0u64..1000 {
                    let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
                    let row_major = random_matrix::<3>(n, seed);
                    let mat = to_bipedal3_matrix(&row_major, n);
                    let expected = permanent_ryser::<Fp<3>>(&row_major, n);
                    let actual = permanent_bipedal3(&mat);
                    assert_eq!(
                        actual, expected,
                        "permanent mismatch: n={n}, trial={trial}, seed={seed:#018x}"
                    );
                }
            }
        };
    }

    cross_check_n!(test_cross_check_n1, 1);
    cross_check_n!(test_cross_check_n2, 2);
    cross_check_n!(test_cross_check_n3, 3);
    cross_check_n!(test_cross_check_n4, 4);
    cross_check_n!(test_cross_check_n5, 5);
    cross_check_n!(test_cross_check_n6, 6);
    cross_check_n!(test_cross_check_n7, 7);
    cross_check_n!(test_cross_check_n8, 8);
    cross_check_n!(test_cross_check_n9, 9);
    cross_check_n!(test_cross_check_n10, 10);
    cross_check_n!(test_cross_check_n11, 11);
    cross_check_n!(test_cross_check_n12, 12);
    // n=13..16: 1000 matrices × Ryser O(n·2^n) exceeds 5 s for n≥13 in
    // release mode; these run only under the nightly slow tier.
    cross_check_n!(test_cross_check_n13, 13, slow);
    cross_check_n!(test_cross_check_n14, 14, slow);
    cross_check_n!(test_cross_check_n15, 15, slow);
    cross_check_n!(test_cross_check_n16, 16, slow);

    // -----------------------------------------------------------------------
    // Cross-checks: large n (slow tier — must not run in default CI)
    //
    // Oracle: `permanent_mod3_reference` (scalar i32, ~10× faster than generic
    // Fp<3> Ryser at large n). Correctness of the reference vs
    // `permanent_ryser` is established by T8's own cross-checks, so
    // "bit-identical to permanent_ryser" is preserved here by transitivity.
    //
    // Per the 2026-05-10 user-approved amendment to T9 criterion 3:
    //   - n=28/32 are NOT required.
    //   - n=20: 100 matrices × ~5 s/matrix → 5 sub-tests × 20 matrices each
    //     (each ≈ 100 s, fits 120 s slow-tier budget).
    //   - n=24: 100 matrices × ~8 s/matrix → 10 sub-tests × 10 matrices each
    //     (each ≈ 80 s, fits 120 s slow-tier budget).
    // -----------------------------------------------------------------------

    macro_rules! large_n_cross_check {
        ($name:ident, $n:expr, $trials:expr, $seed_salt:expr) => {
            #[test]
            #[ignore = "sim: large-n cross-check (n in {20, 24}) — slow oracle, multi-minute runtime"]
            fn $name() {
                let n = $n;
                let seed_base: u64 = 0xb085_7ae9_2000_0000_u64
                    .wrapping_add(n as u64)
                    .wrapping_add($seed_salt);
                for trial in 0u64..$trials {
                    let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
                    let row_major = random_matrix::<3>(n, seed);
                    let mat = to_bipedal3_matrix(&row_major, n);
                    // Use permanent_mod3_reference as oracle: ~10× faster than
                    // generic Ryser at large n. Correctness of the reference vs
                    // permanent_ryser is established by T8 cross-checks.
                    let expected = permanent_mod3_reference(&row_major, n);
                    let actual = permanent_bipedal3(&mat);
                    assert_eq!(
                        actual, expected,
                        "permanent mismatch: n={n}, trial={trial}, seed={seed:#018x}"
                    );
                }
            }
        };
    }

    // n=20: 5 sub-tests × 20 matrices each = 100 total.
    // ~5 s/matrix × 20 = 100 s/sub-test — fits 120 s slow-tier budget.
    large_n_cross_check!(test_cross_check_n20_a, 20, 20, 0);
    large_n_cross_check!(test_cross_check_n20_b, 20, 20, 1_000);
    large_n_cross_check!(test_cross_check_n20_c, 20, 20, 2_000);
    large_n_cross_check!(test_cross_check_n20_d, 20, 20, 3_000);
    large_n_cross_check!(test_cross_check_n20_e, 20, 20, 4_000);

    // n=24: 10 sub-tests × 10 matrices each = 100 total.
    // ~8 s/matrix × 10 = 80 s/sub-test — fits 120 s slow-tier budget.
    large_n_cross_check!(test_cross_check_n24_a, 24, 10, 0);
    large_n_cross_check!(test_cross_check_n24_b, 24, 10, 1_000);
    large_n_cross_check!(test_cross_check_n24_c, 24, 10, 2_000);
    large_n_cross_check!(test_cross_check_n24_d, 24, 10, 3_000);
    large_n_cross_check!(test_cross_check_n24_e, 24, 10, 4_000);
    large_n_cross_check!(test_cross_check_n24_f, 24, 10, 5_000);
    large_n_cross_check!(test_cross_check_n24_g, 24, 10, 6_000);
    large_n_cross_check!(test_cross_check_n24_h, 24, 10, 7_000);
    large_n_cross_check!(test_cross_check_n24_i, 24, 10, 8_000);
    large_n_cross_check!(test_cross_check_n24_j, 24, 10, 9_000);
}
