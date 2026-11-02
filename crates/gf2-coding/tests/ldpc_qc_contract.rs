//! The same posterior, word and outcome contract runs on each QC backend.
use gf2_coding::ldpc::{
    DecoderAlgorithm, DecoderConfig, LdpcCode, LdpcDecoder, QcDecoder, QuasiCyclicLdpc,
};
use gf2_coding::llr::Llr;
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
    assert!(QcDecoder::new(
        &code,
        DecoderConfig::new(DecoderAlgorithm::SumProduct, true)
    )
    .is_none());
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
            for portable in [true, false] {
                let mut canonical = LdpcDecoder::with_config(code.clone(), config);
                let mut candidate = QcDecoder::new(code, config).unwrap();
                if portable {
                    candidate = candidate.with_portable_kernel();
                }
                let mut want_word = BitVec::new();
                let mut got_word = BitVec::ones(code.n() + 1);
                for cap in [0, 1, 5, 0, 2] {
                    for frame in &frames {
                        let want = canonical.decode_codeword_into(frame, cap, &mut want_word);
                        let got = candidate.decode_codeword_into(frame, cap, &mut got_word);
                        assert_eq!(
                            got, want,
                            "{algorithm:?}, stopping {stopping}, portable {portable}, cap {cap}"
                        );
                        assert_eq!(got_word, want_word);
                        assert_eq!(candidate.last_iterations(), got.iterations);
                        for (i, (got, want)) in candidate
                            .posterior_llrs()
                            .iter()
                            .zip(canonical.posterior_llrs())
                            .enumerate()
                        {
                            assert_eq!(got.value().to_bits(), want.value().to_bits(), "position {i}, {algorithm:?}, stopping {stopping}, portable {portable}, cap {cap}");
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

#[test]
fn nr_base_graphs_follow_canonical() {
    for graph in [1, 2] {
        assert_contract(&LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(
            graph, 8,
        )));
    }
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
            for portable in [true, false] {
                for cap in [0, 4] {
                    let config = DecoderConfig::new(algorithm, stopping);
                    let mut single = LdpcDecoder::with_config(code.clone(), config);
                    let mut decoder = QcDecoder::new(&code, config).unwrap();
                    if portable {
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
    for portable in [true, false] {
        for stopping in [true, false] {
            let mut decoder = QcDecoder::new(
                &code,
                DecoderConfig::new(DecoderAlgorithm::MinSum, stopping),
            )
            .unwrap();
            if portable {
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
