#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::{
    kernels::select_backend_for_size,
    tuning::{self, CoreTuning},
};

support::fresh_tuning_test!(install_does_not_govern_bit_backend_selection, {
    let conservative_threshold = CoreTuning::CONSERVATIVE.bit_backend().simd_min_words();
    let profile = support::prepared_core_json(
        r#"
        {"bit_backend": {"simd_min_words": 4}}"#,
    )
    .unwrap();

    let profile_words = 7;
    assert!(
        profile_words < conservative_threshold,
        "{profile_words} words select Scalar under the conservative profile"
    );
    assert_eq!(tuning::install(profile), Ok(()));
    // DEC-G makes the bit-backend threshold compile-time; installing a
    // runtime profile with threshold 4 does not move the selected boundary.
    #[cfg(not(gf2_tuning_baked))]
    assert_eq!(
        select_backend_for_size(profile_words).name(),
        "scalar",
        "{profile_words} words remain scalar under the conservative threshold"
    );
    #[cfg(gf2_tuning_baked)]
    assert_eq!(
        select_backend_for_size(profile_words).name(),
        "simd",
        "{profile_words} words remain SIMD under the baked threshold"
    );
    assert_eq!(
        select_backend_for_size(3).name(),
        "scalar",
        "3 words remain scalar below the baked/default threshold"
    );
});
