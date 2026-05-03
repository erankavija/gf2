//! F_17 LUT-A — 8-bit slot, 8 elements per `u64`, 2^16-entry binary-op LUT.
//!
//! # Layout difference vs F_5/F_7-A
//!
//! F_5 and F_7 fit in 4-bit slots, so LUT-A there packed 16 elements per
//! `u64` and the 2^16 LUT was keyed on `(byte_a) | (byte_b << 8)` returning
//! 2 results per byte. F_17 needs 5+ bits, so the natural variant is
//! 8-bit slots (1 element per byte, 8 elements per `u64`) with the
//! 2^16 LUT keyed on `(a << 8) | b` returning a single byte result.
//!
//! Per `u64` (= 8 F_17 ops):
//!
//! - 8 byte-level LUT lookups, each on 16-bit key (a, b).
//! - 8 byte extracts + 8 byte inserts.
//!
//! Per element: **1 LUT load + ~2 ALU ops**, about 2× the F_5/F_7-A per-
//! element cost (which got 0.5 LUT loads + 4 ALU ops/elem).

use crate::common::{ref_add, ref_mul, ref_sub, F17Encoding, INV_F17};
use std::sync::OnceLock;

const SLOTS_PER_WORD: usize = 8;

type Lut = Box<[u8; 65536]>;

static ADD_LUT: OnceLock<Lut> = OnceLock::new();
static SUB_LUT: OnceLock<Lut> = OnceLock::new();
static MUL_LUT: OnceLock<Lut> = OnceLock::new();
static DIV_LUT: OnceLock<Lut> = OnceLock::new();

fn build_lut(op: fn(u8, u8) -> u8) -> Lut {
    let mut lut = vec![0u8; 65536].into_boxed_slice();
    for a in 0u16..17 {
        for b in 0u16..17 {
            let key = (a << 8) | b;
            lut[key as usize] = op(a as u8, b as u8);
        }
    }
    let arr: Box<[u8; 65536]> = lut.try_into().expect("lut size matches array");
    arr
}

#[inline]
fn add_lut() -> &'static [u8; 65536] {
    ADD_LUT.get_or_init(|| build_lut(ref_add))
}

#[inline]
fn sub_lut() -> &'static [u8; 65536] {
    SUB_LUT.get_or_init(|| build_lut(ref_sub))
}

#[inline]
fn mul_lut() -> &'static [u8; 65536] {
    MUL_LUT.get_or_init(|| build_lut(ref_mul))
}

#[inline]
fn div_lut() -> &'static [u8; 65536] {
    DIV_LUT.get_or_init(|| {
        build_lut(|a, b| {
            if b == 0 {
                0
            } else {
                ((a as u16 * INV_F17[b as usize] as u16) % 17) as u8
            }
        })
    })
}

#[derive(Clone, Debug)]
pub struct Lut17 {
    words: Vec<u64>,
    len: usize,
}

impl Lut17 {
    fn n_words(len: usize) -> usize {
        len.div_ceil(SLOTS_PER_WORD)
    }
}

#[inline]
fn binary_op_word(a: u64, b: u64, lut: &[u8; 65536]) -> u64 {
    let mut r: u64 = 0;
    for i in 0..SLOTS_PER_WORD {
        let ab = ((a >> (8 * i)) & 0xff) as u16;
        let bb = ((b >> (8 * i)) & 0xff) as u16;
        let key = (ab << 8) | bb;
        r |= (lut[key as usize] as u64) << (8 * i);
    }
    r
}

#[inline]
fn binary_op(out: &mut [u64], a: &[u64], b: &[u64], lut: &[u8; 65536]) {
    for ((wa, wb), wo) in a.iter().zip(b.iter()).zip(out.iter_mut()) {
        *wo = binary_op_word(*wa, *wb, lut);
    }
}

impl F17Encoding for Lut17 {
    const NAME: &'static str = "F_17 LUT-A (8-bit slots, 2^16 LUT)";

    fn pack(canonical: &[u8]) -> Self {
        let len = canonical.len();
        let n_words = Self::n_words(len);
        let mut words = vec![0u64; n_words];
        for (i, &v) in canonical.iter().enumerate() {
            debug_assert!(v < 17);
            let w = i / SLOTS_PER_WORD;
            let s = i % SLOTS_PER_WORD;
            words[w] |= (v as u64) << (8 * s);
        }
        Lut17 { words, len }
    }

    fn unpack(&self) -> Vec<u8> {
        (0..self.len)
            .map(|i| ((self.words[i / SLOTS_PER_WORD] >> (8 * (i % SLOTS_PER_WORD))) & 0xff) as u8)
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
        prop::collection::vec(0u8..17, min..=max)
    }

    fn nonzero_vec_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(1u8..17, min..=max)
    }

    #[test]
    fn pack_unpack_roundtrip() {
        let mut rng = Lcg::new(23);
        for &n in &[0usize, 1, 7, 8, 9, 64, 1000] {
            let v = rng.f17_vec(n);
            assert_eq!(Lut17::pack(&v).unpack(), v, "n={n}");
        }
    }

    #[test]
    fn exhaustive_all_ops_pairs() {
        for a in 0u8..17 {
            for b in 0u8..17 {
                let mk = || (Lut17::pack(&[a]), Lut17::pack(&[b]));

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
            prop_assert_eq!(Lut17::pack(&v).unpack(), v);
        }

        #[test]
        fn prop_add_matches_scalar(
            a in vec_strategy(1, 256),
            b in vec_strategy(1, 256),
        ) {
            let n = a.len().min(b.len());
            let a = &a[..n]; let b = &b[..n];
            let mut va = Lut17::pack(a);
            va.add_assign(&Lut17::pack(b));
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
            let mut va = Lut17::pack(a);
            va.mul_assign(&Lut17::pack(b));
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
            let mut va = Lut17::pack(a);
            va.div_assign(&Lut17::pack(b));
            let got = va.unpack();
            for i in 0..n { prop_assert_eq!(got[i], ref_div(a[i], b[i])); }
        }
    }
}
