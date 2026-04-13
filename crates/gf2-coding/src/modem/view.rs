//! Borrowed read-only view over a [`super::ModemSpec`].
//!
//! Exposes both contiguous slices (for SIMD / GPU backends) and per-item
//! accessors (for analysis, examples, and the exact log-MAP reference
//! path). `Copy` so it can be passed through backend boundaries
//! without lifetime plumbing.

use super::scalar::ModemScalar;
use super::types::{
    BitChannelId, BitChannelSemantics, LabelWord, ModemCapabilities, Normalization, SymbolPoint,
};

/// Borrowed read-only view over a [`super::ModemSpec`].
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::ModemSpec;
///
/// let spec = ModemSpec::bpsk();
/// let view = spec.view();
/// assert_eq!(view.points().len(), 2);
/// assert_eq!(view.labels().len(), 2);
/// assert_eq!(view.num_symbols(), 2);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ModemView<'a, S: ModemScalar> {
    points: &'a [SymbolPoint<S>],
    labels: &'a [LabelWord],
    bit_channels: &'a [BitChannelSemantics],
    bits_per_symbol: u8,
    normalization: Normalization<S>,
    normalization_scale: S,
    capabilities: ModemCapabilities,
}

impl<'a, S: ModemScalar> ModemView<'a, S> {
    /// Crate-internal constructor used by [`super::ModemSpec::view`].
    #[inline]
    pub(super) fn new(
        points: &'a [SymbolPoint<S>],
        labels: &'a [LabelWord],
        bit_channels: &'a [BitChannelSemantics],
        bits_per_symbol: u8,
        normalization: Normalization<S>,
        normalization_scale: S,
        capabilities: ModemCapabilities,
    ) -> Self {
        Self {
            points,
            labels,
            bit_channels,
            bits_per_symbol,
            normalization,
            normalization_scale,
            capabilities,
        }
    }

    /// Returns the contiguous slice of constellation points.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::ModemSpec;
    ///
    /// let spec = ModemSpec::bpsk();
    /// assert_eq!(spec.view().points().len(), 2);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn points(&self) -> &'a [SymbolPoint<S>] {
        self.points
    }

    /// Returns the contiguous slice of labels, parallel to `points()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::ModemSpec;
    ///
    /// let spec = ModemSpec::bpsk();
    /// assert_eq!(spec.view().labels().len(), 2);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn labels(&self) -> &'a [LabelWord] {
        self.labels
    }

    /// Returns the per-bit semantic tags, one entry per bit position.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{BitChannelSemantics, ModemSpec};
    ///
    /// let spec = ModemSpec::bpsk();
    /// assert_eq!(spec.view().bit_channels().len(), 1);
    /// assert_eq!(spec.view().bit_channels()[0], BitChannelSemantics::SingleAxisPam(0));
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn bit_channels(&self) -> &'a [BitChannelSemantics] {
        self.bit_channels
    }

    /// Returns the constellation point at index `idx`.
    ///
    /// # Arguments
    ///
    /// * `idx` - Point index in `0..num_symbols()`.
    ///
    /// # Panics
    ///
    /// Panics if `idx >= self.num_symbols()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::ModemSpec;
    ///
    /// let spec = ModemSpec::bpsk();
    /// let p = spec.view().point(0);
    /// assert!(p.i.abs() > 0.0);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn point(&self, idx: usize) -> SymbolPoint<S> {
        self.points[idx]
    }

    /// Returns the label at index `idx`.
    ///
    /// # Arguments
    ///
    /// * `idx` - Label index in `0..num_symbols()`.
    ///
    /// # Panics
    ///
    /// Panics if `idx >= self.num_symbols()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::ModemSpec;
    ///
    /// let spec = ModemSpec::bpsk();
    /// let l = spec.view().label(0);
    /// assert_eq!(l.width, 1);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn label(&self, idx: usize) -> LabelWord {
        self.labels[idx]
    }

    /// Returns the bit-channel semantic tag for bit position `bit_idx`.
    ///
    /// # Arguments
    ///
    /// * `bit_idx` - Bit index within a symbol, `0` is the MSB.
    ///
    /// # Panics
    ///
    /// Panics if `bit_idx >= self.bits_per_symbol()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{BitChannelSemantics, ModemSpec};
    ///
    /// let spec = ModemSpec::gray_square_qam(4);
    /// assert_eq!(spec.view().bit_channel(0), BitChannelSemantics::IAxisPam(0));
    /// assert_eq!(spec.view().bit_channel(1), BitChannelSemantics::QAxisPam(0));
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn bit_channel(&self, bit_idx: u8) -> BitChannelSemantics {
        self.bit_channels[bit_idx as usize]
    }

    /// Returns the canonical [`BitChannelId`] for a bit position.
    ///
    /// # Arguments
    ///
    /// * `bit_idx` - Bit index within a symbol, `0` is the MSB.
    ///
    /// # Panics
    ///
    /// Panics if `bit_idx >= bits_per_symbol()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{BitChannelId, ModemSpec};
    ///
    /// let spec = ModemSpec::gray_square_qam(16);
    /// assert_eq!(spec.view().bit_channel_id(2), BitChannelId { bit_index: 2 });
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn bit_channel_id(&self, bit_idx: u8) -> BitChannelId {
        assert!(
            bit_idx < self.bits_per_symbol,
            "ModemView::bit_channel_id: bit_idx {bit_idx} out of range [0, {})",
            self.bits_per_symbol
        );
        BitChannelId { bit_index: bit_idx }
    }

    /// Number of constellation symbols.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::ModemSpec;
    ///
    /// assert_eq!(ModemSpec::gray_square_qam(16).view().num_symbols(), 16);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn num_symbols(&self) -> usize {
        self.points.len()
    }

    /// Number of bits per symbol.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::ModemSpec;
    ///
    /// assert_eq!(ModemSpec::bpsk().view().bits_per_symbol(), 1);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn bits_per_symbol(&self) -> u8 {
        self.bits_per_symbol
    }

    /// Normalization contract requested at construction.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{ModemSpec, Normalization};
    ///
    /// let spec = ModemSpec::bpsk();
    /// matches!(spec.view().normalization(), Normalization::UnitAverageSymbolEnergy);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn normalization(&self) -> Normalization<S> {
        self.normalization
    }

    /// Normalization scale factor applied to the raw integer grid.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::ModemSpec;
    ///
    /// assert!(ModemSpec::gray_square_qam(16).view().normalization_scale() > 0.0);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn normalization_scale(&self) -> S {
        self.normalization_scale
    }

    /// Demap-method capabilities advertised by the spec.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::ModemSpec;
    ///
    /// assert!(ModemSpec::bpsk().view().capabilities().supports_exact_log_map);
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

#[cfg(test)]
mod tests {
    use super::super::ModemSpec;
    use super::*;

    #[test]
    fn test_view_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<ModemView<'_, f32>>();
    }

    #[test]
    fn test_view_slice_and_per_item_match() {
        let spec = ModemSpec::gray_square_qam(16);
        let v = spec.view();
        for i in 0..v.num_symbols() {
            assert_eq!(v.point(i), v.points()[i]);
            assert_eq!(v.label(i), v.labels()[i]);
        }
        for k in 0..v.bits_per_symbol() {
            assert_eq!(v.bit_channel(k), v.bit_channels()[k as usize]);
            assert_eq!(v.bit_channel_id(k), BitChannelId { bit_index: k });
        }
    }

    #[test]
    #[should_panic(expected = "bit_idx 4 out of range [0, 4)")]
    fn test_view_bit_channel_id_out_of_range_panics() {
        let spec = ModemSpec::gray_square_qam(16);
        let _ = spec.view().bit_channel_id(4);
    }
}
