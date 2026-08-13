//! Permanent algorithms over small prime fields.
//!
//! Hosts the `Permanent` trait, the generic `permanent_ryser<F>` driver,
//! the `permanent_mod3_reference` cross-check, the per-prime
//! `permanent_bipedal{3,5,7}` fast paths, and the rectangular
//! [`permanental_rank_status`] predicate that decides permanental rank
//! deficiency by conjunction over row submatrices. See the epic design at
//! `dev/plans/ae82bd73-gf2-algebra-permanent/gf2_algebra_permanent.md` §6 / §7.3 / §9 for the algorithm
//! family, and `dev/plans/9fe275d3/d1b_packed_field_api.md` for the trait surface
//! frozen at W6.
//!
//! # Status
//!
//! W2 complete — T7 (Ryser driver), T8 (mod-3 reference port), and T9
//! (bipedal3 single-word fast path) all landed in W2. W4 F_5/F_7
//! analogues — [`permanent_bipedal5`] and [`permanent_bipedal7`] —
//! landed in W4-T18/T20 (single-word path; F_5 covers `n ≤ Packed5::LANES = 64`,
//! F_7 covers `n ≤ Packed7::LANES = 16`).
//!
//! The square surface is joined by [`rank`], whose
//! [`permanental_rank_status`] decides `per-rank(A) < k` for a rectangular
//! `n × k` matrix. It adds no numeric kernel: it enumerates row subsets and
//! calls [`permanent_ryser`] on each `k × k` submatrix.
//!
//! # Re-exports
//!
//! [`gray`] is re-exported from [`crate::gray`] so callers can use the
//! permanent-grouped path `gf2_algebra::permanent::gray::gray_code_iter`
//! that the W1-T6 contract names, while the underlying module also
//! remains reachable as `gf2_algebra::gray` per
//! `dev/plans/6e20133d/d1a_gf2_algebra_boundary.md` §4.2.

pub mod bipedal3;
pub mod bipedal3_multiword;
pub mod exact;
pub mod rank;
pub mod reference;
pub mod ryser;

pub use bipedal3::permanent_bipedal3;
pub use bipedal3::permanent_bipedal3_batch;
pub use bipedal3::permanent_bipedal3_singleword;
pub use bipedal3_multiword::permanent_bipedal3_multiword;
pub use exact::{enumerate_permanent_zero_probability, ExactProbability};
pub use rank::{
    permanental_rank_status, permanental_rank_status_with_stats, PermanentalRank,
    PermanentalRankEvaluation,
};
pub use reference::permanent_mod3_reference;
pub use ryser::permanent_ryser;

/// Re-export of [`crate::gray`] so the canonical W1-T6 API
/// `gf2_algebra::permanent::gray::gray_code_iter` resolves.
pub use crate::gray;

#[cfg(feature = "parallel")]
pub mod parallel_bipedal3;

#[cfg(feature = "parallel")]
pub use parallel_bipedal3::permanent_bipedal3_parallel;

#[cfg(feature = "f5")]
pub mod bipedal5;

#[cfg(feature = "f5")]
pub use bipedal5::permanent_bipedal5;

#[cfg(feature = "f7")]
pub mod bipedal7;

#[cfg(feature = "f7")]
pub use bipedal7::permanent_bipedal7;
