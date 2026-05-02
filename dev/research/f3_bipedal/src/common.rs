//! Shared infrastructure for the F_3 prototype — mirrors the F_5/F_7
//! prototype shape so cross-prime numbers are directly comparable.

use std::time::{Duration, Instant};

pub const N_BENCH: usize = 65_536;
pub const REPEATS: usize = 5;

#[inline]
pub fn ref_add(a: u8, b: u8) -> u8 {
    debug_assert!(a < 3 && b < 3);
    (a + b) % 3
}

#[inline]
pub fn ref_sub(a: u8, b: u8) -> u8 {
    debug_assert!(a < 3 && b < 3);
    (a + 3 - b) % 3
}

#[inline]
pub fn ref_mul(a: u8, b: u8) -> u8 {
    debug_assert!(a < 3 && b < 3);
    (a * b) % 3
}

#[inline]
pub fn ref_div(a: u8, b: u8) -> u8 {
    debug_assert!(a < 3 && b > 0 && b < 3);
    // F_3*: 1^-1 = 1, 2^-1 = 2 (since 2*2 = 4 = 1 mod 3). Every nonzero
    // element is its own inverse.
    (a * INV_F3[b as usize]) % 3
}

/// Inverses in F_3: 1->1, 2->2. Index 0 is invalid (set to 0).
pub const INV_F3: [u8; 3] = [0, 1, 2];

/// Common operations on a packed F_3 vector encoding.
#[allow(dead_code)]
pub trait F3Encoding: Sized {
    const NAME: &'static str;
    fn pack(canonical: &[u8]) -> Self;
    fn unpack(&self) -> Vec<u8>;
    fn len(&self) -> usize;
    fn add_assign(&mut self, other: &Self);
    fn sub_assign(&mut self, other: &Self);
    fn mul_assign(&mut self, other: &Self);
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

    /// Generate a vector of canonical F_3 elements (0..=2) of length `n`.
    pub fn f3_vec(&mut self, n: usize) -> Vec<u8> {
        let mut v = Vec::with_capacity(n);
        while v.len() < n {
            // Use 2 bits at a time to draw uniform 0..=2 with rejection
            // (codepoint 3 is rejected).
            let mut x = self.next_u64();
            for _ in 0..30 {
                let r = (x & 0x3) as u8;
                x >>= 2;
                if r < 3 && v.len() < n {
                    v.push(r);
                }
            }
        }
        v
    }

    /// Same as `f3_vec` but every element is in 1..=2 (nonzero).
    pub fn f3_vec_nonzero(&mut self, n: usize) -> Vec<u8> {
        let v = self.f3_vec(n);
        v.into_iter().map(|x| if x == 0 { 1 } else { x }).collect()
    }
}

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

pub fn bench_op_ns_per_elem<F: FnMut()>(f: F, n_elems: usize, repeats: usize) -> f64 {
    let median = time_op(f, repeats);
    median.as_secs_f64() * 1e9 / n_elems as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ref_ops_match_known_table() {
        assert_eq!(ref_add(2, 1), 0);
        assert_eq!(ref_add(2, 2), 1);
        assert_eq!(ref_sub(0, 1), 2);
        assert_eq!(ref_mul(2, 2), 1);
        assert_eq!(ref_div(2, 2), 1); // 2 * 2 = 4 = 1 mod 3
    }

    #[test]
    fn ref_inverse_table_correct() {
        for x in 1..3 {
            assert_eq!(ref_mul(x, INV_F3[x as usize]), 1);
        }
    }

    #[test]
    fn f3_vec_only_canonical() {
        let mut rng = Lcg::new(7);
        let v = rng.f3_vec(2048);
        assert_eq!(v.len(), 2048);
        assert!(v.iter().all(|&x| x < 3));
    }

    #[test]
    fn f3_vec_nonzero_excludes_zero() {
        let mut rng = Lcg::new(11);
        let v = rng.f3_vec_nonzero(2048);
        assert!(v.iter().all(|&x| (1..3).contains(&x)));
    }
}
