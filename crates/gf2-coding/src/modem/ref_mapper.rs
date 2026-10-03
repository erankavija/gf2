//! Reference bit-to-symbol mapper for arbitrary constellations.

use super::{BatchMapper, ModemScalar, ModemSpec, ModemView};

/// Mapper for any validated [`ModemSpec`], backed by a lookup table indexed
/// by [`super::LabelWord::bits`] that is built once at construction.
pub struct ReferenceMapper<S: ModemScalar> {
    spec: ModemSpec<S>,
    /// Label-integer → point lookup, indexed by `LabelWord::bits`.
    label_to_point: Vec<(S, S)>,
    bits_per_symbol: u8,
}

impl<S: ModemScalar> ReferenceMapper<S> {
    /// Takes ownership of a validated [`ModemSpec`] and precomputes the
    /// label → point lookup table.
    pub fn new(spec: ModemSpec<S>) -> Self {
        let view = spec.view();
        let bits_per_symbol = view.bits_per_symbol();
        let n = view.num_symbols();
        let mut label_to_point: Vec<(S, S)> = vec![(S::zero(), S::zero()); n];
        for k in 0..n {
            let label = view.label(k);
            let point = view.point(k);
            label_to_point[label.bits as usize] = (point.i, point.q);
        }
        Self {
            spec,
            label_to_point,
            bits_per_symbol,
        }
    }

    /// Returns a borrowed reference to the owned [`ModemSpec`].
    #[inline]
    pub fn spec_ref(&self) -> &ModemSpec<S> {
        &self.spec
    }
}

impl<S: ModemScalar> BatchMapper<S> for ReferenceMapper<S> {
    fn spec(&self) -> ModemView<'_, S> {
        self.spec.view()
    }

    fn map_bits(&self, bits: &[bool], out_i: &mut [S], out_q: &mut [S]) {
        let m = self.bits_per_symbol as usize;
        let n_symbols = super::bit_pack::check_batch_lengths(
            "ReferenceMapper::map_bits",
            self.bits_per_symbol,
            bits.len(),
            out_i.len(),
            out_q.len(),
        );

        for k in 0..n_symbols {
            let base = k * m;
            let label = super::bit_pack::pack_label_msb_first(&bits[base..base + m]);
            let (i, q) = self.label_to_point[label as usize];
            out_i[k] = i;
            out_q[k] = q;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        BatchMapper, LabelWord, ModemSpec, ModemSpecBuilder, Normalization, SymbolPoint,
    };
    use super::ReferenceMapper;
    use proptest::prelude::*;

    use super::super::bit_pack::unpack_label_msb_first as label_to_bits;
    #[allow(unused_imports)]
    use super::super::test_oracle::{label_stream, permutation, Lcg};

    #[test]
    fn test_map_bits_gray16_roundtrip_against_spec() {
        let spec = ModemSpec::<f32>::gray_square_qam(16);
        let view = spec.view();
        let n = view.num_symbols();
        let mut expected: Vec<(f32, f32)> = vec![(0.0, 0.0); n];
        for k in 0..n {
            let l = view.label(k);
            let p = view.point(k);
            expected[l.bits as usize] = (p.i, p.q);
        }

        let mapper = ReferenceMapper::new(spec);
        for v in 0..n as u16 {
            let bits = label_to_bits(v, 4);
            let mut oi = [0.0_f32; 1];
            let mut oq = [0.0_f32; 1];
            mapper.map_bits(&bits, &mut oi, &mut oq);
            let (ei, eq) = expected[v as usize];
            assert_eq!(oi[0], ei, "I mismatch at label {v}");
            assert_eq!(oq[0], eq, "Q mismatch at label {v}");
        }
    }

    /// Builds a custom 8-point constellation with an explicit,
    /// non-identity label permutation.
    fn custom_8_point() -> (ModemSpec<f32>, [(f32, f32); 8], [u16; 8]) {
        let raw: [(f32, f32); 8] = [
            (1.0, 0.5),
            (-1.0, 0.25),
            (0.5, -1.0),
            (-0.5, 1.0),
            (0.75, 0.75),
            (-0.75, -0.75),
            (0.25, -0.25),
            (-0.25, 0.25),
        ];
        // Non-identity permutation: labels[k] is the label assigned to
        // point index k.
        let labels_perm: [u16; 8] = [3, 1, 6, 4, 0, 7, 2, 5];

        let points: Vec<SymbolPoint<f32>> =
            raw.iter().map(|&(i, q)| SymbolPoint::new(i, q)).collect();
        let labels: Vec<LabelWord> = labels_perm.iter().map(|&b| LabelWord::new(b, 3)).collect();

        let spec = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(3)
            .points(points)
            .labels(labels)
            .normalization(Normalization::UnitAverageSymbolEnergy)
            .build();
        (spec, raw, labels_perm)
    }

    #[test]
    fn test_map_bits_custom_8_point_honors_permutation() {
        let (spec, _raw, labels_perm) = custom_8_point();
        let view = spec.view();
        let mut expected: Vec<(f32, f32)> = vec![(0.0, 0.0); 8];
        for k in 0..8 {
            let l = view.label(k);
            let p = view.point(k);
            expected[l.bits as usize] = (p.i, p.q);
        }
        assert!(labels_perm
            .iter()
            .enumerate()
            .any(|(k, &v)| v as usize != k));

        let mapper = ReferenceMapper::new(spec);
        for v in 0..8u16 {
            let bits = label_to_bits(v, 3);
            let mut oi = [0.0_f32; 1];
            let mut oq = [0.0_f32; 1];
            mapper.map_bits(&bits, &mut oi, &mut oq);
            assert_eq!((oi[0], oq[0]), expected[v as usize]);
        }
    }

    #[test]
    fn test_map_bits_multi_symbol_batch_msb_first() {
        let spec = ModemSpec::<f32>::gray_square_qam(16);
        let view = spec.view();
        let mut expected_by_label: Vec<(f32, f32)> = vec![(0.0, 0.0); 16];
        for k in 0..16 {
            let l = view.label(k);
            let p = view.point(k);
            expected_by_label[l.bits as usize] = (p.i, p.q);
        }
        let mapper = ReferenceMapper::new(spec);

        let mut bits: Vec<bool> = Vec::with_capacity(16 * 4);
        for v in 0..16u16 {
            bits.extend(label_to_bits(v, 4));
        }
        let mut oi = vec![0.0_f32; 16];
        let mut oq = vec![0.0_f32; 16];
        mapper.map_bits(&bits, &mut oi, &mut oq);
        for v in 0..16usize {
            assert_eq!((oi[v], oq[v]), expected_by_label[v]);
        }
    }

    #[test]
    fn test_map_bits_bpsk_preset_smoke() {
        let spec = ModemSpec::<f32>::bpsk();
        let view = spec.view();
        let mut expected: Vec<(f32, f32)> = vec![(0.0, 0.0); 2];
        for k in 0..2 {
            expected[view.label(k).bits as usize] = (view.point(k).i, view.point(k).q);
        }
        let mapper = ReferenceMapper::new(spec);
        let mut oi = [0.0_f32; 2];
        let mut oq = [0.0_f32; 2];
        mapper.map_bits(&[false, true], &mut oi, &mut oq);
        assert_eq!((oi[0], oq[0]), expected[0]);
        assert_eq!((oi[1], oq[1]), expected[1]);
    }

    #[test]
    fn test_map_bits_qpsk_preset_smoke() {
        let spec = ModemSpec::<f32>::gray_square_qam(4);
        let view = spec.view();
        let mut expected: Vec<(f32, f32)> = vec![(0.0, 0.0); 4];
        for k in 0..4 {
            expected[view.label(k).bits as usize] = (view.point(k).i, view.point(k).q);
        }
        let mapper = ReferenceMapper::new(spec);
        for v in 0..4u16 {
            let bits = label_to_bits(v, 2);
            let mut oi = [0.0_f32; 1];
            let mut oq = [0.0_f32; 1];
            mapper.map_bits(&bits, &mut oi, &mut oq);
            assert_eq!((oi[0], oq[0]), expected[v as usize]);
        }
    }

    #[test]
    #[should_panic(expected = "bits length 3 is not a multiple of bits_per_symbol 2")]
    fn test_map_bits_bits_not_multiple_panics() {
        let mapper = ReferenceMapper::new(ModemSpec::<f32>::gray_square_qam(4));
        let mut oi = [0.0_f32; 2];
        let mut oq = [0.0_f32; 2];
        mapper.map_bits(&[false, true, false], &mut oi, &mut oq);
    }

    #[test]
    #[should_panic(expected = "out_i length 3 does not match expected 2")]
    fn test_map_bits_out_i_length_mismatch_panics() {
        let mapper = ReferenceMapper::new(ModemSpec::<f32>::gray_square_qam(4));
        let bits = [false, false, true, true]; // 2 symbols
        let mut oi = [0.0_f32; 3];
        let mut oq = [0.0_f32; 2];
        mapper.map_bits(&bits, &mut oi, &mut oq);
    }

    #[test]
    #[should_panic(expected = "out_q length 1 does not match expected 2")]
    fn test_map_bits_out_q_length_mismatch_panics() {
        let mapper = ReferenceMapper::new(ModemSpec::<f32>::gray_square_qam(4));
        let bits = [false, false, true, true]; // 2 symbols
        let mut oi = [0.0_f32; 2];
        let mut oq = [0.0_f32; 1];
        mapper.map_bits(&bits, &mut oi, &mut oq);
    }

    #[test]
    fn test_map_bits_custom_f64_4_point() {
        let points = vec![
            SymbolPoint::<f64>::new(1.0, 0.0),
            SymbolPoint::<f64>::new(0.0, 1.0),
            SymbolPoint::<f64>::new(-1.0, 0.0),
            SymbolPoint::<f64>::new(0.0, -1.0),
        ];
        let labels_perm: [u16; 4] = [2, 0, 3, 1];
        let labels = labels_perm.iter().map(|&b| LabelWord::new(b, 2)).collect();
        let spec: ModemSpec<f64> = ModemSpecBuilder::<f64>::new()
            .bits_per_symbol(2)
            .points(points)
            .labels(labels)
            .build();
        let view = spec.view();
        let mut expected: Vec<(f64, f64)> = vec![(0.0, 0.0); 4];
        for k in 0..4 {
            expected[view.label(k).bits as usize] = (view.point(k).i, view.point(k).q);
        }
        let mapper = ReferenceMapper::new(spec);
        for v in 0..4u16 {
            let bits = label_to_bits(v, 2);
            let mut oi = [0.0_f64; 1];
            let mut oq = [0.0_f64; 1];
            mapper.map_bits(&bits, &mut oi, &mut oq);
            assert_eq!((oi[0], oq[0]), expected[v as usize]);
        }
    }

    proptest! {
        #[test]
        fn prop_map_bits_matches_spec_for_random_permutation(
            m in 1u8..=4u8,
            seed in 0u64..5_000u64,
            batch_len in 0usize..8usize,
        ) {
            let n = 1usize << m;

            let perm = permutation(seed, n);

            // Points on the unit circle (guarantees normalizable energy).
            let points: Vec<SymbolPoint<f32>> = (0..n)
                .map(|k| {
                    let theta = (k as f32) * core::f32::consts::TAU / (n as f32);
                    SymbolPoint::new(theta.cos(), theta.sin())
                })
                .collect();
            let labels: Vec<LabelWord> = perm.iter().map(|&b| LabelWord::new(b, m)).collect();
            let spec = ModemSpecBuilder::<f32>::new()
                .bits_per_symbol(m)
                .points(points)
                .labels(labels)
                .build();

            let view = spec.view();
            let mut expected: Vec<(f32, f32)> = vec![(0.0, 0.0); n];
            for k in 0..n {
                expected[view.label(k).bits as usize] =
                    (view.point(k).i, view.point(k).q);
            }

            // The XOR constant decorrelates this stream from the permutation RNG.
            let labels_stream: Vec<u16> =
                label_stream(seed ^ 0x9E37_79B9_7F4A_7C15, batch_len, n);
            let mut bits: Vec<bool> = Vec::with_capacity(batch_len * m as usize);
            for &v in &labels_stream {
                bits.extend(super::super::bit_pack::unpack_label_msb_first(v, m));
            }

            let mapper = ReferenceMapper::new(spec);
            let mut oi = vec![0.0_f32; batch_len];
            let mut oq = vec![0.0_f32; batch_len];
            mapper.map_bits(&bits, &mut oi, &mut oq);
            for (k, &v) in labels_stream.iter().enumerate() {
                prop_assert_eq!((oi[k], oq[k]), expected[v as usize]);
            }
        }
    }
}
