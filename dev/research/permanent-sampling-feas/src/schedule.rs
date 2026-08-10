//! Registry-derived scheduling for prototype candidates.
//!
//! The prototype crate owns the candidate list. This adapter is the one place
//! that binds those paths to the harness's named sampler purposes, so callers
//! never manufacture stream offsets or maintain a parallel candidate list.

use crate::backend::Backend;
use crate::sampler::MeasurementPurpose;

/// The harness operation for which a scheduled backend needs a sampler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulePhase {
    /// Cross-backend agreement on shared matrices.
    Equivalence,
    /// One-matrix calibration for a grid cell.
    GridProbe,
    /// Untimed repetitions before a grid cell.
    GridWarmup,
    /// Timed repetitions for a grid cell.
    GridTimed,
    /// Long-running throughput observation.
    Sustained,
}

impl SchedulePhase {
    const fn purpose(self) -> MeasurementPurpose {
        match self {
            Self::Equivalence => MeasurementPurpose::Equivalence,
            Self::GridProbe => MeasurementPurpose::GridProbe,
            Self::GridWarmup => MeasurementPurpose::GridWarmup,
            Self::GridTimed => MeasurementPurpose::GridTimed,
            Self::Sustained => MeasurementPurpose::Sustained,
        }
    }
}

/// One backend together with the sampler purpose assigned by the harness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScheduledBackend {
    backend: Backend,
    purpose: MeasurementPurpose,
}

impl ScheduledBackend {
    /// The backend selected from the canonical schedule.
    #[must_use]
    pub const fn backend(self) -> Backend {
        self.backend
    }

    /// The named sampler purpose bound at scheduling time.
    #[must_use]
    pub const fn purpose(self) -> MeasurementPurpose {
        self.purpose
    }
}

/// Derive the complete harness schedule and bind it to `phase`'s stream domain.
///
/// [`Backend::ALL`] includes the prototype paths by mapping the prototype
/// registry directly. This function is consequently the only call-site
/// surface that attaches a [`MeasurementPurpose`] to a candidate.
pub fn scheduled_backends(phase: SchedulePhase) -> impl Iterator<Item = ScheduledBackend> {
    let purpose = phase.purpose();
    Backend::ALL
        .into_iter()
        .map(move |backend| ScheduledBackend { backend, purpose })
}

/// Look up one scheduled backend and its harness-assigned sampler purpose.
///
/// # Panics
///
/// Panics if `backend` is not part of the canonical schedule. That is an
/// internal invariant: timing and equivalence code must not invent a backend
/// outside the registry-derived schedule.
#[must_use]
pub fn scheduled_backend(backend: Backend, phase: SchedulePhase) -> ScheduledBackend {
    scheduled_backends(phase)
        .find(|scheduled| scheduled.backend == backend)
        .expect("every timed or equivalence backend must be in the canonical schedule")
}

#[cfg(all(test, feature = "prototype-registry"))]
mod tests {
    use super::*;
    use permanent_wave_gpu::MeasurementPath;
    use std::collections::BTreeSet;

    #[test]
    fn scheduled_prototype_paths_equal_the_registry() {
        let scheduled = scheduled_backends(SchedulePhase::GridTimed)
            .filter_map(|scheduled| scheduled.backend().prototype_path())
            .map(MeasurementPath::name)
            .collect::<BTreeSet<_>>();
        let registered = MeasurementPath::ALL
            .into_iter()
            .map(MeasurementPath::name)
            .collect::<BTreeSet<_>>();

        assert_eq!(scheduled, registered);
    }

    #[test]
    fn adapter_binds_named_purposes_for_each_schedule_phase() {
        let cases = [
            (SchedulePhase::Equivalence, MeasurementPurpose::Equivalence),
            (SchedulePhase::GridProbe, MeasurementPurpose::GridProbe),
            (SchedulePhase::GridWarmup, MeasurementPurpose::GridWarmup),
            (SchedulePhase::GridTimed, MeasurementPurpose::GridTimed),
            (SchedulePhase::Sustained, MeasurementPurpose::Sustained),
        ];

        for (phase, purpose) in cases {
            for scheduled in scheduled_backends(phase) {
                assert_eq!(scheduled.purpose(), purpose);
            }
        }
    }

    #[test]
    fn every_currently_unevaluable_path_is_explicitly_unsupported() {
        for scheduled in scheduled_backends(SchedulePhase::GridTimed) {
            let Some(path) = scheduled.backend().prototype_path() else {
                continue;
            };
            match crate::backend::support(scheduled.backend(), 3, 12) {
                crate::backend::Support::Supported => {
                    panic!(
                        "{} lacks a harness evaluator but was scheduled as supported",
                        path.name()
                    )
                }
                crate::backend::Support::Unsupported(reason) => {
                    assert!(
                        !reason.is_empty(),
                        "{} must retain its unsupported reason",
                        path.name()
                    );
                }
            }
        }
    }
}
