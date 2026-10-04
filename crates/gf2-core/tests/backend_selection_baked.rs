//! Routing witnesses for the `bit_backend.simd_min_words` boundary of the
//! measured format-2 core owner, compiled only under `--cfg gf2_tuning_baked`.
#![cfg(gf2_tuning_baked)]

#[path = "support/measured_format2.rs"]
mod measured_format2;
#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::{kernels::select_backend_for_size, tuning};

fn measured_simd_min_words() -> usize {
    measured_format2::measured_section("bit_backend", &["simd_min_words"], &[])
        .bit_backend()
        .simd_min_words()
}

#[test]
fn baked_build_routes_by_current_calibrated_value() {
    let boundary = measured_simd_min_words();
    assert_eq!(select_backend_for_size(boundary - 1).name(), "scalar");
    assert_eq!(select_backend_for_size(boundary).name(), "simd");
}

support::fresh_tuning_test!(install_does_not_govern_bit_backend_selection, {
    let boundary = measured_simd_min_words();
    let installed = boundary * 2;
    let profile = support::prepared_core_json(&format!(
        r#"{{"bit_backend": {{"simd_min_words": {installed}}}}}"#
    ))
    .expect("test profile is valid");
    tuning::install(profile).expect("profile has not been resolved");
    assert_eq!(tuning::active().bit_backend().simd_min_words(), installed);

    // The bit-backend threshold is compile-time; an installed runtime profile
    // does not move it.
    assert_eq!(select_backend_for_size(boundary - 1).name(), "scalar");
    assert_eq!(select_backend_for_size(boundary).name(), "simd");
});
