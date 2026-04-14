//! Shared deterministic input generators for the modem bench suite.
//!
//! This module is not compiled as its own `[[bench]]` target — it is
//! pulled in from sibling bench files via `mod bench_support;`. Kept
//! here to avoid duplicating the `deterministic_bits` / `deterministic_rx`
//! helpers across `modem_cpu.rs` and `modem_generic_vs_fast.rs`. Both
//! helpers route through the workspace SSOT RNG
//! [`gf2_coding::modem::test_oracle::Lcg`].

#![allow(dead_code)]

use gf2_coding::modem::test_oracle::{bit_stream, Lcg};

/// Deterministic bit pattern to feed mappers. Routes through the
/// shared `modem::test_oracle::Lcg` SSOT helper so benches cannot
/// drift from the test-side RNG contract.
pub fn deterministic_bits(n_bits: usize) -> Vec<bool> {
    bit_stream(0x9E37_79B9_7F4A_7C15, n_bits)
}

/// Deterministic received-sample scratch `(rx_i, rx_q, noise_var)` for
/// the demapper bench. Samples are uniform in `[-1, 1]` per axis; noise
/// variance is a constant 0.25. Uses the shared
/// `modem::test_oracle::Lcg` SSOT helper.
pub fn deterministic_rx(n: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let mut rng = Lcg::new(0x9E37_79B9_7F4A_7C15);
    // next_unit_f32() already emits samples in [-1, 1]; no further scaling.
    let rx_i: Vec<f32> = (0..n).map(|_| rng.next_unit_f32()).collect();
    let rx_q: Vec<f32> = (0..n).map(|_| rng.next_unit_f32()).collect();
    let noise_var = vec![0.25_f32; n];
    (rx_i, rx_q, noise_var)
}
