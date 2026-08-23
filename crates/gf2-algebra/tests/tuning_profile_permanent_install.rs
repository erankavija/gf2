//! Installed-profile witness for `permanent.gray_chunk_subsets` (jit:ef18c60b).
//!
//! `dev/active/7d824b2f/design.md` §3.12 and §4 require a runtime resolved
//! read at [`permanent_bipedal3_parallel`] that reaches the existing
//! `_with_chunk` parameter. `tuning::install` resolves the process-wide
//! profile once, so this binary installs exactly one profile, per §4's
//! "one installed profile per test binary" rule.
//!
//! This binary installs a small `gray_chunk_subsets` value, forcing many
//! small Gray-code chunks. The companion binary
//! `tuning_profile_permanent_install_large_chunk.rs` installs a large value
//! that forces a single chunk. Both assert their result equals the same
//! serial reference for the same matrix, which is the determinism witness
//! `dev/active/7d824b2f/design.md` §7.2 requires: the observable permanent
//! value is identical across installed chunk lengths.

use gf2_algebra::packed::Bipedal3Matrix;
use gf2_algebra::permanent::parallel_bipedal3::{
    permanent_bipedal3_parallel_with_chunk, permanent_chunk_len,
};
use gf2_algebra::permanent::{permanent_bipedal3, permanent_bipedal3_parallel};
use gf2_core::gfp::Fp;
use gf2_core::tuning::{
    self, PermanentSelectors, ProfileId, Provenance, SelectorFamilies, TuningProfile,
};

const SMALL_CHUNK: usize = 3;

fn test_matrix() -> Bipedal3Matrix {
    // n = 8 gives 2^8 - 1 = 255 non-empty subsets, so SMALL_CHUNK = 3 spans
    // 85 chunks — many chunk starts, each independently reconstructing its
    // Gray-code column sum.
    let n = 8;
    let data: Vec<Fp<3>> = (0..n * n)
        .map(|i| Fp::<3>::new((i as u64 * 7 + 1) % 3))
        .collect();
    Bipedal3Matrix::from_row_major(&data, n, n)
}

#[test]
fn installed_permanent_profile_resolved_chunk_reaches_with_chunk() {
    let mut selectors = SelectorFamilies::CONSERVATIVE.clone();
    selectors.permanent = PermanentSelectors::try_new(SMALL_CHUNK)
        .expect("small chunk is a valid gray_chunk_subsets value");
    let profile = TuningProfile::try_new(
        ProfileId::parse("permanent-chunk-small").expect("valid kebab-case profile id"),
        Provenance::Inherited,
        selectors,
    )
    .expect("profile with only permanent.gray_chunk_subsets set is valid");
    assert_eq!(tuning::install(profile), Ok(()));

    // The resolved-read accessor reports the installed value, not the
    // compiled-in `CHUNK_SUBSETS` default.
    assert_eq!(permanent_chunk_len(), SMALL_CHUNK);

    let mat = test_matrix();
    let reference = permanent_bipedal3(&mat);

    // The production auto-resolving entry point reaches the same chunk value
    // the accessor reports: this is the "reaches `_with_chunk`" witness.
    assert_eq!(
        permanent_bipedal3_parallel(&mat),
        permanent_bipedal3_parallel_with_chunk(&mat, permanent_chunk_len())
    );

    // Correctness holds under the installed (small, heavily-partitioned) chunk.
    assert_eq!(permanent_bipedal3_parallel(&mat), reference);
}
