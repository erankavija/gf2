//! Error type hierarchy for the simulation pipeline.
//!
//! Lifts the §1 "Error type hierarchy" block of the Phase 0 design doc
//! (`dev/active/ec530af9-pipeline-design.md`) into code, including the
//! `OutOfMemory` variants on both [`RecoverableError`] and [`FatalError`]
//! mandated by the Q7 decision (design doc §8).

use crate::connector::StageId;

/// The top-level error returned by [`Stage::process`](crate::Stage::process)
/// and the pipeline submit/collect APIs.
///
/// Splits into a [`RecoverableError`] (the executor may substitute a CPU
/// fallback and continue) and a [`FatalError`] (the run aborts). See the
/// failure-mode policy in design doc §8.
#[derive(Debug)]
pub enum StageError {
    /// A recoverable error; the executor may retry on a CPU fallback.
    Recoverable(RecoverableError),
    /// A fatal error; the run aborts.
    Fatal(FatalError),
}

/// An error the executor may recover from by substituting a CPU fallback.
#[derive(Debug)]
pub enum RecoverableError {
    /// A GPU allocation failed.
    ///
    /// The executor substitutes the stage's CPU fallback on the offending
    /// batch and continues (design doc §8). Promoted to
    /// [`FatalError::OutOfMemory`] when `--strict-gpu` is set.
    OutOfMemory {
        /// The HIP device that ran out of memory.
        device_id: i32,
        /// The allocation size, in bytes, that failed.
        bytes_requested: usize,
    },
    /// A transient error wrapping an arbitrary underlying cause.
    Transient(Box<dyn std::error::Error + Send + Sync>),
}

/// An unrecoverable error that aborts the run.
#[derive(Debug)]
pub enum FatalError {
    /// A GPU allocation failed and no recovery is permitted.
    ///
    /// Promoted from [`RecoverableError::OutOfMemory`] when `--strict-gpu` is
    /// set, or raised unconditionally when a CPU fallback is also OOM
    /// (design doc §8, Q7 decision).
    OutOfMemory {
        /// The HIP device that ran out of memory.
        device_id: i32,
        /// The allocation size, in bytes, that failed.
        bytes_requested: usize,
    },
    /// A GPU kernel launch failed.
    KernelLaunch {
        /// The HIP error code returned by the launch.
        hip_code: i32,
        /// The kernel name.
        kernel: &'static str,
        /// A rendering of the launch arguments for diagnostics.
        args: String,
    },
    /// No usable GPU device was found at pipeline construction.
    DeviceUnavailable,
    /// The pipeline failed to build.
    BuildError(BuildError),
    /// A recoverable error was retried on a CPU fallback that also failed.
    CpuFallbackAlsoFailed {
        /// The original recoverable error that triggered the fallback.
        original: Box<RecoverableError>,
    },
}

/// An error raised while building a pipeline from a stage graph.
#[derive(Debug)]
pub enum BuildError {
    /// The stage graph contains a cycle.
    Cyclic {
        /// The stages involved in the cycle.
        involved: Vec<StageId>,
    },
    /// A connection joins a producer and consumer with incompatible types.
    TypeMismatch {
        /// The producing stage.
        from_stage: StageId,
        /// The producer's output element type.
        from_type: std::any::TypeId,
        /// The consuming stage.
        to_stage: StageId,
        /// The consumer's expected input element type.
        to_type: std::any::TypeId,
    },
    /// One or more stages are not reachable from the source.
    Disconnected {
        /// The disconnected stages.
        stages: Vec<StageId>,
    },
    /// A GPU stage was used without a registered CPU fallback.
    NoFallback {
        /// The offending GPU stage.
        gpu_stage: StageId,
    },
    /// An invalid `(rate, modulation)` combination was requested.
    InvalidModcod {
        /// The requested code rate.
        rate: NrRate,
        /// The requested modulation.
        modulation: Modulation,
    },
    /// A loaded checkpoint's `config_hash` does not match the live config.
    ///
    /// See design doc §4: loaded checkpoints whose `config_hash` differs from
    /// the live [`PipelineConfig`](crate::PipelineConfig) abort the resume.
    ConfigHashMismatch {
        /// The hash recorded in the loaded checkpoint.
        loaded: String,
        /// The hash of the live configuration.
        expected: String,
    },
}

/// Code-rate selector used by [`BuildError::InvalidModcod`].
///
/// A minimal placeholder so the scaffolding compiles standalone. The DVB-T2 /
/// 5G NR preset waves (`81d05bab`) own the authoritative rate enumeration and
/// may relocate or extend this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NrRate {
    /// Rate 1/2.
    R1_2,
    /// Rate 2/3.
    R2_3,
    /// Rate 3/4.
    R3_4,
}

/// Modulation selector used by [`BuildError::InvalidModcod`].
///
/// A minimal placeholder so the scaffolding compiles standalone. The DVB-T2 /
/// 5G NR preset waves (`81d05bab`) own the authoritative modulation
/// enumeration and may relocate or extend this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modulation {
    /// 16-QAM.
    Qam16,
    /// 64-QAM.
    Qam64,
}

impl std::fmt::Display for StageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StageError::Recoverable(e) => write!(f, "recoverable stage error: {e:?}"),
            StageError::Fatal(e) => write!(f, "fatal stage error: {e:?}"),
        }
    }
}

impl std::error::Error for StageError {}
