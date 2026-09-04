//! Routing witnesses for the baked bit-backend threshold.
//!
//! These tests assert the four-word boundary recorded by
//! `dev/benchmarks/tuning_profiles/2026-09-01-eaae1b56.md` in the measured
//! format-2 core owner
//! `crates/gf2-core/data/tuning-profiles/gf2-eaae1b56-20260904-215231-898522.json`.
//! They are compiled only under the declared cfg `gf2_tuning_baked`
//! (`RUSTFLAGS="--cfg gf2_tuning_baked"`); the default build's conservative
//! boundary is asserted by `backend_selection.rs` and friends.
#![cfg(gf2_tuning_baked)]

#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::{kernels::select_backend_for_size, tuning};

#[test]
fn baked_build_routes_by_current_calibrated_value() {
    assert_eq!(select_backend_for_size(3).name(), "scalar");
    assert_eq!(select_backend_for_size(4).name(), "simd");
}

support::fresh_tuning_test!(install_does_not_govern_bit_backend_selection, {
    let profile = support::prepared_core_json(
        r#"
        {"bit_backend": {"simd_min_words": 16}}"#,
    )
    .expect("test profile is valid");
    tuning::install(profile).expect("profile has not been resolved");

    // DEC-G makes the bit-backend threshold compile-time; the installed
    // runtime profile cannot move the baked four-word boundary.
    assert_eq!(select_backend_for_size(3).name(), "scalar");
    assert_eq!(select_backend_for_size(4).name(), "simd");
});
