//! Prime-field GEMM under an installed tuning profile that narrows
//! `prime_route.f32_k_chunk_cap` and `prime_route.f64_k_chunk_cap`: the caps
//! move where the cascades reduce their accumulators, and the product and the
//! reported arm stay those of the conservative table.

#[path = "support/core_tuning.rs"]
mod support;

use gf2_core::bench_seed::fp_matrix_from_seed;
use gf2_core::field::matrix::gemm;
use gf2_core::tuning::{self, CoreTuning};

/// Asserts `gemm` against the schoolbook sum over canonical residues.
fn assert_schoolbook_product<const P: u64>(m: usize, k: usize, n: usize) {
    let a = fp_matrix_from_seed::<P>(m, k, 0x5eed_0817);
    let b = fp_matrix_from_seed::<P>(k, n, 0x5eed_fc47);
    let product = gemm(&a, &b);
    for i in 0..m {
        for j in 0..n {
            let sum = (0..k).fold(0u64, |sum, t| {
                (sum + a.get(i, t).value() * b.get(t, j).value()) % P
            });
            assert_eq!(product.get(i, j).value(), sum, "P={P} cell ({i}, {j})");
        }
    }
}

#[test]
fn chunk_caps_outside_their_range_are_rejected() {
    let above = gf2_kernels_simd::fp_medium_f64::K_CHUNK_CAP_MAX + 1;
    for (field, value) in [
        ("f32_k_chunk_cap", 0),
        ("f64_k_chunk_cap", 0),
        ("f64_k_chunk_cap", above),
    ] {
        let body = format!(r#"{{"prime_route": {{"{field}": {value}}}}}"#);
        let error = support::prepared_core_json(&body).expect_err("the cap is out of range");
        assert!(error.contains(&format!("prime_route.{field}")), "{error}");
    }
}

support::fresh_tuning_test!(installed_chunk_caps_keep_the_product_and_the_route, {
    let conservative = CoreTuning::CONSERVATIVE.prime_route();
    let (f32_cap, f64_cap) = (7, 5);
    assert!(f32_cap < conservative.f32_k_chunk_cap());
    assert!(f64_cap < conservative.f64_k_chunk_cap());
    let profile = support::prepared_core_json(&format!(
        r#"{{"prime_route": {{"f32_k_chunk_cap": {f32_cap}, "f64_k_chunk_cap": {f64_cap}}}}}"#
    ))
    .expect("test profile is valid");
    assert_eq!(tuning::install(profile), Ok(()));
    let active = tuning::active();
    let prime_route = active.prime_route();
    assert_eq!(prime_route.f32_k_chunk_cap(), f32_cap);
    assert_eq!(prime_route.f64_k_chunk_cap(), f64_cap);
    assert_eq!(prime_route.f32_min_cols(), conservative.f32_min_cols());
    assert_eq!(prime_route.f64_min_cols(), conservative.f64_min_cols());

    // Inner dimensions on both sides of each installed cap and off its
    // multiples, at the narrowest output width each cascade takes.
    for k in [1, f64_cap, f32_cap, f32_cap + 1, 4 * f32_cap * f64_cap + 3] {
        assert_schoolbook_product::<251>(5, k, conservative.f32_min_cols());
        assert_schoolbook_product::<65521>(5, k, conservative.f64_min_cols());
    }

    #[cfg(feature = "simd")]
    {
        use gf2_core::gfp::simd_ops::{prime_gemm_k_chunk_cap, prime_gemm_route, PrimeGemmRoute};
        let small = prime_gemm_route::<251>(5, 64, conservative.f32_min_cols());
        let medium = prime_gemm_route::<65521>(5, 64, conservative.f64_min_cols());
        if gf2_kernels_simd::fp_small_f32::detect().is_some() {
            assert_eq!(small, PrimeGemmRoute::F32CascadeTabled);
            assert_eq!(medium, PrimeGemmRoute::F64Cascade);
        }
        assert_eq!(
            prime_gemm_k_chunk_cap(PrimeGemmRoute::F32CascadeTabled),
            Some(f32_cap)
        );
        assert_eq!(
            prime_gemm_k_chunk_cap(PrimeGemmRoute::F64Cascade),
            Some(f64_cap)
        );
        assert_eq!(prime_gemm_k_chunk_cap(PrimeGemmRoute::U16Panel), None);
    }
});
