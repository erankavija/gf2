//! The complete measurement-path registry for this study.

use crate::{f5_candidates, f7_three_plane, fixtures::Fixture, fold_gf3, wave, wave_gf7};

/// Result of dispatching a candidate path.
pub type DispatchResult = Result<(), Unsupported>;

/// One candidate permanent result, represented by its canonical field value.
///
/// The fixture itself retains the field order, so candidate modules must
/// return the representative in `0..q`.  The oracle compares this value only
/// with the matching field fixture.
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

    /// Dispatches this path to its dedicated candidate module.
    pub fn dispatch(self) -> DispatchResult {
        match self {
            Self::WaveGf3 => wave::run(),
            Self::FoldGf3 => fold_gf3::run(),
            Self::F5ByteControl => f5_candidates::byte_control(),
            Self::F5ThreePlane => f5_candidates::three_plane(),
            Self::F7ThreePlaneAccumulator => f7_three_plane::run(),
            Self::F7LookupTableControl => wave_gf7::lookup_table_control(),
            Self::F7ThreePlanePermanent => wave_gf7::three_plane(),
        }
    }

    /// Evaluate one fixture through this registered candidate.
    ///
    /// This is the only candidate-to-oracle dispatch surface.  The fixture
    /// checker enumerates [`Self::ALL`] and calls this method, so adding a
    /// registered path automatically admits it to correctness reporting.
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
