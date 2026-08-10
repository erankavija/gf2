//! Cross-backend agreement on shared inputs.
//!
//! `@/inv/backend-behavioral-equivalence` requires scalar, SIMD, parallel, and
//! GPU implementations to expose equivalent observable results. A throughput
//! number from a backend that computes a different permanent is worthless, so
//! this check runs before the grid and its outcome is recorded in the study.
//!
//! The comparison is per matrix, not per aggregate: the campaign counts zeros,
//! so agreement on a zero *count* while disagreeing on individual values would
//! still be a defect.

use crate::backend::{evaluate, evaluate_timed, support, Backend, Batch, PhaseTiming, Support};
use crate::sampler::{MatrixSampler, MeasurementPurpose};
use crate::schedule::{scheduled_backends, SchedulePhase};
use gf2_algebra::packed::bipedal3::Bipedal3Matrix;
use gf2_algebra::packed::packed5::Packed5Matrix;
use gf2_algebra::packed::packed7::Packed7Matrix;
use gf2_core::gfp::Fp;

/// One backend's agreement with the scalar reference at a given `(q, n)`.
#[derive(Clone, Debug)]
pub struct EquivalenceRow {
    pub q: u64,
    pub n: usize,
    pub reference: &'static str,
    pub backend: &'static str,
    pub matrices: usize,
    pub mismatches: usize,
    pub zeros_reference: u64,
    pub zeros_backend: u64,
    pub status: String,
}

/// CSV header for [`EquivalenceRow::to_csv_row`].
pub const EQUIVALENCE_CSV_HEADER: &str =
    "q,n,reference,backend,matrices,mismatches,zeros_reference,zeros_backend,status";

impl EquivalenceRow {
    #[must_use]
    pub fn to_csv_row(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{}",
            self.q,
            self.n,
            self.reference,
            self.backend,
            self.matrices,
            self.mismatches,
            self.zeros_reference,
            self.zeros_backend,
            self.status
        )
    }
}

/// Build the same `m` matrices as [`shared_batch`], unpacked, for the generic
/// Ryser path.
fn shared_raw_batch(
    q: u64,
    n: usize,
    m: usize,
    seed_root: u64,
    purpose: MeasurementPurpose,
) -> Batch {
    let mut s = MatrixSampler::new(seed_root, q, n, purpose, 0);
    match q {
        3 => Batch::RawF3(n, (0..m).map(|_| s.next_matrix::<3>(n)).collect()),
        5 => Batch::RawF5(n, (0..m).map(|_| s.next_matrix::<5>(n)).collect()),
        7 => Batch::RawF7(n, (0..m).map(|_| s.next_matrix::<7>(n)).collect()),
        _ => panic!("shared_raw_batch: unsupported q = {q}"),
    }
}

/// Build `m` matrices for `(q, n)` from the equivalence-check stream.
fn shared_batch(q: u64, n: usize, m: usize, seed_root: u64, purpose: MeasurementPurpose) -> Batch {
    // The schedule adapter binds this named purpose, keeping equivalence inputs
    // separate from timing cells without a numeric stream base at this call site.
    let mut s = MatrixSampler::new(seed_root, q, n, purpose, 0);
    match q {
        3 => Batch::F3(
            (0..m)
                .map(|_| Bipedal3Matrix::from_row_major(&s.next_matrix::<3>(n), n, n))
                .collect(),
        ),
        5 => Batch::F5(
            (0..m)
                .map(|_| Packed5Matrix::from_row_major(&s.next_matrix::<5>(n), n, n))
                .collect(),
        ),
        7 => Batch::F7(
            (0..m)
                .map(|_| Packed7Matrix::from_row_major(&s.next_matrix::<7>(n), n, n))
                .collect(),
        ),
        _ => panic!("shared_batch: unsupported q = {q}"),
    }
}

/// Check every backend against a reference kernel at `(q, n)`.
///
/// The reference is the scalar single-word kernel wherever it is supported. At
/// `q = 7, n > 16` that kernel does not exist — `permanent_bipedal7` asserts
/// `n <= Packed7::LANES` — so the generic `permanent_ryser` becomes the
/// reference instead, which is why those cells can be checked at all rather
/// than skipped for want of something to compare against. The `reference`
/// column records which one was used per row.
///
/// Returns one row per non-reference backend. Unsupported backends are
/// reported with their reason rather than dropped.
#[must_use]
pub fn check(q: u64, n: usize, m: usize, seed_root: u64) -> Vec<EquivalenceRow> {
    let mut rows = Vec::new();
    let purpose = scheduled_backends(SchedulePhase::Equivalence)
        .next()
        .expect("the canonical backend schedule is nonempty")
        .purpose();

    let reference_backend = match support(Backend::Scalar, q, n) {
        Support::Supported => Backend::Scalar,
        Support::Unsupported(scalar_reason) => match support(Backend::RyserGeneric, q, n) {
            Support::Supported => Backend::RyserGeneric,
            Support::Unsupported(generic_reason) => {
                rows.push(EquivalenceRow {
                    q,
                    n,
                    reference: Backend::Scalar.name(),
                    backend: "all",
                    matrices: 0,
                    mismatches: 0,
                    zeros_reference: 0,
                    zeros_backend: 0,
                    status: format!("skipped: no reference ({scalar_reason}; {generic_reason})"),
                });
                return rows;
            }
        },
    };

    let batch = shared_batch(q, n, m, seed_root, purpose);
    let raw_ref = shared_raw_batch(q, n, m, seed_root, purpose);
    let reference = evaluate(
        reference_backend,
        if reference_backend == Backend::RyserGeneric {
            &raw_ref
        } else {
            &batch
        },
    );
    let zeros_reference = crate::backend::count_zeros(&reference);

    // The generic path consumes unpacked matrices, so it needs its own batch.
    // Built from the same (seed_root, q, n, equivalence, index 0) tuple as `batch`, and the
    // sampler is deterministic, so both hold the same matrices in the same
    // order - which is what makes the per-matrix comparison meaningful.
    let raw = raw_ref;

    for scheduled in scheduled_backends(SchedulePhase::Equivalence) {
        let backend = scheduled.backend();
        if backend == reference_backend {
            continue;
        }
        debug_assert_eq!(scheduled.purpose(), purpose);
        let mut row = EquivalenceRow {
            q,
            n,
            reference: reference_backend.name(),
            backend: backend.name(),
            matrices: m,
            mismatches: 0,
            zeros_reference,
            zeros_backend: 0,
            status: String::new(),
        };
        match support(backend, q, n) {
            Support::Unsupported(reason) => {
                row.matrices = 0;
                row.status = format!("unsupported: {reason}");
            }
            Support::Supported => {
                if matches!(backend, Backend::Avx2 | Backend::RayonAvx2)
                    && crate::backend::avx2_fns().is_none()
                {
                    row.matrices = 0;
                    row.status = "unsupported: AVX2 not detected at runtime".to_string();
                } else {
                    let backend_batch = if backend == Backend::RyserGeneric {
                        &raw
                    } else {
                        &batch
                    };
                    let (got, timing_note) = if backend == Backend::Gpu {
                        let evaluation = evaluate_timed(backend, backend_batch);
                        let timing_note = match evaluation.phase_timing {
                            PhaseTiming::Unavailable(reason) => Some(reason),
                            PhaseTiming::Measured(_) | PhaseTiming::NotApplicable => None,
                        };
                        (evaluation.values, timing_note)
                    } else {
                        (evaluate(backend, backend_batch), None)
                    };
                    row.mismatches = reference
                        .iter()
                        .zip(got.iter())
                        .filter(|(a, b)| a != b)
                        .count();
                    row.zeros_backend = crate::backend::count_zeros(&got);
                    row.status = match (row.mismatches, timing_note) {
                        (0, Some(reason)) => format!("identical; {reason}"),
                        (0, None) => "identical".to_string(),
                        (_, _) => "MISMATCH".to_string(),
                    };
                }
            }
        }
        rows.push(row);
    }
    rows
}

/// Exact $\Pr[\mathrm{per} = 0]$ over all $q^{9}$ matrices of order 3, computed
/// two ways: once through the production kernel and once by an independent
/// six-term expansion of the $3 \times 3$ permanent.
///
/// The cross-backend check in [`check`] compares implementations of the *same*
/// per-field algorithm against each other. That cannot detect an error shared
/// by all of them, and the campaign's headline statistic is precisely a zero
/// count, so the kernels also need an anchor outside their own family. Order 3
/// is the largest size where full enumeration is instant for every supported
/// `q` ($7^9 \approx 4.0 \times 10^7$).
///
/// Returns `(kernel zero count, independent zero count, total matrices)`.
#[must_use]
pub fn exact_zero_count_order3(q: u64) -> (u64, u64, u64) {
    fn digits(mut m: u64, q: u64) -> [u64; 9] {
        let mut d = [0u64; 9];
        for slot in &mut d {
            *slot = m % q;
            m /= q;
        }
        d
    }
    // Independent permanent: sum over the six permutations of S_3.
    fn per3(d: &[u64; 9], q: u64) -> u64 {
        (d[0] * d[4] * d[8]
            + d[0] * d[5] * d[7]
            + d[1] * d[3] * d[8]
            + d[1] * d[5] * d[6]
            + d[2] * d[3] * d[7]
            + d[2] * d[4] * d[6])
            % q
    }

    let total = q.pow(9);
    let mut kernel_zeros = 0u64;
    let mut independent_zeros = 0u64;
    for m in 0..total {
        let d = digits(m, q);
        if per3(&d, q) == 0 {
            independent_zeros += 1;
        }
        let kernel_value = match q {
            3 => {
                let data: Vec<Fp<3>> = d.iter().map(|&x| Fp::<3>::new(x)).collect();
                gf2_algebra::permanent::bipedal3::permanent_bipedal3_singleword(
                    &Bipedal3Matrix::from_row_major(&data, 3, 3),
                )
                .value()
            }
            5 => {
                let data: Vec<Fp<5>> = d.iter().map(|&x| Fp::<5>::new(x)).collect();
                gf2_algebra::permanent::bipedal5::permanent_bipedal5(
                    &Packed5Matrix::from_row_major(&data, 3, 3),
                )
                .value()
            }
            7 => {
                let data: Vec<Fp<7>> = d.iter().map(|&x| Fp::<7>::new(x)).collect();
                gf2_algebra::permanent::bipedal7::permanent_bipedal7(
                    &Packed7Matrix::from_row_major(&data, 3, 3),
                )
                .value()
            }
            _ => panic!("exact_zero_count_order3: unsupported q = {q}"),
        };
        if kernel_value == 0 {
            kernel_zeros += 1;
        }
    }
    (kernel_zeros, independent_zeros, total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "prototype-registry")]
    use permanent_wave_gpu::MeasurementPath;

    #[cfg(feature = "prototype-registry")]
    #[test]
    fn every_registered_path_is_reported_by_equivalence() {
        let rows = check(3, 3, 2, 0xB488_F02C);
        for path in MeasurementPath::ALL {
            let row = rows
                .iter()
                .find(|row| row.backend == path.name())
                .unwrap_or_else(|| panic!("{} is missing from equivalence", path.name()));
            assert!(
                row.status.starts_with("unsupported: "),
                "{} status={}",
                path.name(),
                row.status
            );
        }
    }

    /// Sampling through the campaign's own sampler must recover the exact
    /// zero fraction at order 3, for every `q`.
    ///
    /// This is the end-to-end anchor. [`exact_zero_count_order3`] validates the
    /// kernels by enumeration, which bypasses the sampler entirely; the
    /// cross-backend check validates backends against each other, and they all
    /// draw from the same sampler. Neither can detect a sampler that biases the
    /// statistic. This test closes that gap by estimating a quantity whose true
    /// value is known exactly.
    #[test]
    fn sampled_zero_fraction_recovers_the_exact_value_at_order_3() {
        for q in [3u64, 5, 7] {
            let (_, exact_zeros, total) = exact_zero_count_order3(q);
            let truth = exact_zeros as f64 / total as f64;

            let draws = 400_000usize;
            let mut sampler =
                MatrixSampler::new(0xB488_F02C, q, 3, MeasurementPurpose::Equivalence, 12_345);
            let mut zeros = 0u64;
            for _ in 0..draws {
                let value = match q {
                    3 => {
                        let d = sampler.next_matrix::<3>(3);
                        gf2_algebra::permanent::bipedal3::permanent_bipedal3_singleword(
                            &Bipedal3Matrix::from_row_major(&d, 3, 3),
                        )
                        .value()
                    }
                    5 => {
                        let d = sampler.next_matrix::<5>(3);
                        gf2_algebra::permanent::bipedal5::permanent_bipedal5(
                            &Packed5Matrix::from_row_major(&d, 3, 3),
                        )
                        .value()
                    }
                    _ => {
                        let d = sampler.next_matrix::<7>(3);
                        gf2_algebra::permanent::bipedal7::permanent_bipedal7(
                            &Packed7Matrix::from_row_major(&d, 3, 3),
                        )
                        .value()
                    }
                };
                if value == 0 {
                    zeros += 1;
                }
            }
            let p_hat = zeros as f64 / draws as f64;
            let se = (truth * (1.0 - truth) / draws as f64).sqrt();
            let z = (p_hat - truth) / se;
            assert!(
                z.abs() < 4.0,
                "q={q}: sampled {p_hat:.6} vs exact {truth:.6} is {z:+.2} sigma \
over {draws} draws; the sampler or the kernel biases the campaign statistic"
            );
        }
    }

    /// The F_3 kernel must reproduce [Scheinerman2024] Table 3's exact
    /// `z(3) = 8163`, and every kernel must agree with an independent
    /// six-term permanent over the whole space. This is the anchor that the
    /// backend-vs-backend comparison cannot provide: it would catch an error
    /// shared by all implementations of one field's algorithm.
    #[test]
    fn kernels_match_exact_enumeration_at_order_3() {
        for q in [3u64, 5, 7] {
            let (kernel, independent, total) = exact_zero_count_order3(q);
            assert_eq!(
                kernel, independent,
                "q={q}: kernel counted {kernel} zeros over {total} matrices, \
independent expansion counted {independent}"
            );
            if q == 3 {
                assert_eq!(kernel, 8_163, "must reproduce Scheinerman2024 z(3)");
            }
        }
    }

    /// The CPU backends must agree per matrix at a size small enough for the
    /// fast test tier. F_7 is included at `n = 8`, inside its `n <= 16` bound.
    #[test]
    fn cpu_backends_agree_on_shared_inputs() {
        for (q, n) in [(3u64, 8usize), (5, 8), (7, 8)] {
            for row in check(q, n, 64, 0xB488_F02C) {
                assert_eq!(
                    row.mismatches, 0,
                    "q={q} n={n} backend={} status={}",
                    row.backend, row.status
                );
            }
        }
    }

    /// A backend that agrees per matrix necessarily agrees on the zero count;
    /// this pins the campaign's actual statistic rather than the raw values.
    #[test]
    fn zero_counts_agree_where_the_backend_ran() {
        for row in check(3, 10, 128, 0xB488_F02C) {
            if row.status == "identical" {
                assert_eq!(row.zeros_reference, row.zeros_backend);
            }
        }
    }

    /// The equivalence command compares the values the timed GPU path emits,
    /// including its same-backend fallback when event instrumentation is not
    /// available on an otherwise working GPU dispatch.
    #[cfg(feature = "hip")]
    #[test]
    fn timed_gpu_values_match_the_reference() {
        let row = check(3, 3, 8, 0xB488_F02C)
            .into_iter()
            .find(|row| row.backend == Backend::Gpu.name())
            .expect("GPU row when the HIP feature is enabled");
        assert_eq!(row.mismatches, 0, "status={}", row.status);
        assert_eq!(
            row.zeros_reference, row.zeros_backend,
            "status={}",
            row.status
        );
        assert!(row.status.starts_with("identical"), "status={}", row.status);
    }
}
