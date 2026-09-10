//! Fluent builder for custom [`ModemSpec`] constellations.
//!
//! Presets in [`super::presets`] already cover BPSK and Gray square-QAM
//! (orders 2/4/16/64/256). This builder is the public, user-facing entry
//! point for research constellations and standards-specific geometries
//! that don't match a preset.
//!
//! All validation funnels through the single choke point
//! `ModemSpec::from_parts_checked` (see plan §5). The builder itself only
//! covers:
//!
//! - Missing required fields (explicit panic at `build()` time).
//! - Computing the normalization scale when the caller asks for
//!   [`Normalization::UnitAverageSymbolEnergy`] or
//!   [`Normalization::ExplicitEs`].
//! - Filling in sensible defaults for [`BitChannelSemantics`] (all
//!   `Opaque`) and [`ModemCapabilities`] (both demap methods supported).
//!
//! Per plan §2 D8 the builder panics with a descriptive message on any
//! failure; there is no `Result` return type.
//!
//! # Examples
//!
//! ```
//! use gf2_coding::modem::{
//!     LabelWord, ModemSpec, ModemSpecBuilder, Normalization, SymbolPoint,
//! };
//!
//! // 4-point research constellation on the axes (not standard QPSK).
//! let spec: ModemSpec<f32> = ModemSpecBuilder::<f32>::new()
//!     .bits_per_symbol(2)
//!     .points(vec![
//!         SymbolPoint::new(1.0, 0.0),
//!         SymbolPoint::new(0.0, 1.0),
//!         SymbolPoint::new(-1.0, 0.0),
//!         SymbolPoint::new(0.0, -1.0),
//!     ])
//!     .labels(vec![
//!         LabelWord::new(0b00, 2),
//!         LabelWord::new(0b01, 2),
//!         LabelWord::new(0b11, 2),
//!         LabelWord::new(0b10, 2),
//!     ])
//!     .normalization(Normalization::UnitAverageSymbolEnergy)
//!     .build();
//!
//! assert_eq!(spec.num_symbols(), 4);
//! assert_eq!(spec.bits_per_symbol(), 2);
//! ```

use super::scalar::ModemScalar;
use super::spec::{ModemSpec, ModemSpecParts};
use super::types::{
    BitChannelAnalysis, BitChannelSemantics, LabelWord, ModemCapabilities, Normalization,
    SymbolPoint,
};

/// Conservative [`BitChannelAnalysis`] used as the builder default.
///
/// For arbitrary custom constellations none of the three analytic flags
/// can be asserted in general, so the builder's default fills every bit
/// channel with this all-`false` entry. Callers with known analytic
/// properties supply an explicit [`ModemCapabilities`] via
/// [`ModemSpecBuilder::capabilities`].
const DEFAULT_ANALYSIS: BitChannelAnalysis = BitChannelAnalysis {
    symmetric_llr_distribution: false,
    conditionally_independent: false,
    closed_form_llr_available: false,
};

/// 16-entry static pool of [`DEFAULT_ANALYSIS`] used to cheaply produce
/// an `&'static [BitChannelAnalysis]` of any length in `[1, 16]`.
///
/// [`ModemSpec`] enforces `bits_per_symbol in [1, 16]`, so a 16-entry
/// pool is always long enough; the builder slices off the needed prefix.
const DEFAULT_ANALYSIS_POOL: &[BitChannelAnalysis; 16] = &[DEFAULT_ANALYSIS; 16];

/// Returns a static [`BitChannelAnalysis`] slice of length
/// `bits_per_symbol` for the builder default.
///
/// Single source of truth for the "no known analytic properties"
/// fallback shared between [`ModemSpecBuilder::build`] and
/// [`ModemCapabilities::default`].
fn default_analysis_slice(bits_per_symbol: u8) -> &'static [BitChannelAnalysis] {
    assert!(
        (1..=16).contains(&bits_per_symbol),
        "default_analysis_slice: bits_per_symbol must be in [1, 16], got {bits_per_symbol}"
    );
    &DEFAULT_ANALYSIS_POOL[..bits_per_symbol as usize]
}

/// Fluent builder for a custom [`ModemSpec`].
///
/// Presets already exist for BPSK and Gray square-QAM; this builder is for
/// research constellations or standards-specific geometries that don't
/// match a preset. All validation runs at [`ModemSpecBuilder::build`] and
/// panics with a descriptive message on any invariant violation (see plan
/// §5 via the underlying `from_parts_checked` choke point).
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::{LabelWord, ModemSpecBuilder, SymbolPoint};
///
/// let spec = ModemSpecBuilder::<f32>::new()
///     .bits_per_symbol(1)
///     .points(vec![
///         SymbolPoint::new(1.0, 0.0),
///         SymbolPoint::new(-1.0, 0.0),
///     ])
///     .labels(vec![LabelWord::new(0, 1), LabelWord::new(1, 1)])
///     .build();
/// assert_eq!(spec.num_symbols(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct ModemSpecBuilder<S: ModemScalar> {
    bits_per_symbol: Option<u8>,
    points: Option<Vec<SymbolPoint<S>>>,
    labels: Option<Vec<LabelWord>>,
    bit_channels: Option<Vec<BitChannelSemantics>>,
    normalization: Normalization<S>,
    capabilities: Option<ModemCapabilities>,
}

impl<S: ModemScalar> ModemSpecBuilder<S> {
    /// Constructs an empty builder with default normalization
    /// ([`Normalization::UnitAverageSymbolEnergy`]).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::ModemSpecBuilder;
    ///
    /// let _builder = ModemSpecBuilder::<f32>::new();
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn new() -> Self {
        Self {
            bits_per_symbol: None,
            points: None,
            labels: None,
            bit_channels: None,
            normalization: Normalization::UnitAverageSymbolEnergy,
            capabilities: None,
        }
    }

    /// Declares the number of bits per symbol.
    ///
    /// Required. Must match `1..=16` and `points.len() == 2^bits_per_symbol`
    /// at build time.
    ///
    /// # Arguments
    ///
    /// * `m` - Bits per symbol, in `[1, 16]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::ModemSpecBuilder;
    ///
    /// let _ = ModemSpecBuilder::<f32>::new().bits_per_symbol(2);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn bits_per_symbol(mut self, m: u8) -> Self {
        self.bits_per_symbol = Some(m);
        self
    }

    /// Provides the constellation as owned I/Q pairs.
    ///
    /// # Arguments
    ///
    /// * `points` - One [`SymbolPoint`] per constellation index, parallel
    ///   to the `labels` vector. Length must equal `2^bits_per_symbol`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{ModemSpecBuilder, SymbolPoint};
    ///
    /// let _ = ModemSpecBuilder::<f32>::new().points(vec![
    ///     SymbolPoint::new(1.0, 0.0),
    ///     SymbolPoint::new(-1.0, 0.0),
    /// ]);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1) (moves the vector).
    #[inline]
    pub fn points(mut self, points: Vec<SymbolPoint<S>>) -> Self {
        self.points = Some(points);
        self
    }

    /// Provides the labels, one per constellation index.
    ///
    /// # Arguments
    ///
    /// * `labels` - Bit labels parallel to the `points` vector. Every
    ///   `LabelWord` must have `width == bits_per_symbol`, and together
    ///   they must form a bijection over `0..2^bits_per_symbol`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{LabelWord, ModemSpecBuilder};
    ///
    /// let _ = ModemSpecBuilder::<f32>::new()
    ///     .labels(vec![LabelWord::new(0, 1), LabelWord::new(1, 1)]);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1) (moves the vector).
    #[inline]
    pub fn labels(mut self, labels: Vec<LabelWord>) -> Self {
        self.labels = Some(labels);
        self
    }

    /// Overrides the default per-bit semantic tags.
    ///
    /// If omitted, every bit position is tagged
    /// [`BitChannelSemantics::Opaque`]`(k)`.
    ///
    /// # Arguments
    ///
    /// * `channels` - One [`BitChannelSemantics`] per bit position; length
    ///   must equal `bits_per_symbol`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{BitChannelSemantics, ModemSpecBuilder};
    ///
    /// let _ = ModemSpecBuilder::<f32>::new()
    ///     .bit_channels(vec![BitChannelSemantics::IAxisPam(0)]);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1) (moves the vector).
    #[inline]
    pub fn bit_channels(mut self, channels: Vec<BitChannelSemantics>) -> Self {
        self.bit_channels = Some(channels);
        self
    }

    /// Changes the normalization request.
    ///
    /// Default: [`Normalization::UnitAverageSymbolEnergy`].
    ///
    /// # Arguments
    ///
    /// * `norm` - Normalization contract; points are scaled to satisfy it
    ///   at build time.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{ModemSpecBuilder, Normalization};
    ///
    /// let _ = ModemSpecBuilder::<f32>::new()
    ///     .normalization(Normalization::ExplicitEs(2.0));
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn normalization(mut self, norm: Normalization<S>) -> Self {
        self.normalization = norm;
        self
    }

    /// Overrides the advertised demap capabilities.
    ///
    /// Default: both [`super::DemapMethod::ExactLogMap`] and
    /// [`super::DemapMethod::MaxLog`] supported.
    ///
    /// # Arguments
    ///
    /// * `caps` - Capability flags advertised by the built spec.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{ModemCapabilities, ModemSpecBuilder};
    ///
    /// let _ = ModemSpecBuilder::<f32>::new().capabilities(ModemCapabilities {
    ///     supports_exact_log_map: true,
    ///     supports_max_log: false,
    ///     analysis: &[],
    /// });
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn capabilities(mut self, caps: ModemCapabilities) -> Self {
        self.capabilities = Some(caps);
        self
    }

    /// Finalizes construction, computing the normalization scale and
    /// delegating invariant enforcement to the sealed
    /// `ModemSpec::from_parts_checked` choke point.
    ///
    /// # Panics
    ///
    /// - Missing required fields: `"ModemSpecBuilder: bits_per_symbol not set"`,
    ///   `"ModemSpecBuilder: points not set"`,
    ///   `"ModemSpecBuilder: labels not set"`.
    /// - Zero raw constellation energy (cannot normalize):
    ///   `"ModemSpecBuilder: raw constellation has zero energy; cannot normalize"`.
    /// - [`Normalization::ExplicitEs`] with a non-positive target.
    /// - Any spec invariant violation surfaced by `from_parts_checked`
    ///   (length mismatches, label bijection failure, tolerance miss, etc.).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{LabelWord, ModemSpecBuilder, SymbolPoint};
    ///
    /// let spec = ModemSpecBuilder::<f32>::new()
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
    /// O(N) in the number of constellation symbols.
    pub fn build(self) -> ModemSpec<S> {
        let Self {
            bits_per_symbol,
            points,
            labels,
            bit_channels,
            normalization,
            capabilities,
        } = self;

        let bits_per_symbol = bits_per_symbol.expect("ModemSpecBuilder: bits_per_symbol not set");
        let mut points = points.expect("ModemSpecBuilder: points not set");
        let labels = labels.expect("ModemSpecBuilder: labels not set");

        // Compute the scale factor from the caller's (pre-normalization)
        // points. Raw mean energy is computed in f64 for numerical
        // robustness across f32 inputs, then converted back to S.
        let scale = compute_scale::<S>(&points, normalization);

        // Apply the scale in-place so the stored points are
        // post-normalized (D4).
        for p in points.iter_mut() {
            *p = SymbolPoint::new(p.i * scale, p.q * scale);
        }

        // Default bit channels: Opaque(k) for every bit position.
        let bit_channels = bit_channels.unwrap_or_else(|| {
            (0..bits_per_symbol)
                .map(BitChannelSemantics::Opaque)
                .collect()
        });

        // Fill in the analysis slot for the "no explicit capabilities"
        // path: one conservative default entry per bit position. If the
        // caller supplied explicit capabilities we honor their analysis
        // slice as-is (it will be validated by from_parts_checked once
        // invariant D9 lands; today the length check happens at
        // ModemCapabilities construction sites).
        let mut capabilities = capabilities.unwrap_or_else(|| ModemCapabilities {
            supports_exact_log_map: true,
            supports_max_log: true,
            analysis: default_analysis_slice(bits_per_symbol),
        });
        // If the caller supplied capabilities via `ModemCapabilities::default()`
        // (which cannot know bits_per_symbol and so returns an empty
        // analysis slice), fill the slot from `default_analysis_slice` so
        // the built spec always satisfies the length invariant.
        if capabilities.analysis.is_empty() {
            capabilities.analysis = default_analysis_slice(bits_per_symbol);
        }

        let parts = ModemSpecParts {
            points,
            labels,
            bits_per_symbol,
            bit_channels,
            normalization,
            normalization_scale: scale,
            capabilities,
        };
        ModemSpec::from_parts_checked(parts)
    }
}

impl<S: ModemScalar> Default for ModemSpecBuilder<S> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ModemCapabilities {
    /// Default: both demap paths advertised, no per-bit analysis.
    ///
    /// The `analysis` slot defaults to an empty slice because
    /// [`ModemCapabilities`] does not know the surrounding spec's
    /// `bits_per_symbol` at this construction site. Prefer constructing
    /// capabilities through [`ModemSpecBuilder`] (which fills in a
    /// length-matched default) or the preset constructors (which ship
    /// compile-time analysis constants), rather than this raw default.
    #[inline]
    fn default() -> Self {
        Self {
            supports_exact_log_map: true,
            supports_max_log: true,
            analysis: &[],
        }
    }
}

/// Computes the normalization scale for the caller's (unnormalized) points.
///
/// Panics with a descriptive message if the raw average energy is zero
/// (cannot be normalized) or if [`Normalization::ExplicitEs`] carries a
/// non-positive target.
fn compute_scale<S: ModemScalar>(points: &[SymbolPoint<S>], normalization: Normalization<S>) -> S {
    if points.is_empty() {
        // Defer length mismatch diagnosis to from_parts_checked, but avoid
        // divide-by-zero here.
        return S::one();
    }

    // Raw mean energy in f64 — lossless for both f32 and f64 inputs.
    let mut acc: f64 = 0.0;
    for p in points {
        let i = p.i.to_f64();
        let q = p.q.to_f64();
        acc += i * i + q * q;
    }
    let mean = acc / (points.len() as f64);
    assert!(
        mean > 0.0,
        "ModemSpecBuilder: raw constellation has zero energy; cannot normalize"
    );

    match normalization {
        Normalization::UnitAverageSymbolEnergy => S::from_f64((1.0_f64 / mean).sqrt()),
        Normalization::ExplicitEs(target) => {
            let target_f = target.to_f64();
            assert!(
                target_f > 0.0,
                "ModemSpecBuilder: ExplicitEs target must be strictly positive, got {target_f}"
            );
            S::from_f64((target_f / mean).sqrt())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::BatchSoftDemapper;
    use super::*;
    use proptest::prelude::*;

    fn axis_4_point_builder() -> ModemSpecBuilder<f32> {
        ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(2)
            .points(vec![
                SymbolPoint::new(1.0, 0.0),
                SymbolPoint::new(0.0, 1.0),
                SymbolPoint::new(-1.0, 0.0),
                SymbolPoint::new(0.0, -1.0),
            ])
            .labels(vec![
                LabelWord::new(0b00, 2),
                LabelWord::new(0b01, 2),
                LabelWord::new(0b11, 2),
                LabelWord::new(0b10, 2),
            ])
    }

    #[test]
    fn test_build_happy_path_4_point() {
        let spec = axis_4_point_builder().build();
        assert_eq!(spec.bits_per_symbol(), 2);
        assert_eq!(spec.num_symbols(), 4);
        // Points were already unit energy (radius = 1 on each axis), so
        // the computed scale is 1.0; mean energy stays 1.
        let mean: f64 = spec
            .view()
            .points()
            .iter()
            .map(|p| (p.i as f64).powi(2) + (p.q as f64).powi(2))
            .sum::<f64>()
            / spec.num_symbols() as f64;
        assert!((mean - 1.0).abs() < 1e-5, "mean = {mean}");
    }

    #[test]
    fn test_build_unit_average_symbol_energy_rescales() {
        // Unnormalized points with varying energy: ±3, ±5 on I.
        let spec: ModemSpec<f32> = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(2)
            .points(vec![
                SymbolPoint::new(3.0, 0.0),
                SymbolPoint::new(-3.0, 0.0),
                SymbolPoint::new(5.0, 0.0),
                SymbolPoint::new(-5.0, 0.0),
            ])
            .labels(vec![
                LabelWord::new(0b00, 2),
                LabelWord::new(0b01, 2),
                LabelWord::new(0b10, 2),
                LabelWord::new(0b11, 2),
            ])
            .build();
        let mean: f64 = spec
            .view()
            .points()
            .iter()
            .map(|p| (p.i as f64).powi(2) + (p.q as f64).powi(2))
            .sum::<f64>()
            / spec.num_symbols() as f64;
        assert!((mean - 1.0).abs() < 1e-5, "mean = {mean}");
    }

    #[test]
    fn test_build_explicit_es_hits_target() {
        let target = 4.0_f32;
        let spec: ModemSpec<f32> = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(1)
            .points(vec![
                SymbolPoint::new(1.0, 0.0),
                SymbolPoint::new(-1.0, 0.0),
            ])
            .labels(vec![LabelWord::new(0, 1), LabelWord::new(1, 1)])
            .normalization(Normalization::ExplicitEs(target))
            .build();
        let mean: f64 = spec
            .view()
            .points()
            .iter()
            .map(|p| (p.i as f64).powi(2) + (p.q as f64).powi(2))
            .sum::<f64>()
            / spec.num_symbols() as f64;
        assert!((mean - target as f64).abs() < 1e-5, "mean = {mean}");
    }

    #[test]
    fn test_build_f64_preserves_full_precision() {
        // Regression: previously `compute_scale` routed f64 coordinates
        // through f32, rounding ±0.1 and violating the tight 1e-10
        // unit-energy tolerance at `from_parts_checked`. After the fix
        // the normalized energy must be accurate to f64 precision.
        let spec: ModemSpec<f64> = ModemSpecBuilder::<f64>::new()
            .bits_per_symbol(1)
            .points(vec![
                SymbolPoint::new(0.1_f64, 0.0),
                SymbolPoint::new(-0.1_f64, 0.0),
            ])
            .labels(vec![LabelWord::new(0, 1), LabelWord::new(1, 1)])
            .build();
        let mean: f64 = spec
            .view()
            .points()
            .iter()
            .map(|p| p.i * p.i + p.q * p.q)
            .sum::<f64>()
            / spec.num_symbols() as f64;
        assert!((mean - 1.0).abs() < 1e-12, "f64 mean = {mean}");
    }

    #[test]
    fn test_build_f64_explicit_es_preserves_target() {
        let target = 9.0_f64;
        let spec: ModemSpec<f64> = ModemSpecBuilder::<f64>::new()
            .bits_per_symbol(1)
            .points(vec![
                SymbolPoint::new(0.3_f64, 0.0),
                SymbolPoint::new(-0.3_f64, 0.0),
            ])
            .labels(vec![LabelWord::new(0, 1), LabelWord::new(1, 1)])
            .normalization(Normalization::ExplicitEs(target))
            .build();
        let mean: f64 = spec
            .view()
            .points()
            .iter()
            .map(|p| p.i * p.i + p.q * p.q)
            .sum::<f64>()
            / spec.num_symbols() as f64;
        assert!((mean - target).abs() < 1e-10, "f64 mean = {mean}");
    }

    #[test]
    fn test_build_default_bit_channels_are_opaque() {
        let spec = axis_4_point_builder().build();
        let bc = spec.view().bit_channels();
        assert_eq!(bc.len(), 2);
        assert_eq!(bc[0], BitChannelSemantics::Opaque(0));
        assert_eq!(bc[1], BitChannelSemantics::Opaque(1));
    }

    #[test]
    fn test_build_default_capabilities_are_both_true() {
        let spec = axis_4_point_builder().build();
        let caps = spec.capabilities();
        assert!(caps.supports_exact_log_map);
        assert!(caps.supports_max_log);
    }

    #[test]
    fn test_build_explicit_capabilities_override_default() {
        let spec = axis_4_point_builder()
            .capabilities(ModemCapabilities {
                supports_exact_log_map: true,
                supports_max_log: false,
                analysis: &[],
            })
            .build();
        assert!(spec.capabilities().supports_exact_log_map);
        assert!(!spec.capabilities().supports_max_log);
    }

    #[test]
    fn test_build_explicit_bit_channels_override_default() {
        let spec = axis_4_point_builder()
            .bit_channels(vec![
                BitChannelSemantics::IAxisPam(0),
                BitChannelSemantics::QAxisPam(0),
            ])
            .build();
        assert_eq!(spec.view().bit_channel(0), BitChannelSemantics::IAxisPam(0));
        assert_eq!(spec.view().bit_channel(1), BitChannelSemantics::QAxisPam(0));
    }

    /// Trivial demapper used only to exercise the `BatchSoftDemapper::spec`
    /// surface with a builder-built spec. Does no work.
    struct DummyDemapper {
        spec: ModemSpec<f32>,
    }

    impl BatchSoftDemapper<f32> for DummyDemapper {
        fn spec(&self) -> super::super::ModemView<'_, f32> {
            self.spec.view()
        }
        fn demap_llrs(
            &self,
            _input: super::super::DemapInput<'_, f32>,
            _out_llrs: &mut [crate::llr::Llr],
        ) {
            // no-op
        }
    }

    #[test]
    fn test_build_integrates_with_batch_soft_demapper_spec() {
        let spec = axis_4_point_builder().build();
        let d = DummyDemapper { spec };
        let v = d.spec();
        assert_eq!(v.num_symbols(), 4);
        assert_eq!(v.bits_per_symbol(), 2);
    }

    #[test]
    #[should_panic(expected = "bits_per_symbol not set")]
    fn test_build_panics_missing_bits_per_symbol() {
        let _ = ModemSpecBuilder::<f32>::new()
            .points(vec![SymbolPoint::new(1.0, 0.0)])
            .labels(vec![LabelWord::new(0, 1)])
            .build();
    }

    #[test]
    #[should_panic(expected = "points not set")]
    fn test_build_panics_missing_points() {
        let _ = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(1)
            .labels(vec![LabelWord::new(0, 1), LabelWord::new(1, 1)])
            .build();
    }

    #[test]
    #[should_panic(expected = "labels not set")]
    fn test_build_panics_missing_labels() {
        let _ = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(1)
            .points(vec![
                SymbolPoint::new(1.0, 0.0),
                SymbolPoint::new(-1.0, 0.0),
            ])
            .build();
    }

    #[test]
    #[should_panic(expected = "raw constellation has zero energy")]
    fn test_build_panics_on_zero_energy() {
        let _ = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(1)
            .points(vec![SymbolPoint::new(0.0, 0.0), SymbolPoint::new(0.0, 0.0)])
            .labels(vec![LabelWord::new(0, 1), LabelWord::new(1, 1)])
            .build();
    }

    #[test]
    #[should_panic(expected = "ExplicitEs target must be strictly positive")]
    fn test_build_panics_on_nonpositive_explicit_es() {
        let _ = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(1)
            .points(vec![
                SymbolPoint::new(1.0, 0.0),
                SymbolPoint::new(-1.0, 0.0),
            ])
            .labels(vec![LabelWord::new(0, 1), LabelWord::new(1, 1)])
            .normalization(Normalization::ExplicitEs(0.0))
            .build();
    }

    #[test]
    #[should_panic(expected = "points/labels length mismatch")]
    fn test_build_delegates_length_mismatch_panic() {
        // 2 points, 1 label: downstream from_parts_checked owns the
        // diagnosis; the builder does not duplicate the check.
        let _ = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(1)
            .points(vec![
                SymbolPoint::new(1.0, 0.0),
                SymbolPoint::new(-1.0, 0.0),
            ])
            .labels(vec![LabelWord::new(0, 1)])
            .build();
    }

    #[test]
    #[should_panic(expected = "not a bijection")]
    fn test_build_delegates_bijection_panic() {
        let _ = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(1)
            .points(vec![
                SymbolPoint::new(1.0, 0.0),
                SymbolPoint::new(-1.0, 0.0),
            ])
            .labels(vec![LabelWord::new(0, 1), LabelWord::new(0, 1)])
            .build();
    }

    #[test]
    fn test_default_builder_matches_new() {
        let a: ModemSpecBuilder<f32> = ModemSpecBuilder::default();
        let b = ModemSpecBuilder::<f32>::new();
        assert!(matches!(
            a.normalization,
            Normalization::UnitAverageSymbolEnergy
        ));
        assert!(matches!(
            b.normalization,
            Normalization::UnitAverageSymbolEnergy
        ));
    }

    #[test]
    fn test_modem_capabilities_default() {
        let caps = ModemCapabilities::default();
        assert!(caps.supports_exact_log_map);
        assert!(caps.supports_max_log);
    }

    #[test]
    fn test_modem_spec_builder_entry_point() {
        // ModemSpec::builder() returns an empty ModemSpecBuilder<S>.
        let spec = ModemSpec::<f32>::builder()
            .bits_per_symbol(1)
            .points(vec![
                SymbolPoint::new(1.0, 0.0),
                SymbolPoint::new(-1.0, 0.0),
            ])
            .labels(vec![LabelWord::new(0, 1), LabelWord::new(1, 1)])
            .build();
        assert_eq!(spec.num_symbols(), 2);
    }

    // Property test: random bijection labels + random axis points always
    // build successfully and satisfy the unit-energy invariant.
    proptest! {
        #[test]
        fn prop_build_succeeds_for_random_bijection(
            m in 1u8..=4u8,
            seed in 0u64..10_000u64,
        ) {
            let n = 1usize << m;

            // Deterministic permutation via the shared SSOT modem test LCG.
            let perm = super::super::test_oracle::permutation(seed, n);

            // Points on the unit circle at distinct angles; energy is
            // always 1 so normalization is trivially well-defined.
            let points: Vec<SymbolPoint<f32>> = (0..n)
                .map(|k| {
                    let theta = (k as f32) * core::f32::consts::TAU / (n as f32);
                    SymbolPoint::new(theta.cos(), theta.sin())
                })
                .collect();
            let labels: Vec<LabelWord> = perm
                .iter()
                .map(|&b| LabelWord::new(b, m))
                .collect();

            let spec = ModemSpecBuilder::<f32>::new()
                .bits_per_symbol(m)
                .points(points)
                .labels(labels)
                .build();

            prop_assert_eq!(spec.num_symbols(), n);
            prop_assert_eq!(spec.bits_per_symbol(), m);
            // Default caps: both true.
            prop_assert!(spec.capabilities().supports_exact_log_map);
            prop_assert!(spec.capabilities().supports_max_log);
        }
    }
}
