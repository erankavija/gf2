//! Cross-validation: GPU BCJR vs CPU BCJR.
//!
//! Compares GPU batch BCJR output against the reference CPU implementation
//! in `gf2-coding::bcjr` to verify numerical correctness.

use gf2_coding::bcjr::BcjrDecoder;
use gf2_coding::drm::DrmCode;
use gf2_coding::llr::Llr;
use gf2_coding::traits::BlockEncoder;
use gf2_core::BitVec;
use gf2_kernels_hip::GpuBcjrBatch;

/// Extract h_cols from a BitMatrix (same logic as BcjrDecoder::new).
fn extract_h_cols(h: &gf2_core::BitMatrix) -> Vec<u32> {
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
fn test_gpu_cpu_hamming74_crosscheck() {
    let h = gf2_core::bitmatrix![
        1, 1, 0, 1, 1, 0, 0;
        1, 0, 1, 1, 0, 1, 0;
        0, 1, 1, 1, 0, 0, 1
    ];
    let h_cols = extract_h_cols(&h);
    let cpu = BcjrDecoder::new(&h);
    let mut gpu = GpuBcjrBatch::new(&h_cols, 7, 4, 32).unwrap();

    let test_vectors: Vec<Vec<f32>> = vec![
        vec![5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0],
        vec![-5.0, -5.0, -5.0, -5.0, -5.0, -5.0, -5.0],
        vec![2.0, -1.5, 3.0, 0.5, -2.0, 1.0, -0.5],
        vec![-3.0, 2.0, 1.0, -1.0, 0.5, -2.5, 3.0],
        vec![1.5, 1.5, -2.0, 2.5, -1.0, 0.8, -1.5],
        vec![4.0, -3.0, 2.0, -1.0, 3.0, -2.0, 1.0],
        vec![-0.5, 0.5, -0.5, 0.5, -0.5, 0.5, -0.5],
        vec![0.1, -0.1, 0.2, -0.3, 0.4, -0.5, 0.6],
    ];

    // Batch all on GPU
    let (gpu_app, _gpu_ext) = gpu.decode_batch(&test_vectors).unwrap();

    // Compare each against CPU
    for (idx, llrs) in test_vectors.iter().enumerate() {
        let cpu_input: Vec<Llr> = llrs.iter().map(|&v| Llr::new(v)).collect();
        let cpu_result = cpu.decode_siso(&cpu_input);

        for j in 0..7 {
            let diff = (gpu_app[idx][j] - cpu_result.app_llrs[j].value()).abs();
            assert!(
                diff < 0.01,
                "Hamming(7,4) vector {}, bit {}: GPU={:.4}, CPU={:.4}, diff={:.4}",
                idx,
                j,
                gpu_app[idx][j],
                cpu_result.app_llrs[j].value(),
                diff
            );
        }
    }
}

#[test]
fn test_gpu_cpu_drm32_noiseless_crosscheck() {
    let code = DrmCode::drm_32_21();
    let h = code.parity_check();
    let h_cols = extract_h_cols(h);
    let cpu = BcjrDecoder::new(h);
    let mut gpu = GpuBcjrBatch::new(&h_cols, 32, 21, 16).unwrap();

    // Test with 10 different codewords (noiseless)
    let mut inputs = Vec::new();
    for seed in 0..10u64 {
        let mut msg = BitVec::with_capacity(21);
        for i in 0..21 {
            msg.push_bit(((seed >> (i % 8)) & 1) == 1);
        }
        let cw = code.encode(&msg);
        let llrs: Vec<f32> = (0..32)
            .map(|j| if cw.get(j) { -8.0 } else { 8.0 })
            .collect();
        inputs.push(llrs);
    }

    let (gpu_app, _) = gpu.decode_batch(&inputs).unwrap();

    for (idx, llrs) in inputs.iter().enumerate() {
        let cpu_input: Vec<Llr> = llrs.iter().map(|&v| Llr::new(v)).collect();
        let cpu_result = cpu.decode_siso(&cpu_input);

        for j in 0..32 {
            let diff = (gpu_app[idx][j] - cpu_result.app_llrs[j].value()).abs();
            assert!(
                diff < 0.2,
                "dRM(32,21) vector {}, bit {}: GPU={:.4}, CPU={:.4}, diff={:.4}",
                idx,
                j,
                gpu_app[idx][j],
                cpu_result.app_llrs[j].value(),
                diff
            );
        }

        // Hard decisions must match
        for j in 0..32 {
            let gpu_hard = gpu_app[idx][j] < 0.0;
            let cpu_hard = cpu_result.app_llrs[j].hard_decision();
            assert_eq!(
                gpu_hard, cpu_hard,
                "dRM(32,21) vector {}, bit {}: hard decision mismatch",
                idx, j
            );
        }
    }
}

#[test]
fn test_gpu_cpu_drm32_noisy_crosscheck() {
    let code = DrmCode::drm_32_21();
    let h = code.parity_check();
    let h_cols = extract_h_cols(h);
    let cpu = BcjrDecoder::new(h);
    let mut gpu = GpuBcjrBatch::new(&h_cols, 32, 21, 16).unwrap();

    // Moderate SNR test vectors (simulating noisy channel)
    let test_vectors: Vec<Vec<f32>> = vec![
        (0..32).map(|j| 1.5 * ((j % 3) as f32 - 1.0)).collect(),
        (0..32).map(|j| 2.0 * (((j * 7 + 3) % 5) as f32 - 2.0) / 2.0).collect(),
        (0..32).map(|j| if j < 16 { 1.0 } else { -1.0 }).collect(),
        (0..32).map(|j| 0.5 * ((j as f32).sin() * 3.0)).collect(),
        (0..32).map(|j| if j % 2 == 0 { 2.5 } else { -1.5 }).collect(),
    ];

    let (gpu_app, _) = gpu.decode_batch(&test_vectors).unwrap();

    for (idx, llrs) in test_vectors.iter().enumerate() {
        let cpu_input: Vec<Llr> = llrs.iter().map(|&v| Llr::new(v)).collect();
        let cpu_result = cpu.decode_siso(&cpu_input);

        for j in 0..32 {
            let diff = (gpu_app[idx][j] - cpu_result.app_llrs[j].value()).abs();
            assert!(
                diff < 0.2,
                "noisy dRM vector {}, bit {}: GPU={:.4}, CPU={:.4}, diff={:.4}",
                idx,
                j,
                gpu_app[idx][j],
                cpu_result.app_llrs[j].value(),
                diff
            );
        }
    }
}

#[test]
fn test_gpu_batch64_matches_serial_cpu() {
    let code = DrmCode::drm_32_21();
    let h = code.parity_check();
    let h_cols = extract_h_cols(h);
    let cpu = BcjrDecoder::new(h);
    let mut gpu = GpuBcjrBatch::new(&h_cols, 32, 21, 64).unwrap();

    // Build 64 diverse inputs
    let inputs: Vec<Vec<f32>> = (0..64)
        .map(|idx| {
            (0..32)
                .map(|j| {
                    let v = ((idx * 7 + j * 13) % 20) as f32 - 10.0;
                    v * 0.3
                })
                .collect()
        })
        .collect();

    let (gpu_app, _) = gpu.decode_batch(&inputs).unwrap();

    for (idx, llrs) in inputs.iter().enumerate() {
        let cpu_input: Vec<Llr> = llrs.iter().map(|&v| Llr::new(v)).collect();
        let cpu_result = cpu.decode_siso(&cpu_input);

        for j in 0..32 {
            let diff = (gpu_app[idx][j] - cpu_result.app_llrs[j].value()).abs();
            assert!(
                diff < 0.2,
                "batch64 idx {}, bit {}: GPU={:.4}, CPU={:.4}, diff={:.4}",
                idx,
                j,
                gpu_app[idx][j],
                cpu_result.app_llrs[j].value(),
                diff
            );
        }
    }
}
