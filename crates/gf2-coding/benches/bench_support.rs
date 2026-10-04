//! Deterministic input generators shared by the modem benches through
//! `mod bench_support;`, drawn from [`gf2_coding::modem::test_oracle::Lcg`].

#![allow(dead_code)]

use gf2_coding::modem::test_oracle::{bit_stream, Lcg};

/// Deterministic bit pattern to feed mappers.
pub fn deterministic_bits(n_bits: usize) -> Vec<bool> {
    bit_stream(0x9E37_79B9_7F4A_7C15, n_bits)
}

/// Deterministic received samples `(rx_i, rx_q, noise_var)`: uniform in
/// `[-1, 1]` per axis, with noise variance 0.25.
pub fn deterministic_rx(n: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let mut rng = Lcg::new(0x9E37_79B9_7F4A_7C15);
    let rx_i: Vec<f32> = (0..n).map(|_| rng.next_unit_f32()).collect();
    let rx_q: Vec<f32> = (0..n).map(|_| rng.next_unit_f32()).collect();
    let noise_var = vec![0.25_f32; n];
    (rx_i, rx_q, noise_var)
}
