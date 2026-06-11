//! Stage trait shapes and the type-erasure layer.
//!
//! This module lifts §1 "Stage / Connector trait shapes" of the Phase 0
//! design doc (`dev/active/ec530af9-pipeline-design.md`) into code:
//! [`Stage`], the [`AnyStage`] / [`TypedBatch`] / [`AnyScratch`] type-erasure
//! layer, and the [`ExecutionClass`] / [`FallbackKind`] enums.

use std::any::TypeId;
use std::marker::PhantomData;

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
    ///
    /// Bounded `'static` so the erasure layer ([`ErasedStage::process_any`])
    /// can `downcast_mut` the type-erased [`AnyScratch`] back to this concrete
    /// type via [`TypeId`]; `Any`-based downcasting requires `'static`. Scratch
    /// is owned, pool-acquired storage, so this is not a practical restriction.
    type Scratch: Default + Send + Sync + 'static;

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

    /// Returns an `&dyn Any` view of this batch for downcasting.
    ///
    /// `&dyn TypedBatch` cannot be coerced to `&dyn Any` directly (the latter
    /// is not a supertrait pointer), so the erasure layer routes downcasts
    /// through this method. Provided by the blanket impl below; implementors
    /// never override it.
    fn as_any(&self) -> &dyn std::any::Any;
}

impl<T: std::any::Any + Send + Sync + BatchSize> TypedBatch for T {
    fn batch_size(&self) -> usize {
        BatchSize::batch_size(self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Provides the frame count for a concrete batch type.
///
/// Implement this on each batch newtype; the blanket [`TypedBatch`] impl then
/// supplies the `as_any` downcast hook automatically. Splitting the
/// `batch_size` requirement out of [`TypedBatch`] lets `TypedBatch` carry a
/// blanket impl (which would otherwise conflict with manual `as_any`
/// definitions) while keeping a single thing for batch types to implement.
pub trait BatchSize {
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
/// A concrete `Stage<I, O>` is erased to `Box<dyn AnyStage>` via the
/// [`ErasedStage`] adapter (or the [`erase`] convenience constructor); the
/// adapter's `process_any` downcasts the input batch via [`TypedBatch`], runs
/// the concrete stage, and re-erases the output. This lets the pipeline own a
/// heterogeneous `Vec<Box<dyn AnyStage>>`.
///
/// # Realisation of the design doc's "blanket impl"
///
/// The Phase 0 design doc (§1) describes `AnyStage` as "implemented for every
/// `Stage<I, O>` via a blanket impl". A literal
/// `impl<I, O, S: Stage<I, O>> AnyStage for S` does **not** compile: the type
/// parameters `I` and `O` are unconstrained by the `Self` type (`S`), which
/// Rust rejects with E0207. The [`ErasedStage`] wrapper threads `I`/`O`
/// through a `PhantomData` field so the impl's `Self` type does constrain
/// them, achieving the same effect. The intent — that no later task ever has
/// to reopen this file to make a stage usable in a pipeline — is preserved:
/// any `Stage` becomes an `AnyStage` through [`erase`].
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

    /// Returns an [`Any`](std::any::Any) view of the **concrete** stage behind
    /// this erased handle, or `None` if the implementor does not expose one.
    ///
    /// The hybrid executor routes a pipeline's stages by
    /// [`execution_class`](AnyStage::execution_class) and must downcast the
    /// discovered `GpuOnly` stage back to its concrete type to drive its
    /// device-specific batch API (`Scheduler`, deliverable 3 of `75c22fa8`);
    /// this hook is how the type-erased stage list supports that without
    /// reopening the erasure layer per stage. The provided default returns
    /// `None` so existing `AnyStage` implementors keep compiling;
    /// [`ErasedStage`] — and therefore everything built via [`erase`] —
    /// overrides it to expose the wrapped stage.
    fn stage_as_any(&self) -> Option<&dyn std::any::Any> {
        None
    }

    /// Allocates a fresh, default-initialised scratch of the concrete
    /// [`Stage::Scratch`] type behind this erased handle.
    ///
    /// The DAG topology executor (`de160fc5`) — and any other consumer driving
    /// erased stages via [`process_any`](AnyStage::process_any) — cannot name
    /// the concrete scratch type, so this hook is the only way to obtain a
    /// scratch that the stage's `process_any` downcast will accept.
    ///
    /// The provided default returns a unit `()` scratch so pre-existing
    /// `AnyStage` implementors keep compiling; [`ErasedStage`] — and therefore
    /// everything built via [`erase`] — overrides it to return
    /// `S::Scratch::default()`.
    fn default_scratch(&self) -> Box<dyn AnyScratch> {
        Box::new(())
    }

    /// A human-readable name for this stage, used as the `stage_name` field of
    /// the executor's per-stage `pipeline_stage` tracing spans (`de160fc5`).
    ///
    /// The provided default returns `"unnamed-stage"` so pre-existing
    /// `AnyStage` implementors keep compiling; [`ErasedStage`] — and therefore
    /// everything built via [`erase`] — overrides it with
    /// [`std::any::type_name`] of the wrapped concrete stage type.
    fn name(&self) -> &'static str {
        "unnamed-stage"
    }

    /// Runs the stage's registered CPU fallback on a type-erased batch.
    ///
    /// The OOM auto-fallback executor (`42eac5cc`) calls this when a GPU stage
    /// returns a recoverable error: instead of calling [`process_any`] (which
    /// would retry the same GPU path), it calls this method to invoke the
    /// concrete [`Stage::CpuFallback`] registered on the stage.
    ///
    /// Returns `None` when the stage has no CPU fallback ([`cpu_fallback()`]
    /// returns `None`). The provided default returns `None` so pre-existing
    /// `AnyStage` implementors keep compiling; [`ErasedStage`] overrides it.
    ///
    /// # The `_scratch` parameter
    ///
    /// `_scratch` is the **faulting GPU stage's own** pooled scratch (the one
    /// the executor holds for this stage position) — NOT the fallback's. The
    /// fallback stage's scratch is a different concrete type, so the erased
    /// impl cannot downcast `_scratch` for it; the parameter exists so a
    /// future stage-specific override can thread positioned state across, and
    /// is unused by the generic [`ErasedStage`] impl.
    ///
    /// # Scratch contract: `()`-scratch fallbacks only (§11)
    ///
    /// The generic erased impl runs the fallback with a **fresh
    /// default-initialised scratch**. That is sound only when the fallback's
    /// [`Stage::Scratch`] is `()` (true for the GPU LDPC BP and demap
    /// fallbacks). A **stateful** fallback scratch — e.g. the hip-gated
    /// `GpuAwgn`'s fallback
    /// [`Awgn`](crate::channels::Awgn), whose
    /// [`ChannelScratch`](crate::channels::awgn::ChannelScratch) default is a
    /// seed-0 RNG — would make the fallback output depend on a
    /// default-initialised RNG position instead of the §3 per-frame keying,
    /// **silently violating the §11 byte-identity contract**. The erased impl
    /// therefore REFUSES such fallbacks with a typed
    /// [`BuildError::ExecutionValidation`](crate::error::BuildError::ExecutionValidation)
    /// error naming the stage, rather than silently default-seeding.
    ///
    /// [`process_any`]: AnyStage::process_any
    /// [`cpu_fallback()`]: Stage::cpu_fallback
    fn cpu_fallback_process_any(
        &self,
        _input: &dyn TypedBatch,
        _scratch: &mut dyn AnyScratch,
    ) -> Option<Result<Box<dyn TypedBatch>, StageError>> {
        None
    }
}

/// Type-erasing adapter wrapping a concrete [`Stage<I, O>`] as an [`AnyStage`].
///
/// This is how the design doc's "blanket impl on `Stage<I, O>`" is realised:
/// a literal blanket impl is rejected by E0207 because `I`/`O` are
/// unconstrained by the `Self` type, so the input/output types are threaded
/// through a `PhantomData<fn(I) -> O>` field instead (which constrains them in
/// the impl's `Self` type, and gives the correct contravariant-in-`I`,
/// covariant-in-`O` variance without affecting drop checking or auto traits).
///
/// Use [`erase`] to box a stage without naming these generics by hand.
pub struct ErasedStage<I, O, S> {
    stage: S,
    _io: PhantomData<fn(I) -> O>,
}

impl<I, O, S> ErasedStage<I, O, S>
where
    S: Stage<I, O>,
    I: TypedBatch,
    O: TypedBatch,
{
    /// Wraps `stage` so it can be held as a `dyn AnyStage`.
    pub fn new(stage: S) -> Self {
        Self {
            stage,
            _io: PhantomData,
        }
    }
}

impl<I, O, S> AnyStage for ErasedStage<I, O, S>
where
    S: Stage<I, O> + 'static,
    I: TypedBatch,
    O: TypedBatch,
{
    fn input_type(&self) -> TypeId {
        TypeId::of::<I>()
    }

    fn output_type(&self) -> TypeId {
        TypeId::of::<O>()
    }

    fn execution_class(&self) -> ExecutionClass {
        Stage::execution_class(&self.stage)
    }

    fn fallback_kind(&self) -> FallbackKind {
        // A generic adapter cannot observe whether `S::CpuFallback == S`, so it
        // cannot distinguish `SelfFallback` from the others. It reports
        // `Registered` when the stage hands back a fallback and `None`
        // otherwise. Presets that know a stage is its own fallback set
        // `SelfFallback` when registering it with the executor (design doc §8).
        if self.stage.cpu_fallback().is_some() {
            FallbackKind::Registered
        } else {
            FallbackKind::None
        }
    }

    fn process_any(
        &self,
        input: &dyn TypedBatch,
        scratch: &mut dyn AnyScratch,
    ) -> Result<Box<dyn TypedBatch>, StageError> {
        let input = input
            .as_any()
            .downcast_ref::<I>()
            .ok_or_else(|| StageError::TypeMismatch {
                expected: TypeId::of::<I>(),
                actual: input.as_any().type_id(),
            })?;
        let scratch = scratch
            .as_any_mut()
            .downcast_mut::<S::Scratch>()
            .ok_or_else(|| StageError::TypeMismatch {
                expected: TypeId::of::<S::Scratch>(),
                // `as_any_mut` was already consumed by `downcast_mut`; report
                // the expected type only (the actual is unobservable here).
                actual: TypeId::of::<S::Scratch>(),
            })?;
        let out = self.stage.process(input, scratch)?;
        Ok(Box::new(out))
    }

    fn stage_as_any(&self) -> Option<&dyn std::any::Any> {
        Some(&self.stage)
    }

    fn default_scratch(&self) -> Box<dyn AnyScratch> {
        Box::new(S::Scratch::default())
    }

    fn name(&self) -> &'static str {
        std::any::type_name::<S>()
    }

    fn cpu_fallback_process_any(
        &self,
        input: &dyn TypedBatch,
        _scratch: &mut dyn AnyScratch,
    ) -> Option<Result<Box<dyn TypedBatch>, StageError>> {
        let fb = self.stage.cpu_fallback()?;
        // §11 guard: the fallback runs with a FRESH default scratch below, which
        // is reproducible only for a stateless `()` scratch (the GPU LDPC BP and
        // demap fallbacks). A stateful fallback scratch (e.g. GpuAwgn's `Awgn`
        // fallback with its `ChannelScratch` seed-0-RNG default) would silently
        // replace the §3 per-frame RNG keying with a default-seeded stream,
        // corrupting byte-identity — so it is REFUSED with a typed error
        // instead (see the trait-method docs).
        if TypeId::of::<<S::CpuFallback as Stage<I, O>>::Scratch>() != TypeId::of::<()>() {
            return Some(Err(StageError::Fatal(crate::error::FatalError::BuildError(
                crate::error::BuildError::ExecutionValidation {
                    reason: format!(
                        "stage `{}` has a CPU fallback with stateful scratch `{}`: a \
                         default-initialised scratch is not reproducible across the \
                         erased fallback boundary (design §11 byte-identity), so the \
                         fallback is refused rather than silently default-seeded",
                        std::any::type_name::<S>(),
                        std::any::type_name::<<S::CpuFallback as Stage<I, O>>::Scratch>(),
                    ),
                },
            ))));
        }
        let input = match input.as_any().downcast_ref::<I>() {
            Some(i) => i,
            None => {
                return Some(Err(StageError::TypeMismatch {
                    expected: TypeId::of::<I>(),
                    actual: input.as_any().type_id(),
                }))
            }
        };
        // Reaching here, the fallback's scratch is `()`: a fresh default IS the
        // (only) correct scratch for the call.
        let mut fb_scratch = <<S::CpuFallback as Stage<I, O>>::Scratch as Default>::default();
        Some(
            fb.process(input, &mut fb_scratch)
                .map(|o| -> Box<dyn TypedBatch> { Box::new(o) }),
        )
    }
}

/// Erases a concrete [`Stage<I, O>`] into a `Box<dyn AnyStage>`.
///
/// Convenience constructor wrapping [`ErasedStage::new`] so callers never name
/// the `ErasedStage<I, O, S>` generics by hand. This is the single entry point
/// by which any stage becomes pipeline-ready, so no later wave needs to reopen
/// this module to make a new stage usable.
///
/// # Examples
///
/// ```
/// use gf2_sim::stage::{erase, BatchSize, ExecutionClass, Stage};
/// use gf2_sim::error::StageError;
///
/// #[derive(Clone)]
/// struct Bits(Vec<u8>);
/// impl BatchSize for Bits {
///     fn batch_size(&self) -> usize {
///         self.0.len()
///     }
/// }
///
/// struct Copy;
/// impl Stage<Bits, Bits> for Copy {
///     type Scratch = ();
///     type CpuFallback = Self;
///     fn process(&self, input: &Bits, _: &mut ()) -> Result<Bits, StageError> {
///         Ok(input.clone())
///     }
///     fn execution_class(&self) -> ExecutionClass {
///         ExecutionClass::CpuOnly
///     }
/// }
///
/// let boxed = erase(Copy);
/// assert_eq!(boxed.input_type(), boxed.output_type());
/// ```
pub fn erase<I, O, S>(stage: S) -> Box<dyn AnyStage>
where
    S: Stage<I, O> + 'static,
    I: TypedBatch,
    O: TypedBatch,
{
    Box::new(ErasedStage::new(stage))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tiny input batch newtype.
    #[derive(Debug, PartialEq, Eq)]
    struct InBatch(u64);
    impl BatchSize for InBatch {
        fn batch_size(&self) -> usize {
            1
        }
    }

    /// Tiny output batch newtype (distinct type, so the output `TypeId` is
    /// observably different from the input's).
    #[derive(Debug, PartialEq, Eq)]
    struct OutBatch(u64);
    impl BatchSize for OutBatch {
        fn batch_size(&self) -> usize {
            1
        }
    }

    /// Trivial stage: doubles the wrapped value, recording the call in scratch.
    struct Doubler;
    impl Stage<InBatch, OutBatch> for Doubler {
        type Scratch = u32;
        type CpuFallback = Self;

        fn process(&self, input: &InBatch, scratch: &mut u32) -> Result<OutBatch, StageError> {
            *scratch += 1;
            Ok(OutBatch(input.0 * 2))
        }

        fn execution_class(&self) -> ExecutionClass {
            ExecutionClass::CpuOnly
        }
    }

    #[test]
    fn test_erase_roundtrips_a_batch_through_process_any() {
        let erased: Box<dyn AnyStage> = erase(Doubler);

        // Type ids reflect the concrete I/O newtypes and differ from each other.
        assert_eq!(erased.input_type(), TypeId::of::<InBatch>());
        assert_eq!(erased.output_type(), TypeId::of::<OutBatch>());
        assert_ne!(erased.input_type(), erased.output_type());

        // CPU-only stage with no separate fallback registered.
        assert_eq!(erased.execution_class(), ExecutionClass::CpuOnly);
        assert_eq!(erased.fallback_kind(), FallbackKind::None);

        // Round-trip one batch through the erased path.
        let input: Box<dyn TypedBatch> = Box::new(InBatch(21));
        let mut scratch: Box<dyn AnyScratch> = Box::new(0u32);
        let out = erased
            .process_any(input.as_ref(), scratch.as_mut())
            .expect("process_any should succeed");

        // The erased output carries the correct concrete type and value.
        assert_eq!(out.as_any().type_id(), TypeId::of::<OutBatch>());
        let out = out
            .as_any()
            .downcast_ref::<OutBatch>()
            .expect("output downcasts to OutBatch");
        assert_eq!(*out, OutBatch(42));

        // Scratch was threaded through and mutated by the concrete stage.
        // Deref the box to the `dyn AnyScratch` so `as_any_mut` dispatches to
        // the inner `u32`'s impl (not the blanket impl on `Box<dyn AnyScratch>`).
        let used = (*scratch)
            .as_any_mut()
            .downcast_mut::<u32>()
            .expect("scratch downcasts to u32");
        assert_eq!(*used, 1);
    }

    #[test]
    fn test_stage_as_any_downcasts_to_concrete_stage() {
        // The erased handle must expose the wrapped concrete stage so the
        // hybrid executor can downcast a class-routed stage (75c22fa8
        // deliverable 3).
        let erased: Box<dyn AnyStage> = erase(Doubler);
        let any = erased
            .stage_as_any()
            .expect("ErasedStage exposes its concrete stage");
        assert!(
            any.downcast_ref::<Doubler>().is_some(),
            "the exposed Any must downcast to the wrapped stage type"
        );
        assert!(
            any.downcast_ref::<u32>().is_none(),
            "a wrong-type downcast must fail cleanly"
        );
    }

    #[test]
    fn test_default_scratch_matches_concrete_scratch_type() {
        // The erased handle must hand back a scratch of the concrete
        // `Stage::Scratch` type (here `u32`) that `process_any` accepts.
        let erased: Box<dyn AnyStage> = erase(Doubler);
        let mut scratch = erased.default_scratch();
        // Deref the box so `as_any_mut` dispatches to the inner scratch (the
        // blanket impl also exists on `Box<dyn AnyScratch>` itself).
        assert!(
            (*scratch).as_any_mut().downcast_mut::<u32>().is_some(),
            "default_scratch must allocate the concrete Scratch type"
        );
        let input: Box<dyn TypedBatch> = Box::new(InBatch(1));
        erased
            .process_any(input.as_ref(), scratch.as_mut())
            .expect("process_any accepts the default scratch");
    }

    #[test]
    fn test_name_reports_concrete_stage_type() {
        let erased: Box<dyn AnyStage> = erase(Doubler);
        assert!(
            erased.name().ends_with("Doubler"),
            "erased stage name must be the concrete type name, got {}",
            erased.name()
        );
    }

    #[test]
    fn test_process_any_type_mismatch_on_wrong_input() {
        let erased: Box<dyn AnyStage> = erase(Doubler);

        // Feed an OutBatch where an InBatch is expected.
        let wrong: Box<dyn TypedBatch> = Box::new(OutBatch(7));
        let mut scratch: Box<dyn AnyScratch> = Box::new(0u32);
        match erased.process_any(wrong.as_ref(), scratch.as_mut()) {
            Err(StageError::TypeMismatch { expected, actual }) => {
                assert_eq!(expected, TypeId::of::<InBatch>());
                assert_eq!(actual, TypeId::of::<OutBatch>());
            }
            Err(other) => panic!("expected TypeMismatch, got {other:?}"),
            Ok(_) => panic!("mismatched input type must fail"),
        }
    }

    // ────────────────────────────────────────────────────────────────────
    // HIGH-1 (42eac5cc r2 review): the erased fallback hook must REFUSE a
    // fallback whose scratch is stateful, never silently default-seed it.
    // ────────────────────────────────────────────────────────────────────

    /// A GpuAwgn-shaped fallback: stateful scratch whose `Default` is a
    /// sentinel "seed-0" position. If the erased hook ever ran this fallback
    /// with a default scratch, the output would betray it (it returns the
    /// scratch state, mirroring how `Awgn`'s seed-0 `ChannelScratch` default
    /// would draw seed-0 noise).
    struct StatefulScratch(u64);
    impl Default for StatefulScratch {
        fn default() -> Self {
            // The "seed 0" default a silent fallback would observe.
            StatefulScratch(0)
        }
    }

    struct StatefulFallback;
    impl Stage<InBatch, OutBatch> for StatefulFallback {
        type Scratch = StatefulScratch;
        type CpuFallback = Self;
        fn process(
            &self,
            _input: &InBatch,
            scratch: &mut StatefulScratch,
        ) -> Result<OutBatch, StageError> {
            // Output depends on the scratch position — the §11 hazard.
            Ok(OutBatch(scratch.0))
        }
        fn execution_class(&self) -> ExecutionClass {
            ExecutionClass::CpuOnly
        }
    }

    /// A GpuAwgn-shaped GPU stage registering `StatefulFallback` as its CPU
    /// fallback (mirroring `GpuAwgn::CpuFallback = Awgn` with `ChannelScratch`).
    struct GpuLikeWithStatefulFallback {
        fallback: StatefulFallback,
    }
    impl Stage<InBatch, OutBatch> for GpuLikeWithStatefulFallback {
        type Scratch = ();
        type CpuFallback = StatefulFallback;
        fn process(&self, _input: &InBatch, _scratch: &mut ()) -> Result<OutBatch, StageError> {
            unreachable!("the GPU path is not under test")
        }
        fn execution_class(&self) -> ExecutionClass {
            ExecutionClass::GpuOnly
        }
        fn cpu_fallback(&self) -> Option<&StatefulFallback> {
            Some(&self.fallback)
        }
    }

    /// HIGH-1: a stateful-scratch fallback is refused with the typed
    /// `BuildError::ExecutionValidation` — NOT silently run on a
    /// default-initialised ("seed-0") scratch.
    #[test]
    fn test_stateful_scratch_fallback_is_refused_not_default_seeded() {
        let erased: Box<dyn AnyStage> = erase(GpuLikeWithStatefulFallback {
            fallback: StatefulFallback,
        });
        let input: Box<dyn TypedBatch> = Box::new(InBatch(5));
        let mut scratch = erased.default_scratch();

        let result = erased
            .cpu_fallback_process_any(input.as_ref(), scratch.as_mut())
            .expect("a fallback IS registered, so the hook must engage");
        match result {
            Err(StageError::Fatal(crate::error::FatalError::BuildError(
                crate::error::BuildError::ExecutionValidation { reason },
            ))) => {
                assert!(
                    reason.contains("stateful scratch"),
                    "refusal must name the stateful-scratch cause, got: {reason}"
                );
                assert!(
                    reason.contains("StatefulScratch"),
                    "refusal must name the offending scratch type, got: {reason}"
                );
            }
            Err(other) => panic!("expected ExecutionValidation refusal, got {other:?}"),
            Ok(out) => panic!(
                "stateful-scratch fallback must be REFUSED, but it ran and \
                 produced {:?} (a silently default-seeded output)",
                out.as_any().downcast_ref::<OutBatch>()
            ),
        }
    }

    /// The `()`-scratch fallback contract is unchanged: stateless fallbacks
    /// (the GPU LDPC BP / demap shape) still run through the erased hook.
    #[test]
    fn test_unit_scratch_fallback_still_runs() {
        /// `()`-scratch fallback that maps the input through.
        struct UnitFallback;
        impl Stage<InBatch, OutBatch> for UnitFallback {
            type Scratch = ();
            type CpuFallback = Self;
            fn process(&self, input: &InBatch, _scratch: &mut ()) -> Result<OutBatch, StageError> {
                Ok(OutBatch(input.0 + 100))
            }
            fn execution_class(&self) -> ExecutionClass {
                ExecutionClass::CpuOnly
            }
        }
        struct GpuLikeWithUnitFallback {
            fallback: UnitFallback,
        }
        impl Stage<InBatch, OutBatch> for GpuLikeWithUnitFallback {
            type Scratch = ();
            type CpuFallback = UnitFallback;
            fn process(&self, _i: &InBatch, _s: &mut ()) -> Result<OutBatch, StageError> {
                unreachable!("the GPU path is not under test")
            }
            fn execution_class(&self) -> ExecutionClass {
                ExecutionClass::GpuOnly
            }
            fn cpu_fallback(&self) -> Option<&UnitFallback> {
                Some(&self.fallback)
            }
        }

        let erased: Box<dyn AnyStage> = erase(GpuLikeWithUnitFallback {
            fallback: UnitFallback,
        });
        let input: Box<dyn TypedBatch> = Box::new(InBatch(5));
        let mut scratch = erased.default_scratch();
        let out = erased
            .cpu_fallback_process_any(input.as_ref(), scratch.as_mut())
            .expect("fallback registered")
            .expect("`()`-scratch fallback must run");
        assert_eq!(
            out.as_any().downcast_ref::<OutBatch>(),
            Some(&OutBatch(105)),
            "the stateless fallback must process the input"
        );
    }
}
