use gf2_kernels_simd::ldpc_lanes::{lane_check_step_portable, F32x8, LaneMinSumRule};
use gf2_kernels_simd::ldpc_qc::{detect, QcCheckFn};

fn backends() -> Vec<QcCheckFn> {
    let mut kernels = vec![QcCheckFn::portable()];
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    assert_eq!(
        detect().is_some(),
        std::arch::is_x86_feature_detected!("avx2")
    );
    if let Some(kernel) = detect() {
        kernels.push(kernel);
    }
    kernels
}

#[test]
fn every_backend_matches_the_shared_check_contract() {
    let special = [
        0.0,
        -0.0,
        f32::NAN,
        -f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MAX,
        -f32::MAX,
        f32::MIN_POSITIVE,
        -f32::MIN_POSITIVE,
        f32::from_bits(1),
        -f32::from_bits(1),
        2.0,
        -2.0,
    ];
    for kernel in backends() {
        for z in [0, 1, 7, 8, 9, 16, 17, 63, 64, 65] {
            for degree in [0, 1, 2, 3, 9, 19] {
                for rule in [
                    LaneMinSumRule::Plain,
                    LaneMinSumRule::Normalized(0.75),
                    LaneMinSumRule::Normalized(1.0),
                    LaneMinSumRule::Offset(0.0),
                    LaneMinSumRule::Offset(0.5),
                ] {
                    let input: Vec<f32> = (0..degree * z)
                        .map(|i| special[i % special.len()])
                        .collect();
                    let mut output = vec![123.0; input.len()];
                    kernel.check_block(rule, &input, &mut output, degree, z);
                    for lane in 0..z {
                        let gathered: Vec<_> = (0..degree)
                            .map(|edge| F32x8([input[edge * z + lane]; 8]))
                            .collect();
                        let mut want = vec![F32x8::ZERO; degree];
                        lane_check_step_portable(rule, &[0, degree as u32], &gathered, &mut want);
                        for edge in 0..degree {
                            assert_eq!(
                                output[edge * z + lane].to_bits(),
                                want[edge].0[0].to_bits(),
                                "{rule:?}, z {z}, degree {degree}, edge {edge}, lane {lane}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn invalid_lengths_panic_before_writing() {
    for kernel in backends() {
        for (inputs, degree, z) in [
            (vec![0.0; 16], 2, 9),
            (vec![0.0; 18], 2, 9),
            (vec![], usize::MAX, 2),
        ] {
            let mut output = vec![123.0; 16];
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                kernel.check_block(LaneMinSumRule::Plain, &inputs, &mut output, degree, z)
            }));
            assert!(result.is_err());
            assert!(output.iter().all(|&value| value == 123.0));
        }
    }
}
