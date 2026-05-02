//! Shared infrastructure for F_7 packed-encoding candidates.
//!
//! Defines the [`F7Encoding`] trait, a deterministic LCG for benchmarks,
//! and reference scalar `Fp<7>` arithmetic against which every candidate's
//! correctness is checked.

use std::time::{Duration, Instant};

pub const N_BENCH: usize = 65_536;
pub const REPEATS: usize = 5;

#[inline]
pub fn ref_add(a: u8, b: u8) -> u8 {
    debug_assert!(a < 7 && b < 7);
    (a + b) % 7
}

#[inline]
pub fn ref_sub(a: u8, b: u8) -> u8 {
    debug_assert!(a < 7 && b < 7);
    (a + 7 - b) % 7
}

#[inline]
pub fn ref_mul(a: u8, b: u8) -> u8 {
    debug_assert!(a < 7 && b < 7);
    (a * b) % 7
}

#[inline]
pub fn ref_div(a: u8, b: u8) -> u8 {
    debug_assert!(a < 7 && b > 0 && b < 7);
    (a * INV_F7[b as usize]) % 7
}

/// Inverses in F_7: 1->1, 2->4, 3->5, 4->2, 5->3, 6->6. Index 0 is invalid (set to 0).
pub const INV_F7: [u8; 7] = [0, 1, 4, 5, 2, 3, 6];

/// Common operations on a packed F_7 vector encoding.
///
/// All in-place ops take a parallel `other` of the same length; the
/// implementations panic if shapes mismatch.
#[allow(dead_code)] // `unpack` / `len` are exercised in tests and via crate downstream.
pub trait F7Encoding: Sized {
    /// Human-readable name for the encoding (used in the bench table).
    const NAME: &'static str;

    /// Pack canonical F_7 elements (each in `0..7`) into the encoding.
    fn pack(canonical: &[u8]) -> Self;

    /// Unpack the encoding back into canonical F_7 elements.
    fn unpack(&self) -> Vec<u8>;

    /// Number of F_7 elements stored.
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

    /// Generate a vector of canonical F_7 elements (0..=6) of length `n`.
    pub fn f7_vec(&mut self, n: usize) -> Vec<u8> {
        let mut v = Vec::with_capacity(n);
        while v.len() < n {
            // Use 3 bits at a time to draw uniform 0..=6 with rejection
            // (3-bit codepoint 7 is rejected). Bias-free is unnecessary
            // for benchmarks but cheap here.
            let mut x = self.next_u64();
            for _ in 0..20 {
                let r = (x & 0x7) as u8;
                x >>= 3;
                if r < 7 && v.len() < n {
                    v.push(r);
                }
            }
        }
        v
    }

    /// Same as [`Self::f7_vec`] but every element is in `1..=6` (nonzero).
    pub fn f7_vec_nonzero(&mut self, n: usize) -> Vec<u8> {
        let v = self.f7_vec(n);
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
        // Spot-check against the F_7 tables.
        assert_eq!(ref_add(3, 4), 0);
        assert_eq!(ref_add(6, 6), 5);
        assert_eq!(ref_sub(0, 1), 6);
        assert_eq!(ref_sub(2, 5), 4);
        assert_eq!(ref_mul(3, 4), 5); // 12 mod 7 = 5
        assert_eq!(ref_mul(6, 6), 1); // 36 mod 7 = 1
        assert_eq!(ref_div(1, 2), 4); // 1 * inv(2) = 1 * 4 = 4
        assert_eq!(ref_div(6, 3), 2); // 6 * inv(3) = 6 * 5 = 30 mod 7 = 2
    }

    #[test]
    fn ref_inverse_table_correct() {
        for x in 1..7 {
            assert_eq!(ref_mul(x, INV_F7[x as usize]), 1, "{x}");
        }
    }

    #[test]
    fn f7_vec_only_canonical() {
        let mut rng = Lcg::new(7);
        let v = rng.f7_vec(1024);
        assert_eq!(v.len(), 1024);
        assert!(v.iter().all(|&x| x < 7));
    }

    #[test]
    fn f7_vec_nonzero_excludes_zero() {
        let mut rng = Lcg::new(9);
        let v = rng.f7_vec_nonzero(1024);
        assert!(v.iter().all(|&x| (1..7).contains(&x)));
    }
}
