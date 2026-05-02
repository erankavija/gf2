//! Candidate C — 4-bit aligned slot + 2^8 nibble-level LUT.
//!
//! # Layout
//!
//! Same as Candidate A: 16 elements per `u64` at 4-bit slots.
//!
//! # Difference from Candidate A
//!
//! Lookup is **per-nibble pair** with an 8-bit key `(a_nibble << 4) | b_nibble`.
//! Each LUT is **256 bytes** — fits in 4 cache lines and stays hot in L1d
//! across the entire inner loop. Per `u64`: 16 lookups (vs. A's 8 lookups
//! against a 64 KiB LUT). The trade-off being measured here is *more
//! lookups vs. better cache locality*.

use crate::common::{ref_add, ref_mul, ref_sub, F5Encoding, INV_F5};
use std::sync::OnceLock;

const SLOTS_PER_WORD: usize = 16;

type Lut = Box<[u8; 256]>;

static ADD_LUT: OnceLock<Lut> = OnceLock::new();
static SUB_LUT: OnceLock<Lut> = OnceLock::new();
static MUL_LUT: OnceLock<Lut> = OnceLock::new();
static DIV_LUT: OnceLock<Lut> = OnceLock::new();

fn build_lut(op: fn(u8, u8) -> u8) -> Lut {
    let mut lut: Box<[u8; 256]> = Box::new([0u8; 256]);
    for a in 0u8..5 {
        for b in 0u8..5 {
            lut[((a as usize) << 4) | b as usize] = op(a, b);
        }
    }
    lut
}

#[inline]
fn add_lut() -> &'static [u8; 256] {
    ADD_LUT.get_or_init(|| build_lut(ref_add))
}

#[inline]
fn sub_lut() -> &'static [u8; 256] {
    SUB_LUT.get_or_init(|| build_lut(ref_sub))
}

#[inline]
fn mul_lut() -> &'static [u8; 256] {
    MUL_LUT.get_or_init(|| build_lut(ref_mul))
}

#[inline]
fn div_lut() -> &'static [u8; 256] {
    DIV_LUT.get_or_init(|| {
        build_lut(|a, b| {
            if b == 0 {
                0
            } else {
                (a * INV_F5[b as usize]) % 5
            }
        })
    })
}

#[derive(Clone, Debug)]
pub struct VecC {
    words: Vec<u64>,
    len: usize,
}

impl VecC {
    fn n_words(len: usize) -> usize {
        len.div_ceil(SLOTS_PER_WORD)
    }
}

#[inline]
fn binary_op_word(a: u64, b: u64, lut: &[u8; 256]) -> u64 {
    let mut r: u64 = 0;
    for i in 0..SLOTS_PER_WORD {
        let an = (a >> (4 * i)) & 0xf;
        let bn = (b >> (4 * i)) & 0xf;
        let key = ((an << 4) | bn) as usize;
        r |= ((lut[key] as u64) & 0xf) << (4 * i);
    }
    r
}

#[inline]
fn binary_op(out: &mut [u64], a: &[u64], b: &[u64], lut: &[u8; 256]) {
    for ((wa, wb), wo) in a.iter().zip(b.iter()).zip(out.iter_mut()) {
        *wo = binary_op_word(*wa, *wb, lut);
    }
}

impl F5Encoding for VecC {
    const NAME: &'static str = "C: 4-bit + 2^8 LUT (nibble-pair)";

    fn pack(canonical: &[u8]) -> Self {
        let len = canonical.len();
        let n_words = Self::n_words(len);
        let mut words = vec![0u64; n_words];
        for (i, &v) in canonical.iter().enumerate() {
            debug_assert!(v < 5);
            let w = i / SLOTS_PER_WORD;
            let s = i % SLOTS_PER_WORD;
            words[w] |= (v as u64) << (4 * s);
        }
        VecC { words, len }
    }

    fn unpack(&self) -> Vec<u8> {
        (0..self.len)
            .map(|i| ((self.words[i / SLOTS_PER_WORD] >> (4 * (i % SLOTS_PER_WORD))) & 0xf) as u8)
            .collect()
    }

    fn len(&self) -> usize {
        self.len
    }

    fn add_assign(&mut self, other: &Self) {
        assert_eq!(self.len, other.len);
        let lut = add_lut();
        let a = self.words.clone();
        binary_op(&mut self.words, &a, &other.words, lut);
    }

    fn sub_assign(&mut self, other: &Self) {
        assert_eq!(self.len, other.len);
        let lut = sub_lut();
        let a = self.words.clone();
        binary_op(&mut self.words, &a, &other.words, lut);
    }

    fn mul_assign(&mut self, other: &Self) {
        assert_eq!(self.len, other.len);
        let lut = mul_lut();
        let a = self.words.clone();
        binary_op(&mut self.words, &a, &other.words, lut);
    }

    fn div_assign(&mut self, other: &Self) {
        assert_eq!(self.len, other.len);
        let lut = div_lut();
        let a = self.words.clone();
        binary_op(&mut self.words, &a, &other.words, lut);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{ref_div, Lcg};
    use proptest::prelude::*;

    fn vec_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(0u8..5, min..=max)
    }

    fn nonzero_vec_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(1u8..5, min..=max)
    }

    #[test]
    fn pack_unpack_roundtrip() {
        let mut rng = Lcg::new(17);
        for &n in &[0usize, 1, 15, 16, 17, 100, 1000] {
            let v = rng.f5_vec(n);
            let p = VecC::pack(&v);
            assert_eq!(p.unpack(), v, "n={n}");
        }
    }

    #[test]
    fn exhaustive_all_ops_pairs() {
        for a in 0u8..5 {
            for b in 0u8..5 {
                let mk = || (VecC::pack(&[a]), VecC::pack(&[b]));
                let (mut va, vb) = mk();
                va.add_assign(&vb);
                assert_eq!(va.unpack()[0], ref_add(a, b), "+ {a},{b}");

                let (mut va, vb) = mk();
                va.sub_assign(&vb);
                assert_eq!(va.unpack()[0], ref_sub(a, b), "- {a},{b}");

                let (mut va, vb) = mk();
                va.mul_assign(&vb);
                assert_eq!(va.unpack()[0], ref_mul(a, b), "* {a},{b}");

                if b != 0 {
                    let (mut va, vb) = mk();
                    va.div_assign(&vb);
                    assert_eq!(va.unpack()[0], ref_div(a, b), "/ {a},{b}");
                }
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn prop_pack_unpack(v in vec_strategy(0, 256)) {
            prop_assert_eq!(VecC::pack(&v).unpack(), v);
        }

        #[test]
        fn prop_add_matches_scalar(
            a in vec_strategy(1, 256),
            b in vec_strategy(1, 256),
        ) {
            let n = a.len().min(b.len());
            let a = &a[..n]; let b = &b[..n];
            let mut va = VecC::pack(a);
            va.add_assign(&VecC::pack(b));
            let got = va.unpack();
            for i in 0..n { prop_assert_eq!(got[i], ref_add(a[i], b[i])); }
        }

        #[test]
        fn prop_mul_matches_scalar(
            a in vec_strategy(1, 256),
            b in vec_strategy(1, 256),
        ) {
            let n = a.len().min(b.len());
            let a = &a[..n]; let b = &b[..n];
            let mut va = VecC::pack(a);
            va.mul_assign(&VecC::pack(b));
            let got = va.unpack();
            for i in 0..n { prop_assert_eq!(got[i], ref_mul(a[i], b[i])); }
        }

        #[test]
        fn prop_sub_matches_scalar(
            a in vec_strategy(1, 256),
            b in vec_strategy(1, 256),
        ) {
            let n = a.len().min(b.len());
            let a = &a[..n]; let b = &b[..n];
            let mut va = VecC::pack(a);
            va.sub_assign(&VecC::pack(b));
            let got = va.unpack();
            for i in 0..n { prop_assert_eq!(got[i], ref_sub(a[i], b[i])); }
        }

        #[test]
        fn prop_div_matches_scalar(
            a in vec_strategy(1, 256),
            b in nonzero_vec_strategy(1, 256),
        ) {
            let n = a.len().min(b.len());
            let a = &a[..n]; let b = &b[..n];
            let mut va = VecC::pack(a);
            va.div_assign(&VecC::pack(b));
            let got = va.unpack();
            for i in 0..n { prop_assert_eq!(got[i], ref_div(a[i], b[i])); }
        }
    }
}
