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

use crate::backend::{evaluate, support, Backend, Batch, Support};
use crate::sampler::MatrixSampler;
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

/// Build `m` matrices for `(q, n)` from the equivalence-check stream.
fn shared_batch(q: u64, n: usize, m: usize, seed_root: u64) -> Batch {
    // Stream index 0 is reserved for the equivalence check, so its inputs never
    // coincide with a timing cell's inputs.
    let mut s = MatrixSampler::new(seed_root, q, n, 0);
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

/// Check every backend against the scalar single-word kernel at `(q, n)`.
///
/// Returns one row per non-reference backend. Unsupported backends are
/// reported with their reason rather than dropped.
#[must_use]
pub fn check(q: u64, n: usize, m: usize, seed_root: u64) -> Vec<EquivalenceRow> {
    let mut rows = Vec::new();

    if let Support::Unsupported(reason) = support(Backend::Scalar, q, n) {
        rows.push(EquivalenceRow {
            q,
            n,
            reference: Backend::Scalar.name(),
            backend: "all",
            matrices: 0,
            mismatches: 0,
            zeros_reference: 0,
            zeros_backend: 0,
            status: format!("skipped: reference unsupported ({reason})"),
        });
        return rows;
    }

    let batch = shared_batch(q, n, m, seed_root);
    let reference = evaluate(Backend::Scalar, &batch);
    let zeros_reference = crate::backend::count_zeros(&reference);

    for backend in [
        Backend::Avx2,
        Backend::Rayon,
        Backend::RayonAvx2,
        Backend::RayonIntra,
        Backend::Gpu,
    ] {
        let mut row = EquivalenceRow {
            q,
            n,
            reference: Backend::Scalar.name(),
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
                    let got = evaluate(backend, &batch);
                    row.mismatches = reference
                        .iter()
                        .zip(got.iter())
                        .filter(|(a, b)| a != b)
                        .count();
                    row.zeros_backend = crate::backend::count_zeros(&got);
                    row.status = if row.mismatches == 0 {
                        "identical".to_string()
                    } else {
                        "MISMATCH".to_string()
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
            let mut sampler = MatrixSampler::new(0xB488_F02C, q, 3, 12_345);
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
}
