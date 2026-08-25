//! Ordered-statistics decoding building blocks.
//!
//! The OSD module owns Hamming-weight reprocessing semantics.  In particular,
//! its pattern source is deliberately separate from the logistic-weight
//! iterator used by [`crate::grand`].
//!
//! [`MostReliableBasis`] and [`reprocess`] are the shared engine: they turn a
//! canonical reliability order plus an ordered GF(2) elimination into a
//! bounded, soft-ranked candidate search.  What a candidate means stays with
//! the caller's [`OsdSemantics`] adapter.

mod engine;
mod patterns;

pub use engine::{
    reprocess, reprocess_with_cancellation, ColumnPreference, MostReliableBasis, OsdCandidate,
    OsdEngineError, OsdOutcome, OsdSemantics, OsdWork, ReprocessedColumns,
};
pub use patterns::{
    checked_candidate_bound, enumerate_patterns, OsdConfig, OsdTermination, PatternControl,
    PatternEnumerationError, PatternEnumerationReport, PatternEnumerator,
};
