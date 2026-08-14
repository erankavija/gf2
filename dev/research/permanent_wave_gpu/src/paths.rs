//! The complete measurement-path registry for this study.

use crate::device_batch::DeviceBatchKernel;
#[cfg(feature = "fixture-oracle")]
use crate::fixtures::Fixture;
use crate::{f5_candidates, f7_three_plane, fold_gf3, wave, wave_gf7};

/// The device batch kernel a candidate owns, or why it has none.
pub type DeviceBatchResult = Result<DeviceBatchKernel, String>;

/// One candidate permanent result, represented by its canonical field value.
///
/// The fixture itself retains the field order, so candidate modules must
/// return the representative in `0..q`.  The oracle compares this value only
/// with the matching field fixture.
#[cfg(feature = "fixture-oracle")]
pub type EvaluationResult = Result<u64, Unsupported>;

/// A registered candidate that does not yet have an executable implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Unsupported {
    reason: &'static str,
}

impl Unsupported {
    pub(crate) const fn new(reason: &'static str) -> Self {
        Self { reason }
    }

    /// Explains why this registered path cannot execute yet.
    #[must_use]
    pub const fn reason(self) -> &'static str {
        self.reason
    }
}

/// A candidate measurement path in the permanent wave study.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeasurementPath {
    /// F_3 wave-cooperative Ryser control.
    WaveGf3,
    /// F_3 zero-mask and sign-popcount fold.
    FoldGf3,
    /// F_5 byte-oriented modular arithmetic representation control.
    F5ByteControl,
    /// F_5 canonical three-plane accumulator.
    F5ThreePlane,
    /// F_7 three-plane Mersenne accumulator.
    F7ThreePlaneAccumulator,
    /// F_7 permanent-shaped lookup-table arithmetic representation control.
    F7LookupTableControl,
    /// F_7 permanent-shaped three-plane kernel.
    F7ThreePlanePermanent,
}

impl MeasurementPath {
    /// Every planned study path, in stable measurement order.
    pub const ALL: [Self; 7] = [
        Self::WaveGf3,
        Self::FoldGf3,
        Self::F5ByteControl,
        Self::F5ThreePlane,
        Self::F7ThreePlaneAccumulator,
        Self::F7LookupTableControl,
        Self::F7ThreePlanePermanent,
    ];

    /// Stable registry name used by the measurement driver and receipts.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::WaveGf3 => "wave-gf3",
            Self::FoldGf3 => "fold-gf3",
            Self::F5ByteControl => "f5-byte-control",
            Self::F5ThreePlane => "f5-three-plane",
            Self::F7ThreePlaneAccumulator => "f7-three-plane-accumulator",
            Self::F7LookupTableControl => "f7-lookup-table-control",
            Self::F7ThreePlanePermanent => "f7-three-plane-permanent",
        }
    }

    /// The device batch kernel this candidate owns, or why it has none.
    ///
    /// Each path answers from its dedicated candidate module, so a later
    /// implementation lands beside its own kernel rather than in this registry.
    /// The answer describes the committed device sources and is therefore the
    /// same in every build; whether that kernel is *reachable* from this build
    /// and host is [`Self::prepare_batch_evaluator`]'s question.
    ///
    /// # Errors
    ///
    /// Returns the reason this candidate has no full-permanent batch kernel.
    pub fn device_batch_kernel(self) -> DeviceBatchResult {
        match self {
            Self::WaveGf3 => wave::device_batch_kernel(),
            Self::FoldGf3 => fold_gf3::device_batch_kernel(),
            Self::F5ByteControl => f5_candidates::byte_control_device_batch_kernel(),
            Self::F5ThreePlane => f5_candidates::three_plane_device_batch_kernel(),
            Self::F7ThreePlaneAccumulator => f7_three_plane::device_batch_kernel(),
            Self::F7LookupTableControl => wave_gf7::lookup_table_control_device_batch_kernel(),
            Self::F7ThreePlanePermanent => wave_gf7::three_plane_device_batch_kernel(),
        }
    }

    /// Evaluate one fixture through this registered candidate.
    ///
    /// This is the only candidate-to-oracle dispatch surface.  The fixture
    /// checker enumerates [`Self::ALL`] and calls this method, so adding a
    /// registered path automatically admits it to correctness reporting.
    #[cfg(feature = "fixture-oracle")]
    pub fn evaluate(self, fixture: &Fixture) -> EvaluationResult {
        match self {
            Self::WaveGf3 => wave::evaluate(fixture),
            Self::FoldGf3 => fold_gf3::evaluate(fixture),
            Self::F5ByteControl => f5_candidates::evaluate_byte_control(fixture),
            Self::F5ThreePlane => f5_candidates::evaluate_three_plane(fixture),
            Self::F7ThreePlaneAccumulator => f7_three_plane::evaluate(fixture),
            Self::F7LookupTableControl => wave_gf7::evaluate_lookup_table_control(fixture),
            Self::F7ThreePlanePermanent => wave_gf7::evaluate_three_plane(fixture),
        }
    }
}
