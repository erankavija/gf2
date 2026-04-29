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

/// Number of extension elements processed by one rayon task.
///
/// The chunk is large enough to amortise rayon scheduling overhead while still
/// keeping the six coefficient input lanes plus three output lanes for cubic
/// multiplication in the private-cache working set. Inner chunk arithmetic is
/// delegated to the existing SIMD Karatsuba hooks.
pub const SOA_PARALLEL_CHUNK_LEN: usize = 16 * 1024;

/// Minimum batch size that enables rayon fan-out.
///
/// Smaller batches keep the exact single-thread path to avoid losing the Tier-C
/// micro-benchmark shape to scheduling overhead.
pub const SOA_PARALLEL_MIN_LEN: usize = 2 * SOA_PARALLEL_CHUNK_LEN;

/// Returns whether a SoA batch of `len` elements should use rayon.
#[inline]
pub(crate) fn should_parallelize_soa_batch(len: usize) -> bool {
    #[cfg(feature = "parallel")]
    {
        len >= SOA_PARALLEL_MIN_LEN && rayon::current_num_threads() > 1
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
/// rayon workers. Complexity is `O(len)`.
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

    let len = lhs.len();
    let mut c0 = vec![F::zero(); len];
    let mut c1 = vec![F::zero(); len];

    use rayon::prelude::*;
    c0.par_chunks_mut(SOA_PARALLEL_CHUNK_LEN)
        .zip(c1.par_chunks_mut(SOA_PARALLEL_CHUNK_LEN))
        .enumerate()
        .for_each(|(chunk_idx, (out_c0, out_c1))| {
            let start = chunk_idx * SOA_PARALLEL_CHUNK_LEN;
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
#[cfg(feature = "parallel")]
pub(crate) fn batch_square_quadratic_parallel<F, C>(xs: &BatchExtField<F, 2>) -> BatchExtField<F, 2>
where
    F: ConstField + SimdKaratsubaHook + Send + Sync,
    C: ExtConfig<BaseField = F>,
{
    let len = xs.len();
    let mut c0 = vec![F::zero(); len];
    let mut c1 = vec![F::zero(); len];

    use rayon::prelude::*;
    c0.par_chunks_mut(SOA_PARALLEL_CHUNK_LEN)
        .zip(c1.par_chunks_mut(SOA_PARALLEL_CHUNK_LEN))
        .enumerate()
        .for_each(|(chunk_idx, (out_c0, out_c1))| {
            let start = chunk_idx * SOA_PARALLEL_CHUNK_LEN;
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
/// single-thread implementation. Complexity is `O(len)`.
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

    let len = lhs.len();
    let mut c0 = vec![F::zero(); len];
    let mut c1 = vec![F::zero(); len];
    let mut c2 = vec![F::zero(); len];

    use rayon::prelude::*;
    c0.par_chunks_mut(SOA_PARALLEL_CHUNK_LEN)
        .zip(c1.par_chunks_mut(SOA_PARALLEL_CHUNK_LEN))
        .zip(c2.par_chunks_mut(SOA_PARALLEL_CHUNK_LEN))
        .enumerate()
        .for_each(|(chunk_idx, ((out_c0, out_c1), out_c2))| {
            let start = chunk_idx * SOA_PARALLEL_CHUNK_LEN;
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
#[cfg(feature = "parallel")]
pub(crate) fn batch_square_cubic_parallel<F, C>(xs: &BatchExtField<F, 3>) -> BatchExtField<F, 3>
where
    F: ConstField + SimdKaratsubaHook + Send + Sync,
    C: ExtConfig<BaseField = F>,
{
    let len = xs.len();
    let mut c0 = vec![F::zero(); len];
    let mut c1 = vec![F::zero(); len];
    let mut c2 = vec![F::zero(); len];

    use rayon::prelude::*;
    c0.par_chunks_mut(SOA_PARALLEL_CHUNK_LEN)
        .zip(c1.par_chunks_mut(SOA_PARALLEL_CHUNK_LEN))
        .zip(c2.par_chunks_mut(SOA_PARALLEL_CHUNK_LEN))
        .enumerate()
        .for_each(|(chunk_idx, ((out_c0, out_c1), out_c2))| {
            let start = chunk_idx * SOA_PARALLEL_CHUNK_LEN;
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
