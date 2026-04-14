//! Batched soft and hard demapper traits plus the per-batch input struct.
//!
//! These traits are the public, backend-agnostic interface that the
//! arbitrary-constellation reference path, the Gray square-QAM fast path,
//! SIMD kernels, and any future GPU backend implement. They are
//! deliberately minimal: the hot path consumes a [`DemapInput`] and
//! writes into a caller-provided output slice.
//!
//! # Zero-overhead analysis
//!
//! [`DemapInput`] intentionally carries no analysis flags, histogram
//! knobs, or mutual-information fields. Bit-channel analysis
//! (task `e2c0f65a`) composes around these traits rather than threading
//! observability into the hot demap loop. This is the load-bearing design
//! decision documented in
//! `dev/active/d4851c3d-modem-framework-design.md` under
//! "Zero-overhead analysis split".
//!
//! Concrete backends land in tasks `51334873` (reference path) and
//! `52112411` (Gray-QAM fast path).

use crate::llr::Llr;

use super::{DemapMethod, ModemScalar, ModemView};

/// Per-batch input to a soft or hard demapper.
///
/// Backend-agnostic and `Copy` so backends can freely pass it through
/// internal helpers without lifetime plumbing. All slices except the
/// optional channel-gain pair have length `num_symbols`, the number of
/// received symbols in the batch.
///
/// # Fields
///
/// * `rx_i` / `rx_q` - In-phase and quadrature components of the received
///   samples, one element per symbol. Lengths must be equal and define
///   `num_symbols`.
/// * `gain_i` / `gain_q` - Optional per-symbol complex channel tap split
///   into real and imaginary parts. Pass `None` for AWGN. When provided,
///   both slices must be `Some` and have length `num_symbols`;
///   implementations panic on half-specified gains.
/// * `noise_var` - Per-symbol total complex AWGN noise variance
///   `N0 = 2 sigma^2`. For real AWGN with independent Gaussian noise of
///   variance `sigma^2` on each of I and Q, pass `2 * sigma^2` here.
///   This matches the log-MAP LLR formulas
///   `LLR = log(p(y|bit=0)/p(y|bit=1))` with per-point distances scaled
///   by `1/N0`. Length `num_symbols`.
/// * `method` - Selected demapper semantics; implementations must reject
///   methods not advertised by the [`super::ModemSpec`]'s
///   [`super::ModemCapabilities`].
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::{DemapInput, DemapMethod};
///
/// let rx_i = [0.5_f32, -0.5];
/// let rx_q = [0.5_f32, -0.5];
/// let noise_var = [0.1_f32, 0.1];
/// let input = DemapInput::<f32> {
///     rx_i: &rx_i,
///     rx_q: &rx_q,
///     gain_i: None,
///     gain_q: None,
///     noise_var: &noise_var,
///     method: DemapMethod::MaxLog,
/// };
/// assert_eq!(input.rx_i.len(), 2);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct DemapInput<'a, S: ModemScalar> {
    /// In-phase component of each received symbol. Length `num_symbols`.
    pub rx_i: &'a [S],
    /// Quadrature component of each received symbol. Length `num_symbols`.
    pub rx_q: &'a [S],
    /// Optional in-phase channel-gain component for fading channels.
    /// `None` signals AWGN (implicit unit gain).
    pub gain_i: Option<&'a [S]>,
    /// Optional quadrature channel-gain component for fading channels.
    /// `None` signals AWGN (implicit unit gain).
    pub gain_q: Option<&'a [S]>,
    /// Per-symbol total complex AWGN noise variance `N0 = 2 sigma^2`.
    /// For real AWGN with independent Gaussian noise of variance
    /// `sigma^2` on each of I and Q, pass `2 * sigma^2` here. This
    /// matches the log-MAP LLR formulas
    /// `LLR = log(p(y|bit=0)/p(y|bit=1))` with per-point distances
    /// scaled by `1/N0`. Length `num_symbols`.
    pub noise_var: &'a [S],
    /// Which demap semantics to use.
    pub method: DemapMethod,
}

/// Computes the per-bit log-MAP or max-log LLR given per-label squared
/// distances, a label accessor, and the demap method.
///
/// The core subset-reduction pattern used by both the arbitrary-
/// constellation reference path (which walks every `LabelWord` in a
/// `ModemSpec`) and the Gray square-QAM fast path (which walks the
/// `2^m_bits` implicit labels of a PAM axis). Kept here so neither
/// backend reimplements the min-shift + log-sum-exp math.
///
/// # Arguments
///
/// * `distances` - Per-label noise-weighted squared distances,
///   `distances[j]` giving the distance for label `label_bits(j)`.
/// * `label_bits` - Returns the MSB-first `u16` label for index `j`.
///   For the reference path this reads `labels[j].bits`; for the fast
///   path the label is the index itself.
/// * `n_labels` - Total number of labels (`distances.len()`).
/// * `bits_per_symbol` - Label width in bits.
/// * `bit_idx` - Bit position (MSB-first; `0` is the MSB).
/// * `method` - Exact log-MAP or max-log.
///
/// # Complexity
///
/// O(`n_labels`).
#[inline]
pub(crate) fn subset_log_map_llr(
    distances: &[f64],
    label_bits: impl Fn(usize) -> u16,
    n_labels: usize,
    bits_per_symbol: u8,
    bit_idx: u8,
    method: DemapMethod,
) -> f64 {
    use super::bit_pack::bit_at_msb_first;
    let mut d_min0 = f64::INFINITY;
    let mut d_min1 = f64::INFINITY;
    for (j, &dj) in distances.iter().enumerate().take(n_labels) {
        let bit = bit_at_msb_first(label_bits(j), bit_idx, bits_per_symbol);
        if bit == 0 {
            if dj < d_min0 {
                d_min0 = dj;
            }
        } else if dj < d_min1 {
            d_min1 = dj;
        }
    }
    match method {
        DemapMethod::MaxLog => -d_min0 + d_min1,
        DemapMethod::ExactLogMap => {
            let mut sum0 = 0.0_f64;
            let mut sum1 = 0.0_f64;
            for (j, &dj) in distances.iter().enumerate().take(n_labels) {
                let bit = bit_at_msb_first(label_bits(j), bit_idx, bits_per_symbol);
                if bit == 0 {
                    sum0 += (d_min0 - dj).exp();
                } else {
                    sum1 += (d_min1 - dj).exp();
                }
            }
            let log0 = if sum0 > 0.0 {
                -d_min0 + sum0.ln()
            } else {
                f64::NEG_INFINITY
            };
            let log1 = if sum1 > 0.0 {
                -d_min1 + sum1.ln()
            } else {
                f64::NEG_INFINITY
            };
            log0 - log1
        }
    }
}

/// Validates a [`DemapInput`] and output slice against a modem view.
///
/// Shared pre-flight check used by every `BatchSoftDemapper`
/// implementation so the trait's length and capability contracts live in
/// exactly one place. The caller passes its own `backend_name` so panic
/// messages still identify which backend rejected the input.
///
/// # Arguments
///
/// * `backend_name` - Short string embedded in panic messages (e.g.
///   `"ReferenceSoftDemapper::demap_llrs"`).
/// * `view` - Borrowed modem view used to read `bits_per_symbol` and
///   `capabilities`.
/// * `input` - The [`DemapInput`] to validate.
/// * `out_llrs_len` - Length of the caller's destination slice.
///
/// # Returns
///
/// `num_symbols == input.rx_i.len()` for convenience.
///
/// # Panics
///
/// Panics with a descriptive message on any length mismatch,
/// half-specified gains, or when `input.method` is not advertised by
/// `view.capabilities()`.
///
/// # Complexity
///
/// O(1).
pub(crate) fn validate_demap_input<S: ModemScalar>(
    backend_name: &str,
    view: &ModemView<'_, S>,
    input: &DemapInput<'_, S>,
    out_llrs_len: usize,
) -> usize {
    let m = view.bits_per_symbol() as usize;
    let num_symbols = input.rx_i.len();
    assert_eq!(
        input.rx_q.len(),
        num_symbols,
        "{backend_name}: rx_i.len() ({}) != rx_q.len() ({})",
        num_symbols,
        input.rx_q.len()
    );
    assert_eq!(
        input.noise_var.len(),
        num_symbols,
        "{backend_name}: rx_i.len() ({}) != noise_var.len() ({})",
        num_symbols,
        input.noise_var.len()
    );
    match (input.gain_i, input.gain_q) {
        (Some(gi), Some(gq)) => {
            assert_eq!(
                gi.len(),
                num_symbols,
                "{backend_name}: gain_i.len() ({}) != num_symbols ({})",
                gi.len(),
                num_symbols
            );
            assert_eq!(
                gq.len(),
                num_symbols,
                "{backend_name}: gain_q.len() ({}) != num_symbols ({})",
                gq.len(),
                num_symbols
            );
        }
        (None, None) => {}
        _ => panic!("{backend_name}: gain_i and gain_q must be both Some or both None"),
    }
    assert_eq!(
        out_llrs_len,
        num_symbols * m,
        "{backend_name}: out_llrs.len() ({}) != num_symbols * bits_per_symbol ({})",
        out_llrs_len,
        num_symbols * m
    );

    let caps = view.capabilities();
    match input.method {
        DemapMethod::ExactLogMap => assert!(
            caps.supports_exact_log_map,
            "{backend_name}: spec does not advertise ExactLogMap support"
        ),
        DemapMethod::MaxLog => assert!(
            caps.supports_max_log,
            "{backend_name}: spec does not advertise MaxLog support"
        ),
    }
    num_symbols
}

/// Batched soft (LLR) demapper.
///
/// Implementations compute one LLR per bit per received symbol and write
/// them into a caller-provided slice, in the canonical symbol-major,
/// MSB-first-within-symbol order used by the rest of the modem framework.
///
/// # Output layout
///
/// For `num_symbols` received symbols and `bits_per_symbol = m`:
///
/// - `out_llrs.len() == num_symbols * m`.
/// - Entry `out_llrs[s * m + k]` is the LLR of bit position `k` of the
///   `s`-th received symbol, with `k = 0` being the MSB under the
///   [`super::LabelWord`] convention.
/// - LLR sign convention matches [`Llr`]: positive means bit 0 is more
///   likely.
///
/// # Examples
///
/// ```no_run
/// use gf2_coding::llr::Llr;
/// use gf2_coding::modem::{
///     BatchSoftDemapper, DemapInput, DemapMethod, ModemScalar,
/// };
///
/// fn run_soft<S: ModemScalar, D: BatchSoftDemapper<S>>(
///     demapper: &D,
///     input: DemapInput<'_, S>,
///     out: &mut [Llr],
/// ) {
///     let _caps = demapper.spec().capabilities();
///     demapper.demap_llrs(input, out);
/// }
///
/// # fn main() {}
/// ```
pub trait BatchSoftDemapper<S: ModemScalar> {
    /// Returns a borrowed view of the [`super::ModemSpec`] this demapper
    /// was constructed for.
    ///
    /// # Complexity
    ///
    /// O(1).
    fn spec(&self) -> ModemView<'_, S>;

    /// Demaps a batch of received symbols into per-bit LLRs.
    ///
    /// # Arguments
    ///
    /// * `input` - Received samples, optional channel gains, per-symbol
    ///   noise variance, and the chosen demap method.
    /// * `out_llrs` - Destination slice; length must equal
    ///   `num_symbols * bits_per_symbol`, where `num_symbols == rx_i.len()`.
    ///   Layout is symbol-major, MSB-first within each symbol.
    ///
    /// # Panics
    ///
    /// Implementations must panic with a descriptive message if:
    ///
    /// - `rx_i.len() != rx_q.len()` or `rx_i.len() != noise_var.len()`;
    /// - exactly one of `gain_i` / `gain_q` is `Some(_)`, or a provided
    ///   gain slice has a length different from `rx_i.len()`;
    /// - `out_llrs.len() != num_symbols * bits_per_symbol`;
    /// - the selected [`DemapMethod`] is not advertised by
    ///   [`super::ModemSpec::capabilities`].
    ///
    /// # Complexity
    ///
    /// Implementation-dependent: the exact log-MAP reference path is
    /// `O(num_symbols * num_constellation_points * m)`; the Gray-QAM fast
    /// path is `O(num_symbols * m)`.
    fn demap_llrs(&self, input: DemapInput<'_, S>, out_llrs: &mut [Llr]);
}

/// Blanket implementation so `Box<dyn BatchSoftDemapper<S> + Send + Sync>`
/// (the return type of [`super::ModemSpec::preferred_soft_demapper`])
/// can be passed anywhere a `D: BatchSoftDemapper<S>` is required —
/// notably to [`super::ModemChannelAdapter::new`] without the caller
/// having to unwrap the box or name the concrete backend.
impl<S: ModemScalar, T: BatchSoftDemapper<S> + ?Sized> BatchSoftDemapper<S> for Box<T> {
    #[inline]
    fn spec(&self) -> ModemView<'_, S> {
        (**self).spec()
    }

    #[inline]
    fn demap_llrs(&self, input: DemapInput<'_, S>, out_llrs: &mut [Llr]) {
        (**self).demap_llrs(input, out_llrs);
    }
}

/// Batched hard demapper.
///
/// Emits bit decisions (`bool`) rather than LLRs, in the same layout as
/// [`BatchSoftDemapper::demap_llrs`]. Useful for uncoded-BER benchmarking
/// and simple receivers that do not feed a soft decoder.
///
/// # Output layout
///
/// For `num_symbols` received symbols and `bits_per_symbol = m`:
///
/// - `out_bits.len() == num_symbols * m`.
/// - Entry `out_bits[s * m + k]` is the hard decision for bit position `k`
///   of the `s`-th symbol, with `k = 0` being the MSB. `false` means bit
///   0, `true` means bit 1.
///
/// # Examples
///
/// ```no_run
/// use gf2_coding::modem::{
///     BatchHardDemapper, DemapInput, DemapMethod, ModemScalar,
/// };
///
/// fn run_hard<S: ModemScalar, D: BatchHardDemapper<S>>(
///     demapper: &D,
///     input: DemapInput<'_, S>,
///     out: &mut [bool],
/// ) {
///     let _view = demapper.spec();
///     demapper.demap_bits(input, out);
/// }
///
/// # fn main() {}
/// ```
pub trait BatchHardDemapper<S: ModemScalar> {
    /// Returns a borrowed view of the [`super::ModemSpec`] this demapper
    /// was constructed for.
    ///
    /// # Complexity
    ///
    /// O(1).
    fn spec(&self) -> ModemView<'_, S>;

    /// Demaps a batch of received symbols into hard bit decisions.
    ///
    /// # Arguments
    ///
    /// * `input` - Received samples, optional channel gains, per-symbol
    ///   noise variance, and the chosen demap method.
    /// * `out_bits` - Destination slice; length must equal
    ///   `num_symbols * bits_per_symbol`, symbol-major, MSB-first within
    ///   each symbol.
    ///
    /// # Panics
    ///
    /// Same length-checks as [`BatchSoftDemapper::demap_llrs`].
    ///
    /// # Complexity
    ///
    /// Implementation-dependent; matches the soft-demapper variant for
    /// the same backend.
    fn demap_bits(&self, input: DemapInput<'_, S>, out_bits: &mut [bool]);
}
