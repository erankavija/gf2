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
//! # Contents
//!
//! - [`packed::PackedField`] / [`packed::PackedFieldVec`] traits with the
//!   [`packed::ScalarPackedFp3`] / [`packed::ScalarPackedFp3Vec`] scalar
//!   reference impls.
//! - [`packed::Bipedal3`], a 64-lane packed `F_3` element (bitwise Scheinerman
//!   2024 formulas); [`packed::Bipedal3Vec`], a variable-length vector of two
//!   parallel `Vec<u64>` with the mask-tail invariant; and
//!   [`packed::Bipedal3Matrix`], a column-major `rows × cols` matrix of
//!   `Bipedal3Vec` columns.
//! - [`gray::gray_code_iter`].
//! - [`permanent::ryser::permanent_ryser`], the generic Ryser-formula permanent
//!   over any `FiniteField` and the correctness oracle for every packed
//!   permanent kernel.
//! - [`permanent::reference::permanent_mod3_reference`], a Rust port of
//!   Scheinerman 2024 Algorithm 1 / Listing 1, the speedup denominator and fast
//!   oracle for large-n cross-checks.
//! - [`permanent::bipedal3::permanent_bipedal3`], which selects the scalar
//!   single-word `n ≤ 63` fast path with a bipedal-multiplication-tree
//!   horizontal fold, then the multi-word path for larger matrices; direct
//!   single-matrix AVX2 and four-matrix AVX2 batch entry points are separate.
//!   `permanent_bipedal5` and `permanent_bipedal7` are the F_5/F_7 single-word
//!   analogues.
//! - [`permanent::rank::permanental_rank_status`] decides permanental rank
//!   deficiency for a rectangular `n × k` matrix (`k ≤ n`) as the conjunction
//!   over its `k × k` row submatrices. It adds no numeric kernel — it
//!   enumerates row subsets and calls `permanent_ryser` on each, exiting at
//!   the first nonzero permanent — and it decides one matrix without touching
//!   sampling or statistics.
//!
//! The type → crate map is `@/issue/6e20133d` §2.
//!
//! # Module map (D1a §2)
//!
//! | Module       | Purpose                                                                           |
//! |--------------|-----------------------------------------------------------------------------------|
//! | [`packed`]   | `PackedField` / `PackedFieldVec` traits and `Bipedal3` (F_3) / `Packed5` (F_5) / `Packed7` (F_7) impls. |
//! | [`permanent`]| Square permanent algorithms, the rectangular `permanental_rank_status` predicate, exact small-order anchors, and canonical compressed-state propagation for three-column deficiency over F_3, F_5, and F_7. |
//! | [`gray`]     | Gray-code subset enumerator used by Ryser's formula and the bipedal kernels.      |
//! | [`tuning`]   | Algebra-owned permanent selectors and process-wide typed access.                  |
//! | `parallel`   | Rayon-based work-stealing dispatch (cfg `feature = "parallel"`, default on).      |
//! | `gpu`        | HIP/ROCm host-side dispatcher (cfg `feature = "hip"`, default off).               |
//!
//! # Features
//!
//! The crate manifest is the feature catalogue. The compatibility sweep in
//! `crates/gf2-algebra/scripts/check-feature-matrix.sh` covers the established 64-cell
//! arithmetic/backend matrix plus the non-default `tuning-profile` codec
//! overlay in profile-only, crate-default, and complete host-supported
//! configurations. Typed algebra selectors and active access are always built.
//!
//! # See also
//!
//! - Epic design: `@/issue/ae82bd73`.
//! - Crate boundary decision:
//!   `@/issue/6e20133d`.
//! - Trait surface decision:
//!   `@/issue/9fe275d3`.
//! - Feature-gate matrix decision:
//!   `@/issue/4fced99b`.

pub mod gray;
pub mod packed;
pub mod permanent;
pub mod tuning;

#[cfg(feature = "parallel")]
pub mod parallel;

#[cfg(feature = "hip")]
pub mod gpu;

/// Test-only helpers exposed for integration tests, benchmarks, and downstream
/// crates via the `test-support` feature. The module is also compiled under
/// `cfg(test)` for internal unit tests in this crate; the dual gate mirrors the
/// `gf2-core::test-support` workspace pattern.
#[cfg(any(test, feature = "test-support"))]
pub mod testutil;

#[cfg(test)]
mod tests {
    /// Verifies the crate compiles and links into a test binary.
    #[test]
    fn test_skeleton_compiles_smoke() {}
}
