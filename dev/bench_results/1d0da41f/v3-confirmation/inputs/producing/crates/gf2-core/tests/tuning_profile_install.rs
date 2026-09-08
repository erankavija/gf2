#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::tuning::{self, CoreTuning};

support::fresh_tuning_test!(installs_profile_before_active_resolution, {
    let text = r#"
    {"bit_backend": {"simd_min_words": 16}}"#;
    let profile = support::prepared_core_json(text).unwrap();
    assert_eq!(tuning::install(profile), Ok(()));
    assert_eq!(tuning::active().bit_backend().simd_min_words(), 16);
    assert!(tuning::install(support::prepared_core(CoreTuning::CONSERVATIVE)).is_err());
});
