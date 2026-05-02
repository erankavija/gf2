//! Candidate D — bit-sliced 3-plane Boolean.
//!
//! # Layout
//!
//! Each F_5 element is a canonical 3-bit value `v = 4·b2 + 2·b1 + b0`. Three
//! `Vec<u64>` planes (`b0`, `b1`, `b2`) each hold one canonical bit per
//! element. One `u64`-triple covers 64 elements.
//!
//! # Circuit shape
//!
//! Every binary op uses a **5-way decode** of each operand into
//! mutually-exclusive selectors `ea_i`, `eb_j` for `i, j ∈ {0..5}`, then OR-
//! combines `ea_i & eb_j` into per-output-bit results.
//!
//! - Decode (per operand): 11 bitwise ops (3 NOTs + 8 ANDs, via shared sub-
//!   expressions). 22 ops for both operands together.
//! - **Mul**: 16 cross-product ANDs (`ea_i & eb_j` for `i, j ∈ 1..5`) +
//!   12 result ORs + 2 final ORs = 30 ops. Total mul ≈ 52 ops / 64 elements.
//! - **Add**: 20 cross-product ANDs (`ea_i & eb_j` where the result is
//!   nonzero) + 16 result ORs + 2 final ORs = 38 ops. Total add ≈ 60 ops /
//!   64 elements.
//!
//! Sub and div delegate to add/mul via element-wise neg/inv computed on the
//! canonical bits, since canonical bit-sliced negate is a small Boolean
//! `(b0', b1', b2') = (b1 XOR b2 XOR (b0 AND something) ...)` — for the
//! prototype we just compute neg/inv via a small lookup on (b0, b1, b2).

use crate::common::{ref_add, ref_div, ref_mul, ref_sub, F5Encoding};

const ELEMS_PER_WORD: usize = 64;

#[derive(Clone, Debug)]
pub struct VecD {
    b0: Vec<u64>,
    b1: Vec<u64>,
    b2: Vec<u64>,
    len: usize,
}

impl VecD {
    fn n_words(len: usize) -> usize {
        len.div_ceil(ELEMS_PER_WORD)
    }
}

/// Decode a single `(b0, b1, b2)` operand into 5 mutually-exclusive
/// selectors `e0..e4`. 11 bitwise ops (3 NOTs reused, 8 ANDs).
#[inline]
fn decode5(b0: u64, b1: u64, b2: u64) -> [u64; 5] {
    let n0 = !b0;
    let n1 = !b1;
    let n2 = !b2;
    let n2n1 = n2 & n1;
    let n2_1 = n2 & b1;
    let n1n0 = n1 & n0;
    let e0 = n2n1 & n0;
    let e1 = n2n1 & b0;
    let e2 = n2_1 & n0;
    let e3 = n2_1 & b0;
    let e4 = b2 & n1n0;
    [e0, e1, e2, e3, e4]
}

/// Apply F_5 binary op `op_table[i][j] = (i op j) mod 5` bit-sliced.
/// Returns `(c0, c1, c2)` as bit planes.
#[inline]
fn apply_table(ea: [u64; 5], eb: [u64; 5], op_table: &[[u8; 5]; 5]) -> (u64, u64, u64) {
    // Build per-result selectors r0..r4 by ORing the cells producing that result.
    let mut r = [0u64; 5];
    for (i, row) in op_table.iter().enumerate() {
        for (j, &v) in row.iter().enumerate() {
            r[v as usize] |= ea[i] & eb[j];
        }
    }
    let c0 = r[1] | r[3];
    let c1 = r[2] | r[3];
    let c2 = r[4];
    (c0, c1, c2)
}

#[inline]
fn make_table(op: fn(u8, u8) -> u8) -> [[u8; 5]; 5] {
    let mut t = [[0u8; 5]; 5];
    for (i, row) in t.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = op(i as u8, j as u8);
        }
    }
    t
}

#[inline]
fn make_div_table() -> [[u8; 5]; 5] {
    let mut t = [[0u8; 5]; 5];
    for (i, row) in t.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = if j == 0 { 0 } else { ref_div(i as u8, j as u8) };
        }
    }
    t
}

#[inline]
fn binary_op(out: &mut VecD, other: &VecD, op_table: &[[u8; 5]; 5]) {
    assert_eq!(out.len, other.len);
    let n = out.b0.len();
    for w in 0..n {
        let ea = decode5(out.b0[w], out.b1[w], out.b2[w]);
        let eb = decode5(other.b0[w], other.b1[w], other.b2[w]);
        let (c0, c1, c2) = apply_table(ea, eb, op_table);
        out.b0[w] = c0;
        out.b1[w] = c1;
        out.b2[w] = c2;
    }
}

impl F5Encoding for VecD {
    const NAME: &'static str = "D: bit-sliced 3-plane Boolean";

    fn pack(canonical: &[u8]) -> Self {
        let len = canonical.len();
        let n_words = Self::n_words(len);
        let mut b0 = vec![0u64; n_words];
        let mut b1 = vec![0u64; n_words];
        let mut b2 = vec![0u64; n_words];
        for (i, &v) in canonical.iter().enumerate() {
            debug_assert!(v < 5);
            let w = i / ELEMS_PER_WORD;
            let s = i % ELEMS_PER_WORD;
            b0[w] |= ((v as u64) & 1) << s;
            b1[w] |= (((v as u64) >> 1) & 1) << s;
            b2[w] |= (((v as u64) >> 2) & 1) << s;
        }
        VecD { b0, b1, b2, len }
    }

    fn unpack(&self) -> Vec<u8> {
        (0..self.len)
            .map(|i| {
                let w = i / ELEMS_PER_WORD;
                let s = i % ELEMS_PER_WORD;
                let bit0 = ((self.b0[w] >> s) & 1) as u8;
                let bit1 = ((self.b1[w] >> s) & 1) as u8;
                let bit2 = ((self.b2[w] >> s) & 1) as u8;
                bit0 | (bit1 << 1) | (bit2 << 2)
            })
            .collect()
    }

    fn len(&self) -> usize {
        self.len
    }

    fn add_assign(&mut self, other: &Self) {
        let table = make_table(ref_add);
        binary_op(self, other, &table);
    }

    fn sub_assign(&mut self, other: &Self) {
        let table = make_table(ref_sub);
        binary_op(self, other, &table);
    }

    fn mul_assign(&mut self, other: &Self) {
        let table = make_table(ref_mul);
        binary_op(self, other, &table);
    }

    fn div_assign(&mut self, other: &Self) {
        let table = make_div_table();
        binary_op(self, other, &table);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::Lcg;
    use proptest::prelude::*;

    fn vec_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(0u8..5, min..=max)
    }

    fn nonzero_vec_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(1u8..5, min..=max)
    }

    #[test]
    fn pack_unpack_roundtrip() {
        let mut rng = Lcg::new(19);
        for &n in &[0usize, 1, 63, 64, 65, 100, 1000] {
            let v = rng.f5_vec(n);
            let p = VecD::pack(&v);
            assert_eq!(p.unpack(), v, "n={n}");
        }
    }

    #[test]
    fn exhaustive_all_ops_pairs() {
        for a in 0u8..5 {
            for b in 0u8..5 {
                let mk = || (VecD::pack(&[a]), VecD::pack(&[b]));

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
            prop_assert_eq!(VecD::pack(&v).unpack(), v);
        }

        #[test]
        fn prop_add_matches_scalar(
            a in vec_strategy(1, 256),
            b in vec_strategy(1, 256),
        ) {
            let n = a.len().min(b.len());
            let a = &a[..n]; let b = &b[..n];
            let mut va = VecD::pack(a);
            va.add_assign(&VecD::pack(b));
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
            let mut va = VecD::pack(a);
            va.mul_assign(&VecD::pack(b));
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
            let mut va = VecD::pack(a);
            va.sub_assign(&VecD::pack(b));
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
            let mut va = VecD::pack(a);
            va.div_assign(&VecD::pack(b));
            let got = va.unpack();
            for i in 0..n { prop_assert_eq!(got[i], ref_div(a[i], b[i])); }
        }
    }
}
