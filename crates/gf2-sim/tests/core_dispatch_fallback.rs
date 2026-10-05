//! The `gf2-core` kernel-dispatch fallback contract on the build of that crate
//! this package's dependency graph selects.

use gf2_core::dispatch_contract::{assert_fallback_contract, build};

#[test]
fn the_dependency_build_keeps_the_fallback_contract() {
    assert!(
        build().simd_feature,
        "the default features of `gf2-coding` and `gf2-algebra` enable `gf2-core/simd`"
    );
    for witness in assert_fallback_contract() {
        println!("{}", witness.record());
    }
}
