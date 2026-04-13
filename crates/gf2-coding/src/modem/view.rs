//! Borrowed read-only view over a [`super::ModemSpec`].
//!
//! Exposes both contiguous slices (for SIMD / GPU backends) and per-item
//! accessors (for analysis, examples, and the exact log-MAP reference
//! path). `Copy` so it can be passed through backend boundaries
//! without lifetime plumbing.

use super::scalar::ModemScalar;
use super::types::{
    BitChannelAnalysis, BitChannelId, BitChannelSemantics, LabelWord, ModemCapabilities,
    Normalization, SymbolPoint,
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

    /// Returns the per-bit-channel analytic metadata for bit position
    /// `bit_idx`.
    ///
    /// Borrowed from the [`ModemCapabilities::analysis`] slice attached
    /// to the underlying [`super::ModemSpec`]. Consumers use the flags
    /// to specialize analysis paths (closed-form LLR, symmetric
    /// distribution assumptions, BICM independence shortcuts).
    ///
    /// # Arguments
    ///
    /// * `bit_idx` - Bit index within a symbol, `0` is the MSB.
    ///
    /// # Panics
    ///
    /// Panics if `bit_idx >= bits_per_symbol()` or if the attached
    /// capabilities do not carry a populated analysis slice (length
    /// different from `bits_per_symbol()`). Preset- and builder-built
    /// specs always populate the slice; the latter case can only occur
    /// when a caller manually constructs [`ModemCapabilities`] via its
    /// [`Default`] impl (which leaves `analysis` empty).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::ModemSpec;
    ///
    /// // BPSK places one bit per axis, so every analytic flag holds.
    /// let spec = ModemSpec::<f32>::bpsk();
    /// let a = spec.view().bit_channel_analysis(0);
    /// assert!(a.symmetric_llr_distribution);
    /// assert!(a.conditionally_independent);
    /// assert!(a.closed_form_llr_available);
    ///
    /// // 16-QAM carries two PAM bits per axis; those are symmetric and
    /// // closed-form but NOT conditionally independent given the received
    /// // sample, so the flag is advertised as `false`.
    /// let spec16 = ModemSpec::<f32>::gray_square_qam(16);
    /// assert!(!spec16.view().bit_channel_analysis(0).conditionally_independent);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn bit_channel_analysis(&self, bit_idx: u8) -> &'static BitChannelAnalysis {
        assert!(
            bit_idx < self.bits_per_symbol,
            "ModemView::bit_channel_analysis: bit_idx {bit_idx} out of range [0, {})",
            self.bits_per_symbol
        );
        let analysis = self.capabilities.analysis;
        assert!(
            analysis.len() == self.bits_per_symbol as usize,
            "ModemView::bit_channel_analysis: capabilities.analysis length {} does not match bits_per_symbol {}",
            analysis.len(),
            self.bits_per_symbol
        );
        &analysis[bit_idx as usize]
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

    #[test]
    fn test_view_bit_channel_analysis_bpsk_roundtrip() {
        let spec = ModemSpec::bpsk();
        let v = spec.view();
        let caps = v.capabilities();
        assert_eq!(caps.analysis.len(), v.bits_per_symbol() as usize);
        for k in 0..v.bits_per_symbol() {
            let a = v.bit_channel_analysis(k);
            assert_eq!(a, &caps.analysis[k as usize]);
            assert!(a.symmetric_llr_distribution);
            assert!(a.conditionally_independent);
            assert!(a.closed_form_llr_available);
        }
    }

    #[test]
    fn test_view_bit_channel_analysis_qpsk_roundtrip() {
        let spec = ModemSpec::<f32>::gray_square_qam(4);
        let v = spec.view();
        let caps = v.capabilities();
        assert_eq!(caps.analysis.len(), 2);
        for k in 0..v.bits_per_symbol() {
            let a = v.bit_channel_analysis(k);
            assert_eq!(a, &caps.analysis[k as usize]);
            assert!(a.symmetric_llr_distribution);
            assert!(a.conditionally_independent);
            assert!(a.closed_form_llr_available);
        }
    }

    #[test]
    fn test_view_bit_channel_analysis_gray_qam_higher_order_roundtrip() {
        // For 16/64/256-QAM the per-axis multi-bit presets advertise
        // `conditionally_independent = false`; every entry must match the
        // slice returned from capabilities() on each preset order.
        for order in [16usize, 64, 256] {
            let spec = ModemSpec::<f32>::gray_square_qam(order);
            let v = spec.view();
            let caps = v.capabilities();
            assert_eq!(
                caps.analysis.len(),
                v.bits_per_symbol() as usize,
                "analysis len mismatch for order {order}"
            );
            for k in 0..v.bits_per_symbol() {
                let a = v.bit_channel_analysis(k);
                assert_eq!(
                    a, &caps.analysis[k as usize],
                    "order {order} bit {k} mismatch between view accessor and capabilities slice"
                );
                assert!(a.symmetric_llr_distribution);
                assert!(!a.conditionally_independent);
                assert!(a.closed_form_llr_available);
            }
        }
    }

    #[test]
    #[should_panic(expected = "bit_idx 4 out of range [0, 4)")]
    fn test_view_bit_channel_analysis_out_of_range_panics() {
        let spec = ModemSpec::gray_square_qam(16);
        let _ = spec.view().bit_channel_analysis(4);
    }

    #[test]
    fn test_reference_surfaces_expose_matching_analysis() {
        // The reference mapper and reference soft demapper both carry a
        // ModemSpec; the analysis metadata surfaced via their
        // `ModemView` must match the spec's own capabilities entry for
        // each preset. Downstream analysis tools rely on this equality
        // to avoid re-deriving hints from the constellation geometry.
        use crate::modem::{
            BatchMapper, BatchSoftDemapper, ReferenceMapper, ReferenceSoftDemapper,
        };
        for order in [2usize, 4, 16, 64, 256] {
            let spec = if order == 2 {
                ModemSpec::<f32>::bpsk()
            } else {
                ModemSpec::<f32>::gray_square_qam(order)
            };
            let ref_mapper = ReferenceMapper::new(spec.clone());
            let ref_demapper = ReferenceSoftDemapper::new(spec.clone());
            let spec_caps = spec.view().capabilities();
            let mapper_caps = ref_mapper.spec().capabilities();
            let demapper_caps = ref_demapper.spec().capabilities();
            assert_eq!(mapper_caps.analysis, spec_caps.analysis, "order {order}");
            assert_eq!(demapper_caps.analysis, spec_caps.analysis, "order {order}");
        }
    }
}
