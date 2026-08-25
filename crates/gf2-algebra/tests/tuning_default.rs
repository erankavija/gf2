//! Feature-independent default witness for the algebra-owned tuning section.

use gf2_algebra::tuning::{self, AlgebraTuning, CHUNK_SUBSETS};
use gf2_core::tuning::SectionResolution;

#[test]
fn first_algebra_access_uses_the_crate_owned_conservative_declaration() {
    let active = tuning::active();

    assert_eq!(active.section, &AlgebraTuning::CONSERVATIVE);
    assert_eq!(active.permanent().gray_chunk_subsets(), CHUNK_SUBSETS);
    assert!(matches!(
        active.resolution,
        SectionResolution::FrozenBeforeInstall { .. }
    ));
}
