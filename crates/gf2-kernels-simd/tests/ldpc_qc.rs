use gf2_kernels_simd::ldpc_lanes::{lane_check_step_portable, F32x8, LaneMinSumRule};
use gf2_kernels_simd::ldpc_qc::{detect, QcKernels};

fn backends() -> Vec<QcKernels> {
    let mut kernels = vec![QcKernels::portable()];
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

/// The scalar contract's output on each edge of one lifted check.
fn want(rule: LaneMinSumRule, column: &[f32]) -> Vec<u32> {
    let gathered: Vec<_> = column.iter().map(|&value| F32x8([value; 8])).collect();
    let mut want = vec![F32x8::ZERO; column.len()];
    lane_check_step_portable(rule, &[0, column.len() as u32], &gathered, &mut want);
    want.iter().map(|lanes| lanes.0[0].to_bits()).collect()
}

/// Inputs from a small alphabet, so lifted checks hold duplicate minima,
/// signed zeros, NaN and infinite magnitudes in every arrangement.
#[test]
fn every_backend_matches_the_check_contract_on_ties() {
    let alphabet = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        2.0,
        -2.0,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ];
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        alphabet[(state % alphabet.len() as u64) as usize]
    };
    for kernel in backends() {
        for z in [1, 5, 8, 13, 24] {
            for degree in [2, 3, 4, 7, 19] {
                for rule in [
                    LaneMinSumRule::Plain,
                    LaneMinSumRule::Normalized(0.75),
                    LaneMinSumRule::Offset(0.5),
                ] {
                    for _ in 0..8 {
                        let input: Vec<f32> = (0..degree * z).map(|_| next()).collect();
                        let mut output = vec![123.0; input.len()];
                        kernel.check_block(rule, &input, &mut output, degree, z);
                        for lane in 0..z {
                            let column: Vec<f32> =
                                (0..degree).map(|edge| input[edge * z + lane]).collect();
                            let got: Vec<u32> = (0..degree)
                                .map(|edge| output[edge * z + lane].to_bits())
                                .collect();
                            assert_eq!(got, want(rule, &column), "{rule:?} {column:?}");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn every_backend_rotates_differences_and_sums_lane_by_lane() {
    let special = [
        0.0,
        -0.0,
        f32::NAN,
        f32::INFINITY,
        -f32::MAX,
        1.5,
        -2.25,
        f32::from_bits(1),
    ];
    for kernel in backends() {
        for z in [1, 2, 7, 8, 9, 16, 17, 63, 64, 65, 384] {
            let value = |i: usize, salt: usize| special[(i * 7 + salt) % special.len()] + i as f32;
            let block: Vec<f32> = (0..z).map(|i| value(i, 1)).collect();
            let other: Vec<f32> = (0..z).map(|i| value(i, 3)).collect();
            for shift in [0, 1, z / 2, z - 1].into_iter().filter(|&shift| shift < z) {
                let mut out = vec![123.0; z];
                kernel.rotated_difference(&block, shift, &other, &mut out);
                for i in 0..z {
                    let want = block[(i + shift) % z] - other[i];
                    assert_eq!(
                        out[i].to_bits(),
                        want.to_bits(),
                        "z {z}, shift {shift}, lane {i}"
                    );
                }
                for lanes in [0..z, 0..z / 2, z / 3..z, shift..shift, shift.min(z - 1)..z] {
                    let mut sums = block.clone();
                    kernel.add_rotated(&mut sums, lanes.clone(), &other, shift);
                    for j in 0..z {
                        let want = if lanes.contains(&j) {
                            block[j] + other[(j + z - shift) % z]
                        } else {
                            block[j]
                        };
                        assert_eq!(
                            sums[j].to_bits(),
                            want.to_bits(),
                            "z {z}, shift {shift}, lane {j}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn rotations_panic_before_writing_outside_their_domain() {
    for kernel in backends() {
        let block = vec![1.0; 9];
        let mut out = vec![123.0; 9];
        for (shift, length) in [(9, 9), (0, 8)] {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                kernel.rotated_difference(&block, shift, &vec![0.0; length], &mut out)
            }));
            assert!(result.is_err());
        }
        assert!(out.iter().all(|&value| value == 123.0));
        let mut sums = vec![123.0; 9];
        for (lanes, shift) in [(0..10, 0), (0..9, 9)] {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                kernel.add_rotated(&mut sums, lanes.clone(), &block, shift)
            }));
            assert!(result.is_err());
        }
        assert!(sums.iter().all(|&value| value == 123.0));
    }
}
