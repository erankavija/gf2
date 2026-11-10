use gf2_kernels_simd::ldpc_lanes::{lane_check_step_portable, F32x8, LaneMinSumRule};
use gf2_kernels_simd::ldpc_qc::{detect, QcAccumulation, QcBlock, QcKernels, QcSegment, QcTerm};

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
                    LaneMinSumRule::Offset(f32::MAX),
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
                    LaneMinSumRule::Offset(2.0),
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
fn every_backend_rotates_differences_and_accumulates_in_term_order() {
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
            }

            // A plane of four circulants; the term lists repeat one, wrap
            // inside every eight-lane chunk and add in a fixed order.
            let plane: Vec<f32> = (0..4 * z).map(|i| value(i, 5) * 0.5).collect();
            let term = |circulant: usize, shift: usize| QcTerm {
                offset: circulant * z,
                shift: shift % z,
            };
            let lists = [
                vec![],
                vec![term(0, 0)],
                vec![term(1, z - 1), term(0, 3), term(3, z / 2)],
                vec![term(2, 5), term(2, 5), term(0, 11), term(3, 1), term(1, 7)],
            ];
            for terms in &lists {
                for lanes in [0..z, 0..z / 2, z / 3..z, z / 2..z / 2, (z - 1)..z] {
                    let plan = QcAccumulation::new(
                        z,
                        plane.len(),
                        vec![0, 1],
                        vec![QcSegment {
                            lanes: lanes.clone(),
                            terms: 0..terms.len(),
                        }],
                        terms.clone(),
                    );
                    let mut beliefs = vec![123.0; z];
                    kernel.accumulate(&plan, &mut beliefs, &block, &plane);
                    for j in 0..z {
                        let want = if lanes.contains(&j) {
                            terms.iter().fold(block[j], |sum, term| {
                                sum + plane[term.offset + (j + z - term.shift) % z]
                            })
                        } else {
                            123.0
                        };
                        assert_eq!(
                            beliefs[j].to_bits(),
                            want.to_bits(),
                            "z {z}, terms {terms:?}, lanes {lanes:?}, lane {j}"
                        );
                    }
                }
            }
        }
    }
}

/// Several column blocks, each split into segments with their own term
/// lists, one block without segments: every lane equals its segment's sum.
#[test]
fn every_backend_accumulates_each_segment_of_each_column_block() {
    for kernel in backends() {
        for z in [1, 7, 8, 9, 17, 64, 65] {
            let plane: Vec<f32> = (0..3 * z).map(|i| (i as f32).mul_add(0.25, -3.0)).collect();
            let term = |circulant: usize, shift: usize| QcTerm {
                offset: circulant * z,
                shift: shift % z,
            };
            let terms = vec![
                term(0, 1),
                term(2, z / 2),
                term(1, 3),
                term(1, 0),
                term(2, 5),
            ];
            let split = z / 3;
            let segments = vec![
                QcSegment {
                    lanes: 0..split,
                    terms: 0..2,
                },
                QcSegment {
                    lanes: split..z,
                    terms: 2..5,
                },
                QcSegment {
                    lanes: 0..z,
                    terms: 1..4,
                },
            ];
            let plan = QcAccumulation::new(
                z,
                plane.len(),
                vec![0, 2, 2, 3],
                segments.clone(),
                terms.clone(),
            );
            assert_eq!(plan.col_blocks(), 3);
            let channel: Vec<f32> = (0..3 * z).map(|i| i as f32 - 1.5).collect();
            let mut beliefs = vec![123.0; 3 * z];
            kernel.accumulate(&plan, &mut beliefs, &channel, &plane);
            let owner = [vec![0, 1], vec![], vec![2]];
            for block in 0..3 {
                for j in 0..z {
                    let segment = owner[block]
                        .iter()
                        .map(|&s| &segments[s])
                        .find(|s| s.lanes.contains(&j));
                    let want = segment.map_or(123.0, |segment| {
                        terms[segment.terms.clone()]
                            .iter()
                            .fold(channel[block * z + j], |sum, term| {
                                sum + plane[term.offset + (j + z - term.shift) % z]
                            })
                    });
                    assert_eq!(
                        beliefs[block * z + j].to_bits(),
                        want.to_bits(),
                        "z {z}, block {block}, lane {j}"
                    );
                }
            }
        }
    }
}

#[test]
fn accumulation_plans_outside_their_domain_are_refused() {
    let term = |offset, shift| QcTerm { offset, shift };
    let segment = |lanes, terms| QcSegment { lanes, terms };
    for (offsets, segments, terms) in [
        (vec![0, 1], vec![segment(0..10, 0..1)], vec![term(0, 0)]),
        (vec![0, 1], vec![segment(0..9, 0..2)], vec![term(0, 0)]),
        (vec![0, 1], vec![segment(0..9, 0..1)], vec![term(0, 9)]),
        (vec![0, 1], vec![segment(0..9, 0..1)], vec![term(10, 0)]),
        (vec![1, 1], vec![segment(0..9, 0..1)], vec![term(0, 0)]),
        (vec![0, 2], vec![segment(0..9, 0..1)], vec![term(0, 0)]),
        (vec![], vec![], vec![]),
    ] {
        let result =
            std::panic::catch_unwind(|| QcAccumulation::new(9, 18, offsets, segments, terms));
        assert!(result.is_err());
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
        let plan = QcAccumulation::new(
            9,
            18,
            vec![0, 1],
            vec![QcSegment {
                lanes: 0..9,
                terms: 0..1,
            }],
            vec![QcTerm {
                offset: 0,
                shift: 0,
            }],
        );
        let mut beliefs = vec![123.0; 9];
        for (beliefs_len, channel, plane) in [
            (9, &block[..8], 18),
            (8, &block[..], 18),
            (9, &block[..], 17),
        ] {
            let mut beliefs = vec![123.0; beliefs_len];
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                kernel.accumulate(&plan, &mut beliefs, channel, &vec![0.0; plane])
            }));
            assert!(
                result.is_err(),
                "{beliefs_len} beliefs, {} channel, {plane} messages",
                channel.len()
            );
            assert!(beliefs.iter().all(|&value| value == 123.0));
        }
        kernel.accumulate(&plan, &mut beliefs, &block, &[0.0; 18]);
        assert!(beliefs.iter().all(|&value| value == 1.0));
    }
}

/// The fused row update equals the rotated differences followed by the
/// check kernel on the same backend, bit for bit, over every degree up to
/// past the register-resident cap, lifting sizes with and without a tail,
/// and shifts whose groups wrap.
#[test]
fn every_backend_fuses_rotated_differences_into_the_check() {
    let special = [
        0.0,
        -0.0,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MAX,
        -f32::MAX,
        f32::from_bits(1),
        -2.0,
        2.0,
        -0.5,
        3.25,
        -7.0,
    ];
    for kernel in backends() {
        for z in [1, 7, 8, 9, 16, 17, 23, 64, 65] {
            for degree in 0..=20 {
                for rule in [
                    LaneMinSumRule::Plain,
                    LaneMinSumRule::Normalized(0.75),
                    LaneMinSumRule::Offset(0.5),
                ] {
                    let columns = degree + 2;
                    let beliefs: Vec<f32> = (0..columns * z)
                        .map(|i| special[(i * 7 + 3) % special.len()])
                        .collect();
                    let edges: Vec<QcBlock> = (0..degree)
                        .map(|edge| QcBlock {
                            offset: ((edge * 5 + 1) % columns) * z,
                            shift: (edge * 11 + degree) % z,
                        })
                        .collect();
                    let messages: Vec<f32> = (0..degree * z)
                        .map(|i| special[(i * 5 + 1) % special.len()])
                        .collect();

                    let mut differences = vec![0.0; degree * z];
                    for (edge, term) in edges.iter().enumerate() {
                        kernel.rotated_difference(
                            &beliefs[term.offset..term.offset + z],
                            term.shift,
                            &messages[edge * z..(edge + 1) * z],
                            &mut differences[edge * z..(edge + 1) * z],
                        );
                    }
                    let mut want = vec![0.0; degree * z];
                    kernel.check_block(rule, &differences, &mut want, degree, z);

                    let mut got = messages.clone();
                    let mut scratch = vec![123.0; degree * z];
                    kernel.check_row_rotated(rule, &beliefs, &edges, &mut got, &mut scratch, z);
                    for (i, (got, want)) in got.iter().zip(&want).enumerate() {
                        assert_eq!(
                            got.to_bits(),
                            want.to_bits(),
                            "{rule:?}, z {z}, degree {degree}, edge {}, lane {}",
                            i / z,
                            i % z
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn fused_rows_panic_before_writing_outside_their_domain() {
    let term = |offset, shift| QcBlock { offset, shift };
    for kernel in backends() {
        for (edges, messages, scratch, z) in [
            (vec![term(0, 8), term(8, 0)], 16, 16, 8),
            (vec![term(0, 0), term(9, 0)], 16, 16, 8),
            (vec![term(0, 0), term(8, 0)], 15, 16, 8),
            (vec![term(0, 0), term(8, 0)], 16, 15, 8),
        ] {
            let beliefs = vec![1.0; 16];
            let mut messages = vec![123.0; messages];
            let mut scratch = vec![0.0; scratch];
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                kernel.check_row_rotated(
                    LaneMinSumRule::Plain,
                    &beliefs,
                    &edges,
                    &mut messages,
                    &mut scratch,
                    z,
                );
            }));
            assert!(result.is_err(), "{edges:?}, {} messages", messages.len());
            assert!(messages.iter().all(|&value| value == 123.0));
        }
    }
}
