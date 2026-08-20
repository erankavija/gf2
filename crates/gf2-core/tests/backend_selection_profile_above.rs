use gf2_core::{
    kernels::select_backend_for_size,
    tuning::{self, TuningProfile},
};

#[test]
fn installed_profile_above_default_moves_production_route() {
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
        select_backend_for_size(conservative_threshold).name(),
        "scalar",
        "the installed higher threshold moves the default boundary to scalar"
    );
    assert_eq!(select_backend_for_size(16).name(), "simd");
}
