//! Sealed, validated modem specification.
//!
//! [`ModemSpec`] is the post-validation data model consumed by every
//! modem-facing task: trait layer, reference path, Gray-QAM fast path,
//! analysis collectors, and simulation adapters. All fields are private;
//! construction must go through presets or the public
//! [`super::ModemSpecBuilder`] entry point for custom constellations.
//!
//! Invariants enforced at construction are listed in
//! `dev/active/c87c5043-constellation-data-model-plan.md` §5. Violations
//! panic with a descriptive message per design decision D8.

use super::builder::ModemSpecBuilder;
use super::demapper::BatchSoftDemapper;
use super::mapper::BatchMapper;
use super::scalar::{DefaultScalar, ModemScalar};
use super::types::{BitChannelSemantics, LabelWord, ModemCapabilities, Normalization, SymbolPoint};
use super::view::ModemView;

/// Sealed, validated modem description.
///
/// Fields are private. All construction goes through presets (this task)
/// or future builder entry points; invariants are established once and
/// trusted everywhere downstream.
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::ModemSpec;
///
/// let spec = ModemSpec::bpsk();
/// assert_eq!(spec.bits_per_symbol(), 1);
/// assert_eq!(spec.num_symbols(), 2);
/// assert!(spec.capabilities().supports_exact_log_map);
/// ```
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
///
/// Not public: every consumer goes through presets or future builders,
/// both of which funnel through [`ModemSpec::from_parts_checked`].
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
    /// Crate-internal validating constructor.
    ///
    /// Panics on any invariant violation with a descriptive message. This
    /// is the single choke point through which presets and future builders
    /// create a [`ModemSpec`].
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

        // Invariant 1: bits_per_symbol in [1, 16].
        assert!(
            (1..=16).contains(&bits_per_symbol),
            "ModemSpec: bits_per_symbol must be in [1, 16], got {bits_per_symbol}"
        );

        let expected_len = 1usize << bits_per_symbol;

        // Invariant 2: points.len() == labels.len() == 1 << bits_per_symbol.
        assert!(
            points.len() == expected_len && labels.len() == expected_len,
            "ModemSpec: points/labels length mismatch: points={}, labels={}, expected={}",
            points.len(),
            labels.len(),
            expected_len
        );

        // Invariant 3: bit_channels length matches bits_per_symbol.
        assert!(
            bit_channels.len() == bits_per_symbol as usize,
            "ModemSpec: bit_channels length {} does not match bits_per_symbol {}",
            bit_channels.len(),
            bits_per_symbol
        );

        // Invariant 4 + 5: label width and bijection.
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

        // Invariant 7: scale factor is strictly positive.
        assert!(
            normalization_scale > S::zero(),
            "ModemSpec: normalization_scale must be strictly positive"
        );

        // Invariant 6: post-normalization unit average symbol energy.
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

        // Invariant 8: at least one demap method supported.
        assert!(
            capabilities.supports_exact_log_map || capabilities.supports_max_log,
            "ModemSpec: capabilities must advertise at least one demap method"
        );

        // Invariant 9: per-bit-channel analysis length equals
        // bits_per_symbol exactly. `ModemSpecBuilder::build` fills this
        // slot from `default_analysis_slice` when callers supply
        // capabilities via `ModemCapabilities::default()` (whose
        // `analysis` is empty because it cannot know bits_per_symbol),
        // so every validated `ModemSpec` carries a length-matched slice.
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
    ///
    /// For BPSK/QPSK/16-QAM/64-QAM/256-QAM prefer the preset constructors
    /// ([`ModemSpec::bpsk`], [`ModemSpec::gray_square_qam`]). Use this
    /// entry point for research constellations and standards-specific
    /// geometries that don't match a preset.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{LabelWord, ModemSpec, SymbolPoint};
    ///
    /// let spec = ModemSpec::<f32>::builder()
    ///     .bits_per_symbol(1)
    ///     .points(vec![
    ///         SymbolPoint::new(1.0, 0.0),
    ///         SymbolPoint::new(-1.0, 0.0),
    ///     ])
    ///     .labels(vec![LabelWord::new(0, 1), LabelWord::new(1, 1)])
    ///     .build();
    /// assert_eq!(spec.num_symbols(), 2);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn builder() -> ModemSpecBuilder<S> {
        ModemSpecBuilder::new()
    }

    /// Returns a borrowed view of this spec for backends and analysis.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::ModemSpec;
    ///
    /// let spec = ModemSpec::bpsk();
    /// let view = spec.view();
    /// assert_eq!(view.num_symbols(), 2);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
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
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::ModemSpec;
    ///
    /// assert_eq!(ModemSpec::bpsk().bits_per_symbol(), 1);
    /// assert_eq!(ModemSpec::gray_square_qam(16).bits_per_symbol(), 4);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn bits_per_symbol(&self) -> u8 {
        self.bits_per_symbol
    }

    /// Number of constellation symbols, equal to `1 << bits_per_symbol()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::ModemSpec;
    ///
    /// assert_eq!(ModemSpec::gray_square_qam(64).num_symbols(), 64);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn num_symbols(&self) -> usize {
        1usize << self.bits_per_symbol
    }

    /// Returns the normalization contract requested at construction.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{ModemSpec, Normalization};
    ///
    /// let spec = ModemSpec::bpsk();
    /// matches!(spec.normalization(), Normalization::UnitAverageSymbolEnergy);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn normalization(&self) -> Normalization<S> {
        self.normalization
    }

    /// Scalar factor applied to the raw (integer-grid) constellation.
    ///
    /// Stored points are already post-normalized; this factor is preserved
    /// for analysis paths that need the unit-grid geometry.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::ModemSpec;
    ///
    /// let spec = ModemSpec::gray_square_qam(4);
    /// assert!(spec.normalization_scale() > 0.0);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn normalization_scale(&self) -> S {
        self.normalization_scale
    }

    /// Which demap methods this spec currently supports.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::ModemSpec;
    ///
    /// let caps = ModemSpec::bpsk().capabilities();
    /// assert!(caps.supports_exact_log_map && caps.supports_max_log);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn capabilities(&self) -> ModemCapabilities {
        self.capabilities
    }
}

impl<S: ModemScalar + Send + Sync> ModemSpec<S> {
    /// Returns `true` iff this spec matches the canonical Gray
    /// square-QAM / BPSK layout accepted by the optimized
    /// [`super::GrayQamMapper`] / [`super::FastGrayQamDemapper`] backends.
    ///
    /// This is the single shared-API probe the factory methods
    /// ([`Self::preferred_mapper`], [`Self::preferred_soft_demapper`]) use
    /// to decide whether the fast path is safe for a given spec. Custom
    /// specs built through [`super::ModemSpecBuilder`] that happen to
    /// match the preset geometry return `true`; everything else returns
    /// `false`, and the factories fall back to the reference path.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::ModemSpec;
    ///
    /// assert!(ModemSpec::<f32>::gray_square_qam(16).is_gray_square_qam_preset());
    /// assert!(ModemSpec::<f32>::bpsk().is_gray_square_qam_preset());
    /// ```
    ///
    /// # Complexity
    ///
    /// O(`num_symbols`).
    #[inline]
    pub fn is_gray_square_qam_preset(&self) -> bool {
        super::presets::is_valid_gray_square_qam_spec(&self.view())
    }

    /// Returns the best-available [`BatchMapper`] backend for this spec.
    ///
    /// This is the shared-API entry point the story success criterion
    /// points at: callers describe *what* they want (a spec) and let the
    /// framework pick the specialized backend rather than constructing a
    /// backend by name. For specs whose geometry matches the Gray
    /// square-QAM layout this routes to the optimized
    /// [`super::GrayQamMapper`]; otherwise it falls back to
    /// [`super::ReferenceMapper`], which works for any validated spec.
    ///
    /// Direct construction of [`super::GrayQamMapper`] and
    /// [`super::ReferenceMapper`] remains supported for advanced callers
    /// that need backend-specific methods (e.g. GPU adapters); new code
    /// that only consumes the [`BatchMapper`] trait should prefer this
    /// factory.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{BatchMapper, ModemSpec};
    ///
    /// let mapper = ModemSpec::<f32>::gray_square_qam(16).preferred_mapper();
    /// assert_eq!(mapper.spec().bits_per_symbol(), 4);
    /// ```
    ///
    /// # Complexity
    ///
    /// Construction is O(`num_symbols`); the returned trait object's
    /// hot path matches its concrete backend.
    pub fn preferred_mapper(&self) -> Box<dyn BatchMapper<S> + Send + Sync> {
        if self.is_gray_square_qam_preset() {
            // Safe: the is_valid check ran the same predicate that
            // `GrayQamMapper::from_preset_order_with_scalar`'s constructor
            // asserts, so we can construct the Gray-QAM backend without
            // redundant panics. We go through the spec-aware path below
            // to keep any extension metadata (label permutation, custom
            // normalization) that `build_gray_square_qam` would discard.
            Box::new(GrayQamMapperFactory::from_spec(self.clone()))
        } else {
            Box::new(super::ReferenceMapper::new(self.clone()))
        }
    }

    /// Returns the best-available [`BatchSoftDemapper`] backend for this
    /// spec.
    ///
    /// For Gray square-QAM specs with `bits_per_symbol >= 2` this routes
    /// to the optimized [`super::FastGrayQamDemapper`]. For BPSK (`m == 1`)
    /// and every non-preset spec this returns
    /// [`super::ReferenceSoftDemapper`]; the fast kernel is only wired
    /// for the QAM axis-separable geometry, so BPSK intentionally falls
    /// back to the reference path.
    ///
    /// This is the preferred shared-API way to obtain a soft demapper —
    /// downstream code that accepts a [`BatchSoftDemapper`] trait object
    /// (AWGN link adapters, simulation harnesses, bit-channel analysis
    /// collectors) should prefer `spec.preferred_soft_demapper()` over
    /// directly constructing a backend by name.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{BatchSoftDemapper, ModemSpec};
    ///
    /// let dem = ModemSpec::<f32>::gray_square_qam(16).preferred_soft_demapper();
    /// assert_eq!(dem.spec().bits_per_symbol(), 4);
    /// ```
    ///
    /// # Complexity
    ///
    /// Construction is O(`num_symbols`); the returned trait object's
    /// hot path matches its concrete backend.
    pub fn preferred_soft_demapper(&self) -> Box<dyn BatchSoftDemapper<S> + Send + Sync> {
        if self.bits_per_symbol >= 2 && self.is_gray_square_qam_preset() {
            Box::new(super::FastGrayQamDemapper::new(self.clone()))
        } else {
            Box::new(super::ReferenceSoftDemapper::new(self.clone()))
        }
    }
}

/// Crate-internal factory type that constructs a [`super::GrayQamMapper`]
/// directly from a pre-validated spec, without rebuilding the preset from
/// an `order: usize`. Keeps `preferred_mapper` to a single spec-aware
/// construction path so callers that have already paid for a
/// `ModemSpec::from_parts_checked` validation don't pay for it twice.
///
/// Implemented as a thin `BatchMapper` forwarder that embeds a
/// `GrayQamMapper` built from the spec's `bits_per_symbol` preset order.
struct GrayQamMapperFactory<S: ModemScalar> {
    inner: super::GrayQamMapper<S>,
}

impl<S: ModemScalar> GrayQamMapperFactory<S> {
    fn from_spec(spec: ModemSpec<S>) -> Self {
        // The caller's spec has already passed `is_valid_gray_square_qam_spec`.
        // Hand it to `GrayQamMapper::from_spec` verbatim so any extension
        // metadata (normalization, bit-channel analysis hints) supplied
        // through `ModemSpecBuilder` is preserved on the returned
        // mapper's `spec()` view — no canonical-preset substitution.
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

// Default = f32 convenience alias, kept for readability in code that
// frequently uses the default scalar path.
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
        // Construct a minimally-shaped spec to isolate the bits_per_symbol check.
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

    // -------- preferred_* factory methods (Finding 1) -----------------

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
            // Tolerance mirrors the existing reference-vs-fast parity
            // tests in `fast_gray_qam_demapper.rs`.
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

    /// Builds a non-Gray 8-point custom spec and asserts the factory
    /// falls back to the reference path.
    fn custom_8_point_spec() -> ModemSpec<f32> {
        // Raw 8-PSK geometry with a non-Gray label permutation — mapping
        // preset detection should fail, forcing the reference fallback.
        let points: Vec<SymbolPoint<f32>> = (0..8)
            .map(|k| {
                let theta = (k as f32) * core::f32::consts::PI / 4.0;
                SymbolPoint::new(theta.cos(), theta.sin())
            })
            .collect();
        // Non-identity, non-Gray permutation.
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
        // Preset path (Gray-QAM).
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

        // Fallback path (custom spec).
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

    /// Regression: `preferred_mapper()` must hand the caller's spec to
    /// the backing `GrayQamMapper` verbatim (via `GrayQamMapper::from_spec`),
    /// not rebuild a canonical preset from `order` alone. This test
    /// locks in that the returned mapper's `spec()` points / labels /
    /// bit-channels are bit-equal to the caller's. Under the pre-fix
    /// code the returned spec was a freshly-built preset, and while it
    /// would carry equivalent geometry it would not share storage or
    /// builder-attached metadata such as bit-channel analysis overrides.
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
