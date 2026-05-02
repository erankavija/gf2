//! Naive F_3 — per-element scalar `(a OP b) % 3` on a `Vec<u8>`.
//!
//! This is the in-Rust analogue of the Julia naive Ryser baseline that the
//! Scheinerman paper (arxiv 2407.20205v2) reports an 86.9× speedup over.
//! Reproducing the paper's exact wall-clock comparison requires a Julia
//! Ryser implementation on the same machine, which is out of scope for
//! this prototype — but the per-element op cost in **Rust** gives an
//! apples-to-apples baseline against the bipedal F_3 numbers measured by
//! the same harness on the same host.
//!
//! `[hard]` correctness is guaranteed by exhaustive 3×3 verification + a
//! property-based suite, identical in shape to the bipedal candidate's
//! tests.

use crate::common::{ref_add, ref_div, ref_mul, ref_sub, F3Encoding};

#[derive(Clone, Debug)]
pub struct Naive3 {
    elems: Vec<u8>,
}

impl F3Encoding for Naive3 {
    const NAME: &'static str = "naive F_3 (scalar Vec<u8>)";

    fn pack(canonical: &[u8]) -> Self {
        debug_assert!(canonical.iter().all(|&v| v < 3));
        Naive3 {
            elems: canonical.to_vec(),
        }
    }

    fn unpack(&self) -> Vec<u8> {
        self.elems.clone()
    }

    fn len(&self) -> usize {
        self.elems.len()
    }

    fn add_assign(&mut self, other: &Self) {
        assert_eq!(self.len(), other.len());
        for (a, &b) in self.elems.iter_mut().zip(other.elems.iter()) {
            *a = ref_add(*a, b);
        }
    }

    fn sub_assign(&mut self, other: &Self) {
        assert_eq!(self.len(), other.len());
        for (a, &b) in self.elems.iter_mut().zip(other.elems.iter()) {
            *a = ref_sub(*a, b);
        }
    }

    fn mul_assign(&mut self, other: &Self) {
        assert_eq!(self.len(), other.len());
        for (a, &b) in self.elems.iter_mut().zip(other.elems.iter()) {
            *a = ref_mul(*a, b);
        }
    }

    fn div_assign(&mut self, other: &Self) {
        assert_eq!(self.len(), other.len());
        for (a, &b) in self.elems.iter_mut().zip(other.elems.iter()) {
            debug_assert!(b != 0);
            *a = ref_div(*a, b);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::Lcg;
    use proptest::prelude::*;

    fn vec_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(0u8..3, min..=max)
    }

    fn nonzero_vec_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(1u8..3, min..=max)
    }

    #[test]
    fn pack_unpack_roundtrip() {
        let mut rng = Lcg::new(17);
        for &n in &[0usize, 1, 64, 1000] {
            let v = rng.f3_vec(n);
            assert_eq!(Naive3::pack(&v).unpack(), v);
        }
    }

    #[test]
    fn exhaustive_all_ops_pairs() {
        for a in 0u8..3 {
            for b in 0u8..3 {
                let mk = || (Naive3::pack(&[a]), Naive3::pack(&[b]));

                let (mut va, vb) = mk();
                va.add_assign(&vb);
                assert_eq!(va.unpack()[0], ref_add(a, b));

                let (mut va, vb) = mk();
                va.sub_assign(&vb);
                assert_eq!(va.unpack()[0], ref_sub(a, b));

                let (mut va, vb) = mk();
                va.mul_assign(&vb);
                assert_eq!(va.unpack()[0], ref_mul(a, b));

                if b != 0 {
                    let (mut va, vb) = mk();
                    va.div_assign(&vb);
                    assert_eq!(va.unpack()[0], ref_div(a, b));
                }
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn prop_add_matches_scalar(
            a in vec_strategy(1, 256),
            b in vec_strategy(1, 256),
        ) {
            let n = a.len().min(b.len());
            let a = &a[..n]; let b = &b[..n];
            let mut va = Naive3::pack(a);
            va.add_assign(&Naive3::pack(b));
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
            let mut va = Naive3::pack(a);
            va.mul_assign(&Naive3::pack(b));
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
            let mut va = Naive3::pack(a);
            va.div_assign(&Naive3::pack(b));
            let got = va.unpack();
            for i in 0..n { prop_assert_eq!(got[i], ref_div(a[i], b[i])); }
        }
    }
}
