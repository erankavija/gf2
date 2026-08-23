//! Rayon-backed compute helpers for batched extension-field arithmetic.
//!
//! The public field API stays on [`crate::gfpn::BatchExtField`]. This module
//! only owns the V10 execution policy: split large Structure-of-Arrays batches
//! into cache-local chunks and let each rayon worker run the same SIMD-batched
//! Karatsuba kernels used by the single-thread path.
//!
//! # Thread control
//!
//! When the `parallel` feature is enabled, rayon's global pool controls the
//! number of workers. Set `RAYON_NUM_THREADS=1`, `2`, `4`, … before the process
//! starts to reproduce strong-scaling measurements.

use crate::field::ConstField;
use crate::gfpn::{BatchExtField, ExtConfig, SimdKaratsubaHook};
use crate::tuning;

pub use super::{SOA_PARALLEL_CHUNK_LEN, SOA_PARALLEL_MIN_LEN};

/// The selected arm of the [`should_parallelize_soa_batch`] length dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SoaParallelRoute {
    /// Run the batch on the calling thread.
    Sequential,
    /// Fan the batch out across the rayon pool.
    Parallel,
}

/// Reports the length-based arm of [`should_parallelize_soa_batch`] for a
/// SoA batch of `len` elements.
///
/// The comparison uses the active `soa_batch.parallel_min_len()` profile
/// value. It does not account for rayon thread availability;
/// [`should_parallelize_soa_batch`] applies that separately.
#[must_use]
pub fn soa_parallel_route(len: usize) -> SoaParallelRoute {
    soa_parallel_route_resolved(tuning::active().soa_batch().parallel_min_len(), len)
}

/// Reports the [`soa_parallel_route`] arm against an already-resolved
/// `parallel_min_len`.
fn soa_parallel_route_resolved(parallel_min_len: usize, len: usize) -> SoaParallelRoute {
    if len >= parallel_min_len {
        SoaParallelRoute::Parallel
    } else {
        SoaParallelRoute::Sequential
    }
}

/// Returns whether a SoA batch of `len` elements should use rayon.
///
/// Uses the active `soa_batch.parallel_min_len()` profile value via
/// [`soa_parallel_route`].
#[inline]
pub(crate) fn should_parallelize_soa_batch(len: usize) -> bool {
    #[cfg(feature = "parallel")]
    {
        soa_parallel_route(len) == SoaParallelRoute::Parallel && rayon::current_num_threads() > 1
    }

    #[cfg(not(feature = "parallel"))]
    {
        let _ = len;
        false
    }
}

/// Parallel Karatsuba multiplication for quadratic SoA batches.
///
/// This is bit-exact with [`BatchExtField::batch_mul_quadratic`]; it only
/// changes the schedule by assigning contiguous coefficient-lane chunks to
/// rayon workers. The chunk length is the active `soa_batch.parallel_chunk_len()`
/// profile value, resolved once for this call. Complexity is `O(len)`.
#[cfg(feature = "parallel")]
pub(crate) fn batch_mul_quadratic_parallel<F, C>(
    lhs: &BatchExtField<F, 2>,
    rhs: &BatchExtField<F, 2>,
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

    let chunk_len = tuning::active().soa_batch().parallel_chunk_len();
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

/// Parallel Karatsuba squaring for quadratic SoA batches.
///
/// The chunk length is the active `soa_batch.parallel_chunk_len()` profile
/// value, resolved once for this call.
#[cfg(feature = "parallel")]
pub(crate) fn batch_square_quadratic_parallel<F, C>(xs: &BatchExtField<F, 2>) -> BatchExtField<F, 2>
where
    F: ConstField + SimdKaratsubaHook + Send + Sync,
    C: ExtConfig<BaseField = F>,
{
    let chunk_len = tuning::active().soa_batch().parallel_chunk_len();
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

/// Parallel Karatsuba-3 multiplication for cubic SoA batches.
///
/// Each rayon task receives contiguous SoA slices and then dispatches through
/// the same fused SIMD hook (or scalar straight-line fallback) as the
/// single-thread implementation. The chunk length is the active
/// `soa_batch.parallel_chunk_len()` profile value, resolved once for this
/// call. Complexity is `O(len)`.
#[cfg(feature = "parallel")]
pub(crate) fn batch_mul_cubic_parallel<F, C>(
    lhs: &BatchExtField<F, 3>,
    rhs: &BatchExtField<F, 3>,
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

    let chunk_len = tuning::active().soa_batch().parallel_chunk_len();
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

/// Parallel Karatsuba-3 squaring for cubic SoA batches.
///
/// The chunk length is the active `soa_batch.parallel_chunk_len()` profile
/// value, resolved once for this call.
#[cfg(feature = "parallel")]
pub(crate) fn batch_square_cubic_parallel<F, C>(xs: &BatchExtField<F, 3>) -> BatchExtField<F, 3>
where
    F: ConstField + SimdKaratsubaHook + Send + Sync,
    C: ExtConfig<BaseField = F>,
{
    let chunk_len = tuning::active().soa_batch().parallel_chunk_len();
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
