//! Shared infrastructure for F_5 packed-encoding candidates.
//!
//! Defines the [`F5Encoding`] trait, a deterministic LCG for benchmarks,
//! and reference scalar `Fp<5>` arithmetic against which every candidate's
//! correctness is checked.

use std::time::{Duration, Instant};

pub const N_BENCH: usize = 65_536;
pub const REPEATS: usize = 5;

#[inline]
pub fn ref_add(a: u8, b: u8) -> u8 {
    debug_assert!(a < 5 && b < 5);
    (a + b) % 5
}

#[inline]
pub fn ref_sub(a: u8, b: u8) -> u8 {
    debug_assert!(a < 5 && b < 5);
    (a + 5 - b) % 5
}

#[inline]
pub fn ref_mul(a: u8, b: u8) -> u8 {
    debug_assert!(a < 5 && b < 5);
    (a * b) % 5
}

#[inline]
pub fn ref_div(a: u8, b: u8) -> u8 {
    debug_assert!(a < 5 && b > 0 && b < 5);
    (a * INV_F5[b as usize]) % 5
}

/// Inverses in F_5: 1->1, 2->3, 3->2, 4->4. Index 0 is invalid (set to 0).
pub const INV_F5: [u8; 5] = [0, 1, 3, 2, 4];

/// Common operations on a packed F_5 vector encoding.
///
/// All in-place ops take a parallel `other` of the same length; the
/// implementations panic if shapes mismatch.
#[allow(dead_code)] // `unpack` / `len` are exercised in tests and via crate downstream.
pub trait F5Encoding: Sized {
    /// Human-readable name for the encoding (used in the bench table).
    const NAME: &'static str;

    /// Pack canonical F_5 elements (each in `0..5`) into the encoding.
    fn pack(canonical: &[u8]) -> Self;

    /// Unpack the encoding back into canonical F_5 elements.
    fn unpack(&self) -> Vec<u8>;

    /// Number of F_5 elements stored.
    fn len(&self) -> usize;

    /// `self ← self + other`.
    fn add_assign(&mut self, other: &Self);

    /// `self ← self - other`.
    fn sub_assign(&mut self, other: &Self);

    /// `self ← self * other`.
    fn mul_assign(&mut self, other: &Self);

    /// `self ← self / other`. `other` must contain no zeros.
    fn div_assign(&mut self, other: &Self);
}

/// Deterministic 64-bit LCG (Knuth MMIX constants).
pub struct Lcg(pub u64);

impl Lcg {
    pub fn new(seed: u64) -> Self {
        Lcg(seed)
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }

    /// Generate a vector of canonical F_5 elements (0..=4) of length `n`.
    pub fn f5_vec(&mut self, n: usize) -> Vec<u8> {
        let mut v = Vec::with_capacity(n);
        while v.len() < n {
            // Use 6 bits at a time to draw uniform 0..=4 with rejection;
            // a bias-free draw is unnecessary for benchmarks but cheap here.
            let mut x = self.next_u64();
            for _ in 0..10 {
                let r = (x & 0x7) as u8;
                x >>= 3;
                if r < 5 && v.len() < n {
                    v.push(r);
                }
            }
        }
        v
    }

    /// Same as [`Self::f5_vec`] but every element is in `1..=4` (nonzero).
    pub fn f5_vec_nonzero(&mut self, n: usize) -> Vec<u8> {
        let v = self.f5_vec(n);
        v.into_iter().map(|x| if x == 0 { 1 } else { x }).collect()
    }
}

/// Time `repeats` invocations of `f` and return the median elapsed time.
pub fn time_op<F: FnMut()>(mut f: F, repeats: usize) -> Duration {
    let mut ts: Vec<Duration> = (0..repeats)
        .map(|_| {
            let t0 = Instant::now();
            f();
            t0.elapsed()
        })
        .collect();
    ts.sort();
    ts[repeats / 2]
}

/// Run a single (op-name, op) measurement and return median ns/element.
pub fn bench_op_ns_per_elem<F: FnMut()>(f: F, n_elems: usize, repeats: usize) -> f64 {
    let median = time_op(f, repeats);
    median.as_secs_f64() * 1e9 / n_elems as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lcg_is_deterministic() {
        let mut a = Lcg::new(42);
        let mut b = Lcg::new(42);
        for _ in 0..10 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn ref_ops_match_known_table() {
        // Spot-check against the F_5 tables.
        assert_eq!(ref_add(2, 3), 0);
        assert_eq!(ref_add(4, 4), 3);
        assert_eq!(ref_sub(0, 1), 4);
        assert_eq!(ref_sub(2, 4), 3);
        assert_eq!(ref_mul(2, 3), 1);
        assert_eq!(ref_mul(4, 4), 1);
        assert_eq!(ref_div(1, 2), 3);
        assert_eq!(ref_div(4, 3), 3); // 4 * 3^-1 = 4 * 2 = 8 = 3
    }

    #[test]
    fn ref_inverse_table_correct() {
        for x in 1..5 {
            assert_eq!(ref_mul(x, INV_F5[x as usize]), 1, "{x}");
        }
    }

    #[test]
    fn f5_vec_only_canonical() {
        let mut rng = Lcg::new(7);
        let v = rng.f5_vec(1024);
        assert_eq!(v.len(), 1024);
        assert!(v.iter().all(|&x| x < 5));
    }

    #[test]
    fn f5_vec_nonzero_excludes_zero() {
        let mut rng = Lcg::new(9);
        let v = rng.f5_vec_nonzero(1024);
        assert!(v.iter().all(|&x| (1..5).contains(&x)));
    }
}
