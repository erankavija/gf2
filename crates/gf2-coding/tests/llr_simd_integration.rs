//! Integration tests for SIMD-accelerated LLR operations.
//!
//! Tests that gf2-coding::Llr properly integrates with gf2-kernels-simd
//! and provides correct results with SIMD acceleration.

use gf2_coding::llr::Llr;

#[test]
fn test_boxplus_minsum_n_scalar_matches_simd() {
    // Test that SIMD and scalar implementations give same results
    let test_cases = vec![
        vec![Llr::new(1.0), Llr::new(2.0), Llr::new(3.0)],
        vec![Llr::new(-1.0), Llr::new(-2.0), Llr::new(-3.0)],
        vec![Llr::new(1.5), Llr::new(-2.3), Llr::new(4.7), Llr::new(-0.8)],
        vec![Llr::new(3.0), Llr::new(-2.0), Llr::new(1.0)],
    ];

    for llrs in test_cases {
        let result = Llr::boxplus_minsum_n(&llrs);

        // Compute expected with scalar implementation
        let mut min_abs = f64::INFINITY;
        let mut sign_product = 1.0f64;
        for llr in &llrs {
            let val = llr.value();
            min_abs = min_abs.min(val.abs());
            if val < 0.0 {
                sign_product = -sign_product;
            }
        }
        let expected = sign_product * min_abs;

        let diff = (result.value() - expected).abs();
        assert!(
            diff < 1e-6,
            "boxplus_minsum_n mismatch: got {}, expected {} for {:?}",
            result.value(),
            expected,
            llrs
        );
    }
}

#[test]
fn test_saturate_batch() {
    let llrs = vec![
        Llr::new(100.0),
        Llr::new(-100.0),
        Llr::new(5.0),
        Llr::new(-5.0),
    ];

    let saturated = Llr::saturate_batch(&llrs, 10.0);

    assert_eq!(saturated[0].value(), 10.0);
    assert_eq!(saturated[1].value(), -10.0);
    assert_eq!(saturated[2].value(), 5.0);
    assert_eq!(saturated[3].value(), -5.0);
}

#[test]
fn test_hard_decision_batch() {
    let llrs = vec![
        Llr::new(3.0),  // bit 0
        Llr::new(-2.0), // bit 1
        Llr::new(0.5),  // bit 0
        Llr::new(-0.1), // bit 1
        Llr::new(0.0),  // bit 0 (tie)
    ];

    let bits = Llr::hard_decision_batch(&llrs);

    assert_eq!(bits, vec![false, true, false, true, false]);
}

#[test]
fn test_large_batch_performance() {
    // Create a large batch to test SIMD efficiency
    let llrs: Vec<Llr> = (0..1000)
        .map(|i| {
            let val = (i as f64) * 0.1;
            if i % 3 == 0 {
                Llr::new(-val)
            } else {
                Llr::new(val)
            }
        })
        .collect();

    // Just verify it completes without panicking
    let result = Llr::boxplus_minsum_n(&llrs);
    assert!(result.value().is_finite());

    let saturated = Llr::saturate_batch(&llrs, 50.0);
    assert_eq!(saturated.len(), llrs.len());

    let bits = Llr::hard_decision_batch(&llrs);
    assert_eq!(bits.len(), llrs.len());
}

#[cfg(feature = "simd")]
#[test]
fn test_simd_detection() {
    // Just verify that SIMD detection doesn't panic
    // Actual detection happens internally in gf2-kernels-simd
    let llrs = vec![Llr::new(1.0), Llr::new(2.0)];
    let _ = Llr::boxplus_minsum_n(&llrs);
}
