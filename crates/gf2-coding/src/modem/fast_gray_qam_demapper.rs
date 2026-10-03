//! Gray square-QAM soft demapper. Under AWGN with independent I/Q noise the
//! 2D log-MAP factorizes into two 1D Gray-PAM demaps of `sqrt(M)` levels
//! each, so a batch costs `O(num_symbols * sqrt(M) * m)`.

use crate::llr::Llr;

use super::{BatchSoftDemapper, DemapInput, DemapMethod, ModemScalar, ModemSpec, ModemView};

use gf2_kernels_simd::modem::{self as kernel_modem, GrayPamDistanceFnsF64};
use std::sync::OnceLock;

/// Scalar Gray-PAM distance kernels.
#[inline]
fn scalar_fns_f64_static() -> &'static GrayPamDistanceFnsF64 {
    static FNS: OnceLock<GrayPamDistanceFnsF64> = OnceLock::new();
    FNS.get_or_init(kernel_modem::scalar_fns_f64)
}

/// Gray-PAM distance kernels selected once by CPU-feature detection: AVX2
/// where the host advertises it, scalar otherwise.
fn kernel_fns_f64() -> &'static GrayPamDistanceFnsF64 {
    static FNS: OnceLock<GrayPamDistanceFnsF64> = OnceLock::new();
    FNS.get_or_init(kernel_modem::detect_f64)
}

/// Soft demapper for BPSK and the Gray square-QAM presets of orders 4, 16,
/// 64 and 256; [`super::ReferenceSoftDemapper`] covers other constellations.
pub struct FastGrayQamDemapper<S: ModemScalar> {
    spec: ModemSpec<S>,
    /// Total bits per symbol (`log2(order)`).
    m_total: u8,
    /// Half bits per symbol for QAM (`m_total / 2`); `0` for BPSK.
    m_half: u8,
    /// `true` if this is the BPSK (single-axis) preset.
    is_bpsk: bool,
    /// PAM squared-distance kernels: runtime-detected in `new`, scalar in
    /// `new_with_scalar_kernel`.
    kernel_fns: &'static GrayPamDistanceFnsF64,
    /// Post-normalization Gray-PAM levels on each axis, indexed by the
    /// `m_half`-bit Gray label (MSB-first within the half-label). Length
    /// `1 << m_half` for QAM, or `2` for BPSK (indexed by the raw bit).
    pam_levels: Vec<f64>,
}

impl<S: ModemScalar> FastGrayQamDemapper<S> {
    /// Constructs the demapper with the runtime-detected distance kernel.
    ///
    /// # Panics
    ///
    /// Panics unless `spec` has the BPSK / Gray square-QAM preset layout:
    ///
    /// - `bits_per_symbol` is one of `1, 2, 4, 6, 8`;
    /// - bit channels are `SingleAxisPam(0)` (BPSK) or `m/2` `IAxisPam`
    ///   entries followed by `m/2` `QAxisPam` entries (QAM);
    /// - capabilities advertise both `ExactLogMap` and `MaxLog`;
    /// - every point lies on the preset Gray-PAM level of its I and Q
    ///   half-labels within `1e-6`;
    /// - (BPSK) label `k` is stored at index `k` and both points share one
    ///   Q coordinate.
    pub fn new(spec: ModemSpec<S>) -> Self {
        Self::new_with_kernel(spec, kernel_fns_f64())
    }

    /// [`Self::new`] pinned to the scalar distance kernel, for measuring the
    /// scalar path independently of the host's detected kernel.
    ///
    /// # Panics
    ///
    /// Same conditions as [`Self::new`].
    pub fn new_with_scalar_kernel(spec: ModemSpec<S>) -> Self {
        Self::new_with_kernel(spec, scalar_fns_f64_static())
    }

    fn new_with_kernel(spec: ModemSpec<S>, kernel_fns: &'static GrayPamDistanceFnsF64) -> Self {
        super::presets::assert_valid_gray_square_qam_spec(&spec.view());

        let m_total = spec.bits_per_symbol();
        let is_bpsk = m_total == 1;
        let m_half = if is_bpsk { 0u8 } else { m_total / 2 };

        // The validation above pins the spec's points to this table.
        let pam_levels: Vec<f64> = super::presets::gray_pam_levels::<f64>(m_total);

        Self {
            spec,
            m_total,
            m_half,
            is_bpsk,
            kernel_fns,
            pam_levels,
        }
    }

    /// Post-normalization Gray-PAM level table shared by the I and Q axes,
    /// indexed by the raw Gray-PAM axis label: `1 << (m / 2)` entries for
    /// QAM, `2` for BPSK.
    #[inline]
    pub fn pam_levels(&self) -> &[f64] {
        &self.pam_levels
    }

    /// Returns a borrowed reference to the owned [`ModemSpec`].
    #[inline]
    pub fn spec_ref(&self) -> &ModemSpec<S> {
        &self.spec
    }
}

/// Computes the 1D Gray-PAM LLR for bit `bit_idx` (MSB-first within the
/// `m_bits`-wide half-label) from the per-level squared distances `d`,
/// indexed by the raw Gray label (`d.len() == 1 << m_bits`).
#[inline]
fn pam_axis_llr(d: &[f64], m_bits: u8, bit_idx: u8, method: DemapMethod) -> f64 {
    super::demapper::subset_log_map_llr(d, |j| j as u16, d.len(), m_bits, bit_idx, method)
}

impl<S: ModemScalar> BatchSoftDemapper<S> for FastGrayQamDemapper<S> {
    fn spec(&self) -> ModemView<'_, S> {
        self.spec.view()
    }

    /// Pre-rotates each sample by `conj(h)`, fills one Gray-PAM
    /// squared-distance slab per axis (I only for BPSK), and reduces each
    /// slab to LLRs. A symbol with zero channel gain gets zero LLRs.
    ///
    /// # Panics
    ///
    /// Panics on the [`BatchSoftDemapper::demap_llrs`] conditions and if a
    /// `noise_var` entry is not positive and finite.
    ///
    /// # Complexity
    ///
    /// `O(num_symbols * sqrt(M) * m)` where `M = 2^m` is the constellation
    /// order.
    fn demap_llrs(&self, input: DemapInput<'_, S>, out_llrs: &mut [Llr]) {
        let m = self.m_total as usize;
        let view = self.spec.view();
        let num_symbols = super::demapper::validate_demap_input(
            "FastGrayQamDemapper::demap_llrs",
            &view,
            &input,
            out_llrs.len(),
        );

        if num_symbols == 0 {
            return;
        }

        let axis_len = if self.is_bpsk {
            2
        } else {
            1usize << self.m_half
        };

        let mut z_i: Vec<f64> = Vec::with_capacity(num_symbols);
        let mut z_q: Vec<f64> = Vec::with_capacity(num_symbols);
        let mut g_scratch: Vec<f64> = Vec::with_capacity(num_symbols);
        let mut inv_n0_eq: Vec<f64> = Vec::with_capacity(num_symbols);
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
            z_i.push(h_i * y_i + h_q * y_q);
            z_q.push(h_i * y_q - h_q * y_i);
            g_scratch.push(g);

            // At zero gain every point is equidistant from y and the
            // balanced presets give LLR 0. `inv_n0_eq = 0` makes the
            // kernel emit a zero distance slab, which reduces to zero
            // LLRs without NaN.
            let n0_eq = n0 * g;
            let inv = if n0_eq <= f64::EPSILON * n0 {
                0.0
            } else {
                1.0 / n0_eq
            };
            inv_n0_eq.push(inv);
        }

        // Both axes share one level table.
        let mut d_i: Vec<f64> = vec![0.0; num_symbols * axis_len];
        let mut d_q: Vec<f64> = if self.is_bpsk {
            Vec::new()
        } else {
            vec![0.0; num_symbols * axis_len]
        };

        run_pam_distance_kernel(
            self.kernel_fns,
            &z_i,
            &g_scratch,
            &inv_n0_eq,
            &self.pam_levels,
            &mut d_i,
        );
        if !self.is_bpsk {
            run_pam_distance_kernel(
                self.kernel_fns,
                &z_q,
                &g_scratch,
                &inv_n0_eq,
                &self.pam_levels,
                &mut d_q,
            );
        }

        if self.is_bpsk {
            for k in 0..num_symbols {
                let slab = &d_i[k * axis_len..(k + 1) * axis_len];
                let llr = pam_axis_llr(slab, 1, 0, input.method);
                out_llrs[k * m] = Llr::new(llr as f32);
            }
        } else {
            let m_half = self.m_half;
            for k in 0..num_symbols {
                let i_slab = &d_i[k * axis_len..(k + 1) * axis_len];
                let q_slab = &d_q[k * axis_len..(k + 1) * axis_len];
                // First m_half bits are I-axis, remainder are Q-axis.
                for b in 0..m_half {
                    let llr = pam_axis_llr(i_slab, m_half, b, input.method);
                    out_llrs[k * m + b as usize] = Llr::new(llr as f32);
                }
                for b in 0..m_half {
                    let llr = pam_axis_llr(q_slab, m_half, b, input.method);
                    out_llrs[k * m + (m_half + b) as usize] = Llr::new(llr as f32);
                }
            }
        }
    }
}

/// Runs the Gray-PAM squared-distance kernel on one axis:
/// `out[s * pam_levels.len() + l]` is the distance between `z[s]` and
/// level `l`, and all zero where `inv_n0_eq[s] == 0`.
#[inline]
fn run_pam_distance_kernel(
    fns: &GrayPamDistanceFnsF64,
    z: &[f64],
    g: &[f64],
    inv_n0_eq: &[f64],
    pam_levels: &[f64],
    out: &mut [f64],
) {
    (fns.pam_sq_distances_fn)(z, g, inv_n0_eq, pam_levels, out);
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

    use crate::modem::test_oracle::Lcg;

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
                    rx_i.push((rng.next_unit_f64() * 2.0) as f32);
                    rx_q.push((rng.next_unit_f64() * 2.0) as f32);
                    nv.push(rng.next_positive_f64(0.05, 2.0) as f32);
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
                    rx_i.push(rng.next_unit_f64() * 2.0);
                    rx_q.push(rng.next_unit_f64() * 2.0);
                    nv.push(rng.next_positive_f64(0.05, 2.0));
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
                    rx_i.push(rng.next_unit_f64() * 2.0);
                    rx_q.push(rng.next_unit_f64() * 2.0);
                    // Avoid near-zero |h| that would blow up the
                    // equalized noise variance.
                    let hi = if rng.next_unit_f64() > 0.0 { 0.6 } else { -0.6 }
                        + 0.3 * rng.next_unit_f64();
                    let hq = 0.2 * rng.next_unit_f64();
                    gi.push(hi);
                    gq.push(hq);
                    nv.push(rng.next_positive_f64(0.05, 1.0));
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

    /// Sizes straddle multiples of the SIMD lane width; size 0 takes the
    /// empty-batch return.
    #[test]
    fn test_batch_size_sweep_crosses_avx2_boundary() {
        let methods = [DemapMethod::ExactLogMap, DemapMethod::MaxLog];
        let sizes = [0usize, 1, 7, 8, 9, 15, 16, 17, 31, 32];
        for &order in &PRESET_ORDERS {
            for method in methods {
                for &n in &sizes {
                    let spec = spec_for_order_f32(order);
                    let m = spec.bits_per_symbol() as usize;
                    let fast = FastGrayQamDemapper::new(spec.clone());
                    let reference = ReferenceSoftDemapper::new(spec);

                    let mut rng =
                        Lcg::new(0xB47C_415E ^ (order as u64) ^ method_seed(method) ^ (n as u64));
                    let mut rx_i = Vec::with_capacity(n);
                    let mut rx_q = Vec::with_capacity(n);
                    let mut nv = Vec::with_capacity(n);
                    for _ in 0..n {
                        rx_i.push((rng.next_unit_f64() * 2.0) as f32);
                        rx_q.push((rng.next_unit_f64() * 2.0) as f32);
                        nv.push(rng.next_positive_f64(0.05, 2.0) as f32);
                    }
                    let input = DemapInput::<f32> {
                        rx_i: &rx_i,
                        rx_q: &rx_q,
                        gain_i: None,
                        gain_q: None,
                        noise_var: &nv,
                        method,
                    };
                    let mut out_fast = vec![Llr::new(0.0); n * m];
                    let mut out_ref = vec![Llr::new(0.0); n * m];
                    fast.demap_llrs(input, &mut out_fast);
                    reference.demap_llrs(input, &mut out_ref);
                    assert_close_f32(
                        &out_fast,
                        &out_ref,
                        1e-3,
                        &format!("batch sweep order={order} n={n} method={method:?}"),
                    );
                }
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
        // BPSK closed form at unit gain: L = 4*y / N0.
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
    #[should_panic(expected = "BPSK spec must store label 0 at index 0")]
    fn test_permuted_bpsk_label_order_rejected() {
        // Custom BPSK spec with labels stored in reversed order:
        // index 0 => label 1, index 1 => label 0. The fast kernel
        // assumes label == index, so must reject this at construction.
        use crate::modem::{BitChannelSemantics, ModemCapabilities};
        let points = vec![SymbolPoint::new(1.0, 0.0), SymbolPoint::new(-1.0, 0.0)];
        let labels = vec![LabelWord::new(1, 1), LabelWord::new(0, 1)];
        let spec = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(1)
            .points(points)
            .labels(labels)
            .bit_channels(vec![BitChannelSemantics::SingleAxisPam(0)])
            .capabilities(ModemCapabilities {
                supports_exact_log_map: true,
                supports_max_log: true,
                analysis: &[],
            })
            .normalization(Normalization::UnitAverageSymbolEnergy)
            .build();
        let _ = FastGrayQamDemapper::new(spec);
    }

    #[test]
    #[should_panic(expected = "common Q coordinate")]
    fn test_bpsk_non_common_q_coordinate_rejected() {
        // Custom BPSK spec where the two points have different Q
        // coordinates. The fast kernel drops Q as a common additive
        // constant, so a non-common Q would yield wrong LLRs — reject.
        use crate::modem::{BitChannelSemantics, ModemCapabilities};
        let points = vec![SymbolPoint::new(1.0, 0.3), SymbolPoint::new(-1.0, -0.3)];
        let labels = vec![LabelWord::new(0, 1), LabelWord::new(1, 1)];
        let spec = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(1)
            .points(points)
            .labels(labels)
            .bit_channels(vec![BitChannelSemantics::SingleAxisPam(0)])
            .capabilities(ModemCapabilities {
                supports_exact_log_map: true,
                supports_max_log: true,
                analysis: &[],
            })
            .normalization(Normalization::ExplicitEs(1.09))
            .build();
        let _ = FastGrayQamDemapper::new(spec);
    }

    #[test]
    #[should_panic(expected = "expected canonical Gray-PAM level")]
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
    #[should_panic(expected = "expected canonical Gray-PAM level")]
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
    #[should_panic(expected = "expected canonical Gray-PAM level")]
    fn test_spoofed_iaxispam_metadata_with_non_preset_geometry_rejected() {
        // IAxisPam/QAxisPam metadata (so the bit-channel check accepts it)
        // with asymmetric geometry that breaks the I/Q factorisation the
        // fast path relies on.
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

            let mut rng = super::super::test_oracle::Lcg::new(y_seed | 1);
            let rx_i: Vec<f64> = (0..n_sym).map(|_| rng.next_unit_f64() * 1.5).collect();
            let rx_q: Vec<f64> = (0..n_sym).map(|_| rng.next_unit_f64() * 1.5).collect();
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
