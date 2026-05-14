#![deny(unsafe_code)]
#![warn(missing_docs)]
//! Packed finite-field abstractions and permanent algorithms.
//!
//! `gf2-algebra` is the workspace home for the `PackedField<F>` trait,
//! the per-prime packed types (`Bipedal3` for F_3, `Packed5` for F_5,
//! `Packed7` for F_7), and the `permanent_*` algorithm family that the
//! **gf2-algebra-permanent** epic introduces. It sits on
//! top of [`gf2_core`] (for `FiniteField`, `Fp<P>`, `BitVec`) and stays
//! `#![deny(unsafe_code)]` — every SIMD or GPU path it dispatches through
//! lives in the dedicated `gf2-kernels-simd` and `gf2-kernels-hip`
//! crates, in keeping with the project's unsafe-isolation invariant
//! (CLAUDE.md §Architecture, point 3).
//!
//! # Status
//!
//! W2 complete. T2/T3/T4/T5/T6/T7/T8/T9 all landed:
//!
//! - [`packed::PackedField`] / [`packed::PackedFieldVec`] traits and the
//!   [`packed::ScalarPackedFp3`] / [`packed::ScalarPackedFp3Vec`] scalar
//!   reference impls are landed (W1-T2/T3).
//! - [`packed::Bipedal3`] fixed-width packed `F_3` element (64 lanes, bitwise
//!   Scheinerman 2024 formulas) is landed; cross-checked via proptest (1000
//!   cases) against [`packed::ScalarPackedFp3`] (W1-T3).
//! - [`packed::Bipedal3Vec`] variable-length packed `F_3` vector (two parallel
//!   `Vec<u64>` with mask-tail invariant) is landed; cross-checked via proptest
//!   (200 cases) against [`packed::ScalarPackedFp3Vec`]. Includes `fold_mul`
//!   inherent method (W1-T4 deliverable).
//! - [`packed::Bipedal3Matrix`] rectangular `rows × cols` column-major matrix
//!   (`Vec<Bipedal3Vec>`, one per column) is landed; includes `from_row_major`,
//!   `to_row_major`, `column`, `row`, `get`, and `transpose`. Covered by
//!   unit tests (word-boundary shapes) and proptest (100 random shapes,
//!   double-transpose roundtrip) (W1-T5 deliverable).
//! - [`gray::gray_code_iter`] is landed (W1-T6).
//! - [`permanent::ryser::permanent_ryser`] is landed (W2-T7); the generic
//!   Ryser-formula permanent over any `FiniteField`, used as the
//!   correctness oracle for every packed permanent kernel.
//! - [`permanent::reference::permanent_mod3_reference`] is landed (W2-T8);
//!   faithful Rust port of Scheinerman 2024 Algorithm 1 / Listing 1, serving
//!   as the 50× speedup denominator and fast oracle for large-n cross-checks.
//! - [`permanent::bipedal3::permanent_bipedal3`] is landed (W2-T9);
//!   single-word `n ≤ 63` fast path with bipedal-multiplication-tree
//!   horizontal fold. Per-n cross-checks: 1000 matrices for each `n ∈ 1..=12`
//!   (default tier) and `n ∈ 13..=16` (slow tier); 100 matrices for `n ∈
//!   {20, 24}` (slow tier, split sub-tests). F_5/F_7 single-word analogues —
//!   `permanent_bipedal5` and `permanent_bipedal7` — landed in W4-T18/T20.
//!
//! The full type → crate map this crate satisfies on completion is in
//! [`dev/plans/d1a_gf2_algebra_boundary.md`](../../../dev/plans/d1a_gf2_algebra_boundary.md)
//! §2.
//!
//! # Module map (D1a §2)
//!
//! | Module       | Purpose                                                                           |
//! |--------------|-----------------------------------------------------------------------------------|
//! | [`packed`]   | `PackedField` / `PackedFieldVec` traits and `Bipedal3` (F_3) / `Packed5` (F_5) / `Packed7` (F_7) impls. |
//! | [`permanent`]| `Permanent` trait, `permanent_ryser`, and per-prime `permanent_bipedal*` family.  |
//! | [`gray`]     | Gray-code subset enumerator used by Ryser's formula and the bipedal kernels.      |
//! | `parallel`   | Rayon-based work-stealing dispatch (cfg `feature = "parallel"`, default on).      |
//! | `gpu`        | HIP/ROCm host-side dispatcher (cfg `feature = "hip"`, default off).               |
//!
//! # Features
//!
//! See [`dev/plans/d1c_feature_matrix.md`](../../../dev/plans/d1c_feature_matrix.md)
//! for the authoritative feature catalogue and the 64-cell compatibility
//! matrix. Defaults are `["simd", "parallel", "f5", "f7"]`; `f5` and
//! `f7` were flipped default-on as the W4 closing edit after the
//! per-prime encodings landed in `packed5` / `packed7`.
//!
//! # See also
//!
//! - Epic design: `dev/plans/gf2_algebra_permanent.md`.
//! - Crate boundary decision: `dev/plans/d1a_gf2_algebra_boundary.md`.
//! - Trait surface decision: `dev/plans/d1b_packed_field_api.md`.
//! - Feature-gate matrix decision: `dev/plans/d1c_feature_matrix.md`.

pub mod gray;
pub mod packed;
pub mod permanent;

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
    //! W1-T1 skeleton smoke test. Exists only so
    //! `cargo nextest run -p gf2-algebra --profile ci` finds at least
    //! one test binary and exits zero before W1 (T2-T6) lands real
    //! coverage. Replace / extend in those issues; do not delete.

    /// Verifies the crate compiles and links into a test binary.
    ///
    /// This is a placeholder; the trait + algorithm coverage is added
    /// by T2-T6 of the W1 wave per `dev/plans/d1a_gf2_algebra_boundary.md`
    /// §5 validation checklist.
    #[test]
    fn test_skeleton_compiles_smoke() {
        // Intentionally empty: presence of this `#[test]` is sufficient
        // for the criterion-4 nextest invocation to pass with no tests
        // disabled. The real packed-field / permanent test surface lands
        // in W1 implementation issues.
    }
}
