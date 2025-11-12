//! Test SparseMatrixDual for LDPC belief propagation efficiency.

use gf2_core::sparse::SparseMatrixDual;
use gf2_core::BitVec;

#[test]
fn test_dual_ldpc_regular_code() {
    // Regular (3,6) LDPC code: 3 ones per column, 6 ones per row
    // 4 check nodes × 8 variable nodes
    let coo = vec![
        (0, 0),
        (1, 0),
        (2, 0), // Column 0
        (0, 1),
        (1, 1),
        (3, 1), // Column 1
        (0, 2),
        (2, 2),
        (3, 2), // Column 2
        (1, 3),
        (2, 3),
        (3, 3), // Column 3
        (0, 4),
        (1, 4),
        (2, 4), // Column 4
        (0, 5),
        (1, 5),
        (3, 5), // Column 5
        (0, 6),
        (2, 6),
        (3, 6), // Column 6
        (1, 7),
        (2, 7),
        (3, 7), // Column 7
    ];

    let h = SparseMatrixDual::from_coo(4, 8, &coo);

    // Verify both access patterns work efficiently
    assert_eq!(h.rows(), 4);
    assert_eq!(h.cols(), 8);
    assert_eq!(h.nnz(), 24);

    // Check node (row) iteration
    for r in 0..4 {
        let degree = h.row_iter(r).count();
        assert_eq!(degree, 6, "Check node {} should have degree 6", r);
    }

    // Variable node (column) iteration - NO TRANSPOSE NEEDED!
    for c in 0..8 {
        let degree = h.col_iter(c).count();
        assert_eq!(degree, 3, "Variable node {} should have degree 3", c);
    }
}

#[test]
fn test_dual_bidirectional_neighbor_access() {
    // Small LDPC-like matrix for testing bidirectional access
    let coo = vec![
        (0, 0),
        (0, 2),
        (0, 4), // Check 0 neighbors: vars 0,2,4
        (1, 1),
        (1, 2),
        (1, 5), // Check 1 neighbors: vars 1,2,5
        (2, 0),
        (2, 3),
        (2, 5), // Check 2 neighbors: vars 0,3,5
    ];

    let h = SparseMatrixDual::from_coo(3, 6, &coo);

    // Check-to-variable (row iteration)
    let check0_neighbors: Vec<_> = h.row_iter(0).collect();
    assert_eq!(check0_neighbors, vec![0, 2, 4]);

    let check1_neighbors: Vec<_> = h.row_iter(1).collect();
    assert_eq!(check1_neighbors, vec![1, 2, 5]);

    let check2_neighbors: Vec<_> = h.row_iter(2).collect();
    assert_eq!(check2_neighbors, vec![0, 3, 5]);

    // Variable-to-check (column iteration) - efficient, no transpose!
    let var0_checks: Vec<_> = h.col_iter(0).collect();
    assert_eq!(var0_checks, vec![0, 2]); // Variable 0 connected to checks 0,2

    let var2_checks: Vec<_> = h.col_iter(2).collect();
    assert_eq!(var2_checks, vec![0, 1]); // Variable 2 connected to checks 0,1

    let var5_checks: Vec<_> = h.col_iter(5).collect();
    assert_eq!(var5_checks, vec![1, 2]); // Variable 5 connected to checks 1,2
}

#[test]
fn test_dual_syndrome_and_transpose() {
    // Simple parity check matrix for syndrome computation
    let coo = vec![
        (0, 0),
        (0, 1),
        (0, 2), // Check 0: x0 + x1 + x2 = 0
        (1, 2),
        (1, 3),
        (1, 4), // Check 1: x2 + x3 + x4 = 0
        (2, 0),
        (2, 4),
        (2, 5), // Check 2: x0 + x4 + x5 = 0
    ];

    let h = SparseMatrixDual::from_coo(3, 6, &coo);

    // Forward: syndrome = H × codeword
    let mut codeword = BitVec::new();
    for &b in &[true, true, false, false, false, true] {
        codeword.push_bit(b);
    }

    let syndrome = h.matvec(&codeword);
    assert_eq!(syndrome.len(), 3);
    // Check 0: 1+1+0 = 0 ✓
    // Check 1: 0+0+0 = 0 ✓
    // Check 2: 1+0+1 = 0 ✓
    assert!(!syndrome.get(0) && !syndrome.get(1) && !syndrome.get(2));

    // Transpose: y = H^T × syndrome
    // This is useful for message passing from checks back to variables
    let mut syndrome_vec = BitVec::new();
    for &b in &[true, false, true] {
        syndrome_vec.push_bit(b);
    }

    let result = h.matvec_transpose(&syndrome_vec);
    assert_eq!(result.len(), 6);

    // Variable 0: connected to checks 0,2 → 1⊕1 = 0
    assert!(!result.get(0));
    // Variable 1: connected to check 0 → 1
    assert!(result.get(1));
    // Variable 2: connected to checks 0,1 → 1⊕0 = 1
    assert!(result.get(2));
    // Variable 3: connected to check 1 → 0
    assert!(!result.get(3));
    // Variable 4: connected to checks 1,2 → 0⊕1 = 1
    assert!(result.get(4));
    // Variable 5: connected to check 2 → 1
    assert!(result.get(5));
}

#[test]
fn test_dual_ldpc_message_passing_pattern() {
    // Simulate typical belief propagation access pattern:
    // 1. Check-to-variable messages (iterate rows)
    // 2. Variable-to-check messages (iterate columns)
    // This should be efficient with dual representation

    let mut coo = Vec::new();
    let n_checks = 100;
    let n_vars = 200;

    // Create irregular LDPC structure
    for check in 0..n_checks {
        // Each check connected to 4-8 variables
        let degree = 4 + (check % 5);
        for i in 0..degree {
            let var = ((check * 7) + i * 13) % n_vars;
            coo.push((check, var));
        }
    }

    let h = SparseMatrixDual::from_coo(n_checks, n_vars, &coo);

    // Simulate check-to-variable sweep (row-wise)
    let mut check_to_var_msgs = 0;
    for check in 0..n_checks {
        for _var in h.row_iter(check) {
            check_to_var_msgs += 1;
            // In real decoder: compute message from check to variable
        }
    }
    assert_eq!(check_to_var_msgs, h.nnz());

    // Simulate variable-to-check sweep (column-wise)
    // NO TRANSPOSE overhead!
    let mut var_to_check_msgs = 0;
    for var in 0..n_vars {
        for _check in h.col_iter(var) {
            var_to_check_msgs += 1;
            // In real decoder: compute message from variable to check
        }
    }
    assert_eq!(var_to_check_msgs, h.nnz());

    // Both sweeps touched same total edges
    assert_eq!(check_to_var_msgs, var_to_check_msgs);
}

#[test]
fn test_dual_memory_efficiency() {
    // Verify dual representation is still memory-efficient for sparse matrices
    let mut coo = Vec::new();

    // 1000×2000 matrix with 0.5% density (typical LDPC)
    let n_checks = 1000;
    let n_vars = 2000;
    let nnz = 10_000; // 0.5% density

    for i in 0..nnz {
        let check = (i * 7) % n_checks;
        let var = (i * 13) % n_vars;
        coo.push((check, var));
    }

    let h = SparseMatrixDual::from_coo(n_checks, n_vars, &coo);

    let density = h.nnz() as f64 / (n_checks * n_vars) as f64;
    assert!(density < 0.01, "Should maintain low density");

    // Dual uses 2× sparse storage, but still << dense at low density
    // Dense would need: 1000 * 2000 bits = 250,000 bytes
    // Dual sparse needs: ~2 * 10,000 * 8 bytes = ~160,000 bytes
    // (Plus small overhead for indptr arrays)
    println!(
        "Dual representation: {} nonzeros, {:.4}% density",
        h.nnz(),
        density * 100.0
    );
}
