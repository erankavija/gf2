//! The layered quantized decoder's contract, run on each kernel backend.
use gf2_coding::channel::AwgnChannel;
use gf2_coding::ldpc::nr_5g::FILLER_LLR;
use gf2_coding::ldpc::{
    LaneBackend, LayerError, LayeredConfig, LayeredDecoder, LayeredRule, LdpcCode, LdpcEncoder,
    QuasiCyclicLdpc,
};
use gf2_coding::llr::Llr;
use gf2_coding::traits::BlockEncoder;
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use rand::{rngs::StdRng, Rng, SeedableRng};

/// Normalized min-sum at 3/4 with two fractional bits, channel values
/// within ±31 and messages within ±40.
fn standard() -> LayeredConfig {
    LayeredConfig::new(LayeredRule::Normalized { eighths: 6 }, 4.0, 31, 40, true)
}

fn configs() -> Vec<LayeredConfig> {
    vec![
        standard(),
        LayeredConfig::new(LayeredRule::Normalized { eighths: 8 }, 2.0, 127, 126, false),
        LayeredConfig::new(LayeredRule::Offset { beta: 1 }, 8.0, 63, 15, true),
    ]
}

/// A code with its layer partition and a noise deviation near its waterfall.
struct Case {
    name: String,
    code: LdpcCode,
    layers: Option<Vec<Vec<usize>>>,
    sigma: f64,
}

impl Case {
    fn decoder(&self, config: LayeredConfig) -> LayeredDecoder {
        match &self.layers {
            Some(layers) => LayeredDecoder::with_layers(&self.code, layers, config).unwrap(),
            None => LayeredDecoder::new(&self.code, config).unwrap(),
        }
    }
}

fn nr(base_graph: u8, z: usize, sigma: f64) -> Case {
    Case {
        name: format!("nr bg{base_graph} z{z}"),
        code: LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(base_graph, z)),
        layers: None,
        sigma,
    }
}

/// Base rows of degree zero, one and two beside ordinary rows.
fn small_qc() -> Case {
    let base = vec![
        vec![0, 3, 7, 1, 0, -1],
        vec![-1, -1, -1, -1, -1, -1],
        vec![-1, -1, 4, -1, -1, -1],
        vec![5, -1, -1, -1, 2, -1],
        vec![9, 13, 0, 6, -1, 4],
    ];
    Case {
        name: "small qc".into(),
        code: LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::new(base, 16)),
        layers: None,
        sigma: 0.7,
    }
}

/// DVB-T2 short codes through their retained QC structure, whose orders
/// are not the identity and whose row blocks hold several circulants of one
/// column block.
fn dvb_t2_short(rate: CodeRate, sigma: f64) -> Case {
    let code = LdpcCode::dvb_t2_short(rate);
    let qc = code
        .quasi_cyclic()
        .expect("DVB-T2 codes retain QC structure");
    assert!(qc.col_order().is_some());
    let stacked = qc.circulants().windows(2).any(|pair| {
        (pair[0].row_block(), pair[0].col_block()) == (pair[1].row_block(), pair[1].col_block())
    });
    assert!(stacked, "{rate:?}");
    Case {
        name: format!("dvb-t2 short {rate:?}"),
        code,
        layers: None,
        sigma,
    }
}

/// The row blocks of [`small_qc`] as an explicit partition.
fn small_qc_explicit() -> Case {
    let mut case = small_qc();
    case.name = "small qc, explicit layers".into();
    case.layers = Some((0..5).map(|b| (b * 16..(b + 1) * 16).collect()).collect());
    case
}

fn cases() -> Vec<Case> {
    vec![
        small_qc(),
        nr(1, 2, 0.6),
        nr(1, 15, 0.7),
        nr(1, 40, 0.75),
        nr(2, 3, 0.6),
        nr(2, 52, 0.75),
        nr(1, 384, 0.8),
        dvb_t2_short(CodeRate::Rate1_2, 0.85),
        dvb_t2_short(CodeRate::Rate5_6, 0.5),
        small_qc_explicit(),
    ]
}

/// Noisy all-zero words at deviations from clean to undecodable, followed by
/// frames of zeros, saturating, non-finite and alternating values.
fn frames(n: usize, sigma: f64, seed: u64) -> Vec<Vec<Llr>> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut frames: Vec<Vec<Llr>> = [0.5, 1.0, 1.25, 2.0]
        .iter()
        .map(|&scale| {
            let channel = AwgnChannel::from_variance((sigma * scale).powi(2));
            let gain = 2.0 / channel.variance();
            (0..n)
                .map(|_| Llr::new((gain * channel.transmit(1.0, &mut rng)) as f32))
                .collect()
        })
        .collect();
    frames.push(vec![Llr::zero(); n]);
    frames.push(
        (0..n)
            .map(|i| Llr::new(if i % 3 == 0 { -1e30 } else { 1e30 }))
            .collect(),
    );
    let specials = [
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        -0.0,
        0.0,
        7.875,
        -7.875,
        0.125,
    ];
    frames.push(
        (0..n)
            .map(|_| Llr::new(specials[rng.gen_range(0..specials.len())]))
            .collect(),
    );
    frames.push(
        (0..n)
            .map(|i| Llr::new(if i % 2 == 0 { 0.3 } else { -0.3 }))
            .collect(),
    );
    frames
}

/// Decoders on the backends of this build and host: the portable kernel, and
/// the AVX2 kernel a `simd` build must select on an AVX2 host.
fn backends(case: &Case, config: LayeredConfig) -> Vec<LayeredDecoder> {
    let default = case.decoder(config);
    let portable = case.decoder(config).with_portable_kernel();
    assert_eq!(portable.backend(), LaneBackend::Portable);
    if cfg!(feature = "simd") && std::arch::is_x86_feature_detected!("avx2") {
        assert_eq!(default.backend(), LaneBackend::Avx2);
        vec![portable, default]
    } else {
        assert_eq!(default.backend(), LaneBackend::Portable);
        vec![portable]
    }
}

type Observation = (Vec<i8>, BitVec, usize, bool, bool, usize);

fn observe(decoder: &mut LayeredDecoder, llrs: &[Llr], cap: usize) -> Observation {
    let mut word = BitVec::new();
    let outcome = decoder.decode_codeword_into(llrs, cap, &mut word);
    (
        decoder.posterior().to_vec(),
        word,
        outcome.iterations,
        outcome.converged,
        outcome.syndrome_check_passed,
        decoder.last_iterations(),
    )
}

#[test]
fn backends_agree_on_posteriors_words_and_outcomes() {
    for (index, case) in cases().iter().enumerate() {
        let frames = frames(case.code.n(), case.sigma, 0x51ed + index as u64);
        for config in configs() {
            let mut decoders = backends(case, config);
            // One decoder per backend runs every frame and cap in sequence,
            // so the comparison also covers reuse.
            for cap in [0, 1, 3, 25] {
                for llrs in &frames {
                    let seen: Vec<Observation> = decoders
                        .iter_mut()
                        .map(|decoder| observe(decoder, llrs, cap))
                        .collect();
                    for other in &seen[1..] {
                        assert_eq!(&seen[0], other, "{} {config:?} cap {cap}", case.name);
                    }
                }
            }
        }
    }
}

#[test]
fn a_reused_decoder_equals_a_fresh_one() {
    for case in [nr(1, 15, 0.7), dvb_t2_short(CodeRate::Rate5_6, 0.5)] {
        let frames = frames(case.code.n(), case.sigma, 0xfee1);
        for mut reused in backends(&case, standard()) {
            for llrs in frames.iter().chain(frames.iter().rev()) {
                let backend = reused.backend();
                let mut fresh = case.decoder(standard());
                if backend == LaneBackend::Portable {
                    fresh = fresh.with_portable_kernel();
                }
                assert_eq!(
                    observe(&mut reused, llrs, 25),
                    observe(&mut fresh, llrs, 25)
                );
            }
        }
    }
}

#[test]
fn cap_zero_reports_the_quantized_channel_and_its_syndrome() {
    for case in [
        small_qc(),
        nr(2, 3, 0.6),
        dvb_t2_short(CodeRate::Rate1_2, 0.85),
    ] {
        let code = &case.code;
        let zero_word = vec![Llr::new(2.0); code.n()];
        for llrs in frames(code.n(), case.sigma, 7).iter().chain([&zero_word]) {
            for mut decoder in backends(&case, standard()) {
                let (posterior, word, iterations, converged, passed, last) =
                    observe(&mut decoder, llrs, 0);
                let config = decoder.config();
                let expected: Vec<i8> = llrs.iter().map(|&l| config.quantize(l)).collect();
                assert_eq!(posterior, expected);
                for (i, &value) in expected.iter().enumerate() {
                    assert_eq!(word.get(i), value < 0);
                }
                assert_eq!((iterations, last), (0, 0));
                assert_eq!(passed, code.is_valid_codeword(&word));
                assert_eq!(converged, passed);
            }
        }
    }
}

#[test]
fn quantization_rounds_saturates_and_maps_rate_matching_values() {
    let config = standard();
    let q = |x: f32| config.quantize(Llr::new(x));
    assert_eq!((q(0.0), q(-0.0), q(f32::NAN)), (0, 0, 0));
    assert_eq!((q(0.125), q(-0.125), q(0.124), q(-0.375)), (1, -1, 0, -2));
    assert_eq!((q(f32::INFINITY), q(f32::NEG_INFINITY)), (31, -31));
    assert_eq!((q(7.75), q(7.876), q(-1e30)), (31, 31, -31));
    assert_eq!(q(FILLER_LLR), config.channel_limit() as i8);
}

#[test]
fn termination_follows_the_syndrome_and_the_cap() {
    for case in [nr(1, 15, 0.7), dvb_t2_short(CodeRate::Rate5_6, 0.5)] {
        termination_case(&case);
    }
}

fn termination_case(case: &Case) {
    let encoder = LdpcEncoder::new(case.code.clone());
    let mut rng = StdRng::seed_from_u64(0xa11);
    let codeword = encoder.encode(&BitVec::random(encoder.k(), &mut rng));
    let clean: Vec<Llr> = (0..codeword.len())
        .map(|i| Llr::new(if codeword.get(i) { -4.0 } else { 4.0 }))
        .collect();
    let undecodable = &frames(case.code.n(), case.sigma, 3)[3];
    for early in [true, false] {
        let config = LayeredConfig::new(LayeredRule::Normalized { eighths: 6 }, 4.0, 31, 31, early);
        for mut decoder in backends(case, config) {
            let (_, word, iterations, converged, passed, _) = observe(&mut decoder, &clean, 10);
            assert_eq!(word, codeword, "{}", case.name);
            assert!(converged && passed);
            assert_eq!(iterations, if early { 1 } else { 10 });

            let (_, word, iterations, converged, passed, _) =
                observe(&mut decoder, undecodable, 10);
            assert_eq!(iterations, 10);
            assert_eq!(passed, case.code.is_valid_codeword(&word));
            assert!(!converged && !passed);
        }
    }
}

/// Punctured and truncated positions arrive as zeros and fillers at
/// `FILLER_LLR`; frames below the waterfall decode to their messages.
#[test]
fn rate_matched_nr_frames_decode_to_their_messages() {
    for (base_graph, n, k, sigma) in [(1, 600, 200, 0.6), (2, 256, 49, 0.6)] {
        let code = QuasiCyclicLdpc::nr_5g_rate_matched(base_graph, n, k);
        let params = code.params();
        assert!(params.num_punctured_systematic > 0 && params.num_punctured_parity > 0);
        assert!(params.num_shortened > 0);
        let case = Case {
            name: format!("rate-matched bg{base_graph}"),
            code: code.mother_code().clone(),
            layers: None,
            sigma,
        };
        let channel = AwgnChannel::from_variance(sigma * sigma);
        let mut rng = StdRng::seed_from_u64(0x7e57);
        for _ in 0..8 {
            let message = BitVec::random(code.k(), &mut rng);
            let sent = code.encode(&message);
            let received: Vec<Llr> = (0..sent.len())
                .map(|i| {
                    let symbol = if sent.get(i) { -1.0 } else { 1.0 };
                    Llr::new((2.0 / channel.variance() * channel.transmit(symbol, &mut rng)) as f32)
                })
                .collect();
            let prepared = code.prepare_llrs(&received);
            let observed: Vec<Observation> = backends(&case, standard())
                .iter_mut()
                .map(|decoder| observe(decoder, &prepared, 25))
                .collect();
            for other in &observed[1..] {
                assert_eq!(&observed[0], other, "{}", case.name);
            }
            let (_, word, _, converged, _, _) = &observed[0];
            assert!(converged, "{}", case.name);
            assert_eq!(code.extract_message(word), message, "{}", case.name);
        }
    }
}

#[test]
fn layer_partitions_are_validated() {
    let code = small_qc().code;
    let config = standard();
    let err = |layers: Vec<Vec<usize>>| LayeredDecoder::with_layers(&code, &layers, config).err();
    let m = code.m();
    assert!(err(vec![(0..m).collect::<Vec<_>>()])
        .is_some_and(|e| matches!(e, LayerError::SharedVariable { layer: 0, .. })));
    assert_eq!(
        err((0..m).map(|c| vec![c]).chain([vec![m]]).collect()),
        Some(LayerError::CheckOutOfRange { check: m })
    );
    assert_eq!(
        err((0..m).map(|c| vec![c]).chain([vec![0]]).collect()),
        Some(LayerError::DuplicateCheck { check: 0 })
    );
    assert_eq!(
        err((1..m).map(|c| vec![c]).collect()),
        Some(LayerError::MissingCheck { check: 0 })
    );
    assert!(err((0..m).map(|c| vec![c]).collect()).is_none());
    assert!(LayeredDecoder::new(
        &LdpcCode::from_edges(2, 4, &[(0, 0), (0, 1), (1, 2), (1, 3)]),
        config
    )
    .is_none());
}

/// Single-check layers in reverse order are a valid, if slow, schedule; a
/// clean frame converges on it as on the row-block schedule.
#[test]
fn any_valid_partition_decodes() {
    let case = small_qc();
    let layers: Vec<Vec<usize>> = (0..case.code.m()).rev().map(|c| vec![c]).collect();
    let clean = vec![Llr::new(3.0); case.code.n()];
    let mut decoder = LayeredDecoder::with_layers(&case.code, &layers, standard()).unwrap();
    let mut word = BitVec::new();
    let outcome = decoder.decode_codeword_into(&clean, 5, &mut word);
    assert!(outcome.converged && word.count_ones() == 0);
    assert_eq!(outcome.iterations, 1);
}
