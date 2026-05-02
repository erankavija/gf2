//! F_3 LUT-A — symmetric port of F_5/F_7 Candidate A for cross-prime
//! comparison. Same encoding shape as `f5_packing/src/cand_a.rs` and
//! `f7_packing/src/cand_a.rs`: 16 elements per `u64` at 4-bit-aligned
//! slots; 2^16-entry binary-op LUT keyed on packed byte-pairs.
//!
//! For F_3 specifically, only canonical codepoints `0..=2` are used —
//! 13 of the 16 4-bit codepoints are wasted. This is structurally
//! inefficient but the **point** of including it: it shows what happens
//! if you take the F_5/F_7 R-decision winner and apply it unmodified to
//! F_3. The bipedal F_3 candidate is the natural "specialised" winner;
//! this LUT-A is the natural "uniform" runner-up that the cross-prime
//! comparison needs to make the case sharp.

use crate::common::{ref_add, ref_mul, ref_sub, F3Encoding, INV_F3};
use std::sync::OnceLock;

const SLOTS_PER_WORD: usize = 16;

type Lut = Box<[u8; 65536]>;

static ADD_LUT: OnceLock<Lut> = OnceLock::new();
static SUB_LUT: OnceLock<Lut> = OnceLock::new();
static MUL_LUT: OnceLock<Lut> = OnceLock::new();
static DIV_LUT: OnceLock<Lut> = OnceLock::new();

fn build_lut(op: fn(u8, u8) -> u8) -> Lut {
    let mut lut = vec![0u8; 65536].into_boxed_slice();
    for ap in 0u16..256 {
        let a0 = (ap & 0xf) as u8;
        let a1 = (ap >> 4) as u8;
        if a0 >= 3 || a1 >= 3 {
            continue;
        }
        for bp in 0u16..256 {
            let b0 = (bp & 0xf) as u8;
            let b1 = (bp >> 4) as u8;
            if b0 >= 3 || b1 >= 3 {
                continue;
            }
            let r0 = op(a0, b0);
            let r1 = op(a1, b1);
            let key = ap | (bp << 8);
            lut[key as usize] = r0 | (r1 << 4);
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
                (a * INV_F3[b as usize]) % 3
            }
        })
    })
}

#[derive(Clone, Debug)]
pub struct Lut3 {
    words: Vec<u64>,
    len: usize,
}

impl Lut3 {
    fn n_words(len: usize) -> usize {
        len.div_ceil(SLOTS_PER_WORD)
    }
}

#[inline]
fn binary_op_word(a: u64, b: u64, lut: &[u8; 65536]) -> u64 {
    let mut r: u64 = 0;
    for i in 0..8 {
        let ap = ((a >> (8 * i)) & 0xff) as u16;
        let bp = ((b >> (8 * i)) & 0xff) as u16;
        let key = ap | (bp << 8);
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

impl F3Encoding for Lut3 {
    const NAME: &'static str = "F_3 LUT-A (4-bit slots, 2^16 LUT)";

    fn pack(canonical: &[u8]) -> Self {
        let len = canonical.len();
        let n_words = Self::n_words(len);
        let mut words = vec![0u64; n_words];
        for (i, &v) in canonical.iter().enumerate() {
            debug_assert!(v < 3);
            let w = i / SLOTS_PER_WORD;
            let s = i % SLOTS_PER_WORD;
            words[w] |= (v as u64) << (4 * s);
        }
        Lut3 { words, len }
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
        prop::collection::vec(0u8..3, min..=max)
    }

    fn nonzero_vec_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(1u8..3, min..=max)
    }

    #[test]
    fn pack_unpack_roundtrip() {
        let mut rng = Lcg::new(23);
        for &n in &[0usize, 1, 15, 16, 17, 100, 1000] {
            let v = rng.f3_vec(n);
            assert_eq!(Lut3::pack(&v).unpack(), v, "n={n}");
        }
    }

    #[test]
    fn exhaustive_all_ops_pairs() {
        for a in 0u8..3 {
            for b in 0u8..3 {
                let mk = || (Lut3::pack(&[a]), Lut3::pack(&[b]));

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
            prop_assert_eq!(Lut3::pack(&v).unpack(), v);
        }

        #[test]
        fn prop_add_matches_scalar(
            a in vec_strategy(1, 256),
            b in vec_strategy(1, 256),
        ) {
            let n = a.len().min(b.len());
            let a = &a[..n]; let b = &b[..n];
            let mut va = Lut3::pack(a);
            va.add_assign(&Lut3::pack(b));
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
            let mut va = Lut3::pack(a);
            va.mul_assign(&Lut3::pack(b));
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
            let mut va = Lut3::pack(a);
            va.div_assign(&Lut3::pack(b));
            let got = va.unpack();
            for i in 0..n { prop_assert_eq!(got[i], ref_div(a[i], b[i])); }
        }
    }
}
