//! Rayon-parallel `permanent_bipedal3` over fixed Gray-code chunks:
//! [`permanent_bipedal3_parallel`] reads its chunk length from
//! [`permanent_chunk_len`].

use gf2_core::gfp::Fp;
use rayon::prelude::*;

use crate::gray::gray_code_index_to_subset;
use crate::packed::bipedal3::{Bipedal3, Bipedal3Matrix};
use crate::packed::PackedField;
use crate::packed::PackedFieldVec;

pub use super::CHUNK_SUBSETS;

/// Returns the Gray-code chunk length [`permanent_bipedal3_parallel`] passes
/// to [`permanent_bipedal3_parallel_with_chunk`].
///
/// Reads [`crate::tuning::AlgebraTuning::permanent`]'s
/// [`crate::tuning::PermanentSelectors::gray_chunk_subsets`] once per call.
/// With no envelope installed,
/// [`crate::tuning::active`] resolves to [`crate::tuning::AlgebraTuning::CONSERVATIVE`],
/// whose value is the canonical [`CHUNK_SUBSETS`] constant.
///
/// This is one `O(log s)` immutable section lookup for `s` installed sections;
/// after process resolution it allocates nothing and takes no lock.
///
/// # Panics
///
/// Panics only if installed erased tuning storage violates its internal typed
/// section invariant. Malformed profile input is rejected before installation.
#[must_use]
pub fn permanent_chunk_len() -> usize {
    crate::tuning::active().permanent().gray_chunk_subsets()
}

/// Records the `chunk_subsets` value most recently received by
/// [`permanent_bipedal3_parallel_with_chunk`]. Read through
/// [`last_effective_chunk`].
#[cfg(any(test, feature = "test-support"))]
static LAST_EFFECTIVE_CHUNK: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Route-observation hook: returns the `chunk_subsets` value most recently
/// received by [`permanent_bipedal3_parallel_with_chunk`].
///
/// Every valid chunk length produces the same permanent (`# Determinism`
/// above), so output equality cannot show that the resolved
/// [`permanent_chunk_len`] reached the callee; this hook exposes the
/// received value directly.
#[cfg(any(test, feature = "test-support"))]
#[must_use]
pub fn last_effective_chunk() -> usize {
    LAST_EFFECTIVE_CHUNK.load(std::sync::atomic::Ordering::SeqCst)
}

/// Executed chunk lengths and count for one completed parallel permanent walk.
///
/// These are facts about the effective production partition, rather than the
/// requested chunk value reported by [`last_effective_chunk`]. For a walk over
/// `S = 2^n - 1` non-empty subsets with requested chunk length `q`, the fields
/// are `min(q, S)`, `ceil(S / q)`, and `S - (chunk_count - 1) * q`.
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PermanentPartitionObservation {
    /// Greatest number of subsets executed by any chunk in the walk.
    pub maximum_chunk_len: u64,
    /// Number of chunks executed by the completed walk.
    pub chunk_count: u64,
    /// Number of subsets executed by the final chunk.
    pub last_chunk_len: u64,
}

/// Effective production partition published by the last completed non-empty
/// parallel permanent walk.
#[cfg(any(test, feature = "test-support"))]
static LAST_EFFECTIVE_PARTITION: std::sync::Mutex<Option<PermanentPartitionObservation>> =
    std::sync::Mutex::new(None);

#[cfg(any(test, feature = "test-support"))]
fn record_effective_partition(total_subsets: u64, chunk_subsets: usize) {
    let requested = chunk_subsets as u64;
    let chunk_count = 1 + (total_subsets - 1) / requested;
    let observation = PermanentPartitionObservation {
        maximum_chunk_len: requested.min(total_subsets),
        chunk_count,
        last_chunk_len: total_subsets - (chunk_count - 1) * requested,
    };
    *LAST_EFFECTIVE_PARTITION
        .lock()
        .expect("parallel permanent partition observation lock is not poisoned") =
        Some(observation);
}

/// Clears the test-support observation of the effective permanent partition.
///
/// This resets only evidence instrumentation. It does not reset tuning or the
/// requested-value observation returned by [`last_effective_chunk`].
#[cfg(any(test, feature = "test-support"))]
pub fn reset_last_effective_partition() {
    *LAST_EFFECTIVE_PARTITION
        .lock()
        .expect("parallel permanent partition observation lock is not poisoned") = None;
}

/// Returns the effective partition from the last completed non-empty walk.
///
/// Call this after exactly one parallel permanent invocation whose call was
/// preceded by [`reset_last_effective_partition`]. The observation is
/// published once, after the production reduction completes. An empty `0 x 0`
/// permanent performs no subset walk and therefore leaves this as `None`.
/// Concurrent permanent invocations intentionally have no attribution
/// guarantee.
#[cfg(any(test, feature = "test-support"))]
#[must_use]
pub fn last_effective_partition() -> Option<PermanentPartitionObservation> {
    *LAST_EFFECTIVE_PARTITION
        .lock()
        .expect("parallel permanent partition observation lock is not poisoned")
}

/// Compute the permanent of an `n × n` matrix over `F_3` using rayon-parallel
/// Ryser's formula, splitting the Gray-code subset enumeration across worker
/// threads.
///
/// Splits the `2^n - 1` non-empty-subset walk of
/// [`super::bipedal3::permanent_bipedal3`] into chunks of
/// [`permanent_chunk_len`] subsets. Output is identical to
/// `permanent_bipedal3` on the same matrix for every thread count and
/// schedule.
///
/// # Panics
///
/// Panics if `mat.rows() != mat.cols()` (matrix must be square).
///
/// Panics if `mat.cols() > 63` (single-`u64` fast path requires `n ≤ 63`).
///
/// # Complexity
///
/// `O(n · 2^n / T)` field operations per thread for `T` rayon threads, plus
/// `O(n)` per chunk start to reconstruct the initial `col_sum`. Matrix prep
/// is `O(n^2)`.
pub fn permanent_bipedal3_parallel(mat: &Bipedal3Matrix) -> Fp<3> {
    permanent_bipedal3_parallel_with_chunk(mat, permanent_chunk_len())
}

/// Same as [`permanent_bipedal3_parallel`] but takes the chunk size as an
/// argument. The `parallel_chunk_sweep` example calls this function, so its
/// sweep measures the code path [`permanent_bipedal3_parallel`] runs.
///
/// # Panics
///
/// Panics on non-square matrices, `n > 63`, or `chunk_subsets == 0`.
///
/// # Complexity
///
/// As [`permanent_bipedal3_parallel`], with `ceil((2^n - 1) / chunk_subsets)`
/// chunk starts.
pub fn permanent_bipedal3_parallel_with_chunk(mat: &Bipedal3Matrix, chunk_subsets: usize) -> Fp<3> {
    let n = mat.cols();
    assert_eq!(
        mat.rows(),
        n,
        "permanent_bipedal3_parallel_with_chunk: matrix must be square (rows={}, cols={})",
        mat.rows(),
        n
    );
    assert!(
        n <= 63,
        "permanent_bipedal3_parallel_with_chunk: single-u64 fast path requires n <= 63; got n = {}",
        n
    );
    assert!(
        chunk_subsets >= 1,
        "permanent_bipedal3_parallel_with_chunk: chunk_subsets must be >= 1; got {chunk_subsets}"
    );

    #[cfg(any(test, feature = "test-support"))]
    LAST_EFFECTIVE_CHUNK.store(chunk_subsets, std::sync::atomic::Ordering::SeqCst);

    // 0×0: one (empty) permutation, product = 1.
    if n == 0 {
        return Fp::<3>::new(1);
    }

    // Lane i of columns[j] holds A[i,j] for i in 0..n; lanes n..63 are 0.
    let columns: Vec<Bipedal3> = (0..n)
        .map(|j| {
            let col_vec = mat.column(j);
            let mut col = Bipedal3::zero();
            for i in 0..n {
                col = col.with_lane(i, col_vec.get(i));
            }
            col
        })
        .collect();

    let total_subsets = (1u64 << n) - 1; // count of non-empty subsets

    // Each chunk covers [chunk_start, chunk_end) of the Gray-code index space
    // 1..=total_subsets. F_3 addition is commutative and associative, so the
    // reduce is independent of rayon's scheduling.
    let partial_total: Fp<3> = (1..=total_subsets)
        .step_by(chunk_subsets)
        .par_bridge()
        .map(|chunk_start| {
            process_chunk(
                &columns,
                n,
                chunk_start,
                chunk_start
                    .saturating_add(chunk_subsets as u64)
                    .min(total_subsets + 1),
            )
        })
        .reduce(|| Fp::<3>::new(0), |a, b| a + b);

    #[cfg(any(test, feature = "test-support"))]
    record_effective_partition(total_subsets, chunk_subsets);

    // Outer (-1)^n factor from Ryser's formula.
    if n % 2 == 1 {
        -partial_total
    } else {
        partial_total
    }
}

/// Partial Ryser sum over the Gray-code indices `[start, end)`, `start ≥ 1`,
/// for `n` active lanes per column word.
fn process_chunk(columns: &[Bipedal3], n: usize, start: u64, end: u64) -> Fp<3> {
    // g(start) = start ^ (start >> 1) is the bitmask of columns in the subset.
    let start_mask = gray_code_index_to_subset(start);
    let mut col_sum = Bipedal3::zero();
    for (j, &col) in columns.iter().enumerate().take(n) {
        if (start_mask >> j) & 1 == 1 {
            col_sum = col_sum.add(col);
        }
    }

    let mut subset_size: usize = start_mask.count_ones() as usize;

    let term = col_sum.fold_mul_first_n(n);
    let mut partial = if subset_size % 2 == 1 { -term } else { term };

    // Step k flips column trailing_zeros(k); bit `flip` of g(k) gives add vs sub.
    for k in (start + 1)..end {
        let flip = k.trailing_zeros() as usize;
        let g_k = k ^ (k >> 1);
        let parity: i8 = if ((g_k >> flip) & 1) == 1 { 1 } else { -1 };

        if parity == 1 {
            subset_size += 1;
            col_sum = col_sum.add(columns[flip]);
        } else {
            subset_size -= 1;
            col_sum = col_sum.sub(columns[flip]);
        }

        let term = col_sum.fold_mul_first_n(n);
        if subset_size % 2 == 1 {
            partial = partial - term;
        } else {
            partial += term;
        }
    }

    partial
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packed::Bipedal3Matrix;
    use crate::permanent::bipedal3::permanent_bipedal3;

    #[cfg(feature = "test-support")]
    use crate::testutil::random_matrix;

    fn to_bipedal3_matrix(row_major: &[Fp<3>], n: usize) -> Bipedal3Matrix {
        Bipedal3Matrix::from_row_major(row_major, n, n)
    }

    #[test]
    fn test_parallel_permanent_empty_matrix() {
        let m = Bipedal3Matrix::from_row_major(&[], 0, 0);
        assert_eq!(
            permanent_bipedal3_parallel(&m),
            Fp::<3>::new(1),
            "0x0 permanent must be 1"
        );
    }

    #[test]
    fn test_parallel_permanent_1x1() {
        for v in 0u64..3 {
            let row = vec![Fp::<3>::new(v)];
            let m = Bipedal3Matrix::from_row_major(&row, 1, 1);
            assert_eq!(
                permanent_bipedal3_parallel(&m),
                Fp::<3>::new(v),
                "1x1 permanent of [{v}] must be {v}"
            );
        }
    }

    #[test]
    fn test_parallel_permanent_identity_n() {
        for n in 1..=4usize {
            let mut id = vec![Fp::<3>::new(0); n * n];
            for i in 0..n {
                id[i * n + i] = Fp::<3>::new(1);
            }
            let m = Bipedal3Matrix::from_row_major(&id, n, n);
            assert_eq!(
                permanent_bipedal3_parallel(&m),
                Fp::<3>::new(1),
                "identity permanent must be 1 for n={n}"
            );
        }
    }

    #[test]
    fn test_parallel_permanent_all_ones_n() {
        // n! mod 3: {1, 2, 0, 0}
        let expected = [1u64, 2, 0, 0];
        for n in 1..=4usize {
            let ones = vec![Fp::<3>::new(1); n * n];
            let m = Bipedal3Matrix::from_row_major(&ones, n, n);
            assert_eq!(
                permanent_bipedal3_parallel(&m),
                Fp::<3>::new(expected[n - 1]),
                "all-ones permanent for n={n} must be {}",
                expected[n - 1]
            );
        }
    }

    #[test]
    #[should_panic(expected = "matrix must be square")]
    fn test_parallel_permanent_panics_on_non_square() {
        let data = vec![Fp::<3>::new(0); 3 * 5];
        let m = Bipedal3Matrix::from_row_major(&data, 3, 5);
        let _ = permanent_bipedal3_parallel(&m);
    }

    #[test]
    #[should_panic(expected = "single-u64 fast path requires n <= 63")]
    fn test_parallel_permanent_panics_on_n_64() {
        let data = vec![Fp::<3>::new(0); 64 * 64];
        let m = Bipedal3Matrix::from_row_major(&data, 64, 64);
        let _ = permanent_bipedal3_parallel(&m);
    }

    #[test]
    fn test_parallel_with_chunk_matches_default_wrapper() {
        // Several chunk sizes, including 1 (every Gray index in its own chunk)
        // and a value larger than 2^n (a single chunk).
        let n = 6;
        let data: Vec<Fp<3>> = (0..n * n).map(|i| Fp::<3>::new((i as u64) % 3)).collect();
        let mat = Bipedal3Matrix::from_row_major(&data, n, n);
        let expected = permanent_bipedal3_parallel(&mat);
        for &chunk in &[1usize, 2, 4, 8, 64, 1024, 1 << 20] {
            let actual = permanent_bipedal3_parallel_with_chunk(&mat, chunk);
            assert_eq!(
                actual, expected,
                "chunk-parametrised result diverged from wrapper at chunk={chunk}"
            );
        }
    }

    #[test]
    #[should_panic(expected = "chunk_subsets must be >= 1")]
    fn test_parallel_with_chunk_panics_on_zero() {
        let data = vec![Fp::<3>::new(0); 4 * 4];
        let m = Bipedal3Matrix::from_row_major(&data, 4, 4);
        let _ = permanent_bipedal3_parallel_with_chunk(&m, 0);
    }

    #[test]
    #[should_panic(expected = "matrix must be square")]
    fn test_parallel_with_chunk_panics_on_non_square() {
        let data = vec![Fp::<3>::new(0); 3 * 5];
        let m = Bipedal3Matrix::from_row_major(&data, 3, 5);
        let _ = permanent_bipedal3_parallel_with_chunk(&m, 4);
    }

    #[test]
    #[should_panic(expected = "single-u64 fast path requires n <= 63")]
    fn test_parallel_with_chunk_panics_on_n_64() {
        let data = vec![Fp::<3>::new(0); 64 * 64];
        let m = Bipedal3Matrix::from_row_major(&data, 64, 64);
        let _ = permanent_bipedal3_parallel_with_chunk(&m, 1024);
    }

    macro_rules! cross_check_parallel_n {
        ($name:ident, $n:expr) => {
            #[test]
            #[cfg(feature = "test-support")]
            fn $name() {
                let n = $n;
                let seed_base: u64 = 0x0525_0df5_0000_0000_u64.wrapping_add(n as u64);
                for trial in 0u64..100 {
                    let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
                    let row_major = random_matrix::<3>(n, seed);
                    let mat = to_bipedal3_matrix(&row_major, n);
                    let expected = permanent_bipedal3(&mat);
                    let actual = permanent_bipedal3_parallel(&mat);
                    assert_eq!(
                        actual, expected,
                        "parallel vs serial mismatch: n={n}, trial={trial}, seed={seed:#018x}"
                    );
                }
            }
        };
        ($name:ident, $n:expr, slow) => {
            #[test]
            #[ignore = "sim: parallel vs serial cross-check (large n, 100 matrices) — slow, multi-minute runtime"]
            #[cfg(feature = "test-support")]
            fn $name() {
                let n = $n;
                let seed_base: u64 = 0x0525_0df5_0000_0000_u64.wrapping_add(n as u64);
                for trial in 0u64..100 {
                    let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
                    let row_major = random_matrix::<3>(n, seed);
                    let mat = to_bipedal3_matrix(&row_major, n);
                    let expected = permanent_bipedal3(&mat);
                    let actual = permanent_bipedal3_parallel(&mat);
                    assert_eq!(
                        actual, expected,
                        "parallel vs serial mismatch: n={n}, trial={trial}, seed={seed:#018x}"
                    );
                }
            }
        };
    }

    cross_check_parallel_n!(test_parallel_cross_check_n1, 1);
    cross_check_parallel_n!(test_parallel_cross_check_n2, 2);
    cross_check_parallel_n!(test_parallel_cross_check_n3, 3);
    cross_check_parallel_n!(test_parallel_cross_check_n4, 4);
    cross_check_parallel_n!(test_parallel_cross_check_n5, 5);
    cross_check_parallel_n!(test_parallel_cross_check_n6, 6);
    cross_check_parallel_n!(test_parallel_cross_check_n7, 7);
    cross_check_parallel_n!(test_parallel_cross_check_n8, 8);
    cross_check_parallel_n!(test_parallel_cross_check_n9, 9);
    cross_check_parallel_n!(test_parallel_cross_check_n10, 10);
    cross_check_parallel_n!(test_parallel_cross_check_n11, 11);
    cross_check_parallel_n!(test_parallel_cross_check_n12, 12);

    cross_check_parallel_n!(test_parallel_cross_check_n20, 20);

    macro_rules! large_n_parallel_cross_check {
        ($name:ident, $n:expr, $trials:expr, $seed_salt:expr) => {
            #[test]
            #[ignore = "sim: parallel vs serial cross-check (large n, 10 matrices) — slow, multi-minute runtime"]
            #[cfg(feature = "test-support")]
            fn $name() {
                let n = $n;
                let seed_base: u64 = 0x0525_0df5_2000_0000_u64
                    .wrapping_add(n as u64)
                    .wrapping_add($seed_salt);
                for trial in 0u64..$trials {
                    let seed = seed_base.wrapping_add(trial.wrapping_mul(1_000_003));
                    let row_major = random_matrix::<3>(n, seed);
                    let mat = to_bipedal3_matrix(&row_major, n);
                    let expected = permanent_bipedal3(&mat);
                    let actual = permanent_bipedal3_parallel(&mat);
                    assert_eq!(
                        actual, expected,
                        "parallel vs serial mismatch: n={n}, trial={trial}, seed={seed:#018x}"
                    );
                }
            }
        };
    }

    large_n_parallel_cross_check!(test_parallel_cross_check_n24_a, 24, 10, 0);
    large_n_parallel_cross_check!(test_parallel_cross_check_n24_b, 24, 10, 1_000);
    large_n_parallel_cross_check!(test_parallel_cross_check_n24_c, 24, 10, 2_000);
    large_n_parallel_cross_check!(test_parallel_cross_check_n24_d, 24, 10, 3_000);
    large_n_parallel_cross_check!(test_parallel_cross_check_n24_e, 24, 10, 4_000);
    large_n_parallel_cross_check!(test_parallel_cross_check_n24_f, 24, 10, 5_000);
    large_n_parallel_cross_check!(test_parallel_cross_check_n24_g, 24, 10, 6_000);
    large_n_parallel_cross_check!(test_parallel_cross_check_n24_h, 24, 10, 7_000);
    large_n_parallel_cross_check!(test_parallel_cross_check_n24_i, 24, 10, 8_000);
    large_n_parallel_cross_check!(test_parallel_cross_check_n24_j, 24, 10, 9_000);

    large_n_parallel_cross_check!(test_parallel_cross_check_n28_a, 28, 5, 0);
    large_n_parallel_cross_check!(test_parallel_cross_check_n28_b, 28, 5, 1_000);
    large_n_parallel_cross_check!(test_parallel_cross_check_n28_c, 28, 5, 2_000);
    large_n_parallel_cross_check!(test_parallel_cross_check_n28_d, 28, 5, 3_000);
    large_n_parallel_cross_check!(test_parallel_cross_check_n28_e, 28, 5, 4_000);

    macro_rules! determinism_test {
        ($name:ident, $num_threads:expr) => {
            #[test]
            #[ignore = "sim: determinism test across thread counts at n=24 — slow, multi-minute runtime"]
            #[cfg(feature = "test-support")]
            fn $name() {
                const N: usize = 24;
                const SEED: u64 = 0x0525_0df5_dead_beef;
                const RUNS: usize = 10;

                let row_major = random_matrix::<3>(N, SEED);
                let mat = to_bipedal3_matrix(&row_major, N);

                let reference = permanent_bipedal3_parallel(&mat);

                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads($num_threads)
                    .build()
                    .expect("failed to build thread pool");

                for run in 0..RUNS {
                    let result = pool.install(|| permanent_bipedal3_parallel(&mat));
                    assert_eq!(
                        result, reference,
                        "determinism failure: n={N}, threads={}, run={run}",
                        $num_threads
                    );
                }
            }
        };
    }

    determinism_test!(test_determinism_threads_1, 1);
    determinism_test!(test_determinism_threads_2, 2);
    determinism_test!(test_determinism_threads_4, 4);
    determinism_test!(test_determinism_threads_8, 8);
    determinism_test!(test_determinism_threads_12, 12);
}
