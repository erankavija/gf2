//! [`ComputeBackend`] on the CPU.

use super::backend::ComputeBackend;
use crate::{alg::rref::RrefResult, kernels::Backend, BitMatrix, BitVec};

/// [`ComputeBackend`] over a kernel backend chosen at construction; the batch
/// operations use rayon's global pool under the `parallel` feature.
pub struct CpuBackend {
    kernel: Box<dyn Backend>,
}

impl CpuBackend {
    /// Selects the SIMD kernel backend when the `simd` feature is enabled and
    /// the CPU supports it, otherwise the scalar one.
    pub fn new() -> Self {
        #[cfg(feature = "simd")]
        let kernel: Box<dyn Backend> = {
            if let Some(simd) = crate::kernels::simd::maybe_simd() {
                Box::new(*simd)
            } else {
                Box::new(crate::kernels::scalar::SCALAR_BACKEND)
            }
        };

        #[cfg(not(feature = "simd"))]
        let kernel: Box<dyn Backend> = Box::new(crate::kernels::scalar::SCALAR_BACKEND);

        Self { kernel }
    }
}

impl Default for CpuBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputeBackend for CpuBackend {
    fn name(&self) -> &str {
        #[cfg(all(feature = "parallel", feature = "simd"))]
        return "CPU (rayon + SIMD)";

        #[cfg(all(feature = "parallel", not(feature = "simd")))]
        return "CPU (rayon)";

        #[cfg(all(not(feature = "parallel"), feature = "simd"))]
        return "CPU (SIMD)";

        #[cfg(all(not(feature = "parallel"), not(feature = "simd")))]
        return "CPU (scalar)";
    }

    fn kernel_backend(&self) -> &dyn Backend {
        self.kernel.as_ref()
    }

    fn matmul(&self, a: &BitMatrix, b: &BitMatrix) -> BitMatrix {
        // Serial in every configuration.
        a * b
    }

    fn rref(&self, matrix: &BitMatrix, pivot_from_right: bool) -> RrefResult {
        crate::alg::rref::rref(matrix, pivot_from_right)
    }

    fn matvec(&self, matrix: &BitMatrix, vector: &BitVec) -> BitVec {
        assert_eq!(
            vector.len(),
            matrix.cols(),
            "Vector length {} must match matrix columns {}",
            vector.len(),
            matrix.cols()
        );
        matrix.matvec(vector)
    }

    fn matvec_transpose(&self, matrix: &BitMatrix, vector: &BitVec) -> BitVec {
        assert_eq!(
            vector.len(),
            matrix.rows(),
            "Vector length {} must match matrix rows {}",
            vector.len(),
            matrix.rows()
        );
        matrix.matvec_transpose(vector)
    }

    fn batch_matvec(&self, matrix: &BitMatrix, vectors: &[BitVec]) -> Vec<BitVec> {
        for (i, vec) in vectors.iter().enumerate() {
            assert_eq!(
                vec.len(),
                matrix.cols(),
                "Vector at index {} has length {} but matrix has {} columns",
                i,
                vec.len(),
                matrix.cols()
            );
        }

        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;
            use std::sync::Arc;
            let matrix = Arc::new(matrix);
            (0..vectors.len())
                .into_par_iter()
                .map(|i| matrix.matvec(&vectors[i]))
                .collect()
        }

        #[cfg(not(feature = "parallel"))]
        {
            vectors.iter().map(|v| self.matvec(matrix, v)).collect()
        }
    }

    fn batch_matvec_transpose(&self, matrix: &BitMatrix, vectors: &[BitVec]) -> Vec<BitVec> {
        for (i, vec) in vectors.iter().enumerate() {
            assert_eq!(
                vec.len(),
                matrix.rows(),
                "Vector at index {} has length {} but matrix has {} rows",
                i,
                vec.len(),
                matrix.rows()
            );
        }

        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;
            use std::sync::Arc;
            let matrix = Arc::new(matrix);
            (0..vectors.len())
                .into_par_iter()
                .map(|i| matrix.matvec_transpose(&vectors[i]))
                .collect()
        }

        #[cfg(not(feature = "parallel"))]
        {
            vectors
                .iter()
                .map(|v| self.matvec_transpose(matrix, v))
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BitMatrix;

    #[test]
    fn test_cpu_backend_creation() {
        let backend = CpuBackend::new();
        assert!(backend.name().contains("CPU"));
    }

    #[test]
    fn test_cpu_backend_default() {
        let backend = CpuBackend::default();
        assert!(backend.name().contains("CPU"));
    }

    #[test]
    fn test_kernel_backend_works() {
        let backend = CpuBackend::new();
        let kernel = backend.kernel_backend();
        let mut dst = vec![0xFF, 0x00];
        let src = vec![0x0F, 0xF0];
        kernel.xor(&mut dst, &src);
        assert_eq!(dst, vec![0xF0, 0xF0]);
    }

    #[test]
    fn test_matmul_correctness() {
        let backend = CpuBackend::new();
        let identity = BitMatrix::identity(2);

        let mut b = BitMatrix::zeros(2, 2);
        b.set(0, 0, true);
        b.set(0, 1, true);
        b.set(1, 0, true);
        b.set(1, 1, true);

        let c = backend.matmul(&identity, &b);

        assert_eq!(c, b, "I × B = B");
    }

    #[test]
    #[cfg(feature = "rand")]
    fn test_matmul_associativity() {
        use rand::thread_rng;
        let backend = CpuBackend::new();

        let mut rng = thread_rng();
        let a = BitMatrix::random(3, 4, &mut rng);
        let b = BitMatrix::random(4, 5, &mut rng);
        let c = BitMatrix::random(5, 6, &mut rng);
        let left = backend.matmul(&backend.matmul(&a, &b), &c);
        let right = backend.matmul(&a, &backend.matmul(&b, &c));

        assert_eq!(left, right, "Matrix multiplication should be associative");
    }

    #[test]
    fn test_rref_simple_matrix() {
        let backend = CpuBackend::new();
        let mut m = BitMatrix::zeros(3, 3);
        m.set(0, 0, true);
        m.set(0, 2, true);
        m.set(1, 1, true);
        m.set(1, 2, true);
        m.set(2, 0, true);
        m.set(2, 1, true);

        let result = backend.rref(&m, false);
        assert_eq!(
            result.rank, 2,
            "Matrix has rank 2 (row 2 = row 0 XOR row 1)"
        );
    }

    #[test]
    fn test_rref_rank_deficient() {
        let backend = CpuBackend::new();
        // Row 2 is row 0 + row 1.
        let mut m = BitMatrix::zeros(3, 3);
        m.set(0, 0, true);
        m.set(0, 2, true);
        m.set(1, 1, true);
        m.set(2, 0, true);
        m.set(2, 1, true);
        m.set(2, 2, true);

        let result = backend.rref(&m, false);

        assert!(result.rank < 3, "Matrix should be rank deficient");
    }

    #[test]
    #[cfg(feature = "rand")]
    fn test_rref_with_both_pivot_directions() {
        use rand::thread_rng;
        let backend = CpuBackend::new();

        let mut rng = thread_rng();
        let m = BitMatrix::random(5, 8, &mut rng);

        let left_pivot = backend.rref(&m, false);
        let right_pivot = backend.rref(&m, true);
        assert_eq!(left_pivot.rank, right_pivot.rank);
        assert!(left_pivot.reduced.rows() == m.rows());
        assert!(right_pivot.reduced.rows() == m.rows());
    }
}
