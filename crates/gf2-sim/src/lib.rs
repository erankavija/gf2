//! `gf2-sim` — research-grade CPU+GPU FEC simulation pipeline.
//!
//! This crate provides the [`Pipeline`] / [`Stage`] / [`Connector`] primitives
//! that compose error-correcting-code building blocks from `gf2-coding` into a
//! parallel, optionally GPU-accelerated, deterministic simulation harness. It
//! is the v2 successor to the simulation machinery in
//! `gf2_coding::simulation`.
//!
//! # Module map
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`pipeline`] | [`Pipeline`] and its batch-submission API |
//! | [`stage`] | [`Stage`], [`AnyStage`], [`ErasedStage`], [`erase`], [`TypedBatch`], [`AnyScratch`] |
//! | [`batch`] | concrete batch types: [`BitPackedBatch`], [`SymbolBatch`], [`LlrBatch`], [`HardDecisionBatch`] |
//! | [`stages`] | DVB-T2 codec+modem [`Stage`] wrappers + [`dvb_t2_bicm_stages`](stages::dvb_t2_bicm_stages) wiring factory |
//! | [`connector`] | [`Connector`], [`Edge`], [`StageId`] |
//! | [`error`] | [`StageError`], [`RecoverableError`], [`FatalError`], [`BuildError`] |
//! | [`config`] | [`PipelineConfig`] (with `From<&SimulationConfig>`) |
//! | [`observability`] | tracing setup, [`observability::install_campaign_subscriber`] |
//! | [`parallel`] | per-worker dispatch + ChaCha20 seek (owned by `3fcb7025`) |
//! | [`presets`] | typestate preset builders (owned by `81d05bab`) |
//! | [`graph`] | graph API + `build()` (owned by `c09d3e95`) |
//! | [`channels`] | channel stages (owned by `db9836e4`) |
//! | [`checkpoint`] | v2 checkpoint schema (owned by `5f12e7ff`) |
//! | [`executor`] | hybrid executor (Phase C stub) |
//! | [`gpu`] | HIP host dispatch (Phase B; `feature = "hip"`) |
//!
//! # Design reference
//!
//! The trait shapes, error hierarchy, module layout, and determinism contract
//! are specified in the Phase 0 design doc
//! `dev/active/ec530af9-pipeline-design.md`, which is the single source of
//! truth for this crate.
#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod batch;
pub mod channels;
pub mod checkpoint;
pub mod config;
pub mod connector;
pub mod error;
pub mod executor;
pub mod gpu;
pub mod graph;
pub mod observability;
pub mod parallel;
pub mod pipeline;
pub mod presets;
pub mod stage;
pub mod stages;

#[doc(inline)]
pub use batch::{BitPackedBatch, HardDecisionBatch, LlrBatch, SymbolBatch};
#[doc(inline)]
pub use config::PipelineConfig;
#[doc(inline)]
pub use connector::{Connector, Edge, StageId};
#[doc(inline)]
pub use error::{BuildError, FatalError, RecoverableError, StageError};
#[doc(inline)]
pub use graph::Chain;
#[doc(inline)]
pub use pipeline::Pipeline;
#[doc(inline)]
pub use stage::{
    erase, AnyScratch, AnyStage, BatchSize, ErasedStage, ExecutionClass, FallbackKind, Stage,
    TypedBatch,
};
