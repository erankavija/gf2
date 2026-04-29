use gf2_core::sparse::{block_csr_from_csr, SpBitMatrix};
use gf2_core::{matrix::BitMatrix, BitVec};

#[test]
fn test_sparse_zeros_and_identity() {
    let s0 = SpBitMatrix::zeros(3, 4);
    assert_eq!(s0.rows(), 3);
    assert_eq!(s0.cols(), 4);
    assert_eq!(s0.nnz(), 0);

    let si = SpBitMatrix::identity(4);
    assert_eq!(si.rows(), 4);
    assert_eq!(si.cols(), 4);
    assert_eq!(si.nnz(), 4);

    let d = si.to_dense();
    for i in 0..4 {
        assert!(d.get(i, i));
    }
}

#[test]
fn test_sparse_from_coo_dedup_xor() {
    // Two entries at same (r,c) cancel under XOR
    let coo = vec![(0, 1), (0, 1), (1, 2)];
    let s = SpBitMatrix::from_coo(3, 4, &coo);
    assert_eq!(s.nnz(), 1);
    let d = s.to_dense();
    assert!(!d.get(0, 1));
    assert!(d.get(1, 2));
}

#[test]
fn test_sparse_from_dense_roundtrip() {
    let mut m = BitMatrix::zeros(2, 3);
    m.set(0, 0, true);
    m.set(0, 2, true);
    m.set(1, 1, true);

    let s = SpBitMatrix::from_dense(&m);
    assert_eq!(s.nnz(), 3);
    let d = s.to_dense();
    assert_eq!(d, m);
}

#[test]
fn test_sparse_matvec() {
    let mut m = BitMatrix::zeros(3, 5);
    m.set(0, 1, true);
    m.set(0, 3, true);
    m.set(1, 0, true);
    m.set(1, 4, true);
    m.set(2, 2, true);
    let s = SpBitMatrix::from_dense(&m);

    let mut x = BitVec::with_capacity(5);
    for b in [false, true, true, false, true] {
        x.push_bit(b);
    }

    let y = s.matvec(&x);
    assert_eq!(y.len(), 3);
    // Row 0: x1 XOR x3 = 1 XOR 0 = 1
    assert!(y.get(0));
    // Row 1: x0 XOR x4 = 0 XOR 1 = 1
    assert!(y.get(1));
    // Row 2: x2 = 1
    assert!(y.get(2));
}

#[test]
fn test_block_csr_matvec_matches_csr_word_boundaries() {
    let cols_cases = [0usize, 1, 63, 64, 65, 127, 128, 129];
    let block_cases = [1usize, 2, 3, 8];

    for &cols in &cols_cases {
        let rows = 7;
        let mut entries = Vec::new();
        for r in 0..rows {
            for &c in &[0usize, 1, 62, 63, 64, 65, 126, 127, 128] {
                if c < cols && (r + c).count_ones() % 2 == 0 {
                    entries.push((r, c));
                }
            }
        }

        let csr = SpBitMatrix::from_coo(rows, cols, &entries);
        let mut x = BitVec::with_capacity(cols);
        for i in 0..cols {
            x.push_bit((i % 3) == 1 || i == 63 || i == 64 || i == 128);
        }

        let expected = csr.matvec(&x);
        for &block_rows in &block_cases {
            let blocked = block_csr_from_csr(&csr, block_rows);
            assert_eq!(blocked.rows(), csr.rows());
            assert_eq!(blocked.cols(), csr.cols());
            assert_eq!(blocked.nnz(), csr.nnz());
            assert_eq!(
                blocked.matvec(&x),
                expected,
                "cols={cols} block_rows={block_rows}"
            );
            assert_eq!(
                blocked.matvec_with_prefetch_distance(&x, 0),
                expected,
                "no-prefetch cols={cols} block_rows={block_rows}"
            );
        }
    }
}

#[test]
fn test_block_csr_transformer_handles_empty_rows_and_tail_block() {
    let csr = SpBitMatrix::from_coo(6, 130, &[(0, 0), (0, 64), (3, 129), (5, 63)]);
    let blocked = csr.to_block_csr(4);
    let mut x = BitVec::with_capacity(130);
    for i in 0..130 {
        x.push_bit(matches!(i, 0 | 63 | 64 | 129));
    }

    assert_eq!(blocked.block_rows(), 4);
    assert_eq!(blocked.matvec(&x), csr.matvec(&x));
}

// ============================================================================
// Deduplication Tests (from_coo_deduplicated)
// ============================================================================

#[test]
fn test_sparse_from_coo_deduplicated_basic() {
    // Test from SPARSE_MATRIX_REQUIREMENTS.md
    let edges = vec![(0, 0), (0, 1), (0, 1), (1, 2)];
    let matrix = SpBitMatrix::from_coo_deduplicated(2, 3, &edges);

    let d = matrix.to_dense();
    assert!(d.get(0, 0));
    assert!(d.get(0, 1)); // NOT false (dedup, not XOR)
    assert!(d.get(1, 2));

    // Total number of edges (after dedup) should be 3
    assert_eq!(matrix.nnz(), 3);
}

#[test]
fn test_sparse_from_coo_deduplicated_vs_xor() {
    // Demonstrate difference between XOR and dedup semantics
    let edges = vec![(0, 1), (0, 1), (1, 2)];

    // XOR: duplicates cancel
    let xor_matrix = SpBitMatrix::from_coo(2, 3, &edges);
    let xor_dense = xor_matrix.to_dense();
    assert!(!xor_dense.get(0, 1)); // Canceled
    assert_eq!(xor_matrix.nnz(), 1);

    // Dedup: duplicates ignored (first wins)
    let dedup_matrix = SpBitMatrix::from_coo_deduplicated(2, 3, &edges);
    let dedup_dense = dedup_matrix.to_dense();
    assert!(dedup_dense.get(0, 1)); // Kept
    assert_eq!(dedup_matrix.nnz(), 2);
}

#[test]
fn test_sparse_from_coo_deduplicated_empty() {
    let edges: Vec<(usize, usize)> = vec![];
    let matrix = SpBitMatrix::from_coo_deduplicated(3, 4, &edges);

    assert_eq!(matrix.rows(), 3);
    assert_eq!(matrix.cols(), 4);
    assert_eq!(matrix.nnz(), 0);
}

#[test]
fn test_sparse_from_coo_deduplicated_all_duplicates() {
    // All entries are duplicates of (0, 0)
    let edges = vec![(0, 0), (0, 0), (0, 0), (0, 0)];
    let matrix = SpBitMatrix::from_coo_deduplicated(2, 2, &edges);

    let d = matrix.to_dense();
    assert!(d.get(0, 0));
    assert_eq!(matrix.nnz(), 1);
}

#[test]
fn test_sparse_from_coo_deduplicated_mixed() {
    // Mix of unique and duplicate entries
    let edges = vec![
        (0, 0),
        (0, 1),
        (0, 1),
        (0, 1), // 0,1 appears 3 times
        (1, 0),
        (1, 1),
        (2, 2),
        (2, 2), // 2,2 appears 2 times
    ];
    let matrix = SpBitMatrix::from_coo_deduplicated(3, 3, &edges);

    let d = matrix.to_dense();
    assert!(d.get(0, 0));
    assert!(d.get(0, 1));
    assert!(d.get(1, 0));
    assert!(d.get(1, 1));
    assert!(d.get(2, 2));
    assert_eq!(matrix.nnz(), 5);
}

#[test]
fn test_sparse_from_coo_deduplicated_unsorted_input() {
    // Input not sorted - should still deduplicate correctly
    let edges = vec![
        (1, 2),
        (0, 1),
        (0, 1), // Duplicate
        (0, 0),
        (1, 2), // Duplicate
    ];
    let matrix = SpBitMatrix::from_coo_deduplicated(2, 3, &edges);

    let d = matrix.to_dense();
    assert!(d.get(0, 0));
    assert!(d.get(0, 1));
    assert!(d.get(1, 2));
    assert_eq!(matrix.nnz(), 3);
}

#[test]
fn test_sparse_from_coo_deduplicated_roundtrip_dense() {
    // Deduplication should produce same result as from_dense
    let mut dense = BitMatrix::zeros(3, 4);
    dense.set(0, 1, true);
    dense.set(0, 3, true);
    dense.set(1, 1, true);
    dense.set(2, 0, true);

    // Build COO with duplicates
    let edges = vec![
        (0, 1),
        (0, 1), // Duplicate
        (0, 3),
        (1, 1),
        (1, 1),
        (1, 1), // Triple
        (2, 0),
    ];

    let from_coo = SpBitMatrix::from_coo_deduplicated(3, 4, &edges);
    let from_dense = SpBitMatrix::from_dense(&dense);

    assert_eq!(from_coo.to_dense(), from_dense.to_dense());
}
