//! Heap-allocation census of LDPC decoding, counted per section of the calling
//! thread. Construction and the first decode into a caller's buffer allocate;
//! later decodes through the same decoder and prepared buffer allocate nothing.
//! The rate-matched 5G NR batch decoder (`@/citation/ThreeGpp2017`) is counted
//! over preparation and decoding together. The batch decoders are counted on
//! each lane backend of the build, on the single-frame route and on the QC route.

use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedBatchDecoder;
use gf2_coding::ldpc::{
    BatchRoute, DecodedWord, DecoderAlgorithm, DecoderConfig, LaneBackend, LdpcBatchDecoder,
    LdpcCode, LdpcDecoder, QuasiCyclicLdpc,
};
use gf2_coding::llr::Llr;
use gf2_coding::test_support::{ldpc_batch_route_of, ldpc_batch_routes};
use gf2_coding::traits::{BatchSoftDecoder, DecodeOutcome, IterativeSoftDecoder};
use gf2_coding::tuning::DecodeSelectors;
use gf2_core::BitVec;
use thread_allocations::{count, CountingAllocator, Counts};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Prints one counted section as a JSON record, so a `--nocapture` run yields a
/// machine-readable census.
fn report(phase: &str, case: &str, counts: Counts) {
    println!(
        "{{\"schema\":\"ldpc-decode-allocation-census-v1\",\"phase\":\"{phase}\",\"case\":\"{case}\",\
\"allocations\":{},\"reallocations\":{},\"deallocations\":{},\"bytes_requested\":{}}}",
        counts.allocations, counts.reallocations, counts.deallocations, counts.bytes
    );
}

/// Channel LLRs with both signs and a spread of magnitudes.
fn channel_llrs(n: usize, seed: u64) -> Vec<Llr> {
    // A small integer recurrence, so the inputs are fixed by the seed and the
    // generator itself allocates nothing during a counted section.
    let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    (0..n)
        .map(|_| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let magnitude = 0.05 + (state >> 40) as f32 / 1e5;
            let sign = if state & (1 << 33) == 0 { 1.0 } else { -1.0 };
            Llr::new(sign * magnitude)
        })
        .collect()
}

const ALGORITHMS: [DecoderAlgorithm; 4] = [
    DecoderAlgorithm::MinSum,
    DecoderAlgorithm::NormalizedMinSum(0.75),
    DecoderAlgorithm::OffsetMinSum(0.5),
    DecoderAlgorithm::SumProduct,
];

fn codes() -> Vec<(&'static str, LdpcCode)> {
    vec![
        (
            "dvb-t2-short-r12",
            LdpcCode::dvb_t2_short(gf2_coding::CodeRate::Rate1_2),
        ),
        (
            "nr-bg1-z8",
            LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(1, 8)),
        ),
    ]
}

#[test]
fn steady_state_decoding_allocates_nothing() {
    for (label, code) in codes() {
        let llrs = channel_llrs(code.n(), 17);
        for algorithm in ALGORITHMS {
            for early_termination in [true, false] {
                let config = DecoderConfig::new(algorithm, early_termination);
                let mut decoder = LdpcDecoder::with_config(code.clone(), config);
                let mut codeword = BitVec::with_capacity(code.n());

                // Prepare the workspace: one decode grows the caller's buffer.
                decoder.decode_codeword_into(&llrs, 8, &mut codeword);

                let context =
                    format!("{label}, {algorithm:?}, early_termination={early_termination}");
                for repeat in 0..3 {
                    let (_, counts) =
                        count(|| decoder.decode_codeword_into(&llrs, 8, &mut codeword));
                    report(
                        "steady-state",
                        &format!("{context}, decode {repeat}"),
                        counts,
                    );
                    assert!(
                        counts.is_zero(),
                        "steady-state decode {repeat} of {context} requested {counts:?}"
                    );
                }
            }
        }
    }
}

/// Asserts that construction and workspace growth allocate, without a figure:
/// the sizes follow from the code's dimensions.
#[test]
fn construction_and_workspace_growth_are_counted_separately() {
    let code = LdpcCode::dvb_t2_short(gf2_coding::CodeRate::Rate1_2);
    let llrs = channel_llrs(code.n(), 23);
    let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);

    let code_for_build = code.clone();
    let (mut decoder, construction) =
        count(move || LdpcDecoder::with_config(code_for_build, config));
    report(
        "construction",
        "dvb-t2-short-r12, NormalizedMinSum(0.75)",
        construction,
    );
    assert!(
        construction.allocations > 0,
        "constructing a decoder allocates its layout and message arrays"
    );

    let mut codeword = BitVec::new();
    let (_, growth) = count(|| decoder.decode_codeword_into(&llrs, 6, &mut codeword));
    report(
        "workspace-growth",
        "dvb-t2-short-r12, NormalizedMinSum(0.75)",
        growth,
    );
    assert!(
        growth.allocations + growth.reallocations > 0,
        "the first decode into an empty buffer grows it"
    );
    assert_eq!(codeword.len(), code.n());

    let (_, steady) = count(|| decoder.decode_codeword_into(&llrs, 6, &mut codeword));
    report(
        "steady-state",
        "dvb-t2-short-r12, NormalizedMinSum(0.75), prepared buffer",
        steady,
    );
    assert!(steady.is_zero(), "prepared decode requested {steady:?}");
}

#[test]
fn decoding_after_reset_allocates_nothing() {
    let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(2, 8));
    let llrs = channel_llrs(code.n(), 29);
    let config = DecoderConfig::new(DecoderAlgorithm::MinSum, true);
    let mut decoder = LdpcDecoder::with_config(code.clone(), config);
    let mut codeword = BitVec::with_capacity(code.n());
    decoder.decode_codeword_into(&llrs, 8, &mut codeword);

    let (_, counts) = count(|| {
        decoder.reset();
        decoder.decode_codeword_into(&llrs, 8, &mut codeword);
    });
    report("steady-state-after-reset", "nr-bg2-z8, MinSum", counts);
    assert!(counts.is_zero(), "decode after reset requested {counts:?}");
}

/// A batch of one full wave and a final wave of one frame, decoded into words
/// and a posterior buffer that an earlier decode has sized, under each setup
/// of [`ldpc_batch_routes`].
#[test]
fn steady_state_batch_decoding_allocates_nothing() {
    const FRAMES: usize = 9;
    let routes = ldpc_batch_routes();
    for (label, code) in codes() {
        let frames: Vec<Vec<Llr>> = (0..FRAMES as u64)
            .map(|frame| channel_llrs(code.n(), 41 + frame))
            .collect();
        let views: Vec<&[Llr]> = frames.iter().map(Vec::as_slice).collect();
        let mut outcomes = vec![DecodeOutcome::default(); FRAMES];
        let mut posteriors = vec![Llr::zero(); FRAMES * code.n()];

        for algorithm in ALGORITHMS {
            for word in [DecodedWord::Codeword, DecodedWord::Message] {
                for (selectors, backend, full_wave) in &routes {
                    let config = DecoderConfig::new(algorithm, true);
                    let mut decoder = LdpcBatchDecoder::with_selectors(
                        code.clone(),
                        config,
                        2,
                        word,
                        selectors.clone(),
                    );
                    if *backend == LaneBackend::Portable {
                        decoder = decoder.with_portable_lanes();
                    }
                    let route = decoder.batch_route(FRAMES);
                    if algorithm == DecoderAlgorithm::SumProduct {
                        assert_eq!(route, BatchRoute::SingleFrame);
                    } else {
                        assert_eq!(route, ldpc_batch_route_of(&code, *full_wave));
                    }
                    let mut decoded = vec![BitVec::new(); FRAMES];

                    // Prepare the outputs: one decode grows each word.
                    decoder.decode_batch(&views, &mut decoded, &mut outcomes);

                    let context =
                        format!("{label}, {algorithm:?}, {word:?}, {selectors:?}, {route:?}");
                    let (_, counts) =
                        count(|| decoder.decode_batch(&views, &mut decoded, &mut outcomes));
                    report("batch-steady-state", &context, counts);
                    assert!(
                        counts.is_zero(),
                        "batch decode of {context} requested {counts:?}"
                    );

                    let (_, counts) = count(|| {
                        decoder.decode_batch_with_posteriors(
                            &views,
                            &mut decoded,
                            &mut outcomes,
                            &mut posteriors,
                        );
                    });
                    report("batch-steady-state-posteriors", &context, counts);
                    assert!(
                        counts.is_zero(),
                        "batch decode with posteriors of {context} requested {counts:?}"
                    );
                }
            }
        }
    }
}

/// Rate-matched frames of one full wave and a final wave of one frame, each
/// prepared onto the mother code and decoded into message words that an
/// earlier decode has sized, under each setup of [`ldpc_batch_routes`].
#[test]
fn steady_state_rate_matched_batch_decoding_allocates_nothing() {
    const FRAMES: usize = 9;
    let routes = ldpc_batch_routes();
    for (base_graph, n, k) in [(1, 600, 200), (2, 256, 49)] {
        let code = QuasiCyclicLdpc::nr_5g_rate_matched(base_graph, n, k);
        let params = code.params();
        assert!(
            params.num_punctured_systematic > 0
                && params.num_punctured_parity > 0
                && params.num_shortened > 0
        );
        let frames: Vec<Vec<Llr>> = (0..FRAMES as u64)
            .map(|frame| channel_llrs(n, 53 + frame))
            .collect();
        let views: Vec<&[Llr]> = frames.iter().map(Vec::as_slice).collect();
        let mut outcomes = vec![DecodeOutcome::default(); FRAMES];

        for algorithm in ALGORITHMS {
            for (selectors, backend, full_wave) in &routes {
                let mut decoder = Nr5gRateMatchedBatchDecoder::with_selectors(
                    code.clone(),
                    algorithm,
                    2,
                    selectors.clone(),
                );
                if *backend == LaneBackend::Portable {
                    decoder = decoder.with_portable_lanes();
                }
                let route = decoder.batch_route(FRAMES);
                if algorithm == DecoderAlgorithm::SumProduct {
                    assert_eq!(route, BatchRoute::SingleFrame);
                } else {
                    assert_eq!(route, ldpc_batch_route_of(code.mother_code(), *full_wave));
                }
                let mut decoded = vec![BitVec::new(); FRAMES];

                // Prepare the outputs: one decode grows each word.
                decoder.decode_batch(&views, &mut decoded, &mut outcomes);

                let context =
                    format!("nr-bg{base_graph}-n{n}-k{k}, {algorithm:?}, {selectors:?}, {route:?}");
                for repeat in 0..2 {
                    let (_, counts) =
                        count(|| decoder.decode_batch(&views, &mut decoded, &mut outcomes));
                    report(
                        "rate-matched-batch-steady-state",
                        &format!("{context}, decode {repeat}"),
                        counts,
                    );
                    assert!(
                        counts.is_zero(),
                        "rate-matched batch decode {repeat} of {context} requested {counts:?}"
                    );
                }
            }
        }
    }
}

/// The owning entry points allocate the vectors they return, which is why the
/// steady-state claim is made about the buffer-writing one.
#[test]
fn owning_entry_points_allocate_what_they_return() {
    let code = LdpcCode::dvb_t2_short(gf2_coding::CodeRate::Rate1_2);
    let llrs = channel_llrs(code.n(), 31);
    let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
    let mut decoder = LdpcDecoder::with_config(code.clone(), config);
    let mut codeword = BitVec::with_capacity(code.n());
    decoder.decode_codeword_into(&llrs, 6, &mut codeword);

    let (result, counts) = count(|| decoder.decode_to_codeword(&llrs, 6));
    report(
        "owning-decode-to-codeword",
        "dvb-t2-short-r12, NormalizedMinSum(0.75)",
        counts,
    );
    assert_eq!(result.decoded_bits.len(), code.n());
    assert!(
        counts.allocations > 0,
        "decode_to_codeword returns an owned codeword"
    );

    // The first `decode_iterative` also resolves the systematic columns, so a
    // later call is the one that shows the per-call cost.
    decoder.decode_iterative(&llrs, 6);
    let (message, counts) = count(|| decoder.decode_iterative(&llrs, 6));
    report(
        "owning-decode-iterative",
        "dvb-t2-short-r12, NormalizedMinSum(0.75)",
        counts,
    );
    assert_eq!(message.decoded_bits.len(), code.k());
    assert!(
        counts.allocations > 0,
        "decode_iterative returns an owned message"
    );
}

#[test]
fn the_qc_route_allocates_nothing_after_a_first_decode() {
    let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(2, 6));
    let frames: Vec<Vec<Llr>> = (0..4u64).map(|f| channel_llrs(code.n(), 41 + f)).collect();
    let views: Vec<&[Llr]> = frames.iter().map(Vec::as_slice).collect();
    let selectors = DecodeSelectors::CONSERVATIVE;
    let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
    let mut outcomes = vec![DecodeOutcome::default(); views.len()];
    let mut posteriors = vec![Llr::zero(); views.len() * code.n()];
    for word in [DecodedWord::Codeword, DecodedWord::Message] {
        let mut decoder =
            LdpcBatchDecoder::with_selectors(code.clone(), config, 2, word, selectors.clone());
        let mut decoded = vec![BitVec::new(); views.len()];
        decoder.decode_batch(&views, &mut decoded, &mut outcomes);
        assert!(matches!(decoder.last_wave_route(), Some(BatchRoute::Qc(_))));
        let (_, counts) = count(|| {
            decoder.decode_batch(&views, &mut decoded, &mut outcomes);
            decoder.decode_batch_with_posteriors(
                &views,
                &mut decoded,
                &mut outcomes,
                &mut posteriors,
            );
        });
        report("qc-batch-steady-state", &format!("{word:?}"), counts);
        assert!(counts.is_zero(), "{word:?} requested {counts:?}");
    }
}
