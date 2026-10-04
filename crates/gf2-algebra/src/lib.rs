#![deny(unsafe_code)]
#![warn(missing_docs)]
//! Packed finite-field abstractions and permanent algorithms: the
//! `PackedField<F>` trait, the per-prime packed types, and the `permanent_*`
//! algorithm family. The crate builds on [`gf2_core`] and denies unsafe code;
//! every SIMD or GPU path it dispatches through lives in `gf2-kernels-simd`
//! and `gf2-kernels-hip`.

pub mod gray;
pub mod packed;
pub mod permanent;
pub mod tuning;

#[cfg(feature = "parallel")]
pub mod parallel;

#[cfg(feature = "hip")]
pub mod gpu;

/// Test-only helpers exposed for integration tests, benchmarks, and downstream
/// crates via the `test-support` feature.
#[cfg(any(test, feature = "test-support"))]
pub mod testutil;

#[cfg(test)]
mod tests {
    #[test]
    fn test_skeleton_compiles_smoke() {}
}
