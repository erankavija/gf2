//! [`ModemSpec`], the validated constellation description built by the
//! presets and by [`super::ModemSpecBuilder`].

use super::builder::ModemSpecBuilder;
use super::demapper::BatchSoftDemapper;
use super::mapper::BatchMapper;
use super::scalar::{DefaultScalar, ModemScalar};
use super::types::{BitChannelSemantics, LabelWord, ModemCapabilities, Normalization, SymbolPoint};
use super::view::ModemView;

/// Validated modem description: constellation points, bit labels and
/// per-bit metadata.
///
/// Construction goes through the presets or [`super::ModemSpecBuilder`],
/// which establish the invariants once.
#[derive(Debug, Clone)]
pub struct ModemSpec<S: ModemScalar> {
    points: Vec<SymbolPoint<S>>,
    labels: Vec<LabelWord>,
    bits_per_symbol: u8,
    bit_channels: Vec<BitChannelSemantics>,
    normalization: Normalization<S>,
    normalization_scale: S,
    capabilities: ModemCapabilities,
}

/// Raw (unvalidated) field bundle used by the crate-internal constructor.
pub(super) struct ModemSpecParts<S: ModemScalar> {
    pub points: Vec<SymbolPoint<S>>,
    pub labels: Vec<LabelWord>,
    pub bits_per_symbol: u8,
    pub bit_channels: Vec<BitChannelSemantics>,
    pub normalization: Normalization<S>,
    pub normalization_scale: S,
    pub capabilities: ModemCapabilities,
}

impl<S: ModemScalar> ModemSpec<S> {
    /// Validating constructor shared by the presets and the builder; panics
    /// with a descriptive message on any invariant violation.
    pub(super) fn from_parts_checked(parts: ModemSpecParts<S>) -> Self {
        let ModemSpecParts {
            points,
            labels,
            bits_per_symbol,
            bit_channels,
            normalization,
            normalization_scale,
            capabilities,
        } = parts;

        assert!(
            (1..=16).contains(&bits_per_symbol),
            "ModemSpec: bits_per_symbol must be in [1, 16], got {bits_per_symbol}"
        );

        let expected_len = 1usize << bits_per_symbol;

        assert!(
            points.len() == expected_len && labels.len() == expected_len,
            "ModemSpec: points/labels length mismatch: points={}, labels={}, expected={}",
            points.len(),
            labels.len(),
            expected_len
        );

        assert!(
            bit_channels.len() == bits_per_symbol as usize,
            "ModemSpec: bit_channels length {} does not match bits_per_symbol {}",
            bit_channels.len(),
            bits_per_symbol
        );

        let mut seen = vec![false; expected_len];
        for (idx, label) in labels.iter().enumerate() {
            assert!(
                label.width == bits_per_symbol,
                "ModemSpec: label at index {idx} has width {}, expected {bits_per_symbol}",
                label.width
            );
            let v = label.bits as usize;
            assert!(
                v < expected_len,
                "ModemSpec: label at index {idx} bits {v} out of range [0, {expected_len})"
            );
            assert!(
                !seen[v],
                "ModemSpec: labels are not a bijection (duplicate label bits {v})"
            );
            seen[v] = true;
        }
        // Bijection implies no missing labels given matching lengths, but
        // guard explicitly for clearer panic diagnostics.
        for (v, present) in seen.iter().enumerate() {
            assert!(
                *present,
                "ModemSpec: labels are not a bijection (missing label bits {v})"
            );
        }

        assert!(
            normalization_scale > S::zero(),
            "ModemSpec: normalization_scale must be strictly positive"
        );

        if let Normalization::UnitAverageSymbolEnergy = normalization {
            let mut acc = S::zero();
            for p in &points {
                acc = acc + p.energy();
            }
            let n = S::from_f64(points.len() as f64);
            let mean = acc / n;
            let tol = S::unit_energy_tolerance();
            let err = (mean - S::one()).abs();
            assert!(
                err <= tol,
                "ModemSpec: post-normalization mean symbol energy {mean:?} deviates from 1 by more than tolerance {tol:?}"
            );
        }

        assert!(
            capabilities.supports_exact_log_map || capabilities.supports_max_log,
            "ModemSpec: capabilities must advertise at least one demap method"
        );

        assert!(
            capabilities.analysis.len() == bits_per_symbol as usize,
            "ModemSpec: capabilities.analysis length {} does not match bits_per_symbol {}",
            capabilities.analysis.len(),
            bits_per_symbol
        );

        Self {
            points,
            labels,
            bits_per_symbol,
            bit_channels,
            normalization,
            normalization_scale,
            capabilities,
        }
    }
}

impl<S: ModemScalar> ModemSpec<S> {
    /// Starts a fluent [`ModemSpecBuilder`] for a custom constellation.
    #[inline]
    pub fn builder() -> ModemSpecBuilder<S> {
        ModemSpecBuilder::new()
    }

    /// Returns a borrowed view of this spec for backends and analysis.
    #[inline]
    pub fn view(&self) -> ModemView<'_, S> {
        ModemView::new(
            &self.points,
            &self.labels,
            &self.bit_channels,
            self.bits_per_symbol,
            self.normalization,
            self.normalization_scale,
            self.capabilities,
        )
    }

    /// Number of bits per symbol (label width).
    #[inline]
    pub fn bits_per_symbol(&self) -> u8 {
        self.bits_per_symbol
    }

    /// Number of constellation symbols, equal to `1 << bits_per_symbol()`.
    #[inline]
    pub fn num_symbols(&self) -> usize {
        1usize << self.bits_per_symbol
    }

    /// Returns the normalization contract requested at construction.
    #[inline]
    pub fn normalization(&self) -> Normalization<S> {
        self.normalization
    }

    /// Scalar factor applied to the raw (integer-grid) constellation.
    ///
    /// Stored points are already post-normalized; this factor is preserved
    /// for analysis paths that need the unit-grid geometry.
    #[inline]
    pub fn normalization_scale(&self) -> S {
        self.normalization_scale
    }

    /// Which demap methods this spec supports.
    #[inline]
    pub fn capabilities(&self) -> ModemCapabilities {
        self.capabilities
    }
}

impl<S: ModemScalar + Send + Sync> ModemSpec<S> {
    /// Returns `true` iff this spec matches the BPSK / Gray square-QAM
    /// layout accepted by [`super::GrayQamMapper`] and
    /// [`super::FastGrayQamDemapper`]; a builder-built spec with the preset
    /// geometry qualifies.
    ///
    /// # Complexity
    ///
    /// O(`num_symbols`).
    #[inline]
    pub fn is_gray_square_qam_preset(&self) -> bool {
        super::presets::is_valid_gray_square_qam_spec(&self.view())
    }

    /// Returns a [`BatchMapper`] backend for this spec:
    /// [`super::GrayQamMapper`] when [`Self::is_gray_square_qam_preset`]
    /// holds, [`super::ReferenceMapper`] otherwise.
    ///
    /// # Complexity
    ///
    /// O(`num_symbols`).
    pub fn preferred_mapper(&self) -> Box<dyn BatchMapper<S> + Send + Sync> {
        if self.is_gray_square_qam_preset() {
            // `from_spec` cannot panic: it asserts the predicate just
            // checked.
            Box::new(GrayQamMapperFactory::from_spec(self.clone()))
        } else {
            Box::new(super::ReferenceMapper::new(self.clone()))
        }
    }

    /// Returns a [`BatchSoftDemapper`] backend for this spec:
    /// [`super::FastGrayQamDemapper`] when `bits_per_symbol >= 2` and
    /// [`Self::is_gray_square_qam_preset`] holds,
    /// [`super::ReferenceSoftDemapper`] otherwise, BPSK included.
    ///
    /// # Complexity
    ///
    /// O(`num_symbols`).
    pub fn preferred_soft_demapper(&self) -> Box<dyn BatchSoftDemapper<S> + Send + Sync> {
        if self.bits_per_symbol >= 2 && self.is_gray_square_qam_preset() {
            Box::new(super::FastGrayQamDemapper::new(self.clone()))
        } else {
            Box::new(super::ReferenceSoftDemapper::new(self.clone()))
        }
    }
}

/// Wraps the [`super::GrayQamMapper`] that [`ModemSpec::preferred_mapper`] returns.
struct GrayQamMapperFactory<S: ModemScalar> {
    inner: super::GrayQamMapper<S>,
}

impl<S: ModemScalar> GrayQamMapperFactory<S> {
    fn from_spec(spec: ModemSpec<S>) -> Self {
        // The spec is stored verbatim, so builder-supplied metadata reaches
        // the mapper's `spec()` view.
        Self {
            inner: super::GrayQamMapper::<S>::from_spec(spec),
        }
    }
}

impl<S: ModemScalar> BatchMapper<S> for GrayQamMapperFactory<S> {
    #[inline]
    fn spec(&self) -> super::ModemView<'_, S> {
        self.inner.spec()
    }

    #[inline]
    fn map_bits(&self, bits: &[bool], out_i: &mut [S], out_q: &mut [S]) {
        self.inner.map_bits(bits, out_i, out_q);
    }
}

#[allow(dead_code)]
pub(super) type DefaultSpec = ModemSpec<DefaultScalar>;

#[cfg(test)]
mod tests {
    use super::super::demapper::DemapInput;
    use super::super::types::BitChannelAnalysis;
    use super::super::types::DemapMethod;
    use super::super::{BatchMapper, BatchSoftDemapper, ReferenceMapper, ReferenceSoftDemapper};
    use super::*;
    use crate::llr::Llr;

    fn valid_bpsk_parts() -> ModemSpecParts<f32> {
        ModemSpecParts {
            points: vec![SymbolPoint::new(1.0, 0.0), SymbolPoint::new(-1.0, 0.0)],
            labels: vec![LabelWord::new(0, 1), LabelWord::new(1, 1)],
            bits_per_symbol: 1,
            bit_channels: vec![BitChannelSemantics::SingleAxisPam(0)],
            normalization: Normalization::UnitAverageSymbolEnergy,
            normalization_scale: 1.0,
            capabilities: ModemCapabilities {
                supports_exact_log_map: true,
                supports_max_log: true,
                analysis: &[BitChannelAnalysis {
                    symmetric_llr_distribution: true,
                    conditionally_independent: true,
                    closed_form_llr_available: true,
                }],
            },
        }
    }

    #[test]
    fn test_from_parts_checked_accepts_bpsk() {
        let spec = ModemSpec::from_parts_checked(valid_bpsk_parts());
        assert_eq!(spec.num_symbols(), 2);
        assert_eq!(spec.bits_per_symbol(), 1);
    }

    #[test]
    #[should_panic(expected = "bits_per_symbol must be in [1, 16]")]
    fn test_invariant_bits_per_symbol_zero() {
        let mut parts = valid_bpsk_parts();
        parts.bits_per_symbol = 0;
        parts.bit_channels.clear();
        let _ = ModemSpec::from_parts_checked(parts);
    }

    #[test]
    #[should_panic(expected = "bits_per_symbol must be in [1, 16]")]
    fn test_invariant_bits_per_symbol_too_large() {
        let parts = ModemSpecParts::<f32> {
            points: Vec::new(),
            labels: Vec::new(),
            bits_per_symbol: 17,
            bit_channels: Vec::new(),
            normalization: Normalization::UnitAverageSymbolEnergy,
            normalization_scale: 1.0,
            capabilities: ModemCapabilities {
                supports_exact_log_map: true,
                supports_max_log: true,
                analysis: &[BitChannelAnalysis {
                    symmetric_llr_distribution: true,
                    conditionally_independent: true,
                    closed_form_llr_available: true,
                }],
            },
        };
        let _ = ModemSpec::from_parts_checked(parts);
    }

    #[test]
    #[should_panic(expected = "points/labels length mismatch")]
    fn test_invariant_length_mismatch() {
        let mut parts = valid_bpsk_parts();
        parts.points.pop();
        let _ = ModemSpec::from_parts_checked(parts);
    }

    #[test]
    #[should_panic(expected = "bit_channels length")]
    fn test_invariant_bit_channels_length() {
        let mut parts = valid_bpsk_parts();
        parts.bit_channels.clear();
        let _ = ModemSpec::from_parts_checked(parts);
    }

    #[test]
    #[should_panic(expected = "expected 1")]
    fn test_invariant_label_width_mismatch() {
        let mut parts = valid_bpsk_parts();
        parts.labels[0] = LabelWord::new(0, 2);
        // Must still fit in width; bits=0, width=2 is fine on the LabelWord
        // side but violates the ModemSpec invariant.
        let _ = ModemSpec::from_parts_checked(parts);
    }

    #[test]
    #[should_panic(expected = "not a bijection (duplicate")]
    fn test_invariant_duplicate_label() {
        let mut parts = valid_bpsk_parts();
        parts.labels[1] = LabelWord::new(0, 1);
        let _ = ModemSpec::from_parts_checked(parts);
    }

    #[test]
    #[should_panic(expected = "normalization_scale must be strictly positive")]
    fn test_invariant_nonpositive_scale() {
        let mut parts = valid_bpsk_parts();
        parts.normalization_scale = 0.0;
        let _ = ModemSpec::from_parts_checked(parts);
    }

    #[test]
    #[should_panic(expected = "post-normalization mean symbol energy")]
    fn test_invariant_unit_energy_violated() {
        let mut parts = valid_bpsk_parts();
        // Break normalization: both points at ±2 gives mean energy 4.
        parts.points = vec![SymbolPoint::new(2.0, 0.0), SymbolPoint::new(-2.0, 0.0)];
        parts.normalization_scale = 2.0;
        let _ = ModemSpec::from_parts_checked(parts);
    }

    #[test]
    #[should_panic(expected = "at least one demap method")]
    fn test_invariant_no_demap_capability() {
        let mut parts = valid_bpsk_parts();
        parts.capabilities = ModemCapabilities {
            supports_exact_log_map: false,
            supports_max_log: false,
            analysis: &[],
        };
        let _ = ModemSpec::from_parts_checked(parts);
    }

    fn deterministic_rx(n: usize, seed: u64) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        let mut rng = crate::modem::test_oracle::Lcg::new(seed);
        // next_unit_f32() already emits samples in [-1, 1]; no further scaling.
        let rx_i: Vec<f32> = (0..n).map(|_| rng.next_unit_f32()).collect();
        let rx_q: Vec<f32> = (0..n).map(|_| rng.next_unit_f32()).collect();
        let noise_var: Vec<f32> = vec![0.25_f32; n];
        (rx_i, rx_q, noise_var)
    }

    #[test]
    fn test_preferred_soft_demapper_matches_reference_on_presets() {
        for &order in &[2usize, 4, 16, 64, 256] {
            let spec = ModemSpec::<f32>::gray_square_qam(order);
            let m = spec.bits_per_symbol() as usize;
            let n = 32usize;
            let (rx_i, rx_q, noise_var) = deterministic_rx(n, order as u64);
            let input = DemapInput::<f32> {
                rx_i: &rx_i,
                rx_q: &rx_q,
                gain_i: None,
                gain_q: None,
                noise_var: &noise_var,
                method: DemapMethod::MaxLog,
            };
            let preferred = spec.preferred_soft_demapper();
            let reference = ReferenceSoftDemapper::new(spec.clone());
            let mut out_pref = vec![Llr::new(0.0); n * m];
            let mut out_ref = vec![Llr::new(0.0); n * m];
            preferred.demap_llrs(input, &mut out_pref);
            reference.demap_llrs(input, &mut out_ref);
            for k in 0..n * m {
                let a = out_pref[k].value();
                let b = out_ref[k].value();
                let diff = (a - b).abs();
                assert!(
                    diff <= 1e-3 + 1e-3 * b.abs(),
                    "order={order} bit={k} preferred={a} reference={b} diff={diff}"
                );
            }
        }
    }

    /// 8-PSK with a non-Gray label permutation: a spec that is not a preset.
    fn custom_8_point_spec() -> ModemSpec<f32> {
        let points: Vec<SymbolPoint<f32>> = (0..8)
            .map(|k| {
                let theta = (k as f32) * core::f32::consts::PI / 4.0;
                SymbolPoint::new(theta.cos(), theta.sin())
            })
            .collect();
        let labels_perm: [u16; 8] = [3, 1, 6, 4, 0, 7, 2, 5];
        let labels: Vec<LabelWord> = labels_perm.iter().map(|&b| LabelWord::new(b, 3)).collect();

        super::super::ModemSpecBuilder::new()
            .bits_per_symbol(3)
            .points(points)
            .labels(labels)
            .build()
    }

    #[test]
    fn test_preferred_soft_demapper_falls_back_to_reference_on_custom_spec() {
        let spec = custom_8_point_spec();
        assert!(!spec.is_gray_square_qam_preset());
        let m = spec.bits_per_symbol() as usize;
        let n = 16usize;
        let (rx_i, rx_q, noise_var) = deterministic_rx(n, 0xDEADBEEF);
        let input = DemapInput::<f32> {
            rx_i: &rx_i,
            rx_q: &rx_q,
            gain_i: None,
            gain_q: None,
            noise_var: &noise_var,
            method: DemapMethod::ExactLogMap,
        };
        let preferred = spec.preferred_soft_demapper();
        let reference = ReferenceSoftDemapper::new(spec.clone());
        let mut out_pref = vec![Llr::new(0.0); n * m];
        let mut out_ref = vec![Llr::new(0.0); n * m];
        preferred.demap_llrs(input, &mut out_pref);
        reference.demap_llrs(input, &mut out_ref);
        for k in 0..n * m {
            // Exact equality: fallback must route through the same
            // reference kernel, not a subtly different numerical path.
            assert_eq!(
                out_pref[k].value(),
                out_ref[k].value(),
                "fallback diverged from reference at bit {k}"
            );
        }
    }

    #[test]
    fn test_preferred_mapper_matches_reference_on_any_spec() {
        for &order in &[2usize, 4, 16, 64, 256] {
            let spec = ModemSpec::<f32>::gray_square_qam(order);
            let m = spec.bits_per_symbol() as usize;
            let n_sym = spec.num_symbols();
            let bits: Vec<bool> = (0..n_sym * m).map(|i| (i * 13 + 7) & 1 == 1).collect();
            let preferred = spec.preferred_mapper();
            let reference = ReferenceMapper::new(spec.clone());
            let mut i_pref = vec![0.0_f32; n_sym];
            let mut q_pref = vec![0.0_f32; n_sym];
            let mut i_ref = vec![0.0_f32; n_sym];
            let mut q_ref = vec![0.0_f32; n_sym];
            preferred.map_bits(&bits, &mut i_pref, &mut q_pref);
            reference.map_bits(&bits, &mut i_ref, &mut q_ref);
            for k in 0..n_sym {
                assert!(
                    (i_pref[k] - i_ref[k]).abs() < 1e-6 && (q_pref[k] - q_ref[k]).abs() < 1e-6,
                    "order={order} sym={k}"
                );
            }
        }

        let spec = custom_8_point_spec();
        let m = spec.bits_per_symbol() as usize;
        let n_sym = spec.num_symbols();
        let bits: Vec<bool> = (0..n_sym * m).map(|i| (i * 5 + 1) & 1 == 1).collect();
        let preferred = spec.preferred_mapper();
        let reference = ReferenceMapper::new(spec.clone());
        let mut i_pref = vec![0.0_f32; n_sym];
        let mut q_pref = vec![0.0_f32; n_sym];
        let mut i_ref = vec![0.0_f32; n_sym];
        let mut q_ref = vec![0.0_f32; n_sym];
        preferred.map_bits(&bits, &mut i_pref, &mut q_pref);
        reference.map_bits(&bits, &mut i_ref, &mut q_ref);
        assert_eq!(i_pref, i_ref);
        assert_eq!(q_pref, q_ref);
    }

    #[test]
    fn test_is_gray_square_qam_preset_detects_presets_and_rejects_custom() {
        for &order in &[2usize, 4, 16, 64, 256] {
            assert!(ModemSpec::<f32>::gray_square_qam(order).is_gray_square_qam_preset());
        }
        assert!(ModemSpec::<f32>::bpsk().is_gray_square_qam_preset());
        assert!(!custom_8_point_spec().is_gray_square_qam_preset());
    }

    #[test]
    fn test_preferred_mapper_preserves_caller_spec() {
        let caller = ModemSpec::<f32>::gray_square_qam(16);
        let preferred = caller.clone().preferred_mapper();
        let pref_view = preferred.spec();
        assert_eq!(pref_view.points(), caller.view().points());
        assert_eq!(pref_view.labels(), caller.view().labels());
        assert_eq!(pref_view.bit_channels(), caller.view().bit_channels());
        assert_eq!(pref_view.bits_per_symbol(), caller.view().bits_per_symbol());
    }
}
