#[cfg(gf2_tuning_baked)]
#[path = "support/measured_format2.rs"]
mod measured_format2;
#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::{kernels::select_backend_for_size, tuning::CoreTuning};

support::fresh_tuning_test!(
    installed_section_does_not_move_backend_boundary_below_conservative_default,
    {
        let conservative_threshold = CoreTuning::CONSERVATIVE.bit_backend().simd_min_words();
        assert_eq!(conservative_threshold, 8);

        let prepared = support::prepared_core_json(
            r#"
        {"bit_backend": {"simd_min_words": 4}}"#,
        )
        .expect("test profile is valid");
        assert_eq!(
            prepared
                .section::<CoreTuning>()
                .unwrap()
                .unwrap()
                .section
                .bit_backend()
                .simd_min_words(),
            4
        );
        gf2_core::tuning::install(prepared).expect("profile has not been resolved");

        #[cfg(not(gf2_tuning_baked))]
        let compiled_threshold = conservative_threshold;
        #[cfg(gf2_tuning_baked)]
        let compiled_threshold =
            measured_format2::measured_section("bit_backend", &["simd_min_words"], &[])
                .bit_backend()
                .simd_min_words();
        for words in 4..=7 {
            // The bit-backend boundary is compile-time, so these words follow it
            // after a section with a lower value is installed.
            assert!(
                words < conservative_threshold,
                "{words} words sits below the conservative threshold of {conservative_threshold}"
            );
            let expected = if words < compiled_threshold {
                "scalar"
            } else {
                "simd"
            };
            assert_eq!(
                select_backend_for_size(words).name(),
                expected,
                "{words} words follow the compile-time threshold {compiled_threshold}"
            );
        }
        assert_eq!(
            select_backend_for_size(3).name(),
            "scalar",
            "3 words remain below the baked/default threshold"
        );
    }
);
