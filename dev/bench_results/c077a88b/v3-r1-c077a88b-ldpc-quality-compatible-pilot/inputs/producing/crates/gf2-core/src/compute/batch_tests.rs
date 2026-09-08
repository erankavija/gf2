//! Tests for batch operations in ComputeBackend.
//!
//! This module defines tests for batch encoding/decoding operations that will
//! be used by higher-level coding algorithms (LDPC, BCH, etc).

#[cfg(test)]
mod tests {
    use crate::compute::{ComputeBackend, CpuBackend};
    use crate::BitVec;

    /// Test batch matrix-vector multiplication: parallel computation of A × b_i for multiple vectors.
    ///
    /// This operation is fundamental for batch encoding in linear codes where the
    /// same generator matrix G is multiplied with multiple message vectors.
    #[test]
    #[cfg(feature = "rand")]
    fn test_batch_matvec_empty_batch() {
        use crate::BitMatrix;
        let backend = CpuBackend::new();
        let matrix = BitMatrix::identity(5);
        let vectors: Vec<BitVec> = vec![];

        let results = backend.batch_matvec(&matrix, &vectors);
        assert_eq!(results.len(), 0, "Empty batch should return empty results");
    }

    #[test]
    #[cfg(feature = "rand")]
    fn test_batch_matvec_single_vector() {
        use crate::BitMatrix;
        let backend = CpuBackend::new();
        let identity = BitMatrix::identity(5);
        let vec = BitVec::ones(5);

        let results = backend.batch_matvec(&identity, std::slice::from_ref(&vec));
        assert_eq!(
            results.len(),
            1,
            "Single vector should return single result"
        );
        assert_eq!(results[0], vec, "Identity matrix should preserve vector");
    }

    #[test]
    #[cfg(feature = "rand")]
    fn test_batch_matvec_multiple_vectors() {
        use crate::BitMatrix;
        let backend = CpuBackend::new();
        let identity = BitMatrix::identity(5);

        // Create multiple input vectors
        let vectors: Vec<BitVec> = (0..10)
            .map(|i| {
                let mut v = BitVec::zeros(5);
                v.set(i % 5, true);
                v
            })
            .collect();

        let results = backend.batch_matvec(&identity, &vectors);
        assert_eq!(results.len(), 10, "Should return result for each input");

        // Identity matrix preserves all vectors
        for (input, output) in vectors.iter().zip(results.iter()) {
            assert_eq!(input, output, "Identity should preserve each vector");
        }
    }

    #[test]
    #[cfg(feature = "rand")]
    fn test_batch_matvec_correctness() {
        use crate::BitMatrix;
        use rand::thread_rng;

        let backend = CpuBackend::new();
        let mut rng = thread_rng();
        let matrix = BitMatrix::random(10, 8, &mut rng);

        // Generate random input vectors
        let vectors: Vec<BitVec> = (0..5).map(|_| BitVec::random(8, &mut rng)).collect();

        // Batch operation
        let batch_results = backend.batch_matvec(&matrix, &vectors);

        // Compare with individual operations
        for (i, vec) in vectors.iter().enumerate() {
            let individual_result = backend.matvec(&matrix, vec);
            assert_eq!(
                batch_results[i], individual_result,
                "Batch result at index {} should match individual computation",
                i
            );
        }
    }

    /// Test batch matrix-vector multiplication with transposed matrix.
    ///
    /// This is used for LDPC systematic encoding where we compute G^T × message.
    #[test]
    #[cfg(feature = "rand")]
    fn test_batch_matvec_transpose_correctness() {
        use crate::BitMatrix;
        use rand::thread_rng;

        let backend = CpuBackend::new();
        let mut rng = thread_rng();
        let matrix = BitMatrix::random(10, 15, &mut rng);

        // Generate input vectors matching matrix rows
        let vectors: Vec<BitVec> = (0..3).map(|_| BitVec::random(10, &mut rng)).collect();

        // Batch transpose operation
        let batch_results = backend.batch_matvec_transpose(&matrix, &vectors);

        // Compare with individual operations
        for (i, vec) in vectors.iter().enumerate() {
            let individual_result = backend.matvec_transpose(&matrix, vec);
            assert_eq!(
                batch_results[i], individual_result,
                "Batch transpose result at index {} should match individual",
                i
            );
        }
    }

    #[test]
    #[cfg(all(feature = "rand", feature = "parallel"))]
    fn test_batch_matvec_parallel_speedup() {
        use crate::BitMatrix;
        use rand::thread_rng;
        use std::time::Instant;

        let backend = CpuBackend::new();
        let mut rng = thread_rng();

        // Large matrix and many vectors to see parallel benefit
        let matrix = BitMatrix::random(1000, 1000, &mut rng);
        let vectors: Vec<BitVec> = (0..100).map(|_| BitVec::random(1000, &mut rng)).collect();

        // Time batch operation (should use parallelism)
        let start = Instant::now();
        let batch_results = backend.batch_matvec(&matrix, &vectors);
        let batch_time = start.elapsed();

        // Time sequential operations
        let start = Instant::now();
        let seq_results: Vec<_> = vectors.iter().map(|v| backend.matvec(&matrix, v)).collect();
        let seq_time = start.elapsed();

        // Results should match
        assert_eq!(
            batch_results, seq_results,
            "Parallel results should match sequential"
        );

        // Parallel should be faster (at least 1.5x on multi-core)
        // Note: This is a soft check; actual speedup depends on CPU cores
        println!(
            "Batch time: {:?}, Sequential time: {:?}",
            batch_time, seq_time
        );
        println!(
            "Speedup: {:.2}x",
            seq_time.as_secs_f64() / batch_time.as_secs_f64()
        );
    }

    /// Test that batch operations handle dimension mismatches correctly
    #[test]
    #[should_panic(expected = "columns")]
    #[cfg(feature = "rand")]
    fn test_batch_matvec_dimension_mismatch() {
        use crate::BitMatrix;
        let backend = CpuBackend::new();
        let matrix = BitMatrix::identity(5);
        let wrong_size = BitVec::ones(3); // Wrong size!

        backend.batch_matvec(&matrix, &[wrong_size]);
    }
}
