//! Stage trait shapes and the type-erasure layer.
//!
//! This module lifts §1 "Stage / Connector trait shapes" of the Phase 0
//! design doc (`dev/active/ec530af9-pipeline-design.md`) into code:
//! [`Stage`], the [`AnyStage`] / [`TypedBatch`] / [`AnyScratch`] type-erasure
//! layer, and the [`ExecutionClass`] / [`FallbackKind`] enums.

use std::any::TypeId;

use crate::error::StageError;

/// A processing stage transforming a batch of `I` into a batch of `O`.
///
/// Stages are the unit of composition in a [`Pipeline`](crate::Pipeline).
/// Each stage is `Send + Sync` so the executor can run many in parallel.
///
/// # Associated types
///
/// * [`Scratch`](Stage::Scratch) — per-stage scratch storage acquired from a
///   pool by the executor; reused across batches to amortise allocation.
/// * [`CpuFallback`](Stage::CpuFallback) — the compile-time-bound CPU stage the
///   executor substitutes on GPU out-of-memory (see design doc §8). A pure-CPU
///   stage names `Self` as its own fallback.
///
/// # Deviation from the design doc
///
/// The design doc writes `type CpuFallback: Stage<I, O> = Self;` using an
/// associated-type default. Associated-type defaults are unstable on the
/// MSRV (Rust 1.95); the default is therefore omitted and each implementor
/// names its fallback explicitly (`type CpuFallback = Self;` for CPU stages).
/// The intent — a compile-bound CPU fallback per Q6 — is preserved.
///
/// # Examples
///
/// ```
/// use gf2_sim::stage::{ExecutionClass, Stage};
/// use gf2_sim::error::StageError;
///
/// struct Identity;
///
/// impl Stage<u8, u8> for Identity {
///     type Scratch = ();
///     type CpuFallback = Self;
///
///     fn process(&self, input: &u8, _scratch: &mut ()) -> Result<u8, StageError> {
///         Ok(*input)
///     }
///
///     fn execution_class(&self) -> ExecutionClass {
///         ExecutionClass::CpuOnly
///     }
/// }
/// ```
pub trait Stage<I, O>: Send + Sync {
    /// Per-stage scratch storage (acquired from a pool by the executor).
    type Scratch: Default + Send + Sync;

    /// Compile-time-bound CPU fallback for OOM substitution (design doc §8).
    ///
    /// A pure-CPU stage names `Self`. GPU stages name the paired CPU stage so
    /// the executor (`42eac5cc`) can substitute on out-of-memory.
    type CpuFallback: Stage<I, O>;

    /// Processes one batch, writing into the supplied `scratch` as needed.
    ///
    /// # Arguments
    ///
    /// * `input` — the input batch.
    /// * `scratch` — reusable per-stage scratch storage.
    ///
    /// # Errors
    ///
    /// Returns a [`StageError`] if the stage cannot process the batch.
    fn process(&self, input: &I, scratch: &mut Self::Scratch) -> Result<O, StageError>;

    /// Whether this stage prefers a structure-of-arrays input layout.
    ///
    /// Defaults to `true`; SoA is the pipeline's internal layout (design doc §2).
    fn prefers_soa(&self) -> bool {
        true
    }

    /// The execution class (CPU, GPU, or hybrid) of this stage.
    fn execution_class(&self) -> ExecutionClass;

    /// Returns the paired CPU fallback stage, if any.
    ///
    /// Defaults to `None` for CPU-only stages. GPU stages MUST override and
    /// return `Some(&fallback)` so the executor can substitute on OOM
    /// (design doc §8).
    fn cpu_fallback(&self) -> Option<&Self::CpuFallback> {
        None
    }
}

/// The execution class of a [`Stage`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionClass {
    /// Runs only on the CPU.
    CpuOnly,
    /// Runs only on the GPU.
    GpuOnly,
    /// May run on either CPU or GPU.
    Hybrid,
}

/// How a [`Stage`]'s CPU fallback is provided.
///
/// Consumed by the executor (design doc §8) to decide OOM substitution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackKind {
    /// The stage is its own fallback (CPU stages).
    SelfFallback,
    /// The stage has a separate CPU fallback registered.
    Registered,
    /// No fallback; the stage fails on OOM unless a fallback was registered
    /// externally by the preset and `--strict-gpu` is off.
    None,
}

/// Marker trait implemented by all batch types crossing stage boundaries.
///
/// Concrete impls are auto-derived for the batch types introduced by later
/// waves (`LlrBatch`, `SymbolBatch`, `BitPackedBatch`, `HardDecisionBatch`,
/// …). The blanket [`AnyStage`] impl downcasts through this trait at the
/// connector boundary.
pub trait TypedBatch: std::any::Any + Send + Sync {
    /// The number of frames in this batch.
    fn batch_size(&self) -> usize;
}

/// Type-erased scratch holder.
///
/// Concrete [`Stage::Scratch`] types implement this via the blanket impl
/// below, letting the executor hold heterogeneous scratch behind one type.
pub trait AnyScratch: Send {
    /// Returns a mutable `Any` view for downcasting back to the concrete type.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

impl<T: std::any::Any + Send> AnyScratch for T {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Type-erased [`Stage`] handle held by a [`Pipeline`](crate::Pipeline).
///
/// The blanket impl (added by a later wave) downcasts the input batch via
/// [`TypedBatch`] and re-erases the output, so the pipeline can own a
/// heterogeneous `Vec<Box<dyn AnyStage>>`.
pub trait AnyStage: Send + Sync {
    /// The [`TypeId`] of the input batch type.
    fn input_type(&self) -> TypeId;

    /// The [`TypeId`] of the output batch type.
    fn output_type(&self) -> TypeId;

    /// The execution class of the erased stage.
    fn execution_class(&self) -> ExecutionClass;

    /// How this stage's CPU fallback is provided.
    fn fallback_kind(&self) -> FallbackKind;

    /// Processes a type-erased batch.
    ///
    /// Downcasts `input` via [`TypedBatch`], runs the concrete stage with the
    /// downcast `scratch`, and re-erases the output.
    ///
    /// # Errors
    ///
    /// Returns a [`StageError`] if the downcast fails or the stage errors.
    fn process_any(
        &self,
        input: &dyn TypedBatch,
        scratch: &mut dyn AnyScratch,
    ) -> Result<Box<dyn TypedBatch>, StageError>;
}
