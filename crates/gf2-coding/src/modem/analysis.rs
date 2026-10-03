//! Per-bit-position LLR statistics computed from demapper output:
//! [`PerBitLlrStats`] accumulates conditional running statistics and
//! optional histograms of `p(L_k | B_k)`, and [`gmi_bits`] sums per-bit
//! mutual-information estimates into a GMI.

use core::num::NonZeroUsize;

use crate::llr::Llr;

/// Welford running statistics over a stream of `f64` samples: count, mean,
/// `M2` (sum of squared deviations from the mean), minimum and maximum.
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

    /// Updates the running statistics with a single sample; NaN and
    /// infinite samples are dropped.
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
    #[inline]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Running arithmetic mean. Returns `0.0` when `count == 0`.
    #[inline]
    pub fn mean(&self) -> f64 {
        self.mean
    }

    /// Sum of squared deviations from the running mean (`M2`). Returns
    /// `0.0` when `count == 0`. Callers computing an unbiased sample
    /// variance can use `m2() / (count() - 1)`.
    #[inline]
    pub fn m2(&self) -> f64 {
        self.m2
    }

    /// Population variance `M2 / count`. Returns `0.0` when `count == 0`.
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
    #[inline]
    pub fn min(&self) -> f64 {
        self.min
    }

    /// Largest finite sample seen. Returns `-INFINITY` for empty
    /// accumulators.
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
    /// # Panics
    ///
    /// Panics if `min` or `max` is non-finite, or if `!(min < max)`.
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
        let idx = ((x - self.min) / self.width).floor() as usize;
        let last = self.bins.len() - 1;
        let idx = if idx >= self.bins.len() { last } else { idx };
        self.bins[idx] += 1;
    }

    /// Total number of finite samples accumulated, including underflow
    /// and overflow. Non-finite samples dropped by [`Histogram::push`]
    /// are not counted.
    ///
    /// # Complexity
    ///
    /// O(`num_bins`).
    pub fn total(&self) -> u64 {
        self.bins.iter().sum::<u64>() + self.underflow + self.overflow
    }

    /// Bin counts as a slice.
    pub fn bins(&self) -> &[u64] {
        &self.bins
    }

    /// Count of samples below `min`.
    pub fn underflow(&self) -> u64 {
        self.underflow
    }

    /// Count of samples above `max`.
    pub fn overflow(&self) -> u64 {
        self.overflow
    }

    /// Inclusive left edge of the binned range.
    pub fn range_min(&self) -> f64 {
        self.min
    }

    /// Right edge of the binned range.
    pub fn range_max(&self) -> f64 {
        self.max
    }

    /// Bin width `(max - min) / num_bins`.
    pub fn bin_width(&self) -> f64 {
        self.width
    }

    /// Half-open bin edges `[min + i*w, min + (i+1)*w)` for bin `i`.
    ///
    /// The last bin's right edge is inclusive (see [`Histogram`]). The
    /// returned pair always uses strictly ascending endpoints.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.bins().len()`.
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

/// Range and bin count shared by the `B_k = 0` and `B_k = 1` histograms of
/// every bit position, so exported distributions compare bin for bin.
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
/// MSB-first to match [`super::LabelWord`] and [`super::BitChannelId`].
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
    /// Mean of `|L_k|` over all samples (both conditional streams).
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
    /// Demapper method whose LLRs produced these statistics, or `None` if
    /// the accumulator was never stamped. The MI and GMI estimates read
    /// differently under the two methods (see [`gmi_bits`]).
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
    // Closed-form J-function fit `I = J(sigma)` cited from
    // `@/citation/TenBrink2001`, with sigma^2 = 2 * mean(L | bit=0) for
    // consistent Gaussian LLRs; `mean_abs_llr` stands in for
    // mu = mean(L | 0).
    let sigma = (2.0 * mean_abs_llr).sqrt();
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GmiMethod {
    /// Sum the closed-form Gaussian-approximation per-bit MI estimate
    /// (field
    /// [`PerBitChannelStats::mutual_info_bits_gaussian_approximation`]).
    ///
    /// Needs no histograms. **Not** a rigorous bound: it can
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
/// # Returns
///
/// `Some(mi_bits)` with `mi_bits` in `[0, 1]`, or `None` if either
/// histogram is absent, if the shared bin grid disagrees, or if either
/// conditional stream has no in-range samples.
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
/// Not internally synchronized: build one accumulator per worker and
/// combine them with [`PerBitLlrStats::merge`].
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
    /// Demap method of the consumed LLRs; `None` until stamped.
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
    /// # Panics
    ///
    /// Panics if `bits_per_symbol == 0` or `bits_per_symbol > 16`.
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

    /// Enables per-position conditional histograms that share `cfg`'s range
    /// and bin count.
    ///
    /// # Panics
    ///
    /// Panics if `cfg.min` or `cfg.max` is non-finite, or if
    /// `!(cfg.min < cfg.max)`.
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
    #[inline]
    pub fn bits_per_symbol(&self) -> u8 {
        self.bits_per_symbol
    }

    /// Returns the demapper method whose LLRs populated this accumulator,
    /// or `None` if it was never stamped.
    ///
    /// The stamp is copied into every [`PerBitChannelStats::demap_method`]
    /// and [`PerBitLlrStats::merge`] rejects a different one.
    #[inline]
    pub fn demap_method(&self) -> Option<super::DemapMethod> {
        self.demap_method
    }

    /// Stamps the accumulator with the demapper method whose LLRs will
    /// be accumulated; a matching re-stamp is a no-op.
    ///
    /// # Panics
    ///
    /// Panics if this accumulator carries a different method.
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
    /// # Panics
    ///
    /// Panics if `llrs.len() != truth_bits.len()` or if the length is
    /// not a multiple of `bits_per_symbol()`.
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
    /// # Panics
    ///
    /// Panics if `bits_per_symbol`, the histogram configurations, or two
    /// stamped demap methods differ.
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
        // One stamped side propagates its method to the merged accumulator.
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

    proptest! {
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

            let gmi_g = gmi_bits(&r, GmiMethod::GaussianApproximation);
            prop_assert!((0.0..=m as f64 + 1e-12).contains(&gmi_g),
                "gmi_bits (gaussian) {gmi_g} out of [0, m={m}]");
        }
    }

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
        let mut s = PerBitLlrStats::new(1);
        s.accumulate(&[Llr::new(2.0), Llr::new(-2.0)], &[false, true]);
        let r = s.report();
        assert!(per_bit_mi_histogram_bits(&r[0]).is_none());
    }

    #[test]
    fn test_gmi_gaussian_approximation_sums_per_bit_mi() {
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
