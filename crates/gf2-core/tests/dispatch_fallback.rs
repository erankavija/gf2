//! The kernel-dispatch fallback contract on this crate's own build.

#![cfg(feature = "test-support")]

use gf2_core::dispatch_contract::assert_fallback_contract;

#[test]
fn every_route_keeps_the_fallback_contract() {
    for witness in assert_fallback_contract() {
        println!("{}", witness.record());
    }
}
