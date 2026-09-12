#![cfg(all(feature = "tuning-profile", feature = "test-support"))]

#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::alg::m4rm::{
    m4rm_effective_schedule_observation, m4rm_schedule_route, m4rm_schedule_route_for_selectors,
    multiply, reset_m4rm_effective_schedule_observation, M4rmEffectiveScheduleObservation,
    M4rmScheduleRoute, M4rmScheduleTier, M4rmTableBand, M4rmTiledEffectiveObservation,
};
use gf2_core::matrix::{
    reset_transpose_effective_observation, transpose_effective_observation, BitMatrix,
    TransposeEffectiveObservation,
};
use gf2_core::tuning::{self, M4rmSelectors, SectionResolution};

const MACRO_TILE_BLOCKS: usize = 2;

fn naive_multiply(lhs: &BitMatrix, rhs: &BitMatrix) -> BitMatrix {
    let mut out = BitMatrix::zeros(lhs.rows(), rhs.cols());
    for row in 0..lhs.rows() {
        for col in 0..rhs.cols() {
            let mut value = false;
            for inner in 0..lhs.cols() {
                value ^= lhs.get(row, inner) & rhs.get(inner, col);
            }
            out.set(row, col, value);
        }
    }
    out
}

fn assert_completed_schedule_matches(route: M4rmScheduleRoute) {
    let M4rmEffectiveScheduleObservation::Completed(schedule) =
        m4rm_effective_schedule_observation()
    else {
        panic!("the completed multiplication did not publish its effective schedule");
    };
    assert_eq!(schedule.tier(), route.tier());
    assert_eq!(schedule.table_band(), route.table_band());
    assert_eq!(schedule.table_bytes(), route.table_bytes());
    assert_eq!(schedule.panel_width_cap(), route.panel_width_cap());
    assert_eq!(schedule.panel_width(), route.panel_width());
    assert_eq!(
        schedule.tiled_stride_admitted(),
        route.tiled_stride_admitted()
    );
    assert_eq!(schedule.c_update(), M4rmTiledEffectiveObservation::RowWise);
}

support::fresh_tuning_test!(completed_extent_observations_report_consumed_values, {
    let profile = support::prepared_core_json(&format!(
        r#"{{
            "bit_matrix": {{
              "transpose_simple_max_blocks": 1,
              "transpose_macro_tile_blocks": {MACRO_TILE_BLOCKS}
            }},
            "m4rm": {{
              "wide_tier_min_stride_words": 16,
              "tiled_min_stride_words": {},
              "default_table_bytes": 16384,
              "mid_table_bytes": 32768,
              "wide_table_bytes": 65536,
              "wide_max_k": 6,
              "small_n_max_k": 5
            }}
          }}"#,
        usize::MAX
    ))
    .expect("the extent observation profile is valid");
    assert_eq!(tuning::install(profile), Ok(()));
    assert!(matches!(
        tuning::active().resolution,
        SectionResolution::Installed { .. }
    ));

    reset_transpose_effective_observation();
    assert_eq!(
        transpose_effective_observation(),
        TransposeEffectiveObservation::None
    );
    let simple = BitMatrix::random_seeded(63, 63, 0xa835_000f);
    let simple_transposed = simple.transpose();
    assert_eq!(
        transpose_effective_observation(),
        TransposeEffectiveObservation::Simple
    );
    assert_eq!(simple_transposed.transpose(), simple);

    reset_transpose_effective_observation();
    let matrix = BitMatrix::random_seeded(129, 129, 0xa835_0010);
    let transposed = matrix.transpose();
    assert_eq!(
        transpose_effective_observation(),
        TransposeEffectiveObservation::MacroTiled {
            macro_tile_blocks: MACRO_TILE_BLOCKS,
        }
    );
    assert_eq!(transposed.transpose(), matrix);

    let installed = M4rmSelectors::try_new(16, usize::MAX, 16_384, 32_768, 65_536, 6, 5)
        .expect("installed M4RM selectors are valid");
    for (index, (n, tier, band, bytes)) in [
        (512, M4rmScheduleTier::SmallN, M4rmTableBand::SmallN, None),
        (
            1024,
            M4rmScheduleTier::Wide,
            M4rmTableBand::Default,
            Some(16_384),
        ),
        (
            2048,
            M4rmScheduleTier::Wide,
            M4rmTableBand::Mid,
            Some(32_768),
        ),
        (
            4096,
            M4rmScheduleTier::Wide,
            M4rmTableBand::Wide,
            Some(65_536),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let route = m4rm_schedule_route(64, n);
        assert_eq!(route, m4rm_schedule_route_for_selectors(&installed, 64, n));
        assert_eq!(route.tier(), tier);
        assert_eq!(route.table_band(), band);
        assert_eq!(route.table_bytes(), bytes);

        reset_m4rm_effective_schedule_observation();
        assert_eq!(
            m4rm_effective_schedule_observation(),
            M4rmEffectiveScheduleObservation::None
        );
        let lhs = BitMatrix::random_seeded(1, 64, 0xa835_0011 + index as u64);
        let rhs = BitMatrix::random_seeded(64, n, 0xa835_0021 + index as u64);
        assert_eq!(multiply(&lhs, &rhs), naive_multiply(&lhs, &rhs));
        assert_completed_schedule_matches(route);
    }

    let nonconservative = M4rmSelectors::try_new(8, 16, 8_192, 32_768, 131_072, 7, 4)
        .expect("nonconservative M4RM selectors are valid");
    let explicit = m4rm_schedule_route_for_selectors(&nonconservative, 64, 512);
    assert_eq!(explicit.tier(), M4rmScheduleTier::Wide);
    assert_eq!(explicit.table_band(), M4rmTableBand::Default);
    assert_eq!(explicit.table_bytes(), Some(8_192));
    assert_eq!(explicit.panel_width_cap(), 7);
    assert_ne!(explicit, m4rm_schedule_route(64, 512));
});
