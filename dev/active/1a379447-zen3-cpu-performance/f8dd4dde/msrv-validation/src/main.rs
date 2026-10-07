//! Prints the two facts `validate-msrv.sh` cannot get from a bare nextest
//! pass/fail: the host's own `bmi2` detection, and the route
//! `gf2_core::residual_shift`'s lane witness reports for each arm of its
//! force switch. Single-threaded and process-wide state is untouched by
//! any other process, so no serialization is needed here (unlike the
//! shared test suite, which runs both arms inside one test binary).

use gf2_core::residual_shift::{
    force_scalar_residual_shift, last_residual_shift_route, reset_last_residual_shift_route,
};
use gf2_core::BitVec;

/// Runs one residual shift and returns the route the witness recorded,
/// mirroring `residual_shift_routes.rs`'s own `observed_route` helper.
fn observed_route() -> &'static str {
    reset_last_residual_shift_route();
    let mut bv = BitVec::ones(200);
    bv.shift_left(65);
    last_residual_shift_route()
        .expect("a shift by 65 bits reaches the residual branch")
        .name()
}

fn main() {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    let host_bmi2 = std::arch::is_x86_feature_detected!("bmi2");
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    let host_bmi2 = false;
    println!("host_bmi2={host_bmi2}");

    force_scalar_residual_shift(true);
    println!("arm=scalar-forced route={}", observed_route());

    force_scalar_residual_shift(false);
    println!("arm=released route={}", observed_route());
}
