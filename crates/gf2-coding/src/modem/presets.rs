//! Preset [`ModemSpec`] constructors for BPSK and Gray square-QAM.
//!
//! This file ships the preset-side entry points. Custom constellations
//! are constructed via the public [`super::ModemSpecBuilder`]; both
//! paths funnel through the same validating
//! [`super::ModemSpec::from_parts_checked`] choke point.
//!
//! Bit-to-symbol mapping for Gray square-QAM (locked in plan §4.5):
//!
//! - `m = log2(order)` total bits; for `m >= 2` the first `m/2` MSBs form
//!   the I-axis Gray-PAM label (top-down by PAM significance) and the
//!   remaining `m/2` LSBs form the Q-axis label.
//! - For BPSK (`m = 1`): a single [`BitChannelSemantics::SingleAxisPam`]
//!   bit.
//! - Adjacent PAM levels differ in exactly one PAM-label bit.
//! - Unit average symbol energy; for `M = 2^m`-QAM over the symmetric grid
//!   `{±1, ±3, ..., ±(√M − 1)}` the scale factor is
//!   `sqrt(3 / (2·(M − 1)))`.
//!
//! The generic `*_with_scalar` variants let f64 research workflows reuse
//! the same presets.

use super::scalar::{DefaultScalar, ModemScalar};
use super::spec::{ModemSpec, ModemSpecParts};
use super::types::{
    BitChannelAnalysis, BitChannelSemantics, LabelWord, ModemCapabilities, Normalization,
    SymbolPoint,
};

/// Per-axis analysis entry for BPSK and QPSK, where each bit channel
/// lives on its own uncorrelated I- or Q-axis.
///
/// BPSK has one axis (I-only) and QPSK has two (one bit per axis), so
/// every bit is conditionally independent of every other bit in the
/// same symbol given the received sample under independent I/Q AWGN.
/// The LLR distribution is symmetric about zero and admits the exact
/// closed form `LLR = 4 y_axis / N0`.
const BPSK_QPSK_ANALYSIS: BitChannelAnalysis = BitChannelAnalysis {
    symmetric_llr_distribution: true,
    conditionally_independent: true,
    closed_form_llr_available: true,
};

/// Per-axis analysis entry for 16/64/256-QAM (Gray-coded square QAM).
///
/// Each axis carries `m/2` Gray-labelled PAM bits. Bits on *different*
/// axes are conditionally independent given the received sample (I and Q
/// noise are independent), but bits on the *same* axis are not: for
/// 4-PAM with Gray labels `00 → +3, 01 → +1, 11 → -1, 10 → -3`, the
/// posterior `P(b0, b1 | y)` does not generally factor as
/// `P(b0 | y) · P(b1 | y)` — equality only holds at `y = 0`. The
/// `conditionally_independent` field therefore cannot be advertised as
/// `true` for these presets. Symmetry still holds because the
/// constellation and labelling are symmetric about 0, and a
/// piecewise-linear closed-form max-log expression exists per PAM axis.
const QAM_MULTI_BIT_AXIS_ANALYSIS: BitChannelAnalysis = BitChannelAnalysis {
    symmetric_llr_distribution: true,
    conditionally_independent: false,
    closed_form_llr_available: true,
};

/// Per-bit-channel analysis arrays for the built-in presets.
///
/// Indexed by bit count; the slice length equals the preset's
/// `bits_per_symbol()`. BPSK (m=1) and QPSK (m=2) share
/// [`BPSK_QPSK_ANALYSIS`]; 16/64/256-QAM (m=4/6/8) share
/// [`QAM_MULTI_BIT_AXIS_ANALYSIS`] — see each constant's doc comment for
/// the analysis-facing reasoning.
const PRESET_ANALYSIS_M1: &[BitChannelAnalysis] = &[BPSK_QPSK_ANALYSIS; 1];
const PRESET_ANALYSIS_M2: &[BitChannelAnalysis] = &[BPSK_QPSK_ANALYSIS; 2];
const PRESET_ANALYSIS_M4: &[BitChannelAnalysis] = &[QAM_MULTI_BIT_AXIS_ANALYSIS; 4];
const PRESET_ANALYSIS_M6: &[BitChannelAnalysis] = &[QAM_MULTI_BIT_AXIS_ANALYSIS; 6];
const PRESET_ANALYSIS_M8: &[BitChannelAnalysis] = &[QAM_MULTI_BIT_AXIS_ANALYSIS; 8];

/// Returns the preset analysis slice for a given `bits_per_symbol`.
///
/// Single source of truth shared by every preset constructor so the
/// `bits_per_symbol` → `&'static [BitChannelAnalysis]` mapping is not
/// duplicated per constellation order.
const fn preset_analysis(bits_per_symbol: u8) -> &'static [BitChannelAnalysis] {
    match bits_per_symbol {
        1 => PRESET_ANALYSIS_M1,
        2 => PRESET_ANALYSIS_M2,
        4 => PRESET_ANALYSIS_M4,
        6 => PRESET_ANALYSIS_M6,
        8 => PRESET_ANALYSIS_M8,
        _ => panic!("preset_analysis: unsupported bits_per_symbol for built-in presets"),
    }
}

/// Returns the inverse-Gray decoding of `g` over `width` bits.
///
/// Equivalent to finding `k` such that `k ^ (k >> 1) == g`. Used by the
/// property tests that verify the Gray code round-trip.
#[cfg(test)]
#[inline]
fn inverse_gray(g: u32, width: u8) -> u32 {
    // Standard binary-reflected Gray inverse: fold in powers of two of
    // itself until all bits settle.
    let mut b = g;
    let mut shift = 1u32;
    while shift < width as u32 {
        b ^= b >> shift;
        shift *= 2;
    }
    b & ((1u32 << width) - 1)
}

/// Builds the PAM-label → signed level lookup for an `m`-bit Gray PAM.
///
/// The returned `Vec` is indexed by the raw `m`-bit PAM label (with MSB
/// corresponding to the most significant PAM bit / sign bit). The output
/// level is an odd integer in `{-(2^m - 1), -(2^m - 3), ..., +(2^m - 1)}`.
///
/// Ordering rule: position `k = 0..2^m` enumerated from the highest PAM
/// level (`+(2^m - 1)`) down to the lowest. The label at position `k` is
/// the standard binary-reflected Gray code `g(k) = k ^ (k >> 1)`. Under
/// this rule the MSB of the label is `0` for the top half (positive side)
/// and `1` for the bottom half (negative side), matching the design
/// expectation that the MSB is the "sign" PAM bit.
fn gray_pam_label_to_level(m: u8) -> Vec<i32> {
    let n = 1usize << m;
    let mut out = vec![0i32; n];
    for k in 0..n {
        // Gray code of k fits in m bits.
        let g = (k ^ (k >> 1)) as u32;
        // Level at position k (0 is top, n-1 is bottom): (2^m - 1) - 2*k.
        let level = (n as i32 - 1) - 2 * (k as i32);
        out[g as usize] = level;
    }
    out
}

/// Safe square-root of `order`, returning `sqrt(order)` as a `usize`.
///
/// Panics if `order` is not a perfect square.
fn isqrt_exact(order: usize) -> usize {
    let r = (order as f64).sqrt().round() as usize;
    assert!(
        r * r == order,
        "gray_square_qam: order {order} is not a perfect square"
    );
    r
}

/// Core builder for a Gray-coded square-QAM preset over any [`ModemScalar`].
fn build_gray_square_qam<S: ModemScalar>(order: usize) -> ModemSpec<S> {
    // Accept BPSK as a special case via this preset too.
    if order == 2 {
        return build_bpsk::<S>();
    }

    assert!(
        matches!(order, 4 | 16 | 64 | 256),
        "gray_square_qam: order must be one of 2, 4, 16, 64, 256 (got {order})"
    );

    let m_total = order.trailing_zeros() as u8; // log2(order)
    debug_assert!(m_total == 2 || m_total == 4 || m_total == 6 || m_total == 8);
    let m_half = m_total / 2;
    let sqrt_m = isqrt_exact(order); // 2^(m_total/2)
    let pam_levels = gray_pam_label_to_level(m_half);
    debug_assert_eq!(pam_levels.len(), sqrt_m);

    // Scale factor for unit average symbol energy:
    //     E_raw = 2 * (M - 1) / 3
    //     scale = sqrt(3 / (2 * (M - 1)))
    let scale_f64 = (3.0_f64 / (2.0 * (order as f64 - 1.0))).sqrt();
    let scale = S::from_f64(scale_f64);

    let n = 1usize << m_total;
    let mut points = Vec::with_capacity(n);
    let mut labels = Vec::with_capacity(n);
    let mask_half = (1u16 << m_half) - 1;

    for v in 0..n {
        let i_label = ((v >> m_half) as u16) & mask_half;
        let q_label = (v as u16) & mask_half;
        let i_level = pam_levels[i_label as usize];
        let q_level = pam_levels[q_label as usize];
        let i_f = S::from_f64(i_level as f64) * scale;
        let q_f = S::from_f64(q_level as f64) * scale;
        points.push(SymbolPoint::new(i_f, q_f));
        labels.push(LabelWord::new(v as u16, m_total));
    }

    // Bit-channel semantics: MSBs are I-axis PAM bits (top-down), LSBs are
    // Q-axis PAM bits (top-down). PAM-bit index 0 is the most significant
    // PAM bit.
    let mut bit_channels = Vec::with_capacity(m_total as usize);
    for k in 0..m_half {
        bit_channels.push(BitChannelSemantics::IAxisPam(k));
    }
    for k in 0..m_half {
        bit_channels.push(BitChannelSemantics::QAxisPam(k));
    }

    let parts = ModemSpecParts {
        points,
        labels,
        bits_per_symbol: m_total,
        bit_channels,
        normalization: Normalization::UnitAverageSymbolEnergy,
        normalization_scale: scale,
        capabilities: ModemCapabilities {
            supports_exact_log_map: true,
            supports_max_log: true,
            analysis: preset_analysis(m_total),
        },
    };
    ModemSpec::from_parts_checked(parts)
}

/// Core builder for the BPSK preset over any [`ModemScalar`].
fn build_bpsk<S: ModemScalar>() -> ModemSpec<S> {
    let one = S::one();
    let zero = S::zero();
    let points = vec![SymbolPoint::new(one, zero), SymbolPoint::new(-one, zero)];
    let labels = vec![LabelWord::new(0, 1), LabelWord::new(1, 1)];
    let bit_channels = vec![BitChannelSemantics::SingleAxisPam(0)];

    let parts = ModemSpecParts {
        points,
        labels,
        bits_per_symbol: 1,
        bit_channels,
        normalization: Normalization::UnitAverageSymbolEnergy,
        normalization_scale: one,
        capabilities: ModemCapabilities {
            supports_exact_log_map: true,
            supports_max_log: true,
            analysis: preset_analysis(1),
        },
    };
    ModemSpec::from_parts_checked(parts)
}

impl ModemSpec<DefaultScalar> {
    /// BPSK preset: `±1` on the I axis with unit symbol energy.
    ///
    /// Label mapping: bit `0` → `+1`, bit `1` → `-1`. The single bit
    /// position carries [`BitChannelSemantics::SingleAxisPam`]`(0)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::ModemSpec;
    ///
    /// let spec = ModemSpec::bpsk();
    /// assert_eq!(spec.num_symbols(), 2);
    /// assert_eq!(spec.bits_per_symbol(), 1);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn bpsk() -> Self {
        build_bpsk::<DefaultScalar>()
    }

    /// Gray-coded square-QAM preset.
    ///
    /// # Arguments
    ///
    /// * `order` - Constellation order; must be one of `2, 4, 16, 64, 256`.
    ///   `order = 2` is equivalent to [`ModemSpec::bpsk`].
    ///
    /// The label layout is locked: for `order >= 4` the first `m/2` MSBs
    /// form the I-axis Gray-PAM label (MSB = coarsest level) and the
    /// remaining `m/2` bits form the Q-axis Gray-PAM label, matching
    /// DVB-T2 EN 302 755 Table 14 bit-to-cell mapping. Points are stored
    /// post-normalized to unit average symbol energy.
    ///
    /// # Panics
    ///
    /// Panics if `order` is not one of the listed values.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::ModemSpec;
    ///
    /// let qpsk = ModemSpec::gray_square_qam(4);
    /// assert_eq!(qpsk.num_symbols(), 4);
    ///
    /// let q16 = ModemSpec::gray_square_qam(16);
    /// assert_eq!(q16.bits_per_symbol(), 4);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(M) in `order = M`.
    pub fn gray_square_qam(order: usize) -> Self {
        build_gray_square_qam::<DefaultScalar>(order)
    }
}

impl<S: ModemScalar> ModemSpec<S> {
    /// Scalar-generic companion of [`ModemSpec::bpsk`].
    ///
    /// Useful for f64 research workflows.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::ModemSpec;
    ///
    /// let spec: ModemSpec<f64> = ModemSpec::<f64>::bpsk_with_scalar();
    /// assert_eq!(spec.num_symbols(), 2);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn bpsk_with_scalar() -> Self {
        build_bpsk::<S>()
    }

    /// Scalar-generic companion of [`ModemSpec::gray_square_qam`].
    ///
    /// # Arguments
    ///
    /// * `order` - Constellation order; must be one of `2, 4, 16, 64, 256`.
    ///
    /// # Panics
    ///
    /// Panics if `order` is not one of the listed values.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::ModemSpec;
    ///
    /// let spec: ModemSpec<f64> = ModemSpec::<f64>::gray_square_qam_with_scalar(16);
    /// assert_eq!(spec.bits_per_symbol(), 4);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(M) in `order = M`.
    pub fn gray_square_qam_with_scalar(order: usize) -> Self {
        build_gray_square_qam::<S>(order)
    }
}

#[cfg(test)]
mod tests {
    use super::super::DefaultScalar;
    use super::*;

    fn all_orders() -> [usize; 5] {
        [2, 4, 16, 64, 256]
    }

    #[test]
    fn test_inverse_gray_roundtrip() {
        for width in 1u8..=8 {
            for k in 0u32..(1 << width) {
                let g = k ^ (k >> 1);
                assert_eq!(inverse_gray(g, width), k, "width={width}, k={k}");
            }
        }
    }

    #[test]
    fn test_gray_pam_levels_adjacent_differ_by_one_bit() {
        for m in 1u8..=4 {
            let lut = gray_pam_label_to_level(m);
            // Sort labels by level and check adjacent Hamming distance = 1.
            let mut pairs: Vec<(i32, u16)> = lut
                .iter()
                .enumerate()
                .map(|(lbl, &lvl)| (lvl, lbl as u16))
                .collect();
            pairs.sort_by_key(|p| p.0);
            for w in pairs.windows(2) {
                let d = (w[0].1 ^ w[1].1).count_ones();
                assert_eq!(d, 1, "Gray adjacency broken at m={m}, pair={w:?}");
            }
        }
    }

    #[test]
    fn test_preset_bpsk_basic() {
        let spec = ModemSpec::bpsk();
        assert_eq!(spec.num_symbols(), 2);
        assert_eq!(spec.bits_per_symbol(), 1);
        let v = spec.view();
        assert_eq!(v.point(0), SymbolPoint::<DefaultScalar>::new(1.0, 0.0));
        assert_eq!(v.point(1), SymbolPoint::<DefaultScalar>::new(-1.0, 0.0));
        assert_eq!(v.bit_channel(0), BitChannelSemantics::SingleAxisPam(0));
    }

    #[test]
    fn test_preset_gray_square_qam_counts() {
        for order in all_orders() {
            let spec = ModemSpec::gray_square_qam(order);
            assert_eq!(spec.num_symbols(), order, "num_symbols for order {order}");
            let m = order.trailing_zeros() as u8;
            assert_eq!(
                spec.bits_per_symbol(),
                m,
                "bits_per_symbol for order {order}"
            );
            assert!(spec.capabilities().supports_exact_log_map);
            assert!(spec.capabilities().supports_max_log);
        }
    }

    #[test]
    fn test_preset_labels_are_bijection() {
        for order in all_orders() {
            let spec = ModemSpec::gray_square_qam(order);
            let m = spec.bits_per_symbol();
            let mut seen = vec![false; order];
            for l in spec.view().labels() {
                assert_eq!(l.width, m);
                assert!(!seen[l.bits as usize]);
                seen[l.bits as usize] = true;
            }
            assert!(seen.iter().all(|b| *b));
        }
    }

    #[test]
    fn test_preset_unit_average_symbol_energy_f32() {
        for order in all_orders() {
            let spec = ModemSpec::gray_square_qam(order);
            let mut acc = 0.0_f64;
            for p in spec.view().points() {
                acc += (p.i as f64).powi(2) + (p.q as f64).powi(2);
            }
            let mean = acc / order as f64;
            assert!(
                (mean - 1.0).abs() < 1e-5,
                "order {order} f32 mean energy = {mean}"
            );
        }
    }

    #[test]
    fn test_preset_unit_average_symbol_energy_f64() {
        for order in all_orders() {
            let spec: ModemSpec<f64> = ModemSpec::<f64>::gray_square_qam_with_scalar(order);
            let mut acc = 0.0_f64;
            for p in spec.view().points() {
                acc += p.i * p.i + p.q * p.q;
            }
            let mean = acc / order as f64;
            assert!(
                (mean - 1.0).abs() < 1e-10,
                "order {order} f64 mean energy = {mean}"
            );
        }
    }

    #[test]
    fn test_preset_gray_axis_adjacency() {
        // For each Gray square-QAM preset, verify that adjacent I-axis
        // levels (same Q) differ in exactly one I-label bit, and similarly
        // for Q-axis.
        for order in [4usize, 16, 64, 256] {
            let spec = ModemSpec::gray_square_qam(order);
            let m = spec.bits_per_symbol();
            let m_half = m / 2;
            let mask_half = (1u16 << m_half) - 1;
            let sqrt_m = 1usize << m_half;

            // Group points by Q-axis label; check adjacency on the I axis.
            for q_label in 0u16..sqrt_m as u16 {
                let mut row: Vec<(f64, u16)> = Vec::with_capacity(sqrt_m);
                for (idx, l) in spec.view().labels().iter().enumerate() {
                    if (l.bits & mask_half) == q_label {
                        let i_bits = (l.bits >> m_half) & mask_half;
                        row.push((spec.view().point(idx).i as f64, i_bits));
                    }
                }
                row.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                for w in row.windows(2) {
                    let d = (w[0].1 ^ w[1].1).count_ones();
                    assert_eq!(
                        d, 1,
                        "I-axis Gray adjacency broken for order {order}, q_label {q_label}: {w:?}"
                    );
                }
            }
            for i_label in 0u16..sqrt_m as u16 {
                let mut col: Vec<(f64, u16)> = Vec::with_capacity(sqrt_m);
                for (idx, l) in spec.view().labels().iter().enumerate() {
                    if ((l.bits >> m_half) & mask_half) == i_label {
                        let q_bits = l.bits & mask_half;
                        col.push((spec.view().point(idx).q as f64, q_bits));
                    }
                }
                col.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                for w in col.windows(2) {
                    let d = (w[0].1 ^ w[1].1).count_ones();
                    assert_eq!(
                        d, 1,
                        "Q-axis Gray adjacency broken for order {order}, i_label {i_label}: {w:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_preset_qpsk_matches_legacy_layout() {
        // Legacy QpskModulator with delta = 1/sqrt(2) matches the new
        // Gray-square-QAM(4) preset exactly, confirming the shared
        // bit-to-point layout for the simulation migration path.
        let spec = ModemSpec::gray_square_qam(4);
        let delta = (0.5_f64).sqrt();
        // Label layout (MSB = I-bit, LSB = Q-bit):
        //   bit I = 0 -> I = +delta;  bit I = 1 -> I = -delta
        //   bit Q = 0 -> Q = +delta;  bit Q = 1 -> Q = -delta
        let expected = [
            (0b00_u16, delta, delta),
            (0b01_u16, delta, -delta),
            (0b10_u16, -delta, delta),
            (0b11_u16, -delta, -delta),
        ];
        for (label_bits, want_i, want_q) in expected {
            let idx = spec
                .view()
                .labels()
                .iter()
                .position(|l| l.bits == label_bits)
                .expect("label present");
            let p = spec.view().point(idx);
            assert!(
                (p.i as f64 - want_i).abs() < 1e-6,
                "I mismatch for label {label_bits:02b}: got {}, want {}",
                p.i,
                want_i
            );
            assert!(
                (p.q as f64 - want_q).abs() < 1e-6,
                "Q mismatch for label {label_bits:02b}: got {}, want {}",
                p.q,
                want_q
            );
        }
    }

    #[test]
    fn test_preset_bit_channel_semantics_layout() {
        let spec = ModemSpec::gray_square_qam(16);
        let bc = spec.view().bit_channels();
        assert_eq!(bc[0], BitChannelSemantics::IAxisPam(0));
        assert_eq!(bc[1], BitChannelSemantics::IAxisPam(1));
        assert_eq!(bc[2], BitChannelSemantics::QAxisPam(0));
        assert_eq!(bc[3], BitChannelSemantics::QAxisPam(1));
    }

    #[test]
    #[should_panic(expected = "order must be one of 2, 4, 16, 64, 256")]
    fn test_preset_invalid_order_panics() {
        let _ = ModemSpec::gray_square_qam(8);
    }

    #[test]
    fn test_preset_bpsk_bit_channel_analysis() {
        let spec = ModemSpec::bpsk();
        let caps = spec.capabilities();
        assert_eq!(caps.analysis.len(), spec.bits_per_symbol() as usize);
        for a in caps.analysis {
            assert!(a.symmetric_llr_distribution);
            assert!(a.conditionally_independent);
            assert!(a.closed_form_llr_available);
        }
    }

    #[test]
    fn test_preset_qpsk_bit_channel_analysis_all_flags_true() {
        // QPSK places exactly one bit per axis, so each bit is
        // conditionally independent of every other bit in the symbol.
        let spec = ModemSpec::gray_square_qam(4);
        let caps = spec.capabilities();
        assert_eq!(caps.analysis.len(), 2);
        for (k, a) in caps.analysis.iter().enumerate() {
            assert!(a.symmetric_llr_distribution, "QPSK bit {k}");
            assert!(a.conditionally_independent, "QPSK bit {k}");
            assert!(a.closed_form_llr_available, "QPSK bit {k}");
        }
    }

    #[test]
    fn test_preset_gray_square_qam_multi_bit_axis_not_conditionally_independent() {
        // 16/64/256-QAM carry multiple Gray-PAM bits per axis. Bits on
        // different axes are conditionally independent, but bits on the
        // same axis are not in general (P(b0,b1|y) != P(b0|y)P(b1|y)
        // except at y=0), so the flag must be advertised as `false`.
        // Symmetry and closed-form availability still hold.
        for order in [16usize, 64, 256] {
            let spec = ModemSpec::gray_square_qam(order);
            let caps = spec.capabilities();
            assert_eq!(
                caps.analysis.len(),
                spec.bits_per_symbol() as usize,
                "analysis len mismatch for order {order}"
            );
            for (k, a) in caps.analysis.iter().enumerate() {
                assert!(
                    a.symmetric_llr_distribution,
                    "order {order} bit {k} symmetric"
                );
                assert!(
                    !a.conditionally_independent,
                    "order {order} bit {k} must not claim conditional independence"
                );
                assert!(
                    a.closed_form_llr_available,
                    "order {order} bit {k} closed_form"
                );
            }
        }
    }
}
