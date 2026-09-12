//! The semantic conservative table is independent of baked selection.

#![cfg(feature = "test-support")]

#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::field::matrix::{conservative_gemm_tiles, selected_gemm_tiles};
use gf2_core::tuning::{self, CoreTuning};

support::fresh_tuning_test!(conservative_gemm_tiles_are_unconditional, {
    let conservative = conservative_gemm_tiles();
    assert_eq!(
        (
            CoreTuning::CONSERVATIVE.gemm().row_tile(),
            CoreTuning::CONSERVATIVE.gemm().col_tile(),
        ),
        conservative,
    );
    #[cfg(not(gf2_tuning_baked))]
    assert_eq!(
        selected_gemm_tiles(),
        conservative_gemm_tiles(),
        "an ordinary build must select the unconditional conservative pair"
    );

    let selected_before_install = selected_gemm_tiles();
    let profile = support::prepared_core_json(r#"{"gemm":{"row_tile":16,"col_tile":32}}"#)
        .expect("finite runtime profile is valid");
    tuning::install(profile).expect("fresh process has no installed profile");
    assert_eq!(
        (
            tuning::active().gemm().row_tile(),
            tuning::active().gemm().col_tile(),
        ),
        (16, 32),
    );
    assert_eq!(selected_gemm_tiles(), selected_before_install);
    assert_eq!(conservative_gemm_tiles(), conservative);
});
