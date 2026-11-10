//! The operands, the gf2 arm and the timing loop of the carry-less product
//! comparison (@/issue/befccab3), shared by the comparison binary in
//! `external/` and this crate's tests.

use std::hint::black_box;
use std::time::Instant;

/// Operand lengths in 64-bit words the comparison times.
pub const SIZES: [usize; 4] = [16, 64, 256, 2048];

/// Timed samples per arm and size; the reported rate is their median.
pub const SAMPLES: usize = 11;

/// Minimum duration of one timed sample, in seconds.
pub const SAMPLE_SECONDS: f64 = 0.05;

/// The two `words`-word operands of a size, from a SplitMix64 stream
/// (`@/citation/Steele2014`) seeded by `seed`.
#[must_use]
pub fn operands(words: usize, seed: u64) -> (Vec<u64>, Vec<u64>) {
    let mut state = seed;
    let mut next = || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    let a = (0..words).map(|_| next()).collect();
    let b = (0..words).map(|_| next()).collect();
    (a, b)
}

/// gf2's product through the production long product,
/// [`gf2_core::gf2m::wide::clmul_slice_overwrite`].
pub fn gf2_product(a: &[u64], b: &[u64], out: &mut [u64]) {
    gf2_core::gf2m::wide::clmul_slice_overwrite(a, b, out);
}

/// Median products per second of `product` over [`SAMPLES`] samples of at
/// least [`SAMPLE_SECONDS`] each, after one untimed sample.
pub fn products_per_second(mut product: impl FnMut()) -> f64 {
    let start = Instant::now();
    let mut calls = 0u64;
    while start.elapsed().as_secs_f64() < SAMPLE_SECONDS {
        product();
        calls += 1;
    }
    let mut rates: Vec<f64> = (0..SAMPLES)
        .map(|_| {
            let start = Instant::now();
            for _ in 0..calls {
                product();
                black_box(());
            }
            calls as f64 / start.elapsed().as_secs_f64()
        })
        .collect();
    rates.sort_by(f64::total_cmp);
    rates[SAMPLES / 2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gf2_arm_writes_the_schoolbook_product_over_a_dirty_destination() {
        for words in [1, 16, 17] {
            let (a, b) = operands(words, 7);
            let mut want = vec![0u64; 2 * words];
            for (i, &x) in a.iter().enumerate() {
                for (j, &y) in b.iter().enumerate() {
                    let product = gf2_core::gf2m::barrett::clmul(x, y);
                    want[i + j] ^= product as u64;
                    want[i + j + 1] ^= (product >> 64) as u64;
                }
            }
            let mut got = vec![u64::MAX; 2 * words];
            gf2_product(&a, &b, &mut got);
            assert_eq!(got, want, "{words} words");
        }
    }

    #[test]
    fn operands_are_distinct_per_seed_and_reproducible() {
        assert_eq!(operands(16, 3), operands(16, 3));
        assert_ne!(operands(16, 3), operands(16, 4));
    }
}
