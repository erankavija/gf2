//! Checks of [`DvbT2ConcatBatchDecoder`] against
//! [`DvbT2Concat::decode_soft_counted`], shared by the DVB-T2 suites that
//! soft-decode through the concatenated codec.

#![allow(dead_code)] // each test binary uses only the helpers it needs

use gf2_coding::ldpc::dvb_t2::concat::{ConcatError, DvbT2Concat, DvbT2ConcatBatchDecoder};
use gf2_coding::ldpc::LaneBackend;
use gf2_coding::llr::Llr;
pub use gf2_coding::test_support::ldpc_batch_routes as routes;
use gf2_coding::traits::{BatchSoftDecoder, DecodeOutcome, DecoderResult};
use gf2_coding::tuning::DecodeSelectors;
use gf2_core::BitVec;

/// Selectors under which a wave of at least `lane_min_frames` frames takes
/// the lane route for a min-sum rule, whatever the edge count.
pub fn selectors(lane_min_frames: usize) -> DecodeSelectors {
    DecodeSelectors::try_new(lane_min_frames, usize::MAX, 1).unwrap()
}

/// The batch decoder of `codec` under `selectors` whose lane route runs
/// `backend`, one of the backends of [`routes`].
pub fn decoder_on(
    backend: LaneBackend,
    codec: &DvbT2Concat,
    selectors: DecodeSelectors,
) -> DvbT2ConcatBatchDecoder {
    let decoder = DvbT2ConcatBatchDecoder::with_selectors(codec, selectors);
    match backend {
        LaneBackend::Portable => decoder.with_portable_lanes(),
        LaneBackend::Avx2 => decoder,
    }
}

/// The result of [`DvbT2Concat::decode_soft_counted`] in the form the batch
/// trait reports a frame: the BBFRAME of either arm, the LDPC iteration count,
/// and both flags set where the LDPC decode converged.
pub fn single_frame_result(codec: &DvbT2Concat, llrs: &[Llr]) -> DecoderResult {
    match codec.decode_soft_counted(llrs) {
        Ok((bbframe, iterations)) => DecoderResult::new(bbframe, iterations, true, true),
        Err(ConcatError::LdpcDecodeFailed {
            bbframe,
            iterations,
        }) => DecoderResult::new(bbframe, iterations, false, false),
        Err(error) => panic!("{error}"),
    }
}

/// Decodes `frames` as one batch on `decoder`.
pub fn batch_results(
    decoder: &mut DvbT2ConcatBatchDecoder,
    frames: &[Vec<Llr>],
) -> Vec<DecoderResult> {
    let views: Vec<&[Llr]> = frames.iter().map(Vec::as_slice).collect();
    let mut decoded = vec![BitVec::new(); frames.len()];
    let mut outcomes = vec![DecodeOutcome::default(); frames.len()];
    decoder.decode_batch(&views, &mut decoded, &mut outcomes);
    decoded
        .into_iter()
        .zip(outcomes)
        .map(|(bbframe, outcome)| {
            let mut result = DecoderResult::new(
                bbframe,
                outcome.iterations,
                outcome.converged,
                outcome.syndrome_check_passed,
            );
            result.queries = outcome.queries;
            result
        })
        .collect()
}

/// Asserts that one batch of `frames` through the batch decoder of `codec`
/// gives each frame its [`single_frame_result`] under each of [`routes`], and
/// returns those results. `codec` decodes with a min-sum rule.
pub fn assert_batch_decodes_as_single_frame(
    codec: &DvbT2Concat,
    frames: &[Vec<Llr>],
) -> Vec<DecoderResult> {
    let expected: Vec<DecoderResult> = frames
        .iter()
        .map(|llrs| single_frame_result(codec, llrs))
        .collect();
    for (selectors, backend, full_wave) in routes() {
        let mut decoder = decoder_on(backend, codec, selectors);
        let lanes = decoder.batch_granularity();
        assert_eq!(decoder.ldpc().batch_route(lanes), full_wave);
        let got = batch_results(&mut decoder, frames);
        for (f, (got, expected)) in got.iter().zip(&expected).enumerate() {
            assert_eq!(
                got,
                expected,
                "frame {f} under {:?} on {full_wave:?}",
                decoder.ldpc().selectors()
            );
        }
    }
    expected
}
