//! Permanent algorithms over small prime fields.

pub mod bipedal3;
pub mod bipedal3_multiword;
pub mod compressed_rank;
pub mod exact;
pub mod rank;
pub mod reference;
pub mod ryser;

pub use bipedal3::permanent_bipedal3;
pub use bipedal3::permanent_bipedal3_batch;
pub use bipedal3::permanent_bipedal3_singleword;
pub use bipedal3_multiword::permanent_bipedal3_multiword;
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub use compressed_rank::canonical_subspaces_for_test;
pub use compressed_rank::{
    exact_permanental_rank_deficiency, CanonicalSubspace, CanonicalSubspaceDecodeError,
    CompressedRankState, CompressedStateDecodeError, CompressedTransition,
    CompressedTransitionTable, SupportedPrimeField, Vector3, CANONICAL_ENCODING_VERSION,
    CANONICAL_SUBSPACE_HASH_DOMAIN,
};
pub use exact::{
    determinant_singular_probability, enumerate_permanent_zero_probability,
    try_visit_permanent_anchor_matrices, ExactProbability,
};
pub use rank::{
    permanental_rank_status, permanental_rank_status_with_stats, PermanentalRank,
    PermanentalRankEvaluation,
};
pub use reference::permanent_mod3_reference;
pub use ryser::permanent_ryser;

/// Conservative number of Gray-code subsets assigned to one parallel worker
/// chunk. This permanent-owned declaration is available in every feature
/// layout; [`crate::tuning::CHUNK_SUBSETS`] re-exports it for tuning clients.
pub const CHUNK_SUBSETS: usize = 1 << 16;

/// Re-export of [`crate::gray`] so
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
