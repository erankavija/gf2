//! Error type hierarchy for the simulation pipeline.

use crate::connector::StageId;

/// The top-level error returned by [`Stage::process`](crate::Stage::process)
/// and the pipeline run entry points.
///
/// Splits into a [`RecoverableError`] (the executor may substitute a CPU
/// fallback and continue) and a [`FatalError`] (the run aborts).
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
    /// `Scratch` type.
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
    /// batch and continues. Promoted to
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
    /// set, or raised unconditionally when a CPU fallback is also OOM.
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
    DuplicateFallback {
        /// The GPU stage that was registered more than once.
        gpu_stage: StageId,
    },
    /// A registered CPU fallback has a different input or output batch type
    /// than the GPU stage it substitutes.
    ///
    /// The executor substitutes the fallback on GPU OOM, so both stages need
    /// identical input and output element types.
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
    /// A single stage was registered in two conflicting fallback roles.
    ///
    /// A stage is either a GPU stage with a registered CPU fallback or a CPU
    /// fallback target. A fallback target is excluded from the stage graph.
    FallbackRoleConflict {
        /// The stage registered in both the GPU and the CPU-fallback role.
        stage: StageId,
    },
    /// A CPU fallback was registered for a stage that cannot run on the GPU.
    ///
    /// Only an [`ExecutionClass::GpuOnly`](crate::stage::ExecutionClass::GpuOnly)
    /// or [`ExecutionClass::Hybrid`](crate::stage::ExecutionClass::Hybrid)
    /// stage takes a fallback.
    FallbackForCpuStage {
        /// The stage that was given a fallback despite not running on the GPU.
        gpu_stage: StageId,
    },
    /// A registered CPU fallback cannot run on the CPU.
    ///
    /// A fallback is an
    /// [`ExecutionClass::CpuOnly`](crate::stage::ExecutionClass::CpuOnly) or
    /// [`ExecutionClass::Hybrid`](crate::stage::ExecutionClass::Hybrid) stage.
    FallbackNotCpuCapable {
        /// The CPU fallback stage that is not CPU-capable.
        cpu_stage: StageId,
    },
    /// A CPU fallback target has an incident graph edge.
    ///
    /// A fallback target is excluded from the built pipeline's stage list and
    /// topological order, so [`Chain::connect`](crate::graph::Chain::connect)
    /// on either end of it is rejected at build time.
    FallbackTargetHasEdge {
        /// The CPU fallback target that was (incorrectly) given an edge.
        stage: StageId,
        /// The other endpoint of the offending edge.
        edge_peer: StageId,
    },
    /// An invalid `(rate, modulation)` combination was requested.
    ///
    /// The descriptors are strings so that every preset reports the values
    /// it rejects without mapping them onto a shared enum.
    InvalidModcod {
        /// A human-readable rendering of the requested code rate (e.g.
        /// `"Rate5_6"`).
        rate: String,
        /// A human-readable rendering of the requested modulation (e.g.
        /// `"Qpsk"`).
        modulation: String,
    },
    /// An invalid 5G NR LDPC builder parameter combination was requested
    /// (`@/citation/ThreeGpp2017`).
    ///
    /// Raised by the [`Pipeline::nr_5g`](crate::Pipeline::nr_5g) preset's
    /// `build()` for every parameter rejection other than a channel fault,
    /// for example a lifting size `Z` outside Table 5.3.2-1 or a modulation
    /// order whose bits-per-symbol does not divide the rate-matched length
    /// `E` (`@/citation/ThreeGpp2020` §5.4.2.2).
    InvalidNr5gParams {
        /// A human-readable explanation of the rejected parameter combination.
        reason: String,
    },
    /// A channel parameter is invalid — e.g. a non-finite (`NaN`/`±inf`) Es/N0,
    /// or one so large that the derived demapper noise variance underflows to a
    /// non-positive value.
    ///
    /// Returned by a preset's `build()`.
    InvalidChannel {
        /// A human-readable explanation of the rejected channel parameter.
        reason: String,
    },
    /// Execution-start validation rejected the built pipeline.
    ///
    /// Raised as `StageError::Fatal(FatalError::BuildError(..))` when the
    /// stage order or connector lineage is inconsistent (an edge that does not
    /// go forward in the stage list, an out-of-range edge endpoint), or when a
    /// stage-driven run entry point does not support the pipeline's shape.
    ExecutionValidation {
        /// A human-readable explanation of the inconsistency found.
        reason: String,
    },
    /// A loaded checkpoint's `config_hash` does not match the live
    /// [`PipelineConfig`](crate::PipelineConfig); the resume aborts.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::TypeId;

    #[test]
    fn test_stage_error_display_recoverable() {
        let r = StageError::Recoverable(RecoverableError::OutOfMemory {
            device_id: 0,
            bytes_requested: 1024,
        });
        let s = format!("{r}");
        assert!(s.contains("recoverable"), "Recoverable variant: {s}");
    }

    #[test]
    fn test_stage_error_display_fatal() {
        let f = StageError::Fatal(FatalError::KernelLaunch {
            hip_code: 42,
            kernel: "test_kernel",
            args: "args".to_string(),
        });
        let s = format!("{f}");
        assert!(s.contains("fatal"), "Fatal variant: {s}");
    }

    #[test]
    fn test_stage_error_display_type_mismatch() {
        let tm = StageError::TypeMismatch {
            expected: TypeId::of::<u32>(),
            actual: TypeId::of::<u64>(),
        };
        let s = format!("{tm}");
        assert!(s.contains("downcast"), "TypeMismatch variant: {s}");
    }

    #[test]
    fn test_stage_error_is_error_trait() {
        let e: &dyn std::error::Error = &StageError::Fatal(FatalError::DeviceUnavailable);
        assert!(!format!("{e}").is_empty());
    }
}
