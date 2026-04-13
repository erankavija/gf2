//! Integration tests for the reference mapper + soft demapper pair
//! (JIT issue `0aac93c6`).
//!
//! These tests exercise the public modem surface end-to-end and pin
//! three behaviors:
//!
//! 1. **Round-trip** — bits mapped through [`ReferenceMapper`] and pushed
//!    through [`ReferenceSoftDemapper`] at low noise must recover exactly
//!    under hard-decision for both preset and custom builder specs.
//! 2. **Analytic oracle** — the exact log-MAP LLRs emitted by the
//!    reference demapper must match a brute-force log-MAP oracle computed
//!    directly from the post-normalization spec points and labels.
//! 3. **Builder/spec invariants** — each `ModemSpecBuilder::build` /
//!    `ModemSpec::from_parts_checked` invariant has its own
//!    `#[should_panic(expected = ...)]` test that exercises the public
//!    builder entry point.

use gf2_coding::llr::Llr;
use gf2_coding::modem::{
    unpack_label_msb_first, BatchMapper, BatchSoftDemapper, BitChannelSemantics, DemapInput,
    DemapMethod, LabelWord, ModemCapabilities, ModemScalar, ModemSpec, ModemSpecBuilder,
    Normalization, ReferenceMapper, ReferenceSoftDemapper, SymbolPoint,
};

// ---------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------

/// Deterministic LCG-based Fisher-Yates permutation used when a test needs
/// a reproducible, non-identity label assignment without pulling in an rng.
fn lcg_permutation(seed: u64, n: usize) -> Vec<u16> {
    let mut perm: Vec<u16> = (0..n as u16).collect();
    let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    for i in (1..n).rev() {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let j = (state as usize) % (i + 1);
        perm.swap(i, j);
    }
    perm
}

/// Deterministic pseudo-random label stream of length `batch` over
/// `0..n`, seeded by `seed`. Used to drive the round-trip tests.
fn lcg_label_stream(seed: u64, batch: usize, n: usize) -> Vec<u16> {
    let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    let mut labels = Vec::with_capacity(batch);
    for _ in 0..batch {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        labels.push((state as usize % n) as u16);
    }
    labels
}

/// Brute-force exact log-MAP LLR for a single received sample, bit
/// position, and noise variance.
///
/// Mirrors the oracle used in `ref_demapper.rs` unit tests, but operates
/// directly on the integration-test `Vec<(f64, f64)>` / `Vec<u16>`
/// snapshots of a post-normalization `ModemSpec`. Kept as the **only**
/// brute-force oracle in this file so the reviewer can see a single
/// source of truth for cross-check math.
#[allow(clippy::too_many_arguments)]
fn oracle_log_map_llr(
    points: &[(f64, f64)],
    labels: &[u16],
    bits_per_symbol: u8,
    y_i: f64,
    y_q: f64,
    n0: f64,
    b: u8,
) -> f64 {
    let dists: Vec<f64> = points
        .iter()
        .map(|&(pi, pq)| {
            let ei = y_i - pi;
            let eq = y_q - pq;
            (ei * ei + eq * eq) / n0
        })
        .collect();
    let d_min = dists.iter().cloned().fold(f64::INFINITY, f64::min);
    let mut sum0 = 0.0_f64;
    let mut sum1 = 0.0_f64;
    for (j, &d) in dists.iter().enumerate() {
        let bits = unpack_label_msb_first(labels[j], bits_per_symbol);
        let bit = bits[b as usize];
        let e = (d_min - d).exp();
        if !bit {
            sum0 += e;
        } else {
            sum1 += e;
        }
    }
    sum0.ln() - sum1.ln()
}

/// Snapshots post-normalization points and labels from a spec into flat
/// `f64` form suitable for `oracle_log_map_llr`.
fn snapshot_spec_f64<S: ModemScalar>(spec: &ModemSpec<S>) -> (Vec<(f64, f64)>, Vec<u16>, u8) {
    let view = spec.view();
    let pts: Vec<(f64, f64)> = view
        .points()
        .iter()
        .map(|p| (p.i.to_f64(), p.q.to_f64()))
        .collect();
    let labs: Vec<u16> = view.labels().iter().map(|l| l.bits).collect();
    (pts, labs, view.bits_per_symbol())
}

/// Runs the round-trip test: map a deterministic batch of random labels
/// through `ReferenceMapper`, pass the clean transmitted samples through
/// `ReferenceSoftDemapper` at tiny noise, and assert every hard-decision
/// bit recovers the transmitted bit.
fn check_round_trip_f64(spec: ModemSpec<f64>, seed: u64, batch: usize) {
    let bps = spec.bits_per_symbol();
    let n = spec.num_symbols();
    let label_stream = lcg_label_stream(seed, batch, n);

    // Build the input bit stream in MSB-first symbol-major order.
    let mut bits: Vec<bool> = Vec::with_capacity(batch * bps as usize);
    for &v in &label_stream {
        bits.extend(unpack_label_msb_first(v, bps));
    }

    // Map to transmitted samples via the reference mapper.
    let mapper = ReferenceMapper::new(spec.clone());
    let mut tx_i = vec![0.0_f64; batch];
    let mut tx_q = vec![0.0_f64; batch];
    mapper.map_bits(&bits, &mut tx_i, &mut tx_q);

    // Push through the reference soft demapper at very low noise.
    let demapper = ReferenceSoftDemapper::new(spec);
    let nv = vec![1e-4_f64; batch];
    let input = DemapInput::<f64> {
        rx_i: &tx_i,
        rx_q: &tx_q,
        gain_i: None,
        gain_q: None,
        noise_var: &nv,
        method: DemapMethod::ExactLogMap,
    };
    let mut llrs = vec![Llr::new(0.0); batch * bps as usize];
    demapper.demap_llrs(input, &mut llrs);

    // Confirm every hard-decision bit matches the transmitted bit.
    for k in 0..batch {
        let expected = unpack_label_msb_first(label_stream[k], bps);
        for b in 0..bps as usize {
            let got = llrs[k * bps as usize + b].hard_decision();
            assert_eq!(
                got, expected[b],
                "round-trip bit mismatch at k={k}, b={b}, label={}",
                label_stream[k]
            );
        }
    }
}

/// Runs the oracle cross-check: for each received sample and bit
/// position, compares the demapper's exact log-MAP LLR against the
/// brute-force oracle computed from the spec's post-normalization points.
fn check_oracle_f64(spec: ModemSpec<f64>, rx_i: &[f64], rx_q: &[f64], nv: &[f64], tol: f64) {
    let (pts, labs, bps) = snapshot_spec_f64(&spec);
    let demapper = ReferenceSoftDemapper::new(spec);
    let input = DemapInput::<f64> {
        rx_i,
        rx_q,
        gain_i: None,
        gain_q: None,
        noise_var: nv,
        method: DemapMethod::ExactLogMap,
    };
    let mut llrs = vec![Llr::new(0.0); rx_i.len() * bps as usize];
    demapper.demap_llrs(input, &mut llrs);

    for k in 0..rx_i.len() {
        for b in 0..bps {
            let expected = oracle_log_map_llr(&pts, &labs, bps, rx_i[k], rx_q[k], nv[k], b);
            let got = llrs[k * bps as usize + b as usize].value() as f64;
            assert!(
                (got - expected).abs() <= tol,
                "oracle mismatch at k={k}, b={b}: got {got}, expected {expected}"
            );
        }
    }
}

// ---------------------------------------------------------------------
// Representative spec builders
// ---------------------------------------------------------------------

/// Non-Gray axis-4 constellation: unit-circle points at 0, pi/2, pi,
/// 3pi/2 with the label permutation `[0b00, 0b10, 0b11, 0b01]` (so that
/// adjacent angles differ by more than one bit in some cases).
fn axis4_non_gray_spec() -> ModemSpec<f64> {
    ModemSpecBuilder::<f64>::new()
        .bits_per_symbol(2)
        .points(vec![
            SymbolPoint::new(1.0, 0.0),
            SymbolPoint::new(0.0, 1.0),
            SymbolPoint::new(-1.0, 0.0),
            SymbolPoint::new(0.0, -1.0),
        ])
        .labels(vec![
            LabelWord::new(0b00, 2),
            LabelWord::new(0b10, 2),
            LabelWord::new(0b11, 2),
            LabelWord::new(0b01, 2),
        ])
        .normalization(Normalization::UnitAverageSymbolEnergy)
        .build()
}

/// 8-PSK on the unit circle with an arbitrary bijective label mapping.
fn psk8_spec() -> ModemSpec<f64> {
    let points: Vec<SymbolPoint<f64>> = (0..8)
        .map(|k| {
            let theta = (k as f64) * core::f64::consts::TAU / 8.0;
            SymbolPoint::new(theta.cos(), theta.sin())
        })
        .collect();
    // Non-Gray, non-identity bijection.
    let labels: Vec<LabelWord> = [0u16, 3, 1, 5, 2, 7, 4, 6]
        .iter()
        .map(|&b| LabelWord::new(b, 3))
        .collect();
    ModemSpecBuilder::<f64>::new()
        .bits_per_symbol(3)
        .points(points)
        .labels(labels)
        .normalization(Normalization::UnitAverageSymbolEnergy)
        .build()
}

/// Asymmetric 4-PAM on the I axis only (points `-7, -1, +3, +5` scaled
/// to unit average energy). Q is zero for every point; this exercises
/// the reference path's handling of constellations that are not
/// symmetric in I or around the origin.
fn pam4_asymmetric_spec() -> ModemSpec<f64> {
    ModemSpecBuilder::<f64>::new()
        .bits_per_symbol(2)
        .points(vec![
            SymbolPoint::new(-7.0, 0.0),
            SymbolPoint::new(-1.0, 0.0),
            SymbolPoint::new(3.0, 0.0),
            SymbolPoint::new(5.0, 0.0),
        ])
        .labels(vec![
            LabelWord::new(0b00, 2),
            LabelWord::new(0b01, 2),
            LabelWord::new(0b11, 2),
            LabelWord::new(0b10, 2),
        ])
        .normalization(Normalization::UnitAverageSymbolEnergy)
        .build()
}

// ---------------------------------------------------------------------
// Round-trip coverage (success criterion 1)
// ---------------------------------------------------------------------

#[test]
fn test_round_trip_axis4_non_gray_recovers_bits() {
    check_round_trip_f64(axis4_non_gray_spec(), 0xA11CE, 256);
}

#[test]
fn test_round_trip_8_psk_recovers_bits() {
    check_round_trip_f64(psk8_spec(), 0xBEEF, 256);
}

#[test]
fn test_round_trip_pam4_asymmetric_recovers_bits() {
    check_round_trip_f64(pam4_asymmetric_spec(), 0xD00D, 256);
}

#[test]
fn test_round_trip_preset_bpsk_recovers_bits() {
    let spec: ModemSpec<f64> = ModemSpec::<f64>::bpsk_with_scalar();
    check_round_trip_f64(spec, 0xB9, 128);
}

#[test]
fn test_round_trip_preset_qpsk_recovers_bits() {
    let spec: ModemSpec<f64> = ModemSpec::<f64>::gray_square_qam_with_scalar(4);
    check_round_trip_f64(spec, 0xC0FFEE, 256);
}

#[test]
fn test_round_trip_preset_qam16_recovers_bits() {
    let spec: ModemSpec<f64> = ModemSpec::<f64>::gray_square_qam_with_scalar(16);
    check_round_trip_f64(spec, 0xFEED, 256);
}

// ---------------------------------------------------------------------
// Analytic oracle cross-check (success criterion 2)
// ---------------------------------------------------------------------

fn oracle_rx_samples() -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    // Mix of points near/far from the origin, both axes, a range of N0.
    let rx_i = vec![0.7, -0.4, 0.1, -1.1, 0.9, -0.05, 0.33];
    let rx_q = vec![0.2, 0.8, -0.6, 0.05, -0.35, -1.2, 0.71];
    let nv = vec![0.3, 0.5, 0.7, 0.4, 0.25, 0.9, 0.6];
    (rx_i, rx_q, nv)
}

#[test]
fn test_oracle_axis4_non_gray_matches_brute_force() {
    let (rx_i, rx_q, nv) = oracle_rx_samples();
    check_oracle_f64(axis4_non_gray_spec(), &rx_i, &rx_q, &nv, 1e-3);
}

#[test]
fn test_oracle_8_psk_matches_brute_force() {
    let (rx_i, rx_q, nv) = oracle_rx_samples();
    check_oracle_f64(psk8_spec(), &rx_i, &rx_q, &nv, 1e-3);
}

#[test]
fn test_oracle_pam4_asymmetric_matches_brute_force() {
    let (rx_i, rx_q, nv) = oracle_rx_samples();
    check_oracle_f64(pam4_asymmetric_spec(), &rx_i, &rx_q, &nv, 1e-3);
}

#[test]
fn test_oracle_preset_bpsk_matches_brute_force() {
    let (rx_i, rx_q, nv) = oracle_rx_samples();
    let spec: ModemSpec<f64> = ModemSpec::<f64>::bpsk_with_scalar();
    check_oracle_f64(spec, &rx_i, &rx_q, &nv, 1e-3);
}

#[test]
fn test_oracle_preset_qpsk_matches_brute_force() {
    let (rx_i, rx_q, nv) = oracle_rx_samples();
    let spec: ModemSpec<f64> = ModemSpec::<f64>::gray_square_qam_with_scalar(4);
    check_oracle_f64(spec, &rx_i, &rx_q, &nv, 1e-3);
}

#[test]
fn test_oracle_preset_qam16_matches_brute_force() {
    let (rx_i, rx_q, nv) = oracle_rx_samples();
    let spec: ModemSpec<f64> = ModemSpec::<f64>::gray_square_qam_with_scalar(16);
    check_oracle_f64(spec, &rx_i, &rx_q, &nv, 1e-3);
}

#[test]
fn test_oracle_random_bijection_matches_brute_force() {
    // Random 3-bit permutation on the unit circle — exercises the
    // oracle on a builder-built, non-preset constellation that is not
    // one of the three named representative specs above.
    let n = 8usize;
    let perm = lcg_permutation(0x51EED, n);
    let points: Vec<SymbolPoint<f64>> = (0..n)
        .map(|k| {
            let theta = (k as f64) * core::f64::consts::TAU / (n as f64);
            SymbolPoint::new(theta.cos(), theta.sin())
        })
        .collect();
    let labels: Vec<LabelWord> = perm.iter().map(|&b| LabelWord::new(b, 3)).collect();
    let spec = ModemSpecBuilder::<f64>::new()
        .bits_per_symbol(3)
        .points(points)
        .labels(labels)
        .build();

    let (rx_i, rx_q, nv) = oracle_rx_samples();
    check_oracle_f64(spec, &rx_i, &rx_q, &nv, 1e-3);
}

// ---------------------------------------------------------------------
// Invalid builder input coverage (success criterion 3)
//
// One test per invariant enforced by `ModemSpecBuilder::build` /
// `ModemSpec::from_parts_checked`. Each test drives the public builder
// entry point and asserts the expected panic message.
// ---------------------------------------------------------------------

#[test]
#[should_panic(expected = "bits_per_symbol not set")]
fn test_builder_invariant_missing_bits_per_symbol() {
    let _ = ModemSpecBuilder::<f32>::new()
        .points(vec![
            SymbolPoint::new(1.0, 0.0),
            SymbolPoint::new(-1.0, 0.0),
        ])
        .labels(vec![LabelWord::new(0, 1), LabelWord::new(1, 1)])
        .build();
}

#[test]
#[should_panic(expected = "points not set")]
fn test_builder_invariant_missing_points() {
    let _ = ModemSpecBuilder::<f32>::new()
        .bits_per_symbol(1)
        .labels(vec![LabelWord::new(0, 1), LabelWord::new(1, 1)])
        .build();
}

#[test]
#[should_panic(expected = "labels not set")]
fn test_builder_invariant_missing_labels() {
    let _ = ModemSpecBuilder::<f32>::new()
        .bits_per_symbol(1)
        .points(vec![
            SymbolPoint::new(1.0, 0.0),
            SymbolPoint::new(-1.0, 0.0),
        ])
        .build();
}

#[test]
#[should_panic(expected = "raw constellation has zero energy")]
fn test_builder_invariant_zero_energy_constellation() {
    let _ = ModemSpecBuilder::<f32>::new()
        .bits_per_symbol(1)
        .points(vec![SymbolPoint::new(0.0, 0.0), SymbolPoint::new(0.0, 0.0)])
        .labels(vec![LabelWord::new(0, 1), LabelWord::new(1, 1)])
        .build();
}

#[test]
#[should_panic(expected = "ExplicitEs target must be strictly positive")]
fn test_builder_invariant_nonpositive_explicit_es() {
    let _ = ModemSpecBuilder::<f32>::new()
        .bits_per_symbol(1)
        .points(vec![
            SymbolPoint::new(1.0, 0.0),
            SymbolPoint::new(-1.0, 0.0),
        ])
        .labels(vec![LabelWord::new(0, 1), LabelWord::new(1, 1)])
        .normalization(Normalization::ExplicitEs(-1.0))
        .build();
}

#[test]
#[should_panic(expected = "bits_per_symbol must be in [1, 16]")]
fn test_builder_invariant_bits_per_symbol_out_of_range() {
    // bits_per_symbol = 0 forces `expected_len = 1` (1 << 0). We still
    // have to supply a non-empty constellation (to pass the zero-energy
    // check inside `compute_scale`) and a label; from_parts_checked
    // rejects the bits_per_symbol value before any other check.
    let _ = ModemSpecBuilder::<f32>::new()
        .bits_per_symbol(0)
        .points(vec![SymbolPoint::new(1.0, 0.0)])
        .labels(vec![LabelWord::new(0, 1)])
        .build();
}

#[test]
#[should_panic(expected = "points/labels length mismatch")]
fn test_builder_invariant_points_labels_length_mismatch() {
    let _ = ModemSpecBuilder::<f32>::new()
        .bits_per_symbol(2)
        .points(vec![
            SymbolPoint::new(1.0, 0.0),
            SymbolPoint::new(-1.0, 0.0),
        ])
        .labels(vec![LabelWord::new(0, 2), LabelWord::new(1, 2)])
        .build();
}

#[test]
#[should_panic(expected = "bit_channels length")]
fn test_builder_invariant_bit_channels_length_mismatch() {
    // 2 bits per symbol, but caller supplies only 1 bit-channel tag.
    let _ = ModemSpecBuilder::<f32>::new()
        .bits_per_symbol(2)
        .points(vec![
            SymbolPoint::new(1.0, 0.0),
            SymbolPoint::new(0.0, 1.0),
            SymbolPoint::new(-1.0, 0.0),
            SymbolPoint::new(0.0, -1.0),
        ])
        .labels(vec![
            LabelWord::new(0b00, 2),
            LabelWord::new(0b01, 2),
            LabelWord::new(0b10, 2),
            LabelWord::new(0b11, 2),
        ])
        .bit_channels(vec![BitChannelSemantics::Opaque(0)])
        .build();
}

#[test]
#[should_panic(expected = "expected 2")]
fn test_builder_invariant_label_width_mismatch() {
    // Labels declared as width 3 but bits_per_symbol is 2.
    let _ = ModemSpecBuilder::<f32>::new()
        .bits_per_symbol(2)
        .points(vec![
            SymbolPoint::new(1.0, 0.0),
            SymbolPoint::new(0.0, 1.0),
            SymbolPoint::new(-1.0, 0.0),
            SymbolPoint::new(0.0, -1.0),
        ])
        .labels(vec![
            LabelWord::new(0, 3),
            LabelWord::new(1, 3),
            LabelWord::new(2, 3),
            LabelWord::new(3, 3),
        ])
        .build();
}

#[test]
#[should_panic(expected = "not a bijection (duplicate")]
fn test_builder_invariant_duplicate_label_bits() {
    let _ = ModemSpecBuilder::<f32>::new()
        .bits_per_symbol(2)
        .points(vec![
            SymbolPoint::new(1.0, 0.0),
            SymbolPoint::new(0.0, 1.0),
            SymbolPoint::new(-1.0, 0.0),
            SymbolPoint::new(0.0, -1.0),
        ])
        .labels(vec![
            LabelWord::new(0b00, 2),
            LabelWord::new(0b01, 2),
            LabelWord::new(0b10, 2),
            LabelWord::new(0b10, 2), // duplicate of index 2
        ])
        .build();
}

// Note on invariants 6 (post-normalization unit energy) and 7
// (`normalization_scale > 0`): both live inside the sealed
// `ModemSpec::from_parts_checked` choke point but are not reachable
// through the public `ModemSpecBuilder` surface. `compute_scale`
// always derives a strictly-positive, unit-energy-matching scale from
// the supplied raw points (rejecting zero-energy and non-positive
// `ExplicitEs` earlier), so no public call path can deliver a bad
// scale to `from_parts_checked`. Those invariants are covered by the
// unit tests in `crates/gf2-coding/src/modem/spec.rs`
// (`test_invariant_nonpositive_scale`,
// `test_invariant_unit_energy_violated`), which exercise
// `from_parts_checked` directly.

#[test]
#[should_panic(expected = "at least one demap method")]
fn test_builder_invariant_no_demap_method_advertised() {
    let _ = ModemSpecBuilder::<f32>::new()
        .bits_per_symbol(1)
        .points(vec![
            SymbolPoint::new(1.0, 0.0),
            SymbolPoint::new(-1.0, 0.0),
        ])
        .labels(vec![LabelWord::new(0, 1), LabelWord::new(1, 1)])
        .capabilities(ModemCapabilities {
            supports_exact_log_map: false,
            supports_max_log: false,
            analysis: &[],
        })
        .build();
}

#[test]
#[should_panic(expected = "capabilities.analysis length")]
fn test_builder_invariant_capabilities_analysis_length_mismatch() {
    // Supply a non-empty but wrong-length analysis slice (length 1 for
    // bits_per_symbol = 2). `ModemSpecBuilder::build` only back-fills
    // the analysis slot when the caller's slice is empty, so a
    // length-1 slice is forwarded verbatim and rejected by
    // `from_parts_checked`'s invariant 9.
    use gf2_coding::modem::BitChannelAnalysis;
    static ANALYSIS_ONE: &[BitChannelAnalysis] = &[BitChannelAnalysis {
        symmetric_llr_distribution: false,
        conditionally_independent: false,
        closed_form_llr_available: false,
    }];
    let _ = ModemSpecBuilder::<f32>::new()
        .bits_per_symbol(2)
        .points(vec![
            SymbolPoint::new(1.0, 0.0),
            SymbolPoint::new(0.0, 1.0),
            SymbolPoint::new(-1.0, 0.0),
            SymbolPoint::new(0.0, -1.0),
        ])
        .labels(vec![
            LabelWord::new(0b00, 2),
            LabelWord::new(0b01, 2),
            LabelWord::new(0b10, 2),
            LabelWord::new(0b11, 2),
        ])
        .capabilities(ModemCapabilities {
            supports_exact_log_map: true,
            supports_max_log: true,
            analysis: ANALYSIS_ONE,
        })
        .build();
}
