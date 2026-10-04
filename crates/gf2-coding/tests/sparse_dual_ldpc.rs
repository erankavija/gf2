//! `SpBitMatrixDual` row and column access on LDPC-shaped matrices.

use gf2_core::sparse::SpBitMatrixDual;
use gf2_core::BitVec;

#[test]
fn test_dual_ldpc_regular_code() {
    // Regular LDPC structure: 3 ones per column, 6 ones per row.
    let coo = vec![
        (0, 0),
        (1, 0),
        (2, 0),
        (0, 1),
        (1, 1),
        (3, 1),
        (0, 2),
        (2, 2),
        (3, 2),
        (1, 3),
        (2, 3),
        (3, 3),
        (0, 4),
        (1, 4),
        (2, 4),
        (0, 5),
        (1, 5),
        (3, 5),
        (0, 6),
        (2, 6),
        (3, 6),
        (1, 7),
        (2, 7),
        (3, 7),
    ];

    let h = SpBitMatrixDual::from_coo(4, 8, &coo);

    assert_eq!(h.rows(), 4);
    assert_eq!(h.cols(), 8);
    assert_eq!(h.nnz(), 24);

    for r in 0..4 {
        let degree = h.row_iter(r).count();
        assert_eq!(degree, 6, "Check node {} should have degree 6", r);
    }

    for c in 0..8 {
        let degree = h.col_iter(c).count();
        assert_eq!(degree, 3, "Variable node {} should have degree 3", c);
    }
}

#[test]
fn test_dual_bidirectional_neighbor_access() {
    let coo = vec![
        (0, 0),
        (0, 2),
        (0, 4),
        (1, 1),
        (1, 2),
        (1, 5),
        (2, 0),
        (2, 3),
        (2, 5),
    ];

    let h = SpBitMatrixDual::from_coo(3, 6, &coo);

    let check0_neighbors: Vec<_> = h.row_iter(0).collect();
    assert_eq!(check0_neighbors, vec![0, 2, 4]);

    let check1_neighbors: Vec<_> = h.row_iter(1).collect();
    assert_eq!(check1_neighbors, vec![1, 2, 5]);

    let check2_neighbors: Vec<_> = h.row_iter(2).collect();
    assert_eq!(check2_neighbors, vec![0, 3, 5]);

    let var0_checks: Vec<_> = h.col_iter(0).collect();
    assert_eq!(var0_checks, vec![0, 2]);

    let var2_checks: Vec<_> = h.col_iter(2).collect();
    assert_eq!(var2_checks, vec![0, 1]);

    let var5_checks: Vec<_> = h.col_iter(5).collect();
    assert_eq!(var5_checks, vec![1, 2]);
}

#[test]
fn test_dual_syndrome_and_transpose() {
    let coo = vec![
        (0, 0),
        (0, 1),
        (0, 2),
        (1, 2),
        (1, 3),
        (1, 4),
        (2, 0),
        (2, 4),
        (2, 5),
    ];

    let h = SpBitMatrixDual::from_coo(3, 6, &coo);

    let mut codeword = BitVec::new();
    for &b in &[true, true, false, false, false, true] {
        codeword.push_bit(b);
    }

    let syndrome = h.matvec(&codeword);
    assert_eq!(syndrome.len(), 3);
    assert!(!syndrome.get(0) && !syndrome.get(1) && !syndrome.get(2));

    let mut syndrome_vec = BitVec::new();
    for &b in &[true, false, true] {
        syndrome_vec.push_bit(b);
    }

    let result = h.matvec_transpose(&syndrome_vec);
    assert_eq!(result.len(), 6);

    assert!(!result.get(0));
    assert!(result.get(1));
    assert!(result.get(2));
    assert!(!result.get(3));
    assert!(result.get(4));
    assert!(result.get(5));
}

#[test]
fn test_dual_ldpc_message_passing_pattern() {
    let mut coo = Vec::new();
    let n_checks = 100;
    let n_vars = 200;

    for check in 0..n_checks {
        let degree = 4 + (check % 5);
        for i in 0..degree {
            let var = ((check * 7) + i * 13) % n_vars;
            coo.push((check, var));
        }
    }

    let h = SpBitMatrixDual::from_coo(n_checks, n_vars, &coo);

    let mut check_to_var_msgs = 0;
    for check in 0..n_checks {
        for _var in h.row_iter(check) {
            check_to_var_msgs += 1;
        }
    }
    assert_eq!(check_to_var_msgs, h.nnz());

    let mut var_to_check_msgs = 0;
    for var in 0..n_vars {
        for _check in h.col_iter(var) {
            var_to_check_msgs += 1;
        }
    }
    assert_eq!(var_to_check_msgs, h.nnz());

    assert_eq!(check_to_var_msgs, var_to_check_msgs);
}

#[test]
fn test_dual_memory_efficiency() {
    let mut coo = Vec::new();

    let n_checks = 1000;
    let n_vars = 2000;
    let nnz = 10_000;

    for i in 0..nnz {
        let check = (i * 7) % n_checks;
        let var = (i * 13) % n_vars;
        coo.push((check, var));
    }

    let h = SpBitMatrixDual::from_coo(n_checks, n_vars, &coo);

    let density = h.nnz() as f64 / (n_checks * n_vars) as f64;
    assert!(density < 0.01, "Should maintain low density");

    println!(
        "Dual representation: {} nonzeros, {:.4}% density",
        h.nnz(),
        density * 100.0
    );
}
