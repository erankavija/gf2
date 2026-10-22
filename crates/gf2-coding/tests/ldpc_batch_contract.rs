//! Posterior-level contract of [`LdpcBatchDecoder`]: on each route and lane
//! backend a frame's posterior bit patterns, hard decisions, iteration count
//! and flags equal [`LdpcDecoder`]'s on the same LLRs, under the batch clauses
//! of `docs/reference/ldpc-decoder-numerical-contract.md`. The AVX2 backend is
//! under test in a build with the `simd` cargo feature. The codes are a DVB-T2
//! short code (`@/citation/Etsi2015`), 5G NR mother codes
//! (`@/citation/ThreeGpp2017`) and hand-built codes with low-degree and
//! high-degree checks.

use gf2_coding::bch::CodeRate;
use gf2_coding::channel::AwgnChannel;
use gf2_coding::ldpc::nr_5g::FILLER_LLR;
use gf2_coding::ldpc::{
    BatchRoute, DecodedWord, DecoderAlgorithm, DecoderConfig, EdgeLayout, LaneBackend,
    LdpcBatchDecoder, LdpcCode, LdpcDecoder, LdpcEncoder, QuasiCyclicLdpc,
};
use gf2_coding::llr::Llr;
use gf2_coding::test_support::ldpc_lane_backends;
use gf2_coding::traits::conformance::{batch_soft_decoder_contract, ConvergenceMix};
use gf2_coding::traits::{BatchSoftDecoder, BlockEncoder, DecodeOutcome};
use gf2_coding::tuning::DecodeSelectors;
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

const SEED: u64 = 0x941a_c609;

const RULES: [DecoderAlgorithm; 3] = [
    DecoderAlgorithm::MinSum,
    DecoderAlgorithm::NormalizedMinSum(0.75),
    DecoderAlgorithm::OffsetMinSum(0.5),
];

/// Batch sizes around each wave boundary of the first three waves.
const BATCH_SIZES: [usize; 8] = [0, 1, 7, 8, 9, 15, 16, 17];

/// Frames of every frame set: the largest batch size.
const FRAMES: usize = 17;

/// Noise deviations of consecutive frames relative to a code's operating
/// deviation, from clean to undecodable, so that a wave holds frames of each
/// kind.
const SIGMA_LADDER: [f64; 6] = [0.5, 1.0, 2.2, 0.9, 1.7, 1.1];

/// A route of [`LdpcBatchDecoder`] under test, opened by its selectors and
/// its lane backend.
#[derive(Clone, Copy, Debug)]
enum Route {
    /// Every wave decodes in the lane kernels of the backend.
    Lane(LaneBackend),
    /// A wave of at least four frames decodes in the lane kernels of the
    /// backend and a shorter one on the single-frame decoder.
    LaneFromFourFrames(LaneBackend),
    /// Every wave decodes on the single-frame decoder.
    SingleFrame,
}

impl Route {
    fn selectors(self) -> DecodeSelectors {
        let lane_min_frames = match self {
            Route::Lane(_) => 1,
            Route::LaneFromFourFrames(_) => 4,
            Route::SingleFrame => usize::MAX,
        };
        DecodeSelectors::try_new(lane_min_frames, usize::MAX, 1, 1).unwrap()
    }

    /// The route of a wave of `frames` frames under a min-sum rule.
    fn of_wave(self, frames: usize) -> BatchRoute {
        match self {
            Route::Lane(backend) => BatchRoute::Lane(backend),
            Route::LaneFromFourFrames(backend) if frames >= 4 => BatchRoute::Lane(backend),
            Route::LaneFromFourFrames(_) | Route::SingleFrame => BatchRoute::SingleFrame,
        }
    }

    /// A decoder of `code` on this route.
    ///
    /// # Panics
    ///
    /// Panics when the build or the host lacks the backend of the route.
    fn decoder(self, code: &LdpcCode, config: DecoderConfig, cap: usize) -> LdpcBatchDecoder {
        let decoder = LdpcBatchDecoder::with_selectors(
            code.clone(),
            config,
            cap,
            DecodedWord::Codeword,
            self.selectors(),
        );
        match self {
            Route::Lane(backend) | Route::LaneFromFourFrames(backend) => {
                assert!(ldpc_lane_backends().contains(&backend));
                match backend {
                    LaneBackend::Portable => decoder.with_portable_lanes(),
                    LaneBackend::Avx2 => decoder,
                }
            }
            Route::SingleFrame => decoder,
        }
    }
}

/// The observable result of one frame.
#[derive(Debug, PartialEq)]
struct FrameResult {
    codeword: BitVec,
    outcome: DecodeOutcome,
    posterior: Vec<u32>,
}

fn posterior_bits(posterior: &[Llr]) -> Vec<u32> {
    posterior.iter().map(|llr| llr.value().to_bits()).collect()
}

fn single_frame_results(
    code: &LdpcCode,
    config: DecoderConfig,
    cap: usize,
    frames: &[Vec<Llr>],
) -> Vec<FrameResult> {
    let mut single = LdpcDecoder::with_config(code.clone(), config);
    frames
        .iter()
        .map(|llrs| {
            let result = single.decode_to_codeword(llrs, cap);
            FrameResult {
                outcome: result.outcome(),
                codeword: result.decoded_bits,
                posterior: posterior_bits(single.posterior_llrs()),
            }
        })
        .collect()
}

fn batch_results(decoder: &mut LdpcBatchDecoder, frames: &[&[Llr]]) -> Vec<FrameResult> {
    let n = decoder.frame_len();
    let mut decoded = vec![BitVec::new(); frames.len()];
    let mut outcomes = vec![DecodeOutcome::default(); frames.len()];
    let mut posteriors = vec![Llr::zero(); frames.len() * n];
    decoder.decode_batch_with_posteriors(frames, &mut decoded, &mut outcomes, &mut posteriors);
    decoded
        .into_iter()
        .zip(outcomes)
        .enumerate()
        .map(|(f, (codeword, outcome))| FrameResult {
            codeword,
            outcome,
            posterior: posterior_bits(&posteriors[f * n..(f + 1) * n]),
        })
        .collect()
}

fn assert_frame_eq(got: &FrameResult, expected: &FrameResult, context: std::fmt::Arguments) {
    assert_eq!(got.outcome, expected.outcome, "outcome of {context}");
    assert!(
        got.codeword == expected.codeword,
        "hard decisions of {context}"
    );
    let differing =
        (0..expected.posterior.len()).find(|&v| got.posterior[v] != expected.posterior[v]);
    if let Some(v) = differing {
        panic!(
            "posterior of {context} at variable {v}: {:#010x}, single-frame {:#010x}",
            got.posterior[v], expected.posterior[v]
        );
    }
}

/// Whether the frames of one wave converge at different iterations and one
/// reaches the cap without converging.
fn mixes_convergence(wave: &[FrameResult], cap: usize) -> bool {
    let stops = || {
        wave.iter()
            .filter(|frame| frame.outcome.converged)
            .map(|frame| frame.outcome.iterations)
    };
    let capped = wave
        .iter()
        .any(|frame| !frame.outcome.converged && frame.outcome.iterations == cap);
    capped && stops().min() < stops().max()
}

/// Frames of one input kind for one code.
struct FrameSet {
    name: &'static str,
    frames: Vec<Vec<Llr>>,
    /// Whether each full wave holds frames that converge early, converge late
    /// and reach the cap under every rule with early termination.
    wave_mix: bool,
}

struct Case {
    code: LdpcCode,
    cap: usize,
    sets: Vec<FrameSet>,
}

/// Asserts that `route` gives each frame of each set of `case` the result of
/// the single-frame decoder, for every rule and both early-termination
/// settings: over each of [`BATCH_SIZES`], under a permutation of the frames,
/// and with the other frames of its wave replaced by a non-converging frame.
fn assert_route_equals_single_frame(route: Route, case: &Case) {
    let Case { code, cap, sets } = case;
    for set in sets {
        assert_eq!(set.frames.len(), FRAMES, "{}", set.name);
        let views: Vec<&[Llr]> = set.frames.iter().map(Vec::as_slice).collect();
        for (algorithm, early_termination) in RULES
            .into_iter()
            .flat_map(|rule| [(rule, true), (rule, false)])
        {
            let config = DecoderConfig::new(algorithm, early_termination);
            let label = format!("{} under {config:?} on {route:?}", set.name);
            let expected = single_frame_results(code, config, *cap, &set.frames);
            let mut decoder = route.decoder(code, config, *cap);
            let lanes = decoder.batch_granularity();
            for size in 1..=lanes {
                assert_eq!(decoder.batch_route(size), route.of_wave(size), "{label}");
            }

            if set.wave_mix && early_termination {
                for wave in expected.chunks_exact(lanes) {
                    let stops: Vec<_> = wave.iter().map(|frame| frame.outcome).collect();
                    assert!(
                        mixes_convergence(wave, *cap),
                        "{label}: a wave needs early, late and capped frames, has {stops:?}"
                    );
                }
            }

            for size in BATCH_SIZES {
                let got = batch_results(&mut decoder, &views[..size]);
                assert_eq!(got.len(), size);
                for (f, (got, expected)) in got.iter().zip(&expected).enumerate() {
                    assert_frame_eq(got, expected, format_args!("frame {f} of {size}, {label}"));
                }
            }

            let mut order: Vec<usize> = (0..FRAMES).collect();
            order.shuffle(&mut StdRng::seed_from_u64(SEED));
            assert!(order.iter().enumerate().any(|(slot, &f)| slot != f));
            let in_order = batch_results(&mut decoder, &views);
            let permuted: Vec<&[Llr]> = order.iter().map(|&f| views[f]).collect();
            for (slot, got) in batch_results(&mut decoder, &permuted).iter().enumerate() {
                let f = order[slot];
                assert_frame_eq(
                    got,
                    &in_order[f],
                    format_args!("frame {f} permuted to slot {slot}, {label}"),
                );
            }

            let stalled = expected
                .iter()
                .position(|frame| !frame.outcome.converged)
                .unwrap_or_else(|| panic!("{label}: the set needs a non-converging frame"));
            for lane in 0..lanes {
                let mut wave = vec![views[stalled]; lanes];
                wave[lane] = views[lane];
                let got = batch_results(&mut decoder, &wave);
                assert_frame_eq(
                    &got[lane],
                    &expected[lane],
                    format_args!("frame {lane} among non-converging frames, {label}"),
                );
            }
        }
    }
}

/// Asserts the shared batch contract on `route` over the first frame set of
/// `case`, for every rule with early termination.
fn assert_route_satisfies_the_batch_contract(route: Route, case: &Case) {
    let Case { code, cap, sets } = case;
    for algorithm in RULES {
        let config = DecoderConfig::new(algorithm, true);
        let mut decoder = route.decoder(code, config, *cap);
        let mut single = LdpcDecoder::with_config(code.clone(), config);
        batch_soft_decoder_contract(
            &mut decoder,
            |llrs| single.decode_to_codeword(llrs, *cap),
            &sets[0].frames,
            ConvergenceMix::EarlyLateAndNonConverging,
        );
    }
}

/// The LLR frame of BPSK-modulated `codeword` through AWGN of deviation
/// `sigma`.
fn noisy_frame(codeword: &BitVec, sigma: f64, rng: &mut StdRng) -> Vec<Llr> {
    let channel = AwgnChannel::from_variance(sigma * sigma);
    let scale = 2.0 / channel.variance();
    (0..codeword.len())
        .map(|i| {
            let symbol = if codeword.get(i) { -1.0 } else { 1.0 };
            Llr::new((scale * channel.transmit(symbol, rng)) as f32)
        })
        .collect()
}

/// [`FRAMES`] noisy frames, frame `f` of `codeword(f)` at the deviation
/// `sigma` scaled by its entry of [`SIGMA_LADDER`].
fn noisy_frames(sigma: f64, mut codeword: impl FnMut(&mut StdRng) -> BitVec) -> Vec<Vec<Llr>> {
    let mut rng = StdRng::seed_from_u64(SEED);
    (0..FRAMES)
        .map(|f| {
            let codeword = codeword(&mut rng);
            let sigma = sigma * SIGMA_LADDER[f % SIGMA_LADDER.len()];
            noisy_frame(&codeword, sigma, &mut rng)
        })
        .collect()
}

fn random_codeword_frames(code: &LdpcCode, sigma: f64) -> FrameSet {
    let encoder = LdpcEncoder::new(code.clone());
    let frames = noisy_frames(sigma, |rng| {
        let codeword = encoder.encode(&BitVec::random(encoder.k(), rng));
        assert!(code.is_valid_codeword(&codeword));
        codeword
    });
    FrameSet {
        name: "random codewords through noise",
        frames,
        wave_mix: true,
    }
}

fn all_zero_codeword_frames(code: &LdpcCode, sigma: f64) -> FrameSet {
    FrameSet {
        name: "the all-zero codeword through noise",
        frames: noisy_frames(sigma, |_| BitVec::zeros(code.n())),
        wave_mix: true,
    }
}

const SIGNED_ZEROS: [f32; 2] = [0.0, -0.0];

const EXTREME_FINITE: [f32; 6] = [
    f32::MAX,
    f32::MIN,
    f32::MIN_POSITIVE,
    -f32::MIN_POSITIVE,
    f32::from_bits(1),
    -f32::from_bits(1),
];

/// Infinities and NaNs of both signs, quiet and signalling, with payloads.
const NON_FINITE: [f32; 6] = [
    f32::INFINITY,
    f32::NEG_INFINITY,
    f32::NAN,
    f32::from_bits(0xffc0_0000),
    f32::from_bits(0x7fc1_2345),
    f32::from_bits(0xffa0_0001),
];

/// The magnitude frames are saturated to; below most noisy magnitudes, so
/// that checks see tied inputs.
const SATURATION: f32 = 1.0;

/// Fractions of a frame's positions that take a special value, by frame.
const SPECIAL_DENSITIES: [f64; 4] = [0.02, 0.2, 0.6, 1.0];

/// `base` with seeded positions of each frame overwritten by values of
/// `values`, at the fraction [`SPECIAL_DENSITIES`] gives the frame.
fn overwritten(name: &'static str, base: &[Vec<Llr>], values: &[f32]) -> FrameSet {
    let mut rng = StdRng::seed_from_u64(SEED);
    let frames = base
        .iter()
        .enumerate()
        .map(|(f, frame)| {
            let density = SPECIAL_DENSITIES[f % SPECIAL_DENSITIES.len()];
            frame
                .iter()
                .map(|&llr| {
                    if rng.gen_bool(density) {
                        Llr::new(values[rng.gen_range(0..values.len())])
                    } else {
                        llr
                    }
                })
                .collect()
        })
        .collect();
    FrameSet {
        name,
        frames,
        wave_mix: false,
    }
}

/// The frame sets of special values over the frames of `base`.
fn special_value_frames(base: &[Vec<Llr>]) -> Vec<FrameSet> {
    vec![
        overwritten("frames holding signed zeros", base, &SIGNED_ZEROS),
        FrameSet {
            name: "saturated frames",
            frames: base
                .iter()
                .map(|frame| Llr::saturate_batch(frame, SATURATION))
                .collect(),
            wave_mix: false,
        },
        overwritten("frames holding extreme finite LLRs", base, &EXTREME_FINITE),
        overwritten("frames holding NaN and infinite LLRs", base, &NON_FINITE),
    ]
}

/// Noisy frames of random codewords and of the all-zero codeword at the
/// operating deviation `sigma`, with the special-value sets when
/// `special_values` is set.
fn case(code: LdpcCode, cap: usize, sigma: f64, special_values: bool) -> Case {
    let mut sets = vec![
        random_codeword_frames(&code, sigma),
        all_zero_codeword_frames(&code, sigma),
    ];
    if special_values {
        sets.extend(special_value_frames(&sets[0].frames));
    }
    Case { code, cap, sets }
}

const CAP: usize = 20;

fn dvb_t2_short(set: fn(&LdpcCode, f64) -> FrameSet) -> Case {
    let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    let sets = vec![set(&code, 0.6)];
    Case { code, cap: 8, sets }
}

fn nr_mother(base_graph: u8, lifting_size: usize, sigma: f64, special_values: bool) -> Case {
    let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(base_graph, lifting_size));
    case(code, CAP, sigma, special_values)
}

/// Frames of a rate-matched 5G NR code as its rate-matching layer hands them
/// to the mother-code decoder.
fn nr_rate_matched(base_graph: u8, n: usize, k: usize, sigma: f64) -> Case {
    let code = QuasiCyclicLdpc::nr_5g_rate_matched(base_graph, n, k);
    let params = code.params();
    assert!(params.num_punctured_systematic > 0 && params.num_shortened > 0);

    let frames = noisy_frames(sigma, |rng| code.encode(&BitVec::random(code.k(), rng)));
    let frames: Vec<Vec<Llr>> = frames
        .iter()
        .map(|frame| code.prepare_llrs(frame))
        .collect();
    for frame in &frames {
        let count = |value: f32| frame.iter().filter(|llr| llr.value() == value).count();
        assert!(count(0.0) >= params.num_punctured_systematic);
        assert!(count(FILLER_LLR) >= params.num_shortened);
    }
    Case {
        code: code.mother_code().clone(),
        cap: CAP,
        sets: vec![FrameSet {
            name: "rate-matched frames with punctured and filler positions",
            frames,
            wave_mix: true,
        }],
    }
}

/// A (96, 48) quasi-cyclic code of column weight three and check degree five.
fn regular_edges() -> Vec<(usize, usize)> {
    let base = vec![
        vec![0, 3, 7, 1, 0, -1],
        vec![5, 0, 11, -1, 2, 9],
        vec![9, 13, 0, 6, -1, 4],
    ];
    QuasiCyclicLdpc::new(base, 16).to_edges()
}

fn check_degrees(code: &LdpcCode) -> Vec<usize> {
    EdgeLayout::from_parity_check(code.parity_check_matrix())
        .check_offsets()
        .windows(2)
        .map(|bounds| (bounds[1] - bounds[0]) as usize)
        .collect()
}

/// The code of [`regular_edges`] with four more checks, each on one variable.
fn degree_one_checks() -> Case {
    let mut edges = regular_edges();
    edges.extend([0, 17, 40, 77].into_iter().zip(48..).map(|(v, c)| (c, v)));
    let code = LdpcCode::from_edges(52, 96, &edges);
    assert!(check_degrees(&code).contains(&1));
    case(code, CAP, 0.8, true)
}

/// The code of [`regular_edges`] with eight more variables, each joined to
/// one of its variables by a check of two edges.
fn degree_two_checks() -> Case {
    let mut edges = regular_edges();
    for copy in 0..8 {
        edges.extend([(48 + copy, 7 * copy + 1), (48 + copy, 96 + copy)]);
    }
    let code = LdpcCode::from_edges(56, 104, &edges);
    assert!(check_degrees(&code).contains(&2));
    case(code, CAP, 0.8, true)
}

/// A (192, 144) quasi-cyclic code whose checks have ten and eleven edges.
fn high_degree_checks() -> Case {
    let base = vec![
        vec![1, 5, 9, 2, 14, 7, 0, 11, 4, 0, -1, -1],
        vec![3, 8, 0, 13, 6, 10, 15, 2, 12, 7, 0, -1],
        vec![6, 0, 11, 4, 9, 1, 13, 8, 3, -1, 5, 0],
    ];
    let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::new(base, 16));
    assert!(check_degrees(&code).iter().all(|&degree| degree >= 9));
    case(code, CAP, 0.5, true)
}

/// The cases of the suite on one route. Each 5G NR base graph is taken at
/// three lifting sizes of different lifting-size sets.
macro_rules! route_suite {
    ($route:ident, $value:expr) => {
        mod $route {
            use super::*;

            fn assert_case(case: Case) {
                assert_route_equals_single_frame($value, &case);
            }

            #[test]
            fn dvb_t2_short_random_codewords() {
                assert_case(dvb_t2_short(random_codeword_frames));
            }

            #[test]
            fn dvb_t2_short_all_zero_codeword() {
                assert_case(dvb_t2_short(all_zero_codeword_frames));
            }

            #[test]
            fn nr_base_graph_1_lifting_size_4() {
                assert_case(nr_mother(1, 4, 1.0, true));
            }

            #[test]
            fn nr_base_graph_1_lifting_size_10() {
                assert_case(nr_mother(1, 10, 1.0, false));
            }

            #[test]
            fn nr_base_graph_1_lifting_size_24() {
                assert_case(nr_mother(1, 24, 1.0, false));
            }

            #[test]
            fn nr_base_graph_2_lifting_size_6() {
                assert_case(nr_mother(2, 6, 1.2, true));
            }

            #[test]
            fn nr_base_graph_2_lifting_size_14() {
                assert_case(nr_mother(2, 14, 1.2, false));
            }

            #[test]
            fn nr_base_graph_2_lifting_size_36() {
                assert_case(nr_mother(2, 36, 1.2, false));
            }

            #[test]
            fn nr_base_graph_1_rate_matched_punctured_and_filler() {
                assert_case(nr_rate_matched(1, 600, 200, 0.9));
            }

            #[test]
            fn nr_base_graph_2_rate_matched_punctured_and_filler() {
                assert_case(nr_rate_matched(2, 256, 49, 1.0));
            }

            #[test]
            fn hand_built_degree_one_checks() {
                assert_case(degree_one_checks());
            }

            #[test]
            fn hand_built_degree_two_checks() {
                assert_case(degree_two_checks());
            }

            #[test]
            fn hand_built_high_degree_checks() {
                assert_case(high_degree_checks());
            }

            #[test]
            fn shared_batch_contract_on_nr_base_graph_2() {
                assert_route_satisfies_the_batch_contract($value, &nr_mother(2, 6, 1.2, false));
            }

            #[test]
            fn shared_batch_contract_on_high_degree_checks() {
                assert_route_satisfies_the_batch_contract($value, &high_degree_checks());
            }
        }
    };
}

#[cfg(feature = "simd")]
route_suite!(avx2_lane_route, Route::Lane(LaneBackend::Avx2));
#[cfg(feature = "simd")]
route_suite!(
    avx2_lane_route_from_four_frames,
    Route::LaneFromFourFrames(LaneBackend::Avx2)
);
route_suite!(portable_lane_route, Route::Lane(LaneBackend::Portable));
route_suite!(
    portable_lane_route_from_four_frames,
    Route::LaneFromFourFrames(LaneBackend::Portable)
);
route_suite!(single_frame_route, Route::SingleFrame);

/// A decoder of the (96, 48) code of [`regular_edges`] under selectors that
/// open the lane route for every wave, on the backend of the build and host.
fn detected_lane_decoder() -> LdpcBatchDecoder {
    LdpcBatchDecoder::with_selectors(
        LdpcCode::from_edges(48, 96, &regular_edges()),
        DecoderConfig::default(),
        CAP,
        DecodedWord::Codeword,
        Route::Lane(LaneBackend::detected()).selectors(),
    )
}

#[cfg(feature = "simd")]
#[test]
fn a_build_with_simd_runs_the_avx2_lane_backend_where_avx2_is_detected() {
    assert!(ldpc_lane_backends().contains(&LaneBackend::Avx2));
    let decoder = detected_lane_decoder();
    for frames in 1..=decoder.batch_granularity() {
        assert_eq!(
            decoder.batch_route(frames),
            BatchRoute::Lane(LaneBackend::Avx2)
        );
    }
}

#[cfg(not(feature = "simd"))]
#[test]
fn a_build_without_simd_runs_the_portable_lane_backend() {
    assert_eq!(ldpc_lane_backends(), [LaneBackend::Portable]);
    let decoder = detected_lane_decoder();
    for frames in 1..=decoder.batch_granularity() {
        assert_eq!(
            decoder.batch_route(frames),
            BatchRoute::Lane(LaneBackend::Portable)
        );
    }
}

#[test]
fn a_decoder_held_on_the_portable_lanes_reports_the_portable_backend() {
    let decoder = detected_lane_decoder().with_portable_lanes();
    for frames in 1..=decoder.batch_granularity() {
        assert_eq!(
            decoder.batch_route(frames),
            BatchRoute::Lane(LaneBackend::Portable)
        );
    }
}
