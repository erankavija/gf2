//! Finite compile-time candidate seams for extent calibration.

#![cfg(feature = "test-support")]

use gf2_core::field::matrix::{
    gemm, gemm_tile_observations, reset_gemm_tile_observations, FieldMatrix, GemmTilePair,
    GemmTileSite,
};
use gf2_core::gfp::Fp;

type F = Fp<65537>;

fn fixture(rows: usize, cols: usize, salt: u64) -> FieldMatrix<F> {
    let mut matrix = FieldMatrix::zeros(rows, cols);
    for row in 0..rows {
        for col in 0..cols {
            matrix.set(
                row,
                col,
                F::new((salt + 17 * row as u64 + 29 * col as u64) % 65537),
            );
        }
    }
    matrix
}

fn axpy(alpha: F, product: &FieldMatrix<F>, beta: F, addend: &FieldMatrix<F>) -> FieldMatrix<F> {
    let (rows, cols) = product.shape();
    let mut expected = FieldMatrix::zeros(rows, cols);
    for row in 0..rows {
        for col in 0..cols {
            expected.set(
                row,
                col,
                alpha * product.get(row, col) + beta * addend.get(row, col),
            );
        }
    }
    expected
}

#[test]
fn finite_gemm_grid_reaches_all_seven_shared_production_bodies() {
    let (m, k, n) = (65, 2, 129);
    let a = fixture(m, k, 3);
    let b = fixture(k, n, 7);
    let c = fixture(m, n, 11);
    let product = gemm(&a, &b);
    let alpha = F::new(3);
    let beta = F::new(5);
    let fused = axpy(alpha, &product, beta, &c);
    let beta_only = axpy(F::new(1), &product, beta, &c);
    let a_transposed_storage = a.transpose();
    let trans_product = gemm(&a_transposed_storage.transpose(), &b);
    let trans_fused = axpy(alpha, &trans_product, beta, &c);
    let mut implicit_a = a.clone();
    let mut implicit_b = b.clone();
    for diagonal in 0..k {
        implicit_a.set(diagonal, diagonal, F::new(1));
        implicit_b.set(diagonal, diagonal, F::new(1));
    }
    let implicit_fused = axpy(alpha, &gemm(&implicit_a, &implicit_b), beta, &c);

    assert_eq!(
        GemmTilePair::ALL.map(GemmTilePair::extents),
        [
            (16, 32),
            (16, 64),
            (16, 128),
            (32, 32),
            (32, 64),
            (32, 128),
            (64, 32),
            (64, 64),
            (64, 128),
        ]
    );

    for pair in GemmTilePair::ALL {
        reset_gemm_tile_observations();

        assert_eq!((pair.gemm_fn())(&a, &b), product);

        let mut into = FieldMatrix::zeros(m, n);
        (pair.gemm_into_view_fn())(&a, &b, &mut into);
        assert_eq!(into, product);

        let mut axpy_out = c.clone();
        (pair.gemm_axpy_fn())(alpha, &a, &b, beta, &mut axpy_out);
        assert_eq!(axpy_out, fused);

        let mut diag_out = c.clone();
        (pair.gemm_diag_fn())(alpha, &a, &b, beta, &mut diag_out);
        assert_eq!(diag_out, implicit_fused);

        let mut expr_beta = FieldMatrix::zeros(m, n);
        (pair.expr_gemm_beta_fn())(&a, &b, beta, &c, &mut expr_beta);
        assert_eq!(expr_beta, beta_only);

        let mut expr_trans = FieldMatrix::zeros(m, n);
        (pair.expr_gemm_trans_fn())(&a_transposed_storage, &b, &mut expr_trans);
        assert_eq!(expr_trans, trans_product);

        let mut expr_trans_beta = FieldMatrix::zeros(m, n);
        (pair.expr_gemm_trans_beta_fn())(
            alpha,
            &a_transposed_storage,
            &b,
            beta,
            &c,
            &mut expr_trans_beta,
        );
        assert_eq!(expr_trans_beta, trans_fused);

        let extents = pair.extents();
        let observations = gemm_tile_observations();
        assert_eq!(observations.len(), GemmTileSite::ALL.len());
        for site in GemmTileSite::ALL {
            let observed = observations
                .iter()
                .find(|observed| observed.site == site)
                .unwrap_or_else(|| panic!("missing candidate observation for {site:?}"));
            assert_eq!((observed.row_tile, observed.col_tile), extents);
        }
    }
}
