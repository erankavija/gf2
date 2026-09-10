//! Installed-profile route observation for the bit-matrix transpose selectors.
//!
//! The process installs one profile and observes both runtime-resolved values
//! through the production `transpose_route` reporter. The reporter is also
//! the selector called by `BitMatrix::transpose`.

#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::matrix::{transpose_route, TransposeRoute};
use gf2_core::tuning::{self, CoreTuning};

const INSTALLED_SIMPLE_MAX_BLOCKS: usize = 3;
const INSTALLED_MACRO_TILE_BLOCKS: usize = 5;

support::fresh_tuning_test!(installed_matrix_profile_moves_both_transpose_selectors, {
    let conservative_profile = CoreTuning::CONSERVATIVE;
    let conservative = conservative_profile.bit_matrix();
    let profile = support::prepared_core_json(
        r#"
        {
            "bit_matrix": {
              "transpose_simple_max_blocks": 3,
              "transpose_macro_tile_blocks": 5
            }
          }"#,
    )
    .expect("test profile is valid");

    assert_ne!(
        INSTALLED_SIMPLE_MAX_BLOCKS,
        conservative.transpose_simple_max_blocks(),
        "the installed simple-loop boundary must differ from the conservative default"
    );
    assert_ne!(
        INSTALLED_MACRO_TILE_BLOCKS,
        conservative.transpose_macro_tile_blocks(),
        "the installed macro-tile extent must differ from the conservative default"
    );
    assert_eq!(tuning::install(profile), Ok(()));

    // The witness cannot silently run against the conservative table: both
    // resolved values must be active before the production route is observed.
    let active_tuning = tuning::active();
    let bit_matrix = active_tuning.bit_matrix();
    assert_eq!(
        bit_matrix.transpose_simple_max_blocks(),
        INSTALLED_SIMPLE_MAX_BLOCKS
    );
    assert_eq!(
        bit_matrix.transpose_macro_tile_blocks(),
        INSTALLED_MACRO_TILE_BLOCKS
    );

    // Boundary observations come from the production reporter, which the
    // transpose dispatcher calls, rather than from a test-local comparison.
    assert_eq!(
        transpose_route(INSTALLED_SIMPLE_MAX_BLOCKS, INSTALLED_SIMPLE_MAX_BLOCKS),
        TransposeRoute::Simple
    );
    assert_eq!(
        transpose_route(INSTALLED_SIMPLE_MAX_BLOCKS + 1, INSTALLED_SIMPLE_MAX_BLOCKS),
        TransposeRoute::MacroTiled {
            macro_tile_blocks: INSTALLED_MACRO_TILE_BLOCKS,
        }
    );
    assert_eq!(
        transpose_route(INSTALLED_SIMPLE_MAX_BLOCKS, INSTALLED_SIMPLE_MAX_BLOCKS + 1),
        TransposeRoute::MacroTiled {
            macro_tile_blocks: INSTALLED_MACRO_TILE_BLOCKS,
        }
    );
    assert_eq!(
        transpose_route(
            INSTALLED_SIMPLE_MAX_BLOCKS + 1,
            INSTALLED_SIMPLE_MAX_BLOCKS + 1
        )
        .macro_tile_blocks(),
        Some(INSTALLED_MACRO_TILE_BLOCKS)
    );
});
