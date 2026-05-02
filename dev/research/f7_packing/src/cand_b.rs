//! Candidate B — `(zero, log)` split exploiting cyclic `F_7* = ⟨3⟩`.
//!
//! # Encoding
//!
//! `F_7*` is cyclic of order 6 with generator `g = 3`:
//! `3^0=1, 3^1=3, 3^2=2, 3^3=6, 3^4=4, 3^5=5` (all mod 7).
//!
//! Each element is encoded as a 4-bit nibble `(z, l_2, l_1, l_0)`:
//! - `v = 0`  → `(1, 0, 0, 0)`  (canonical zero; `l` bits unused)
//! - `v = 1`  → `(0, 0, 0, 0)`  (`l = 0`)
//! - `v = 3`  → `(0, 0, 0, 1)`  (`l = 1`)
//! - `v = 2`  → `(0, 0, 1, 0)`  (`l = 2`)
//! - `v = 6`  → `(0, 0, 1, 1)`  (`l = 3`)
//! - `v = 4`  → `(0, 1, 0, 0)`  (`l = 4`)
//! - `v = 5`  → `(0, 1, 0, 1)`  (`l = 5`)
//!
//! # Layout
//!
//! Four bit-planes, each `Vec<u64>`. Plane `P` at word `w`, bit `s` carries
//! the `P` bit of element `64 * w + s`. Each `u64`-quad covers 64 elements.
//!
//! # Op cost (per `u64`-quad = 64 F_7 ops)
//!
//! - **Mul**: log addition mod 6 in bit-sliced form. Mod 6 is **not** a
//!   power-of-2, so this is a 3-bit ripple-add followed by a conditional
//!   subtract-6. Estimated ~21 bitwise ops per `u64`-quad (z OR + 3-bit add
//!   + ge6 + conditional subtract).
//! - **Div**: log subtraction mod 6 (similar structure, ~21 ops).
//! - **Add / Sub**: NOT bit-parallelisable in `(z, log)` form; falls back to
//!   a per-element scalar lookup. Each element costs an extract-lookup-pack
//!   sequence.
//!
//! In the decision doc this is the headline asymmetry: B has a fast mul,
//! but its add is the slowest of the four candidates (matches F_5-B shape).

use crate::common::{ref_add, ref_sub, F7Encoding};

const ELEMS_PER_WORD: usize = 64;

/// log_3 for nonzero F_7 elements: 1→0, 3→1, 2→2, 6→3, 4→4, 5→5.
const LOG3: [u8; 7] = [0, 0, 2, 1, 4, 5, 3];
/// 3^k mod 7 for k=0..=5: 1, 3, 2, 6, 4, 5.
const EXP3: [u8; 6] = [1, 3, 2, 6, 4, 5];

/// Canonical → `(z, l_2, l_1, l_0)` encoder.
#[inline]
fn encode_elem(v: u8) -> (u64, u64, u64, u64) {
    debug_assert!(v < 7);
    if v == 0 {
        return (1, 0, 0, 0);
    }
    let l = LOG3[v as usize];
    (
        0,
        ((l >> 2) & 1) as u64,
        ((l >> 1) & 1) as u64,
        (l & 1) as u64,
    )
}

/// `(z, l_2, l_1, l_0)` → canonical decoder. Non-canonical zeros (`z=1`
/// with nonzero `l`) decode to `0` per convention; out-of-range log
/// codepoints (`l ∈ {6, 7}`) likewise decode to `0`.
#[inline]
fn decode_elem(z: u64, l2: u64, l1: u64, l0: u64) -> u8 {
    debug_assert!(z <= 1 && l2 <= 1 && l1 <= 1 && l0 <= 1);
    if z == 1 {
        return 0;
    }
    let l = ((l2 << 2) | (l1 << 1) | l0) as usize;
    if l >= 6 {
        return 0;
    }
    EXP3[l]
}

#[derive(Clone, Debug)]
pub struct VecB {
    z: Vec<u64>,
    l2: Vec<u64>,
    l1: Vec<u64>,
    l0: Vec<u64>,
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
        let l2 = (self.l2[w] >> s) & 1;
        let l1 = (self.l1[w] >> s) & 1;
        let l0 = (self.l0[w] >> s) & 1;
        decode_elem(z, l2, l1, l0)
    }

    #[inline]
    fn set_elem(&mut self, i: usize, v: u8) {
        let w = i / ELEMS_PER_WORD;
        let s = i % ELEMS_PER_WORD;
        let mask = 1u64 << s;
        let (z, l2, l1, l0) = encode_elem(v);
        self.z[w] = (self.z[w] & !mask) | (z << s);
        self.l2[w] = (self.l2[w] & !mask) | (l2 << s);
        self.l1[w] = (self.l1[w] & !mask) | (l1 << s);
        self.l0[w] = (self.l0[w] & !mask) | (l0 << s);
    }
}

/// Bit-sliced log addition mod 6.
///
/// Inputs: `(la2, la1, la0)` and `(lb2, lb1, lb0)`, each carrying
/// `log ∈ {0,…,5}` (bit 2 high). Output: `(c2, c1, c0)` carrying
/// `(la + lb) mod 6 ∈ {0,…,5}`.
///
/// Algorithm: 3-bit ripple-add → 4-bit sum `(c3, s2, s1, s0)`,
/// `sum ∈ {0,…,10}`. Reduce mod 6 by checking `sum ≥ 6` and
/// conditionally subtracting 6.
///
/// Per `u64`-quad (64 elements): 19 bitwise ops.
#[inline]
fn log_add_mod6(la2: u64, la1: u64, la0: u64, lb2: u64, lb1: u64, lb0: u64) -> (u64, u64, u64) {
    // 3-bit ripple add (no carry-in): produces (c3, s2, s1, s0).
    let s0 = la0 ^ lb0;
    let c1 = la0 & lb0;
    let la_xor_lb1 = la1 ^ lb1;
    let s1 = la_xor_lb1 ^ c1;
    let c2 = (la1 & lb1) | (c1 & la_xor_lb1);
    let la_xor_lb2 = la2 ^ lb2;
    let s2 = la_xor_lb2 ^ c2;
    let c3 = (la2 & lb2) | (c2 & la_xor_lb2);
    // 12 ops so far.

    // sum ≥ 6 iff: c3=1 (sum∈{8..=10}) OR (c3=0 AND s2=1 AND s1=1 covering sum∈{6,7}).
    let ge6 = c3 | (s2 & s1);
    // 2 ops; cumulative 14.

    // Mod-6 reduction: when ge6, the input is in {6..=10} and we subtract 6.
    // Truth-table mapping (input → output) for sum ∈ {6..=10}:
    //   6=110→0=000, 7=111→1=001, 8=1000→2=010, 9=1001→3=011, 10=1010→4=100
    // Output bits in terms of (c3, s2, s1, s0):
    //   o0 = s0                           (always)
    //   o1 = !s1                          (when ge6)
    //   o2 = c3 & s1                      (when ge6)

    // Branchless mux: `res = ge6 ? sub6(sum_low3) : sum_low3`.
    //   res_lo = s0
    //   res_mid = ge6 ? !s1 : s1 = s1 ^ ge6
    //   res_hi = ge6 ? (c3 & s1) : s2
    let res_lo = s0;
    let res_mid = s1 ^ ge6;
    let not_ge6 = !ge6;
    let res_hi = (s2 & not_ge6) | (c3 & s1);
    // 5 ops; cumulative 19.

    (res_hi, res_mid, res_lo)
}

/// Bit-sliced log subtraction mod 6.
///
/// Algorithm: 3-bit ripple-sub → 4-bit signed result `(borrow_out, d2, d1, d0)`.
/// When `borrow_out = 1` the raw difference is in `{−6..=−1}`; add 6 to
/// canonicalise into `{0..=5}`. Per `u64`-quad: 20 ops.
#[inline]
fn log_sub_mod6(la2: u64, la1: u64, la0: u64, lb2: u64, lb1: u64, lb0: u64) -> (u64, u64, u64) {
    // 3-bit ripple subtract (no borrow-in): a − b → (borrow_out, d2, d1, d0).
    let d0 = la0 ^ lb0;
    let bw1 = (!la0) & lb0;
    let la_xor_lb1 = la1 ^ lb1;
    let d1 = la_xor_lb1 ^ bw1;
    // borrow into bit 2: ((!a1 & b1) | ((!(a1 ^ b1)) & bw1))
    let bw2 = ((!la1) & lb1) | ((!la_xor_lb1) & bw1);
    let la_xor_lb2 = la2 ^ lb2;
    let d2 = la_xor_lb2 ^ bw2;
    let bw3 = ((!la2) & lb2) | ((!la_xor_lb2) & bw2);
    // 13 ops so far.

    // When bw3 = 0: result = (d2, d1, d0) ∈ {0..=5} (no further fold needed
    // because la, lb ∈ {0..=5}, so a − b ∈ {−5..=5}; positive case is always
    // in range).
    // When bw3 = 1: raw d ∈ {−6..=−1}; canonical = d + 6.
    //   Mapping (d2, d1, d0)_signed-3-bit → output ∈ {0..=5}:
    //     −1=111→5=101: o2=1, o1=0, o0=1
    //     −2=110→4=100: o2=1, o1=0, o0=0
    //     −3=101→3=011: o2=0, o1=1, o0=1
    //     −4=100→2=010: o2=0, o1=1, o0=0
    //     −5=011→1=001: o2=0, o1=0, o0=1
    //     −6=010→0=000: o2=0, o1=0, o0=0
    //   Re-table on (d2, d1):
    //     (1, 1) → o2=1, o1=0
    //     (1, 0) → o2=0, o1=1
    //     (0, 1) → o2=0, o1=0
    //   And o0 = d0 in all cases.
    //   So when bw3 = 1: o0 = d0, o1 = d2 & !d1, o2 = d2 & d1.
    let res_lo = d0;
    // When bw3 = 0: res_mid = d1, res_hi = d2.
    // When bw3 = 1: res_mid = d2 & !d1, res_hi = d2 & d1.
    let not_bw3 = !bw3;
    let res_mid = (d1 & not_bw3) | (bw3 & d2 & !d1);
    let res_hi = (d2 & not_bw3) | (bw3 & d2 & d1);
    // 7 ops; cumulative 20.
    (res_hi, res_mid, res_lo)
}

impl F7Encoding for VecB {
    const NAME: &'static str = "B: (z, log) split, F_7* cyclic";

    fn pack(canonical: &[u8]) -> Self {
        let len = canonical.len();
        let n_words = Self::n_words(len);
        let mut z = vec![0u64; n_words];
        let mut l2 = vec![0u64; n_words];
        let mut l1 = vec![0u64; n_words];
        let mut l0 = vec![0u64; n_words];
        for (i, &v) in canonical.iter().enumerate() {
            let w = i / ELEMS_PER_WORD;
            let s = i % ELEMS_PER_WORD;
            let (zi, l2i, l1i, l0i) = encode_elem(v);
            z[w] |= zi << s;
            l2[w] |= l2i << s;
            l1[w] |= l1i << s;
            l0[w] |= l0i << s;
        }
        VecB { z, l2, l1, l0, len }
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
        // c = a * b in F_7: z_c = z_a | z_b; log_c = (log_a + log_b) mod 6.
        let n = self.z.len();
        for w in 0..n {
            let zc = self.z[w] | other.z[w];
            let (c2, c1, c0) = log_add_mod6(
                self.l2[w],
                self.l1[w],
                self.l0[w],
                other.l2[w],
                other.l1[w],
                other.l0[w],
            );
            self.z[w] = zc;
            self.l2[w] = c2;
            self.l1[w] = c1;
            self.l0[w] = c0;
        }
    }

    fn div_assign(&mut self, other: &Self) {
        assert_eq!(self.len, other.len);
        // c = a / b in F_7: requires z_b = 0. log_c = (log_a − log_b) mod 6.
        let n = self.z.len();
        for w in 0..n {
            // z_c = z_a (since b nonzero, c = 0 ⇔ a = 0).
            let (c2, c1, c0) = log_sub_mod6(
                self.l2[w],
                self.l1[w],
                self.l0[w],
                other.l2[w],
                other.l1[w],
                other.l0[w],
            );
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
        prop::collection::vec(0u8..7, min..=max)
    }

    fn nonzero_vec_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(1u8..7, min..=max)
    }

    #[test]
    fn pack_unpack_roundtrip() {
        let mut rng = Lcg::new(13);
        for &n in &[0usize, 1, 63, 64, 65, 100, 1000] {
            let v = rng.f7_vec(n);
            let p = VecB::pack(&v);
            assert_eq!(p.unpack(), v, "n={n}");
        }
    }

    #[test]
    fn log_exp_tables_are_inverses() {
        for v in 1u8..7 {
            let l = LOG3[v as usize];
            assert_eq!(EXP3[l as usize], v, "v={v}");
        }
    }

    #[test]
    fn exhaustive_mul_pairs() {
        for a in 0u8..7 {
            for b in 0u8..7 {
                let mut va = VecB::pack(&[a]);
                let vb = VecB::pack(&[b]);
                va.mul_assign(&vb);
                assert_eq!(va.unpack()[0], ref_mul(a, b), "{a}*{b}");
            }
        }
    }

    #[test]
    fn exhaustive_div_pairs_nonzero_b() {
        for a in 0u8..7 {
            for b in 1u8..7 {
                let mut va = VecB::pack(&[a]);
                let vb = VecB::pack(&[b]);
                va.div_assign(&vb);
                assert_eq!(va.unpack()[0], ref_div(a, b), "{a}/{b}");
            }
        }
    }

    #[test]
    fn exhaustive_add_pairs() {
        for a in 0u8..7 {
            for b in 0u8..7 {
                let mut va = VecB::pack(&[a]);
                let vb = VecB::pack(&[b]);
                va.add_assign(&vb);
                assert_eq!(va.unpack()[0], ref_add(a, b), "{a}+{b}");
            }
        }
    }

    #[test]
    fn exhaustive_sub_pairs() {
        for a in 0u8..7 {
            for b in 0u8..7 {
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
