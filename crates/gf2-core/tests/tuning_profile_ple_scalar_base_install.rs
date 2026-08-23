//! PLE scalar-base route observation under an installed tuning profile that
//! widens `ple.scalar_base_max_cols` past the conservative single-column leaf.
//!
//! `tuning::install` resolves once per process, so this binary installs the one
//! profile it observes.

use gf2_core::field::matrix::gemm;
use gf2_core::field::ple::{ple_base_route, PleBaseRoute};
use gf2_core::field::test_random_matrix::random_fp;
use gf2_core::tuning::{self, TuningProfile};

/// Mersenne-31 sits above every PLE panel kernel's prime range, so its column
/// windows reach the scalar base and the halving split alone.
const MERSENNE_31: u64 = 2_147_483_647;

#[test]
fn installed_ple_profile_widens_the_scalar_base_window() {
    let text = r#"
    {
      "schema_version": 1,
      "profile_id": "ple-scalar-base-test",
      "provenance": {"kind": "inherited"},
      "selectors": {
        "ple": {
          "scalar_base_max_cols": 8
        }
      }
    }
    "#;
    let profile = TuningProfile::from_json(text).expect("test profile is valid");
    assert_eq!(tuning::install(profile), Ok(()));
    let ple = tuning::active().ple();
    assert_eq!(ple.scalar_base_max_cols(), 8);
    // The absent keys resolve to their conservative defaults.
    assert_eq!(
        ple.panel_base_max_cols(),
        TuningProfile::CONSERVATIVE.ple().panel_base_max_cols()
    );

    // ── Base-case boundary ──────────────────────────────────────────────────
    let scalar_base_max_cols = ple.scalar_base_max_cols();
    assert_eq!(
        ple_base_route(scalar_base_max_cols),
        PleBaseRoute::ScalarBase,
        "the widest window the direct base case handles is the field value"
    );
    assert_eq!(
        ple_base_route(scalar_base_max_cols + 1),
        PleBaseRoute::BlockRecursive
    );
    assert_eq!(ple_base_route(1), PleBaseRoute::ScalarBase);

    let conservative_base = TuningProfile::CONSERVATIVE.ple().scalar_base_max_cols();
    assert_eq!(
        ple_base_route(conservative_base + 1),
        PleBaseRoute::ScalarBase,
        "a window the conservative width sends to the block-recursive driver \
         now takes the scalar base"
    );

    // ── The widened value reaches the dispatcher ────────────────────────────
    //
    // Mersenne-31 registers no panel kernel on any host, so every window of
    // eight columns or fewer is eliminated directly by the scalar base.
    let a = random_fp::<MERSENNE_31>(48, 48, 0xA701);
    let (p, l, e, rank) = a.ple();
    assert_eq!(l.cols(), rank);
    assert_eq!(e.rows(), rank);
    assert_eq!(p.apply(&gemm(&l, &e)), a, "P · L · E != A");

    // A rank-deficient input exercises the scalar base's pivot-column scatter
    // under the widened window.
    let deficient = gemm(
        &random_fp::<MERSENNE_31>(40, 12, 0xA702),
        &random_fp::<MERSENNE_31>(12, 40, 0xA703),
    );
    let (p2, l2, e2, rank2) = deficient.ple();
    assert!(rank2 > 0 && rank2 <= 12);
    assert_eq!(
        p2.apply(&gemm(&l2, &e2)),
        deficient,
        "P · L · E != A (rank-deficient)"
    );
}
