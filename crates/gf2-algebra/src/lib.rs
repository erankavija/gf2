#![deny(unsafe_code)]
#![warn(missing_docs)]
//! Packed finite-field abstractions and permanent algorithms.
//!
//! `gf2-algebra` is the workspace home for the `PackedField<F>` trait, the
//! per-prime packed types (`Bipedal3` for F_3, `Packed5` for F_5, `Packed7` for
//! F_7), and the `permanent_*` algorithm family. It sits on top of [`gf2_core`]
//! (for `FiniteField`, `Fp<P>`, `BitVec`) and stays `#![deny(unsafe_code)]` —
//! every SIMD or GPU path it dispatches through lives in the dedicated
//! `gf2-kernels-simd` and `gf2-kernels-hip` crates
//! (`@/inv/unsafe-kernel-isolation`).
//!
//! # Features
//!
//! The crate manifest is the feature catalogue: `parallel` (default on) gates
//! the `parallel` module and `hip` (default off) gates the `gpu` module.
//! `crates/gf2-algebra/scripts/check-feature-matrix.sh` checks the feature
//! combinations, including the non-default `tuning-profile` codec overlay.
//! Typed algebra selectors and active access are always built.

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
    /// Verifies the crate compiles and links into a test binary.
    #[test]
    fn test_skeleton_compiles_smoke() {}
}
