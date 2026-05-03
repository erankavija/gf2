//! F_17 Candidate B — `(zero, log)` split with Fermat-like mod-16 log.
//!
//! # Why this matters for the cross-prime story
//!
//! `F_17` is the next Fermat-like prime after `F_5` (`p − 1 = 16 = 2^4`),
//! so the (z, log) encoding's mul reduces to a clean **bit-parallel
//! 4-bit log addition with no conditional subtract** — log space addition
//! mod `2^k` is just truncated-carry add. This is the cleanest mul
//! bit-trick available for any prime field of size > 5.
//!
//! Add/sub fall back to per-element extract-add-repack (the same blocker
//! that sank F_5-B and F_7-B).
//!
//! # Encoding
//!
//! `F_17* = ⟨3⟩` of order 16. Each element encodes as `(z, l_3, l_2, l_1, l_0)`:
//! - `v = 0` → `(z=1, l=0)`
//! - `v ≠ 0` → `(z=0, l=log_3(v))` with `l ∈ {0, …, 15}` (4 bits).
//!
//! # Layout
//!
//! Five bit-planes, each `Vec<u64>`. One `u64`-quint covers 64 F_17 elements.
//!
//! # Op cost (per `u64`-quint = 64 F_17 ops)
//!
//! - **Mul**: zero-flag OR (1 op) + 4-bit ripple-add with carry-out
//!   discarded (= 14 ops for the add, mod-16 falls out for free) = **15 ops**.
//! - **Div**: zero-flag passthrough + 4-bit ripple-subtract = **~14 ops**.
//! - **Add / Sub**: per-element fallback.

use crate::common::{ref_add, ref_sub, F17Encoding, EXP3, LOG3};

const ELEMS_PER_WORD: usize = 64;

#[inline]
fn encode_elem(v: u8) -> (u64, u64, u64, u64, u64) {
    debug_assert!(v < 17);
    if v == 0 {
        return (1, 0, 0, 0, 0);
    }
    let l = LOG3[v as usize];
    (
        0,
        ((l >> 3) & 1) as u64,
        ((l >> 2) & 1) as u64,
        ((l >> 1) & 1) as u64,
        (l & 1) as u64,
    )
}

#[inline]
fn decode_elem(z: u64, l3: u64, l2: u64, l1: u64, l0: u64) -> u8 {
    if z == 1 {
        return 0;
    }
    let l = ((l3 << 3) | (l2 << 2) | (l1 << 1) | l0) as usize;
    EXP3[l]
}

#[derive(Clone, Debug)]
pub struct ZLog17 {
    z: Vec<u64>,
    l3: Vec<u64>,
    l2: Vec<u64>,
    l1: Vec<u64>,
    l0: Vec<u64>,
    len: usize,
}

impl ZLog17 {
    fn n_words(len: usize) -> usize {
        len.div_ceil(ELEMS_PER_WORD)
    }

    #[inline]
    fn get_elem(&self, i: usize) -> u8 {
        let w = i / ELEMS_PER_WORD;
        let s = i % ELEMS_PER_WORD;
        decode_elem(
            (self.z[w] >> s) & 1,
            (self.l3[w] >> s) & 1,
            (self.l2[w] >> s) & 1,
            (self.l1[w] >> s) & 1,
            (self.l0[w] >> s) & 1,
        )
    }

    #[inline]
    fn set_elem(&mut self, i: usize, v: u8) {
        let w = i / ELEMS_PER_WORD;
        let s = i % ELEMS_PER_WORD;
        let mask = 1u64 << s;
        let (z, l3, l2, l1, l0) = encode_elem(v);
        self.z[w] = (self.z[w] & !mask) | (z << s);
        self.l3[w] = (self.l3[w] & !mask) | (l3 << s);
        self.l2[w] = (self.l2[w] & !mask) | (l2 << s);
        self.l1[w] = (self.l1[w] & !mask) | (l1 << s);
        self.l0[w] = (self.l0[w] & !mask) | (l0 << s);
    }
}

/// Bit-sliced 4-bit log add mod 16 — truncated-carry ripple add.
///
/// Inputs: `(la3, la2, la1, la0)`, `(lb3, lb2, lb1, lb0)` ∈ {0, …, 15}.
/// Output: `(c3, c2, c1, c0) = (la + lb) mod 16 ∈ {0, …, 15}`.
///
/// 14 bitwise ops per `u64`-quad over 64 elements (the carry-out is
/// discarded, which **is** the mod-16 reduction — clean Fermat-like trick).
#[inline]
#[allow(clippy::too_many_arguments)]
fn log_add_mod16(
    la3: u64,
    la2: u64,
    la1: u64,
    la0: u64,
    lb3: u64,
    lb2: u64,
    lb1: u64,
    lb0: u64,
) -> (u64, u64, u64, u64) {
    let c0 = la0 ^ lb0;
    let cy1 = la0 & lb0;
    let xor1 = la1 ^ lb1;
    let c1 = xor1 ^ cy1;
    let cy2 = (la1 & lb1) | (cy1 & xor1);
    let xor2 = la2 ^ lb2;
    let c2 = xor2 ^ cy2;
    let cy3 = (la2 & lb2) | (cy2 & xor2);
    let c3 = la3 ^ lb3 ^ cy3;
    // 12 ops, carry-out (cy4) discarded → mod 16.
    (c3, c2, c1, c0)
}

/// Bit-sliced 4-bit log subtract mod 16 — truncated-borrow ripple sub.
///
/// Output: `(la − lb) mod 16 ∈ {0, …, 15}`. The borrow-out is discarded;
/// because `2^4 = 16`, that is exactly the mod-16 reduction for the
/// negative case (e.g., `1 − 2 = −1 ≡ 15 (mod 16)` falls out of the
/// two's-complement-style ripple-sub for free).
#[inline]
#[allow(clippy::too_many_arguments)]
fn log_sub_mod16(
    la3: u64,
    la2: u64,
    la1: u64,
    la0: u64,
    lb3: u64,
    lb2: u64,
    lb1: u64,
    lb0: u64,
) -> (u64, u64, u64, u64) {
    let d0 = la0 ^ lb0;
    let bw1 = (!la0) & lb0;
    let xor1 = la1 ^ lb1;
    let d1 = xor1 ^ bw1;
    let bw2 = ((!la1) & lb1) | ((!xor1) & bw1);
    let xor2 = la2 ^ lb2;
    let d2 = xor2 ^ bw2;
    let bw3 = ((!la2) & lb2) | ((!xor2) & bw2);
    let d3 = la3 ^ lb3 ^ bw3;
    (d3, d2, d1, d0)
}

impl F17Encoding for ZLog17 {
    const NAME: &'static str = "B: (z, log) split, F_17* cyclic mod-16";

    fn pack(canonical: &[u8]) -> Self {
        let len = canonical.len();
        let n_words = Self::n_words(len);
        let mut z = vec![0u64; n_words];
        let mut l3 = vec![0u64; n_words];
        let mut l2 = vec![0u64; n_words];
        let mut l1 = vec![0u64; n_words];
        let mut l0 = vec![0u64; n_words];
        for (i, &v) in canonical.iter().enumerate() {
            let w = i / ELEMS_PER_WORD;
            let s = i % ELEMS_PER_WORD;
            let (zi, l3i, l2i, l1i, l0i) = encode_elem(v);
            z[w] |= zi << s;
            l3[w] |= l3i << s;
            l2[w] |= l2i << s;
            l1[w] |= l1i << s;
            l0[w] |= l0i << s;
        }
        ZLog17 {
            z,
            l3,
            l2,
            l1,
            l0,
            len,
        }
    }

    fn unpack(&self) -> Vec<u8> {
        (0..self.len).map(|i| self.get_elem(i)).collect()
    }

    fn len(&self) -> usize {
        self.len
    }

    fn add_assign(&mut self, other: &Self) {
        assert_eq!(self.len, other.len);
        for i in 0..self.len {
            let a = self.get_elem(i);
            let b = other.get_elem(i);
            self.set_elem(i, ref_add(a, b));
        }
    }

    fn sub_assign(&mut self, other: &Self) {
        assert_eq!(self.len, other.len);
        for i in 0..self.len {
            let a = self.get_elem(i);
            let b = other.get_elem(i);
            self.set_elem(i, ref_sub(a, b));
        }
    }

    fn mul_assign(&mut self, other: &Self) {
        assert_eq!(self.len, other.len);
        let n = self.z.len();
        for w in 0..n {
            let zc = self.z[w] | other.z[w];
            let (c3, c2, c1, c0) = log_add_mod16(
                self.l3[w],
                self.l2[w],
                self.l1[w],
                self.l0[w],
                other.l3[w],
                other.l2[w],
                other.l1[w],
                other.l0[w],
            );
            self.z[w] = zc;
            self.l3[w] = c3;
            self.l2[w] = c2;
            self.l1[w] = c1;
            self.l0[w] = c0;
        }
    }

    fn div_assign(&mut self, other: &Self) {
        assert_eq!(self.len, other.len);
        let n = self.z.len();
        for w in 0..n {
            let (c3, c2, c1, c0) = log_sub_mod16(
                self.l3[w],
                self.l2[w],
                self.l1[w],
                self.l0[w],
                other.l3[w],
                other.l2[w],
                other.l1[w],
                other.l0[w],
            );
            self.l3[w] = c3;
            self.l2[w] = c2;
            self.l1[w] = c1;
            self.l0[w] = c0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{ref_div, ref_mul, Lcg};
    use proptest::prelude::*;

    fn vec_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(0u8..17, min..=max)
    }

    fn nonzero_vec_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(1u8..17, min..=max)
    }

    #[test]
    fn pack_unpack_roundtrip() {
        let mut rng = Lcg::new(13);
        for &n in &[0usize, 1, 63, 64, 65, 100, 1000] {
            let v = rng.f17_vec(n);
            let p = ZLog17::pack(&v);
            assert_eq!(p.unpack(), v, "n={n}");
        }
    }

    /// Bit-parallel mod-16 log addition is the structural payoff of B.
    /// Cover all 16×16 = 256 (la, lb) pairs against scalar `(la + lb) % 16`.
    #[test]
    fn log_add_mod16_truth_table() {
        for la in 0u8..16 {
            for lb in 0u8..16 {
                let la_u = la as u64;
                let lb_u = lb as u64;
                let (c3, c2, c1, c0) = log_add_mod16(
                    (la_u >> 3) & 1,
                    (la_u >> 2) & 1,
                    (la_u >> 1) & 1,
                    la_u & 1,
                    (lb_u >> 3) & 1,
                    (lb_u >> 2) & 1,
                    (lb_u >> 1) & 1,
                    lb_u & 1,
                );
                let c = (c3 << 3) | (c2 << 2) | (c1 << 1) | c0;
                assert_eq!(c, ((la + lb) & 0xf) as u64, "la={la}, lb={lb}");
            }
        }
    }

    #[test]
    fn log_sub_mod16_truth_table() {
        for la in 0u8..16 {
            for lb in 0u8..16 {
                let la_u = la as u64;
                let lb_u = lb as u64;
                let (d3, d2, d1, d0) = log_sub_mod16(
                    (la_u >> 3) & 1,
                    (la_u >> 2) & 1,
                    (la_u >> 1) & 1,
                    la_u & 1,
                    (lb_u >> 3) & 1,
                    (lb_u >> 2) & 1,
                    (lb_u >> 1) & 1,
                    lb_u & 1,
                );
                let d = (d3 << 3) | (d2 << 2) | (d1 << 1) | d0;
                let want = (((la as i32) - (lb as i32)).rem_euclid(16)) as u64;
                assert_eq!(d, want, "la={la}, lb={lb}");
            }
        }
    }

    #[test]
    fn exhaustive_mul_pairs() {
        for a in 0u8..17 {
            for b in 0u8..17 {
                let mut va = ZLog17::pack(&[a]);
                let vb = ZLog17::pack(&[b]);
                va.mul_assign(&vb);
                assert_eq!(va.unpack()[0], ref_mul(a, b), "{a}*{b}");
            }
        }
    }

    #[test]
    fn exhaustive_div_pairs_nonzero_b() {
        for a in 0u8..17 {
            for b in 1u8..17 {
                let mut va = ZLog17::pack(&[a]);
                let vb = ZLog17::pack(&[b]);
                va.div_assign(&vb);
                assert_eq!(va.unpack()[0], ref_div(a, b), "{a}/{b}");
            }
        }
    }

    #[test]
    fn exhaustive_add_pairs() {
        for a in 0u8..17 {
            for b in 0u8..17 {
                let mut va = ZLog17::pack(&[a]);
                let vb = ZLog17::pack(&[b]);
                va.add_assign(&vb);
                assert_eq!(va.unpack()[0], ref_add(a, b), "{a}+{b}");
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn prop_pack_unpack(v in vec_strategy(0, 256)) {
            prop_assert_eq!(ZLog17::pack(&v).unpack(), v);
        }

        #[test]
        fn prop_mul_matches_scalar(
            a in vec_strategy(1, 256),
            b in vec_strategy(1, 256),
        ) {
            let n = a.len().min(b.len());
            let a = &a[..n]; let b = &b[..n];
            let mut va = ZLog17::pack(a);
            va.mul_assign(&ZLog17::pack(b));
            let got = va.unpack();
            for i in 0..n { prop_assert_eq!(got[i], ref_mul(a[i], b[i])); }
        }

        #[test]
        fn prop_div_matches_scalar(
            a in vec_strategy(1, 256),
            b in nonzero_vec_strategy(1, 256),
        ) {
            let n = a.len().min(b.len());
            let a = &a[..n]; let b = &b[..n];
            let mut va = ZLog17::pack(a);
            va.div_assign(&ZLog17::pack(b));
            let got = va.unpack();
            for i in 0..n { prop_assert_eq!(got[i], ref_div(a[i], b[i])); }
        }

        #[test]
        fn prop_add_matches_scalar(
            a in vec_strategy(1, 256),
            b in vec_strategy(1, 256),
        ) {
            let n = a.len().min(b.len());
            let a = &a[..n]; let b = &b[..n];
            let mut va = ZLog17::pack(a);
            va.add_assign(&ZLog17::pack(b));
            let got = va.unpack();
            for i in 0..n { prop_assert_eq!(got[i], ref_add(a[i], b[i])); }
        }
    }
}
