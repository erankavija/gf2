//! Cross-validation: GPU BCJR vs CPU BCJR.
//!
//! Compares GPU batch BCJR output against the reference CPU implementation
//! in `gf2-coding::bcjr` to verify numerical correctness.

use gf2_coding::bcjr::BcjrDecoder;
use gf2_coding::drm::DrmCode;
use gf2_coding::llr::Llr;
use gf2_coding::traits::BlockEncoder;
use gf2_core::BitVec;
use gf2_kernels_hip::{extract_h_cols, GpuBcjrBatch};

#[test]
fn test_gpu_cpu_hamming74_crosscheck() {
    let h = gf2_core::bitmatrix![
        1, 1, 0, 1, 1, 0, 0;
        1, 0, 1, 1, 0, 1, 0;
        0, 1, 1, 1, 0, 0, 1
    ];
    let h_cols = extract_h_cols(&h);
    let cpu = BcjrDecoder::new(&h);
    let gpu = GpuBcjrBatch::new(&h_cols, 7, 4, 32).unwrap();

    // 25 deterministic test vectors (acceptance criterion requires 25)
    let test_vectors: Vec<Vec<f32>> = vec![
        vec![5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0],
        vec![-5.0, -5.0, -5.0, -5.0, -5.0, -5.0, -5.0],
        vec![2.0, -1.5, 3.0, 0.5, -2.0, 1.0, -0.5],
        vec![-3.0, 2.0, 1.0, -1.0, 0.5, -2.5, 3.0],
        vec![1.5, 1.5, -2.0, 2.5, -1.0, 0.8, -1.5],
        vec![4.0, -3.0, 2.0, -1.0, 3.0, -2.0, 1.0],
        vec![-0.5, 0.5, -0.5, 0.5, -0.5, 0.5, -0.5],
        vec![0.1, -0.1, 0.2, -0.3, 0.4, -0.5, 0.6],
        vec![10.0, -10.0, 10.0, -10.0, 10.0, -10.0, 10.0],
        vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0],
        vec![-7.0, -6.0, -5.0, -4.0, -3.0, -2.0, -1.0],
        vec![3.3, -2.2, 1.1, -0.5, 0.5, -1.1, 2.2],
        vec![-1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0],
        vec![8.0, 0.1, -0.1, 0.1, -0.1, 0.1, 8.0],
        vec![0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01],
        vec![-0.01, -0.01, -0.01, -0.01, -0.01, -0.01, -0.01],
        vec![5.0, -5.0, 5.0, -5.0, 0.0, 0.0, 0.0],
        vec![0.0, 0.0, 0.0, 5.0, -5.0, 5.0, -5.0],
        vec![2.5, 2.5, -2.5, -2.5, 2.5, -2.5, 2.5],
        vec![-4.0, 3.0, -2.0, 1.0, 0.0, -1.0, 2.0],
        vec![6.0, -4.0, 2.0, 0.0, -2.0, 4.0, -6.0],
        vec![1.0, -1.0, 2.0, -2.0, 3.0, -3.0, 4.0],
        vec![-8.0, 8.0, -4.0, 4.0, -2.0, 2.0, -1.0],
        vec![3.0, 3.0, 3.0, -3.0, -3.0, -3.0, 0.0],
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
    let gpu = GpuBcjrBatch::new(&h_cols, 32, 21, 16).unwrap();

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
    let gpu = GpuBcjrBatch::new(&h_cols, 32, 21, 16).unwrap();

    // 10 moderate-SNR test vectors (acceptance criterion requires 10)
    let test_vectors: Vec<Vec<f32>> = vec![
        (0..32).map(|j| 1.5 * ((j % 3) as f32 - 1.0)).collect(),
        (0..32)
            .map(|j| 2.0 * (((j * 7 + 3) % 5) as f32 - 2.0) / 2.0)
            .collect(),
        (0..32).map(|j| if j < 16 { 1.0 } else { -1.0 }).collect(),
        (0..32).map(|j| 0.5 * ((j as f32).sin() * 3.0)).collect(),
        (0..32)
            .map(|j| if j % 2 == 0 { 2.5 } else { -1.5 })
            .collect(),
        (0..32).map(|j| ((j * 11 + 5) % 7) as f32 - 3.0).collect(),
        (0..32).map(|j| 0.8 * ((j as f32).cos() * 4.0)).collect(),
        (0..32)
            .map(|j| if j % 4 < 2 { 1.5 } else { -2.0 })
            .collect(),
        (0..32)
            .map(|j| ((j * 3 + 1) % 9) as f32 * 0.5 - 2.0)
            .collect(),
        (0..32).map(|j| 3.0 * ((j as f32) * 0.2).sin()).collect(),
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
    let gpu = GpuBcjrBatch::new(&h_cols, 32, 21, 64).unwrap();

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

#[test]
fn test_gpu_turbo_ebch16_convergence() {
    use gf2_coding::bch::extended::ExtendedBchCode;
    use gf2_coding::product::{ProductCode, TurboDecoder, TurboDecoderConfig};

    let component = ExtendedBchCode::ebch_16_11();
    let product = ProductCode::new(component.clone());
    // gf2-coding is compiled with feature "hip" here (see Cargo.toml dev-dep),
    // so use_gpu_bcjr is available and routes through SisoEngine::GpuBcjr.
    let config = TurboDecoderConfig {
        max_iterations: 5,
        use_gpu_bcjr: true,
        ..TurboDecoderConfig::default()
    };
    let decoder = TurboDecoder::new(component, config);

    // All-zero codeword with high-confidence LLRs
    let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];
    let result = decoder.decode(&llrs);

    assert!(
        result.converged,
        "GPU BCJR turbo decoder should converge on high-SNR all-zeros"
    );
    assert_eq!(result.decoded_bits.len(), product.k());
    assert_eq!(
        result.decoded_bits.count_ones(),
        0,
        "Decoded message should be all zeros"
    );
}
