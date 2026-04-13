//! Value types used by the modem framework data model.
//!
//! These types are the fixed vocabulary consumed by every downstream modem
//! task: point geometry, bit labels, bit-channel identity and semantics,
//! normalization contract, demapper method, and capability advertisement.
//! See `dev/active/c87c5043-constellation-data-model-plan.md` §4 for the
//! locked surface.

use super::scalar::ModemScalar;

/// An I/Q constellation point.
///
/// The coordinate scalar is generic over [`ModemScalar`]; presets default
/// to [`super::DefaultScalar`] (`f32`).
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::SymbolPoint;
///
/// let p = SymbolPoint::<f32>::new(1.0, -2.0);
/// assert_eq!(p.i, 1.0);
/// assert_eq!(p.q, -2.0);
/// assert!((p.energy() - 5.0).abs() < 1e-6);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SymbolPoint<S: ModemScalar> {
    /// In-phase coordinate.
    pub i: S,
    /// Quadrature coordinate.
    pub q: S,
}

impl<S: ModemScalar> SymbolPoint<S> {
    /// Constructs a [`SymbolPoint`] from explicit coordinates.
    ///
    /// # Arguments
    ///
    /// * `i` - In-phase coordinate.
    /// * `q` - Quadrature coordinate.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::SymbolPoint;
    ///
    /// let p = SymbolPoint::<f64>::new(0.5, -0.5);
    /// assert_eq!(p.i, 0.5);
    /// assert_eq!(p.q, -0.5);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn new(i: S, q: S) -> Self {
        Self { i, q }
    }

    /// Returns the squared radius `i*i + q*q` (the symbol energy).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::SymbolPoint;
    ///
    /// let p = SymbolPoint::<f32>::new(3.0, 4.0);
    /// assert!((p.energy() - 25.0).abs() < 1e-5);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn energy(self) -> S {
        self.i * self.i + self.q * self.q
    }
}

/// Bit label for a single constellation symbol.
///
/// Bit `k` of the label corresponds to LLR position `k` within the symbol
/// under the MSB-first intra-symbol ordering: `k = 0` is the most
/// significant bit of the `width`-bit label. See the plan §4.2 for the
/// locked bit ordering rationale.
///
/// `width` is the number of meaningful MSBs used. `bits` must fit in
/// `width` bits (i.e. `bits >> width == 0`).
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::LabelWord;
///
/// let l = LabelWord::new(0b10, 2);
/// assert_eq!(l.bits, 0b10);
/// assert_eq!(l.width, 2);
/// assert!(l.bit(0));  // MSB
/// assert!(!l.bit(1)); // LSB
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LabelWord {
    /// Raw label value; only the low `width` bits are meaningful.
    pub bits: u16,
    /// Number of meaningful bits in `bits`, in `[1, 16]`.
    pub width: u8,
}

impl LabelWord {
    /// Constructs a [`LabelWord`].
    ///
    /// # Arguments
    ///
    /// * `bits` - Raw label bits; only the low `width` bits are meaningful.
    /// * `width` - Number of meaningful bits, in `[1, 16]`.
    ///
    /// # Panics
    ///
    /// Panics if `width == 0`, `width > 16`, or `bits` does not fit in
    /// `width` bits.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::LabelWord;
    ///
    /// let l = LabelWord::new(0b101, 3);
    /// assert_eq!(l.bits, 5);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn new(bits: u16, width: u8) -> Self {
        assert!(
            (1..=16).contains(&width),
            "LabelWord width must be in [1, 16], got {width}"
        );
        if width < 16 {
            assert!(
                bits >> width == 0,
                "LabelWord bits {bits:#x} do not fit in width {width}"
            );
        }
        Self { bits, width }
    }

    /// Returns bit `k` of the label, with `k = 0` being the MSB.
    ///
    /// # Arguments
    ///
    /// * `k` - Bit index within the label; `0` is the MSB.
    ///
    /// # Panics
    ///
    /// Panics if `k >= self.width`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::LabelWord;
    ///
    /// let l = LabelWord::new(0b1001, 4);
    /// assert!(l.bit(0));    // MSB of 0b1001
    /// assert!(!l.bit(1));
    /// assert!(!l.bit(2));
    /// assert!(l.bit(3));    // LSB
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn bit(self, k: u8) -> bool {
        assert!(
            k < self.width,
            "LabelWord::bit index {k} out of range for width {}",
            self.width
        );
        let shift = self.width - 1 - k;
        (self.bits >> shift) & 1 == 1
    }
}

/// Identifier for a bit position within a symbol.
///
/// `bit_index = 0` is the MSB under the canonical intra-symbol ordering.
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::BitChannelId;
///
/// let id = BitChannelId { bit_index: 2 };
/// assert_eq!(id.bit_index, 2);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BitChannelId {
    /// Bit position within a symbol; `0` is the MSB.
    pub bit_index: u8,
}

/// Semantic role of a bit position within a symbol.
///
/// Set by presets and builders. Analysis and documentation consume this;
/// hot demap loops never read it.
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::BitChannelSemantics;
///
/// let sem = BitChannelSemantics::IAxisPam(0);
/// assert_eq!(sem, BitChannelSemantics::IAxisPam(0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BitChannelSemantics {
    /// Arbitrary constellation; the payload is the bit-position index only.
    Opaque(u8),
    /// Single-axis PAM (e.g. BPSK) with PAM-bit index payload.
    SingleAxisPam(u8),
    /// In-phase axis PAM bit of a Gray square-QAM, with PAM-bit index
    /// (`0` is the most significant PAM bit / coarsest level).
    IAxisPam(u8),
    /// Quadrature axis PAM bit of a Gray square-QAM, with PAM-bit index
    /// (`0` is the most significant PAM bit / coarsest level).
    QAxisPam(u8),
}

/// Normalization contract for a [`super::ModemSpec`].
///
/// Points are stored post-normalized; the retained variant records what
/// the caller requested.
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::Normalization;
///
/// let n = Normalization::<f32>::UnitAverageSymbolEnergy;
/// assert_eq!(n, Normalization::UnitAverageSymbolEnergy);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Normalization<S: ModemScalar> {
    /// Scale so that the average `i^2 + q^2` over all symbols equals 1.
    UnitAverageSymbolEnergy,
    /// Scale so that the average symbol energy equals the provided `Es`.
    ExplicitEs(S),
}

/// Selectable demapper semantics.
///
/// Trait task `d36ae697` consumes this; it is not redefined there.
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::DemapMethod;
///
/// assert_ne!(DemapMethod::ExactLogMap, DemapMethod::MaxLog);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DemapMethod {
    /// Exact log-MAP (reference / analysis) path.
    ExactLogMap,
    /// Max-log approximation.
    MaxLog,
}

/// Per-bit-channel analytic metadata consumed by downstream analysis.
///
/// Advertised by [`ModemCapabilities::analysis`] with one entry per bit
/// position (length `bits_per_symbol()`). Analysis, documentation, and
/// test-vector generators consume this; hot demap loops never read it.
///
/// Each flag describes a property of the bit-channel LLR under AWGN with
/// the normalization contract documented at the [`super::modem`] module
/// level.
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::BitChannelAnalysis;
///
/// let a = BitChannelAnalysis {
///     symmetric_llr_distribution: true,
///     conditionally_independent: true,
///     closed_form_llr_available: true,
/// };
/// assert!(a.symmetric_llr_distribution);
/// assert!(a.conditionally_independent);
/// assert!(a.closed_form_llr_available);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BitChannelAnalysis {
    /// LLR conditional distribution is symmetric about 0 under
    /// equiprobable input bits.
    pub symmetric_llr_distribution: bool,
    /// This bit is conditionally independent of the other bits in the
    /// same symbol given the received sample. Holds for Gray-coded
    /// square QAM under AWGN because I and Q decouple and the two axes
    /// separate by PAM.
    pub conditionally_independent: bool,
    /// A closed-form analytic LLR expression exists for this bit
    /// channel (for example BPSK / QPSK `LLR = 4 y / N0`, or the
    /// piecewise-linear exact log-MAP for small Gray-PAM axes).
    pub closed_form_llr_available: bool,
}

/// Which demap methods a given [`super::ModemSpec`] supports, plus per
/// bit-channel analytic metadata.
///
/// Builders populate this; the trait layer reads the capability flags to
/// reject method/spec mismatches at call time, and analysis consumers
/// read [`ModemCapabilities::analysis`] for per-bit properties.
///
/// # Invariants
///
/// - `analysis.len() == bits_per_symbol()` — one entry per bit position,
///   indexed MSB-first to match [`super::BitChannelSemantics`] and
///   [`super::BitChannelId`].
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::{BitChannelAnalysis, ModemCapabilities};
///
/// const A: &[BitChannelAnalysis] = &[BitChannelAnalysis {
///     symmetric_llr_distribution: true,
///     conditionally_independent: true,
///     closed_form_llr_available: true,
/// }];
/// let caps = ModemCapabilities {
///     supports_exact_log_map: true,
///     supports_max_log: true,
///     analysis: A,
/// };
/// assert!(caps.supports_exact_log_map);
/// assert_eq!(caps.analysis.len(), 1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModemCapabilities {
    /// Whether the spec supports the exact log-MAP demapper path.
    pub supports_exact_log_map: bool,
    /// Whether the spec supports the max-log demapper path.
    pub supports_max_log: bool,
    /// Per-bit-channel analytic metadata. Length equals
    /// `bits_per_symbol()`; entry `k` applies to bit position `k`
    /// (MSB-first within a symbol). `&'static` so presets ship as
    /// compile-time constants.
    pub analysis: &'static [BitChannelAnalysis],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_label_word_new_accepts_within_width() {
        let l = LabelWord::new(0b1011, 4);
        assert_eq!(l.bits, 0b1011);
        assert_eq!(l.width, 4);
    }

    #[test]
    #[should_panic(expected = "LabelWord width must be in [1, 16]")]
    fn test_label_word_new_panics_on_zero_width() {
        let _ = LabelWord::new(0, 0);
    }

    #[test]
    #[should_panic(expected = "LabelWord width must be in [1, 16]")]
    fn test_label_word_new_panics_on_overwide() {
        let _ = LabelWord::new(0, 17);
    }

    #[test]
    #[should_panic(expected = "do not fit in width")]
    fn test_label_word_new_panics_on_overflow() {
        let _ = LabelWord::new(0b1_0000, 4);
    }

    #[test]
    fn test_label_word_bit_msb_first() {
        let l = LabelWord::new(0b1010, 4);
        assert!(l.bit(0));
        assert!(!l.bit(1));
        assert!(l.bit(2));
        assert!(!l.bit(3));
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_label_word_bit_panics_on_out_of_range() {
        let l = LabelWord::new(0b1, 1);
        let _ = l.bit(1);
    }

    #[test]
    fn test_bit_channel_semantics_variants() {
        assert_ne!(
            BitChannelSemantics::IAxisPam(0),
            BitChannelSemantics::QAxisPam(0)
        );
        assert_eq!(
            BitChannelSemantics::SingleAxisPam(0),
            BitChannelSemantics::SingleAxisPam(0)
        );
        assert_ne!(
            BitChannelSemantics::Opaque(0),
            BitChannelSemantics::Opaque(1)
        );
    }

    #[test]
    fn test_symbol_point_energy_f32() {
        let p = SymbolPoint::<f32>::new(3.0, 4.0);
        assert!((p.energy() - 25.0).abs() < 1e-5);
    }

    #[test]
    fn test_symbol_point_energy_f64() {
        let p = SymbolPoint::<f64>::new(3.0, 4.0);
        assert!((p.energy() - 25.0).abs() < 1e-12);
    }

    #[test]
    fn test_bit_channel_analysis_construct_and_fields() {
        let a = BitChannelAnalysis {
            symmetric_llr_distribution: true,
            conditionally_independent: false,
            closed_form_llr_available: true,
        };
        assert!(a.symmetric_llr_distribution);
        assert!(!a.conditionally_independent);
        assert!(a.closed_form_llr_available);
        // Derive checks: Copy + Eq + Hash available.
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn test_label_word_full_width_16() {
        // width=16 path: no bits>>16 check because shift would be UB.
        let l = LabelWord::new(u16::MAX, 16);
        assert!(l.bit(0));
        assert!(l.bit(15));
    }
}
