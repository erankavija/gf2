//! Routing witnesses for the baked bit-backend threshold.
//!
//! These tests assert the `bit_backend.simd_min_words` boundary stated by the
//! measured format-2 core owner
//! `crates/gf2-core/data/tuning-profiles/gf2-dbd8787d-20261001t230000z-2601601.json`,
//! which cites
//! `dev/benchmarks/tuning_profiles/gf2-dbd8787d-20261001t230000z-2601601.md`.
//! They are compiled only under the declared cfg `gf2_tuning_baked`
//! (`RUSTFLAGS="--cfg gf2_tuning_baked"`); the default build's conservative
//! boundary is asserted by `backend_selection.rs` and friends.
#![cfg(gf2_tuning_baked)]

#[path = "support/measured_format2.rs"]
mod measured_format2;
#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::{kernels::select_backend_for_size, tuning};

/// Returns the measured owner's `bit_backend.simd_min_words`.
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

    // DEC-G makes the bit-backend threshold compile-time; the installed
    // runtime profile cannot move the baked boundary.
    assert_eq!(select_backend_for_size(boundary - 1).name(), "scalar");
    assert_eq!(select_backend_for_size(boundary).name(), "simd");
});
