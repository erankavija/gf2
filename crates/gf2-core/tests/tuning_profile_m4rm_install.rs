//! Installed-profile route observation for the M4RM schedule selectors, with
//! the wide-tier boundary moved *below* the conservative default.
//!
//! `tuning::install` is one-shot per process, so this file owns exactly one
//! installed profile and one test, following
//! `tests/tuning_profile_polynomial_install.rs`. Every size is derived from the
//! installed profile's accessors rather than from a literal.

#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::alg::m4rm::{m4rm_schedule_route, multiply, M4rmScheduleTier};
use gf2_core::matrix::BitMatrix;
use gf2_core::tuning;

/// Panel widths this profile produces per stride band, all distinct from the
/// conservative table's:
///
/// * stride 7 (below the boundary) — the small-`n` ceiling binds at 3;
/// * stride 8 (the boundary) — the wide ceiling binds at 6;
/// * strides 24, 32, 64 — the default, middle, and wide byte budgets bind.
const PROFILE: &str = r#"
{
    "m4rm": {
      "wide_tier_min_stride_words": 8,
      "tiled_min_stride_words": 8,
      "default_table_bytes": 8192,
      "mid_table_bytes": 4096,
      "wide_table_bytes": 16384,
      "wide_max_k": 6,
      "small_n_max_k": 3
    }
  }"#;

support::fresh_tuning_test!(
    installed_m4rm_profile_moves_the_tier_boundary_and_binds_every_budget,
    {
        let profile = support::prepared_core_json(PROFILE).expect("test profile is valid");
        assert_eq!(tuning::install(profile), Ok(()));
        let active_tuning = tuning::active();
        let m4rm = active_tuning.m4rm();
        assert_eq!(m4rm.wide_tier_min_stride_words(), 8);
        assert_eq!(m4rm.tiled_min_stride_words(), 8);
        assert_eq!(m4rm.default_table_bytes(), 8192);
        assert_eq!(m4rm.mid_table_bytes(), 4096);
        assert_eq!(m4rm.wide_table_bytes(), 16384);
        assert_eq!(m4rm.wide_max_k(), 6);
        assert_eq!(m4rm.small_n_max_k(), 3);

        let boundary = m4rm.wide_tier_min_stride_words();
        let k = 4096;

        // The widest output that still falls below the installed boundary, and the
        // narrowest that reaches it. The conservative table puts both in the
        // small-`n` tier, so the reported tiers observe the installed value.
        let n_below = (boundary - 1) * 64;
        let n_at = n_below + 1;
        assert_eq!(
            m4rm_schedule_route(k, n_below).tier(),
            M4rmScheduleTier::SmallN
        );
        assert_eq!(m4rm_schedule_route(k, n_at).tier(), M4rmScheduleTier::Wide);

        // Below the boundary the installed small-`n` ceiling is what binds: the
        // cost-balance heuristic asks for a wider panel at this width.
        assert_eq!(
            m4rm_schedule_route(k, n_below).panel_width(),
            m4rm.small_n_max_k()
        );

        // At the boundary the installed default budget admits a wider table than
        // the installed wide ceiling, so the ceiling binds.
        assert_eq!(
            m4rm_schedule_route(k, n_at).panel_width(),
            m4rm.wide_max_k()
        );

        // Wider strides make each of the three byte budgets the binding
        // constraint: the selected panel is the widest whose table fits the
        // installed budget, and it stays under the installed ceiling.
        for (stride_words, budget) in [
            (24usize, m4rm.default_table_bytes()),
            (32, m4rm.mid_table_bytes()),
            (64, m4rm.wide_table_bytes()),
        ] {
            let n = stride_words * 64;
            let route = m4rm_schedule_route(k, n);
            assert_eq!(
                route.tier(),
                M4rmScheduleTier::Wide,
                "stride {stride_words} is above the installed boundary"
            );
            let width = route.panel_width();
            let bytes_per_entry = stride_words * 8;
            assert!(
                width < m4rm.wide_max_k(),
                "stride {stride_words}: the budget binds below the ceiling, got {width}"
            );
            assert!(
                (1usize << width) * bytes_per_entry <= budget,
                "stride {stride_words}: panel {width} exceeds the installed budget {budget}"
            );
            assert!(
                (1usize << (width + 1)) * bytes_per_entry > budget,
                "stride {stride_words}: panel {width} leaves room for a wider one"
            );
        }

        // The register-tiled C-update gate follows the installed
        // `tiled_min_stride_words`, on both sides of it. The conservative table
        // admits the tile at every stride from 4 up, so the stride below the
        // installed value is where the two tables disagree.
        let tiled_min = m4rm.tiled_min_stride_words();
        let n_tiled_below = (tiled_min - 1) * 64;
        let n_tiled_at = n_tiled_below + 1;
        assert!(!m4rm_schedule_route(k, n_tiled_below).tiled_stride_admitted());
        assert!(m4rm_schedule_route(k, n_tiled_at).tiled_stride_admitted());
        assert!(!m4rm_schedule_route(k, (tiled_min - 3) * 64).tiled_stride_admitted());

        // The product is still the product on both sides of the moved boundary,
        // with the register-tiled C-update gate at the installed
        // `tiled_min_stride_words` rather than the conservative one.
        let rows = 16;
        let inner = 33;
        for n in [n_below, n_at] {
            let a = BitMatrix::random_seeded(rows, inner, 0x6d34_a904);
            let b = BitMatrix::random_seeded(inner, n, 0x9a11_0f5c);
            assert_eq!(multiply(&a, &b), naive_multiply(&a, &b), "n={n}");
        }
    }
);

/// Reference product used as the correctness witness: no schedule, no tables.
fn naive_multiply(a: &BitMatrix, b: &BitMatrix) -> BitMatrix {
    let mut c = BitMatrix::zeros(a.rows(), b.cols());
    for row in 0..a.rows() {
        for inner in 0..a.cols() {
            if a.get(row, inner) {
                for col in 0..b.cols() {
                    if b.get(inner, col) {
                        c.set(row, col, !c.get(row, col));
                    }
                }
            }
        }
    }
    c
}
