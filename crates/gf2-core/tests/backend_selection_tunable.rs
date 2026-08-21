use gf2_core::{kernels::select_backend_for_size, tuning::TuningProfile};

#[test]
fn calibrated_profile_does_not_move_backend_boundary_below_conservative_default() {
    let conservative_threshold = TuningProfile::CONSERVATIVE.bit_backend().simd_min_words();
    assert_eq!(conservative_threshold, 8);

    let profile = TuningProfile::from_json(
        r#"
        {
          "schema_version": 1,
          "profile_id": "backend-selection-tunable-test",
          "provenance": {"kind": "inherited"},
          "selectors": {"bit_backend": {"simd_min_words": 4}}
        }
        "#,
    )
    .expect("test profile is valid");
    assert_eq!(profile.bit_backend().simd_min_words(), 4);
    gf2_core::tuning::install(profile).expect("profile has not been resolved");

    for words in 4..=7 {
        // DEC-G keeps the bit-backend boundary compile-time, so these words
        // remain scalar even after installing the calibrated profile.
        assert!(
            words < conservative_threshold,
            "{words} words sits below the conservative threshold of {conservative_threshold}"
        );
        #[cfg(not(gf2_tuning_baked))]
        assert_eq!(
            select_backend_for_size(words).name(),
            "scalar",
            "{words} words remain scalar under the conservative threshold"
        );
        #[cfg(gf2_tuning_baked)]
        assert_eq!(
            select_backend_for_size(words).name(),
            "simd",
            "{words} words remain SIMD under the baked threshold"
        );
    }
    assert_eq!(
        select_backend_for_size(3).name(),
        "scalar",
        "3 words remain below the baked/default threshold"
    );
}
