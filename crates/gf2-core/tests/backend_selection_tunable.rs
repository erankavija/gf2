use gf2_core::{kernels::select_backend_for_size, tuning::TuningProfile};

#[test]
fn calibrated_profile_moves_backend_boundary_below_conservative_default() {
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
        // Below the conservative threshold, so the conservative profile would
        // select Scalar here. That side is asserted by backend_selection.rs in
        // its own binary; this process has already installed, and active()
        // resolves once per process.
        assert!(
            words < conservative_threshold,
            "{words} words sits below the conservative threshold of {conservative_threshold}"
        );
        assert_eq!(
            select_backend_for_size(words).name(),
            "simd",
            "{words} words select SIMD under the installed threshold"
        );
    }
    assert_eq!(
        select_backend_for_size(3).name(),
        "scalar",
        "3 words remain below the installed threshold"
    );
}
