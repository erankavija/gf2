//! Stage trait shapes and the type-erasure layer.

use std::any::TypeId;
use std::marker::PhantomData;

use crate::error::StageError;

/// A processing stage transforming a batch of `I` into a batch of `O`.
///
/// Stages are the unit of composition in a [`Pipeline`](crate::Pipeline).
/// Each stage is `Send + Sync` so the executor can run many in parallel.
pub trait Stage<I, O>: Send + Sync {
    /// Per-stage scratch storage (acquired from a pool by the executor).
    ///
    /// `'static` because [`ErasedStage::process_any`] downcasts the
    /// type-erased [`AnyScratch`] through `Any`.
    type Scratch: Default + Send + Sync + 'static;

    /// CPU stage the executor substitutes on GPU out-of-memory. A pure-CPU
    /// stage names `Self`.
    type CpuFallback: Stage<I, O>;

    /// Processes one batch, writing into the supplied `scratch` as needed.
    ///
    /// # Errors
    ///
    /// Returns a [`StageError`] if the stage cannot process the batch.
    fn process(&self, input: &I, scratch: &mut Self::Scratch) -> Result<O, StageError>;

    /// Whether this stage prefers a structure-of-arrays input layout.
    ///
    /// Defaults to `true`.
    fn prefers_soa(&self) -> bool {
        true
    }

    /// The execution class (CPU, GPU, or hybrid) of this stage.
    fn execution_class(&self) -> ExecutionClass;

    /// Returns the paired CPU fallback stage, if any.
    ///
    /// Defaults to `None`. A GPU stage overrides it so the executor can
    /// substitute on OOM.
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

/// A batch type crossing a stage boundary.
///
/// Implemented for every [`BatchSize`] type by a blanket impl;
/// [`ErasedStage`] downcasts through it.
pub trait TypedBatch: std::any::Any + Send + Sync {
    /// The number of frames in this batch.
    fn batch_size(&self) -> usize;

    /// Returns an `&dyn Any` view of this batch for downcasting.
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
/// Implementing it supplies [`TypedBatch`] through the blanket impl.
pub trait BatchSize {
    /// The number of frames in this batch.
    fn batch_size(&self) -> usize;
}

/// Type-erased scratch holder, implemented for every `Any + Send` type.
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
/// [`erase`] turns a concrete `Stage<I, O>` into a `Box<dyn AnyStage>` through
/// the [`ErasedStage`] adapter.
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
    /// The default returns `None`; [`ErasedStage`] exposes the wrapped stage.
    fn stage_as_any(&self) -> Option<&dyn std::any::Any> {
        None
    }

    /// Allocates a fresh, default-initialised scratch of the concrete
    /// [`Stage::Scratch`] type behind this erased handle.
    ///
    /// The default returns a `()` scratch; [`ErasedStage`] returns
    /// `S::Scratch::default()`.
    fn default_scratch(&self) -> Box<dyn AnyScratch> {
        Box::new(())
    }

    /// A human-readable name for this stage, used as the `stage_name` field of
    /// the executor's per-stage `pipeline_stage` tracing spans.
    ///
    /// The default returns `"unnamed-stage"`; [`ErasedStage`] returns
    /// [`std::any::type_name`] of the wrapped stage type.
    fn name(&self) -> &'static str {
        "unnamed-stage"
    }

    /// Runs the stage's registered CPU fallback on a type-erased batch.
    ///
    /// Returns `None` when the stage has no CPU fallback; the default always
    /// does. `_scratch` is the faulting GPU stage's scratch, which
    /// [`ErasedStage`] ignores: it runs the fallback with a default-initialised
    /// scratch of the fallback's own type. That is reproducible only for a
    /// `()` scratch, so for any other fallback scratch type [`ErasedStage`]
    /// returns
    /// [`BuildError::ExecutionValidation`](crate::error::BuildError::ExecutionValidation).
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
/// `I` and `O` are carried in a `PhantomData<fn(I) -> O>` field: a blanket
/// `impl<I, O, S: Stage<I, O>> AnyStage for S` leaves them unconstrained
/// (E0207).
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
        // The adapter cannot observe whether `S::CpuFallback == S`, so it
        // never reports `SelfFallback`.
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
        // A default scratch is reproducible only for a stateless `()` scratch.
        if TypeId::of::<<S::CpuFallback as Stage<I, O>>::Scratch>() != TypeId::of::<()>() {
            return Some(Err(StageError::Fatal(
                crate::error::FatalError::BuildError(
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
                ),
            )));
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
        let mut fb_scratch = <<S::CpuFallback as Stage<I, O>>::Scratch as Default>::default();
        Some(
            fb.process(input, &mut fb_scratch)
                .map(|o| -> Box<dyn TypedBatch> { Box::new(o) }),
        )
    }
}

/// Erases a concrete [`Stage<I, O>`] into a `Box<dyn AnyStage>`.
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

    #[derive(Debug, PartialEq, Eq)]
    struct InBatch(u64);
    impl BatchSize for InBatch {
        fn batch_size(&self) -> usize {
            1
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    struct OutBatch(u64);
    impl BatchSize for OutBatch {
        fn batch_size(&self) -> usize {
            1
        }
    }

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

        assert_eq!(erased.input_type(), TypeId::of::<InBatch>());
        assert_eq!(erased.output_type(), TypeId::of::<OutBatch>());
        assert_ne!(erased.input_type(), erased.output_type());

        assert_eq!(erased.execution_class(), ExecutionClass::CpuOnly);
        assert_eq!(erased.fallback_kind(), FallbackKind::None);

        let input: Box<dyn TypedBatch> = Box::new(InBatch(21));
        let mut scratch: Box<dyn AnyScratch> = Box::new(0u32);
        let out = erased
            .process_any(input.as_ref(), scratch.as_mut())
            .expect("process_any should succeed");

        assert_eq!(out.as_any().type_id(), TypeId::of::<OutBatch>());
        let out = out
            .as_any()
            .downcast_ref::<OutBatch>()
            .expect("output downcasts to OutBatch");
        assert_eq!(*out, OutBatch(42));

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

    struct StatefulScratch(u64);
    #[allow(clippy::derivable_impls)]
    impl Default for StatefulScratch {
        fn default() -> Self {
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
            Ok(OutBatch(scratch.0))
        }
        fn execution_class(&self) -> ExecutionClass {
            ExecutionClass::CpuOnly
        }
    }

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

    #[test]
    fn test_unit_scratch_fallback_still_runs() {
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

    #[test]
    fn test_prefers_soa_default_returns_true() {
        assert!(Doubler.prefers_soa());
    }

    #[test]
    fn test_anystage_default_method_impls() {
        struct MinimalAny;
        impl AnyStage for MinimalAny {
            fn input_type(&self) -> std::any::TypeId {
                std::any::TypeId::of::<InBatch>()
            }
            fn output_type(&self) -> std::any::TypeId {
                std::any::TypeId::of::<OutBatch>()
            }
            fn execution_class(&self) -> ExecutionClass {
                ExecutionClass::CpuOnly
            }
            fn fallback_kind(&self) -> FallbackKind {
                FallbackKind::None
            }
            fn process_any(
                &self,
                _: &dyn TypedBatch,
                _: &mut dyn AnyScratch,
            ) -> Result<Box<dyn TypedBatch>, StageError> {
                Ok(Box::new(OutBatch(0)))
            }
        }

        let m = MinimalAny;
        assert!(m.stage_as_any().is_none());
        let mut scratch = m.default_scratch();
        assert!((*scratch).as_any_mut().downcast_mut::<()>().is_some());
        assert_eq!(m.name(), "unnamed-stage");
        let input: Box<dyn TypedBatch> = Box::new(InBatch(0));
        assert!(m
            .cpu_fallback_process_any(input.as_ref(), scratch.as_mut())
            .is_none());
    }

    #[test]
    fn test_fallback_kind_registered_for_stage_with_fallback() {
        struct UnitFb2;
        impl Stage<InBatch, OutBatch> for UnitFb2 {
            type Scratch = ();
            type CpuFallback = Self;
            fn process(&self, i: &InBatch, _: &mut ()) -> Result<OutBatch, StageError> {
                Ok(OutBatch(i.0))
            }
            fn execution_class(&self) -> ExecutionClass {
                ExecutionClass::CpuOnly
            }
        }
        struct GpuWithFb {
            fb: UnitFb2,
        }
        impl Stage<InBatch, OutBatch> for GpuWithFb {
            type Scratch = ();
            type CpuFallback = UnitFb2;
            fn process(&self, _: &InBatch, _: &mut ()) -> Result<OutBatch, StageError> {
                Ok(OutBatch(0))
            }
            fn execution_class(&self) -> ExecutionClass {
                ExecutionClass::GpuOnly
            }
            fn cpu_fallback(&self) -> Option<&UnitFb2> {
                Some(&self.fb)
            }
        }
        let erased: Box<dyn AnyStage> = erase(GpuWithFb { fb: UnitFb2 });
        assert_eq!(erased.fallback_kind(), FallbackKind::Registered);
    }

    #[test]
    fn test_process_any_scratch_type_mismatch() {
        let erased: Box<dyn AnyStage> = erase(Doubler);
        let input: Box<dyn TypedBatch> = Box::new(InBatch(5));
        let mut wrong_scratch: Box<dyn AnyScratch> = Box::new(0u64);
        let result = erased.process_any(input.as_ref(), wrong_scratch.as_mut());
        assert!(
            matches!(result, Err(StageError::TypeMismatch { .. })),
            "expected TypeMismatch for wrong scratch type"
        );
    }

    #[test]
    fn test_cpu_fallback_process_any_type_mismatch_on_wrong_input() {
        struct UnitFb3;
        impl Stage<InBatch, OutBatch> for UnitFb3 {
            type Scratch = ();
            type CpuFallback = Self;
            fn process(&self, i: &InBatch, _: &mut ()) -> Result<OutBatch, StageError> {
                Ok(OutBatch(i.0))
            }
            fn execution_class(&self) -> ExecutionClass {
                ExecutionClass::CpuOnly
            }
        }
        struct GpuWithUnitFb3 {
            fb: UnitFb3,
        }
        impl Stage<InBatch, OutBatch> for GpuWithUnitFb3 {
            type Scratch = ();
            type CpuFallback = UnitFb3;
            fn process(&self, _: &InBatch, _: &mut ()) -> Result<OutBatch, StageError> {
                Ok(OutBatch(0))
            }
            fn execution_class(&self) -> ExecutionClass {
                ExecutionClass::GpuOnly
            }
            fn cpu_fallback(&self) -> Option<&UnitFb3> {
                Some(&self.fb)
            }
        }
        let erased: Box<dyn AnyStage> = erase(GpuWithUnitFb3 { fb: UnitFb3 });
        let wrong_input: Box<dyn TypedBatch> = Box::new(OutBatch(99));
        let mut scratch = erased.default_scratch();
        let result = erased
            .cpu_fallback_process_any(wrong_input.as_ref(), scratch.as_mut())
            .expect("fallback IS registered");
        assert!(
            matches!(result, Err(StageError::TypeMismatch { .. })),
            "wrong input type must produce TypeMismatch"
        );
    }
}
