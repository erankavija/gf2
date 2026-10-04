//! Rayon execution of [`crate::gfpn::BatchExtField`] products: a
//! Structure-of-Arrays batch is split into chunks and each rayon task runs
//! the kernels of the single-thread path on one chunk.

use crate::field::ConstField;
use crate::gfpn::{BatchExtField, ExtConfig, SimdKaratsubaHook};
use crate::tuning;

pub use super::{SOA_PARALLEL_CHUNK_LEN, SOA_PARALLEL_MIN_LEN};

/// Chunk length last used by a parallel batch product; zero when none ran
/// since [`reset_last_effective_soa_chunk`].
/// [`crate::tuning::SoaBatchSelectors::parallel_chunk_len`] is at least 1, so
/// zero is never a recorded value.
#[cfg(any(test, feature = "test-support"))]
static LAST_EFFECTIVE_SOA_CHUNK: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Returns the chunk length the most recent parallel batch product received,
/// or `None` if none ran since [`reset_last_effective_soa_chunk`].
#[cfg(any(test, feature = "test-support"))]
#[must_use]
pub fn last_effective_soa_chunk() -> Option<usize> {
    match LAST_EFFECTIVE_SOA_CHUNK.load(std::sync::atomic::Ordering::SeqCst) {
        0 => None,
        chunk_len => Some(chunk_len),
    }
}

/// Runs `f` inside a dedicated rayon pool of `threads` workers, so that
/// `rayon::current_num_threads` inside `f` reports `threads`.
///
/// # Panics
///
/// Panics if the pool cannot be built.
#[cfg(all(feature = "parallel", any(test, feature = "test-support")))]
pub fn run_in_dedicated_parallel_pool<R: Send>(threads: usize, f: impl FnOnce() -> R + Send) -> R {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("dedicated test pool builds")
        .install(f)
}

/// Clears the value [`last_effective_soa_chunk`] reports.
#[cfg(any(test, feature = "test-support"))]
pub fn reset_last_effective_soa_chunk() {
    LAST_EFFECTIVE_SOA_CHUNK.store(0, std::sync::atomic::Ordering::SeqCst);
}

/// Arm that [`soa_parallel_route`] selects for a batch length.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SoaParallelRoute {
    /// Run the batch on the calling thread.
    Sequential,
    /// Fan the batch out across the rayon pool.
    Parallel,
}

/// Compares `len` with the active `soa_batch.parallel_min_len()` profile
/// value. A [`SoaParallelRoute::Parallel`] batch still runs on the calling
/// thread when the rayon pool has one worker.
#[must_use]
pub fn soa_parallel_route(len: usize) -> SoaParallelRoute {
    soa_parallel_route_resolved(tuning::active().soa_batch().parallel_min_len(), len)
}

fn soa_parallel_route_resolved(parallel_min_len: usize, len: usize) -> SoaParallelRoute {
    if len >= parallel_min_len {
        SoaParallelRoute::Parallel
    } else {
        SoaParallelRoute::Sequential
    }
}

/// Returns whether a SoA batch of `len` elements fans out: `len` reaches
/// `parallel_min_len` and the rayon pool has more than one worker.
#[cfg(feature = "parallel")]
#[inline]
pub(crate) fn should_parallelize_soa_batch_resolved(parallel_min_len: usize, len: usize) -> bool {
    soa_parallel_route_resolved(parallel_min_len, len) == SoaParallelRoute::Parallel
        && rayon::current_num_threads() > 1
}

/// [`BatchExtField::batch_mul_quadratic`] with `chunk_len` elements per rayon
/// task.
#[cfg(feature = "parallel")]
pub(crate) fn batch_mul_quadratic_parallel<F, C>(
    lhs: &BatchExtField<F, 2>,
    rhs: &BatchExtField<F, 2>,
    chunk_len: usize,
) -> BatchExtField<F, 2>
where
    F: ConstField + SimdKaratsubaHook + Send + Sync,
    C: ExtConfig<BaseField = F>,
{
    assert_eq!(
        lhs.len(),
        rhs.len(),
        "batch_mul_quadratic_parallel: length mismatch ({} vs {})",
        lhs.len(),
        rhs.len()
    );

    #[cfg(any(test, feature = "test-support"))]
    LAST_EFFECTIVE_SOA_CHUNK.store(chunk_len, std::sync::atomic::Ordering::SeqCst);
    let len = lhs.len();
    let mut c0 = vec![F::zero(); len];
    let mut c1 = vec![F::zero(); len];

    use rayon::prelude::*;
    c0.par_chunks_mut(chunk_len)
        .zip(c1.par_chunks_mut(chunk_len))
        .enumerate()
        .for_each(|(chunk_idx, (out_c0, out_c1))| {
            let start = chunk_idx * chunk_len;
            let end = start + out_c0.len();
            quadratic_chunk::<F, C>(
                &lhs.coeff(0)[start..end],
                &lhs.coeff(1)[start..end],
                &rhs.coeff(0)[start..end],
                &rhs.coeff(1)[start..end],
                out_c0,
                out_c1,
            );
        });

    BatchExtField::new([c0, c1])
}

/// Quadratic batch squaring with `chunk_len` elements per rayon task.
#[cfg(feature = "parallel")]
pub(crate) fn batch_square_quadratic_parallel<F, C>(
    xs: &BatchExtField<F, 2>,
    chunk_len: usize,
) -> BatchExtField<F, 2>
where
    F: ConstField + SimdKaratsubaHook + Send + Sync,
    C: ExtConfig<BaseField = F>,
{
    #[cfg(any(test, feature = "test-support"))]
    LAST_EFFECTIVE_SOA_CHUNK.store(chunk_len, std::sync::atomic::Ordering::SeqCst);
    let len = xs.len();
    let mut c0 = vec![F::zero(); len];
    let mut c1 = vec![F::zero(); len];

    use rayon::prelude::*;
    c0.par_chunks_mut(chunk_len)
        .zip(c1.par_chunks_mut(chunk_len))
        .enumerate()
        .for_each(|(chunk_idx, (out_c0, out_c1))| {
            let start = chunk_idx * chunk_len;
            let end = start + out_c0.len();
            quadratic_chunk::<F, C>(
                &xs.coeff(0)[start..end],
                &xs.coeff(1)[start..end],
                &xs.coeff(0)[start..end],
                &xs.coeff(1)[start..end],
                out_c0,
                out_c1,
            );
        });

    BatchExtField::new([c0, c1])
}

#[cfg(feature = "parallel")]
fn quadratic_chunk<F, C>(a0: &[F], a1: &[F], b0: &[F], b1: &[F], out_c0: &mut [F], out_c1: &mut [F])
where
    F: ConstField + SimdKaratsubaHook,
    C: ExtConfig<BaseField = F>,
{
    debug_assert_eq!(a0.len(), out_c0.len());
    debug_assert_eq!(a0.len(), out_c1.len());
    if let Some((c0, c1)) = F::try_simd_karatsuba::<C>(a0, a1, b0, b1) {
        out_c0.copy_from_slice(&c0);
        out_c1.copy_from_slice(&c1);
        return;
    }

    for i in 0..a0.len() {
        let sum_a = a0[i] + a1[i];
        let sum_b = b0[i] + b1[i];
        let v0 = a0[i] * b0[i];
        let v1 = a1[i] * b1[i];
        let cross = sum_a * sum_b;
        out_c0[i] = v0 + C::mul_by_non_residue(v1);
        out_c1[i] = cross - v0 - v1;
    }
}

/// Cubic batch multiplication with `chunk_len` elements per rayon task.
#[cfg(feature = "parallel")]
pub(crate) fn batch_mul_cubic_parallel<F, C>(
    lhs: &BatchExtField<F, 3>,
    rhs: &BatchExtField<F, 3>,
    chunk_len: usize,
) -> BatchExtField<F, 3>
where
    F: ConstField + SimdKaratsubaHook + Send + Sync,
    C: ExtConfig<BaseField = F>,
{
    assert_eq!(
        lhs.len(),
        rhs.len(),
        "batch_mul_cubic_parallel: length mismatch ({} vs {})",
        lhs.len(),
        rhs.len()
    );

    #[cfg(any(test, feature = "test-support"))]
    LAST_EFFECTIVE_SOA_CHUNK.store(chunk_len, std::sync::atomic::Ordering::SeqCst);
    let len = lhs.len();
    let mut c0 = vec![F::zero(); len];
    let mut c1 = vec![F::zero(); len];
    let mut c2 = vec![F::zero(); len];

    use rayon::prelude::*;
    c0.par_chunks_mut(chunk_len)
        .zip(c1.par_chunks_mut(chunk_len))
        .zip(c2.par_chunks_mut(chunk_len))
        .enumerate()
        .for_each(|(chunk_idx, ((out_c0, out_c1), out_c2))| {
            let start = chunk_idx * chunk_len;
            let end = start + out_c0.len();
            cubic_chunk::<F, C>(
                &lhs.coeff(0)[start..end],
                &lhs.coeff(1)[start..end],
                &lhs.coeff(2)[start..end],
                &rhs.coeff(0)[start..end],
                &rhs.coeff(1)[start..end],
                &rhs.coeff(2)[start..end],
                out_c0,
                out_c1,
                out_c2,
            );
        });

    BatchExtField::new([c0, c1, c2])
}

/// Cubic batch squaring with `chunk_len` elements per rayon task.
#[cfg(feature = "parallel")]
pub(crate) fn batch_square_cubic_parallel<F, C>(
    xs: &BatchExtField<F, 3>,
    chunk_len: usize,
) -> BatchExtField<F, 3>
where
    F: ConstField + SimdKaratsubaHook + Send + Sync,
    C: ExtConfig<BaseField = F>,
{
    #[cfg(any(test, feature = "test-support"))]
    LAST_EFFECTIVE_SOA_CHUNK.store(chunk_len, std::sync::atomic::Ordering::SeqCst);
    let len = xs.len();
    let mut c0 = vec![F::zero(); len];
    let mut c1 = vec![F::zero(); len];
    let mut c2 = vec![F::zero(); len];

    use rayon::prelude::*;
    c0.par_chunks_mut(chunk_len)
        .zip(c1.par_chunks_mut(chunk_len))
        .zip(c2.par_chunks_mut(chunk_len))
        .enumerate()
        .for_each(|(chunk_idx, ((out_c0, out_c1), out_c2))| {
            let start = chunk_idx * chunk_len;
            let end = start + out_c0.len();
            cubic_chunk::<F, C>(
                &xs.coeff(0)[start..end],
                &xs.coeff(1)[start..end],
                &xs.coeff(2)[start..end],
                &xs.coeff(0)[start..end],
                &xs.coeff(1)[start..end],
                &xs.coeff(2)[start..end],
                out_c0,
                out_c1,
                out_c2,
            );
        });

    BatchExtField::new([c0, c1, c2])
}

#[cfg(feature = "parallel")]
#[allow(clippy::too_many_arguments)]
fn cubic_chunk<F, C>(
    a0: &[F],
    a1: &[F],
    a2: &[F],
    b0: &[F],
    b1: &[F],
    b2: &[F],
    out_c0: &mut [F],
    out_c1: &mut [F],
    out_c2: &mut [F],
) where
    F: ConstField + SimdKaratsubaHook,
    C: ExtConfig<BaseField = F>,
{
    debug_assert_eq!(a0.len(), out_c0.len());
    debug_assert_eq!(a0.len(), out_c1.len());
    debug_assert_eq!(a0.len(), out_c2.len());
    if let Some([c0, c1, c2]) = F::try_simd_cubic_karatsuba::<C>(a0, a1, a2, b0, b1, b2) {
        out_c0.copy_from_slice(&c0);
        out_c1.copy_from_slice(&c1);
        out_c2.copy_from_slice(&c2);
        return;
    }

    for i in 0..a0.len() {
        let v0 = a0[i] * b0[i];
        let v1 = a1[i] * b1[i];
        let v2 = a2[i] * b2[i];

        let cross12 = (a1[i] + a2[i]) * (b1[i] + b2[i]);
        let x = cross12 - v1 - v2;

        let cross01 = (a0[i] + a1[i]) * (b0[i] + b1[i]);
        let y = cross01 - v0 - v1;

        let cross02 = (a0[i] + a2[i]) * (b0[i] + b2[i]);
        let z = cross02 - v0 + v1 - v2;

        out_c0[i] = v0 + C::mul_by_non_residue(x);
        out_c1[i] = y + C::mul_by_non_residue(v2);
        out_c2[i] = z;
    }
}
