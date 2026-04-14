//! Correctness-first, backend-agnostic reference mapper.
//!
//! [`ReferenceMapper`] is the arbitrary-constellation path of the modem
//! framework. It works for any validated [`ModemSpec`] — Gray-QAM presets
//! as well as user-defined research constellations built through
//! [`super::ModemSpecBuilder`] — by precomputing a flat
//! `label -> (i, q)` lookup table at construction time.
//!
//! The Gray square-QAM fast path lives in a sibling module (task
//! `625f5e1b`). Callers who only need DVB-T2 Gray geometries should
//! prefer that implementation; everyone else (custom constellations,
//! research work, conformance tests) should use this reference mapper.
//!
//! Bit ordering follows the framework-wide MSB-first convention
//! documented on [`BatchMapper`]: within each symbol, offset `0` is the
//! most-significant bit of the [`super::LabelWord`].

use super::{BatchMapper, ModemScalar, ModemSpec, ModemView};

/// Correctness-first, backend-agnostic mapper for any validated
/// [`ModemSpec`].
///
/// Construction scans the spec's `(label, point)` pairs once and builds a
/// flat lookup table indexed by [`super::LabelWord::bits`]. The hot loop
/// then performs one table load per symbol. This makes the mapper O(1)
/// per symbol regardless of constellation geometry and gives a simple,
/// easily audited reference path for correctness testing and research
/// constellations.
///
/// For the optimized Gray square-QAM fast path, see the sibling mapper
/// implementation.
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::{BatchMapper, ModemSpec, ReferenceMapper};
///
/// let spec = ModemSpec::gray_square_qam(4);
/// let mapper = ReferenceMapper::new(spec);
/// let bits = [false, false, true, true];
/// let mut out_i = [0.0_f32; 2];
/// let mut out_q = [0.0_f32; 2];
/// mapper.map_bits(&bits, &mut out_i, &mut out_q);
/// assert_eq!(out_i.len(), 2);
/// ```
pub struct ReferenceMapper<S: ModemScalar> {
    spec: ModemSpec<S>,
    /// Label-integer → point lookup, indexed by `LabelWord::bits`.
    label_to_point: Vec<(S, S)>,
    bits_per_symbol: u8,
}

impl<S: ModemScalar> ReferenceMapper<S> {
    /// Takes ownership of a validated [`ModemSpec`] and precomputes the
    /// label → point lookup table.
    ///
    /// Because [`ModemSpec`] is sealed and all invariants (bijection,
    /// label width, length) are enforced at construction, no extra
    /// validation is required here.
    ///
    /// # Arguments
    ///
    /// * `spec` - The owned, validated modem specification. Can be a
    ///   preset ([`ModemSpec::bpsk`], [`ModemSpec::gray_square_qam`]) or
    ///   a custom spec built through [`super::ModemSpecBuilder`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{ModemSpec, ReferenceMapper};
    ///
    /// let mapper = ReferenceMapper::new(ModemSpec::bpsk());
    /// assert_eq!(mapper.spec_ref().num_symbols(), 2);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(M) where `M = 2^bits_per_symbol`.
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
    ///
    /// Useful for chaining into a demapper that needs to read the same
    /// spec, or for cloning into a sibling backend.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{ModemSpec, ReferenceMapper};
    ///
    /// let mapper = ReferenceMapper::new(ModemSpec::gray_square_qam(16));
    /// assert_eq!(mapper.spec_ref().bits_per_symbol(), 4);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
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
        // Cache expected (i, q) by label.bits before we move spec into the mapper.
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
        // Arbitrary 8 points on a non-standard geometry.
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
        // Snapshot (post-normalization) points keyed by label.bits.
        let mut expected: Vec<(f32, f32)> = vec![(0.0, 0.0); 8];
        for k in 0..8 {
            let l = view.label(k);
            let p = view.point(k);
            expected[l.bits as usize] = (p.i, p.q);
        }
        // Sanity: the permutation is non-identity.
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

        // Batch of 16 symbols: labels 0..16 in order.
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
        // 4 points on the axes, non-identity label permutation, f64 scalar.
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

    // Property test: random label permutations + random bit streams
    // round-trip correctly through the mapper.
    proptest! {
        #[test]
        fn prop_map_bits_matches_spec_for_random_permutation(
            m in 1u8..=4u8,
            seed in 0u64..5_000u64,
            batch_len in 0usize..8usize,
        ) {
            let n = 1usize << m;

            // Deterministic permutation (Fisher-Yates) via the shared
            // modem test LCG — SSOT helper in `test_oracle::Lcg`.
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

            // Generate a deterministic bit stream. A distinct seed mix
            // (XOR with a constant) keeps the stream decorrelated from
            // the permutation RNG above while still routing through the
            // SSOT `Lcg::label_stream` helper.
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
