//! Ordered-statistics decoding building blocks.
//!
//! The OSD module owns Hamming-weight reprocessing semantics.  In particular,
//! its pattern source is deliberately separate from the logistic-weight
//! iterator used by [`crate::grand`].

mod patterns;

pub use patterns::{
    checked_candidate_bound, enumerate_patterns, OsdConfig, OsdTermination, PatternControl,
    PatternEnumerationError, PatternEnumerationReport, PatternEnumerator,
};
