//! `gf2-sim`: CPU and GPU FEC simulation pipeline. [`Pipeline`], [`Stage`] and
//! [`Connector`] compose the codes, modems and channels of `gf2-coding` into a
//! parallel, seeded simulation. The typestate [`presets`]
//! ([`Pipeline::dvb_t2`], [`Pipeline::nr_5g`]) build the standard chains with
//! the builder order checked at compile time; [`graph::Chain`] wires an
//! arbitrary DAG of [`Stage`]s. [`Pipeline::run`] drives a DVB-T2 BICM SNR
//! sweep and [`TopologyExecutor::run`] drives one batch through any built
//! pipeline.
//!
//! # Examples
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
//! # Determinism
//!
//! `tests/determinism.rs` asserts that, at a fixed seed, the columns `fer`,
//! `frames`, `errors` and `mean_iters` are byte-identical across CPU worker
//! counts {1, 2, 4, 8, 24} and that a run resumed from a checkpoint equals an
//! uninterrupted one. `tests/gpu_byte_identity.rs` asserts byte-identical
//! `fer`, `frames` and `errors` between the CPU and CPU+GPU paths.
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
pub mod osd_campaign;
pub mod parallel;
pub mod permanent_campaign;
pub mod permanent_rare_event;
pub mod pipeline;
pub mod presets;
pub mod snr_checkpoint;
pub mod stage;
pub mod stages;

/// Deterministic generators for tests and benches.
#[cfg(any(test, feature = "test-support"))]
pub mod testutil;

#[doc(inline)]
pub use batch::{BitPackedBatch, HardDecisionBatch, LlrBatch, SymbolBatch};
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
    map_indices_in_order, run_snr_point, run_snr_point_range, run_snr_point_stateless,
    worker_index_partition, worker_offset, FrameOutcome, SnrPointRangeOutcome, WorkerCounters,
    WorkerCtx, FRAME_STRIDE, SNR_STRIDE, WORKER_STRIDE,
};
#[doc(inline)]
pub use pipeline::{BatchHandle, Pipeline};
#[doc(inline)]
pub use snr_checkpoint::{
    config_hash, run_snr_point_checkpointed, run_sweep_checkpointed, CheckpointReader,
    CheckpointV2, CheckpointWriter, CheckpointedRun, SweepError, SweepRun, WorkerState,
};
#[doc(inline)]
pub use stage::{
    erase, AnyScratch, AnyStage, BatchSize, ErasedStage, ExecutionClass, FallbackKind, Stage,
    TypedBatch,
};
