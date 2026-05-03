//! Shared infrastructure for F_17 prototype.
//!
//! F_17 was selected as the test case for the **Fermat-like** prediction
//! from the cross-prime generalization analysis: `p − 1 = 16 = 2^4` makes
//! `F_17*` cyclic of order 2^4, so the (zero, log) encoding's mul reduces
//! to bit-parallel 4-bit log addition with no conditional subtract.

use std::time::{Duration, Instant};

pub const N_BENCH: usize = 65_536;
pub const REPEATS: usize = 5;

#[inline]
pub fn ref_add(a: u8, b: u8) -> u8 {
    debug_assert!(a < 17 && b < 17);
    (a + b) % 17
}

#[inline]
pub fn ref_sub(a: u8, b: u8) -> u8 {
    debug_assert!(a < 17 && b < 17);
    (a + 17 - b) % 17
}

#[inline]
pub fn ref_mul(a: u8, b: u8) -> u8 {
    debug_assert!(a < 17 && b < 17);
    ((a as u16 * b as u16) % 17) as u8
}

#[inline]
pub fn ref_div(a: u8, b: u8) -> u8 {
    debug_assert!(a < 17 && b > 0 && b < 17);
    ((a as u16 * INV_F17[b as usize] as u16) % 17) as u8
}

/// Inverses in F_17 derived from the LOG3/EXP3 tables. Index 0 is invalid (set to 0).
/// `inv(b) = EXP3[(16 − LOG3[b]) mod 16]`.
pub const INV_F17: [u8; 17] = [
    0, // 0 — invalid
    1, 9, 6, 13, 7, 3, 5, 15, 2, 12, 14, 10, 4, 11, 8, 16,
];

/// `log_3(v)` for `v ∈ {1, …, 16}`. Index 0 is unused. 3 is a primitive root mod 17.
pub const LOG3: [u8; 17] = [
    0, // 0 — invalid
    0, 14, 1, 12, 5, 15, 11, 10, 2, 3, 7, 13, 4, 9, 6, 8,
];

/// `3^k mod 17` for `k ∈ {0, …, 15}`.
pub const EXP3: [u8; 16] = [1, 3, 9, 10, 13, 5, 15, 11, 16, 14, 8, 7, 4, 12, 2, 6];

#[allow(dead_code)]
pub trait F17Encoding: Sized {
    const NAME: &'static str;
    fn pack(canonical: &[u8]) -> Self;
    fn unpack(&self) -> Vec<u8>;
    fn len(&self) -> usize;
    fn add_assign(&mut self, other: &Self);
    fn sub_assign(&mut self, other: &Self);
    fn mul_assign(&mut self, other: &Self);
    fn div_assign(&mut self, other: &Self);
}

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

    /// Generate a vector of canonical F_17 elements (0..=16) of length `n`.
    pub fn f17_vec(&mut self, n: usize) -> Vec<u8> {
        let mut v = Vec::with_capacity(n);
        while v.len() < n {
            // 5 bits at a time, reject codepoints in {17..=31}.
            let mut x = self.next_u64();
            for _ in 0..12 {
                let r = (x & 0x1f) as u8;
                x >>= 5;
                if r < 17 && v.len() < n {
                    v.push(r);
                }
            }
        }
        v
    }

    pub fn f17_vec_nonzero(&mut self, n: usize) -> Vec<u8> {
        let v = self.f17_vec(n);
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
        assert_eq!(ref_add(8, 9), 0); // 17 mod 17 = 0
        assert_eq!(ref_add(16, 16), 15); // 32 mod 17 = 15
        assert_eq!(ref_sub(0, 1), 16);
        assert_eq!(ref_mul(3, 6), 1); // 18 mod 17 = 1
        assert_eq!(ref_mul(16, 16), 1); // (-1)^2 = 1
        assert_eq!(ref_div(1, 3), 6); // inv(3) = 6
    }

    #[test]
    fn log_exp_tables_are_inverses() {
        for v in 1u8..17 {
            let l = LOG3[v as usize];
            assert_eq!(EXP3[l as usize], v, "v={v}");
        }
        for k in 0u8..16 {
            let v = EXP3[k as usize];
            assert_eq!(LOG3[v as usize], k, "k={k}");
        }
    }

    #[test]
    fn inverse_table_correct() {
        for x in 1..17 {
            assert_eq!(ref_mul(x, INV_F17[x as usize]), 1, "{x}");
        }
    }

    #[test]
    fn primitive_root_3_has_order_16() {
        let mut p = 1u8;
        for _ in 0..16 {
            p = ref_mul(p, 3);
        }
        assert_eq!(p, 1, "3^16 should be 1");
    }

    #[test]
    fn f17_vec_only_canonical() {
        let mut rng = Lcg::new(7);
        let v = rng.f17_vec(2048);
        assert_eq!(v.len(), 2048);
        assert!(v.iter().all(|&x| x < 17));
    }

    #[test]
    fn f17_vec_nonzero_excludes_zero() {
        let mut rng = Lcg::new(9);
        let v = rng.f17_vec_nonzero(2048);
        assert!(v.iter().all(|&x| (1..17).contains(&x)));
    }
}
