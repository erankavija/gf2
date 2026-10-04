//! `proptest` invariants over the public modem API for every Gray preset:
//! labeling bijection, demap output shape and finiteness, sign symmetry, and
//! hard-decision convergence as the noise variance tends to zero.

use gf2_coding::llr::Llr;
use gf2_coding::modem::test_oracle::Lcg;
use gf2_coding::modem::{
    unpack_label_msb_first, BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod,
    FastGrayQamDemapper, GrayQamMapper, ModemSpec, ReferenceMapper, ReferenceSoftDemapper,
};

use proptest::prelude::*;

const PRESET_ORDERS: [usize; 5] = [2, 4, 16, 64, 256];

fn preset_order() -> impl Strategy<Value = usize> {
    prop::sample::select(PRESET_ORDERS.to_vec())
}

fn check_full_label_bijection<M: BatchMapper<f64>>(mapper: &M, m: u8) {
    let n = 1usize << m;
    let mut pts: Vec<(i64, i64)> = Vec::with_capacity(n);
    for label in 0u16..n as u16 {
        let bits = unpack_label_msb_first(label, m);
        let mut oi = [0.0_f64; 1];
        let mut oq = [0.0_f64; 1];
        mapper.map_bits(&bits, &mut oi, &mut oq);
        // A 1e-9 fixed-point grid separates every normalized preset point
        // and absorbs float rounding.
        let qi = (oi[0] * 1e9).round() as i64;
        let qq = (oq[0] * 1e9).round() as i64;
        pts.push((qi, qq));
    }
    let mut sorted = pts.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        n,
        "label->point mapping is not a bijection (order {n}): unique={} expected={n}",
        sorted.len()
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    #[test]
    fn prop_labeling_bijection_reference(order in preset_order()) {
        let spec: ModemSpec<f64> = ModemSpec::<f64>::gray_square_qam_with_scalar(order);
        let m = spec.bits_per_symbol();
        let mapper = ReferenceMapper::new(spec);
        check_full_label_bijection(&mapper, m);
    }

    #[test]
    fn prop_labeling_bijection_fast(order in preset_order()) {
        let mapper: GrayQamMapper<f64> =
            GrayQamMapper::<f64>::from_preset_order_with_scalar(order);
        let m = mapper.spec().bits_per_symbol();
        check_full_label_bijection(&mapper, m);
    }
}

/// `noise_var` is strictly positive so the log-MAP reduction is well-defined.
fn synthesize_demap_batch(seed: u64, batch: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut rng = Lcg::new(seed);
    let mut rx_i = Vec::with_capacity(batch);
    let mut rx_q = Vec::with_capacity(batch);
    let mut nv = Vec::with_capacity(batch);
    for _ in 0..batch {
        rx_i.push(rng.next_unit_f64() * 2.0);
        rx_q.push(rng.next_unit_f64() * 2.0);
        nv.push(rng.next_positive_f64(1e-3, 2.0));
    }
    (rx_i, rx_q, nv)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Batch size 0 covers the empty-batch contract.
    #[test]
    fn prop_demap_output_shape_and_finiteness(
        order in preset_order(),
        batch in 0usize..=256,
        seed in any::<u64>(),
        method_idx in 0usize..2,
    ) {
        let spec: ModemSpec<f64> = ModemSpec::<f64>::gray_square_qam_with_scalar(order);
        let m = spec.bits_per_symbol() as usize;
        let method = if method_idx == 0 { DemapMethod::ExactLogMap } else { DemapMethod::MaxLog };

        let (rx_i, rx_q, nv) = synthesize_demap_batch(seed, batch);
        let input = DemapInput::<f64> {
            rx_i: &rx_i,
            rx_q: &rx_q,
            gain_i: None,
            gain_q: None,
            noise_var: &nv,
            method,
        };

        let reference = ReferenceSoftDemapper::new(spec.clone());
        let mut out_ref = vec![Llr::new(0.0); batch * m];
        reference.demap_llrs(input, &mut out_ref);
        prop_assert_eq!(out_ref.len(), batch * m);
        for l in &out_ref {
            prop_assert!(l.value().is_finite(), "reference path emitted non-finite LLR");
        }

        let fast = FastGrayQamDemapper::new(spec);
        let mut out_fast = vec![Llr::new(0.0); batch * m];
        fast.demap_llrs(input, &mut out_fast);
        prop_assert_eq!(out_fast.len(), batch * m);
        for l in &out_fast {
            prop_assert!(l.value().is_finite(), "fast path emitted non-finite LLR");
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    /// Under a sign flip of both axes the axis-sign bits (MSB of each axis
    /// half-label) are odd, `LLR(-y) == -LLR(y)`, and the remaining Gray-PAM
    /// bits are even, `LLR(-y) == LLR(y)`.
    #[test]
    fn prop_gray_sign_flip_symmetry(
        order in preset_order(),
        seed in any::<u64>(),
    ) {
        let spec: ModemSpec<f64> = ModemSpec::<f64>::gray_square_qam_with_scalar(order);
        let m = spec.bits_per_symbol() as usize;
        // `m_half` bits on I then `m_half` bits on Q for QAM; for BPSK
        // (m == 1) the single bit is the I-axis sign bit.
        let m_half = if m == 1 { 0 } else { m / 2 };
        let batch = 24usize;

        let (rx_i, rx_q, nv) = synthesize_demap_batch(seed, batch);
        let rx_i_neg: Vec<f64> = rx_i.iter().map(|v| -v).collect();
        let rx_q_neg: Vec<f64> = rx_q.iter().map(|v| -v).collect();

        let demapper = ReferenceSoftDemapper::new(spec);

        let mut out_pos = vec![Llr::new(0.0); batch * m];
        let mut out_neg = vec![Llr::new(0.0); batch * m];
        let in_pos = DemapInput::<f64> {
            rx_i: &rx_i, rx_q: &rx_q, gain_i: None, gain_q: None,
            noise_var: &nv, method: DemapMethod::ExactLogMap,
        };
        let in_neg = DemapInput::<f64> {
            rx_i: &rx_i_neg, rx_q: &rx_q_neg, gain_i: None, gain_q: None,
            noise_var: &nv, method: DemapMethod::ExactLogMap,
        };
        demapper.demap_llrs(in_pos, &mut out_pos);
        demapper.demap_llrs(in_neg, &mut out_neg);

        for k in 0..batch {
            for b in 0..m {
                let p = out_pos[k * m + b].value();
                let n = out_neg[k * m + b].value();
                let is_axis_sign = b == 0 || (m > 1 && b == m_half);
                let tol = 1e-3_f32.max((p.abs() * 1e-3).max(n.abs() * 1e-3));
                if is_axis_sign {
                    prop_assert!(
                        (p + n).abs() <= tol,
                        "axis-sign bit {} at k={}: LLR(+y)={} LLR(-y)={} sum={} (expect antisymmetric)",
                        b, k, p, n, p + n
                    );
                } else {
                    prop_assert!(
                        (p - n).abs() <= tol,
                        "inner/outer bit {} at k={}: LLR(+y)={} LLR(-y)={} diff={} (expect symmetric)",
                        b, k, p, n, p - n
                    );
                }
            }
        }
    }
}

fn nearest_point_label(spec: &ModemSpec<f64>, y_i: f64, y_q: f64) -> u16 {
    let view = spec.view();
    let mut best_d = f64::INFINITY;
    let mut best_label = 0u16;
    for idx in 0..view.num_symbols() {
        let p = view.point(idx);
        let d = (y_i - p.i).powi(2) + (y_q - p.q).powi(2);
        if d < best_d {
            best_d = d;
            best_label = view.label(idx).bits;
        }
    }
    best_label
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn prop_hard_decision_converges_to_nearest_neighbour(
        order in preset_order(),
        seed in any::<u64>(),
    ) {
        let spec: ModemSpec<f64> = ModemSpec::<f64>::gray_square_qam_with_scalar(order);
        let m = spec.bits_per_symbol();
        let batch = 32usize;

        // [-1.2, 1.2] per axis covers every preset's normalized layout.
        let mut rng = Lcg::new(seed);
        let mut rx_i = Vec::with_capacity(batch);
        let mut rx_q = Vec::with_capacity(batch);
        for _ in 0..batch {
            rx_i.push(rng.next_unit_f64() * 1.2);
            rx_q.push(rng.next_unit_f64() * 1.2);
        }
        let nv = vec![1e-8_f64; batch];

        let demapper = ReferenceSoftDemapper::new(spec.clone());
        let input = DemapInput::<f64> {
            rx_i: &rx_i, rx_q: &rx_q, gain_i: None, gain_q: None,
            noise_var: &nv, method: DemapMethod::MaxLog,
        };
        let mut llrs = vec![Llr::new(0.0); batch * m as usize];
        demapper.demap_llrs(input, &mut llrs);

        for k in 0..batch {
            let expected_label = nearest_point_label(&spec, rx_i[k], rx_q[k]);
            let expected_bits = unpack_label_msb_first(expected_label, m);
            for b in 0..m as usize {
                let got = llrs[k * m as usize + b].hard_decision();
                prop_assert_eq!(
                    got, expected_bits[b],
                    "hard decision at k={} b={}: got {} want {} (label={}, rx=({}, {}))",
                    k, b, got, expected_bits[b], expected_label, rx_i[k], rx_q[k]
                );
            }
        }
    }
}
