//! Companion to `tuning_profile_permanent_install.rs`: installs a large
//! `permanent.gray_chunk_subsets` value in its own process (`tuning::install`
//! resolves once per binary), forcing a single Gray-code chunk instead of the
//! many small chunks the sibling binary forces.
//!
//! Both binaries assert their [`permanent_bipedal3_parallel`] result equals
//! the same serial [`permanent_bipedal3`] reference for the identical matrix,
//! which is the "results are identical across installed chunk lengths"
//! determinism witness `dev/active/7d824b2f/design.md` §7.2 requires.

use gf2_algebra::packed::Bipedal3Matrix;
use gf2_algebra::permanent::parallel_bipedal3::{
    permanent_bipedal3_parallel_with_chunk, permanent_chunk_len,
};
use gf2_algebra::permanent::{permanent_bipedal3, permanent_bipedal3_parallel};
use gf2_core::gfp::Fp;
use gf2_core::tuning::{
    self, PermanentSelectors, ProfileId, Provenance, SelectorFamilies, TuningProfile,
};

// Larger than 2^8 - 1 = 255 (the total non-empty-subset count for the n=8
// matrix below), so the Gray-code walk resolves to exactly one chunk.
const LARGE_CHUNK: usize = 4_000_000;

fn test_matrix() -> Bipedal3Matrix {
    // Same construction as `tuning_profile_permanent_install.rs`'s matrix, so
    // the two binaries' reference permanents are the same value.
    let n = 8;
    let data: Vec<Fp<3>> = (0..n * n)
        .map(|i| Fp::<3>::new((i as u64 * 7 + 1) % 3))
        .collect();
    Bipedal3Matrix::from_row_major(&data, n, n)
}

#[test]
fn installed_permanent_profile_large_chunk_matches_small_chunk_determinism() {
    let mut selectors = SelectorFamilies::CONSERVATIVE.clone();
    selectors.permanent = PermanentSelectors::try_new(LARGE_CHUNK)
        .expect("large chunk is a valid gray_chunk_subsets value");
    let profile = TuningProfile::try_new(
        ProfileId::parse("permanent-chunk-large").expect("valid kebab-case profile id"),
        Provenance::Inherited,
        selectors,
    )
    .expect("profile with only permanent.gray_chunk_subsets set is valid");
    assert_eq!(tuning::install(profile), Ok(()));

    assert_eq!(permanent_chunk_len(), LARGE_CHUNK);

    let mat = test_matrix();
    let reference = permanent_bipedal3(&mat);

    assert_eq!(
        permanent_bipedal3_parallel(&mat),
        permanent_bipedal3_parallel_with_chunk(&mat, permanent_chunk_len())
    );

    // Same matrix, same serial reference as the small-chunk binary: proves
    // the installed chunk length (large here, small there) does not move the
    // observable result.
    assert_eq!(permanent_bipedal3_parallel(&mat), reference);
}
