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
//!
//! [`GeneratorMatrixOsdDecoder`] supplies the generator-row-space semantics
//! for linear block codes through the immutable [`crate::traits::SoftDecoder`]
//! surface.
//!
//! [`SyndromeOsdCorrector`] supplies the parity-check semantics: it corrects a
//! failed hard word `y` from posterior reliabilities by solving
//! `H eᵀ = H yᵀ` over the same engine.

mod engine;
mod generator;
mod patterns;
mod syndrome;

pub use engine::{
    reprocess, reprocess_segmented, reprocess_segmented_with_cancellation,
    reprocess_with_cancellation, ColumnPreference, MostReliableBasis, OsdCandidate,
    OsdComplexityMetric, OsdComplexityPolicy, OsdEngineError, OsdOutcome, OsdSegmentWork,
    OsdSegmentedOutcome, OsdSegmentedWork, OsdSemantics, OsdWork, ReprocessedColumns,
};
pub use generator::{GeneratorMatrixOsdDecoder, GeneratorMatrixOsdResult};
pub use patterns::{
    checked_candidate_bound, enumerate_patterns, OsdConfig, OsdTermination, PatternControl,
    PatternEnumerationError, PatternEnumerationReport, PatternEnumerator, PatternSegment,
    PatternSegmentEnumerationReport, PatternSegmentReport, PatternSegmentation,
};
pub use syndrome::{SyndromeOsdCorrector, SyndromeOsdError, SyndromeOsdResult};
