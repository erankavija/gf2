//! Batched soft and hard demapper traits and their per-batch input.

use crate::llr::Llr;

use super::{DemapMethod, ModemScalar, ModemView};

/// Per-batch input to a soft or hard demapper.
///
/// `rx_i` defines `num_symbols`; every other slice has that length.
/// `gain_i` and `gain_q` are both `Some` or both `None`.
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
    /// Per-symbol total complex AWGN noise variance `N0 = 2 sigma^2`,
    /// where `sigma^2` is the variance on each of I and Q.
    pub noise_var: &'a [S],
    /// Which demap semantics to use.
    pub method: DemapMethod,
}

/// Per-bit log-MAP or max-log LLR from per-label noise-weighted squared
/// distances.
///
/// `label_bits(j)` is the MSB-first label of `distances[j]` for
/// `j < n_labels`; `bit_idx = 0` is the MSB.
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

/// Validates a [`DemapInput`] and output length against a modem view and
/// returns `num_symbols`.
///
/// # Panics
///
/// Panics, naming `backend_name`, on any length mismatch, half-specified
/// gains, or when `input.method` is not advertised by
/// `view.capabilities()`.
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
pub trait BatchSoftDemapper<S: ModemScalar> {
    /// Returns a borrowed view of the [`super::ModemSpec`] this demapper
    /// was constructed for.
    fn spec(&self) -> ModemView<'_, S>;

    /// Demaps a batch of received symbols into per-bit LLRs.
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
    fn demap_llrs(&self, input: DemapInput<'_, S>, out_llrs: &mut [Llr]);
}

/// Lets the boxed trait object returned by
/// [`super::ModemSpec::preferred_soft_demapper`] satisfy a
/// `D: BatchSoftDemapper<S>` bound.
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
/// [`BatchSoftDemapper::demap_llrs`].
///
/// # Output layout
///
/// For `num_symbols` received symbols and `bits_per_symbol = m`:
///
/// - `out_bits.len() == num_symbols * m`.
/// - Entry `out_bits[s * m + k]` is the hard decision for bit position `k`
///   of the `s`-th symbol, with `k = 0` being the MSB. `false` means bit
///   0, `true` means bit 1.
pub trait BatchHardDemapper<S: ModemScalar> {
    /// Returns a borrowed view of the [`super::ModemSpec`] this demapper
    /// was constructed for.
    fn spec(&self) -> ModemView<'_, S>;

    /// Demaps a batch of received symbols into hard bit decisions.
    ///
    /// # Panics
    ///
    /// Same length-checks as [`BatchSoftDemapper::demap_llrs`].
    fn demap_bits(&self, input: DemapInput<'_, S>, out_bits: &mut [bool]);
}
