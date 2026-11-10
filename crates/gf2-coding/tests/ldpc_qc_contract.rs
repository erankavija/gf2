//! The same posterior, word and outcome contract runs on each QC backend.
use gf2_coding::ldpc::{
    DecoderAlgorithm, DecoderConfig, LaneBackend, LdpcCode, LdpcDecoder, QcCirculant, QcDecoder,
    QuasiCyclicLdpc,
};
use gf2_coding::llr::Llr;
use gf2_coding::test_support::{ldpc_dvb_t2_codes, ldpc_lane_backends, DVB_T2_RATES};
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use rand::{rngs::StdRng, Rng, SeedableRng};

fn small_qc(z: usize) -> QuasiCyclicLdpc {
    QuasiCyclicLdpc::new(
        vec![
            vec![0, 1 % z as i32, 0, -1],
            vec![1 % z as i32, 0, -1, 0],
            vec![-1, -1, -1, -1],
            vec![-1, -1, 0, -1],
        ],
        z,
    )
}

#[test]
fn retains_the_qc_relation_across_clones_and_reports_absence() {
    let qc = small_qc(9);
    let code = LdpcCode::from_quasi_cyclic(&qc).clone();
    let retained = code.quasi_cyclic().unwrap();
    let expanded = LdpcCode::from_quasi_cyclic(retained);
    for row in 0..code.m() {
        assert_eq!(
            code.parity_check_matrix().row_iter(row).collect::<Vec<_>>(),
            expanded
                .parity_check_matrix()
                .row_iter(row)
                .collect::<Vec<_>>()
        );
    }
    let unstructured = LdpcCode::from_edges(qc.expanded_rows(), qc.expanded_cols(), &qc.to_edges());
    assert!(unstructured.quasi_cyclic().is_none());
    assert!(QcDecoder::new(&unstructured, DecoderConfig::default()).is_none());
    let detached = code.clone().without_quasi_cyclic();
    assert!(detached.quasi_cyclic().is_none());
    for row in 0..code.m() {
        assert!(code
            .parity_check_matrix()
            .row_iter(row)
            .eq(detached.parity_check_matrix().row_iter(row)));
    }
    assert!(QcDecoder::new(
        &code,
        DecoderConfig::new(DecoderAlgorithm::SumProduct, true)
    )
    .is_none());
}

#[test]
fn attaching_structure_requires_the_exact_parity_check_matrix() {
    let qc = small_qc(9);
    let mut edges = qc.to_edges();
    edges.reverse();

    let mut matching = LdpcCode::from_edges(qc.expanded_rows(), qc.expanded_cols(), &edges);
    assert!(matching.quasi_cyclic().is_none());
    assert!(matching.attach_quasi_cyclic(&qc));
    assert_eq!(matching.clone().quasi_cyclic(), Some(&qc));

    let mut moved = LdpcCode::from_edges(
        qc.expanded_rows(),
        qc.expanded_cols(),
        &small_qc_with_shift(9, 2).to_edges(),
    );
    assert!(!moved.attach_quasi_cyclic(&qc));
    assert!(moved.quasi_cyclic().is_none());

    let mut other_size = LdpcCode::from_edges(qc.expanded_rows(), qc.expanded_cols() + 1, &edges);
    assert!(!other_size.attach_quasi_cyclic(&qc));
    assert!(other_size.quasi_cyclic().is_none());
}

fn small_qc_with_shift(z: usize, shift: i32) -> QuasiCyclicLdpc {
    QuasiCyclicLdpc::new(
        vec![
            vec![0, shift, 0, -1],
            vec![1, 0, -1, 0],
            vec![-1, -1, -1, -1],
            vec![-1, -1, 0, -1],
        ],
        z,
    )
}

fn assert_contract(code: &LdpcCode) {
    let mut rng = StdRng::seed_from_u64(91);
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
    let mut frames: Vec<Vec<Llr>> = (0..3)
        .map(|_| {
            (0..code.n())
                .map(|_| Llr::new(rng.gen_range(-8.0..8.0)))
                .collect()
        })
        .collect();
    frames.push(
        (0..code.n())
            .map(|i| Llr::new(special[i % special.len()]))
            .collect(),
    );
    frames.push(
        (0..code.n())
            .map(|i| {
                Llr::new(if i < code.n() / 2 {
                    0.0
                } else {
                    gf2_coding::ldpc::nr_5g::FILLER_LLR
                })
            })
            .collect(),
    );
    frames.push(vec![Llr::new(2.0); code.n()]);
    // The zero word under noise converges after a few iterations.
    frames.push(
        (0..code.n())
            .map(|_| Llr::new(rng.gen_range(-1.5..4.0)))
            .collect(),
    );
    frames.push(
        (0..code.n())
            .map(|i| Llr::new(if i == 0 { -3.0 } else { 2.0 }))
            .collect(),
    );
    for algorithm in [
        DecoderAlgorithm::MinSum,
        DecoderAlgorithm::NormalizedMinSum(0.75),
        DecoderAlgorithm::OffsetMinSum(0.5),
    ] {
        for stopping in [false, true] {
            let config = DecoderConfig::new(algorithm, stopping);
            for backend in ldpc_lane_backends() {
                let mut canonical = LdpcDecoder::with_config(code.clone(), config);
                let mut candidate = QcDecoder::new(code, config).unwrap();
                if backend == LaneBackend::Portable {
                    candidate = candidate.with_portable_kernel();
                }
                assert_eq!(candidate.backend(), backend);
                let mut want_word = BitVec::new();
                let mut got_word = BitVec::ones(code.n() + 1);
                for cap in [0, 1, 5, 0, 2] {
                    for frame in &frames {
                        let want = canonical.decode_codeword_into(frame, cap, &mut want_word);
                        let got = candidate.decode_codeword_into(frame, cap, &mut got_word);
                        assert_eq!(
                            got, want,
                            "{algorithm:?}, stopping {stopping}, {backend:?}, cap {cap}"
                        );
                        assert_eq!(got_word, want_word);
                        assert_eq!(candidate.last_iterations(), got.iterations);
                        for (i, (got, want)) in candidate
                            .posterior_llrs()
                            .iter()
                            .zip(canonical.posterior_llrs())
                            .enumerate()
                        {
                            assert_eq!(got.value().to_bits(), want.value().to_bits(), "position {i}, {algorithm:?}, stopping {stopping}, {backend:?}, cap {cap}");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn scalar_and_simd_match_canonical_including_zero_cap_after_reuse() {
    for z in [1, 3, 7, 8, 9, 16, 17, 63, 64, 65] {
        assert_contract(&LdpcCode::from_quasi_cyclic(&small_qc(z)));
    }
}

#[test]
fn empty_graphs_and_isolated_variables_follow_canonical() {
    for n in [0, 1, 63, 64, 65] {
        assert_contract(&LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::new(
            vec![vec![-1; n]],
            1,
        )));
    }
}

fn assert_nr_contract(lifting_sizes: &[usize]) {
    for graph in [1, 2] {
        for &z in lifting_sizes {
            let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(graph, z));
            assert_eq!(code.quasi_cyclic().unwrap().expansion_factor(), z);
            assert_contract(&code);
        }
    }
}

/// Lifting sizes the AVX2 kernel decodes in its scalar tail alone.
#[test]
fn nr_base_graphs_below_one_vector_follow_canonical() {
    assert_nr_contract(&[2, 3, 5, 7]);
}

/// Lifting sizes the AVX2 kernel splits into vector groups and a scalar tail.
#[test]
fn nr_base_graphs_with_a_remainder_modulo_eight_follow_canonical() {
    assert_nr_contract(&[9, 13, 15, 52]);
}

#[test]
fn nr_base_graphs_at_a_multiple_of_eight_follow_canonical() {
    assert_nr_contract(&[8, 64]);
}

#[test]
fn nr_base_graphs_at_the_largest_lifting_size_follow_canonical() {
    assert_nr_contract(&[384]);
}

// Exposes sequential QC calls to the common frame-order and reuse contract.
struct QcFrames {
    decoder: QcDecoder,
    n: usize,
    cap: usize,
}

impl gf2_coding::traits::BatchSoftDecoder for QcFrames {
    fn frame_len(&self) -> usize {
        self.n
    }
    fn decoded_len(&self) -> usize {
        self.n
    }
    fn batch_granularity(&self) -> usize {
        1
    }
    fn decode_batch(
        &mut self,
        frames: &[&[Llr]],
        decoded: &mut [BitVec],
        outcomes: &mut [gf2_coding::traits::DecodeOutcome],
    ) {
        assert_eq!(frames.len(), decoded.len());
        assert_eq!(frames.len(), outcomes.len());
        for ((frame, word), outcome) in frames.iter().zip(decoded).zip(outcomes) {
            *outcome = self.decoder.decode_codeword_into(frame, self.cap, word);
        }
    }
}

#[test]
fn qc_calls_satisfy_the_shared_frame_contract() {
    use gf2_coding::traits::conformance::{batch_soft_decoder_contract, ConvergenceMix};
    // Distinct valid words remain distinct even with early termination.
    let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::new(vec![vec![0, 0]], 9));
    let frames: Vec<Vec<Llr>> = (0..5)
        .map(|pattern| {
            (0..code.n())
                .map(|i| Llr::new(if i % 9 == pattern { -4.0 } else { 4.0 }))
                .collect()
        })
        .collect();
    for algorithm in [
        DecoderAlgorithm::MinSum,
        DecoderAlgorithm::NormalizedMinSum(0.75),
        DecoderAlgorithm::OffsetMinSum(0.5),
    ] {
        for stopping in [true, false] {
            for backend in ldpc_lane_backends() {
                for cap in [0, 4] {
                    let config = DecoderConfig::new(algorithm, stopping);
                    let mut single = LdpcDecoder::with_config(code.clone(), config);
                    let mut decoder = QcDecoder::new(&code, config).unwrap();
                    if backend == LaneBackend::Portable {
                        decoder = decoder.with_portable_kernel();
                    }
                    let mut batch = QcFrames {
                        decoder,
                        n: code.n(),
                        cap,
                    };
                    batch_soft_decoder_contract(
                        &mut batch,
                        |frame| single.decode_to_codeword(frame, cap),
                        &frames,
                        ConvergenceMix::Any,
                    );
                }
            }
        }
    }
}

#[test]
fn zero_cap_copies_the_current_frame_after_a_nonzero_decode() {
    let code = LdpcCode::from_quasi_cyclic(&small_qc(9));
    for backend in ldpc_lane_backends() {
        for stopping in [true, false] {
            let mut decoder = QcDecoder::new(
                &code,
                DecoderConfig::new(DecoderAlgorithm::MinSum, stopping),
            )
            .unwrap();
            if backend == LaneBackend::Portable {
                decoder = decoder.with_portable_kernel();
            }
            let mut word = BitVec::new();
            decoder.decode_codeword_into(&vec![Llr::new(2.0); code.n()], 3, &mut word);
            let frame: Vec<_> = (0..code.n())
                .map(|i| Llr::new(if i % 3 == 0 { -3.0 } else { 1.0 }))
                .collect();
            let got = decoder.decode_codeword_into(&frame, 0, &mut word);
            assert_eq!(
                decoder.posterior_llrs(),
                frame,
                "zero cap must replace stale posteriors"
            );
            assert_eq!(got.iterations, 0);
            assert_eq!(got.syndrome_check_passed, code.is_valid_codeword(&word));
            for (i, llr) in frame.iter().enumerate() {
                assert_eq!(word.get(i), llr.hard_decision());
            }
        }
    }
}

#[test]
fn dvb_t2_block_descriptions_lift_to_the_parity_check_matrix() {
    let mut weight_above_one = false;
    for (frame, rate, code) in ldpc_dvb_t2_codes() {
        let qc = code.quasi_cyclic().expect("DVB-T2 codes retain structure");
        assert_eq!(qc.expansion_factor(), 360, "{frame:?} {rate:?}");
        assert!(qc.row_order().is_some() && qc.col_order().is_some());
        let h = code.parity_check_matrix();
        let edges: Vec<_> = (0..code.m())
            .flat_map(|row| h.row_iter(row).map(move |col| (row, col)))
            .collect();
        let mut plain = LdpcCode::from_edges(code.m(), code.n(), &edges);
        assert!(plain.attach_quasi_cyclic(qc), "{frame:?} {rate:?}");
        assert_eq!(
            qc.circulants()
                .iter()
                .filter(|c| !c.masked_rows().is_empty())
                .count(),
            1,
            "{frame:?} {rate:?}: the staircase wrap is the one masked circulant"
        );
        weight_above_one |= qc.circulants().windows(2).any(|pair| {
            (pair[0].row_block(), pair[0].col_block()) == (pair[1].row_block(), pair[1].col_block())
        });
        assert!(qc.base_matrix().is_none());
    }
    assert!(
        weight_above_one,
        "the tables hold blocks of weight above one"
    );
}

/// Masked rows, a block of weight two and non-identity orders at word-boundary
/// lifting sizes.
fn general_qc(z: usize) -> QuasiCyclicLdpc {
    let circulants = vec![
        QcCirculant::new(0, 0, 0),
        QcCirculant::new(0, 0, 1 % z),
        QcCirculant::new(0, 1, 2 % z),
        QcCirculant::new(0, 2, 0).with_masked_rows(vec![0, z - 1]),
        QcCirculant::new(1, 1, 0),
        QcCirculant::new(1, 2, (z - 1) % z),
        QcCirculant::new(1, 0, 3 % z),
    ];
    let rows = 2 * z;
    let cols = 3 * z;
    let row_order = (0..rows).map(|r| 2 * (r % z) + r / z).collect();
    let col_order = (0..cols).rev().collect();
    QuasiCyclicLdpc::from_circulants(2, 3, z, circulants).with_orders(row_order, col_order)
}

#[test]
fn masked_weighted_and_permuted_blocks_follow_canonical() {
    for z in [2, 3, 63, 64, 65] {
        let qc = general_qc(z);
        let code = LdpcCode::from_quasi_cyclic(&qc);
        assert!(code.clone().attach_quasi_cyclic(&qc));
        assert_contract(&code);
    }
}

#[test]
fn a_masked_check_left_with_one_edge_has_no_qc_decoder() {
    let qc = QuasiCyclicLdpc::from_circulants(
        1,
        2,
        4,
        vec![
            QcCirculant::new(0, 0, 0),
            QcCirculant::new(0, 1, 1).with_masked_rows(vec![2]),
        ],
    );
    let code = LdpcCode::from_quasi_cyclic(&qc);
    assert!(QcDecoder::new(&code, DecoderConfig::default()).is_none());
}

#[test]
fn dvb_t2_short_codes_follow_canonical() {
    for rate in DVB_T2_RATES {
        assert_contract(&LdpcCode::dvb_t2_short(rate));
    }
}

#[test]
fn dvb_t2_normal_1_2_follows_canonical() {
    assert_contract(&LdpcCode::dvb_t2_normal(CodeRate::Rate1_2));
}

#[test]
fn dvb_t2_normal_3_5_follows_canonical() {
    assert_contract(&LdpcCode::dvb_t2_normal(CodeRate::Rate3_5));
}

#[test]
fn dvb_t2_normal_2_3_follows_canonical() {
    assert_contract(&LdpcCode::dvb_t2_normal(CodeRate::Rate2_3));
}

#[test]
fn dvb_t2_normal_3_4_follows_canonical() {
    assert_contract(&LdpcCode::dvb_t2_normal(CodeRate::Rate3_4));
}

#[test]
fn dvb_t2_normal_4_5_follows_canonical() {
    assert_contract(&LdpcCode::dvb_t2_normal(CodeRate::Rate4_5));
}

#[test]
fn dvb_t2_normal_5_6_follows_canonical() {
    assert_contract(&LdpcCode::dvb_t2_normal(CodeRate::Rate5_6));
}
