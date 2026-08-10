//! Backend selection and batch evaluation for the permanent kernels.
//!
//! Every backend consumes already-constructed packed matrices and returns one
//! permanent value per matrix, so a cell's zero count is exact rather than
//! aggregated on the device. Matrix construction (`from_row_major`) is charged
//! to the generation phase, not to evaluation; see [`crate::protocol`].
//!
//! # Forcing a specific CPU path
//!
//! `gf2_algebra::permanent::permanent_bipedal3` dispatches internally: with the
//! `simd` feature active and AVX2 detected at runtime it calls
//! `permanent_bipedal3_singleword_simd`, otherwise `permanent_bipedal3_singleword`.
//! The study needs both timed separately, so this module never calls the public
//! dispatcher for F_3 — it calls the two single-word entry points directly.
//! F_5 and F_7 have no SIMD permanent path in `gf2-algebra`, so their scalar
//! entry points are the only CPU option.

use gf2_algebra::packed::bipedal3::Bipedal3Matrix;
use gf2_algebra::packed::packed5::Packed5Matrix;
use gf2_algebra::packed::packed7::Packed7Matrix;
use gf2_algebra::permanent::bipedal3::permanent_bipedal3_singleword;
use gf2_algebra::permanent::bipedal5::permanent_bipedal5;
use gf2_algebra::permanent::bipedal7::permanent_bipedal7;
use gf2_algebra::permanent::permanent_bipedal3_parallel;
use gf2_algebra::permanent::permanent_ryser;
use gf2_core::gfp::Fp;
#[cfg(feature = "prototype-registry")]
use permanent_wave_gpu::MeasurementPath;
use rayon::prelude::*;

/// Phase spans returned by an event-instrumented GPU permanent dispatch.
///
/// Device-event durations and the host duration of submission remain separate:
/// no field is calculated by combining timestamps from those two clocks. A
/// non-GPU backend returns [`PhaseTiming::NotApplicable`] from
/// [`evaluate_timed`] instead.
#[derive(Clone, Copy, Debug)]
pub struct GpuPhaseTimings {
    /// Device-clock host-to-device copy span.
    pub h2d: std::time::Duration,
    /// Device-clock kernel-only span.
    pub kernel: std::time::Duration,
    /// Device-clock device-to-host copy span.
    pub d2h: std::time::Duration,
    /// Host-clock duration of the submission wrapper call.
    pub host_submission: std::time::Duration,
    /// Device-clock interval from submission marker to kernel start marker.
    pub device_submission_to_kernel: std::time::Duration,
}

/// Whether one evaluation supplied event-measured GPU phase timings.
#[derive(Clone, Debug)]
pub enum PhaseTiming {
    /// Device and host-submission spans from the instrumented GPU boundary.
    Measured(GpuPhaseTimings),
    /// Instrumentation failed, but the same synchronous GPU backend returned
    /// values. The narrative names the failed timing boundary rather than
    /// presenting its evaluator wall clock as a device measurement.
    Unavailable(String),
    /// The backend has no GPU timing boundary.
    NotApplicable,
}

/// Values and the availability of per-phase GPU timing data.
pub struct TimedEvaluation {
    /// One canonical permanent value for every input matrix, in input order.
    pub values: Vec<u64>,
    /// Event timing outcome, which never represents a host evaluation wall
    /// clock as a device duration.
    pub phase_timing: PhaseTiming,
}

/// The measured evaluation paths.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Backend {
    /// Single thread, pure-Rust single-word kernel.
    Scalar,
    /// Single thread, AVX2 single-word kernel (F_3 only).
    Avx2,
    /// Rayon across matrices, scalar kernel per matrix.
    Rayon,
    /// Rayon across matrices, AVX2 kernel per matrix (F_3 only).
    RayonAvx2,
    /// The in-tree `permanent_bipedal3_parallel`: rayon *inside* one matrix's
    /// Gray-code walk, matrices processed one at a time (F_3 only).
    RayonIntra,
    /// HIP/ROCm batch dispatcher.
    Gpu,
    /// The generic `permanent_ryser<F: FiniteField>` over unpacked `Fp<q>`
    /// elements: single thread, no packing, no field-specific kernel.
    ///
    /// Included because it is an *applicable* in-tree path for every field at
    /// every `n <= 63`, which the packed kernels are not — `permanent_bipedal7`
    /// stops at `n = 16`, so this is the only CPU path that evaluates an
    /// F_7 permanent above that size. Its rustdoc describes it as intended for
    /// cross-checks, but that is a statement about intent: the function is a
    /// complete Ryser evaluation and returns the same value the packed kernels
    /// do, which the equivalence check verifies.
    RyserGeneric,
    /// A candidate reached directly from the wave-prototype registry.
    #[cfg(feature = "prototype-registry")]
    Prototype(MeasurementPath),
}

impl Backend {
    /// The harness-native backends, in their stable measurement order.
    ///
    /// Prototype candidates deliberately do not appear here. They are derived
    /// from `MeasurementPath::ALL` by [`Self::ALL`], preventing a second
    /// hand-maintained candidate list in this crate.
    pub const BUILTIN: [Backend; 7] = [
        Backend::Scalar,
        Backend::Avx2,
        Backend::Rayon,
        Backend::RayonAvx2,
        Backend::RayonIntra,
        Backend::Gpu,
        Backend::RyserGeneric,
    ];

    /// Every backend, including every registered prototype path.
    #[cfg(feature = "prototype-registry")]
    pub const ALL: BackendSchedule = BackendSchedule;
    /// Every harness-native backend when the prototype registry feature is
    /// deliberately disabled.
    #[cfg(not(feature = "prototype-registry"))]
    pub const ALL: [Backend; 7] = Self::BUILTIN;

    /// Stable identifier used in the CSV `backend` column.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Backend::Scalar => "cpu_scalar",
            Backend::Avx2 => "cpu_avx2",
            Backend::Rayon => "cpu_rayon_batch_scalar",
            Backend::RayonAvx2 => "cpu_rayon_batch_avx2",
            Backend::RayonIntra => "cpu_rayon_intra_matrix",
            Backend::Gpu => "gpu_hip",
            Backend::RyserGeneric => "cpu_ryser_generic",
            #[cfg(feature = "prototype-registry")]
            Backend::Prototype(path) => path.name(),
        }
    }

    /// Whether the backend uses every core (affects thread pinning and the
    /// reported thread count).
    #[must_use]
    pub fn is_multithreaded(self) -> bool {
        matches!(
            self,
            Backend::Rayon | Backend::RayonAvx2 | Backend::RayonIntra
        )
    }

    /// The registry path behind this backend, if it is a prototype candidate.
    #[cfg(feature = "prototype-registry")]
    #[must_use]
    pub const fn prototype_path(self) -> Option<MeasurementPath> {
        match self {
            Self::Prototype(path) => Some(path),
            _ => None,
        }
    }
}

/// Iterator source for the canonical default backend schedule.
///
/// It appends the prototype crate's registry directly to the harness-native
/// paths, rather than restating any prototype candidate here.
#[cfg(feature = "prototype-registry")]
#[derive(Clone, Copy, Debug)]
pub struct BackendSchedule;

#[cfg(feature = "prototype-registry")]
impl BackendSchedule {
    /// Number of paths in the complete schedule.
    #[must_use]
    pub const fn len(self) -> usize {
        Backend::BUILTIN.len() + MeasurementPath::ALL.len()
    }

    /// Whether the complete schedule has no paths.
    ///
    /// The harness-native schedule is nonempty even before prototype paths are
    /// appended, so this is always `false`.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        false
    }
}

#[cfg(feature = "prototype-registry")]
impl IntoIterator for BackendSchedule {
    type Item = Backend;
    type IntoIter = BackendScheduleIter;

    fn into_iter(self) -> Self::IntoIter {
        BackendScheduleIter { index: 0 }
    }
}

/// Iterator over native backends followed by the prototype registry.
///
/// The registry half deliberately indexes `MeasurementPath::ALL` directly:
/// changing that array's size or contents requires no schedule edit here.
#[cfg(feature = "prototype-registry")]
#[derive(Clone, Debug)]
pub struct BackendScheduleIter {
    index: usize,
}

#[cfg(feature = "prototype-registry")]
impl Iterator for BackendScheduleIter {
    type Item = Backend;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(backend) = Backend::BUILTIN.get(self.index).copied() {
            self.index = self.index.checked_add(1)?;
            return Some(backend);
        }

        let registry_index = self.index.checked_sub(Backend::BUILTIN.len())?;
        let path = MeasurementPath::ALL.get(registry_index).copied()?;
        self.index = self.index.checked_add(1)?;
        Some(Backend::Prototype(path))
    }
}

/// Whether a `(backend, q, n)` cell can run at all, and if not, why.
///
/// A cell that is `Unsupported` is recorded with its reason rather than
/// omitted, so the study's grid has no silent holes.
#[derive(Clone, Debug)]
pub enum Support {
    Supported,
    Unsupported(String),
}

/// Classify a `(backend, q, n)` cell against the kernels' declared bounds.
///
/// The bounds are read from the kernels themselves:
/// `permanent_bipedal3_singleword` and `permanent_bipedal5` assert `n <= 63`,
/// `permanent_bipedal7` asserts `n <= Packed7::LANES = 16`, and the GPU
/// dispatchers assert `1 <= n <= 63`.
#[must_use]
pub fn support(backend: Backend, q: u64, n: usize) -> Support {
    let unsupported = |s: String| Support::Unsupported(s);
    match backend {
        Backend::Scalar | Backend::Rayon => match q {
            3 | 5 => {
                if n <= 63 {
                    Support::Supported
                } else {
                    unsupported(format!("single-word F_{q} kernel asserts n <= 63; n = {n}"))
                }
            }
            7 => {
                if n <= 16 {
                    Support::Supported
                } else {
                    unsupported(format!(
                        "permanent_bipedal7 asserts n <= Packed7::LANES = 16; n = {n}"
                    ))
                }
            }
            _ => unsupported(format!("no CPU kernel for q = {q}")),
        },
        Backend::Avx2 | Backend::RayonAvx2 => {
            if q != 3 {
                unsupported(format!(
                    "gf2-algebra exposes no AVX2 permanent path for F_{q}"
                ))
            } else if n <= 63 {
                Support::Supported
            } else {
                unsupported(format!(
                    "permanent_bipedal3_singleword_simd asserts n <= 63; n = {n}"
                ))
            }
        }
        Backend::RayonIntra => {
            if q != 3 {
                unsupported(format!(
                    "gf2-algebra exposes no rayon permanent path for F_{q}"
                ))
            } else if n <= 63 {
                Support::Supported
            } else {
                unsupported(format!(
                    "permanent_bipedal3_parallel asserts n <= 63; n = {n}"
                ))
            }
        }
        Backend::RyserGeneric => {
            if !matches!(q, 3 | 5 | 7) {
                unsupported(format!("no Fp<{q}> sampler for the generic path"))
            } else if n <= 63 {
                Support::Supported
            } else {
                unsupported(format!("permanent_ryser asserts n <= 63; n = {n}"))
            }
        }
        Backend::Gpu => {
            if !cfg!(feature = "hip") {
                unsupported("built without the `hip` feature".to_string())
            } else if !(1..=63).contains(&n) {
                unsupported(format!("GPU dispatcher asserts 1 <= n <= 63; n = {n}"))
            } else if !matches!(q, 3 | 5 | 7) {
                unsupported(format!("no GPU kernel for q = {q}"))
            } else {
                Support::Supported
            }
        }
        #[cfg(feature = "prototype-registry")]
        Backend::Prototype(path) => match path.dispatch() {
            Ok(()) => unsupported(format!(
                "prototype candidate {} has no harness batch evaluator yet",
                path.name()
            )),
            Err(reason) => unsupported(reason.reason().to_string()),
        },
    }
}

/// A batch of packed matrices for one field order.
pub enum Batch {
    F3(Vec<Bipedal3Matrix>),
    F5(Vec<Packed5Matrix>),
    F7(Vec<Packed7Matrix>),
    /// Unpacked row-major matrices for [`Backend::RyserGeneric`], carrying `n`
    /// because the generic kernel takes it as an argument. These are what the
    /// sampler emits before any packed constructor runs, so a cell on this
    /// backend is not charged for packing it never uses.
    RawF3(usize, Vec<Vec<Fp<3>>>),
    RawF5(usize, Vec<Vec<Fp<5>>>),
    RawF7(usize, Vec<Vec<Fp<7>>>),
}

impl Batch {
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Batch::F3(v) => v.len(),
            Batch::F5(v) => v.len(),
            Batch::F7(v) => v.len(),
            Batch::RawF3(_, v) => v.len(),
            Batch::RawF5(_, v) => v.len(),
            Batch::RawF7(_, v) => v.len(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The AVX2 kernel bundle, or `None` on a host without AVX2.
///
/// Detection is cached upstream in `gf2_kernels_simd::bipedal::detect_avx2`;
/// this wrapper exists so callers can report "AVX2 absent" as a cell outcome.
#[must_use]
pub fn avx2_fns() -> Option<gf2_kernels_simd::bipedal::BipedalAvx2Fns> {
    gf2_kernels_simd::bipedal::detect_avx2()
}

/// Evaluate every matrix in `batch` on `backend`, returning one canonical
/// permanent value per matrix in input order.
///
/// # Panics
///
/// Panics if the cell is unsupported (call [`support`] first), or if the AVX2
/// backend is requested on a host where `detect_avx2` returns `None`.
pub fn evaluate(backend: Backend, batch: &Batch) -> Vec<u64> {
    match (backend, batch) {
        (Backend::Scalar, Batch::F3(v)) => v
            .iter()
            .map(|m| permanent_bipedal3_singleword(m).value())
            .collect(),
        (Backend::Scalar, Batch::F5(v)) => {
            v.iter().map(|m| permanent_bipedal5(m).value()).collect()
        }
        (Backend::Scalar, Batch::F7(v)) => {
            v.iter().map(|m| permanent_bipedal7(m).value()).collect()
        }

        (Backend::Avx2, Batch::F3(v)) => {
            let fns = avx2_fns().expect("cpu_avx2 backend requires AVX2 at runtime");
            v.iter()
                .map(|m| {
                    gf2_algebra::permanent::bipedal3::permanent_bipedal3_singleword_simd(m, &fns)
                        .value()
                })
                .collect()
        }

        (Backend::Rayon, Batch::F3(v)) => v
            .par_iter()
            .map(|m| permanent_bipedal3_singleword(m).value())
            .collect(),
        (Backend::Rayon, Batch::F5(v)) => v
            .par_iter()
            .map(|m| permanent_bipedal5(m).value())
            .collect(),
        (Backend::Rayon, Batch::F7(v)) => v
            .par_iter()
            .map(|m| permanent_bipedal7(m).value())
            .collect(),

        (Backend::RayonAvx2, Batch::F3(v)) => {
            let fns = avx2_fns().expect("cpu_rayon_batch_avx2 backend requires AVX2 at runtime");
            v.par_iter()
                .map(|m| {
                    gf2_algebra::permanent::bipedal3::permanent_bipedal3_singleword_simd(m, &fns)
                        .value()
                })
                .collect()
        }

        (Backend::RayonIntra, Batch::F3(v)) => v
            .iter()
            .map(|m| permanent_bipedal3_parallel(m).value())
            .collect(),

        (Backend::RyserGeneric, Batch::RawF3(n, v)) => {
            v.iter().map(|m| permanent_ryser(m, *n).value()).collect()
        }
        (Backend::RyserGeneric, Batch::RawF5(n, v)) => {
            v.iter().map(|m| permanent_ryser(m, *n).value()).collect()
        }
        (Backend::RyserGeneric, Batch::RawF7(n, v)) => {
            v.iter().map(|m| permanent_ryser(m, *n).value()).collect()
        }

        #[cfg(feature = "hip")]
        (Backend::Gpu, Batch::F3(v)) => gf2_algebra::gpu::permanent_batch_bipedal3(v)
            .into_iter()
            .map(|x| x.value())
            .collect(),
        #[cfg(feature = "hip")]
        (Backend::Gpu, Batch::F5(v)) => gf2_algebra::gpu::permanent_batch_bipedal5(v)
            .into_iter()
            .map(|x| x.value())
            .collect(),
        #[cfg(feature = "hip")]
        (Backend::Gpu, Batch::F7(v)) => gf2_algebra::gpu::permanent_batch_bipedal7(v)
            .into_iter()
            .map(|x| x.value())
            .collect(),

        (b, batch) => panic!(
            "unsupported cell: backend {} with a q-batch of {} matrices",
            b.name(),
            batch.len()
        ),
    }
}

/// Evaluate a batch and retain the instrumented HIP phase spans when present.
///
/// CPU backends deliberately return no phase spans: their evaluation wall
/// clock is not presented as a device measurement. The GPU path uses the
/// stream-local instrumented permanent boundary, whose kernel span excludes
/// allocation, transfer, and host serialisation.
///
/// # Panics
///
/// Propagates the selected backend's input and runtime failures. An
/// instrumentation setup or dispatch failure falls back to the synchronous
/// `gpu_hip` evaluator and returns [`PhaseTiming::Unavailable`]; a failure of
/// that actual backend evaluation remains explicit.
pub fn evaluate_timed(backend: Backend, batch: &Batch) -> TimedEvaluation {
    #[cfg(feature = "hip")]
    if backend == Backend::Gpu {
        return timing_or_same_backend_values(evaluate_gpu_instrumented(batch), || {
            evaluate(backend, batch)
        });
    }

    TimedEvaluation {
        values: evaluate(backend, batch),
        phase_timing: PhaseTiming::NotApplicable,
    }
}

#[cfg(feature = "hip")]
fn evaluate_gpu_instrumented(batch: &Batch) -> Result<TimedEvaluation, String> {
    use gf2_algebra::gpu::{
        initialise_permanent_gf7_luts, serialise_permanent_bipedal3, serialise_permanent_packed5,
        serialise_permanent_packed7,
    };
    use gf2_kernels_hip::host::HipStream;
    use gf2_kernels_hip::permanent::{dispatch_permanent_batch_instrumented, PermanentField};

    let (field, n, host_matrices) = match batch {
        Batch::F3(matrices) => {
            let (host_matrices, n) = serialise_permanent_bipedal3(matrices);
            (PermanentField::F3, n, host_matrices)
        }
        Batch::F5(matrices) => {
            let (host_matrices, n) = serialise_permanent_packed5(matrices);
            (PermanentField::F5, n, host_matrices)
        }
        Batch::F7(matrices) => {
            initialise_permanent_gf7_luts()
                .map_err(|rc| format!("F_7 LUT timing setup returned HIP error {rc}"))?;
            let (host_matrices, n) = serialise_permanent_packed7(matrices);
            (PermanentField::F7, n, host_matrices)
        }
        _ => return Err("GPU timing requires a packed F_3, F_5, or F_7 batch".to_string()),
    };

    let stream = HipStream::new()
        .map_err(|error| format!("create instrumented permanent stream: {error}"))?;
    let dispatch =
        dispatch_permanent_batch_instrumented(field, &host_matrices, n, batch.len(), &stream)
            .map_err(|error| format!("start instrumented permanent dispatch: {error}"))?;
    let (values, timings) = dispatch
        .finish()
        .map_err(|error| format!("finish instrumented permanent dispatch: {error}"))?;
    let timings = GpuPhaseTimings {
        h2d: timings
            .h2d
            .ok_or_else(|| "instrumented permanent dispatch omitted H2D timing".to_string())?,
        kernel: timings
            .kernel
            .ok_or_else(|| "instrumented permanent dispatch omitted kernel timing".to_string())?,
        d2h: timings
            .d2h
            .ok_or_else(|| "instrumented permanent dispatch omitted D2H timing".to_string())?,
        host_submission: timings.host_submission,
        device_submission_to_kernel: timings.device_submission_to_kernel.ok_or_else(|| {
            "instrumented permanent dispatch omitted submission-to-kernel timing".to_string()
        })?,
    };
    Ok(TimedEvaluation {
        values,
        phase_timing: PhaseTiming::Measured(timings),
    })
}

#[cfg(any(feature = "hip", test))]
fn timing_or_same_backend_values(
    instrumented: Result<TimedEvaluation, String>,
    fallback_values: impl FnOnce() -> Vec<u64>,
) -> TimedEvaluation {
    match instrumented {
        Ok(evaluation) => evaluation,
        Err(reason) => TimedEvaluation {
            values: fallback_values(),
            phase_timing: PhaseTiming::Unavailable(format!(
                "event timing unavailable: {reason}; values came from synchronous gpu_hip dispatch"
            )),
        },
    }
}

/// Count the matrices in `values` whose permanent is zero.
#[must_use]
pub fn count_zeros(values: &[u64]) -> u64 {
    values.iter().filter(|&&v| v == 0).count() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_failure_keeps_same_backend_values_with_a_reason() {
        let evaluation = timing_or_same_backend_values(
            Err("create instrumented permanent stream: no HIP device".to_string()),
            || vec![2, 0, 1],
        );

        assert_eq!(evaluation.values, vec![2, 0, 1]);
        match evaluation.phase_timing {
            PhaseTiming::Unavailable(reason) => {
                assert!(reason.contains("create instrumented permanent stream"));
                assert!(reason.contains("synchronous gpu_hip dispatch"));
            }
            PhaseTiming::Measured(_) | PhaseTiming::NotApplicable => {
                panic!("timing failure must remain unavailable")
            }
        }
    }
}
