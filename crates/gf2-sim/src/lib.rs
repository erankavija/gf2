//! `gf2-sim` — research-grade CPU+GPU FEC simulation pipeline.
//!
//! This crate provides the [`Pipeline`] / [`Stage`] / [`Connector`] primitives
//! that compose error-correcting-code building blocks from `gf2-coding` into a
//! parallel, optionally GPU-accelerated, deterministic simulation harness. It
//! is the v2 successor to the simulation machinery in
//! `gf2_coding::simulation`.
//!
//! # Quickstart
//!
//! Build a standard pipeline with a typestate preset, configure a short SNR
//! sweep, and run it:
//!
//! ```no_run
//! use std::num::NonZeroUsize;
//! use gf2_sim::Pipeline;
//! use gf2_sim::presets::dvb_t2::{Channel, Modcod};
//! use gf2_coding::CodeRate;
//! use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
//! use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
//! use gf2_coding::modem::DemapMethod;
//!
//! let mut pipeline = Pipeline::dvb_t2()
//!     .modcod(Modcod::Normal { rate: CodeRate::Rate1_2, modulation: DvbT2Modulation::Qam16 })
//!     .decoder(DecoderConfig::new(DecoderAlgorithm::SumProduct, true))
//!     .demap(DemapMethod::ExactLogMap)
//!     .channel(Channel::awgn(6.0))
//!     .seed(0xDE16_0FC5)
//!     .parallelism(NonZeroUsize::new(4).unwrap())
//!     .build()
//!     .unwrap();
//! pipeline.config_mut().esn0_db_points = vec![6.0];
//! pipeline.config_mut().max_frames = 24;
//! let results = pipeline.run().unwrap();
//! println!("FER = {}", results.per_point[0].fer);
//! ```
//!
//! # Two ways to build a pipeline
//!
//! * **Typestate presets** ([`presets`]) — the production path for the standard
//!   chains. [`Pipeline::dvb_t2`] and [`Pipeline::nr_5g`] are fluent builders
//!   whose method order is checked at compile time (calling `.decoder(...)`
//!   before `.modcod(...)` does not compile). Each emits the same validated
//!   [`Pipeline`] the graph API would.
//! * **Graph API** ([`graph::Chain`]) — the low-level path for non-standard
//!   chains. [`Chain::add`](graph::Chain::add) /
//!   [`Chain::connect`](graph::Chain::connect) /
//!   [`Chain::build`](graph::Chain::build) hand-wire any DAG of [`Stage`]s
//!   (including your own custom stage), topo-sort it, re-validate the edge
//!   types, and emit a [`Pipeline`].
//!
//! Both paths produce a [`Pipeline`] driven either by the sweep-level
//! [`Pipeline::run`] (DVB-T2 BICM SNR sweep) or, per-batch, by the generic
//! [`TopologyExecutor::run`] (the 5G NR drive path — there is no NR
//! sweep-level `Pipeline::run`).
//!
//! # Worked examples (`crates/gf2-sim/examples/`)
//!
//! | Example | Shows |
//! |---------|-------|
//! | `dvb_t2_quickstart.rs` | build a DVB-T2 pipeline via the preset, run a short sweep, print the summary |
//! | `nr_5g_quickstart.rs` | the same shape for the 5G NR BG1 / `Z` = 384 / rate-1/2 preset, driven per-batch via [`TopologyExecutor::run`] |
//! | `dvb_t2_typestate.rs` | the compile-time-checked typestate builder order |
//! | `dvb_t2_graph_api.rs` | the same DVB-T2 chain hand-wired through [`graph::Chain`] |
//! | `novel_chain_via_graph.rs` | a non-standard chain with a **custom** [`Stage`] (a periodic puncturer) spliced into the graph |
//! | `parallel_byte_identity.rs` | the §11 CPU byte-identity contract — the same config at parallelism {1, 24} agrees on all four columns |
//! | `gpu_hybrid.rs` (`--features hip`) | the same chain on the CPU+GPU path vs CPU-only, asserting the §11 CPU-vs-GPU three-column byte-identity |
//!
//! Run any example with `cargo run -p gf2-sim --example <name> --release`
//! (add `--features hip` for `gpu_hybrid`).
//!
//! # Determinism contract (design doc §11)
//!
//! At a fixed seed the four columns `fer` / `frames` / `errors` / `mean_iters`
//! are byte-identical across CPU worker counts {1, 2, 4, 8, 24}, and
//! resume-from-checkpoint reproduces an uninterrupted run bit-for-bit. The
//! CPU-vs-GPU contract is relaxed to **three** columns (`fer` / `frames` /
//! `errors`); `mean_iters` is excluded (RDNA2 transcendental ULP drift can
//! shift the BP convergence iteration by ±1). `ber` and `wall_seconds` are
//! always excluded.
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
//! | [`parallel`] | per-worker dispatch + ChaCha20 seek + counter reduction (owned by `3fcb7025`) |
//! | [`frame_sim`] | reusable DVB-T2 BICM-AWGN single-frame simulation kernel (owned by `3fcb7025`) |
//! | [`presets`] | typestate preset builders (owned by `81d05bab`) |
//! | [`graph`] | graph API + `build()` (owned by `c09d3e95`) |
//! | [`channels`] | channel stages (owned by `db9836e4`) |
//! | [`checkpoint`] | v2 checkpoint schema (owned by `5f12e7ff`) |
//! | [`executor`] | hybrid CPU/GPU [`Scheduler`] + [`SimulationResults`] (Phase C `75c22fa8`) + DAG [`TopologyExecutor`] (`de160fc5`) + GPU drain-for-checkpoint / checkpointed hybrid sweep (`571c11c4`) |
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
pub mod frame_sim;
pub mod gpu;
pub mod graph;
pub mod observability;
pub mod parallel;
pub mod pipeline;
pub mod presets;
pub mod stage;
pub mod stages;

/// Test/bench-only deterministic generators (the shared AWGN channel-LLR
/// source) exposed for integration tests and benches via the `test-support`
/// feature. Also compiled under `cfg(test)` for internal unit tests; the dual
/// gate mirrors the `gf2-algebra::testutil` / `gf2-core::test-support`
/// workspace pattern.
#[cfg(any(test, feature = "test-support"))]
pub mod testutil;

#[doc(inline)]
pub use batch::{BitPackedBatch, HardDecisionBatch, LlrBatch, SymbolBatch};
#[doc(inline)]
pub use checkpoint::{
    config_hash, run_snr_point_checkpointed, run_sweep_checkpointed, CheckpointReader,
    CheckpointV2, CheckpointWriter, CheckpointedRun, SweepError, SweepRun, WorkerState,
};
#[doc(inline)]
pub use config::PipelineConfig;
#[doc(inline)]
pub use connector::{Connector, Edge, StageId};
#[doc(inline)]
pub use error::{BuildError, FatalError, RecoverableError, StageError};
#[doc(inline)]
pub use executor::{
    ActivityInterval, ActivityKind, CheckpointedSweep, DagOutputs, OverlapTimeline, RunPlan,
    Scheduler, SimulationResults, SnrPointResult, StreamInFlight, TopologyExecutor,
};
#[doc(inline)]
pub use frame_sim::DvbT2BicmFrameSim;
#[doc(inline)]
pub use graph::Chain;
#[doc(inline)]
pub use parallel::{
    run_snr_point, run_snr_point_range, run_snr_point_stateless, worker_offset, FrameOutcome,
    SnrPointRangeOutcome, WorkerCounters, WorkerCtx, FRAME_STRIDE, SNR_STRIDE, WORKER_STRIDE,
};
#[doc(inline)]
pub use pipeline::{BatchHandle, Pipeline};
#[doc(inline)]
pub use stage::{
    erase, AnyScratch, AnyStage, BatchSize, ErasedStage, ExecutionClass, FallbackKind, Stage,
    TypedBatch,
};
