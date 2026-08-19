use gf2_core::tuning::{self, AlreadyResolved};

#[test]
fn installs_profile_before_active_resolution() {
    let text = r#"
    {
      "schema_version": 1,
      "profile_id": "test-profile",
      "provenance": {"kind": "inherited"},
      "selectors": {"bit_backend": {"simd_min_words": 16}}
    }
    "#;
    let profile = gf2_core::tuning::TuningProfile::from_json(text).unwrap();
    assert_eq!(tuning::install(profile), Ok(()));
    assert_eq!(tuning::active().bit_backend().simd_min_words(), 16);
    assert_eq!(
        tuning::install(gf2_core::tuning::TuningProfile::CONSERVATIVE),
        Err(AlreadyResolved)
    );
}
