#![deny(unsafe_code)]
#![warn(missing_docs)]
//! Packed finite-field abstractions and permanent algorithms.
//!
//! `gf2-algebra` is the workspace home for the `PackedField<F>` trait,
//! the `Bipedal{3,5,7}` packed types, and the `permanent_*` algorithm
//! family that the **gf2-algebra-permanent** epic introduces. It sits on
//! top of [`gf2_core`] (for `FiniteField`, `Fp<P>`, `BitVec`) and stays
//! `#![deny(unsafe_code)]` — every SIMD or GPU path it dispatches through
//! lives in the dedicated `gf2-kernels-simd` and `gf2-kernels-hip`
//! crates, in keeping with the project's unsafe-isolation invariant
//! (CLAUDE.md §Architecture, point 3).
//!
//! # Status
//!
//! This crate is the W1-T1 skeleton. The module tree exists but the
//! public types listed in
//! [`dev/plans/d1a_gf2_algebra_boundary.md`](../../../dev/plans/d1a_gf2_algebra_boundary.md)
//! §2 are landed by the W1 (T2-T6) and later W3-W5 issues. Building this
//! crate at the skeleton stage produces a library with no functional
//! surface; doc-tests and unit tests are added by downstream issues as
//! the modules fill in.
//!
//! # Module map (D1a §2)
//!
//! | Module       | Purpose                                                                           |
//! |--------------|-----------------------------------------------------------------------------------|
//! | [`packed`]   | `PackedField` / `PackedFieldVec` traits and `Bipedal{3,5,7}` impls.               |
//! | [`permanent`]| `Permanent` trait, `permanent_ryser`, and per-prime `permanent_bipedal*` family.  |
//! | [`gray`]     | Gray-code subset enumerator used by Ryser's formula and the bipedal kernels.      |
//! | `parallel`   | Rayon-based work-stealing dispatch (cfg `feature = "parallel"`, default on).      |
//! | `gpu`        | HIP/ROCm host-side dispatcher (cfg `feature = "hip"`, default off).               |
//!
//! # Features
//!
//! See [`dev/plans/d1c_feature_matrix.md`](../../../dev/plans/d1c_feature_matrix.md)
//! for the authoritative feature catalogue and the 64-cell compatibility
//! matrix. Defaults are `["simd", "parallel"]`.
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
