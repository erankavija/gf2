use gf2_stats::weighted::{ExactRatio, ExponentHistogram, WeightedRuns};

#[test]
fn exact_exponent_histograms_drive_weighted_run_statistics() {
    let mut first = ExponentHistogram::new(3, 12).unwrap();
    first.record_many(2, 3).unwrap();
    first.record(5).unwrap();
    let mut second = ExponentHistogram::new(3, 12).unwrap();
    second.record_many(3, 4).unwrap();

    let summary = WeightedRuns::from_histograms(vec![first, second]).unwrap();
    assert_eq!(summary.sample_count(), 8);
    assert!(summary.weight_sum() > &ExactRatio::zero());
    assert!(summary.final_weight_ess() <= ExactRatio::from_integer(8_u64));
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
    let interval = summary.student_interval(ExactRatio::new(1_019_756_723_u64, 500_000_000_u64).unwrap()).unwrap();

    assert!(interval.contains_exact(summary.mean()));
    assert!(interval.render_outward(18).unwrap().lower.contains('e'));
}
