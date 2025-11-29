//! Compute backend abstraction for algorithm-level operations.
//!
//! This module defines the `ComputeBackend` trait for high-level algorithmic operations
//! (matrix multiply, RREF, batch encoding/decoding) that may benefit from different
//! execution strategies (CPU with rayon, GPU, FPGA).
//!
//! This complements the lower-level `kernels::Backend` trait, which focuses on
//! primitive operations (XOR, AND, popcount). The relationship is:
//!
//! - `kernels::Backend`: Primitive bit operations (building blocks)
//! - `compute::ComputeBackend`: Algorithm operations (uses primitives)
//!
//! # Design Philosophy
//!
//! - **Composable**: ComputeBackend uses kernels::Backend for primitive ops
//! - **Optional**: Enable via `parallel` or `gpu` features (pay-as-you-go)
//! - **Type-safe**: Compile-time backend selection (static dispatch preferred)
//! - **Testable**: Mock backends for unit testing
//!
//! # Examples
//!
//! ```
//! use gf2_core::{BitMatrix, compute::{ComputeBackend, CpuBackend}};
//!
//! let backend = CpuBackend::new();
//! let a = BitMatrix::identity(10);
//! let b = BitMatrix::identity(10);
//! let c = backend.matmul(&a, &b);
//! assert_eq!(c.rows(), 10);
//! assert_eq!(c.cols(), 10);
//! ```

use crate::{alg::rref::RrefResult, BitMatrix, BitVec};

/// Compute backend for algorithm-level operations.
///
/// This trait abstracts execution strategies for computationally intensive
/// operations. Implementations may use different hardware (CPU, GPU, FPGA)
/// or parallelization strategies (rayon, SIMD).
///
/// # Implementations
///
/// - `CpuBackend`: CPU execution with optional rayon parallelism
/// - `GpuBackend`: GPU execution via Vulkan (future, opt-in feature)
pub trait ComputeBackend: Send + Sync {
    /// Returns a human-readable name for this backend.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::compute::{ComputeBackend, CpuBackend};
    ///
    /// let backend = CpuBackend::new();
    /// assert!(backend.name().contains("CPU"));
    /// ```
    fn name(&self) -> &str;

    /// Returns the underlying kernel backend for primitive operations.
    ///
    /// This allows ComputeBackend to leverage optimized kernel implementations
    /// (scalar or SIMD) for low-level bit operations.
    fn kernel_backend(&self) -> &dyn crate::kernels::Backend;

    /// Matrix multiplication over GF(2): C = A × B.
    ///
    /// Computes the product of two boolean matrices where addition is XOR
    /// and multiplication is AND.
    ///
    /// # Arguments
    ///
    /// * `a` - Left matrix (m×k)
    /// * `b` - Right matrix (k×n)
    ///
    /// # Returns
    ///
    /// Result matrix (m×n)
    ///
    /// # Panics
    ///
    /// Panics if `a.cols() != b.rows()` (dimension mismatch).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::{BitMatrix, compute::{ComputeBackend, CpuBackend}};
    ///
    /// let backend = CpuBackend::new();
    /// let a = BitMatrix::identity(3);
    /// let b = BitMatrix::ones(3, 4);
    /// let c = backend.matmul(&a, &b);
    /// assert_eq!(c.rows(), 3);
    /// assert_eq!(c.cols(), 4);
    /// ```
    fn matmul(&self, a: &BitMatrix, b: &BitMatrix) -> BitMatrix;

    /// Reduced Row Echelon Form (RREF) with configurable pivoting.
    ///
    /// Transforms a matrix to RREF using Gaussian elimination. This is used
    /// for solving systems of linear equations, computing matrix rank, and
    /// deriving generator matrices from parity-check matrices.
    ///
    /// # Arguments
    ///
    /// * `matrix` - Input matrix to reduce
    /// * `pivot_from_right` - If true, pivots from rightmost columns (useful for systematic codes)
    ///
    /// # Returns
    ///
    /// `RrefResult` containing reduced matrix, pivot columns, row permutation, and rank.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::{BitMatrix, compute::{ComputeBackend, CpuBackend}};
    ///
    /// let backend = CpuBackend::new();
    /// let matrix = BitMatrix::identity(5);
    /// let result = backend.rref(&matrix, false);
    /// assert!(result.rank <= matrix.rows().min(matrix.cols()));
    /// ```
    fn rref(&self, matrix: &BitMatrix, pivot_from_right: bool) -> RrefResult;

    /// Matrix-vector multiplication: y = A × x over GF(2).
    ///
    /// Computes the product of a matrix with a single vector.
    ///
    /// # Arguments
    ///
    /// * `matrix` - Matrix A (m×n)
    /// * `vector` - Input vector x (length n)
    ///
    /// # Returns
    ///
    /// Result vector y (length m)
    ///
    /// # Panics
    ///
    /// Panics if `vector.len() != matrix.cols()`.
    fn matvec(&self, matrix: &BitMatrix, vector: &BitVec) -> BitVec;

    /// Matrix-vector multiplication with transposed matrix: y = A^T × x over GF(2).
    ///
    /// # Arguments
    ///
    /// * `matrix` - Matrix A (m×n)
    /// * `vector` - Input vector x (length m)
    ///
    /// # Returns
    ///
    /// Result vector y (length n)
    ///
    /// # Panics
    ///
    /// Panics if `vector.len() != matrix.rows()`.
    fn matvec_transpose(&self, matrix: &BitMatrix, vector: &BitVec) -> BitVec;

    /// Batch matrix-vector multiplication: compute A × x_i for multiple vectors.
    ///
    /// This operation is fundamental for batch encoding in linear codes where
    /// the same generator matrix is multiplied with multiple message vectors.
    /// Implementations may parallelize across vectors using rayon or GPU.
    ///
    /// # Arguments
    ///
    /// * `matrix` - Matrix A (m×n)
    /// * `vectors` - Input vectors x_i (each of length n)
    ///
    /// # Returns
    ///
    /// Vector of result vectors y_i (each of length m)
    ///
    /// # Panics
    ///
    /// Panics if any vector length doesn't match `matrix.cols()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::{BitMatrix, BitVec, compute::{ComputeBackend, CpuBackend}};
    ///
    /// let backend = CpuBackend::new();
    /// let identity = BitMatrix::identity(5);
    /// let vectors = vec![BitVec::ones(5), BitVec::zeros(5)];
    ///
    /// let results = backend.batch_matvec(&identity, &vectors);
    /// assert_eq!(results.len(), 2);
    /// assert_eq!(results[0], BitVec::ones(5));
    /// assert_eq!(results[1], BitVec::zeros(5));
    /// ```
    fn batch_matvec(&self, matrix: &BitMatrix, vectors: &[BitVec]) -> Vec<BitVec>;

    /// Batch matrix-vector multiplication with transposed matrix: compute A^T × x_i.
    ///
    /// Used for LDPC systematic encoding where we compute G^T × message_i.
    ///
    /// # Arguments
    ///
    /// * `matrix` - Matrix A (m×n)
    /// * `vectors` - Input vectors x_i (each of length m)
    ///
    /// # Returns
    ///
    /// Vector of result vectors y_i (each of length n)
    ///
    /// # Panics
    ///
    /// Panics if any vector length doesn't match `matrix.rows()`.
    fn batch_matvec_transpose(&self, matrix: &BitMatrix, vectors: &[BitVec]) -> Vec<BitVec>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BitMatrix;

    /// Test that backend name is non-empty and descriptive
    #[test]
    fn test_backend_name_is_descriptive() {
        let backend = crate::compute::CpuBackend::new();
        let name = backend.name();
        assert!(!name.is_empty(), "Backend name should not be empty");
        assert!(name.len() >= 3, "Backend name should be descriptive");
    }

    /// Test that kernel_backend returns a valid backend
    #[test]
    fn test_kernel_backend_is_valid() {
        let backend = crate::compute::CpuBackend::new();
        let kernel = backend.kernel_backend();
        let name = kernel.name();
        assert!(!name.is_empty(), "Kernel backend should have a name");
    }

    /// Test matrix multiplication with identity matrix
    #[test]
    #[cfg(feature = "rand")]
    fn test_matmul_with_identity() {
        use rand::thread_rng;
        let backend = crate::compute::CpuBackend::new();

        let mut rng = thread_rng();
        let a = BitMatrix::random(5, 5, &mut rng);
        let identity = BitMatrix::identity(5);

        // A × I = A
        let result = backend.matmul(&a, &identity);
        assert_eq!(result, a, "A × I should equal A");

        // I × A = A
        let result = backend.matmul(&identity, &a);
        assert_eq!(result, a, "I × A should equal A");
    }

    /// Test matrix multiplication dimensions
    #[test]
    fn test_matmul_dimensions() {
        let backend = crate::compute::CpuBackend::new();

        let a = BitMatrix::zeros(3, 5);
        let b = BitMatrix::zeros(5, 7);

        let c = backend.matmul(&a, &b);
        assert_eq!(c.rows(), 3, "Result should have left matrix rows");
        assert_eq!(c.cols(), 7, "Result should have right matrix cols");
    }

    /// Test matrix multiplication with zero matrix
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

    /// Test RREF with identity matrix
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

    /// Test RREF with zero matrix
    #[test]
    fn test_rref_zeros() {
        let backend = crate::compute::CpuBackend::new();

        let zero = BitMatrix::zeros(3, 5);
        let result = backend.rref(&zero, false);

        assert_eq!(result.rank, 0, "Zero matrix should have rank 0");
        assert_eq!(result.reduced, zero, "Zero matrix stays zero");
    }

    /// Test RREF preserves rank
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

    /// Test RREF with both pivot directions
    #[test]
    #[cfg(feature = "rand")]
    fn test_rref_pivot_directions() {
        use rand::thread_rng;
        let backend = crate::compute::CpuBackend::new();

        let mut rng = thread_rng();
        let matrix = BitMatrix::random(5, 10, &mut rng);

        let left_result = backend.rref(&matrix, false);
        let right_result = backend.rref(&matrix, true);

        // Both should give same rank
        assert_eq!(
            left_result.rank, right_result.rank,
            "Pivot direction should not change rank"
        );
    }
}
