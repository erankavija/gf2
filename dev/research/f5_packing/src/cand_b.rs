//! Candidate B — `(zero, log)` split exploiting cyclic `F_5* = ⟨2⟩`.
//!
//! # Encoding
//!
//! `F_5*` is cyclic of order 4 with generator `g = 2`:
//! `2^0 = 1, 2^1 = 2, 2^2 = 4, 2^3 = 3`.
//!
//! Each element is encoded as a 3-bit triple `(z, l_hi, l_lo)`:
//! - `v = 0`  → `(1, 0, 0)`  (canonical zero; `l` bits unused)
//! - `v = 1`  → `(0, 0, 0)`  (`l = 0`)
//! - `v = 2`  → `(0, 0, 1)`  (`l = 1`)
//! - `v = 4`  → `(0, 1, 0)`  (`l = 2`)
//! - `v = 3`  → `(0, 1, 1)`  (`l = 3`)
//!
//! # Layout
//!
//! Three bit-planes, each `Vec<u64>`. Plane `P` at word `w`, bit `s` carries
//! the `P` bit of element `64 * w + s`. Each `u64`-triple covers 64 elements.
//!
//! # Op cost (per `u64`-triple = 64 F_5 ops)
//!
//! - **Mul**: 5 bitwise ops (`z|z`, two XORs, one AND, one XOR with carry).
//! - **Div**: 5 bitwise ops (XOR, ANDNOT, XOR-with-borrow; `z` carries unchanged).
//! - **Add / Sub**: NOT bit-parallelisable in `(z, log)` form; falls back to
//!   a per-element scalar lookup. Each element costs an extract-lookup-pack
//!   sequence.
//!
//! In the decision doc this is the headline asymmetry: B is the fastest mul,
//! but its add is the slowest of the four candidates.

use crate::common::{ref_add, ref_sub, F5Encoding};

const ELEMS_PER_WORD: usize = 64;

/// Canonical → `(z, l_hi, l_lo)` encoder.
#[inline]
fn encode_elem(v: u8) -> (u64, u64, u64) {
    debug_assert!(v < 5);
    match v {
        0 => (1, 0, 0),
        1 => (0, 0, 0),
        2 => (0, 0, 1),
        3 => (0, 1, 1),
        4 => (0, 1, 0),
        _ => unreachable!(),
    }
}

/// `(z, l_hi, l_lo)` → canonical decoder. Non-canonical zeros (`z=1` with
/// nonzero `l`) decode to `0` per convention.
#[inline]
fn decode_elem(z: u64, l_hi: u64, l_lo: u64) -> u8 {
    debug_assert!(z <= 1 && l_hi <= 1 && l_lo <= 1);
    if z == 1 {
        return 0;
    }
    match (l_hi, l_lo) {
        (0, 0) => 1,
        (0, 1) => 2,
        (1, 1) => 3,
        (1, 0) => 4,
        _ => unreachable!(),
    }
}

#[derive(Clone, Debug)]
pub struct VecB {
    z: Vec<u64>,
    l_hi: Vec<u64>,
    l_lo: Vec<u64>,
    len: usize,
}

impl VecB {
    fn n_words(len: usize) -> usize {
        len.div_ceil(ELEMS_PER_WORD)
    }

    #[inline]
    fn get_elem(&self, i: usize) -> u8 {
        let w = i / ELEMS_PER_WORD;
        let s = i % ELEMS_PER_WORD;
        let z = (self.z[w] >> s) & 1;
        let lh = (self.l_hi[w] >> s) & 1;
        let ll = (self.l_lo[w] >> s) & 1;
        decode_elem(z, lh, ll)
    }

    #[inline]
    fn set_elem(&mut self, i: usize, v: u8) {
        let w = i / ELEMS_PER_WORD;
        let s = i % ELEMS_PER_WORD;
        let mask = 1u64 << s;
        let (z, lh, ll) = encode_elem(v);
        self.z[w] = (self.z[w] & !mask) | (z << s);
        self.l_hi[w] = (self.l_hi[w] & !mask) | (lh << s);
        self.l_lo[w] = (self.l_lo[w] & !mask) | (ll << s);
    }
}

impl F5Encoding for VecB {
    const NAME: &'static str = "B: (z, log) split, F_5* cyclic";

    fn pack(canonical: &[u8]) -> Self {
        let len = canonical.len();
        let n_words = Self::n_words(len);
        let mut z = vec![0u64; n_words];
        let mut l_hi = vec![0u64; n_words];
        let mut l_lo = vec![0u64; n_words];
        for (i, &v) in canonical.iter().enumerate() {
            let w = i / ELEMS_PER_WORD;
            let s = i % ELEMS_PER_WORD;
            let (zi, lh, ll) = encode_elem(v);
            z[w] |= zi << s;
            l_hi[w] |= lh << s;
            l_lo[w] |= ll << s;
        }
        VecB { z, l_hi, l_lo, len }
    }

    fn unpack(&self) -> Vec<u8> {
        (0..self.len).map(|i| self.get_elem(i)).collect()
    }

    fn len(&self) -> usize {
        self.len
    }

    fn add_assign(&mut self, other: &Self) {
        assert_eq!(self.len, other.len);
        // Per-element scalar fallback: B's add is not naturally bit-parallel.
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
        // c = a * b in F_5: z_c = z_a | z_b; log_c = (log_a + log_b) mod 4.
        // 2-bit add mod 4 = drop carry beyond bit 1.
        let n = self.z.len();
        for w in 0..n {
            let za = self.z[w];
            let zb = other.z[w];
            let lha = self.l_hi[w];
            let lla = self.l_lo[w];
            let lhb = other.l_hi[w];
            let llb = other.l_lo[w];

            let zc = za | zb;
            let llc = lla ^ llb;
            let carry = lla & llb;
            let lhc = lha ^ lhb ^ carry;

            self.z[w] = zc;
            self.l_hi[w] = lhc;
            self.l_lo[w] = llc;
        }
    }

    fn div_assign(&mut self, other: &Self) {
        assert_eq!(self.len, other.len);
        // c = a / b in F_5: requires z_b = 0. log_c = (log_a - log_b) mod 4.
        // 2-bit sub mod 4 with borrow.
        let n = self.z.len();
        for w in 0..n {
            let za = self.z[w];
            let lha = self.l_hi[w];
            let lla = self.l_lo[w];
            let lhb = other.l_hi[w];
            let llb = other.l_lo[w];

            let llc = lla ^ llb;
            let borrow = (!lla) & llb;
            let lhc = lha ^ lhb ^ borrow;
            // z_c = z_a (since b nonzero, c=0 ⇔ a=0).
            self.z[w] = za;
            self.l_hi[w] = lhc;
            self.l_lo[w] = llc;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{ref_div, ref_mul, Lcg};
    use proptest::prelude::*;

    fn vec_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(0u8..5, min..=max)
    }

    fn nonzero_vec_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(1u8..5, min..=max)
    }

    #[test]
    fn pack_unpack_roundtrip() {
        let mut rng = Lcg::new(13);
        for &n in &[0usize, 1, 63, 64, 65, 100, 1000] {
            let v = rng.f5_vec(n);
            let p = VecB::pack(&v);
            assert_eq!(p.unpack(), v, "n={n}");
        }
    }

    #[test]
    fn exhaustive_mul_pairs() {
        for a in 0u8..5 {
            for b in 0u8..5 {
                let mut va = VecB::pack(&[a]);
                let vb = VecB::pack(&[b]);
                va.mul_assign(&vb);
                assert_eq!(va.unpack()[0], ref_mul(a, b), "{a}*{b}");
            }
        }
    }

    #[test]
    fn exhaustive_div_pairs_nonzero_b() {
        for a in 0u8..5 {
            for b in 1u8..5 {
                let mut va = VecB::pack(&[a]);
                let vb = VecB::pack(&[b]);
                va.div_assign(&vb);
                assert_eq!(va.unpack()[0], ref_div(a, b), "{a}/{b}");
            }
        }
    }

    #[test]
    fn exhaustive_add_pairs() {
        for a in 0u8..5 {
            for b in 0u8..5 {
                let mut va = VecB::pack(&[a]);
                let vb = VecB::pack(&[b]);
                va.add_assign(&vb);
                assert_eq!(va.unpack()[0], ref_add(a, b), "{a}+{b}");
            }
        }
    }

    #[test]
    fn exhaustive_sub_pairs() {
        for a in 0u8..5 {
            for b in 0u8..5 {
                let mut va = VecB::pack(&[a]);
                let vb = VecB::pack(&[b]);
                va.sub_assign(&vb);
                assert_eq!(va.unpack()[0], ref_sub(a, b), "{a}-{b}");
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn prop_pack_unpack(v in vec_strategy(0, 256)) {
            prop_assert_eq!(VecB::pack(&v).unpack(), v);
        }

        #[test]
        fn prop_mul_matches_scalar(
            a in vec_strategy(1, 256),
            b in vec_strategy(1, 256),
        ) {
            let n = a.len().min(b.len());
            let a = &a[..n]; let b = &b[..n];
            let mut va = VecB::pack(a);
            va.mul_assign(&VecB::pack(b));
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
            let mut va = VecB::pack(a);
            va.div_assign(&VecB::pack(b));
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
            let mut va = VecB::pack(a);
            va.add_assign(&VecB::pack(b));
            let got = va.unpack();
            for i in 0..n { prop_assert_eq!(got[i], ref_add(a[i], b[i])); }
        }

        #[test]
        fn prop_sub_matches_scalar(
            a in vec_strategy(1, 256),
            b in vec_strategy(1, 256),
        ) {
            let n = a.len().min(b.len());
            let a = &a[..n]; let b = &b[..n];
            let mut va = VecB::pack(a);
            va.sub_assign(&VecB::pack(b));
            let got = va.unpack();
            for i in 0..n { prop_assert_eq!(got[i], ref_sub(a[i], b[i])); }
        }
    }
}
