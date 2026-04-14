//! Property tests for the modem surface (JIT issue `dafb938a`).
//!
//! `proptest`-driven invariants over the public modem API. Focuses on
//! behaviours that the regression tests in `modem_regression.rs` only
//! pin at fixed seeds / fixed samples:
//!
//! 1. **Labeling bijection** — for every preset the full mapper output
//!    over all `2^m` labels is a bijection in both directions (labels ↔
//!    constellation points).
//! 2. **Output shape / finiteness** — for random batch sizes in
//!    `[0, 256]`, the demapper output length is exactly
//!    `batch_size * m` and every entry is finite when the input is
//!    finite and `noise_var > 0`.
//! 3. **Noise / sign symmetry** — for Gray presets, flipping the sign
//!    of every received sample must flip the sign of every output LLR.
//! 4. **Hard-decision convergence as noise → 0** — the hard decisions
//!    from the soft demapper converge to the labels of the
//!    nearest-constellation-point map across a noise sweep.
//!
//! These share the SSOT [`Lcg`] + brute-force oracle in
//! `gf2_coding::modem::test_oracle` with the reference-model and
//! regression integration tests.

use gf2_coding::llr::Llr;
use gf2_coding::modem::test_oracle::Lcg;
use gf2_coding::modem::{
    unpack_label_msb_first, BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod,
    FastGrayQamDemapper, GrayQamMapper, ModemSpec, ReferenceMapper, ReferenceSoftDemapper,
};

use proptest::prelude::*;

/// Constellation orders that the preset surface promises.
const PRESET_ORDERS: [usize; 5] = [2, 4, 16, 64, 256];

/// Strategy over the five supported preset orders.
fn preset_order() -> impl Strategy<Value = usize> {
    prop::sample::select(PRESET_ORDERS.to_vec())
}

// ---------------------------------------------------------------------
// 1. Labeling bijection
// ---------------------------------------------------------------------

/// Maps every label in `[0, 2^m)` through a [`BatchMapper`] and confirms:
///   - the same label always produces the same point (determinism);
///   - distinct labels produce distinct points (no collisions);
///   - the total set of mapped points has cardinality `2^m` (no gaps).
fn check_full_label_bijection<M: BatchMapper<f64>>(mapper: &M, m: u8) {
    let n = 1usize << m;
    let mut pts: Vec<(i64, i64)> = Vec::with_capacity(n);
    for label in 0u16..n as u16 {
        let bits = unpack_label_msb_first(label, m);
        let mut oi = [0.0_f64; 1];
        let mut oq = [0.0_f64; 1];
        mapper.map_bits(&bits, &mut oi, &mut oq);
        // Quantize to `i64` fixed-point at 1e-9 resolution: the presets
        // we test against all produce rational coordinates scaled by
        // a unit-energy factor, so a 1e-9 grid distinguishes every
        // post-normalization point but tolerates float rounding.
        let qi = (oi[0] * 1e9).round() as i64;
        let qq = (oq[0] * 1e9).round() as i64;
        pts.push((qi, qq));
    }
    // No duplicates.
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

    /// Every supported preset is a bijection under the reference mapper.
    #[test]
    fn prop_labeling_bijection_reference(order in preset_order()) {
        let spec: ModemSpec<f64> = ModemSpec::<f64>::gray_square_qam_with_scalar(order);
        let m = spec.bits_per_symbol();
        let mapper = ReferenceMapper::new(spec);
        check_full_label_bijection(&mapper, m);
    }

    /// Every supported preset is a bijection under the Gray-QAM fast
    /// mapper — this is the invariant the fast path's axis-separable
    /// kernel relies on.
    #[test]
    fn prop_labeling_bijection_fast(order in preset_order()) {
        let mapper: GrayQamMapper<f64> =
            GrayQamMapper::<f64>::from_preset_order_with_scalar(order);
        let m = mapper.spec().bits_per_symbol();
        check_full_label_bijection(&mapper, m);
    }
}

// ---------------------------------------------------------------------
// 2. Output shape and finiteness
// ---------------------------------------------------------------------

/// Generates a `(rx_i, rx_q, noise_var)` triple of `batch` elements from
/// the shared LCG. `noise_var` is strictly positive so the log-MAP
/// reduction is well-defined.
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

    /// Output length is exactly `batch * m`, and every LLR is finite when
    /// inputs are finite and `noise_var > 0`. Batch sizes include 0
    /// (empty-batch contract) and up to 256 (plenty of coverage for the
    /// size sweep without blowing up test runtime on debug builds).
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

// ---------------------------------------------------------------------
// 3. Noise / sign symmetry on Gray presets
// ---------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    /// Gray-square-QAM and BPSK presets split their bit labels into two
    /// groups by the Gray-code structure:
    ///
    /// - The **axis-sign bits** (MSB of the I-axis half-label and MSB of
    ///   the Q-axis half-label) flip when the corresponding axis sample
    ///   flips sign: `LLR(-y) == -LLR(+y)` for those bits under a
    ///   symmetric constellation.
    /// - The **inner/outer bits** (the remaining Gray-PAM bits on each
    ///   axis) are *even* under a sign flip: the inner-vs-outer
    ///   distinction only cares about `|y|`, so those LLRs satisfy
    ///   `LLR(-y) == +LLR(+y)`.
    ///
    /// This property pins both invariants. Together they are the
    /// symmetry fingerprint of a canonical Gray square-QAM mapping.
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

        // Bit-layout-aware check: the two axis-sign bits flip, the
        // remaining Gray-PAM bits are even.
        for k in 0..batch {
            for b in 0..m {
                let p = out_pos[k * m + b].value();
                let n = out_neg[k * m + b].value();
                // MSB of the I-axis half-label: bit index 0. MSB of the
                // Q-axis half-label: bit index m_half. BPSK only has the
                // I-axis sign bit at index 0.
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

// ---------------------------------------------------------------------
// 4. Hard-decision convergence as noise → 0
// ---------------------------------------------------------------------

/// Returns the MSB-first label of the post-normalization constellation
/// point in `spec` closest to `(y_i, y_q)`. Used as the ground-truth
/// hard-decision target when `noise_var → 0`.
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

    /// As `noise_var → 0`, the soft demapper's hard decisions converge
    /// to the labels of the nearest constellation point. Concretely, at
    /// `noise_var = 1e-8` the bit decisions must match the brute-force
    /// nearest-neighbour labels for every sample in a randomly-generated
    /// batch.
    #[test]
    fn prop_hard_decision_converges_to_nearest_neighbour(
        order in preset_order(),
        seed in any::<u64>(),
    ) {
        let spec: ModemSpec<f64> = ModemSpec::<f64>::gray_square_qam_with_scalar(order);
        let m = spec.bits_per_symbol();
        let batch = 32usize;

        // Sample inside the convex hull of the constellation so there's
        // no ambiguity about which point is closest: uniformly in
        // [-1.2, 1.2] on each axis comfortably covers every preset's
        // post-normalization layout.
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
