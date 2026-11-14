//! The layered quantized decoder's contract, run on each kernel backend.
use gf2_coding::channel::AwgnChannel;
use gf2_coding::ldpc::nr_5g::{all_lifting_sizes, FILLER_LLR};
use gf2_coding::ldpc::{
    LaneBackend, LayerError, LayeredConfig, LayeredDecoder, LayeredRule, LdpcCode, LdpcEncoder,
    QuasiCyclicLdpc,
};
use gf2_coding::llr::Llr;
use gf2_coding::traits::BlockEncoder;
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use rand::{rngs::StdRng, Rng, SeedableRng};

fn standard() -> LayeredConfig {
    LayeredConfig::nr_5g()
}

fn configs() -> Vec<LayeredConfig> {
    vec![
        standard(),
        LayeredConfig::dvb_t2(),
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

fn host_has_avx2() -> bool {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        std::arch::is_x86_feature_detected!("avx2")
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        false
    }
}

/// Decoders on the backends of this build and host: the portable kernel, and
/// the AVX2 kernel a `simd` build must select on an AVX2 host.
fn backends(case: &Case, config: LayeredConfig) -> Vec<LayeredDecoder> {
    let default = case.decoder(config);
    let portable = case.decoder(config).with_portable_kernel();
    assert_eq!(portable.backend(), LaneBackend::Portable);
    if cfg!(feature = "simd") && host_has_avx2() {
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
                let scale = config.frame_scale(llrs);
                let expected: Vec<i8> = llrs.iter().map(|&l| config.quantize(l, scale)).collect();
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
    // Scale 4, channel limit 31.
    let config = LayeredConfig::dvb_t2();
    let q = |x: f32| config.quantize(Llr::new(x), config.channel_scale());
    assert_eq!((q(0.0), q(-0.0), q(f32::NAN)), (0, 0, 0));
    assert_eq!((q(0.125), q(-0.125), q(0.124), q(-0.375)), (1, -1, 0, -2));
    assert_eq!((q(f32::INFINITY), q(f32::NEG_INFINITY)), (31, -31));
    assert_eq!((q(7.75), q(7.876), q(-1e30)), (31, 31, -31));
    assert_eq!(q(FILLER_LLR), config.channel_limit() as i8);
    // Every half-integer level and its float neighbours, against the definition.
    for config in configs() {
        let limit = f32::from(config.channel_limit());
        for half in -400..=400 {
            let level = 0.5 * half as f32;
            for scaled in [level.next_down(), level, level.next_up()] {
                let scale = config.channel_scale();
                let llr = scaled / scale;
                let expected = (llr * scale).round().clamp(-limit, limit);
                assert_eq!(
                    config.quantize(Llr::new(llr), scale),
                    expected as i8,
                    "{llr}"
                );
            }
        }
    }
}

/// The mean magnitude of the finite nonzero values, summed in the order the
/// contract fixes: 16 accumulators by index modulo 16, then in order.
fn reference_mean(llrs: &[Llr]) -> f32 {
    let mut sums = [0.0f32; 16];
    let mut count = 0usize;
    for (i, llr) in llrs.iter().enumerate() {
        let magnitude = llr.value().abs();
        if magnitude.is_finite() && magnitude > 0.0 {
            sums[i % 16] += magnitude;
            count += 1;
        }
    }
    sums.iter().fold(0.0, |total, &sum| total + sum) / count.max(1) as f32
}

#[test]
fn the_frame_scale_follows_the_llr_scale_and_the_cap_guard() {
    let mut rng = StdRng::seed_from_u64(0x5ca1e);
    let specials = [0.0, -0.0, f32::NAN, f32::INFINITY, FILLER_LLR];
    for config in configs() {
        let fixed = config.with_fixed_scale();
        let halved = config.with_llr_scale(0.5);
        assert_eq!(halved.channel_scale(), config.channel_scale());
        for magnitude in [0.0f32, 0.01, 1.0, 3.0, 8.0, 50.0, 1e6] {
            for len in [0, 1, 15, 16, 17, 1000] {
                let llrs: Vec<Llr> = (0..len)
                    .map(|i| {
                        let value = if i % 7 == 3 {
                            specials[rng.gen_range(0..specials.len())]
                        } else {
                            magnitude * rng.gen_range(0.5..1.5)
                        };
                        Llr::new(if rng.gen::<bool>() { -value } else { value })
                    })
                    .collect();
                let mean = reference_mean(&llrs);
                let limit = f32::from(config.channel_limit());
                let guarded = |base: f32, multiple: f32| {
                    if mean > 0.0 {
                        base.min(limit / (multiple * mean))
                            .max(base.min(limit / FILLER_LLR))
                    } else {
                        base
                    }
                };
                let natural = config.channel_scale();
                let context = format!("{config:?} {magnitude} {len}");
                assert_eq!(
                    config.frame_scale(&llrs),
                    guarded(natural, 2.0),
                    "{context}"
                );
                assert_eq!(fixed.frame_scale(&llrs), natural, "{context}");
                assert_eq!(
                    halved.frame_scale(&llrs),
                    guarded(2.0 * natural, 2.0),
                    "{context}"
                );
                for multiple in [0.5, 1.0, 3.5] {
                    assert_eq!(
                        config.with_guard_multiple(multiple).frame_scale(&llrs),
                        guarded(natural, multiple),
                        "{context} multiple {multiple}"
                    );
                }
            }
        }
    }
}

/// The default guard multiple is 2: on BPSK AWGN frames of DVB-T2 rate 1/2
/// and NR base graph 1, below and above the guard's onset, the frame scales
/// of `dvb_t2` and `nr_5g` equal the multiple-2 formula bit for bit.
#[test]
fn the_default_guard_multiple_reproduces_the_multiple_two_scale() {
    let cases = [
        (
            LayeredConfig::dvb_t2(),
            LdpcCode::dvb_t2_short(CodeRate::Rate1_2),
        ),
        (
            LayeredConfig::nr_5g(),
            LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(1, 384)),
        ),
    ];
    for (config, code) in cases {
        assert_eq!(config.guard_multiple(), 2.0);
        assert_eq!(config.with_guard_multiple(2.0), config);
        let limit = f32::from(config.channel_limit());
        let base = config.channel_scale();
        let mut rng = StdRng::seed_from_u64(0x2a5e);
        let (mut engaged, mut idle) = (0, 0);
        // The all-zero codeword's LLRs `2y/σ²`, `y = 1 + noise`, have mean
        // magnitude near `2/σ²`.
        for sigma in [2.0f64, 1.2, 0.9, 0.7, 0.6, 0.5, 0.4, 0.3] {
            let channel = AwgnChannel::from_variance(sigma * sigma);
            let gain = 2.0 / channel.variance();
            let llrs: Vec<Llr> = (0..code.n())
                .map(|_| Llr::new((gain * channel.transmit(1.0, &mut rng)) as f32))
                .collect();
            let expected = base
                .min(limit / (2.0 * reference_mean(&llrs)))
                .max(base.min(limit / FILLER_LLR));
            let scale = config.frame_scale(&llrs);
            assert_eq!(scale.to_bits(), expected.to_bits(), "{config:?} σ {sigma}");
            if scale < base {
                engaged += 1;
            } else {
                idle += 1;
            }
        }
        assert!(engaged > 0 && idle > 0, "{config:?} {engaged} {idle}");
    }
}

/// A random codeword and its BPSK AWGN natural-log LLRs at `ebn0` dB, drawn
/// from `seed`.
fn awgn_frame(encoder: &LdpcEncoder, n: usize, ebn0: f64, seed: u64) -> (BitVec, Vec<Llr>) {
    let mut rng = StdRng::seed_from_u64(seed);
    let codeword = encoder.encode(&BitVec::random(encoder.k(), &mut rng));
    let rate = encoder.k() as f64 / n as f64;
    let channel = AwgnChannel::from_variance(1.0 / (2.0 * rate * 10f64.powf(ebn0 / 10.0)));
    let gain = 2.0 / channel.variance();
    let llrs = (0..n)
        .map(|i| {
            let symbol = if codeword.get(i) { -1.0 } else { 1.0 };
            Llr::new((gain * channel.transmit(symbol, &mut rng)) as f32)
        })
        .collect();
    (codeword, llrs)
}

/// At the waterfall of high-rate DVB-T2 codes the mean channel magnitude
/// exceeds the scale-4 channel cap of 7.75; these frames, found by search
/// at the stated points, fail at the fixed natural-log scale and decode
/// under the cap guard.
#[test]
fn the_cap_guard_decodes_high_rate_dvb_t2_frames_the_fixed_scale_loses() {
    use CodeRate::*;
    let points: [(bool, CodeRate, f64, &[u64]); 4] = [
        (false, Rate5_6, 3.5, &[1569, 1842]),
        (true, Rate5_6, 3.6, &[1169, 2321]),
        (true, Rate4_5, 3.4, &[535]),
        (true, Rate3_4, 3.0, &[5354]),
    ];
    for (normal, rate, ebn0, seeds) in points {
        let code = if normal {
            LdpcCode::dvb_t2_normal(rate)
        } else {
            LdpcCode::dvb_t2_short(rate)
        };
        let encoder = LdpcEncoder::new(code.clone());
        let case = Case {
            name: format!("dvb-t2 normal {normal} {rate:?}"),
            code: code.clone(),
            layers: None,
            sigma: 0.0,
        };
        for &seed in seeds {
            let (codeword, llrs) = awgn_frame(&encoder, code.n(), ebn0, seed);
            for (config, decodes) in [
                (LayeredConfig::dvb_t2(), true),
                (LayeredConfig::dvb_t2().with_fixed_scale(), false),
            ] {
                for mut decoder in backends(&case, config) {
                    let (_, word, ..) = observe(&mut decoder, &llrs, 25);
                    assert_eq!(
                        word == codeword,
                        decodes,
                        "{} seed {seed} {config:?}",
                        case.name
                    );
                }
            }
        }
    }
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

/// Decodes a noiseless and a noisy random codeword at Eb/N0 6 dB on every
/// backend, with and without early termination: the backends agree and both
/// frames decode to the codeword. Without early termination the decoder must
/// also stay on a codeword it has reached.
fn decodes_random_codewords(name: &str, code: &LdpcCode, config: LayeredConfig, seed: u64) {
    let encoder = LdpcEncoder::new(code.clone());
    let mut rng = StdRng::seed_from_u64(seed);
    let codeword = encoder.encode(&BitVec::random(encoder.k(), &mut rng));
    let rate = encoder.k() as f64 / code.n() as f64;
    let channel = AwgnChannel::from_variance(1.0 / (2.0 * rate * 10f64.powf(0.6)));
    let gain = 2.0 / channel.variance();
    let symbol = |i: usize| if codeword.get(i) { -1.0 } else { 1.0 };
    let noiseless: Vec<Llr> = (0..code.n())
        .map(|i| Llr::new((gain * symbol(i)) as f32))
        .collect();
    let noisy: Vec<Llr> = (0..code.n())
        .map(|i| Llr::new((gain * channel.transmit(symbol(i), &mut rng)) as f32))
        .collect();
    let flipped = (0..code.n())
        .filter(|&i| (noisy[i].value() < 0.0) != codeword.get(i))
        .count();
    assert!(flipped > 0, "{name}: the noisy frame has no hard errors");
    let case = Case {
        name: name.into(),
        code: code.clone(),
        layers: None,
        sigma: 0.0,
    };
    let toggled = LayeredConfig::new(
        config.rule(),
        config.channel_scale(),
        config.channel_limit(),
        config.message_limit(),
        !config.early_termination(),
    );
    for (llrs, config) in [&noiseless, &noisy]
        .into_iter()
        .flat_map(|l| [(l, config), (l, toggled)])
    {
        let seen: Vec<Observation> = backends(&case, config)
            .iter_mut()
            .map(|decoder| observe(decoder, llrs, 25))
            .collect();
        for other in &seen[1..] {
            assert_eq!(&seen[0], other, "{name} {config:?}");
        }
        let (_, word, _, converged, passed, _) = &seen[0];
        assert!(*converged && *passed, "{name} {config:?}");
        assert_eq!(word, &codeword, "{name} {config:?}");
    }
}

#[test]
fn every_dvb_t2_code_decodes_on_every_backend() {
    use CodeRate::*;
    for rate in [Rate1_2, Rate3_5, Rate2_3, Rate3_4, Rate4_5, Rate5_6] {
        for (size, code) in [
            ("short", LdpcCode::dvb_t2_short(rate)),
            ("normal", LdpcCode::dvb_t2_normal(rate)),
        ] {
            let name = format!("dvb-t2 {size} {rate:?}");
            decodes_random_codewords(&name, &code, LayeredConfig::dvb_t2(), 0xd7b);
        }
    }
}

#[test]
fn every_nr_lifting_size_off_the_lane_grid_decodes_on_every_backend() {
    for base_graph in [1, 2] {
        for z in all_lifting_sizes().into_iter().filter(|z| z % 32 != 0) {
            let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(base_graph, z.into()));
            let name = format!("nr bg{base_graph} z{z}");
            decodes_random_codewords(&name, &code, standard(), u64::from(z));
        }
    }
}
