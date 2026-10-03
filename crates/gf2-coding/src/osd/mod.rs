//! Ordered-statistics decoding building blocks.
//!
//! [`MostReliableBasis`] and [`reprocess`] are the shared engine: they turn a
//! canonical reliability order plus an ordered GF(2) elimination into a
//! bounded, soft-ranked search over Hamming-weight patterns.  What a candidate
//! means stays with an [`OsdSemantics`] adapter.  [`GeneratorMatrixOsdDecoder`]
//! searches a generator row space, [`SyndromeOsdCorrector`] a parity-check
//! coset, and [`BpOsdDecoder`] runs BP first and falls back to the corrector.

mod bp_osd;
mod engine;
mod generator;
mod patterns;
mod syndrome;

pub use bp_osd::{BpOsdDecodeError, BpOsdDecoder, BpOsdResult, BpOsdStage, BpOsdTermination};
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
