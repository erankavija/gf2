//! Crate-private numerical primitives shared by statistical procedures.

/// Evaluates the natural logarithm of Gamma for positive arguments.
pub(crate) fn log_gamma(value: f64) -> f64 {
    if value == 1.0 || value == 2.0 {
        return 0.0;
    }

    const COEFFICIENTS: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];

    let shifted = value - 1.0;
    let series = COEFFICIENTS
        .iter()
        .enumerate()
        .skip(1)
        .fold(COEFFICIENTS[0], |sum, (index, coefficient)| {
            sum + coefficient / (shifted + index as f64)
        });
    let base = shifted + 7.5;
    0.5 * (2.0 * core::f64::consts::PI).ln() + (shifted + 0.5) * base.ln() - base + series.ln()
}
