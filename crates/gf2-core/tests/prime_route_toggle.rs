//! Toggle observation for the prime-field GEMM route reporter: the reported
//! route follows the GF(251) route-A debug switch. The switch is a
//! process-wide `AtomicBool`, so this binary holds exactly one test.
#![cfg(all(feature = "simd", not(gf2_tuning_baked)))]

use gf2_core::gfp::simd_ops::{prime_gemm_route, set_route_a_gf251_enabled, PrimeGemmRoute};
use gf2_core::tuning::CoreTuning;

#[test]
fn the_route_a_switch_moves_gf251_below_the_column_bound() {
    const M: usize = 64;
    const K: usize = 64;
    let below = CoreTuning::CONSERVATIVE
        .prime_route()
        .f32_min_cols()
        .checked_sub(1)
        .expect("the bound admits a cell below it");

    let untoggled = prime_gemm_route::<251>(M, K, below);
    assert!(
        !untoggled.is_f32_cascade(),
        "below the column bound the production default is not the cascade"
    );

    set_route_a_gf251_enabled(true);
    let toggled = prime_gemm_route::<251>(M, K, below);
    set_route_a_gf251_enabled(false);

    if toggled.is_f32_cascade() {
        // The switch forces route A at any width when the kernel is present.
        assert_eq!(toggled, PrimeGemmRoute::F32CascadeTabled);
    } else {
        // Without the f32 kernel the dispatcher falls through the route-A
        // arm exactly as it does untoggled.
        assert_eq!(
            toggled, untoggled,
            "this host registers no f32 cascade kernel, so the switch cannot \
             move the arm"
        );
    }

    assert_eq!(
        prime_gemm_route::<251>(M, K, below),
        untoggled,
        "restoring the switch restores the production arm"
    );
}
