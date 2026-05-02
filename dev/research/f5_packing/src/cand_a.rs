//! Candidate A — 3-bit packed (4-bit aligned slots) + 2^16-entry LUT.
//!
//! # Layout
//!
//! Each `u64` packs **16 elements** at 4-bit slots. Slot `i` occupies bits
//! `[4i .. 4i+4)`. Canonical values are `0..=4`; the high bit of each slot
//! (bit `4i+3`) is reserved and always zero for canonical packings. There
//! are 3 redundant 4-bit codewords (`5`, `6`, `7`) which we simply do not
//! produce. Slots `8..=15` (top bit set) are illegal and the LUT entries
//! for keys reaching them are left as `0`.
//!
//! # Op cost (per `u64` = 16 F_5 ops)
//!
//! Every binary op is implemented as **8 LUT lookups** (one per byte pair).
//! Each lookup consumes 8 bits of `a` and 8 bits of `b` (= 2 elements each)
//! and produces 8 bits of result (= 2 packed elements).
//!
//! - Add / Sub / Mul / Div: 8 lookups + ~16 shift/mask ops per `u64`.
//! - LUT footprint: 4 × 64 KiB = 256 KiB total.

use crate::common::{ref_add, ref_mul, ref_sub, F5Encoding, INV_F5};
use std::sync::OnceLock;

const SLOTS_PER_WORD: usize = 16;

/// 64-KiB lookup tables, keyed by `(a_pair) | (b_pair << 8)`.
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
        if a0 >= 5 || a1 >= 5 {
            continue;
        }
        for bp in 0u16..256 {
            let b0 = (bp & 0xf) as u8;
            let b1 = (bp >> 4) as u8;
            if b0 >= 5 || b1 >= 5 {
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
    // a / b = a * inv(b). Only valid when b != 0; entry is 0 otherwise.
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

/// Packed F_5 vector for Candidate A.
#[derive(Clone, Debug)]
pub struct VecA {
    words: Vec<u64>,
    len: usize,
}

impl VecA {
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

impl F5Encoding for VecA {
    const NAME: &'static str = "A: 3-bit + 2^16 LUT (baseline)";

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
        VecA { words, len }
    }

    fn unpack(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            let w = i / SLOTS_PER_WORD;
            let s = i % SLOTS_PER_WORD;
            let v = ((self.words[w] >> (4 * s)) & 0xf) as u8;
            out.push(v);
        }
        out
    }

    fn len(&self) -> usize {
        self.len
    }

    fn add_assign(&mut self, other: &Self) {
        assert_eq!(self.len, other.len);
        let lut = add_lut();
        let (a, b) = (self.words.clone(), &other.words);
        binary_op(&mut self.words, &a, b, lut);
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

    fn elem_strategy() -> impl Strategy<Value = u8> {
        0u8..5
    }

    fn nonzero_strategy() -> impl Strategy<Value = u8> {
        1u8..5
    }

    fn vec_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(elem_strategy(), min..=max)
    }

    fn nonzero_vec_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(nonzero_strategy(), min..=max)
    }

    #[test]
    fn pack_unpack_roundtrip_smoke() {
        let mut rng = Lcg::new(11);
        for &n in &[0usize, 1, 15, 16, 17, 32, 100, 1000] {
            let v = rng.f5_vec(n);
            let packed = VecA::pack(&v);
            assert_eq!(packed.unpack(), v, "n={n}");
        }
    }

    #[test]
    fn exhaustive_add_pairs() {
        for a in 0u8..5 {
            for b in 0u8..5 {
                let mut va = VecA::pack(&[a]);
                let vb = VecA::pack(&[b]);
                va.add_assign(&vb);
                assert_eq!(va.unpack()[0], (a + b) % 5, "add {a}+{b}");
            }
        }
    }

    #[test]
    fn exhaustive_sub_pairs() {
        for a in 0u8..5 {
            for b in 0u8..5 {
                let mut va = VecA::pack(&[a]);
                let vb = VecA::pack(&[b]);
                va.sub_assign(&vb);
                assert_eq!(va.unpack()[0], (a + 5 - b) % 5, "sub {a}-{b}");
            }
        }
    }

    #[test]
    fn exhaustive_mul_pairs() {
        for a in 0u8..5 {
            for b in 0u8..5 {
                let mut va = VecA::pack(&[a]);
                let vb = VecA::pack(&[b]);
                va.mul_assign(&vb);
                assert_eq!(va.unpack()[0], (a * b) % 5, "mul {a}*{b}");
            }
        }
    }

    #[test]
    fn exhaustive_div_pairs_nonzero_b() {
        for a in 0u8..5 {
            for b in 1u8..5 {
                let mut va = VecA::pack(&[a]);
                let vb = VecA::pack(&[b]);
                va.div_assign(&vb);
                assert_eq!(va.unpack()[0], ref_div(a, b), "div {a}/{b}");
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn prop_pack_unpack(v in vec_strategy(0, 256)) {
            let p = VecA::pack(&v);
            prop_assert_eq!(p.unpack(), v);
        }

        #[test]
        fn prop_add_matches_scalar(
            a in vec_strategy(1, 256),
            b in vec_strategy(1, 256),
        ) {
            let n = a.len().min(b.len());
            let a = &a[..n]; let b = &b[..n];
            let mut va = VecA::pack(a);
            let vb = VecA::pack(b);
            va.add_assign(&vb);
            let got = va.unpack();
            for i in 0..n {
                prop_assert_eq!(got[i], (a[i] + b[i]) % 5);
            }
        }

        #[test]
        fn prop_mul_matches_scalar(
            a in vec_strategy(1, 256),
            b in vec_strategy(1, 256),
        ) {
            let n = a.len().min(b.len());
            let a = &a[..n]; let b = &b[..n];
            let mut va = VecA::pack(a);
            let vb = VecA::pack(b);
            va.mul_assign(&vb);
            let got = va.unpack();
            for i in 0..n {
                prop_assert_eq!(got[i], (a[i] * b[i]) % 5);
            }
        }

        #[test]
        fn prop_sub_matches_scalar(
            a in vec_strategy(1, 256),
            b in vec_strategy(1, 256),
        ) {
            let n = a.len().min(b.len());
            let a = &a[..n]; let b = &b[..n];
            let mut va = VecA::pack(a);
            let vb = VecA::pack(b);
            va.sub_assign(&vb);
            let got = va.unpack();
            for i in 0..n {
                prop_assert_eq!(got[i], (a[i] + 5 - b[i]) % 5);
            }
        }

        #[test]
        fn prop_div_matches_scalar(
            a in vec_strategy(1, 256),
            b in nonzero_vec_strategy(1, 256),
        ) {
            let n = a.len().min(b.len());
            let a = &a[..n]; let b = &b[..n];
            let mut va = VecA::pack(a);
            let vb = VecA::pack(b);
            va.div_assign(&vb);
            let got = va.unpack();
            for i in 0..n {
                prop_assert_eq!(got[i], ref_div(a[i], b[i]));
            }
        }
    }
}
