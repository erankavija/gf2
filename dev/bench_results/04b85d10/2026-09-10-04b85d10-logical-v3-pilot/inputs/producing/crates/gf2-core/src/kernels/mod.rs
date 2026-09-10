//! Kernel module for runtime dispatch of optimized implementations.
//!
//! This module provides a unified interface for different execution backends:
//! - **Scalar**: Pure Rust baseline (always available)
//! - **SIMD**: AVX2/AVX-512/NEON acceleration (optional, runtime detected)
//! - **GPU**: HIP/ROCm compute via `gf2-kernels-hip` (planned for this layer)
//! - **FPGA**: Future hardware acceleration (planned)
//!
//! # Architecture
//!
//! - `Backend` trait: Defines operations all backends must implement
//! - `ops` module: High-level operations with smart dispatch
//! - `scalar` module: Pure Rust implementations
//! - Backend-specific modules: SIMD, GPU, FPGA implementations

pub mod backend;
pub mod ops;
pub mod scalar;

#[cfg(feature = "simd")]
pub mod simd;

#[cfg(test)]
pub(crate) mod test_utils;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub mod x86;

#[cfg(target_arch = "aarch64")]
pub mod aarch64;

// Re-export core types
pub use backend::{select_backend_for_size, Backend, SelectedBackend};
pub use scalar::ScalarBackend;
