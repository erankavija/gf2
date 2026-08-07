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

#[cfg(test)]
mod tests {
    use super::*;

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
