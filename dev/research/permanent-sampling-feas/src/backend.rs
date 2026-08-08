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
use rayon::prelude::*;

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
}

impl Backend {
    /// Every backend, in the order the grid enumerates them.
    pub const ALL: [Backend; 7] = [
        Backend::Scalar,
        Backend::Avx2,
        Backend::Rayon,
        Backend::RayonAvx2,
        Backend::RayonIntra,
        Backend::Gpu,
        Backend::RyserGeneric,
    ];

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

/// Count the matrices in `values` whose permanent is zero.
#[must_use]
pub fn count_zeros(values: &[u64]) -> u64 {
    values.iter().filter(|&&v| v == 0).count() as u64
}
