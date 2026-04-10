//! Throughput benchmark: GPU batch BCJR vs CPU serial BCJR.
//!
//! Measures wall-clock time for 64 dRM(32,21) SISO decodes using:
//! 1. GPU batch (single kernel launch for all 64)
//! 2. CPU serial (64 individual BcjrDecoder::decode_siso calls)

use gf2_coding::bcjr::BcjrDecoder;
use gf2_coding::drm::DrmCode;
use gf2_coding::llr::Llr;
use gf2_core::BitMatrix;
use gf2_kernels_hip::GpuBcjrBatch;
use std::time::Instant;

fn extract_h_cols(h: &BitMatrix) -> Vec<u32> {
    let m = h.rows();
    let n = h.cols();
    (0..n)
        .map(|j| {
            let col_bv = h.col_as_bitvec(j);
            let mut col = 0u32;
            for i in 0..m {
                if col_bv.get(i) {
                    col |= 1 << i;
                }
            }
            col
        })
        .collect()
}

#[test]
fn bench_gpu_vs_cpu_batch64() {
    let code = DrmCode::drm_32_21();
    let h = code.parity_check();
    let h_cols = extract_h_cols(h);
    let cpu = BcjrDecoder::new(h);
    let mut gpu = GpuBcjrBatch::new(&h_cols, 32, 21, 64).unwrap();

    let batch_size = 64;
    let inputs: Vec<Vec<f32>> = (0..batch_size)
        .map(|idx| {
            (0..32)
                .map(|j| ((idx * 7 + j * 13) % 20) as f32 * 0.3 - 3.0)
                .collect()
        })
        .collect();

    // Warmup GPU
    let _ = gpu.decode_batch(&inputs).unwrap();

    // Benchmark GPU (batch)
    let gpu_iters = 100;
    let gpu_start = Instant::now();
    for _ in 0..gpu_iters {
        let _ = gpu.decode_batch(&inputs).unwrap();
    }
    let gpu_elapsed = gpu_start.elapsed();
    let gpu_per_batch = gpu_elapsed / gpu_iters;

    // Benchmark CPU (serial)
    let cpu_iters = 100;
    let cpu_start = Instant::now();
    for _ in 0..cpu_iters {
        for llrs in &inputs {
            let cpu_input: Vec<Llr> = llrs.iter().map(|&v| Llr::new(v)).collect();
            let _ = cpu.decode_siso(&cpu_input);
        }
    }
    let cpu_elapsed = cpu_start.elapsed();
    let cpu_per_batch = cpu_elapsed / cpu_iters;

    let speedup = cpu_per_batch.as_secs_f64() / gpu_per_batch.as_secs_f64();

    eprintln!(
        "\n=== BCJR Batch-64 Throughput ===\n\
         GPU batch: {:.1} ms / 64 decodes ({:.1} us/decode)\n\
         CPU serial: {:.1} ms / 64 decodes ({:.1} us/decode)\n\
         Speedup: {:.1}x\n",
        gpu_per_batch.as_secs_f64() * 1000.0,
        gpu_per_batch.as_secs_f64() * 1_000_000.0 / batch_size as f64,
        cpu_per_batch.as_secs_f64() * 1000.0,
        cpu_per_batch.as_secs_f64() * 1_000_000.0 / batch_size as f64,
        speedup,
    );

    // Acceptance criterion: GPU must be at least 5x faster
    assert!(
        speedup >= 5.0,
        "GPU speedup {:.1}x is below 5x threshold",
        speedup
    );
}
