//! Compute backend abstraction for algorithm-level operations.
//!
//! This module provides high-level compute backends for matrix algorithms
//! and other computationally intensive operations. It complements the
//! lower-level `kernels` module which focuses on primitive bit operations.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │ Application (gf2-coding, user code)                         │
//! └─────────────────────────────────────────────────────────────┘
//!                             ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │ compute::ComputeBackend                                     │
//! │ - Algorithm operations (matmul, RREF, batch encode/decode) │
//! │ - Implementations: CpuBackend, GpuBackend (future)          │
//! └─────────────────────────────────────────────────────────────┘
//!                             ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │ kernels::Backend                                            │
//! │ - Primitive operations (XOR, AND, popcount)                 │
//! │ - Implementations: ScalarBackend, SimdBackend               │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Features
//!
//! - **default**: Scalar kernel backend (always available)
//! - **simd**: SIMD-accelerated kernel backend (opt-in, runtime detected)
//! - **parallel**: Rayon-based parallel execution for CPU backend (opt-in)
//! - **gpu**: GPU backend via HIP/ROCm (`gf2-kernels-hip`; future, opt-in)
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```
//! use gf2_core::{BitMatrix, compute::{ComputeBackend, CpuBackend}};
//!
//! let backend = CpuBackend::new();
//! let a = BitMatrix::identity(10);
//! let b = BitMatrix::identity(10);
//! let c = backend.matmul(&a, &b);
//! ```
//!
//! ## With Parallel Feature
//!
//! ```toml
//! [dependencies]
//! gf2-core = { version = "0.2", features = ["parallel"] }
//! ```
//!
//! ```
//! use gf2_core::{BitMatrix, compute::{ComputeBackend, CpuBackend}};
//!
//! // CpuBackend automatically uses rayon when parallel feature is enabled
//! let backend = CpuBackend::new();
//! let large_matrix = BitMatrix::identity(100);
//! let result = backend.rref(&large_matrix, false);
//! ```

pub mod backend;
pub mod cpu;
#[cfg(feature = "parallel")]
pub mod field;

/// Conservative default for `soa_batch.parallel_chunk_len()` in the active
/// [`crate::tuning::CoreTuning`]: number of extension elements processed
/// by one rayon task.
///
/// The chunk is large enough to amortise rayon scheduling overhead while still
/// keeping the six coefficient input lanes plus three output lanes for cubic
/// multiplication in the private-cache working set. Inner chunk arithmetic is
/// delegated to the existing SIMD Karatsuba hooks.
///
/// Declared here rather than in `field`, which the `parallel` feature gates,
/// so that the conservative tuning table can name it in every configuration.
/// This constant remains the compiled-in conservative default consumed by
/// [`crate::tuning::CoreTuning::CONSERVATIVE`].
pub const SOA_PARALLEL_CHUNK_LEN: usize = 16 * 1024;

/// Conservative default for `soa_batch.parallel_min_len()` in the active
/// [`crate::tuning::CoreTuning`]: minimum batch size that enables rayon
/// fan-out.
///
/// Smaller batches keep the exact single-thread path to avoid losing the Tier-C
/// micro-benchmark shape to scheduling overhead. Declared here for the same
/// reason as [`SOA_PARALLEL_CHUNK_LEN`]; this constant remains the compiled-in
/// conservative default consumed by
/// [`crate::tuning::CoreTuning::CONSERVATIVE`].
pub const SOA_PARALLEL_MIN_LEN: usize = 2 * SOA_PARALLEL_CHUNK_LEN;

#[cfg(test)]
mod batch_tests;

// Re-export main types
pub use backend::ComputeBackend;
pub use cpu::CpuBackend;
