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

        // Invariant 9: per-bit-channel analysis length matches
        // bits_per_symbol. Length zero is also accepted for backward
        // compatibility with callers constructing ModemCapabilities via
        // its Default impl, which cannot know bits_per_symbol.
        assert!(
            capabilities.analysis.is_empty()
                || capabilities.analysis.len() == bits_per_symbol as usize,
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

// Default = f32 convenience alias, kept for readability in code that
// frequently uses the default scalar path.
#[allow(dead_code)]
pub(super) type DefaultSpec = ModemSpec<DefaultScalar>;

#[cfg(test)]
mod tests {
    use super::*;

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
                analysis: &[],
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
                analysis: &[],
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
}
