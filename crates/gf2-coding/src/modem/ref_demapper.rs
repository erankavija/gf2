//! Correctness-first, backend-agnostic reference soft demapper.
//!
//! [`ReferenceSoftDemapper`] is the arbitrary-constellation path of the
//! modem framework for soft (LLR) demapping. It works for any validated
//! [`ModemSpec`] — Gray-QAM presets as well as custom research
//! constellations built via [`super::ModemSpecBuilder`] — by iterating
//! over every constellation point for every received sample and every
//! bit position. This makes it the slow-but-audit-friendly reference
//! against which the Gray-QAM fast path and SIMD kernels are checked.
//!
//! Bit ordering follows the framework-wide MSB-first intra-symbol
//! convention: index `k = 0` is the MSB of the [`super::LabelWord`].
//! LLR sign follows [`crate::llr::Llr`]: **positive LLR = bit 0 more
//! likely**.

use crate::llr::Llr;

use super::{BatchSoftDemapper, DemapInput, DemapMethod, ModemScalar, ModemSpec, ModemView};

/// Correctness-first soft demapper for any validated [`ModemSpec`].
///
/// Computes per-bit LLRs via the exact log-MAP formula or the max-log
/// approximation by enumerating every constellation point for every
/// received symbol. Arithmetic is performed in [`f64`] internally for
/// numerical stability; the final LLRs are cast to [`f32`] (the storage
/// format of [`crate::llr::Llr`]).
///
/// For the optimized Gray square-QAM fast path, see the sibling demapper
/// implementation (task `52112411`).
///
/// # LLR sign convention
///
/// Positive LLR means bit `0` is more likely. This matches
/// [`crate::llr::Llr`] and the existing `QpskModulator::symbols_to_llrs`.
///
/// # Examples
///
/// ```
/// use gf2_coding::llr::Llr;
/// use gf2_coding::modem::{
///     BatchSoftDemapper, DemapInput, DemapMethod, ModemSpec, ReferenceSoftDemapper,
/// };
///
/// let spec = ModemSpec::<f32>::bpsk();
/// let demapper = ReferenceSoftDemapper::new(spec);
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
pub struct ReferenceSoftDemapper<S: ModemScalar> {
    spec: ModemSpec<S>,
}

impl<S: ModemScalar> ReferenceSoftDemapper<S> {
    /// Takes ownership of a validated [`ModemSpec`] for use as the
    /// constellation table.
    ///
    /// # Arguments
    ///
    /// * `spec` - The owned, validated modem specification. Presets
    ///   ([`ModemSpec::bpsk`], [`ModemSpec::gray_square_qam`]) and
    ///   builder-produced specs are both accepted.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{ModemSpec, ReferenceSoftDemapper};
    ///
    /// let demapper = ReferenceSoftDemapper::new(ModemSpec::<f32>::bpsk());
    /// assert_eq!(demapper.spec_ref().num_symbols(), 2);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn new(spec: ModemSpec<S>) -> Self {
        Self { spec }
    }

    /// Returns a borrowed reference to the owned [`ModemSpec`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{ModemSpec, ReferenceSoftDemapper};
    ///
    /// let demapper = ReferenceSoftDemapper::new(ModemSpec::<f32>::gray_square_qam(16));
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

/// Extracts bit `b` (MSB-first, `b = 0` is the MSB) of an `m`-bit label.
#[inline]
fn label_bit_msb_first(label_bits: u16, bit_idx: u8, bits_per_symbol: u8) -> u16 {
    let shift = bits_per_symbol - 1 - bit_idx;
    (label_bits >> shift) & 1
}

impl<S: ModemScalar> BatchSoftDemapper<S> for ReferenceSoftDemapper<S> {
    fn spec(&self) -> ModemView<'_, S> {
        self.spec.view()
    }

    fn demap_llrs(&self, input: DemapInput<'_, S>, out_llrs: &mut [Llr]) {
        let view = self.spec.view();
        let m = view.bits_per_symbol() as usize;
        let num_symbols = input.rx_i.len();

        assert_eq!(
            input.rx_q.len(),
            num_symbols,
            "ReferenceSoftDemapper::demap_llrs: rx_i.len() ({}) != rx_q.len() ({})",
            num_symbols,
            input.rx_q.len()
        );
        assert_eq!(
            input.noise_var.len(),
            num_symbols,
            "ReferenceSoftDemapper::demap_llrs: rx_i.len() ({}) != noise_var.len() ({})",
            num_symbols,
            input.noise_var.len()
        );
        match (input.gain_i, input.gain_q) {
            (Some(gi), Some(gq)) => {
                assert_eq!(
                    gi.len(),
                    num_symbols,
                    "ReferenceSoftDemapper::demap_llrs: gain_i.len() ({}) != num_symbols ({})",
                    gi.len(),
                    num_symbols
                );
                assert_eq!(
                    gq.len(),
                    num_symbols,
                    "ReferenceSoftDemapper::demap_llrs: gain_q.len() ({}) != num_symbols ({})",
                    gq.len(),
                    num_symbols
                );
            }
            (None, None) => {}
            _ => panic!(
                "ReferenceSoftDemapper::demap_llrs: gain_i and gain_q must be both Some or both None"
            ),
        }
        assert_eq!(
            out_llrs.len(),
            num_symbols * m,
            "ReferenceSoftDemapper::demap_llrs: out_llrs.len() ({}) != num_symbols * bits_per_symbol ({})",
            out_llrs.len(),
            num_symbols * m
        );

        let caps = view.capabilities();
        match input.method {
            DemapMethod::ExactLogMap => assert!(
                caps.supports_exact_log_map,
                "ReferenceSoftDemapper::demap_llrs: spec does not advertise ExactLogMap support"
            ),
            DemapMethod::MaxLog => assert!(
                caps.supports_max_log,
                "ReferenceSoftDemapper::demap_llrs: spec does not advertise MaxLog support"
            ),
        }

        let points = view.points();
        let labels = view.labels();
        let n_points = points.len();
        let bits_per_symbol = view.bits_per_symbol();

        // Scratch buffer: noise-weighted squared distance per point.
        let mut d: Vec<f64> = vec![0.0; n_points];

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
                "ReferenceSoftDemapper::demap_llrs: noise_var[{k}] = {n0} must be positive and finite"
            );

            // Compute noise-weighted squared distances for every point.
            for (j, p) in points.iter().enumerate() {
                let p_i = p.i.to_f64();
                let p_q = p.q.to_f64();
                let ei = y_i - (h_i * p_i - h_q * p_q);
                let eq = y_q - (h_i * p_q + h_q * p_i);
                d[j] = (ei * ei + eq * eq) / n0;
            }

            for b in 0..m {
                let llr = match input.method {
                    DemapMethod::ExactLogMap => {
                        exact_log_map_llr(&d, labels, bits_per_symbol, b as u8)
                    }
                    DemapMethod::MaxLog => max_log_llr(&d, labels, bits_per_symbol, b as u8),
                };
                out_llrs[k * m + b] = Llr::new(llr as f32);
            }
        }
    }
}

/// Exact log-MAP LLR for bit position `b` (MSB-first).
///
/// Returns `log(sum_{j in S0} exp(-d_j)) - log(sum_{j in S1} exp(-d_j))`,
/// computed stably by subtracting the per-set minimum distance before
/// exponentiating. Positive result means bit `0` is more likely.
fn exact_log_map_llr(d: &[f64], labels: &[super::LabelWord], bits_per_symbol: u8, b: u8) -> f64 {
    let mut d_min0 = f64::INFINITY;
    let mut d_min1 = f64::INFINITY;
    for (j, lbl) in labels.iter().enumerate() {
        let bit = label_bit_msb_first(lbl.bits, b, bits_per_symbol);
        if bit == 0 {
            if d[j] < d_min0 {
                d_min0 = d[j];
            }
        } else if d[j] < d_min1 {
            d_min1 = d[j];
        }
    }

    let mut sum0 = 0.0_f64;
    let mut sum1 = 0.0_f64;
    for (j, lbl) in labels.iter().enumerate() {
        let bit = label_bit_msb_first(lbl.bits, b, bits_per_symbol);
        if bit == 0 {
            sum0 += (d_min0 - d[j]).exp();
        } else {
            sum1 += (d_min1 - d[j]).exp();
        }
    }

    // If a subset is empty (constellation should have both by bijection
    // at this bit, but guard defensively), fall back to max-log for the
    // missing side by treating its contribution as -inf.
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

/// Max-log LLR for bit position `b` (MSB-first).
///
/// Returns `-min_{j in S0} d_j + min_{j in S1} d_j`. Positive result
/// means bit `0` is more likely.
fn max_log_llr(d: &[f64], labels: &[super::LabelWord], bits_per_symbol: u8, b: u8) -> f64 {
    let mut d_min0 = f64::INFINITY;
    let mut d_min1 = f64::INFINITY;
    for (j, lbl) in labels.iter().enumerate() {
        let bit = label_bit_msb_first(lbl.bits, b, bits_per_symbol);
        if bit == 0 {
            if d[j] < d_min0 {
                d_min0 = d[j];
            }
        } else if d[j] < d_min1 {
            d_min1 = d[j];
        }
    }
    -d_min0 + d_min1
}

#[cfg(test)]
mod tests {
    use super::super::{
        BatchSoftDemapper, DemapInput, DemapMethod, LabelWord, ModemSpec, ModemSpecBuilder,
        Normalization, SymbolPoint,
    };
    use super::ReferenceSoftDemapper;
    use crate::llr::Llr;
    use proptest::prelude::*;

    /// Brute-force log-MAP LLR computed directly from (post-normalized)
    /// spec points and labels, used as an oracle in tests.
    #[allow(clippy::too_many_arguments)]
    fn brute_force_log_map(
        points: &[(f64, f64)],
        labels: &[u16],
        bits_per_symbol: u8,
        y_i: f64,
        y_q: f64,
        h_i: f64,
        h_q: f64,
        n0: f64,
        b: u8,
    ) -> f64 {
        let mut sum0 = 0.0;
        let mut sum1 = 0.0;
        // Stability shift.
        let mut d_min = f64::INFINITY;
        let dists: Vec<f64> = points
            .iter()
            .map(|&(pi, pq)| {
                let ei = y_i - (h_i * pi - h_q * pq);
                let eq = y_q - (h_i * pq + h_q * pi);
                (ei * ei + eq * eq) / n0
            })
            .collect();
        for &d in &dists {
            if d < d_min {
                d_min = d;
            }
        }
        let shift = bits_per_symbol - 1 - b;
        for (j, &d) in dists.iter().enumerate() {
            let bit = (labels[j] >> shift) & 1;
            let e = (d_min - d).exp();
            if bit == 0 {
                sum0 += e;
            } else {
                sum1 += e;
            }
        }
        sum0.ln() - sum1.ln()
    }

    #[test]
    fn test_bpsk_closed_form_matches_demapper() {
        // BPSK: label 0 -> (+1, 0), label 1 -> (-1, 0). Closed-form LLR
        // for h=1, AWGN: L = 4*y / N0.
        let spec = ModemSpec::<f32>::bpsk();
        let demapper = ReferenceSoftDemapper::new(spec);
        let ys: [f32; 9] = [-2.0, -1.0, -0.5, -0.1, 0.0, 0.1, 0.5, 1.0, 2.0];
        let n0s: [f32; 3] = [0.25, 0.5, 1.0];
        for &n0 in &n0s {
            for &y in &ys {
                let rx_i = [y];
                let rx_q = [0.0_f32];
                let nv = [n0];
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
                let expected = 4.0 * y / n0;
                assert!(
                    (out[0].value() - expected).abs() < 1e-4,
                    "BPSK mismatch at y={y}, n0={n0}: got {}, want {expected}",
                    out[0].value()
                );
            }
        }
    }

    #[test]
    fn test_bpsk_max_log_equals_closed_form() {
        // For BPSK (two points) exact and max-log LLRs are equal.
        let demapper = ReferenceSoftDemapper::new(ModemSpec::<f32>::bpsk());
        let rx_i = [0.7_f32];
        let rx_q = [0.0_f32];
        let nv = [0.5_f32];
        let input = DemapInput::<f32> {
            rx_i: &rx_i,
            rx_q: &rx_q,
            gain_i: None,
            gain_q: None,
            noise_var: &nv,
            method: DemapMethod::MaxLog,
        };
        let mut out = [Llr::new(0.0); 1];
        demapper.demap_llrs(input, &mut out);
        let expected = 4.0 * 0.7 / 0.5;
        assert!((out[0].value() - expected).abs() < 1e-4);
    }

    #[test]
    fn test_qpsk_roundtrip_high_snr() {
        let spec = ModemSpec::<f32>::gray_square_qam(4);
        let view = spec.view();
        // Pre-compute label -> (i, q).
        let n = view.num_symbols();
        let mut lab_to_iq: Vec<(f32, f32)> = vec![(0.0, 0.0); n];
        for k in 0..n {
            let l = view.label(k);
            let p = view.point(k);
            lab_to_iq[l.bits as usize] = (p.i, p.q);
        }
        let demapper = ReferenceSoftDemapper::new(spec);

        // 200 random labels at very low noise (pseudo-random via LCG).
        let batch = 200usize;
        let mut state: u64 = 0xC0FFEE;
        let mut rx_i = Vec::with_capacity(batch);
        let mut rx_q = Vec::with_capacity(batch);
        let mut labels = Vec::with_capacity(batch);
        for _ in 0..batch {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let v = (state as usize) % n;
            labels.push(v as u16);
            let (i, q) = lab_to_iq[v];
            rx_i.push(i);
            rx_q.push(q);
        }
        let nv = vec![0.001_f32; batch];
        let input = DemapInput::<f32> {
            rx_i: &rx_i,
            rx_q: &rx_q,
            gain_i: None,
            gain_q: None,
            noise_var: &nv,
            method: DemapMethod::ExactLogMap,
        };
        let mut out = vec![Llr::new(0.0); batch * 2];
        demapper.demap_llrs(input, &mut out);

        for k in 0..batch {
            let lab = labels[k];
            // MSB first: bit 0 = (lab >> 1) & 1, bit 1 = lab & 1.
            let bit0 = (lab >> 1) & 1;
            let bit1 = lab & 1;
            let got0 = out[2 * k].hard_decision() as u16;
            let got1 = out[2 * k + 1].hard_decision() as u16;
            assert_eq!(got0, bit0, "QPSK bit0 mismatch at k={k}, lab={lab:02b}");
            assert_eq!(got1, bit1, "QPSK bit1 mismatch at k={k}, lab={lab:02b}");
        }
    }

    /// Custom 4-point constellation with a non-Gray label permutation.
    fn custom_non_gray_4() -> ModemSpec<f64> {
        // Unit-circle points.
        let raw = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)];
        // Non-Gray permutation: adjacent angles get labels that differ
        // in two bits (00 -> 11 is fine; 11 -> 01 differs in 1 bit, etc.).
        let labels_perm: [u16; 4] = [0b00, 0b11, 0b01, 0b10];
        let points: Vec<SymbolPoint<f64>> =
            raw.iter().map(|&(i, q)| SymbolPoint::new(i, q)).collect();
        let labels: Vec<LabelWord> = labels_perm.iter().map(|&b| LabelWord::new(b, 2)).collect();
        ModemSpecBuilder::<f64>::new()
            .bits_per_symbol(2)
            .points(points)
            .labels(labels)
            .normalization(Normalization::UnitAverageSymbolEnergy)
            .build()
    }

    #[test]
    fn test_non_gray_4_point_matches_brute_force() {
        let spec = custom_non_gray_4();
        let view = spec.view();
        // Snapshot post-normalization points & labels in oracle form.
        let pts: Vec<(f64, f64)> = view.points().iter().map(|p| (p.i, p.q)).collect();
        let labs: Vec<u16> = view.labels().iter().map(|l| l.bits).collect();
        let bps = view.bits_per_symbol();

        let demapper = ReferenceSoftDemapper::new(spec);
        // Some arbitrary received points.
        let rx_i = [0.7_f64, -0.3, 0.1, -1.1];
        let rx_q = [0.2_f64, 0.8, -0.6, 0.05];
        let nv = [0.3_f64, 0.5, 0.7, 0.4];
        let input = DemapInput::<f64> {
            rx_i: &rx_i,
            rx_q: &rx_q,
            gain_i: None,
            gain_q: None,
            noise_var: &nv,
            method: DemapMethod::ExactLogMap,
        };
        let mut out = vec![Llr::new(0.0); rx_i.len() * 2];
        demapper.demap_llrs(input, &mut out);

        for k in 0..rx_i.len() {
            for b in 0..2u8 {
                let expected =
                    brute_force_log_map(&pts, &labs, bps, rx_i[k], rx_q[k], 1.0, 0.0, nv[k], b);
                let got = out[k * 2 + b as usize].value() as f64;
                assert!(
                    (got - expected).abs() < 1e-3,
                    "non-Gray LLR mismatch at k={k}, b={b}: got {got}, want {expected}"
                );
            }
        }
    }

    /// Build an 8-PSK constellation (non-square, 8 points on the unit
    /// circle) with an arbitrary bit labeling.
    fn custom_8_psk() -> ModemSpec<f64> {
        let points: Vec<SymbolPoint<f64>> = (0..8)
            .map(|k| {
                let theta = (k as f64) * core::f64::consts::TAU / 8.0;
                SymbolPoint::new(theta.cos(), theta.sin())
            })
            .collect();
        // Non-identity permutation.
        let labels_perm: [u16; 8] = [0, 1, 3, 2, 6, 7, 5, 4]; // Gray on circle
        let labels: Vec<LabelWord> = labels_perm.iter().map(|&b| LabelWord::new(b, 3)).collect();
        ModemSpecBuilder::<f64>::new()
            .bits_per_symbol(3)
            .points(points)
            .labels(labels)
            .normalization(Normalization::UnitAverageSymbolEnergy)
            .build()
    }

    #[test]
    fn test_8_psk_matches_brute_force() {
        let spec = custom_8_psk();
        let view = spec.view();
        let pts: Vec<(f64, f64)> = view.points().iter().map(|p| (p.i, p.q)).collect();
        let labs: Vec<u16> = view.labels().iter().map(|l| l.bits).collect();
        let bps = view.bits_per_symbol();

        let demapper = ReferenceSoftDemapper::new(spec);
        let rx_i = [0.6_f64, -0.4];
        let rx_q = [0.4_f64, -0.7];
        let nv = [0.25_f64, 0.4];
        let input = DemapInput::<f64> {
            rx_i: &rx_i,
            rx_q: &rx_q,
            gain_i: None,
            gain_q: None,
            noise_var: &nv,
            method: DemapMethod::ExactLogMap,
        };
        let mut out = vec![Llr::new(0.0); rx_i.len() * 3];
        demapper.demap_llrs(input, &mut out);
        for k in 0..rx_i.len() {
            for b in 0..3u8 {
                let expected =
                    brute_force_log_map(&pts, &labs, bps, rx_i[k], rx_q[k], 1.0, 0.0, nv[k], b);
                let got = out[k * 3 + b as usize].value() as f64;
                assert!(
                    (got - expected).abs() < 1e-3,
                    "8-PSK LLR mismatch at k={k}, b={b}: got {got}, want {expected}"
                );
            }
        }
    }

    #[test]
    fn test_exact_and_max_log_agree_on_hard_decisions_high_snr() {
        let spec = ModemSpec::<f32>::gray_square_qam(16);
        let view = spec.view();
        let n = view.num_symbols();
        let mut lab_to_iq: Vec<(f32, f32)> = vec![(0.0, 0.0); n];
        for k in 0..n {
            lab_to_iq[view.label(k).bits as usize] = (view.point(k).i, view.point(k).q);
        }
        let demapper = ReferenceSoftDemapper::new(spec);
        let rx_i: Vec<f32> = (0..n).map(|v| lab_to_iq[v].0).collect();
        let rx_q: Vec<f32> = (0..n).map(|v| lab_to_iq[v].1).collect();
        let nv = vec![0.001_f32; n];
        let mk = |method| DemapInput::<f32> {
            rx_i: &rx_i,
            rx_q: &rx_q,
            gain_i: None,
            gain_q: None,
            noise_var: &nv,
            method,
        };
        let mut exact = vec![Llr::new(0.0); n * 4];
        let mut maxlog = vec![Llr::new(0.0); n * 4];
        demapper.demap_llrs(mk(DemapMethod::ExactLogMap), &mut exact);
        demapper.demap_llrs(mk(DemapMethod::MaxLog), &mut maxlog);
        for i in 0..n * 4 {
            assert_eq!(
                exact[i].hard_decision(),
                maxlog[i].hard_decision(),
                "hard-decision mismatch at i={i}"
            );
        }
    }

    #[test]
    fn test_with_complex_gains() {
        // h = (cos t, sin t) rotates the constellation by t; demapper
        // must invert it and produce the same LLRs as the h=(1,0) case
        // applied to the rotated-back sample.
        let spec = ModemSpec::<f64>::bpsk_with_scalar();
        let demapper = ReferenceSoftDemapper::new(spec);
        let theta = 0.37_f64;
        let (ht, st) = (theta.cos(), theta.sin());
        // Transmit label 0 -> (+1, 0). Received = h * x + n with n small.
        let tx_i = 1.0;
        let tx_q = 0.0;
        let n_i = 0.05;
        let n_q = -0.02;
        let y_i = ht * tx_i - st * tx_q + n_i;
        let y_q = ht * tx_q + st * tx_i + n_q;
        let rx_i = [y_i];
        let rx_q = [y_q];
        let gi = [ht];
        let gq = [st];
        let nv = [0.1_f64];
        let input = DemapInput::<f64> {
            rx_i: &rx_i,
            rx_q: &rx_q,
            gain_i: Some(&gi),
            gain_q: Some(&gq),
            noise_var: &nv,
            method: DemapMethod::ExactLogMap,
        };
        let mut out = [Llr::new(0.0); 1];
        demapper.demap_llrs(input, &mut out);
        // High confidence in bit 0.
        assert!(
            out[0].value() > 5.0,
            "expected positive LLR, got {}",
            out[0].value()
        );
    }

    #[test]
    #[should_panic(expected = "gain_i and gain_q must be both Some or both None")]
    fn test_half_gain_panics() {
        let demapper = ReferenceSoftDemapper::new(ModemSpec::<f32>::bpsk());
        let rx_i = [0.5_f32];
        let rx_q = [0.0_f32];
        let gi = [1.0_f32];
        let nv = [0.5_f32];
        let mut out = [Llr::new(0.0); 1];
        demapper.demap_llrs(
            DemapInput {
                rx_i: &rx_i,
                rx_q: &rx_q,
                gain_i: Some(&gi),
                gain_q: None,
                noise_var: &nv,
                method: DemapMethod::ExactLogMap,
            },
            &mut out,
        );
    }

    #[test]
    #[should_panic(expected = "rx_i.len()")]
    fn test_rx_q_length_mismatch_panics() {
        let demapper = ReferenceSoftDemapper::new(ModemSpec::<f32>::bpsk());
        let rx_i = [0.5_f32; 2];
        let rx_q = [0.0_f32; 3];
        let nv = [0.5_f32; 2];
        let mut out = [Llr::new(0.0); 2];
        demapper.demap_llrs(
            DemapInput {
                rx_i: &rx_i,
                rx_q: &rx_q,
                gain_i: None,
                gain_q: None,
                noise_var: &nv,
                method: DemapMethod::ExactLogMap,
            },
            &mut out,
        );
    }

    #[test]
    #[should_panic(expected = "out_llrs.len()")]
    fn test_out_length_mismatch_panics() {
        let demapper = ReferenceSoftDemapper::new(ModemSpec::<f32>::gray_square_qam(4));
        let rx_i = [0.5_f32; 2];
        let rx_q = [0.0_f32; 2];
        let nv = [0.5_f32; 2];
        let mut out = [Llr::new(0.0); 3]; // should be 4
        demapper.demap_llrs(
            DemapInput {
                rx_i: &rx_i,
                rx_q: &rx_q,
                gain_i: None,
                gain_q: None,
                noise_var: &nv,
                method: DemapMethod::ExactLogMap,
            },
            &mut out,
        );
    }

    #[test]
    #[should_panic(expected = "does not advertise MaxLog")]
    fn test_unsupported_method_panics() {
        let points = vec![
            SymbolPoint::<f64>::new(1.0, 0.0),
            SymbolPoint::<f64>::new(-1.0, 0.0),
        ];
        let labels = vec![LabelWord::new(0, 1), LabelWord::new(1, 1)];
        let spec = ModemSpecBuilder::<f64>::new()
            .bits_per_symbol(1)
            .points(points)
            .labels(labels)
            .capabilities(super::super::ModemCapabilities {
                supports_exact_log_map: true,
                supports_max_log: false,
            })
            .build();
        let demapper = ReferenceSoftDemapper::new(spec);
        let rx_i = [0.0_f64];
        let rx_q = [0.0_f64];
        let nv = [1.0_f64];
        let mut out = [Llr::new(0.0); 1];
        demapper.demap_llrs(
            DemapInput {
                rx_i: &rx_i,
                rx_q: &rx_q,
                gain_i: None,
                gain_q: None,
                noise_var: &nv,
                method: DemapMethod::MaxLog,
            },
            &mut out,
        );
    }

    proptest! {
        #[test]
        fn prop_random_constellation_no_nan_and_sign_matches_nearest(
            m in 1u8..=3u8,
            seed in 0u64..2_000u64,
            y_scale in 0.1f32..1.5f32,
        ) {
            let n = 1usize << m;
            // Deterministic permutation.
            let mut perm: Vec<u16> = (0..n as u16).collect();
            let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            for i in (1..n).rev() {
                state = state.wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let j = (state as usize) % (i + 1);
                perm.swap(i, j);
            }
            // Points on a circle.
            let points: Vec<SymbolPoint<f32>> = (0..n)
                .map(|k| {
                    let theta = (k as f32) * core::f32::consts::TAU / (n as f32);
                    SymbolPoint::new(theta.cos(), theta.sin())
                })
                .collect();
            let labels_vec: Vec<LabelWord> =
                perm.iter().map(|&b| LabelWord::new(b, m)).collect();
            let spec = ModemSpecBuilder::<f32>::new()
                .bits_per_symbol(m)
                .points(points)
                .labels(labels_vec)
                .build();
            let view = spec.view();
            // Snapshot pts and labels for the nearest-point oracle.
            let pts: Vec<(f32, f32)> = view.points().iter().map(|p| (p.i, p.q)).collect();
            let labs: Vec<u16> = view.labels().iter().map(|l| l.bits).collect();

            let demapper = ReferenceSoftDemapper::new(spec);

            // Construct a received sample not too far from some point.
            state = state.wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let pick = (state as usize) % n;
            let (pi, pq) = pts[pick];
            let y_i = pi + 0.05 * y_scale;
            let y_q = pq - 0.03 * y_scale;
            let rx_i = [y_i];
            let rx_q = [y_q];
            let nv = [0.3_f32];
            let input = DemapInput::<f32> {
                rx_i: &rx_i,
                rx_q: &rx_q,
                gain_i: None,
                gain_q: None,
                noise_var: &nv,
                method: DemapMethod::ExactLogMap,
            };
            let mut out = vec![Llr::new(0.0); m as usize];
            demapper.demap_llrs(input, &mut out);

            // Nearest point has label `labs[pick]`. For each bit b, LLR sign
            // must say "bit equals that of nearest point" at low noise.
            for b in 0..m {
                let v = out[b as usize].value();
                prop_assert!(v.is_finite(), "LLR {v} not finite");
                let shift = m - 1 - b;
                let nearest_bit = (labs[pick] >> shift) & 1;
                let hd = out[b as usize].hard_decision() as u16;
                prop_assert_eq!(hd, nearest_bit);
            }
        }
    }
}
