//! Fast Gray square-QAM soft demapper exploiting I/Q axis separability.
//!
//! [`FastGrayQamDemapper`] is the specialized [`BatchSoftDemapper`] for
//! the Gray-coded square-QAM presets (and BPSK as the degenerate
//! single-axis case). Because under AWGN with independent I/Q noise the
//! per-symbol 2D log-MAP factorizes into two 1D Gray-PAM demaps of size
//! `sqrt(M)` each, the hot-path cost drops from
//! `O(num_symbols * M * m)` for the arbitrary-constellation
//! [`super::ReferenceSoftDemapper`] to
//! `O(num_symbols * sqrt(M) * m)`.
//!
//! # Separability with complex channel gains
//!
//! The demapper accepts the same optional `(gain_i, gain_q)` pair as
//! every other backend. For a complex gain `h = h_i + j h_q`, the
//! squared distance to the (complex) scaled constellation point `h * p`
//! can be rewritten by multiplying through by `conj(h)`:
//!
//! ```text
//! |y - h p|^2 / N0 = |z - |h|^2 p|^2 / (N0 |h|^2)
//! ```
//!
//! where `z = conj(h) * y`. Expanding the right-hand side yields a sum
//! of I-only and Q-only squared terms, so the separable kernel applies
//! after a per-symbol pre-rotation by `conj(h)` and a rescaling of the
//! PAM levels by `|h|^2`. With `gain_i = gain_q = None` this reduces to
//! the identity transform and the kernel falls back to the plain AWGN
//! path.
//!
//! # Bit ordering and sign convention
//!
//! Bit ordering follows the framework-wide MSB-first intra-symbol
//! convention: index `k = 0` is the MSB of the [`super::LabelWord`],
//! and for Gray square-QAM the first `m/2` MSBs are the I-axis Gray-PAM
//! label (MSB = coarsest level) while the remaining `m/2` bits are the
//! Q-axis label. LLR sign follows [`crate::llr::Llr`]: **positive LLR =
//! bit 0 more likely**, exactly matching
//! [`super::ReferenceSoftDemapper`].

use crate::llr::Llr;

use super::{
    BatchSoftDemapper, BitChannelSemantics, DemapInput, DemapMethod, ModemScalar, ModemSpec,
    ModemView,
};

/// Fast soft demapper specialized for Gray square-QAM presets.
///
/// Accepts BPSK (order 2, `m = 1`) and the Gray-coded square-QAM presets
/// of orders 4, 16, 64, and 256 (`m = 2, 4, 6, 8`). The constructor
/// validates the passed [`ModemSpec`] against the locked preset bit-channel
/// layout and panics on any non-matching spec; use
/// [`super::ReferenceSoftDemapper`] for arbitrary constellations.
///
/// # Examples
///
/// ```
/// use gf2_coding::llr::Llr;
/// use gf2_coding::modem::{
///     BatchSoftDemapper, DemapInput, DemapMethod, FastGrayQamDemapper, ModemSpec,
/// };
///
/// let spec = ModemSpec::<f32>::bpsk();
/// let demapper = FastGrayQamDemapper::new(spec);
/// let rx_i = [0.8_f32];
/// let rx_q = [0.0_f32];
/// let noise_var = [0.5_f32];
/// let input = DemapInput::<f32> {
///     rx_i: &rx_i,
///     rx_q: &rx_q,
///     gain_i: None,
///     gain_q: None,
///     noise_var: &noise_var,
///     method: DemapMethod::ExactLogMap,
/// };
/// let mut out = [Llr::new(0.0); 1];
/// demapper.demap_llrs(input, &mut out);
/// assert!(out[0].value() > 0.0);
/// ```
pub struct FastGrayQamDemapper<S: ModemScalar> {
    spec: ModemSpec<S>,
    /// Total bits per symbol (`log2(order)`).
    m_total: u8,
    /// Half bits per symbol for QAM (`m_total / 2`); `0` for BPSK.
    m_half: u8,
    /// `true` if this is the BPSK (single-axis) preset.
    is_bpsk: bool,
    /// Post-normalization Gray-PAM levels on each axis, indexed by the
    /// `m_half`-bit Gray label (MSB-first within the half-label). Length
    /// `1 << m_half` for QAM, or `2` for BPSK (indexed by the raw bit).
    pam_levels: Vec<f64>,
}

impl<S: ModemScalar> FastGrayQamDemapper<S> {
    /// Constructs a fast Gray-QAM demapper from a Gray-square-QAM preset.
    ///
    /// # Arguments
    ///
    /// * `spec` - A [`ModemSpec`] whose metadata advertises the
    ///   Gray-square-QAM layout. The canonical way to obtain one is
    ///   [`ModemSpec::bpsk`], [`ModemSpec::gray_square_qam`], or their
    ///   `*_with_scalar` variants. Custom [`super::ModemSpecBuilder`]
    ///   specs are accepted iff they advertise the same metadata shape
    ///   (see Panics) **and** carry canonical Gray square-PAM geometry
    ///   on each axis; the geometry itself is not verified here, so a
    ///   spec that spoofs the metadata but ships non-preset points
    ///   produces undefined LLRs. Prefer the preset constructors.
    ///
    /// # Panics
    ///
    /// Panics if the spec does not match a supported Gray-square-QAM
    /// preset layout: `bits_per_symbol` must be one of `1, 2, 4, 6, 8`,
    /// `num_symbols` must equal `2^bits_per_symbol`, the bit channels
    /// must follow the preset layout (`SingleAxisPam(0)` for BPSK;
    /// `m/2` `IAxisPam` entries followed by `m/2` `QAxisPam` entries in
    /// MSB-first order for QAM), and (for QAM) every composite symbol
    /// with the same I-half-label must share the same I coordinate so
    /// the I/Q factorisation is well-defined — mismatch panics with a
    /// descriptive message.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{FastGrayQamDemapper, ModemSpec};
    ///
    /// let demapper = FastGrayQamDemapper::new(ModemSpec::<f32>::gray_square_qam(64));
    /// assert_eq!(demapper.spec_ref().bits_per_symbol(), 6);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(M) in `order = M`.
    pub fn new(spec: ModemSpec<S>) -> Self {
        validate_preset_layout(&spec);

        let view = spec.view();
        let m_total = view.bits_per_symbol();
        let is_bpsk = m_total == 1;
        let (m_half, table_len) = if is_bpsk {
            (0u8, 2usize)
        } else {
            (m_total / 2, 1usize << (m_total / 2))
        };

        // Derive post-normalization PAM levels from the preset's points.
        // For BPSK the table is indexed by the raw bit: bit 0 -> +scale,
        // bit 1 -> -scale. For QAM the preset guarantees every composite
        // symbol with the same I-half-label shares the same I coordinate
        // (and analogously for Q), so we can read off levels from any
        // row / column.
        let mut pam_levels = vec![0.0_f64; table_len];
        if is_bpsk {
            pam_levels[0] = view.point(0).i.to_f64();
            pam_levels[1] = view.point(1).i.to_f64();
        } else {
            let mask_half = (1u16 << m_half) - 1;
            let mut filled = vec![false; table_len];
            // Secondary Q-axis level table for validation only. The demap
            // core reuses `pam_levels` for both axes, so we also verify
            // that the Q-axis factorisation uses the same level set and
            // that every Q-half-label resolves to that set consistently.
            let mut q_levels = vec![0.0_f64; table_len];
            let mut q_filled = vec![false; table_len];
            for (idx, label) in view.labels().iter().enumerate() {
                let i_label = ((label.bits >> m_half) & mask_half) as usize;
                let q_label = (label.bits & mask_half) as usize;
                let i_coord = view.point(idx).i.to_f64();
                let q_coord = view.point(idx).q.to_f64();
                if !filled[i_label] {
                    pam_levels[i_label] = i_coord;
                    filled[i_label] = true;
                } else {
                    let diff = (pam_levels[i_label] - i_coord).abs();
                    assert!(
                        diff < 1e-9,
                        "FastGrayQamDemapper::new: I-axis factorisation failed: \
                         symbols with I-half-label {i_label} have different I coordinates \
                         ({} vs {}); spec is not a canonical Gray square-QAM preset",
                        pam_levels[i_label],
                        i_coord,
                    );
                }
                if !q_filled[q_label] {
                    q_levels[q_label] = q_coord;
                    q_filled[q_label] = true;
                } else {
                    let diff = (q_levels[q_label] - q_coord).abs();
                    assert!(
                        diff < 1e-9,
                        "FastGrayQamDemapper::new: Q-axis factorisation failed: \
                         symbols with Q-half-label {q_label} have different Q coordinates \
                         ({} vs {}); spec is not a canonical Gray square-QAM preset",
                        q_levels[q_label],
                        q_coord,
                    );
                }
            }
            assert!(
                filled.iter().all(|b| *b),
                "FastGrayQamDemapper::new: spec does not cover every I-half-label; \
                 not a Gray square-QAM preset"
            );
            assert!(
                q_filled.iter().all(|b| *b),
                "FastGrayQamDemapper::new: spec does not cover every Q-half-label; \
                 not a Gray square-QAM preset"
            );
            // The axis-separable kernel uses the I-derived `pam_levels`
            // table for both I and Q distance computations:
            // `d_q[label] = (z_q - g * pam_levels[label])^2 / n0_eq`.
            // That is correct only when the per-label mapping matches
            // between axes, i.e. `q_levels[label] == pam_levels[label]`
            // for every label value. Set equality alone is not enough,
            // because a permuted Q-label mapping (same level set,
            // different label assignment) would silently produce wrong
            // Q-bit LLRs.
            for (label, (&il, &ql)) in pam_levels.iter().zip(q_levels.iter()).enumerate() {
                assert!(
                    (il - ql).abs() < 1e-9,
                    "FastGrayQamDemapper::new: Q-label-to-level mapping at label {label} \
                     ({ql}) does not match the I mapping ({il}); the fast kernel reuses the \
                     I level table for both axes and requires a symmetric preset"
                );
            }
        }

        Self {
            spec,
            m_total,
            m_half,
            is_bpsk,
            pam_levels,
        }
    }

    /// Returns a borrowed reference to the owned [`ModemSpec`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{FastGrayQamDemapper, ModemSpec};
    ///
    /// let demapper = FastGrayQamDemapper::new(ModemSpec::<f32>::gray_square_qam(16));
    /// assert_eq!(demapper.spec_ref().bits_per_symbol(), 4);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn spec_ref(&self) -> &ModemSpec<S> {
        &self.spec
    }
}

/// Validates that `spec` matches one of the Gray-square-QAM preset layouts.
///
/// Panics with a descriptive message if the spec is not BPSK or one of
/// the Gray square-QAM presets (orders 2/4/16/64/256 with canonical
/// I-axis-then-Q-axis MSB-first bit-channel layout).
fn validate_preset_layout<S: ModemScalar>(spec: &ModemSpec<S>) {
    let view = spec.view();
    let m = view.bits_per_symbol();
    assert!(
        matches!(m, 1 | 2 | 4 | 6 | 8),
        "FastGrayQamDemapper::new: bits_per_symbol {m} is not one of {{1, 2, 4, 6, 8}}"
    );
    let expected_symbols = 1usize << m;
    assert_eq!(
        view.num_symbols(),
        expected_symbols,
        "FastGrayQamDemapper::new: num_symbols {} does not match 2^bits_per_symbol {}",
        view.num_symbols(),
        expected_symbols
    );

    let bit_channels = view.bit_channels();
    assert_eq!(
        bit_channels.len(),
        m as usize,
        "FastGrayQamDemapper::new: bit_channels length {} != bits_per_symbol {}",
        bit_channels.len(),
        m
    );

    if m == 1 {
        assert_eq!(
            bit_channels[0],
            BitChannelSemantics::SingleAxisPam(0),
            "FastGrayQamDemapper::new: BPSK spec must advertise SingleAxisPam(0)"
        );
    } else {
        let m_half = m / 2;
        for k in 0..m_half {
            assert_eq!(
                bit_channels[k as usize],
                BitChannelSemantics::IAxisPam(k),
                "FastGrayQamDemapper::new: bit {k} must be IAxisPam({k}) for Gray square-QAM preset"
            );
        }
        for k in 0..m_half {
            assert_eq!(
                bit_channels[(m_half + k) as usize],
                BitChannelSemantics::QAxisPam(k),
                "FastGrayQamDemapper::new: bit {} must be QAxisPam({k}) for Gray square-QAM preset",
                m_half + k
            );
        }
    }

    let caps = view.capabilities();
    assert!(
        caps.supports_exact_log_map && caps.supports_max_log,
        "FastGrayQamDemapper::new: spec must advertise both ExactLogMap and MaxLog support"
    );
}

/// Computes the 1D Gray-PAM LLR for bit `bit_idx` (MSB-first within the
/// `m_bits`-wide half-label) given per-level squared distances `d`.
///
/// `d.len() == 1 << m_bits`, indexed by the raw Gray label. `method`
/// selects exact log-MAP or max-log. The subset-reduction math itself
/// lives in [`super::demapper::subset_log_map_llr`] — this is a thin
/// wrapper that supplies the "label = index" mapping used on a PAM
/// axis.
#[inline]
fn pam_axis_llr(d: &[f64], m_bits: u8, bit_idx: u8, method: DemapMethod) -> f64 {
    super::demapper::subset_log_map_llr(d, |j| j as u16, d.len(), m_bits, bit_idx, method)
}

impl<S: ModemScalar> BatchSoftDemapper<S> for FastGrayQamDemapper<S> {
    fn spec(&self) -> ModemView<'_, S> {
        self.spec.view()
    }

    fn demap_llrs(&self, input: DemapInput<'_, S>, out_llrs: &mut [Llr]) {
        let m = self.m_total as usize;
        let view = self.spec.view();
        let num_symbols = super::demapper::validate_demap_input(
            "FastGrayQamDemapper::demap_llrs",
            &view,
            &input,
            out_llrs.len(),
        );

        let axis_len = if self.is_bpsk {
            2
        } else {
            1usize << self.m_half
        };
        let mut d_i: Vec<f64> = vec![0.0; axis_len];
        let mut d_q: Vec<f64> = vec![0.0; axis_len];

        for k in 0..num_symbols {
            let y_i = input.rx_i[k].to_f64();
            let y_q = input.rx_q[k].to_f64();
            let (h_i, h_q) = match (input.gain_i, input.gain_q) {
                (Some(gi), Some(gq)) => (gi[k].to_f64(), gq[k].to_f64()),
                _ => (1.0_f64, 0.0_f64),
            };
            let n0 = input.noise_var[k].to_f64();
            assert!(
                n0 > 0.0 && n0.is_finite(),
                "FastGrayQamDemapper::demap_llrs: noise_var[{k}] = {n0} must be positive and finite"
            );

            // Pre-rotate by conj(h) so the kernel runs on axis-separable
            // data: |y - h p|^2 / n0 == |z - g p|^2 / (n0 * g) where
            // z = conj(h) * y and g = |h|^2.
            let g = h_i * h_i + h_q * h_q;
            let z_i = h_i * y_i + h_q * y_q;
            let z_q = h_i * y_q - h_q * y_i;
            let n0_eq = n0 * g;

            // Zero (or vanishingly small) channel gain: every
            // constellation point has identical squared distance from y,
            // so the log-MAP posterior degenerates to a label-count
            // ratio. For our balanced presets every bit channel carries
            // an equal count of 0- and 1-labels, so the LLR is exactly
            // zero. Emit zeros rather than dividing by n0_eq == 0 and
            // propagating NaNs. This matches the reference demapper's
            // behaviour at h = 0 on these presets.
            if n0_eq <= f64::EPSILON * n0 {
                for b in 0..m {
                    out_llrs[k * m + b] = Llr::new(0.0);
                }
                continue;
            }

            if self.is_bpsk {
                // BPSK: two I-axis points (±1), no Q contribution from
                // the constellation. The Q-only term (z_q^2 / n0_eq) is
                // a common additive constant across both points and
                // drops out of the LLR, so we ignore it.
                for (label, &level) in self.pam_levels.iter().enumerate() {
                    let e = z_i - g * level;
                    d_i[label] = e * e / n0_eq;
                }
                let llr = pam_axis_llr(&d_i, 1, 0, input.method);
                out_llrs[k * m] = Llr::new(llr as f32);
                continue;
            }

            // QAM: compute per-level distances on each axis.
            for (label, &level) in self.pam_levels.iter().enumerate() {
                let e_i = z_i - g * level;
                d_i[label] = e_i * e_i / n0_eq;
                let e_q = z_q - g * level;
                d_q[label] = e_q * e_q / n0_eq;
            }

            // First m_half bits are I-axis, remainder are Q-axis.
            for b in 0..self.m_half {
                let llr = pam_axis_llr(&d_i, self.m_half, b, input.method);
                out_llrs[k * m + b as usize] = Llr::new(llr as f32);
            }
            for b in 0..self.m_half {
                let llr = pam_axis_llr(&d_q, self.m_half, b, input.method);
                out_llrs[k * m + (self.m_half + b) as usize] = Llr::new(llr as f32);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        BatchSoftDemapper, DemapInput, DemapMethod, ModemSpec, ModemSpecBuilder, Normalization,
        ReferenceSoftDemapper, SymbolPoint,
    };
    use super::FastGrayQamDemapper;
    use crate::llr::Llr;
    use crate::modem::LabelWord;

    /// All supported preset orders.
    const PRESET_ORDERS: [usize; 5] = [2, 4, 16, 64, 256];

    fn method_seed(m: DemapMethod) -> u64 {
        match m {
            DemapMethod::ExactLogMap => 0xA1,
            DemapMethod::MaxLog => 0xB2,
        }
    }

    fn spec_for_order_f32(order: usize) -> ModemSpec<f32> {
        if order == 2 {
            ModemSpec::<f32>::bpsk()
        } else {
            ModemSpec::<f32>::gray_square_qam(order)
        }
    }

    fn spec_for_order_f64(order: usize) -> ModemSpec<f64> {
        if order == 2 {
            ModemSpec::<f64>::bpsk_with_scalar()
        } else {
            ModemSpec::<f64>::gray_square_qam_with_scalar(order)
        }
    }

    /// Deterministic LCG for test inputs (matches pattern used elsewhere
    /// in the crate so tests stay reproducible without a dev-dep RNG).
    struct Lcg {
        state: u64,
    }
    impl Lcg {
        fn new(seed: u64) -> Self {
            Self { state: seed }
        }
        fn next_u64(&mut self) -> u64 {
            self.state = self
                .state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.state
        }
        fn next_unit(&mut self) -> f64 {
            // Uniform in (-1, 1) using top 32 bits.
            let v = (self.next_u64() >> 32) as u32;
            (v as f64 / u32::MAX as f64) * 2.0 - 1.0
        }
        fn next_positive(&mut self, lo: f64, hi: f64) -> f64 {
            let v = (self.next_u64() >> 32) as u32;
            lo + (v as f64 / u32::MAX as f64) * (hi - lo)
        }
    }

    fn assert_close_f32(a: &[Llr], b: &[Llr], tol: f32, ctx: &str) {
        assert_eq!(a.len(), b.len(), "{ctx}: length mismatch");
        for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
            let dx = (x.value() - y.value()).abs();
            assert!(
                dx <= tol,
                "{ctx}: mismatch at {i}: fast={}, ref={}, |d|={dx}",
                x.value(),
                y.value()
            );
        }
    }

    #[test]
    fn test_fast_matches_reference_awgn_f32() {
        let methods = [DemapMethod::ExactLogMap, DemapMethod::MaxLog];
        for &order in &PRESET_ORDERS {
            for method in methods {
                let spec = spec_for_order_f32(order);
                let m = spec.bits_per_symbol() as usize;
                let fast = FastGrayQamDemapper::new(spec.clone());
                let reference = ReferenceSoftDemapper::new(spec);

                let mut rng = Lcg::new(0xDEADBEEF ^ (order as u64) ^ method_seed(method));
                let batch = 64usize;
                let mut rx_i = Vec::with_capacity(batch);
                let mut rx_q = Vec::with_capacity(batch);
                let mut nv = Vec::with_capacity(batch);
                for _ in 0..batch {
                    rx_i.push((rng.next_unit() * 2.0) as f32);
                    rx_q.push((rng.next_unit() * 2.0) as f32);
                    nv.push(rng.next_positive(0.05, 2.0) as f32);
                }

                let input = DemapInput::<f32> {
                    rx_i: &rx_i,
                    rx_q: &rx_q,
                    gain_i: None,
                    gain_q: None,
                    noise_var: &nv,
                    method,
                };
                let mut out_fast = vec![Llr::new(0.0); batch * m];
                let mut out_ref = vec![Llr::new(0.0); batch * m];
                fast.demap_llrs(input, &mut out_fast);
                reference.demap_llrs(input, &mut out_ref);
                assert_close_f32(
                    &out_fast,
                    &out_ref,
                    1e-3,
                    &format!("f32 AWGN order={order} method={method:?}"),
                );
            }
        }
    }

    #[test]
    fn test_fast_matches_reference_awgn_f64() {
        let methods = [DemapMethod::ExactLogMap, DemapMethod::MaxLog];
        for &order in &PRESET_ORDERS {
            for method in methods {
                let spec = spec_for_order_f64(order);
                let m = spec.bits_per_symbol() as usize;
                let fast = FastGrayQamDemapper::new(spec.clone());
                let reference = ReferenceSoftDemapper::new(spec);

                let mut rng = Lcg::new(0xC0FFEE ^ (order as u64) ^ method_seed(method));
                let batch = 64usize;
                let mut rx_i = Vec::with_capacity(batch);
                let mut rx_q = Vec::with_capacity(batch);
                let mut nv = Vec::with_capacity(batch);
                for _ in 0..batch {
                    rx_i.push(rng.next_unit() * 2.0);
                    rx_q.push(rng.next_unit() * 2.0);
                    nv.push(rng.next_positive(0.05, 2.0));
                }

                let input = DemapInput::<f64> {
                    rx_i: &rx_i,
                    rx_q: &rx_q,
                    gain_i: None,
                    gain_q: None,
                    noise_var: &nv,
                    method,
                };
                let mut out_fast = vec![Llr::new(0.0); batch * m];
                let mut out_ref = vec![Llr::new(0.0); batch * m];
                fast.demap_llrs(input, &mut out_fast);
                reference.demap_llrs(input, &mut out_ref);
                // Llr is f32-backed so tolerance is still f32-sized when
                // comparing values; drive it tighter than the f32 test.
                assert_close_f32(
                    &out_fast,
                    &out_ref,
                    1e-4,
                    &format!("f64 AWGN order={order} method={method:?}"),
                );
            }
        }
    }

    #[test]
    fn test_fast_matches_reference_with_complex_gain_f64() {
        // Exercise the conj(h) pre-rotation path with non-trivial fading.
        let methods = [DemapMethod::ExactLogMap, DemapMethod::MaxLog];
        for &order in &PRESET_ORDERS {
            for method in methods {
                let spec = spec_for_order_f64(order);
                let m = spec.bits_per_symbol() as usize;
                let fast = FastGrayQamDemapper::new(spec.clone());
                let reference = ReferenceSoftDemapper::new(spec);

                let mut rng = Lcg::new(0xFADED ^ (order as u64) ^ method_seed(method));
                let batch = 48usize;
                let mut rx_i = Vec::with_capacity(batch);
                let mut rx_q = Vec::with_capacity(batch);
                let mut gi = Vec::with_capacity(batch);
                let mut gq = Vec::with_capacity(batch);
                let mut nv = Vec::with_capacity(batch);
                for _ in 0..batch {
                    rx_i.push(rng.next_unit() * 2.0);
                    rx_q.push(rng.next_unit() * 2.0);
                    // Avoid near-zero |h| that would blow up the
                    // equalized noise variance.
                    let hi = if rng.next_unit() > 0.0 { 0.6 } else { -0.6 } + 0.3 * rng.next_unit();
                    let hq = 0.2 * rng.next_unit();
                    gi.push(hi);
                    gq.push(hq);
                    nv.push(rng.next_positive(0.05, 1.0));
                }

                let input = DemapInput::<f64> {
                    rx_i: &rx_i,
                    rx_q: &rx_q,
                    gain_i: Some(&gi),
                    gain_q: Some(&gq),
                    noise_var: &nv,
                    method,
                };
                let mut out_fast = vec![Llr::new(0.0); batch * m];
                let mut out_ref = vec![Llr::new(0.0); batch * m];
                fast.demap_llrs(input, &mut out_fast);
                reference.demap_llrs(input, &mut out_ref);
                assert_close_f32(
                    &out_fast,
                    &out_ref,
                    1e-3,
                    &format!("f64 fading order={order} method={method:?}"),
                );
            }
        }
    }

    #[test]
    fn test_zero_channel_gain_emits_zero_llrs() {
        // When h = 0 every constellation point has identical squared
        // distance from y, so the log-MAP LLR collapses to the
        // 0-bit/1-bit label count ratio. For balanced presets that
        // ratio is exactly 1, i.e. LLR = 0. The fast path must produce
        // finite zeros rather than NaN from dividing by |h|^2 == 0.
        for &order in &PRESET_ORDERS {
            let spec = spec_for_order_f64(order);
            let m = spec.bits_per_symbol() as usize;
            let fast = FastGrayQamDemapper::new(spec);
            let rx_i = [0.4_f64];
            let rx_q = [-0.3_f64];
            let gi = [0.0_f64];
            let gq = [0.0_f64];
            let nv = [0.5_f64];
            let input = DemapInput::<f64> {
                rx_i: &rx_i,
                rx_q: &rx_q,
                gain_i: Some(&gi),
                gain_q: Some(&gq),
                noise_var: &nv,
                method: DemapMethod::ExactLogMap,
            };
            let mut out = vec![Llr::new(0.0); m];
            fast.demap_llrs(input, &mut out);
            for (b, llr) in out.iter().enumerate() {
                assert!(
                    llr.value().is_finite(),
                    "order={order} bit={b}: LLR {} not finite at zero channel gain",
                    llr.value()
                );
                assert!(
                    llr.value().abs() < 1e-6,
                    "order={order} bit={b}: expected ~0 LLR at zero gain, got {}",
                    llr.value()
                );
            }
        }
    }

    #[test]
    fn test_bpsk_closed_form_high_snr() {
        // BPSK sanity: L = 4*y / N0 at unit gain. Smoke-tests the
        // single-axis branch beyond the cross-check against the
        // reference.
        let demapper = FastGrayQamDemapper::new(ModemSpec::<f32>::bpsk());
        let rx_i = [0.8_f32];
        let rx_q = [0.0_f32];
        let nv = [0.5_f32];
        let input = DemapInput::<f32> {
            rx_i: &rx_i,
            rx_q: &rx_q,
            gain_i: None,
            gain_q: None,
            noise_var: &nv,
            method: DemapMethod::ExactLogMap,
        };
        let mut out = [Llr::new(0.0); 1];
        demapper.demap_llrs(input, &mut out);
        let expected = 4.0 * 0.8 / 0.5;
        assert!(
            (out[0].value() - expected).abs() < 1e-4,
            "BPSK fast-path closed form mismatch: got {}, want {expected}",
            out[0].value()
        );
    }

    #[test]
    #[should_panic(expected = "rx_i.len()")]
    fn test_length_mismatch_rx_q_panics() {
        let demapper = FastGrayQamDemapper::new(ModemSpec::<f32>::gray_square_qam(4));
        let rx_i = [0.0_f32, 0.0];
        let rx_q = [0.0_f32];
        let nv = [0.5_f32, 0.5];
        let input = DemapInput::<f32> {
            rx_i: &rx_i,
            rx_q: &rx_q,
            gain_i: None,
            gain_q: None,
            noise_var: &nv,
            method: DemapMethod::ExactLogMap,
        };
        let mut out = [Llr::new(0.0); 4];
        demapper.demap_llrs(input, &mut out);
    }

    #[test]
    #[should_panic(expected = "out_llrs.len()")]
    fn test_length_mismatch_out_panics() {
        let demapper = FastGrayQamDemapper::new(ModemSpec::<f32>::gray_square_qam(4));
        let rx_i = [0.0_f32];
        let rx_q = [0.0_f32];
        let nv = [0.5_f32];
        let input = DemapInput::<f32> {
            rx_i: &rx_i,
            rx_q: &rx_q,
            gain_i: None,
            gain_q: None,
            noise_var: &nv,
            method: DemapMethod::ExactLogMap,
        };
        let mut out = [Llr::new(0.0); 3];
        demapper.demap_llrs(input, &mut out);
    }

    #[test]
    #[should_panic(expected = "gain_i and gain_q must be both Some or both None")]
    fn test_half_specified_gain_panics() {
        let demapper = FastGrayQamDemapper::new(ModemSpec::<f32>::gray_square_qam(4));
        let rx_i = [0.0_f32];
        let rx_q = [0.0_f32];
        let nv = [0.5_f32];
        let gi = [1.0_f32];
        let input = DemapInput::<f32> {
            rx_i: &rx_i,
            rx_q: &rx_q,
            gain_i: Some(&gi),
            gain_q: None,
            noise_var: &nv,
            method: DemapMethod::ExactLogMap,
        };
        let mut out = [Llr::new(0.0); 2];
        demapper.demap_llrs(input, &mut out);
    }

    #[test]
    #[should_panic(expected = "bits_per_symbol 3 is not one of")]
    fn test_unsupported_bits_per_symbol_panics() {
        // Build a custom 8-point (m=3) spec via the public builder: it is
        // valid as a research constellation but is not a Gray square-QAM
        // preset, so the fast path must reject it.
        let points: Vec<SymbolPoint<f32>> = (0..8)
            .map(|k| {
                let theta = (k as f32) * std::f32::consts::PI / 4.0;
                SymbolPoint::new(theta.cos(), theta.sin())
            })
            .collect();
        let labels: Vec<LabelWord> = (0u16..8).map(|b| LabelWord::new(b, 3)).collect();
        let spec = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(3)
            .points(points)
            .labels(labels)
            .normalization(Normalization::UnitAverageSymbolEnergy)
            .build();
        let _ = FastGrayQamDemapper::new(spec);
    }

    #[test]
    #[should_panic(expected = "Q-label-to-level mapping")]
    fn test_permuted_q_label_mapping_rejected() {
        // A 4-QAM spec with valid I/Q factorisation, matching level
        // sets ({+1, -1} on both axes), but a Q-label permutation that
        // disagrees with the I mapping: Q-half-label 0 maps to -1 while
        // the I mapping puts label 0 at +1. The fast kernel reuses the
        // I-derived level table for both axes, so this must be
        // rejected at construction.
        use crate::modem::{BitChannelSemantics, ModemCapabilities};
        let points: Vec<SymbolPoint<f32>> = vec![
            // Label 00 (I-half=0, Q-half=0): I=+1, but Q = -1 instead of +1.
            SymbolPoint::new(1.0, -1.0),
            // Label 01 (I-half=0, Q-half=1): I=+1, Q=+1.
            SymbolPoint::new(1.0, 1.0),
            // Label 10 (I-half=1, Q-half=0): I=-1, Q=-1.
            SymbolPoint::new(-1.0, -1.0),
            // Label 11 (I-half=1, Q-half=1): I=-1, Q=+1.
            SymbolPoint::new(-1.0, 1.0),
        ];
        let labels: Vec<LabelWord> = (0u16..4).map(|b| LabelWord::new(b, 2)).collect();
        let spec = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(2)
            .points(points)
            .labels(labels)
            .bit_channels(vec![
                BitChannelSemantics::IAxisPam(0),
                BitChannelSemantics::QAxisPam(0),
            ])
            .capabilities(ModemCapabilities {
                supports_exact_log_map: true,
                supports_max_log: true,
                analysis: &[],
            })
            .normalization(Normalization::ExplicitEs(2.0))
            .build();
        let _ = FastGrayQamDemapper::new(spec);
    }

    #[test]
    #[should_panic(expected = "Q-axis factorisation failed")]
    fn test_spoofed_qaxispam_metadata_with_non_preset_geometry_rejected() {
        // Symmetric of the I-axis case: valid IAxisPam/QAxisPam metadata
        // with I coordinates that pass factorisation but Q coordinates
        // that break it. Two symbols sharing Q-half-label 0b1 (raw
        // labels 0b01 and 0b11) must be rejected when they disagree on
        // their Q coordinate.
        use crate::modem::{BitChannelSemantics, ModemCapabilities};
        let points: Vec<SymbolPoint<f32>> = vec![
            SymbolPoint::new(1.0, 1.0),   // label 00
            SymbolPoint::new(1.0, -1.0),  // label 01 (Q-half = 1, Q = -1)
            SymbolPoint::new(-1.0, 1.0),  // label 10
            SymbolPoint::new(-1.0, -2.0), // label 11 (Q-half = 1, Q = -2)  <- mismatch
        ];
        let labels: Vec<LabelWord> = (0u16..4).map(|b| LabelWord::new(b, 2)).collect();
        let spec = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(2)
            .points(points)
            .labels(labels)
            .bit_channels(vec![
                BitChannelSemantics::IAxisPam(0),
                BitChannelSemantics::QAxisPam(0),
            ])
            .capabilities(ModemCapabilities {
                supports_exact_log_map: true,
                supports_max_log: true,
                analysis: &[],
            })
            .normalization(Normalization::ExplicitEs(1.875))
            .build();
        let _ = FastGrayQamDemapper::new(spec);
    }

    #[test]
    #[should_panic(expected = "I-axis factorisation failed")]
    fn test_spoofed_iaxispam_metadata_with_non_preset_geometry_rejected() {
        // Build a custom 4-point spec with IAxisPam/QAxisPam metadata
        // (so validate_preset_layout accepts it) but asymmetric geometry
        // that breaks the I/Q factorisation the fast path relies on.
        use crate::modem::{BitChannelSemantics, ModemCapabilities};
        let points: Vec<SymbolPoint<f32>> = vec![
            SymbolPoint::new(1.0, 1.0),
            SymbolPoint::new(1.0, -1.0),
            // Two symbols that share I-half-label 0b1 but have different
            // I coordinates break the factorisation and must be caught.
            SymbolPoint::new(-1.0, 1.0),
            SymbolPoint::new(-2.0, -1.0),
        ];
        let labels: Vec<LabelWord> = (0u16..4).map(|b| LabelWord::new(b, 2)).collect();
        let spec = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(2)
            .points(points)
            .labels(labels)
            .bit_channels(vec![
                BitChannelSemantics::IAxisPam(0),
                BitChannelSemantics::QAxisPam(0),
            ])
            .capabilities(ModemCapabilities {
                supports_exact_log_map: true,
                supports_max_log: true,
                analysis: &[],
            })
            .normalization(Normalization::ExplicitEs(2.5))
            .build();
        let _ = FastGrayQamDemapper::new(spec);
    }

    #[test]
    #[should_panic(expected = "for Gray square-QAM preset")]
    fn test_non_preset_layout_panics() {
        // 4-point spec with m=2 but non-Gray-QAM bit-channel layout is
        // rejected. Use the Opaque semantics.
        use crate::modem::BitChannelSemantics;
        let points: Vec<SymbolPoint<f32>> = vec![
            SymbolPoint::new(1.0, 0.0),
            SymbolPoint::new(0.0, 1.0),
            SymbolPoint::new(-1.0, 0.0),
            SymbolPoint::new(0.0, -1.0),
        ];
        let labels: Vec<LabelWord> = (0u16..4).map(|b| LabelWord::new(b, 2)).collect();
        let spec = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(2)
            .points(points)
            .labels(labels)
            .bit_channels(vec![
                BitChannelSemantics::Opaque(0),
                BitChannelSemantics::Opaque(1),
            ])
            .normalization(Normalization::UnitAverageSymbolEnergy)
            .build();
        let _ = FastGrayQamDemapper::new(spec);
    }
}

#[cfg(test)]
mod property_tests {
    //! Property-based coverage matching the sibling
    //! [`super::super::ref_demapper`] proptest surface: random inputs
    //! must not produce NaN and the fast path must agree with the
    //! reference demapper within tight tolerance across all preset
    //! orders and both demap methods.
    use super::super::{
        BatchSoftDemapper, DemapInput, DemapMethod, ModemSpec, ReferenceSoftDemapper,
    };
    use super::FastGrayQamDemapper;
    use crate::llr::Llr;
    use proptest::prelude::*;

    const PRESET_ORDERS: &[usize] = &[2, 4, 16, 64, 256];

    fn spec_for_order(order: usize) -> ModemSpec<f64> {
        if order == 2 {
            ModemSpec::<f64>::bpsk_with_scalar()
        } else {
            ModemSpec::<f64>::gray_square_qam_with_scalar(order)
        }
    }

    proptest! {
        #[test]
        fn prop_fast_matches_reference_on_random_awgn(
            order_idx in 0usize..PRESET_ORDERS.len(),
            method_max_log in any::<bool>(),
            n_sym in 1usize..24usize,
            y_seed in any::<u64>(),
            nv_base in 0.05f64..2.0f64,
        ) {
            let order = PRESET_ORDERS[order_idx];
            let method = if method_max_log {
                DemapMethod::MaxLog
            } else {
                DemapMethod::ExactLogMap
            };
            let spec = spec_for_order(order);
            let m = spec.bits_per_symbol() as usize;
            let fast = FastGrayQamDemapper::new(spec.clone());
            let reference = ReferenceSoftDemapper::new(spec);

            let mut state = y_seed | 1;
            let mut next = || {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                ((state >> 11) as f64 / ((1u64 << 53) as f64)) * 2.0 - 1.0
            };
            let rx_i: Vec<f64> = (0..n_sym).map(|_| next() * 1.5).collect();
            let rx_q: Vec<f64> = (0..n_sym).map(|_| next() * 1.5).collect();
            let nv: Vec<f64> = (0..n_sym).map(|i| nv_base + (i as f64) * 0.01).collect();

            let input = DemapInput::<f64> {
                rx_i: &rx_i,
                rx_q: &rx_q,
                gain_i: None,
                gain_q: None,
                noise_var: &nv,
                method,
            };
            let mut out_fast = vec![Llr::new(0.0); n_sym * m];
            let mut out_ref = vec![Llr::new(0.0); n_sym * m];
            fast.demap_llrs(input, &mut out_fast);
            reference.demap_llrs(input, &mut out_ref);
            for (f, r) in out_fast.iter().zip(out_ref.iter()) {
                prop_assert!(f.value().is_finite());
                prop_assert!((f.value() - r.value()).abs() < 1e-2);
            }
        }
    }
}
