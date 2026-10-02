#[cfg(gf2_tuning_baked)]
#[path = "support/measured_format2.rs"]
mod measured_format2;
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
    let compiled_threshold = conservative_threshold;
    #[cfg(gf2_tuning_baked)]
    let compiled_threshold =
        measured_format2::measured_section("bit_backend", &["simd_min_words"], &[])
            .bit_backend()
            .simd_min_words();
    let expected = if profile_words < compiled_threshold {
        "scalar"
    } else {
        "simd"
    };
    assert_eq!(
        select_backend_for_size(profile_words).name(),
        expected,
        "{profile_words} words follow the compile-time threshold {compiled_threshold}"
    );
    assert_eq!(
        select_backend_for_size(3).name(),
        "scalar",
        "3 words remain scalar below the baked/default threshold"
    );
});
