//! Candidate D — bit-sliced 3-plane Boolean with Mersenne-fold add.
//!
//! # Layout
//!
//! Each F_7 element is a canonical 3-bit value `v = 4·b2 + 2·b1 + b0`. Three
//! `Vec<u64>` planes (`b0`, `b1`, `b2`) each hold one canonical bit per
//! element. One `u64`-triple covers 64 elements.
//!
//! # The F_7-specific advantage
//!
//! `7 = 2^3 − 1` is **Mersenne**. So `(a + b) mod 7` for `a, b ∈ {0..=6}`
//! reduces in pure bit-parallel form via a single conditional subtract:
//!
//! - 4-bit ripple add → `(c3, s2, s1, s0)` with `sum ∈ {0..=12}`.
//! - When `c3 = 0` and `(s2, s1, s0) = (1, 1, 1)` (sum=7): fold to 0.
//! - When `c3 = 1`: `sum_low3 + 1` (no overflow since `sum_low3 ∈ {0..=4}`).
//! - Otherwise: result = `(s2, s1, s0)`.
//!
//! No 7-way decode is required for add. Sub uses `add(a, neg(b))` where
//! `neg(b)` is computed bit-sliced (~8 ops; `7 - b` for `b ≠ 0`, `0`
//! otherwise; `7 - b = b XOR 7` since `7 = 111` in 3 bits, masked by
//! "is non-zero").
//!
//! # Op count summary (per `u64`-triple = 64 elements)
//!
//! - **Add**: 31 bitwise ops (4-bit ripple add: 12; mod-7 conditional fold:
//!   19). ≈ 0.48 ops/element.
//! - **Sub**: 39 bitwise ops (neg: 8; add: 31). ≈ 0.61 ops/element.
//! - **Mul / Div**: 7-way decode (14 ops × 2 operands) + 36-cell cross-
//!   product ANDs + 30 result-tree ORs + 6 encode ORs ≈ 100 ops.
//!   ≈ 1.56 ops/element.

use crate::common::{ref_div, ref_mul, F7Encoding};

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

/// Decode a single `(b0, b1, b2)` operand into 7 mutually-exclusive
/// selectors `e0..e6`. 14 bitwise ops (3 NOTs + 4 reused ANDs +
/// 7 final ANDs). The 8th codeword (`b = 7`, all-ones) is implicitly
/// "not selected" by any e_i (since canonical packings never produce 7).
#[inline]
fn decode7(b0: u64, b1: u64, b2: u64) -> [u64; 7] {
    let n0 = !b0;
    let n1 = !b1;
    let n2 = !b2;
    let n2n1 = n2 & n1;
    let n2_b1 = n2 & b1;
    let b2_n1 = b2 & n1;
    let b2_b1 = b2 & b1;
    let e0 = n2n1 & n0;
    let e1 = n2n1 & b0;
    let e2 = n2_b1 & n0;
    let e3 = n2_b1 & b0;
    let e4 = b2_n1 & n0;
    let e5 = b2_n1 & b0;
    let e6 = b2_b1 & n0;
    [e0, e1, e2, e3, e4, e5, e6]
}

/// Apply F_7 binary op `op_table[i][j] = (i op j) mod 7` bit-sliced.
/// Returns `(c0, c1, c2)` as bit planes.
///
/// Uses the decode7 selectors for both operands. Cells producing result 0
/// are skipped — bit 0 of "result is zero" carries no information into
/// any output plane.
#[inline]
fn apply_table7(ea: [u64; 7], eb: [u64; 7], op_table: &[[u8; 7]; 7]) -> (u64, u64, u64) {
    let mut r = [0u64; 7];
    for (i, row) in op_table.iter().enumerate() {
        for (j, &v) in row.iter().enumerate() {
            r[v as usize] |= ea[i] & eb[j];
        }
    }
    // r[0] is unused: zero result contributes 0 to every output bit.
    let c0 = r[1] | r[3] | r[5];
    let c1 = r[2] | r[3] | r[6];
    let c2 = r[4] | r[5] | r[6];
    (c0, c1, c2)
}

#[inline]
fn make_table(op: fn(u8, u8) -> u8) -> [[u8; 7]; 7] {
    let mut t = [[0u8; 7]; 7];
    for (i, row) in t.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = op(i as u8, j as u8);
        }
    }
    t
}

#[inline]
fn make_div_table() -> [[u8; 7]; 7] {
    let mut t = [[0u8; 7]; 7];
    for (i, row) in t.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = if j == 0 { 0 } else { ref_div(i as u8, j as u8) };
        }
    }
    t
}

/// Bit-sliced negation in F_7: `neg(0) = 0`, `neg(x) = 7 - x` for `x ≠ 0`.
///
/// Since `7 = 0b111`, `7 - x = !x & 7` for `x ∈ {1..=6}`. For `x = 0`,
/// the result must be `0` instead of `7`. Implemented as
/// `(!b_k) & is_nonzero` per plane. 8 bitwise ops.
#[inline]
fn neg7(b0: u64, b1: u64, b2: u64) -> (u64, u64, u64) {
    let nz_lo = b0 | b1; // 1
    let nz = nz_lo | b2; // 2
    let neg0 = (!b0) & nz; // 4
    let neg1 = (!b1) & nz; // 6
    let neg2 = (!b2) & nz; // 8
    (neg0, neg1, neg2)
}

/// Bit-sliced add in F_7 via Mersenne fold (see module docs for the
/// algorithm and op count).
///
/// 31 ops per `u64`-triple = 64 elements ≈ 0.48 ops/element.
#[inline]
fn add7(a0: u64, a1: u64, a2: u64, b0: u64, b1: u64, b2: u64) -> (u64, u64, u64) {
    // 4-bit ripple add: 12 ops.
    let s0 = a0 ^ b0;
    let c1 = a0 & b0;
    let axb1 = a1 ^ b1;
    let s1 = axb1 ^ c1;
    let c2 = (a1 & b1) | (c1 & axb1);
    let axb2 = a2 ^ b2;
    let s2 = axb2 ^ c2;
    let c3 = (a2 & b2) | (c2 & axb2);

    // is7 covers sum_low3 = 0b111 → must fold to 0 (only possible when c3=0).
    let s1_and_s0 = s1 & s0;
    let is7 = s1_and_s0 & s2;
    let not_is7 = !is7;
    let not_c3 = !c3;
    // 4 ops; cumulative 16.

    // res when c3=0: (s2, s1, s0) gated by !is7.
    // res when c3=1: sum_low3 + 1 (no overflow because sum_low3 ∈ {0..=4}).
    //   inc_lo = !s0
    //   inc_mid = s1 ^ s0
    //   inc_hi = s2 ^ (s1 & s0)
    //
    // Combine via c3-mask.
    let branch_lo0 = s0 & not_is7;
    let branch_lo1 = !s0;
    let res_lo = (branch_lo0 & not_c3) | (branch_lo1 & c3);
    // 5 ops; cumulative 21.

    let branch_mid0 = s1 & not_is7;
    let branch_mid1 = s1 ^ s0;
    let res_mid = (branch_mid0 & not_c3) | (branch_mid1 & c3);
    // 5 ops; cumulative 26.

    let branch_hi0 = s2 & not_is7;
    let branch_hi1 = s2 ^ s1_and_s0;
    let res_hi = (branch_hi0 & not_c3) | (branch_hi1 & c3);
    // 5 ops; cumulative 31.

    (res_lo, res_mid, res_hi)
}

/// Bit-sliced sub in F_7: `sub(a, b) = add(a, neg(b))`.
/// 8 (neg) + 31 (add) = 39 ops per `u64`-triple.
#[inline]
fn sub7(a0: u64, a1: u64, a2: u64, b0: u64, b1: u64, b2: u64) -> (u64, u64, u64) {
    let (n0, n1, n2) = neg7(b0, b1, b2);
    add7(a0, a1, a2, n0, n1, n2)
}

impl F7Encoding for VecD {
    const NAME: &'static str = "D: bit-sliced 3-plane (Mersenne fold)";

    fn pack(canonical: &[u8]) -> Self {
        let len = canonical.len();
        let n_words = Self::n_words(len);
        let mut b0 = vec![0u64; n_words];
        let mut b1 = vec![0u64; n_words];
        let mut b2 = vec![0u64; n_words];
        for (i, &v) in canonical.iter().enumerate() {
            debug_assert!(v < 7);
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
        assert_eq!(self.len, other.len);
        let n = self.b0.len();
        for w in 0..n {
            let (lo, mid, hi) = add7(
                self.b0[w],
                self.b1[w],
                self.b2[w],
                other.b0[w],
                other.b1[w],
                other.b2[w],
            );
            self.b0[w] = lo;
            self.b1[w] = mid;
            self.b2[w] = hi;
        }
    }

    fn sub_assign(&mut self, other: &Self) {
        assert_eq!(self.len, other.len);
        let n = self.b0.len();
        for w in 0..n {
            let (lo, mid, hi) = sub7(
                self.b0[w],
                self.b1[w],
                self.b2[w],
                other.b0[w],
                other.b1[w],
                other.b2[w],
            );
            self.b0[w] = lo;
            self.b1[w] = mid;
            self.b2[w] = hi;
        }
    }

    fn mul_assign(&mut self, other: &Self) {
        assert_eq!(self.len, other.len);
        let table = make_table(ref_mul);
        let n = self.b0.len();
        for w in 0..n {
            let ea = decode7(self.b0[w], self.b1[w], self.b2[w]);
            let eb = decode7(other.b0[w], other.b1[w], other.b2[w]);
            let (c0, c1, c2) = apply_table7(ea, eb, &table);
            self.b0[w] = c0;
            self.b1[w] = c1;
            self.b2[w] = c2;
        }
    }

    fn div_assign(&mut self, other: &Self) {
        assert_eq!(self.len, other.len);
        let table = make_div_table();
        let n = self.b0.len();
        for w in 0..n {
            let ea = decode7(self.b0[w], self.b1[w], self.b2[w]);
            let eb = decode7(other.b0[w], other.b1[w], other.b2[w]);
            let (c0, c1, c2) = apply_table7(ea, eb, &table);
            self.b0[w] = c0;
            self.b1[w] = c1;
            self.b2[w] = c2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{ref_add, ref_sub, Lcg};
    use proptest::prelude::*;

    fn vec_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(0u8..7, min..=max)
    }

    fn nonzero_vec_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(1u8..7, min..=max)
    }

    #[test]
    fn pack_unpack_roundtrip() {
        let mut rng = Lcg::new(19);
        for &n in &[0usize, 1, 63, 64, 65, 100, 1000] {
            let v = rng.f7_vec(n);
            let p = VecD::pack(&v);
            assert_eq!(p.unpack(), v, "n={n}");
        }
    }

    /// Mersenne fold correctness: spot-check `add7` against `(a + b) mod 7`
    /// for every (a, b) ∈ {0..=6}².
    #[test]
    fn mersenne_fold_exhaustive() {
        for a in 0u8..7 {
            for b in 0u8..7 {
                let av = VecD::pack(&[a]);
                let bv = VecD::pack(&[b]);
                let (lo, mid, hi) =
                    add7(av.b0[0], av.b1[0], av.b2[0], bv.b0[0], bv.b1[0], bv.b2[0]);
                let result = (lo & 1) as u8 | (((mid & 1) as u8) << 1) | (((hi & 1) as u8) << 2);
                assert_eq!(result, (a + b) % 7, "{a}+{b}");
            }
        }
    }

    #[test]
    fn exhaustive_all_ops_pairs() {
        for a in 0u8..7 {
            for b in 0u8..7 {
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
