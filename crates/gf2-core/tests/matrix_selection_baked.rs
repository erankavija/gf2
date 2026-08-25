//! Baked routing witness for `bit_matrix.matvec_simd_min_words`.
//!
//! This target is compiled by the CI baked step with
//! `RUSTFLAGS="--cfg gf2_tuning_baked"`. The installed profile deliberately
//! carries a different value; the production route remains at the baked
//! boundary.
#![cfg(gf2_tuning_baked)]

#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::matrix::{matvec_route, MatvecRoute};
use gf2_core::tuning::{self, CoreTuning};

support::fresh_tuning_test!(baked_matvec_route_ignores_runtime_profile_install, {
    let conservative = CoreTuning::CONSERVATIVE
        .bit_matrix()
        .matvec_simd_min_words();
    let installed = conservative + 8;
    let profile = support::prepared_core_json(&format!(
        r#"{{"bit_matrix": {{"matvec_simd_min_words": {installed}}}}}"#
    ))
    .expect("test profile is valid");

    assert_ne!(installed, conservative);
    assert_eq!(tuning::install(profile), Ok(()));
    assert_eq!(
        tuning::active().bit_matrix().matvec_simd_min_words(),
        installed,
        "the test must prove the installed profile differs before observing the baked route"
    );

    assert_eq!(
        matvec_route(conservative - 1),
        MatvecRoute::Scalar,
        "the production route must remain below the baked boundary"
    );
    assert_eq!(
        matvec_route(conservative),
        MatvecRoute::Simd,
        "the production route must remain at the baked boundary"
    );
});
