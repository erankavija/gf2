//! Bipedal F_3 — the Scheinerman encoding from arxiv 2407.20205v2.
//!
//! # Encoding
//!
//! Each F_3 element `x ∈ {0, 1, 2}` is encoded as `(mag, sgn) ∈ F_2 × F_2`
//! using the convention `2 ≡ −1 (mod 3)`:
//!
//! | `x` | `mag` | `sgn` | meaning            |
//! |-----|-------|-------|--------------------|
//! | 0   | 0     | 0     | zero               |
//! | 1   | 1     | 0     | `+1`               |
//! | 2   | 1     | 1     | `−1` (canonical 2) |
//!
//! `(mag=0, sgn=1)` is a redundant codeword we never produce.
//!
//! # Layout
//!
//! Two parallel `Vec<u64>` planes (`mag`, `sgn`). Bit `s` of word `w`
//! carries the corresponding bit of element `64 · w + s`. One `u64`-pair
//! covers **64 F_3 elements**.
//!
//! # Op cost (per `u64`-pair = 64 F_3 elements)
//!
//! Following the paper (Algorithm 2):
//!
//! - **add**: `t = a.mag ^ a.sgn ^ b.sgn; u = b.mag & t;`
//!   `mag' = u | (a.mag ^ b.mag); sgn' = u ^ a.sgn` — **6 ops**.
//! - **mul**: `mag' = a.mag & b.mag; sgn' = a.sgn ^ b.sgn` — **2 ops**.
//! - **neg**: `mag' = a.mag; sgn' = a.sgn ^ a.mag` — **1 op**
//!   (zero stays zero; nonzero flips sign).
//! - **sub**: `add(a, neg(b))` — **7 ops**.
//! - **div**: F_3* = {1, 2} and `2 · 2 = 1 (mod 3)`, so every nonzero
//!   element is its own inverse → `div = mul` — **2 ops**.
//!
//! Per element: **0.094 ops/elem (add)**, **0.031 ops/elem (mul)**.
//!
//! Compared to the F_5/F_7 R-decision winners (≈ 4 ALU ops/elem +
//! 0.5 LUT loads/elem each), bipedal F_3 is structurally ~50× cheaper
//! in raw bitwise-op count per element.

use crate::common::F3Encoding;

const ELEMS_PER_WORD: usize = 64;

#[derive(Clone, Debug)]
pub struct Bipedal3 {
    mag: Vec<u64>,
    sgn: Vec<u64>,
    len: usize,
}

impl Bipedal3 {
    fn n_words(len: usize) -> usize {
        len.div_ceil(ELEMS_PER_WORD)
    }

    /// Borrow the raw magnitude word slice (`mag` leg of the bipedal pair).
    ///
    /// Exposes the internal packed representation so SIMD parity tests can
    /// assert bitwise agreement against the AVX2 kernel output rather than
    /// going through `unpack()` (which would only check canonical-decoded
    /// equality and miss alt-zero divergences).
    pub fn raw_mag(&self) -> &[u64] {
        &self.mag
    }

    /// Borrow the raw sign word slice (`sgn` leg of the bipedal pair).
    /// See [`Bipedal3::raw_mag`] for the test-parity rationale.
    pub fn raw_sgn(&self) -> &[u64] {
        &self.sgn
    }

    /// Mask used to clear bits at positions `>= len % 64` in the last word.
    /// Mirrors the project-wide tail-mask invariant from CLAUDE.md, applied
    /// after every mutating op so out-of-range slots stay canonical zero.
    fn tail_mask(len: usize) -> u64 {
        let r = len % ELEMS_PER_WORD;
        if r == 0 {
            !0
        } else {
            (1u64 << r) - 1
        }
    }

    fn mask_tail(&mut self) {
        if let Some(last) = self.mag.last_mut() {
            *last &= Self::tail_mask(self.len);
        }
        if let Some(last) = self.sgn.last_mut() {
            *last &= Self::tail_mask(self.len);
        }
    }
}

impl F3Encoding for Bipedal3 {
    const NAME: &'static str = "bipedal F_3 (paper, mag/sgn pair)";

    fn pack(canonical: &[u8]) -> Self {
        let len = canonical.len();
        let n_words = Self::n_words(len);
        let mut mag = vec![0u64; n_words];
        let mut sgn = vec![0u64; n_words];
        for (i, &v) in canonical.iter().enumerate() {
            debug_assert!(v < 3);
            let w = i / ELEMS_PER_WORD;
            let s = i % ELEMS_PER_WORD;
            // 0 → (0, 0), 1 → (1, 0), 2 → (1, 1).
            let m = if v != 0 { 1u64 } else { 0 };
            let g = if v == 2 { 1u64 } else { 0 };
            mag[w] |= m << s;
            sgn[w] |= g << s;
        }
        Bipedal3 { mag, sgn, len }
    }

    fn unpack(&self) -> Vec<u8> {
        (0..self.len)
            .map(|i| {
                let w = i / ELEMS_PER_WORD;
                let s = i % ELEMS_PER_WORD;
                let m = (self.mag[w] >> s) & 1;
                let g = (self.sgn[w] >> s) & 1;
                if m == 0 {
                    0
                } else if g == 0 {
                    1
                } else {
                    2
                }
            })
            .collect()
    }

    fn len(&self) -> usize {
        self.len
    }

    fn add_assign(&mut self, other: &Self) {
        assert_eq!(self.len, other.len);
        let n = self.mag.len();
        for w in 0..n {
            let am = self.mag[w];
            let asg = self.sgn[w];
            let bm = other.mag[w];
            let bsg = other.sgn[w];
            // Paper's Algorithm 2 — 6 bitwise ops per word (= 64 elements).
            let t = am ^ asg ^ bsg;
            let u = bm & t;
            self.mag[w] = u | (am ^ bm);
            self.sgn[w] = u ^ asg;
        }
        self.mask_tail();
    }

    fn sub_assign(&mut self, other: &Self) {
        assert_eq!(self.len, other.len);
        let n = self.mag.len();
        for w in 0..n {
            let am = self.mag[w];
            let asg = self.sgn[w];
            let bm = other.mag[w];
            // neg(b) = (b.mag, b.sgn ^ b.mag) — 1 op.
            let bsg = other.sgn[w] ^ bm;
            let t = am ^ asg ^ bsg;
            let u = bm & t;
            self.mag[w] = u | (am ^ bm);
            self.sgn[w] = u ^ asg;
        }
        self.mask_tail();
    }

    fn mul_assign(&mut self, other: &Self) {
        assert_eq!(self.len, other.len);
        let n = self.mag.len();
        for w in 0..n {
            // 2 ops per word (= 64 elements).
            self.mag[w] &= other.mag[w];
            self.sgn[w] ^= other.sgn[w];
        }
        self.mask_tail();
    }

    fn div_assign(&mut self, other: &Self) {
        // F_3*: every nonzero element is its own inverse, so div = mul
        // (when divisor is nonzero, which the harness guarantees).
        self.mul_assign(other);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{ref_add, ref_div, ref_mul, ref_sub, Lcg};
    use proptest::prelude::*;

    fn vec_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(0u8..3, min..=max)
    }

    fn nonzero_vec_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(1u8..3, min..=max)
    }

    #[test]
    fn pack_unpack_roundtrip() {
        let mut rng = Lcg::new(13);
        for &n in &[0usize, 1, 63, 64, 65, 127, 128, 129, 1000] {
            let v = rng.f3_vec(n);
            let p = Bipedal3::pack(&v);
            assert_eq!(p.unpack(), v, "n={n}");
        }
    }

    #[test]
    fn exhaustive_add_pairs() {
        for a in 0u8..3 {
            for b in 0u8..3 {
                let mut va = Bipedal3::pack(&[a]);
                let vb = Bipedal3::pack(&[b]);
                va.add_assign(&vb);
                assert_eq!(va.unpack()[0], ref_add(a, b), "{a}+{b}");
            }
        }
    }

    #[test]
    fn exhaustive_sub_pairs() {
        for a in 0u8..3 {
            for b in 0u8..3 {
                let mut va = Bipedal3::pack(&[a]);
                let vb = Bipedal3::pack(&[b]);
                va.sub_assign(&vb);
                assert_eq!(va.unpack()[0], ref_sub(a, b), "{a}-{b}");
            }
        }
    }

    #[test]
    fn exhaustive_mul_pairs() {
        for a in 0u8..3 {
            for b in 0u8..3 {
                let mut va = Bipedal3::pack(&[a]);
                let vb = Bipedal3::pack(&[b]);
                va.mul_assign(&vb);
                assert_eq!(va.unpack()[0], ref_mul(a, b), "{a}*{b}");
            }
        }
    }

    #[test]
    fn exhaustive_div_pairs_nonzero_b() {
        for a in 0u8..3 {
            for b in 1u8..3 {
                let mut va = Bipedal3::pack(&[a]);
                let vb = Bipedal3::pack(&[b]);
                va.div_assign(&vb);
                assert_eq!(va.unpack()[0], ref_div(a, b), "{a}/{b}");
            }
        }
    }

    /// Tail-mask invariant: out-of-range slots in the last word must stay 0.
    #[test]
    fn tail_mask_keeps_padding_zero() {
        let mut va = Bipedal3::pack(&[1, 2, 0, 1, 2]); // len = 5, padding 59 bits
        let vb = Bipedal3::pack(&[2, 2, 2, 2, 2]);
        va.add_assign(&vb);
        // Padding bits must be exactly zero — the mag and sgn words should
        // have only the low 5 bits possibly set.
        let mask = (1u64 << 5) - 1;
        assert_eq!(va.mag[0] & !mask, 0, "mag tail dirty");
        assert_eq!(va.sgn[0] & !mask, 0, "sgn tail dirty");
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn prop_pack_unpack(v in vec_strategy(0, 256)) {
            prop_assert_eq!(Bipedal3::pack(&v).unpack(), v);
        }

        #[test]
        fn prop_add_matches_scalar(
            a in vec_strategy(1, 256),
            b in vec_strategy(1, 256),
        ) {
            let n = a.len().min(b.len());
            let a = &a[..n]; let b = &b[..n];
            let mut va = Bipedal3::pack(a);
            va.add_assign(&Bipedal3::pack(b));
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
            let mut va = Bipedal3::pack(a);
            va.sub_assign(&Bipedal3::pack(b));
            let got = va.unpack();
            for i in 0..n { prop_assert_eq!(got[i], ref_sub(a[i], b[i])); }
        }

        #[test]
        fn prop_mul_matches_scalar(
            a in vec_strategy(1, 256),
            b in vec_strategy(1, 256),
        ) {
            let n = a.len().min(b.len());
            let a = &a[..n]; let b = &b[..n];
            let mut va = Bipedal3::pack(a);
            va.mul_assign(&Bipedal3::pack(b));
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
            let mut va = Bipedal3::pack(a);
            va.div_assign(&Bipedal3::pack(b));
            let got = va.unpack();
            for i in 0..n { prop_assert_eq!(got[i], ref_div(a[i], b[i])); }
        }
    }
}
