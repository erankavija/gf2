//! Steady-state LDPC decoder throughput harness (jit:3be770d5).
//!
//! Extends the recorded-input survey harness of `c077a88b` from whole-call,
//! single-worker cells to reused decoders on pinned worker threads. Each
//! worker owns one decoder built before timing. A timed call makes every
//! worker decode the declared per-worker batch of recorded frames, including
//! LLR conversion and extraction of the information-window decisions, so all
//! workers do identical work and a multi-worker call measures saturation
//! rather than a load-imbalance tail. The bundle contract, child framing,
//! timing windows and prepared quality come from `ldpc_survey`.

#[cfg(feature = "aff3ct")]
pub mod aff3ct;
pub mod driver;
pub mod gf2;
pub mod pool;
pub mod workload;
