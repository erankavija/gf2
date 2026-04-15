//! Shared helpers for the GPU Gray-QAM correctness tests and crossover
//! benchmark. Included via `#[path] mod gpu_bench_support;` from the
//! sibling test/bench files so neither has to duplicate the
//! modem-preset dispatch or the deterministic sample generator.

#![allow(dead_code)]

use gf2_coding::modem::test_oracle::Lcg;
use gf2_coding::modem::ModemSpec;

/// Returns the `ModemSpec<f32>` for a given modulation order, routing
/// BPSK (`order == 2`) through `ModemSpec::bpsk()` and every QAM order
/// through `ModemSpec::gray_square_qam`.
pub fn spec_for_order(order: usize) -> ModemSpec<f32> {
    if order == 2 {
        ModemSpec::<f32>::bpsk()
    } else {
        ModemSpec::<f32>::gray_square_qam(order)
    }
}

/// Generates `(rx_i, rx_q, noise_var)` of length `batch` from the
/// workspace SSOT LCG, seeded by `seed ^ (order as u64)`. Sample range
/// is `[-2, 2]` per axis (scale covers unit-average-energy Gray-QAM
/// constellations up to `m = 8`), noise variance is drawn uniformly
/// from `[0.05, 2.0]`.
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
