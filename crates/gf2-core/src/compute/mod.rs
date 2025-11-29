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
//! - **gpu**: GPU backend via Vulkan (future, opt-in)
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

#[cfg(test)]
mod batch_tests;

// Re-export main types
pub use backend::ComputeBackend;
pub use cpu::CpuBackend;
