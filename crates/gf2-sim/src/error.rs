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
    /// A type-erased batch or scratch could not be downcast to the concrete
    /// type the stage expects.
    ///
    /// Raised by [`AnyStage::process_any`](crate::AnyStage::process_any) when
    /// the runtime batch type does not match the stage's compile-time input
    /// type, or when the supplied scratch does not match the stage's
    /// `Scratch` type. A well-formed pipeline (whose [`Edge`](crate::Edge) types were
    /// validated at build time) never produces this at runtime; it indicates
    /// the erased plumbing was wired with mismatched types and is therefore a
    /// logic error rather than a recoverable condition.
    TypeMismatch {
        /// The [`TypeId`](std::any::TypeId) the stage expected.
        expected: std::any::TypeId,
        /// The [`TypeId`](std::any::TypeId) actually supplied, if it could be
        /// determined.
        actual: std::any::TypeId,
    },
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
    /// The same GPU stage was registered with more than one CPU fallback.
    ///
    /// A GPU stage may have at most one CPU fallback. Registering a GPU stage
    /// twice (possibly with different CPU stages) is a configuration error:
    /// the second registration would silently discard one CPU fallback entry
    /// during `HashMap` construction, violating the one-to-one substitution
    /// contract.
    DuplicateFallback {
        /// The GPU stage that was registered more than once.
        gpu_stage: StageId,
    },
    /// A registered CPU fallback has a different input or output batch type
    /// than the GPU stage it substitutes.
    ///
    /// The executor swaps in the CPU fallback transparently on GPU OOM
    /// (design doc §8), so both stages must have identical input and output
    /// element types. A mismatch here would cause a runtime type-downcast
    /// failure when the executor substitutes the fallback.
    FallbackTypeMismatch {
        /// The GPU stage whose type does not match its CPU fallback.
        gpu_stage: StageId,
        /// The CPU fallback stage.
        cpu_stage: StageId,
        /// The [`TypeId`](std::any::TypeId) of the GPU stage's input element type.
        gpu_input_type: std::any::TypeId,
        /// The [`TypeId`](std::any::TypeId) of the CPU fallback's input element type.
        cpu_input_type: std::any::TypeId,
        /// The [`TypeId`](std::any::TypeId) of the GPU stage's output element type.
        gpu_output_type: std::any::TypeId,
        /// The [`TypeId`](std::any::TypeId) of the CPU fallback's output element type.
        cpu_output_type: std::any::TypeId,
    },
    /// An invalid `(rate, modulation)` combination was requested.
    ///
    /// Carries human-readable, standard-agnostic descriptors of the *actual*
    /// offending values so the error reports exactly what was requested. The
    /// descriptors are plain strings (rather than a closed enum) so every preset
    /// — the DVB-T2 preset today, the future 5G NR preset — can report any
    /// rate / modulation it rejects without a lossy mapping onto a fixed set.
    InvalidModcod {
        /// A human-readable rendering of the requested code rate (e.g.
        /// `"Rate5_6"`).
        rate: String,
        /// A human-readable rendering of the requested modulation (e.g.
        /// `"Qpsk"`).
        modulation: String,
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

impl std::fmt::Display for StageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StageError::Recoverable(e) => write!(f, "recoverable stage error: {e:?}"),
            StageError::Fatal(e) => write!(f, "fatal stage error: {e:?}"),
            StageError::TypeMismatch { expected, actual } => write!(
                f,
                "type-erased downcast failed: expected {expected:?}, got {actual:?}"
            ),
        }
    }
}

impl std::error::Error for StageError {}
