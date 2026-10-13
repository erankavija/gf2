//! Default block-CSR layout under an installed tuning profile that states
//! `sparse.block_csr_default_rows`.

#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::sparse::SpBitMatrix;
use gf2_core::tuning::{self, CoreTuning};
use gf2_core::BitVec;

#[test]
fn a_zero_block_row_count_is_rejected() {
    let error = support::prepared_core_json(r#"{"sparse": {"block_csr_default_rows": 0}}"#)
        .expect_err("a block holds at least one row");
    assert!(error.contains("sparse.block_csr_default_rows"), "{error}");
}

support::fresh_tuning_test!(installed_row_count_shapes_the_default_block_layout, {
    let conservative = CoreTuning::CONSERVATIVE.sparse().block_csr_default_rows();
    let installed = conservative + 5;
    let profile = support::prepared_core_json(&format!(
        r#"{{"sparse": {{"block_csr_default_rows": {installed}}}}}"#
    ))
    .expect("test profile is valid");
    assert_eq!(tuning::install(profile), Ok(()));
    assert_eq!(
        tuning::active().sparse().block_csr_default_rows(),
        installed
    );

    // Rows on both sides of one installed block, columns across a word boundary.
    for rows in [0, 1, installed - 1, installed, installed + 1, 3 * installed] {
        for cols in [1, 63, 64, 65] {
            let entries: Vec<(usize, usize)> = (0..rows)
                .flat_map(|row| [(row, row % cols), (row, (7 * row + 3) % cols)])
                .collect();
            let csr = SpBitMatrix::from_coo_deduplicated(rows, cols, &entries);
            let mut x = BitVec::with_capacity(cols);
            for bit in 0..cols {
                x.push_bit(bit % 3 != 1);
            }

            let blocked = csr.to_default_block_csr();
            assert_eq!(blocked.block_rows(), installed);
            assert_eq!(blocked, csr.to_block_csr(installed));
            assert_eq!(
                blocked.matvec(&x),
                csr.matvec(&x),
                "rows={rows} cols={cols}"
            );
        }
    }
});
