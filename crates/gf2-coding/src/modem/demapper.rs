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
