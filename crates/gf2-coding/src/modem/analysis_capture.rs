//! Capture handle that feeds a simulation runner's LLR batches and truth
//! bits into a caller-owned [`PerBitLlrStats`].

use super::analysis::PerBitLlrStats;
use crate::llr::Llr;
use gf2_core::BitVec;

/// Handle that feeds demapped LLR batches and their ground-truth bits into
/// a caller-owned [`PerBitLlrStats`], tagged with the
/// [`super::DemapMethod`] that produced the LLRs.
#[derive(Debug)]
pub struct AnalysisCapture<'a> {
    stats: &'a mut PerBitLlrStats,
    demap_method: super::DemapMethod,
}

impl<'a> AnalysisCapture<'a> {
    /// Wraps `stats` in a capture tagged with the [`super::DemapMethod`]
    /// that produces the LLRs.
    ///
    /// The simulation runner asserts that the tag equals the channel's
    /// [`crate::simulation::ChannelModel::demap_method`], because per-bit MI
    /// and GMI estimates differ between exact log-MAP and max-log.
    #[inline]
    pub fn with_method(stats: &'a mut PerBitLlrStats, demap_method: super::DemapMethod) -> Self {
        Self {
            stats,
            demap_method,
        }
    }

    /// Wraps `stats` in a capture tagged with [`super::DemapMethod::MaxLog`].
    #[inline]
    pub fn new(stats: &'a mut PerBitLlrStats) -> Self {
        Self::with_method(stats, super::DemapMethod::MaxLog)
    }

    /// Returns the [`super::DemapMethod`] this capture is tagged with.
    #[inline]
    pub fn demap_method(&self) -> super::DemapMethod {
        self.demap_method
    }

    /// Returns the accumulator's `bits_per_symbol`.
    #[inline]
    pub fn bits_per_symbol(&self) -> u8 {
        self.stats.bits_per_symbol()
    }

    /// Immutable borrow of the underlying accumulator.
    #[inline]
    pub fn stats(&self) -> &PerBitLlrStats {
        self.stats
    }

    /// Feeds a batch of LLRs and matching truth bits into the accumulator,
    /// in the layout of [`PerBitLlrStats::accumulate`].
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len() != truth_bits.len()`, if `llrs.len()` is not a
    /// multiple of `bits_per_symbol()`, or if the accumulator is stamped
    /// with a different demap method.
    #[inline]
    pub fn accumulate_slice(&mut self, llrs: &[Llr], truth_bits: &[bool]) {
        self.stats.set_demap_method_once(self.demap_method);
        self.stats.accumulate(llrs, truth_bits);
    }

    /// [`Self::accumulate_slice`] for truth bits held in a [`BitVec`];
    /// allocates a `Vec<bool>` of `llrs.len()` entries.
    ///
    /// # Panics
    ///
    /// Same conditions as [`Self::accumulate_slice`].
    #[inline]
    pub fn accumulate_bitvec(&mut self, llrs: &[Llr], truth_bits: &BitVec) {
        assert_eq!(
            llrs.len(),
            truth_bits.len(),
            "AnalysisCapture::accumulate_bitvec: llrs.len ({}) != truth_bits.len ({})",
            llrs.len(),
            truth_bits.len(),
        );
        self.stats.set_demap_method_once(self.demap_method);
        let truth: Vec<bool> = (0..truth_bits.len()).map(|i| truth_bits.get(i)).collect();
        self.stats.accumulate(llrs, &truth);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modem::analysis::PerBitLlrStats;

    #[test]
    fn test_new_borrows_accumulator() {
        let mut stats = PerBitLlrStats::new(4);
        let capture = AnalysisCapture::new(&mut stats);
        assert_eq!(capture.bits_per_symbol(), 4);
    }

    #[test]
    fn test_accumulate_slice_forwards_to_stats() {
        let mut stats = PerBitLlrStats::new(2);
        {
            let mut capture = AnalysisCapture::new(&mut stats);
            capture.accumulate_slice(
                &[Llr::new(2.0), Llr::new(-2.0), Llr::new(3.0), Llr::new(-3.0)],
                &[false, true, false, true],
            );
        }
        let r = stats.report();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].bit0.count() + r[0].bit1.count(), 2);
        assert_eq!(r[1].bit0.count() + r[1].bit1.count(), 2);
    }

    #[test]
    fn test_accumulate_bitvec_matches_slice_form() {
        let mut stats_slice = PerBitLlrStats::new(2);
        let mut stats_bv = PerBitLlrStats::new(2);
        let llrs = [Llr::new(1.0), Llr::new(-2.0), Llr::new(3.0), Llr::new(-4.0)];
        let truth_bools = [false, true, false, true];
        AnalysisCapture::new(&mut stats_slice).accumulate_slice(&llrs, &truth_bools);

        let mut bv = BitVec::zeros(4);
        for (i, &b) in truth_bools.iter().enumerate() {
            if b {
                bv.set(i, true);
            }
        }
        AnalysisCapture::new(&mut stats_bv).accumulate_bitvec(&llrs, &bv);

        let a = stats_slice.report();
        let b = stats_bv.report();
        assert_eq!(a.len(), b.len());
        for (ra, rb) in a.iter().zip(b.iter()) {
            assert_eq!(ra.bit0.count(), rb.bit0.count());
            assert_eq!(ra.bit1.count(), rb.bit1.count());
            assert!((ra.bit0.mean() - rb.bit0.mean()).abs() < 1e-12);
            assert!((ra.bit1.mean() - rb.bit1.mean()).abs() < 1e-12);
        }
    }

    #[test]
    fn test_stats_view_returns_same_bits_per_symbol() {
        let mut stats = PerBitLlrStats::new(6);
        let capture = AnalysisCapture::new(&mut stats);
        assert_eq!(capture.stats().bits_per_symbol(), 6);
    }
}
