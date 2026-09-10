//! Per-bit-position LLR distribution and statistics tooling.
//!
//! This module is **analysis tooling layered on top of the shared
//! [`super::BatchSoftDemapper`] output**, not a second demapper. Callers
//! run their chosen demapper end-to-end, then feed the resulting LLR
//! vectors together with the ground-truth transmitted bits into
//! [`PerBitLlrStats`]. The accumulator streams samples through per-bit
//! conditional (Welford) running statistics and, optionally, binned
//! histograms so that `p(L_k | B_k = 0)` and `p(L_k | B_k = 1)` can be
//! compared across bit positions of any supported constellation.
//!
//! # Layout contract
//!
//! Samples use the canonical demapper layout documented on
//! [`super::BatchSoftDemapper::demap_llrs`]: symbol-major, MSB-first
//! within each symbol. For `bits_per_symbol = m` and `S` transmitted
//! symbols, `llrs[s * m + k]` and `truth_bits[s * m + k]` both refer to
//! bit position `k` of symbol `s`, with `k = 0` being the MSB (matching
//! [`super::LabelWord`] and [`super::BitChannelId`]).
//!
//! # Why per-bit-position and not per-bit-channel
//!
//! For higher-order Gray square-QAM (16/64/256-QAM) the per-position
//! conditional LLR distributions are genuinely non-Gaussian and
//! non-symmetric: inner PAM bits have bimodal `p(L_k | B_k = 0)` because
//! the two equally-likely transmit levels sit on opposite sides of the
//! decision boundary. This tool therefore never advertises conditional
//! independence between positions; it only reports what it observes
//! *per position*. The authoritative per-preset independence claim lives
//! on [`super::BitChannelAnalysis::conditionally_independent`].
//!
//! # SSOT
//!
//! Nothing in this module redefines Eb/N0, sigma, or N0 conversions.
//! Callers that want per-symbol-energy axes pair the accumulator with
//! [`super::awgn_link::unit_energy_sigma_sq_from_eb_n0_db`] or
//! [`super::awgn_link::unit_energy_n0_from_eb_n0_db`].
//!
//! # Precision
//!
//! All internal running statistics are `f64` regardless of the backend
//! LLR precision. `Llr` is `f32` today; widening to `f64` on ingest
//! avoids silent accuracy loss in mean / variance accumulation for long
//! simulations.
//!
//! # Mutual information and GMI
//!
//! Two per-bit mutual-information estimators are exposed:
//!
//! - [`PerBitChannelStats::mutual_info_bits_gaussian_approximation`] — a
//!   fast, closed-form *Gaussian approximation* computed from
//!   `mean(|L_k|)` alone via the consistent-Gaussian J-function with
//!   `sigma^2 = 2 mu`. Always available and always cheap, but it is
//!   **not** a rigorous lower bound on the true mutual information:
//!   when the actual per-position conditional LLR distribution departs
//!   from the consistent Gaussian (notably the bimodal inner-PAM bits
//!   of higher-order Gray-QAM), the approximation can be either
//!   pessimistic or optimistic depending on how the distribution
//!   deviates. Use the histogram estimator below when strict bounds
//!   matter.
//! - [`per_bit_mi_histogram_bits`] — a histogram-based empirical MI
//!   estimate that integrates `0.5 p(i|0) log2(2 p(i|0) / (p(i|0) +
//!   p(i|1))) + 0.5 p(i|1) log2(2 p(i|1) / (p(i|0) + p(i|1)))` over
//!   the shared `lo..=hi` bin grid of the accumulator. Only available
//!   when the accumulator was built via
//!   [`PerBitLlrStats::with_histogram`]. Any mass that fell into
//!   [`Histogram::underflow`] or [`Histogram::overflow`] is **excluded**
//!   from the bin-by-bin sum; pick
//!   [`HistogramConfig::min`]/[`HistogramConfig::max`] wide enough to
//!   cover roughly ±5σ of the expected LLR distribution (and bins
//!   narrow enough to resolve bimodal peaks) to keep the bias small.
//!
//! Summing per-bit MI across the `m` bit positions of a BICM receiver
//! gives the **generalised mutual information (GMI)**:
//!
//! ```text
//!   GMI = sum_{k=0}^{m-1} I(B_k; L_k)   (bits / symbol)
//! ```
//!
//! GMI is a lower bound on the BICM capacity when demapping uses the
//! max-log rule, and equals the BICM capacity for the exact log-MAP rule
//! whenever per-bit independence holds (the canonical
//! Caire/Taricco/Biglieri decomposition). Use [`GmiMethod`] to choose
//! between the Gaussian-approximation sum (fast; an approximation, not
//! a rigorous bound) and the histogram-based sum (closer to the truth
//! for non-Gaussian per-bit channels, at the cost of requiring
//! histogram accumulation). The
//! histogram estimator's accuracy depends on the caller's choice of
//! [`HistogramConfig::min`], [`HistogramConfig::max`], and
//! [`HistogramConfig::num_bins`] — narrow bins and a range covering
//! ±5σ of the expected LLR distribution keep both the tail-truncation
//! bias and the discretisation bias under control.

use core::num::NonZeroUsize;

use crate::llr::Llr;

/// Welford-style running statistics over a single conditional stream of
/// `f64` samples.
///
/// Tracks count, running mean, running `M2` (sum of squared deviations
/// from the mean), minimum, and maximum. Variance is reported as the
/// population variance `M2 / count`; callers that need an unbiased
/// estimator can divide `m2()` by `count() - 1` themselves.
///
/// # Invariants
///
/// - `count == 0` implies `mean == 0`, `m2 == 0`, `min == +inf`, `max == -inf`.
/// - `variance()` returns `0.0` for `count < 1`; this is the conventional
///   empty-set variance and is what the reporting layer consumes.
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::analysis::RunningStats;
///
/// let mut s = RunningStats::new();
/// s.push(1.0);
/// s.push(3.0);
/// s.push(5.0);
/// assert_eq!(s.count(), 3);
/// assert!((s.mean() - 3.0).abs() < 1e-12);
/// // Population variance of {1, 3, 5} is 8/3.
/// assert!((s.variance() - 8.0 / 3.0).abs() < 1e-12);
/// assert_eq!(s.min(), 1.0);
/// assert_eq!(s.max(), 5.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunningStats {
    count: u64,
    mean: f64,
    m2: f64,
    min: f64,
    max: f64,
}

impl RunningStats {
    /// Constructs an empty accumulator.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::analysis::RunningStats;
    /// let s = RunningStats::new();
    /// assert_eq!(s.count(), 0);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn new() -> Self {
        Self {
            count: 0,
            mean: 0.0,
            m2: 0.0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
        }
    }

    /// Updates the running statistics with a single sample.
    ///
    /// Non-finite samples (NaN or infinity) are silently dropped rather
    /// than corrupting the running mean and `M2`. Saturated LLRs
    /// (`+/- f32::INFINITY`) therefore do not destabilize long
    /// simulations; callers that care about saturation counts should
    /// track them separately.
    ///
    /// # Arguments
    ///
    /// * `x` - Sample value. NaN and infinities are ignored.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::analysis::RunningStats;
    /// let mut s = RunningStats::new();
    /// s.push(2.0);
    /// s.push(f64::NAN);         // dropped
    /// s.push(f64::INFINITY);    // dropped
    /// assert_eq!(s.count(), 1);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn push(&mut self, x: f64) {
        if !x.is_finite() {
            return;
        }
        self.count += 1;
        let delta = x - self.mean;
        self.mean += delta / (self.count as f64);
        let delta2 = x - self.mean;
        self.m2 += delta * delta2;
        if x < self.min {
            self.min = x;
        }
        if x > self.max {
            self.max = x;
        }
    }

    /// Number of finite samples accumulated so far.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::analysis::RunningStats;
    /// let mut s = RunningStats::new();
    /// s.push(1.0);
    /// assert_eq!(s.count(), 1);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Running arithmetic mean. Returns `0.0` when `count == 0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::analysis::RunningStats;
    /// let mut s = RunningStats::new();
    /// s.push(2.0);
    /// s.push(4.0);
    /// assert!((s.mean() - 3.0).abs() < 1e-12);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn mean(&self) -> f64 {
        self.mean
    }

    /// Sum of squared deviations from the running mean (`M2`). Returns
    /// `0.0` when `count == 0`. Callers computing an unbiased sample
    /// variance can use `m2() / (count() - 1)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::analysis::RunningStats;
    /// let mut s = RunningStats::new();
    /// s.push(1.0);
    /// s.push(3.0);
    /// // M2 = (1-2)^2 + (3-2)^2 = 2.
    /// assert!((s.m2() - 2.0).abs() < 1e-12);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn m2(&self) -> f64 {
        self.m2
    }

    /// Population variance `M2 / count`. Returns `0.0` when `count == 0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::analysis::RunningStats;
    /// let mut s = RunningStats::new();
    /// s.push(1.0);
    /// s.push(3.0);
    /// assert!((s.variance() - 1.0).abs() < 1e-12);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn variance(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.m2 / (self.count as f64)
        }
    }

    /// Smallest finite sample seen. Returns `+INFINITY` for empty
    /// accumulators so `min()`/`max()` compose cleanly under merge.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::analysis::RunningStats;
    /// let mut s = RunningStats::new();
    /// s.push(2.0);
    /// s.push(-1.0);
    /// assert_eq!(s.min(), -1.0);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn min(&self) -> f64 {
        self.min
    }

    /// Largest finite sample seen. Returns `-INFINITY` for empty
    /// accumulators.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::analysis::RunningStats;
    /// let mut s = RunningStats::new();
    /// s.push(2.0);
    /// s.push(-1.0);
    /// assert_eq!(s.max(), 2.0);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn max(&self) -> f64 {
        self.max
    }
}

impl Default for RunningStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Uniform-width histogram over a finite LLR range.
///
/// Out-of-range samples accumulate into [`Histogram::underflow`] or
/// [`Histogram::overflow`] rather than silently landing in the edge
/// bins. The bin at index `i` covers the half-open interval
/// `[min + i * width, min + (i + 1) * width)` with `width =
/// (max - min) / num_bins`; the last bin includes its right edge so
/// that `sample == max` still lands in `bins[num_bins - 1]`.
///
/// # Invariants
///
/// - `min < max`, `num_bins >= 1`.
/// - `bins.len() == num_bins`.
///
/// # Examples
///
/// ```
/// use core::num::NonZeroUsize;
/// use gf2_coding::modem::analysis::Histogram;
///
/// let mut h = Histogram::new(-4.0, 4.0, NonZeroUsize::new(8).unwrap());
/// h.push(-3.5);
/// h.push(0.0);
/// h.push(3.99);
/// h.push(10.0);
/// assert_eq!(h.total(), 4);
/// assert_eq!(h.overflow(), 1);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Histogram {
    min: f64,
    max: f64,
    width: f64,
    bins: Vec<u64>,
    underflow: u64,
    overflow: u64,
}

impl Histogram {
    /// Constructs an empty histogram.
    ///
    /// # Arguments
    ///
    /// * `min` - Inclusive left edge of the binned range.
    /// * `max` - Right edge of the binned range (strictly greater than
    ///   `min`; the final bin includes this edge).
    /// * `num_bins` - Number of uniform-width bins.
    ///
    /// # Panics
    ///
    /// Panics if `min` or `max` is non-finite, or if `!(min < max)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use core::num::NonZeroUsize;
    /// use gf2_coding::modem::analysis::Histogram;
    ///
    /// let h = Histogram::new(-1.0, 1.0, NonZeroUsize::new(4).unwrap());
    /// assert_eq!(h.bins().len(), 4);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(`num_bins`).
    pub fn new(min: f64, max: f64, num_bins: NonZeroUsize) -> Self {
        assert!(
            min.is_finite() && max.is_finite(),
            "Histogram bounds must be finite, got min={min} max={max}"
        );
        assert!(
            min < max,
            "Histogram requires min < max, got min={min} max={max}"
        );
        let n = num_bins.get();
        let width = (max - min) / (n as f64);
        Self {
            min,
            max,
            width,
            bins: vec![0; n],
            underflow: 0,
            overflow: 0,
        }
    }

    /// Routes a sample into the appropriate bin or tail counter.
    ///
    /// Non-finite samples are dropped (they cannot be meaningfully
    /// binned). `x == max` lands in the rightmost bin.
    ///
    /// # Arguments
    ///
    /// * `x` - Sample value.
    ///
    /// # Examples
    ///
    /// ```
    /// use core::num::NonZeroUsize;
    /// use gf2_coding::modem::analysis::Histogram;
    ///
    /// let mut h = Histogram::new(0.0, 4.0, NonZeroUsize::new(4).unwrap());
    /// h.push(0.5);
    /// h.push(4.0);
    /// assert_eq!(h.bins()[0], 1);
    /// assert_eq!(h.bins()[3], 1);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn push(&mut self, x: f64) {
        if !x.is_finite() {
            return;
        }
        if x < self.min {
            self.underflow += 1;
            return;
        }
        if x > self.max {
            self.overflow += 1;
            return;
        }
        // x in [min, max]; compute bin.
        let idx = ((x - self.min) / self.width).floor() as usize;
        let last = self.bins.len() - 1;
        let idx = if idx >= self.bins.len() { last } else { idx };
        self.bins[idx] += 1;
    }

    /// Total number of finite samples accumulated, including underflow
    /// and overflow. Non-finite samples dropped by [`Histogram::push`]
    /// are not counted.
    ///
    /// # Examples
    ///
    /// ```
    /// use core::num::NonZeroUsize;
    /// use gf2_coding::modem::analysis::Histogram;
    /// let mut h = Histogram::new(0.0, 1.0, NonZeroUsize::new(2).unwrap());
    /// h.push(0.25);   // in-range
    /// h.push(2.0);    // overflow
    /// assert_eq!(h.total(), 2);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(`num_bins`).
    pub fn total(&self) -> u64 {
        self.bins.iter().sum::<u64>() + self.underflow + self.overflow
    }

    /// Bin counts as a slice.
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn bins(&self) -> &[u64] {
        &self.bins
    }

    /// Count of samples below `min`.
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn underflow(&self) -> u64 {
        self.underflow
    }

    /// Count of samples above `max`.
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn overflow(&self) -> u64 {
        self.overflow
    }

    /// Inclusive left edge of the binned range.
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn range_min(&self) -> f64 {
        self.min
    }

    /// Right edge of the binned range.
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn range_max(&self) -> f64 {
        self.max
    }

    /// Bin width `(max - min) / num_bins`.
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn bin_width(&self) -> f64 {
        self.width
    }

    /// Half-open bin edges `[min + i*w, min + (i+1)*w)` for bin `i`.
    ///
    /// The last bin's right edge is inclusive (see [`Histogram`]). The
    /// returned pair always uses strictly ascending endpoints.
    ///
    /// # Arguments
    ///
    /// * `i` - Bin index.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.bins().len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use core::num::NonZeroUsize;
    /// use gf2_coding::modem::analysis::Histogram;
    /// let h = Histogram::new(0.0, 4.0, NonZeroUsize::new(4).unwrap());
    /// let (lo, hi) = h.bin_edges(1);
    /// assert!((lo - 1.0).abs() < 1e-12);
    /// assert!((hi - 2.0).abs() < 1e-12);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn bin_edges(&self, i: usize) -> (f64, f64) {
        assert!(
            i < self.bins.len(),
            "bin index {i} out of range for {} bins",
            self.bins.len()
        );
        let lo = self.min + (i as f64) * self.width;
        let hi = self.min + ((i + 1) as f64) * self.width;
        (lo, hi)
    }
}

/// Configuration for the optional histogram path.
///
/// Histograms are opt-in: the simple running-stats path is always
/// cheap, but the binned conditional-distribution path requires
/// choosing a range and bin count up front. Both `B_k = 0` and
/// `B_k = 1` histograms share the same range and bin count so that
/// exported distributions are directly comparable bin-for-bin.
///
/// # Examples
///
/// ```
/// use core::num::NonZeroUsize;
/// use gf2_coding::modem::analysis::HistogramConfig;
///
/// let cfg = HistogramConfig {
///     min: -20.0,
///     max: 20.0,
///     num_bins: NonZeroUsize::new(64).unwrap(),
/// };
/// assert_eq!(cfg.num_bins.get(), 64);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HistogramConfig {
    /// Inclusive left edge of the binned LLR range (finite).
    pub min: f64,
    /// Right edge of the binned LLR range; final bin includes this edge
    /// (finite, strictly greater than `min`).
    pub max: f64,
    /// Number of uniform-width bins per conditional stream.
    pub num_bins: NonZeroUsize,
}

/// Empirical per-bit-position statistics exported by
/// [`PerBitLlrStats::report`].
///
/// One entry per bit position (`k = 0 .. bits_per_symbol`), indexed
/// MSB-first to match [`super::LabelWord`] and
/// [`super::BitChannelId`]. All fields are `f64`; no lossy conversions
/// occur between accumulation and reporting.
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::analysis::PerBitChannelStats;
/// fn is_biased(stats: &PerBitChannelStats) -> bool {
///     // Under equiprobable bits, a symmetric channel has
///     // mean(L | 0) = -mean(L | 1). Large asymmetry indicates an
///     // asymmetric per-position LLR distribution.
///     (stats.bit0.mean() + stats.bit1.mean()).abs() > 0.1
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct PerBitChannelStats {
    /// Bit position (MSB-first; `0` is the MSB).
    pub bit_index: u8,
    /// Running statistics of `L_k` conditioned on the transmitted bit
    /// being 0. Under the LLR sign convention of [`Llr`], this stream
    /// should be dominated by positive values.
    pub bit0: RunningStats,
    /// Running statistics of `L_k` conditioned on the transmitted bit
    /// being 1. Under the LLR sign convention of [`Llr`], this stream
    /// should be dominated by negative values.
    pub bit1: RunningStats,
    /// Mean of `|L_k|` over all samples (both conditional streams),
    /// a commonly-reported bit-channel reliability figure.
    pub mean_abs_llr: f64,
    /// Gaussian **approximation** to the bit-channel mutual information
    /// `I(B_k; L_k)`, in bits.
    ///
    /// For a symmetric consistent LLR channel with `mean(L | 0) = mu`
    /// and `var(L | 0) ≈ 2 mu` the mutual information equals
    /// `1 - E[log2(1 + exp(-L))] where L ~ N(mu, 2 mu)`. We plug the
    /// observed `mean(|L|)` (as an estimator of `mu`) into the
    /// consistent-Gaussian J-function approximation with `sigma^2 = 2 mu`
    /// and clip into `[0, 1]`.
    ///
    /// This is **not** a rigorous lower bound on the true MI: when the
    /// actual per-bit LLR distribution departs from the consistent
    /// Gaussian model (notably the bimodal inner-PAM bits of
    /// higher-order Gray-QAM), the plug-in J-function estimate can
    /// over- or under-estimate the true MI depending on how the
    /// distribution deviates. For strict bounds or for non-Gaussian
    /// bit-channels, consume the histograms at
    /// [`PerBitChannelStats::hist_bit0`] / [`PerBitChannelStats::hist_bit1`]
    /// and integrate directly via [`per_bit_mi_histogram_bits`].
    pub mutual_info_bits_gaussian_approximation: f64,
    /// Optional histogram of `L_k` given `B_k = 0`. Present iff the
    /// accumulator was built with [`PerBitLlrStats::with_histogram`].
    pub hist_bit0: Option<Histogram>,
    /// Optional histogram of `L_k` given `B_k = 1`. Present iff the
    /// accumulator was built with [`PerBitLlrStats::with_histogram`].
    pub hist_bit1: Option<Histogram>,
    /// Demapper method whose LLRs produced these statistics, or
    /// `None` if the originating accumulator was never stamped (e.g.
    /// built via the legacy `PerBitLlrStats::new` constructor and
    /// fed untagged samples). `Some(DemapMethod::ExactLogMap)` vs.
    /// `Some(DemapMethod::MaxLog)` distinguishes the two regimes
    /// under which [`PerBitChannelStats::mutual_info_bits_gaussian_approximation`]
    /// and [`per_bit_mi_histogram_bits`] have different interpretations
    /// (see the module-level docs). Mixed streams would be rejected
    /// by [`PerBitLlrStats::merge`] so this field, when `Some`, is
    /// guaranteed to cover every sample in the accumulator.
    pub demap_method: Option<super::DemapMethod>,
}

/// Gaussian approximation to mutual information in bits, using the
/// consistent-LLR assumption `sigma_L^2 = 2 mu_L`.
///
/// Returns a value in `[0, 1]`. `mean_abs_llr <= 0` maps to `0`; large
/// `mean_abs_llr` saturates near `1`. Not a rigorous lower bound —
/// see [`PerBitChannelStats::mutual_info_bits_gaussian_approximation`]
/// for when the plug-in estimator can exceed the true MI.
#[inline]
fn gaussian_mi_approximation_bits(mean_abs_llr: f64) -> f64 {
    if !(mean_abs_llr.is_finite()) || mean_abs_llr <= 0.0 {
        return 0.0;
    }
    // Closed-form J-function approximation (ten Brink, "Convergence
    // behavior of iteratively decoded parallel concatenated codes",
    // IEEE Trans. Commun. 2001): fit of I = J(sigma) with
    // sigma^2 = 2 * mean(L | bit=0) for consistent Gaussian LLRs.
    //
    // We use mean_abs_llr as an estimator of mu = mean(L | 0). This
    // is a plug-in approximation, not a rigorous bound — see the
    // public docstring on `mutual_info_bits_gaussian_approximation`
    // for caveats.
    let sigma = (2.0 * mean_abs_llr).sqrt();
    // Coefficients from ten Brink's fit; good to ~1e-3 over
    // sigma in [0, 10].
    const H1: f64 = 0.3073;
    const H2: f64 = 0.8935;
    const H3: f64 = 1.1064;
    let mi = 1.0 - (-H1 * sigma.powf(2.0 * H2)).exp().powf(H3);
    mi.clamp(0.0, 1.0)
}

/// Strategy selector for the [`gmi_bits`] BICM capacity estimator.
///
/// BICM generalised mutual information is the sum of per-bit-channel
/// mutual informations; this enum picks which per-bit MI estimator is
/// summed. Both options return a value in `[0, m]` bits per symbol for
/// an `m`-bit constellation label. The Gaussian-approximation variant
/// is a plug-in estimate (not a rigorous bound — see
/// [`PerBitChannelStats::mutual_info_bits_gaussian_approximation`]);
/// the histogram variant is the empirical MI restricted to the
/// configured histogram range.
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::analysis::GmiMethod;
/// let m = GmiMethod::GaussianApproximation;
/// assert!(matches!(m, GmiMethod::GaussianApproximation));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GmiMethod {
    /// Sum the closed-form Gaussian-approximation per-bit MI estimate
    /// (field
    /// [`PerBitChannelStats::mutual_info_bits_gaussian_approximation`]).
    ///
    /// Always available and cheap. **Not** a rigorous bound: it can
    /// over- or under-estimate the true MI whenever the per-position
    /// conditional LLR distribution is non-Gaussian (higher-order
    /// Gray-QAM inner-PAM bits in particular).
    GaussianApproximation,
    /// Sum the empirical histogram-based per-bit MI from
    /// [`per_bit_mi_histogram_bits`].
    ///
    /// Requires the accumulator to have been built with
    /// [`PerBitLlrStats::with_histogram`]. The estimator integrates the
    /// two conditional densities only over the shared `[lo, hi]` bin
    /// grid; tail mass that fell into [`Histogram::underflow`] or
    /// [`Histogram::overflow`] is ignored. Widen the histogram range
    /// and/or add bins to tighten the estimate on heavy-tailed LLR
    /// distributions.
    Histogram,
}

/// Empirical per-bit mutual information in bits, computed from the pair
/// of conditional histograms attached to a [`PerBitChannelStats`].
///
/// Implements the equiprobable-prior estimator
///
/// ```text
///   I(B_k; L_k) ~= sum_i [ 0.5 p(i|0) log2( 2 p(i|0) / (p(i|0) + p(i|1)) )
///                        + 0.5 p(i|1) log2( 2 p(i|1) / (p(i|0) + p(i|1)) ) ]
/// ```
///
/// where `p(i|b)` is the empirical bin-`i` probability of `L_k` given
/// `B_k = b` (bin counts normalised by the **in-range** sample total,
/// i.e. [`Histogram::total`] minus [`Histogram::underflow`] and
/// [`Histogram::overflow`]). Bins with `p(i|0) = 0` or `p(i|1) = 0`
/// contribute zero under the standard `0 * log 0 = 0` convention. The
/// result is clipped into `[0, 1]`.
///
/// # Arguments
///
/// * `stats` - Per-bit-position stats with both conditional histograms
///   populated.
///
/// # Returns
///
/// `Some(mi_bits)` with `mi_bits` in `[0, 1]`, or `None` if either
/// histogram is absent, if the shared bin grid disagrees, or if either
/// conditional stream has no in-range samples.
///
/// # Tail handling
///
/// Samples that landed in [`Histogram::underflow`] or
/// [`Histogram::overflow`] are **excluded** from both the normalisation
/// and the bin-by-bin sum. Callers that need to capture the full MI of
/// heavy-tailed LLR distributions should widen
/// [`HistogramConfig::min`] / [`HistogramConfig::max`] (a range of
/// ±5σ of the expected LLR distribution typically suffices).
///
/// # Examples
///
/// ```
/// use core::num::NonZeroUsize;
/// use gf2_coding::llr::Llr;
/// use gf2_coding::modem::analysis::{
///     HistogramConfig, PerBitLlrStats, per_bit_mi_histogram_bits,
/// };
///
/// // Perfectly-separated conditional distributions -> MI ~= 1 bit.
/// let cfg = HistogramConfig {
///     min: -4.0,
///     max: 4.0,
///     num_bins: NonZeroUsize::new(8).unwrap(),
/// };
/// let mut s = PerBitLlrStats::new(1).with_histogram(cfg);
/// for _ in 0..128 {
///     s.accumulate(&[Llr::new(3.0)], &[false]);
///     s.accumulate(&[Llr::new(-3.0)], &[true]);
/// }
/// let r = s.report();
/// let mi = per_bit_mi_histogram_bits(&r[0]).unwrap();
/// assert!(mi > 0.99);
/// ```
///
/// # Complexity
///
/// O(`num_bins`).
pub fn per_bit_mi_histogram_bits(stats: &PerBitChannelStats) -> Option<f64> {
    let h0 = stats.hist_bit0.as_ref()?;
    let h1 = stats.hist_bit1.as_ref()?;
    if h0.bins().len() != h1.bins().len() {
        return None;
    }
    let n0: u64 = h0.bins().iter().sum();
    let n1: u64 = h1.bins().iter().sum();
    if n0 == 0 || n1 == 0 {
        return None;
    }
    let inv_n0 = 1.0 / (n0 as f64);
    let inv_n1 = 1.0 / (n1 as f64);
    let mut mi = 0.0f64;
    for (c0, c1) in h0.bins().iter().zip(h1.bins().iter()) {
        let p0 = (*c0 as f64) * inv_n0;
        let p1 = (*c1 as f64) * inv_n1;
        let denom = p0 + p1;
        if denom <= 0.0 {
            continue;
        }
        if p0 > 0.0 {
            mi += 0.5 * p0 * (2.0 * p0 / denom).log2();
        }
        if p1 > 0.0 {
            mi += 0.5 * p1 * (2.0 * p1 / denom).log2();
        }
    }
    Some(mi.clamp(0.0, 1.0))
}

/// Generalised mutual information (BICM capacity estimate) in bits per
/// symbol.
///
/// For a BICM receiver that treats the `m` per-position bit channels
/// as independent, the GMI is the sum of per-bit-channel mutual
/// informations. This function sums the MI estimator selected by
/// `method` across all entries of `stats`.
///
/// GMI is a **lower bound** on BICM capacity when the demapper uses the
/// max-log rule, and **equals** the BICM capacity for the exact log-MAP
/// rule whenever the per-bit-channel independence assumption holds.
///
/// # Arguments
///
/// * `stats` - One [`PerBitChannelStats`] per bit position, typically
///   the output of [`PerBitLlrStats::report`].
/// * `method` - Which per-bit MI estimator to sum.
///
/// # Returns
///
/// GMI in bits per symbol, in the range `[0, stats.len()]`.
///
/// # Panics
///
/// Panics with a clear message when `method == GmiMethod::Histogram`
/// and any entry of `stats` lacks a conditional histogram or otherwise
/// causes [`per_bit_mi_histogram_bits`] to return `None`. The
/// Gaussian-approximation variant never panics.
///
/// # Examples
///
/// ```
/// use gf2_coding::llr::Llr;
/// use gf2_coding::modem::analysis::{gmi_bits, GmiMethod, PerBitLlrStats};
///
/// let mut s = PerBitLlrStats::new(2);
/// s.accumulate(
///     &[Llr::new(4.0), Llr::new(-4.0), Llr::new(-4.0), Llr::new(4.0)],
///     &[false, true, true, false],
/// );
/// let report = s.report();
/// let gmi = gmi_bits(&report, GmiMethod::GaussianApproximation);
/// assert!((0.0..=2.0).contains(&gmi));
/// ```
///
/// # Complexity
///
/// O(`stats.len()`) for the Gaussian variant; O(`stats.len() *
/// num_bins`) for the histogram variant.
pub fn gmi_bits(stats: &[PerBitChannelStats], method: GmiMethod) -> f64 {
    match method {
        GmiMethod::GaussianApproximation => stats
            .iter()
            .map(|s| s.mutual_info_bits_gaussian_approximation)
            .sum(),
        GmiMethod::Histogram => stats
            .iter()
            .enumerate()
            .map(|(k, s)| {
                per_bit_mi_histogram_bits(s).unwrap_or_else(|| {
                    panic!(
                        "gmi_bits: GmiMethod::Histogram requires both conditional \
                         histograms at bit position {k}; build the accumulator \
                         via PerBitLlrStats::with_histogram and ensure both \
                         conditional streams received samples"
                    )
                })
            })
            .sum(),
    }
}

/// Per-bit-position LLR statistics accumulator.
///
/// Constructed once with the constellation's `bits_per_symbol`, then
/// fed demapper outputs in arbitrary-sized batches via
/// [`PerBitLlrStats::accumulate`]. [`PerBitLlrStats::report`] produces
/// a `Vec<PerBitChannelStats>` sized `bits_per_symbol` that downstream
/// consumers use to compare bit positions within and across
/// constellations.
///
/// # Thread safety
///
/// Not internally synchronized. Callers that fan out simulation
/// sweeps across rayon workers should build one accumulator per
/// worker and merge via [`PerBitLlrStats::merge`] at the end; merging
/// is exact (numerically stable Chan-Golub-LeVeque combine).
///
/// # Examples
///
/// ```
/// use gf2_coding::llr::Llr;
/// use gf2_coding::modem::analysis::PerBitLlrStats;
///
/// // 4-bit label per symbol (e.g. 16-QAM).
/// let mut stats = PerBitLlrStats::new(4);
/// let llrs = [
///     Llr::new(2.0), Llr::new(-1.0), Llr::new(3.0), Llr::new(-2.0),
///     Llr::new(-2.0), Llr::new(1.0), Llr::new(-3.0), Llr::new(2.0),
/// ];
/// let truth = [false, true, false, true, true, false, true, false];
/// stats.accumulate(&llrs, &truth);
/// let report = stats.report();
/// assert_eq!(report.len(), 4);
/// assert_eq!(report[0].bit_index, 0);
/// ```
#[derive(Debug, Clone)]
pub struct PerBitLlrStats {
    bits_per_symbol: u8,
    /// Demapper-method provenance for the LLRs this accumulator has
    /// consumed. `None` on a newly-constructed accumulator; becomes
    /// `Some(method)` as soon as an [`AnalysisCapture`](super::AnalysisCapture)
    /// with a declared method writes into it (see
    /// [`PerBitLlrStats::set_demap_method_once`]). `merge` rejects
    /// heterogeneous methods so callers cannot silently mix e.g.
    /// exact-log-MAP and max-log samples into one report.
    demap_method: Option<super::DemapMethod>,
    bit0: Vec<RunningStats>,
    bit1: Vec<RunningStats>,
    abs_llr: Vec<RunningStats>,
    hist_cfg: Option<HistogramConfig>,
    hist_bit0: Vec<Option<Histogram>>,
    hist_bit1: Vec<Option<Histogram>>,
}

impl PerBitLlrStats {
    /// Constructs an empty accumulator for a constellation with
    /// `bits_per_symbol` label bits.
    ///
    /// # Arguments
    ///
    /// * `bits_per_symbol` - Constellation label width `m`, in `[1, 16]`.
    ///
    /// # Panics
    ///
    /// Panics if `bits_per_symbol == 0` or `bits_per_symbol > 16`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::analysis::PerBitLlrStats;
    /// let stats = PerBitLlrStats::new(4);
    /// assert_eq!(stats.bits_per_symbol(), 4);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(`bits_per_symbol`).
    pub fn new(bits_per_symbol: u8) -> Self {
        assert!(
            (1..=16).contains(&bits_per_symbol),
            "bits_per_symbol must be in [1, 16], got {bits_per_symbol}"
        );
        let m = bits_per_symbol as usize;
        Self {
            bits_per_symbol,
            demap_method: None,
            bit0: vec![RunningStats::new(); m],
            bit1: vec![RunningStats::new(); m],
            abs_llr: vec![RunningStats::new(); m],
            hist_cfg: None,
            hist_bit0: vec![None; m],
            hist_bit1: vec![None; m],
        }
    }

    /// Enables binned per-position conditional histograms.
    ///
    /// Both `B_k = 0` and `B_k = 1` histograms share the configured
    /// range and bin count so exported distributions are
    /// bin-for-bin comparable across bit positions.
    ///
    /// # Arguments
    ///
    /// * `cfg` - Shared histogram range and bin count.
    ///
    /// # Panics
    ///
    /// Panics if `cfg.min` or `cfg.max` is non-finite, or if
    /// `!(cfg.min < cfg.max)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use core::num::NonZeroUsize;
    /// use gf2_coding::modem::analysis::{HistogramConfig, PerBitLlrStats};
    ///
    /// let stats = PerBitLlrStats::new(4).with_histogram(HistogramConfig {
    ///     min: -20.0,
    ///     max: 20.0,
    ///     num_bins: NonZeroUsize::new(64).unwrap(),
    /// });
    /// assert_eq!(stats.bits_per_symbol(), 4);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(`bits_per_symbol * num_bins`).
    pub fn with_histogram(mut self, cfg: HistogramConfig) -> Self {
        let m = self.bits_per_symbol as usize;
        self.hist_bit0 = (0..m)
            .map(|_| Some(Histogram::new(cfg.min, cfg.max, cfg.num_bins)))
            .collect();
        self.hist_bit1 = (0..m)
            .map(|_| Some(Histogram::new(cfg.min, cfg.max, cfg.num_bins)))
            .collect();
        self.hist_cfg = Some(cfg);
        self
    }

    /// Returns the configured constellation label width.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::analysis::PerBitLlrStats;
    /// assert_eq!(PerBitLlrStats::new(6).bits_per_symbol(), 6);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn bits_per_symbol(&self) -> u8 {
        self.bits_per_symbol
    }

    /// Returns the demapper method whose LLRs populated this
    /// accumulator, or `None` if no tagged samples have been
    /// accumulated yet.
    ///
    /// MI/GMI interpretation depends on the demapper method that
    /// produced the LLRs (see the module-level docs). Once set, the
    /// tag is part of every [`PerBitChannelStats::demap_method`] and
    /// prevents [`PerBitLlrStats::merge`] from combining heterogeneous
    /// streams.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::analysis::PerBitLlrStats;
    ///
    /// let stats = PerBitLlrStats::new(1);
    /// assert_eq!(stats.demap_method(), None);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn demap_method(&self) -> Option<super::DemapMethod> {
        self.demap_method
    }

    /// Stamps the accumulator with the demapper method whose LLRs will
    /// be accumulated. Idempotent when the stamp matches; panics on a
    /// mismatch so a single `PerBitLlrStats` cannot silently span two
    /// method regimes.
    ///
    /// Typically invoked by
    /// [`AnalysisCapture::with_method`](super::AnalysisCapture::with_method)
    /// rather than by users directly — the capture layer forwards the
    /// method stamp from the `ChannelModel` into the accumulator on
    /// the first batch.
    ///
    /// # Arguments
    ///
    /// * `method` — the demapper method that produced the LLRs about
    ///   to be folded in.
    ///
    /// # Panics
    ///
    /// Panics if this accumulator was previously stamped with a
    /// *different* method.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::analysis::PerBitLlrStats;
    /// use gf2_coding::modem::DemapMethod;
    ///
    /// let mut stats = PerBitLlrStats::new(1);
    /// stats.set_demap_method_once(DemapMethod::ExactLogMap);
    /// assert_eq!(stats.demap_method(), Some(DemapMethod::ExactLogMap));
    /// // Idempotent on a matching re-stamp:
    /// stats.set_demap_method_once(DemapMethod::ExactLogMap);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn set_demap_method_once(&mut self, method: super::DemapMethod) {
        match self.demap_method {
            None => self.demap_method = Some(method),
            Some(existing) => assert_eq!(
                existing, method,
                "PerBitLlrStats was previously stamped with {existing:?}, cannot re-stamp with {method:?}"
            ),
        }
    }

    /// Folds a batch of demapper LLRs and matching truth bits into the
    /// per-position accumulators.
    ///
    /// Both slices follow the canonical demapper layout: symbol-major,
    /// MSB-first within each symbol. `llrs[s * m + k]` is the LLR of
    /// bit position `k` in the `s`-th symbol and
    /// `truth_bits[s * m + k]` is the corresponding transmitted bit
    /// (`false` = 0, `true` = 1). See
    /// [`super::BatchSoftDemapper::demap_llrs`].
    ///
    /// # Arguments
    ///
    /// * `llrs` - Demapper output. Length must be a non-negative
    ///   multiple of `bits_per_symbol()`.
    /// * `truth_bits` - Ground-truth transmitted bits. Length must
    ///   match `llrs`.
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len() != truth_bits.len()` or if the length is
    /// not a multiple of `bits_per_symbol()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::llr::Llr;
    /// use gf2_coding::modem::analysis::PerBitLlrStats;
    ///
    /// let mut stats = PerBitLlrStats::new(2);
    /// let llrs = [Llr::new(1.5), Llr::new(-2.5)];
    /// let truth = [false, true];
    /// stats.accumulate(&llrs, &truth);
    /// let r = stats.report();
    /// assert_eq!(r[0].bit0.count(), 1);
    /// assert_eq!(r[1].bit1.count(), 1);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(`llrs.len()`).
    pub fn accumulate(&mut self, llrs: &[Llr], truth_bits: &[bool]) {
        assert_eq!(
            llrs.len(),
            truth_bits.len(),
            "llrs.len() ({}) != truth_bits.len() ({})",
            llrs.len(),
            truth_bits.len()
        );
        let m = self.bits_per_symbol as usize;
        assert!(
            llrs.len().is_multiple_of(m),
            "llrs.len() ({}) is not a multiple of bits_per_symbol ({})",
            llrs.len(),
            m
        );
        for chunk_idx in 0..(llrs.len() / m) {
            let base = chunk_idx * m;
            for k in 0..m {
                let x = llrs[base + k].value() as f64;
                let truth = truth_bits[base + k];
                self.abs_llr[k].push(x.abs());
                if truth {
                    self.bit1[k].push(x);
                    if let Some(h) = self.hist_bit1[k].as_mut() {
                        h.push(x);
                    }
                } else {
                    self.bit0[k].push(x);
                    if let Some(h) = self.hist_bit0[k].as_mut() {
                        h.push(x);
                    }
                }
            }
        }
    }

    /// Merges another accumulator into `self` using numerically
    /// stable pairwise combines for mean and `M2`. Both accumulators
    /// must share the same `bits_per_symbol` and the same histogram
    /// configuration (or both have none).
    ///
    /// # Arguments
    ///
    /// * `other` - Accumulator to fold into `self`; consumed.
    ///
    /// # Panics
    ///
    /// Panics if `bits_per_symbol` or histogram configurations differ.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::llr::Llr;
    /// use gf2_coding::modem::analysis::PerBitLlrStats;
    ///
    /// let mut a = PerBitLlrStats::new(2);
    /// a.accumulate(&[Llr::new(1.0), Llr::new(-1.0)], &[false, true]);
    /// let mut b = PerBitLlrStats::new(2);
    /// b.accumulate(&[Llr::new(3.0), Llr::new(-3.0)], &[false, true]);
    /// a.merge(b);
    /// assert_eq!(a.report()[0].bit0.count(), 2);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(`bits_per_symbol * num_bins`) if histograms are enabled;
    /// otherwise O(`bits_per_symbol`).
    pub fn merge(&mut self, other: Self) {
        assert_eq!(
            self.bits_per_symbol, other.bits_per_symbol,
            "merge: bits_per_symbol mismatch ({} vs {})",
            self.bits_per_symbol, other.bits_per_symbol
        );
        assert_eq!(
            self.hist_cfg, other.hist_cfg,
            "merge: histogram configurations differ"
        );
        // Demapper-method provenance: if both accumulators carry a
        // stamp, the stamps must match — MI/GMI semantics differ
        // between exact log-MAP and max-log, so silently combining
        // them would yield un-interpretable summaries. If only one
        // side is stamped, propagate that stamp into the merged
        // accumulator.
        match (self.demap_method, other.demap_method) {
            (Some(a), Some(b)) => assert_eq!(
                a, b,
                "merge: demap_method mismatch ({a:?} vs {b:?}); refusing to combine heterogeneous LLR streams"
            ),
            (None, Some(b)) => self.demap_method = Some(b),
            _ => {}
        }
        let m = self.bits_per_symbol as usize;
        for k in 0..m {
            merge_running(&mut self.bit0[k], &other.bit0[k]);
            merge_running(&mut self.bit1[k], &other.bit1[k]);
            merge_running(&mut self.abs_llr[k], &other.abs_llr[k]);
            if let (Some(dst), Some(src)) =
                (self.hist_bit0[k].as_mut(), other.hist_bit0[k].as_ref())
            {
                merge_hist(dst, src);
            }
            if let (Some(dst), Some(src)) =
                (self.hist_bit1[k].as_mut(), other.hist_bit1[k].as_ref())
            {
                merge_hist(dst, src);
            }
        }
    }

    /// Exports one [`PerBitChannelStats`] per bit position.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::llr::Llr;
    /// use gf2_coding::modem::analysis::PerBitLlrStats;
    ///
    /// let mut stats = PerBitLlrStats::new(2);
    /// stats.accumulate(&[Llr::new(2.0), Llr::new(-2.0)], &[false, true]);
    /// let r = stats.report();
    /// assert_eq!(r.len(), 2);
    /// assert!((r[0].mean_abs_llr - 2.0).abs() < 1e-12);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(`bits_per_symbol * num_bins`) if histograms are enabled;
    /// otherwise O(`bits_per_symbol`).
    pub fn report(&self) -> Vec<PerBitChannelStats> {
        let m = self.bits_per_symbol as usize;
        (0..m)
            .map(|k| {
                let mean_abs_llr = self.abs_llr[k].mean();
                PerBitChannelStats {
                    bit_index: k as u8,
                    bit0: self.bit0[k],
                    bit1: self.bit1[k],
                    mean_abs_llr,
                    mutual_info_bits_gaussian_approximation: gaussian_mi_approximation_bits(
                        mean_abs_llr,
                    ),
                    hist_bit0: self.hist_bit0[k].clone(),
                    hist_bit1: self.hist_bit1[k].clone(),
                    demap_method: self.demap_method,
                }
            })
            .collect()
    }
}

/// Chan-Golub-LeVeque numerically stable combine for two running-stats
/// streams.
fn merge_running(dst: &mut RunningStats, src: &RunningStats) {
    if src.count == 0 {
        return;
    }
    if dst.count == 0 {
        *dst = *src;
        return;
    }
    let n_a = dst.count as f64;
    let n_b = src.count as f64;
    let n = n_a + n_b;
    let delta = src.mean - dst.mean;
    let new_mean = dst.mean + delta * n_b / n;
    let new_m2 = dst.m2 + src.m2 + delta * delta * n_a * n_b / n;
    dst.count += src.count;
    dst.mean = new_mean;
    dst.m2 = new_m2;
    if src.min < dst.min {
        dst.min = src.min;
    }
    if src.max > dst.max {
        dst.max = src.max;
    }
}

/// Bin-by-bin histogram merge. Both histograms must share the same
/// range and bin count (enforced by [`PerBitLlrStats::merge`]).
fn merge_hist(dst: &mut Histogram, src: &Histogram) {
    debug_assert_eq!(dst.bins.len(), src.bins.len());
    debug_assert_eq!(dst.min, src.min);
    debug_assert_eq!(dst.max, src.max);
    for (d, s) in dst.bins.iter_mut().zip(src.bins.iter()) {
        *d += *s;
    }
    dst.underflow += src.underflow;
    dst.overflow += src.overflow;
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_running_stats_empty_state() {
        let s = RunningStats::new();
        assert_eq!(s.count(), 0);
        assert_eq!(s.mean(), 0.0);
        assert_eq!(s.m2(), 0.0);
        assert_eq!(s.variance(), 0.0);
        assert_eq!(s.min(), f64::INFINITY);
        assert_eq!(s.max(), f64::NEG_INFINITY);
    }

    #[test]
    fn test_running_stats_drops_non_finite() {
        let mut s = RunningStats::new();
        s.push(1.0);
        s.push(f64::NAN);
        s.push(f64::INFINITY);
        s.push(f64::NEG_INFINITY);
        s.push(2.0);
        assert_eq!(s.count(), 2);
        assert!((s.mean() - 1.5).abs() < 1e-12);
    }

    #[test]
    fn test_running_stats_matches_closed_form_variance() {
        let xs = [1.0, 2.0, 3.0, 4.0, 5.0];
        let mut s = RunningStats::new();
        for &x in &xs {
            s.push(x);
        }
        let mean = xs.iter().sum::<f64>() / xs.len() as f64;
        let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / xs.len() as f64;
        assert!((s.mean() - mean).abs() < 1e-12);
        assert!((s.variance() - var).abs() < 1e-12);
        assert_eq!(s.min(), 1.0);
        assert_eq!(s.max(), 5.0);
    }

    #[test]
    fn test_histogram_routes_bins_and_tails() {
        let mut h = Histogram::new(-4.0, 4.0, NonZeroUsize::new(8).unwrap());
        h.push(-10.0); // underflow
        h.push(-4.0); // bin 0
        h.push(-3.5); // bin 0
        h.push(0.0); // bin 4
        h.push(3.9999); // bin 7
        h.push(4.0); // bin 7 (right edge inclusive)
        h.push(10.0); // overflow
        h.push(f64::NAN); // dropped
        assert_eq!(h.underflow(), 1);
        assert_eq!(h.overflow(), 1);
        assert_eq!(h.bins()[0], 2);
        assert_eq!(h.bins()[4], 1);
        assert_eq!(h.bins()[7], 2);
        // 5 in-range samples + 1 underflow + 1 overflow.
        assert_eq!(h.total(), 7);
    }

    #[test]
    #[should_panic(expected = "min < max")]
    fn test_histogram_rejects_degenerate_range() {
        let _ = Histogram::new(1.0, 1.0, NonZeroUsize::new(4).unwrap());
    }

    #[test]
    #[should_panic(expected = "finite")]
    fn test_histogram_rejects_non_finite_bounds() {
        let _ = Histogram::new(f64::NAN, 1.0, NonZeroUsize::new(4).unwrap());
    }

    #[test]
    fn test_per_bit_llr_stats_splits_by_truth() {
        // m = 2, 4 symbols; bit 0 always 0, bit 1 always 1.
        // LLRs chosen so bit 0 has mean 2.0, bit 1 has mean -2.0.
        let llrs: Vec<Llr> = [1.0_f32, -1.0, 2.0, -2.0, 3.0, -3.0, 2.0, -2.0]
            .iter()
            .map(|&v| Llr::new(v))
            .collect();
        let truth = [false, true, false, true, false, true, false, true];
        let mut stats = PerBitLlrStats::new(2);
        stats.accumulate(&llrs, &truth);
        let r = stats.report();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].bit_index, 0);
        assert_eq!(r[0].bit0.count(), 4);
        assert_eq!(r[0].bit1.count(), 0);
        assert_eq!(r[1].bit0.count(), 0);
        assert_eq!(r[1].bit1.count(), 4);
        assert!((r[0].bit0.mean() - 2.0).abs() < 1e-6);
        assert!((r[1].bit1.mean() - -2.0).abs() < 1e-6);
        // mean|L| = 2 on both positions.
        assert!((r[0].mean_abs_llr - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_per_bit_llr_stats_histograms_opt_in() {
        let stats = PerBitLlrStats::new(2);
        let r = stats.report();
        assert!(r[0].hist_bit0.is_none());
        assert!(r[0].hist_bit1.is_none());

        let cfg = HistogramConfig {
            min: -10.0,
            max: 10.0,
            num_bins: NonZeroUsize::new(20).unwrap(),
        };
        let mut stats = PerBitLlrStats::new(2).with_histogram(cfg);
        stats.accumulate(&[Llr::new(2.0), Llr::new(-2.0)], &[false, true]);
        let r = stats.report();
        let h0 = r[0].hist_bit0.as_ref().unwrap();
        let h1 = r[1].hist_bit1.as_ref().unwrap();
        assert_eq!(h0.bins().len(), 20);
        assert_eq!(h1.bins().len(), 20);
        assert_eq!(h0.total(), 1);
        assert_eq!(h1.total(), 1);
    }

    #[test]
    #[should_panic(expected = "bits_per_symbol must be in [1, 16]")]
    fn test_per_bit_llr_stats_rejects_zero_bits() {
        let _ = PerBitLlrStats::new(0);
    }

    #[test]
    #[should_panic(expected = "not a multiple")]
    fn test_per_bit_llr_stats_rejects_ragged_input() {
        let mut s = PerBitLlrStats::new(2);
        s.accumulate(&[Llr::new(1.0)], &[false]);
    }

    #[test]
    #[should_panic(expected = "truth_bits.len()")]
    fn test_per_bit_llr_stats_rejects_length_mismatch() {
        let mut s = PerBitLlrStats::new(2);
        s.accumulate(&[Llr::new(1.0), Llr::new(2.0)], &[false]);
    }

    #[test]
    fn test_merge_exact_equivalent_to_single_stream() {
        let all: Vec<Llr> = (0..100)
            .map(|i| Llr::new((i as f32 - 50.0) * 0.1))
            .collect();
        let truth: Vec<bool> = (0..100).map(|i| i % 2 == 1).collect();

        let mut single = PerBitLlrStats::new(2);
        single.accumulate(&all, &truth);

        // Split at an odd boundary; both halves must stay multiples of m=2.
        let split = 50; // 50 is even -> keeps m=2 alignment.
        let mut a = PerBitLlrStats::new(2);
        a.accumulate(&all[..split], &truth[..split]);
        let mut b = PerBitLlrStats::new(2);
        b.accumulate(&all[split..], &truth[split..]);
        a.merge(b);

        let r_single = single.report();
        let r_merged = a.report();
        for k in 0..2 {
            assert_eq!(r_single[k].bit0.count(), r_merged[k].bit0.count());
            assert_eq!(r_single[k].bit1.count(), r_merged[k].bit1.count());
            // Mean and M2 should agree to numerical precision.
            assert!((r_single[k].bit0.mean() - r_merged[k].bit0.mean()).abs() < 1e-10);
            assert!((r_single[k].bit0.m2() - r_merged[k].bit0.m2()).abs() < 1e-9);
            assert!((r_single[k].bit1.mean() - r_merged[k].bit1.mean()).abs() < 1e-10);
            assert!((r_single[k].bit1.m2() - r_merged[k].bit1.m2()).abs() < 1e-9);
        }
    }

    #[test]
    fn test_merge_with_histograms_sums_bins() {
        let cfg = HistogramConfig {
            min: -4.0,
            max: 4.0,
            num_bins: NonZeroUsize::new(8).unwrap(),
        };
        let mut a = PerBitLlrStats::new(1).with_histogram(cfg);
        a.accumulate(&[Llr::new(1.0)], &[false]);
        let mut b = PerBitLlrStats::new(1).with_histogram(cfg);
        b.accumulate(&[Llr::new(1.5)], &[false]);
        a.merge(b);
        let r = a.report();
        let h = r[0].hist_bit0.as_ref().unwrap();
        assert_eq!(h.total(), 2);
    }

    #[test]
    #[should_panic(expected = "histogram configurations differ")]
    fn test_merge_rejects_mismatched_hist_cfg() {
        let cfg_a = HistogramConfig {
            min: -4.0,
            max: 4.0,
            num_bins: NonZeroUsize::new(8).unwrap(),
        };
        let cfg_b = HistogramConfig {
            min: -4.0,
            max: 4.0,
            num_bins: NonZeroUsize::new(16).unwrap(),
        };
        let a = PerBitLlrStats::new(1).with_histogram(cfg_a);
        let b = PerBitLlrStats::new(1).with_histogram(cfg_b);
        let mut a = a;
        a.merge(b);
    }

    #[test]
    fn test_gaussian_mi_monotone_and_bounded() {
        // Monotone non-decreasing in mean_abs_llr, clipped to [0, 1].
        assert_eq!(gaussian_mi_approximation_bits(0.0), 0.0);
        assert_eq!(gaussian_mi_approximation_bits(-1.0), 0.0);
        let small = gaussian_mi_approximation_bits(0.1);
        let mid = gaussian_mi_approximation_bits(2.0);
        let big = gaussian_mi_approximation_bits(50.0);
        assert!(small < mid);
        assert!(mid < big);
        assert!(big <= 1.0);
    }

    #[test]
    fn test_report_populates_mutual_info_from_mean_abs() {
        let mut stats = PerBitLlrStats::new(1);
        stats.accumulate(&[Llr::new(4.0), Llr::new(-4.0)], &[false, true]);
        let r = stats.report();
        assert!((r[0].mean_abs_llr - 4.0).abs() < 1e-6);
        let expected = gaussian_mi_approximation_bits(4.0);
        assert!((r[0].mutual_info_bits_gaussian_approximation - expected).abs() < 1e-12);
    }

    #[test]
    fn test_bit_edges_cover_full_range() {
        let h = Histogram::new(0.0, 4.0, NonZeroUsize::new(4).unwrap());
        let (lo0, _) = h.bin_edges(0);
        let (_, hi_last) = h.bin_edges(3);
        assert!((lo0 - 0.0).abs() < 1e-12);
        assert!((hi_last - 4.0).abs() < 1e-12);
    }

    #[test]
    fn test_accumulate_skips_saturated_llrs_in_running_stats() {
        let mut s = PerBitLlrStats::new(1);
        s.accumulate(
            &[Llr::infinity(), Llr::new(1.0), Llr::neg_infinity()],
            &[false, false, false],
        );
        let r = s.report();
        // All three are bit=0, but infinities are dropped from running stats.
        assert_eq!(r[0].bit0.count(), 1);
    }

    // ----- Property tests ------------------------------------------------
    //
    // Invariants of merge and accumulate that must hold for any finite
    // sequence of samples; example-based tests above cover specific
    // regressions, these cover the mathematical shape.

    proptest! {
        /// `merge(a, b)` with `a` and `b` drawn from the same underlying
        /// stream split into halves must equal accumulating the full
        /// stream in one go, modulo floating-point roundoff.
        #[test]
        fn test_merge_associativity_matches_full_stream(
            seed in 0u64..1024,
            m in 1u8..=4,
            batch in 1usize..=128,
        ) {
            use gf2_core::rng::Lcg;
            let m_us = m as usize;
            let mut rng = Lcg::new(seed);
            let n = batch * m_us;
            let llrs: Vec<Llr> = (0..n)
                .map(|_| Llr::new(rng.next_unit_f32() * 8.0))
                .collect();
            let truth: Vec<bool> = (0..n).map(|_| rng.next_u64() & 1 == 1).collect();

            let mid = (batch / 2) * m_us;
            let mut full = PerBitLlrStats::new(m);
            full.accumulate(&llrs, &truth);

            let mut a = PerBitLlrStats::new(m);
            a.accumulate(&llrs[..mid], &truth[..mid]);
            let mut b = PerBitLlrStats::new(m);
            b.accumulate(&llrs[mid..], &truth[mid..]);
            a.merge(b);

            let r_full = full.report();
            let r_merged = a.report();
            for k in 0..m_us {
                prop_assert_eq!(r_full[k].bit0.count(), r_merged[k].bit0.count());
                prop_assert_eq!(r_full[k].bit1.count(), r_merged[k].bit1.count());
                // Welford merge is only equal to single-pass up to FP
                // roundoff; 1e-9 relative tolerance is sufficient here.
                let e_full = r_full[k].mean_abs_llr;
                let e_merge = r_merged[k].mean_abs_llr;
                let diff = (e_full - e_merge).abs();
                let tol = 1e-9 * (1.0 + e_full.abs());
                prop_assert!(
                    diff <= tol,
                    "mean_abs_llr mismatch after merge: full={e_full} merged={e_merge} diff={diff}"
                );
            }
        }

        /// MI lower bound is monotone non-decreasing in `mean_abs_llr`
        /// and always in `[0, 1]`.
        #[test]
        fn test_gaussian_mi_monotone_in_mean_abs(
            a in 0.0f64..100.0,
            delta in 0.0f64..100.0,
        ) {
            let lo = gaussian_mi_approximation_bits(a);
            let hi = gaussian_mi_approximation_bits(a + delta);
            prop_assert!((0.0..=1.0).contains(&lo));
            prop_assert!((0.0..=1.0).contains(&hi));
            prop_assert!(hi + 1e-12 >= lo,
                "non-monotone: mi(a={a})={lo}, mi(a+delta={}) = {hi}", a + delta);
        }

        /// For any valid histogram-backed accumulator, the histogram GMI
        /// must lie in `[0, m]` bits/symbol.
        #[test]
        fn test_prop_gmi_histogram_bounded_by_m(
            seed in 0u64..1024,
            m in 1u8..=4,
            batch in 4usize..=64,
        ) {
            use gf2_core::rng::Lcg;
            let m_us = m as usize;
            let mut rng = Lcg::new(seed);
            let n = batch * m_us;
            // Symmetric LLR-like samples in [-8, 8].
            let llrs: Vec<Llr> = (0..n)
                .map(|_| Llr::new((rng.next_unit_f32() * 2.0 - 1.0) * 8.0))
                .collect();
            let truth: Vec<bool> = (0..n).map(|_| rng.next_u64() & 1 == 1).collect();

            let cfg = HistogramConfig {
                min: -12.0,
                max: 12.0,
                num_bins: NonZeroUsize::new(24).unwrap(),
            };
            let mut s = PerBitLlrStats::new(m).with_histogram(cfg);
            s.accumulate(&llrs, &truth);
            let r = s.report();

            // If any bit position never saw both truth=0 and truth=1,
            // the histogram estimator returns None; skip the test case.
            let all_populated = r.iter().all(|p| {
                let h0 = p.hist_bit0.as_ref().unwrap();
                let h1 = p.hist_bit1.as_ref().unwrap();
                h0.bins().iter().any(|&c| c > 0) && h1.bins().iter().any(|&c| c > 0)
            });
            prop_assume!(all_populated);

            let gmi = gmi_bits(&r, GmiMethod::Histogram);
            prop_assert!(gmi >= -1e-12,
                "gmi_bits (histogram) went negative: {gmi}");
            prop_assert!(gmi <= m as f64 + 1e-12,
                "gmi_bits (histogram) {gmi} exceeds m={m}");

            // Gaussian-approximation variant is always callable and
            // obeys the same bound.
            let gmi_g = gmi_bits(&r, GmiMethod::GaussianApproximation);
            prop_assert!((0.0..=m as f64 + 1e-12).contains(&gmi_g),
                "gmi_bits (gaussian) {gmi_g} out of [0, m={m}]");
        }
    }

    // ----- MI / GMI tests ------------------------------------------------

    /// Builds a [`PerBitLlrStats`] with a single bit position whose two
    /// conditional histograms are populated from pre-chosen LLR samples.
    fn single_bit_stats_with_hist(
        cfg: HistogramConfig,
        xs_bit0: &[f32],
        xs_bit1: &[f32],
    ) -> PerBitChannelStats {
        let mut s = PerBitLlrStats::new(1).with_histogram(cfg);
        let llrs_bit0: Vec<Llr> = xs_bit0.iter().map(|&x| Llr::new(x)).collect();
        let truth_bit0 = vec![false; xs_bit0.len()];
        s.accumulate(&llrs_bit0, &truth_bit0);
        let llrs_bit1: Vec<Llr> = xs_bit1.iter().map(|&x| Llr::new(x)).collect();
        let truth_bit1 = vec![true; xs_bit1.len()];
        s.accumulate(&llrs_bit1, &truth_bit1);
        s.report().into_iter().next().unwrap()
    }

    #[test]
    fn test_per_bit_mi_histogram_bits_zero_noise() {
        // Perfect separation: bit=0 samples at +3, bit=1 samples at -3.
        // Bin grid includes both, so empirical MI must saturate at 1 bit.
        let cfg = HistogramConfig {
            min: -4.0,
            max: 4.0,
            num_bins: NonZeroUsize::new(8).unwrap(),
        };
        let xs0 = vec![3.0_f32; 256];
        let xs1 = vec![-3.0_f32; 256];
        let stats = single_bit_stats_with_hist(cfg, &xs0, &xs1);
        let mi = per_bit_mi_histogram_bits(&stats).expect("histograms present");
        assert!(
            (mi - 1.0).abs() < 1e-9,
            "zero-noise MI should saturate at 1 bit, got {mi}"
        );
    }

    #[test]
    fn test_per_bit_mi_histogram_bits_pure_noise() {
        // Identical conditional distributions -> MI is exactly 0.
        let cfg = HistogramConfig {
            min: -4.0,
            max: 4.0,
            num_bins: NonZeroUsize::new(8).unwrap(),
        };
        let xs = vec![0.5_f32; 64];
        let stats = single_bit_stats_with_hist(cfg, &xs, &xs);
        let mi = per_bit_mi_histogram_bits(&stats).expect("histograms present");
        assert!(mi.abs() < 1e-12, "pure-noise MI should be 0, got {mi}");
    }

    #[test]
    fn test_per_bit_mi_histogram_bits_requires_histograms() {
        // Accumulator without histograms: MI estimator must return None.
        let mut s = PerBitLlrStats::new(1);
        s.accumulate(&[Llr::new(2.0), Llr::new(-2.0)], &[false, true]);
        let r = s.report();
        assert!(per_bit_mi_histogram_bits(&r[0]).is_none());
    }

    #[test]
    fn test_gmi_gaussian_approximation_sums_per_bit_mi() {
        // The Gaussian-approximation GMI is just the sum of the
        // Gaussian-lower-bound fields on each PerBitChannelStats.
        use gf2_core::rng::Lcg;
        let mut rng = Lcg::new(0xC0FFEE);
        let m = 3;
        let n_syms = 64;
        let llrs: Vec<Llr> = (0..n_syms * m)
            .map(|_| Llr::new((rng.next_unit_f32() * 2.0 - 1.0) * 5.0))
            .collect();
        let truth: Vec<bool> = (0..n_syms * m).map(|_| rng.next_u64() & 1 == 1).collect();
        let mut s = PerBitLlrStats::new(m as u8);
        s.accumulate(&llrs, &truth);
        let r = s.report();
        let expected: f64 = r
            .iter()
            .map(|p| p.mutual_info_bits_gaussian_approximation)
            .sum();
        let actual = gmi_bits(&r, GmiMethod::GaussianApproximation);
        assert!(
            (actual - expected).abs() < 1e-12,
            "gmi_bits(GaussianApproximation) should sum per-bit MI fields: \
             expected={expected}, actual={actual}"
        );
    }

    #[test]
    fn test_gmi_histogram_matches_single_bit() {
        // For m = 1, GMI(Histogram) must equal per_bit_mi_histogram_bits
        // on the single position.
        let cfg = HistogramConfig {
            min: -4.0,
            max: 4.0,
            num_bins: NonZeroUsize::new(16).unwrap(),
        };
        let xs0: Vec<f32> = (0..128).map(|i| 1.5 + (i as f32) * 0.01).collect();
        let xs1: Vec<f32> = (0..128).map(|i| -1.5 + (i as f32) * 0.01).collect();
        let stats = single_bit_stats_with_hist(cfg, &xs0, &xs1);
        let per_bit = per_bit_mi_histogram_bits(&stats).unwrap();
        let gmi = gmi_bits(std::slice::from_ref(&stats), GmiMethod::Histogram);
        assert!(
            (gmi - per_bit).abs() < 1e-12,
            "m=1 GMI(Histogram) should equal per-bit MI: per_bit={per_bit}, gmi={gmi}"
        );
    }

    #[test]
    #[should_panic(expected = "GmiMethod::Histogram requires both conditional histograms")]
    fn test_gmi_histogram_panics_without_histograms() {
        let mut s = PerBitLlrStats::new(1);
        s.accumulate(&[Llr::new(2.0)], &[false]);
        let r = s.report();
        let _ = gmi_bits(&r, GmiMethod::Histogram);
    }
}
