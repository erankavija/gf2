//! Algorithm-level compute backends over GF(2) matrices: [`ComputeBackend`],
//! implemented by [`CpuBackend`] on the word primitives of [`crate::kernels`].

pub mod backend;
pub mod cpu;
#[cfg(feature = "parallel")]
pub mod field;

/// Value of `soa_batch.parallel_chunk_len()` in
/// [`crate::tuning::CoreTuning::CONSERVATIVE`]: extension elements per rayon
/// task.
///
/// Declared outside `field`, which the `parallel` feature gates, so that the
/// conservative table names it in every configuration.
pub const SOA_PARALLEL_CHUNK_LEN: usize = 16 * 1024;

/// Value of `soa_batch.parallel_min_len()` in
/// [`crate::tuning::CoreTuning::CONSERVATIVE`]: minimum batch length that
/// fans out across rayon tasks.
pub const SOA_PARALLEL_MIN_LEN: usize = 2 * SOA_PARALLEL_CHUNK_LEN;

#[cfg(test)]
mod batch_tests;

pub use backend::ComputeBackend;
pub use cpu::CpuBackend;
