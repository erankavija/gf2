use gf2_core::{
    kernels::select_backend_for_size,
    tuning::{self, TuningProfile},
};

#[test]
fn installed_profile_changes_backend_selection_and_preserves_scalar_fallback() {
    let conservative_threshold = TuningProfile::CONSERVATIVE.bit_backend().simd_min_words();
    let profile = TuningProfile::from_json(
        r#"
        {
          "schema_version": 1,
          "profile_id": "backend-selection-test",
          "provenance": {"kind": "inherited"},
          "selectors": {"bit_backend": {"simd_min_words": 4}}
        }
        "#,
    )
    .unwrap();

    let profile_words = 7;
    assert!(
        profile_words < conservative_threshold,
        "{profile_words} words select Scalar under the conservative profile"
    );
    assert_eq!(tuning::install(profile), Ok(()));
    assert_eq!(
        select_backend_for_size(profile_words).name(),
        "simd",
        "{profile_words} words select Simd under the installed profile"
    );
    assert_eq!(
        select_backend_for_size(3).name(),
        "scalar",
        "3 words remain Scalar below the installed profile threshold"
    );
}
