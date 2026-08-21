use gf2_core::{
    kernels::select_backend_for_size,
    tuning::{self, TuningProfile},
};

#[test]
fn installed_profile_above_default_does_not_move_production_route() {
    let conservative_threshold = TuningProfile::CONSERVATIVE.bit_backend().simd_min_words();
    let profile = TuningProfile::from_json(
        r#"
        {
          "schema_version": 1,
          "profile_id": "backend-selection-above-default-test",
          "provenance": {"kind": "inherited"},
          "selectors": {"bit_backend": {"simd_min_words": 16}}
        }
        "#,
    )
    .expect("test profile is valid");

    assert!(conservative_threshold < 16);
    tuning::install(profile).expect("profile has not been resolved");

    assert_eq!(
        tuning::active().bit_backend().simd_min_words(),
        16,
        "the installed profile is observable after install returns"
    );

    assert_eq!(
        select_backend_for_size(conservative_threshold).name(),
        "simd",
        "DEC-G keeps the compile-time default boundary at the conservative value"
    );
    assert_eq!(
        select_backend_for_size(8).name(),
        "simd",
        "the installed threshold does not govern bit-backend selection"
    );
    assert_eq!(select_backend_for_size(16).name(), "simd");
}
