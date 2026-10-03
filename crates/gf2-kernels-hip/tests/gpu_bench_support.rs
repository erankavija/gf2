//! Shared helpers for the GPU Gray-QAM correctness tests and benchmark,
//! included via `#[path] mod gpu_bench_support;`.

#![allow(dead_code)]

use gf2_coding::modem::test_oracle::Lcg;
use gf2_coding::modem::ModemSpec;

pub fn spec_for_order(order: usize) -> ModemSpec<f32> {
    if order == 2 {
        ModemSpec::<f32>::bpsk()
    } else {
        ModemSpec::<f32>::gray_square_qam(order)
    }
}

/// Generates `(rx_i, rx_q, noise_var)` of length `batch`. The `[-2, 2]`
/// sample range per axis covers unit-average-energy Gray-QAM constellations
/// up to `m = 8`.
pub fn gen_batch(order: usize, batch: usize, seed: u64) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let mut rng = Lcg::new(seed ^ (order as u64));
    let mut rx_i = Vec::with_capacity(batch);
    let mut rx_q = Vec::with_capacity(batch);
    let mut nv = Vec::with_capacity(batch);
    for _ in 0..batch {
        rx_i.push(rng.next_unit_f32() * 2.0);
        rx_q.push(rng.next_unit_f32() * 2.0);
        nv.push(rng.next_positive_f32(0.05, 2.0));
    }
    (rx_i, rx_q, nv)
}
