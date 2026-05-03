//! Naive F_17 — per-element scalar `(a OP b) % 17` on `Vec<u8>`. Ground-
//! truth baseline mirroring the F_3 prototype's `naive.rs`.

use crate::common::{ref_add, ref_div, ref_mul, ref_sub, F17Encoding};

#[derive(Clone, Debug)]
pub struct Naive17 {
    elems: Vec<u8>,
}

impl F17Encoding for Naive17 {
    const NAME: &'static str = "naive F_17 (scalar Vec<u8>)";

    fn pack(canonical: &[u8]) -> Self {
        debug_assert!(canonical.iter().all(|&v| v < 17));
        Naive17 {
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
        prop::collection::vec(0u8..17, min..=max)
    }

    fn nonzero_vec_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(1u8..17, min..=max)
    }

    #[test]
    fn pack_unpack_roundtrip() {
        let mut rng = Lcg::new(17);
        for &n in &[0usize, 1, 64, 1000] {
            let v = rng.f17_vec(n);
            assert_eq!(Naive17::pack(&v).unpack(), v);
        }
    }

    #[test]
    fn exhaustive_all_ops_pairs() {
        for a in 0u8..17 {
            for b in 0u8..17 {
                let mk = || (Naive17::pack(&[a]), Naive17::pack(&[b]));

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
            let mut va = Naive17::pack(a);
            va.add_assign(&Naive17::pack(b));
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
            let mut va = Naive17::pack(a);
            va.mul_assign(&Naive17::pack(b));
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
            let mut va = Naive17::pack(a);
            va.div_assign(&Naive17::pack(b));
            let got = va.unpack();
            for i in 0..n { prop_assert_eq!(got[i], ref_div(a[i], b[i])); }
        }
    }
}
