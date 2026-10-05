//! The `gf2-core` kernel-dispatch fallback contract on the build of that crate
//! this package's dependency graph selects.

use gf2_core::dispatch_contract::{assert_fallback_contract, build};

#[test]
fn the_dependency_build_keeps_the_fallback_contract() {
    if cfg!(feature = "simd") {
        assert!(
            build().simd_feature,
            "this package's `simd` feature enables `gf2-core/simd`"
        );
    }
    for witness in assert_fallback_contract() {
        println!("{}", witness.record());
    }
}
