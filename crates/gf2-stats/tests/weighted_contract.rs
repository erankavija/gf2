use gf2_stats::weighted::{ExponentHistogram, ScaledStudentInterval, WeightedRuns};

#[test]
fn exact_exponent_histograms_drive_weighted_run_statistics() {
    let mut first = ExponentHistogram::new(3, 12).unwrap();
    first.record_many(2, 3).unwrap();
    first.record(5).unwrap();
    let mut second = ExponentHistogram::new(3, 12).unwrap();
    second.record_many(3, 4).unwrap();

    let summary = WeightedRuns::from_histograms(vec![first, second]).unwrap();
    assert_eq!(summary.sample_count(), 8);
    assert_eq!(summary.weight_sum_decimal(), ("118".into(), "243".into()));
    assert_eq!(
        summary.squared_weight_sum_decimal(),
        ("2512".into(), "59049".into())
    );
    assert_eq!(summary.mean_decimal(), ("59".into(), "972".into()));
    assert_eq!(
        summary.final_weight_ess_decimal(),
        ("3481".into(), "628".into())
    );
    assert!(summary.ess_fraction_at_least(1_u8, 100_u8).unwrap());
    assert_eq!(summary.run_count(), 2);
}

#[test]
fn scaled_student_interval_contains_without_absolute_float_conversion() {
    let mut runs = Vec::new();
    for exponent in 1_000..1_032 {
        let mut histogram = ExponentHistogram::new(3, 3_072).unwrap();
        histogram.record_many(exponent, 16).unwrap();
        runs.push(histogram);
    }
    let summary = WeightedRuns::from_histograms(runs).unwrap();
    let interval = summary
        .student_interval(1_019_756_723_u64, 500_000_000_u64)
        .unwrap();

    let (mean_numerator, mean_denominator) = summary.mean_decimal();
    assert!(interval
        .contains_exact(
            mean_numerator.parse::<num_bigint::BigUint>().unwrap(),
            mean_denominator.parse::<num_bigint::BigUint>().unwrap(),
        )
        .unwrap());
    assert!(interval.render_outward(18).unwrap().lower.contains('e'));

    let (variance_numerator, variance_denominator) =
        summary.independent_run_variance_decimal().unwrap();
    let reconstructed = ScaledStudentInterval::from_exact_independent_runs(
        mean_numerator.parse::<num_bigint::BigUint>().unwrap(),
        mean_denominator.parse::<num_bigint::BigUint>().unwrap(),
        variance_numerator.parse::<num_bigint::BigUint>().unwrap(),
        variance_denominator.parse::<num_bigint::BigUint>().unwrap(),
        1_019_756_723_u64,
        500_000_000_u64,
        32,
        3,
        1_000,
    )
    .unwrap();
    assert_eq!(
        reconstructed.render_outward(18).unwrap(),
        interval.render_outward(18).unwrap()
    );
}
