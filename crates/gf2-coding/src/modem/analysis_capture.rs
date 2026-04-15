//! Opt-in capture handle wiring [`PerBitLlrStats`] into simulation
//! runners.
//!
//! `AnalysisCapture` is a thin borrow over an already-constructed
//! [`PerBitLlrStats`] (owned by the caller). Simulation runners that
//! support analysis accept it wrapped in `Option<&mut AnalysisCapture<'_>>`:
//! when the option is `None`, no analysis-specific work — no allocations,
//! no extra bookkeeping, no branchy LLR copy — runs inside the hot loop.
//!
//! The capture exists purely to feed `(llrs, truth_bits)` batches into
//! [`PerBitLlrStats::accumulate`] from the correct place in the runner.
//! It does not reimplement any modem math and never produces a second
//! source of truth for demapper output: the runner already owns the
//! LLRs emitted by the channel's [`crate::modem::BatchSoftDemapper`],
//! and the true-bit sequence is the same `bits: &BitVec` the runner
//! uses to count bit errors.
//!
//! # Zero-overhead contract
//!
//! The analysis-enabled runner is gated behind a single `match`/`if let`
//! inside an `#[inline]` sink. When the caller passes `None`, the match
//! has a single arm and the hot-path bench
//! (`benches/simulation_no_analysis_overhead.rs`) measures throughput
//! within a fraction of a percent of the unaugmented
//! [`crate::simulation::SimulationRunner::run_uncoded_ber_with_channel`]
//! path.
//!
//! # Examples
//!
//! ```
//! use gf2_coding::modem::analysis::PerBitLlrStats;
//! use gf2_coding::modem::AnalysisCapture;
//!
//! let mut stats = PerBitLlrStats::new(4);
//! let mut capture = AnalysisCapture::new(&mut stats);
//! assert_eq!(capture.bits_per_symbol(), 4);
//! ```

use super::analysis::PerBitLlrStats;
use crate::llr::Llr;
use gf2_core::BitVec;

/// Opt-in handle that feeds demapped LLR batches and their ground-truth
/// bits into a caller-owned [`PerBitLlrStats`].
///
/// The capture is a borrow; constructing it does not allocate. A
/// simulation runner that accepts `Option<&mut AnalysisCapture<'_>>`
/// takes no analysis work when the option is `None`, so disabled-path
/// throughput matches the analysis-free runner.
///
/// # Field rationale
///
/// Only `&mut PerBitLlrStats` is kept. Histogram configuration, MI
/// estimation, and GMI derivation all live on the accumulator itself
/// (see [`PerBitLlrStats::with_histogram`],
/// [`crate::modem::analysis::per_bit_mi_histogram_bits`],
/// [`crate::modem::analysis::gmi_bits`]); this handle never duplicates
/// any of that surface.
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::analysis::PerBitLlrStats;
/// use gf2_coding::modem::AnalysisCapture;
///
/// let mut stats = PerBitLlrStats::new(2);
/// {
///     let mut capture = AnalysisCapture::new(&mut stats);
///     assert_eq!(capture.bits_per_symbol(), 2);
/// }
/// // After the capture is dropped, the accumulator is free to be
/// // queried or merged with another instance.
/// assert_eq!(stats.report().len(), 2);
/// ```
#[derive(Debug)]
pub struct AnalysisCapture<'a> {
    stats: &'a mut PerBitLlrStats,
    demap_method: super::DemapMethod,
}

impl<'a> AnalysisCapture<'a> {
    /// Wraps an existing [`PerBitLlrStats`] in a capture handle tagged
    /// with the [`super::DemapMethod`] that produced the LLRs being
    /// captured.
    ///
    /// The method stamp is load-bearing: per-bit MI / GMI estimates
    /// have different semantics under exact log-MAP vs. max-log (the
    /// module docs flag this at
    /// [`gf2_coding::modem::analysis`](super::analysis)), so the
    /// runner asserts that the capture's method matches the channel's
    /// [`crate::simulation::ChannelModel::demap_method`] before the
    /// first batch. Heterogeneous batches silently merged into one
    /// accumulator would produce un-interpretable statistics; this
    /// tag is the tripwire that prevents it.
    ///
    /// For legacy callers who do not distinguish the two methods, use
    /// [`AnalysisCapture::new`] which defaults to [`super::DemapMethod::MaxLog`].
    ///
    /// # Arguments
    ///
    /// * `stats` - Caller-owned accumulator.
    /// * `demap_method` - The demap method whose LLRs this capture will
    ///   accumulate. Must match the channel's
    ///   [`crate::simulation::ChannelModel::demap_method`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::analysis::PerBitLlrStats;
    /// use gf2_coding::modem::{AnalysisCapture, DemapMethod};
    ///
    /// let mut stats = PerBitLlrStats::new(4);
    /// let _capture = AnalysisCapture::with_method(&mut stats, DemapMethod::ExactLogMap);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1). No allocation.
    #[inline]
    pub fn with_method(stats: &'a mut PerBitLlrStats, demap_method: super::DemapMethod) -> Self {
        Self {
            stats,
            demap_method,
        }
    }

    /// Legacy constructor: builds a capture tagged with
    /// [`super::DemapMethod::MaxLog`] (the max-log variant). Most
    /// uncoded-BER workflows drive the runner with max-log LLRs, so
    /// this matches the common case. Use
    /// [`AnalysisCapture::with_method`] when you need exact log-MAP.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::analysis::PerBitLlrStats;
    /// use gf2_coding::modem::AnalysisCapture;
    ///
    /// let mut stats = PerBitLlrStats::new(4);
    /// let _capture = AnalysisCapture::new(&mut stats);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1). No allocation.
    #[inline]
    pub fn new(stats: &'a mut PerBitLlrStats) -> Self {
        Self::with_method(stats, super::DemapMethod::MaxLog)
    }

    /// Returns the [`super::DemapMethod`] this capture was tagged
    /// with at construction. Used by the runner to assert consistency
    /// with the channel's advertised demap method.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::analysis::PerBitLlrStats;
    /// use gf2_coding::modem::{AnalysisCapture, DemapMethod};
    ///
    /// let mut stats = PerBitLlrStats::new(1);
    /// let capture = AnalysisCapture::with_method(&mut stats, DemapMethod::MaxLog);
    /// assert_eq!(capture.demap_method(), DemapMethod::MaxLog);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn demap_method(&self) -> super::DemapMethod {
        self.demap_method
    }

    /// Returns the accumulator's `bits_per_symbol`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::analysis::PerBitLlrStats;
    /// use gf2_coding::modem::AnalysisCapture;
    ///
    /// let mut stats = PerBitLlrStats::new(6);
    /// let capture = AnalysisCapture::new(&mut stats);
    /// assert_eq!(capture.bits_per_symbol(), 6);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn bits_per_symbol(&self) -> u8 {
        self.stats.bits_per_symbol()
    }

    /// Immutable borrow of the underlying accumulator.
    ///
    /// Useful when the runner wants to inspect intermediate state mid-sweep
    /// without relinquishing the capture handle.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::analysis::PerBitLlrStats;
    /// use gf2_coding::modem::AnalysisCapture;
    ///
    /// let mut stats = PerBitLlrStats::new(2);
    /// let capture = AnalysisCapture::new(&mut stats);
    /// assert_eq!(capture.stats().bits_per_symbol(), 2);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn stats(&self) -> &PerBitLlrStats {
        self.stats
    }

    /// Feeds a batch of LLRs and matching truth bits into the
    /// accumulator.
    ///
    /// This is the single entry point the runner calls on the
    /// analysis-enabled path. The slices follow the canonical demapper
    /// layout (symbol-major, MSB-first within each symbol); see
    /// [`PerBitLlrStats::accumulate`] for the exact contract.
    ///
    /// # Arguments
    ///
    /// * `llrs` - Demapper output. Length must be a multiple of
    ///   [`AnalysisCapture::bits_per_symbol`].
    /// * `truth_bits` - Ground-truth transmitted bits (same length as
    ///   `llrs`).
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len() != truth_bits.len()` or if
    /// `llrs.len() % bits_per_symbol() != 0` (inherits
    /// [`PerBitLlrStats::accumulate`]'s contract).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::llr::Llr;
    /// use gf2_coding::modem::analysis::PerBitLlrStats;
    /// use gf2_coding::modem::AnalysisCapture;
    ///
    /// let mut stats = PerBitLlrStats::new(2);
    /// {
    ///     let mut capture = AnalysisCapture::new(&mut stats);
    ///     capture.accumulate_slice(
    ///         &[Llr::new(2.0), Llr::new(-2.0)],
    ///         &[false, true],
    ///     );
    /// }
    /// assert_eq!(stats.report()[0].bit0.count(), 1);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(`llrs.len()`).
    #[inline]
    pub fn accumulate_slice(&mut self, llrs: &[Llr], truth_bits: &[bool]) {
        self.stats.accumulate(llrs, truth_bits);
    }

    /// Convenience entry point: feeds a batch of LLRs and the bits of a
    /// [`BitVec`] directly, materializing the `&[bool]` view inline.
    ///
    /// The simulation runners own the truth as a `BitVec`; this helper
    /// keeps the allocation cost on the opt-in path only.
    ///
    /// # Arguments
    ///
    /// * `llrs` - Demapper output. Length must be a multiple of
    ///   [`AnalysisCapture::bits_per_symbol`].
    /// * `truth_bits` - Ground-truth transmitted bits packed in a
    ///   `BitVec`. Must have the same length as `llrs`.
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len() != truth_bits.len()` or if
    /// `llrs.len() % bits_per_symbol() != 0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::llr::Llr;
    /// use gf2_coding::modem::analysis::PerBitLlrStats;
    /// use gf2_coding::modem::AnalysisCapture;
    /// use gf2_core::BitVec;
    ///
    /// let mut stats = PerBitLlrStats::new(2);
    /// let mut bits = BitVec::zeros(2);
    /// bits.set(1, true);
    /// let mut capture = AnalysisCapture::new(&mut stats);
    /// capture.accumulate_bitvec(
    ///     &[Llr::new(2.0), Llr::new(-2.0)],
    ///     &bits,
    /// );
    /// ```
    ///
    /// # Complexity
    ///
    /// O(`llrs.len()`). Allocates a temporary `Vec<bool>` of that size.
    #[inline]
    pub fn accumulate_bitvec(&mut self, llrs: &[Llr], truth_bits: &BitVec) {
        assert_eq!(
            llrs.len(),
            truth_bits.len(),
            "AnalysisCapture::accumulate_bitvec: llrs.len ({}) != truth_bits.len ({})",
            llrs.len(),
            truth_bits.len(),
        );
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
