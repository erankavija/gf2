//! The complete measurement-path registry for this study.

use crate::{f5_candidates, f7_three_plane, fold_gf3, wave, wave_gf7};

/// Result of dispatching a candidate path.
pub type DispatchResult = Result<(), Unsupported>;

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
    /// F_5 byte control and three-plane representation comparison.
    CandidatesGf5,
    /// F_7 three-plane Mersenne accumulator.
    ThreePlaneGf7,
    /// F_7 permanent-shaped wave kernel comparison.
    WaveGf7,
}

impl MeasurementPath {
    /// Every planned study path, in stable measurement order.
    pub const ALL: [Self; 5] = [
        Self::WaveGf3,
        Self::FoldGf3,
        Self::CandidatesGf5,
        Self::ThreePlaneGf7,
        Self::WaveGf7,
    ];

    /// Stable registry name used by the measurement driver and receipts.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::WaveGf3 => "wave-gf3",
            Self::FoldGf3 => "fold-gf3",
            Self::CandidatesGf5 => "candidates-gf5",
            Self::ThreePlaneGf7 => "three-plane-gf7",
            Self::WaveGf7 => "wave-gf7",
        }
    }

    /// Dispatches this path to its dedicated candidate module.
    pub fn dispatch(self) -> DispatchResult {
        match self {
            Self::WaveGf3 => wave::run(),
            Self::FoldGf3 => fold_gf3::run(),
            Self::CandidatesGf5 => f5_candidates::run(),
            Self::ThreePlaneGf7 => f7_three_plane::run(),
            Self::WaveGf7 => wave_gf7::run(),
        }
    }
}
