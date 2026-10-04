//! The [`ComputeBackend`] trait for algorithm-level GF(2) matrix operations.

use crate::{alg::rref::RrefResult, BitMatrix, BitVec};

/// Execution strategy for algorithm-level GF(2) matrix operations.
pub trait ComputeBackend: Send + Sync {
    /// Human-readable backend name.
    fn name(&self) -> &str;

    /// Kernel backend that supplies the word-level primitives.
    fn kernel_backend(&self) -> &dyn crate::kernels::Backend;

    /// Product `a · b` over GF(2).
    ///
    /// # Panics
    ///
    /// Panics if `a.cols() != b.rows()`.
    fn matmul(&self, a: &BitMatrix, b: &BitMatrix) -> BitMatrix;

    /// Reduced row echelon form of `matrix`; `pivot_from_right` searches
    /// pivots from the rightmost column.
    fn rref(&self, matrix: &BitMatrix, pivot_from_right: bool) -> RrefResult;

    /// Product `matrix · vector` over GF(2).
    ///
    /// # Panics
    ///
    /// Panics if `vector.len() != matrix.cols()`.
    fn matvec(&self, matrix: &BitMatrix, vector: &BitVec) -> BitVec;

    /// Product `matrixᵀ · vector` over GF(2).
    ///
    /// # Panics
    ///
    /// Panics if `vector.len() != matrix.rows()`.
    fn matvec_transpose(&self, matrix: &BitMatrix, vector: &BitVec) -> BitVec;

    /// [`Self::matvec`] of each vector, in input order.
    ///
    /// # Panics
    ///
    /// Panics if any vector length differs from `matrix.cols()`.
    fn batch_matvec(&self, matrix: &BitMatrix, vectors: &[BitVec]) -> Vec<BitVec>;

    /// [`Self::matvec_transpose`] of each vector, in input order.
    ///
    /// # Panics
    ///
    /// Panics if any vector length differs from `matrix.rows()`.
    fn batch_matvec_transpose(&self, matrix: &BitMatrix, vectors: &[BitVec]) -> Vec<BitVec>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BitMatrix;
    #[test]
    fn test_backend_name_is_descriptive() {
        let backend = crate::compute::CpuBackend::new();
        let name = backend.name();
        assert!(!name.is_empty(), "Backend name should not be empty");
        assert!(name.len() >= 3, "Backend name should be descriptive");
    }
    #[test]
    fn test_kernel_backend_is_valid() {
        let backend = crate::compute::CpuBackend::new();
        let kernel = backend.kernel_backend();
        let name = kernel.name();
        assert!(!name.is_empty(), "Kernel backend should have a name");
    }
    #[test]
    #[cfg(feature = "rand")]
    fn test_matmul_with_identity() {
        use rand::thread_rng;
        let backend = crate::compute::CpuBackend::new();

        let mut rng = thread_rng();
        let a = BitMatrix::random(5, 5, &mut rng);
        let identity = BitMatrix::identity(5);
        let result = backend.matmul(&a, &identity);
        assert_eq!(result, a, "A × I should equal A");
        let result = backend.matmul(&identity, &a);
        assert_eq!(result, a, "I × A should equal A");
    }
    #[test]
    fn test_matmul_dimensions() {
        let backend = crate::compute::CpuBackend::new();

        let a = BitMatrix::zeros(3, 5);
        let b = BitMatrix::zeros(5, 7);

        let c = backend.matmul(&a, &b);
        assert_eq!(c.rows(), 3, "Result should have left matrix rows");
        assert_eq!(c.cols(), 7, "Result should have right matrix cols");
    }
    #[test]
    #[cfg(feature = "rand")]
    fn test_matmul_with_zeros() {
        use rand::thread_rng;
        let backend = crate::compute::CpuBackend::new();

        let mut rng = thread_rng();
        let a = BitMatrix::random(4, 6, &mut rng);
        let zero = BitMatrix::zeros(6, 8);

        let result = backend.matmul(&a, &zero);
        assert_eq!(
            result,
            BitMatrix::zeros(4, 8),
            "A × 0 should be zero matrix"
        );
    }
    #[test]
    fn test_rref_identity() {
        let backend = crate::compute::CpuBackend::new();

        let identity = BitMatrix::identity(5);
        let result = backend.rref(&identity, false);

        assert_eq!(result.rank, 5, "Identity matrix should have full rank");
        assert_eq!(
            result.reduced, identity,
            "Identity matrix is already in RREF"
        );
    }
    #[test]
    fn test_rref_zeros() {
        let backend = crate::compute::CpuBackend::new();

        let zero = BitMatrix::zeros(3, 5);
        let result = backend.rref(&zero, false);

        assert_eq!(result.rank, 0, "Zero matrix should have rank 0");
        assert_eq!(result.reduced, zero, "Zero matrix stays zero");
    }
    #[test]
    #[cfg(feature = "rand")]
    fn test_rref_rank_invariant() {
        use rand::thread_rng;
        let backend = crate::compute::CpuBackend::new();

        let mut rng = thread_rng();
        let matrix = BitMatrix::random(6, 10, &mut rng);
        let result = backend.rref(&matrix, false);

        assert!(result.rank <= matrix.rows(), "Rank cannot exceed rows");
        assert!(result.rank <= matrix.cols(), "Rank cannot exceed cols");
    }
    #[test]
    #[cfg(feature = "rand")]
    fn test_rref_pivot_directions() {
        use rand::thread_rng;
        let backend = crate::compute::CpuBackend::new();

        let mut rng = thread_rng();
        let matrix = BitMatrix::random(5, 10, &mut rng);

        let left_result = backend.rref(&matrix, false);
        let right_result = backend.rref(&matrix, true);
        assert_eq!(
            left_result.rank, right_result.rank,
            "Pivot direction should not change rank"
        );
    }
}
