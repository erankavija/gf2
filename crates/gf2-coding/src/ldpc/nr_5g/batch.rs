//! Rate-matched 5G NR implementer of [`BatchSoftDecoder`]: each frame is
//! prepared onto the mother code and a batch decodes through
//! [`LdpcBatchDecoder`].

use super::Nr5gRateMatchedCode;
use crate::ldpc::{BatchRoute, DecodedWord, DecoderAlgorithm, DecoderConfig, LdpcBatchDecoder};
use crate::llr::Llr;
use crate::traits::{BatchSoftDecoder, DecodeOutcome};
use crate::tuning::{self, DecodeSelectors};
use gf2_core::BitVec;
use gf2_kernels_simd::ldpc_lanes::LANES;
use std::num::NonZeroUsize;

/// Batch decoder of one rate-matched 5G NR LDPC code under one algorithm and
/// iteration cap.
///
/// A frame is the `target_n` LLRs of one transmitted word. Each frame is
/// prepared onto the mother code as [`Nr5gRateMatchedCode::prepare_llrs`]
/// prepares it, under the rate matching of `@/citation/ThreeGpp2017`: the
/// received LLR at a transmitted position, the zero LLR at a punctured
/// systematic or truncated parity position and
/// [`FILLER_LLR`](super::FILLER_LLR) at a filler position. The prepared frames
/// decode on the mother code in the waves of an [`LdpcBatchDecoder`], which
/// gives each wave its lane or single-frame route, and the decoded word of a
/// frame is its `target_k` message bits.
///
/// A frame's decoded word and outcome equal those of `decode_iterative` on a
/// [`Nr5gRateMatchedDecoder`](super::Nr5gRateMatchedDecoder) built
/// [`with_algorithm`](super::Nr5gRateMatchedDecoder::with_algorithm), on the
/// same LLRs and cap, for every batch size, position and route, under the
/// batch clauses of `docs/reference/ldpc-decoder-numerical-contract.md`.
///
/// The decoder owns the prepared LLRs and mother codewords of one wave beside
/// the workspace of the inner decoder. After a first decode, a decode into
/// words that already hold [`decoded_len`](BatchSoftDecoder::decoded_len)
/// bits of storage allocates nothing.
///
/// # Examples
///
/// ```
/// use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedBatchDecoder;
/// use gf2_coding::ldpc::{DecoderAlgorithm, QuasiCyclicLdpc};
/// use gf2_coding::llr::Llr;
/// use gf2_coding::traits::{BatchSoftDecoder, BlockEncoder, DecodeOutcome};
/// use gf2_core::BitVec;
///
/// let code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 49);
/// let messages: Vec<BitVec> = (0..11)
///     .map(|seed| BitVec::random_seeded(code.k(), seed))
///     .collect();
/// let frames: Vec<Vec<Llr>> = messages
///     .iter()
///     .map(|message| {
///         let sent = code.encode(message);
///         (0..sent.len())
///             .map(|i| Llr::new(if sent.get(i) { -4.0 } else { 4.0 }))
///             .collect()
///     })
///     .collect();
///
/// let mut decoder = Nr5gRateMatchedBatchDecoder::new(
///     code,
///     DecoderAlgorithm::NormalizedMinSum(0.75),
///     20,
/// );
/// assert_eq!((decoder.frame_len(), decoder.decoded_len()), (256, 49));
///
/// let views: Vec<&[Llr]> = frames.iter().map(Vec::as_slice).collect();
/// let mut decoded = vec![BitVec::new(); views.len()];
/// let mut outcomes = vec![DecodeOutcome::default(); views.len()];
/// decoder.decode_batch(&views, &mut decoded, &mut outcomes);
///
/// assert_eq!(decoded, messages);
/// assert!(outcomes.iter().all(|outcome| outcome.converged));
/// ```
pub struct Nr5gRateMatchedBatchDecoder {
    rm_code: Nr5gRateMatchedCode,
    inner: LdpcBatchDecoder,
    /// Mother-code LLRs of one wave, frame after frame.
    prepared: Vec<Llr>,
    /// Mother codewords of one wave.
    codewords: Vec<BitVec>,
}

impl Nr5gRateMatchedBatchDecoder {
    /// Builds the decoder with the decode selectors of the process-wide
    /// coding tuning section ([`tuning::active`]), read once here.
    ///
    /// # Panics
    ///
    /// As [`Self::with_selectors`], and as [`tuning::active`].
    #[must_use]
    pub fn new(
        rm_code: Nr5gRateMatchedCode,
        algorithm: DecoderAlgorithm,
        max_iterations: usize,
    ) -> Self {
        let selectors = tuning::active().section.decode().clone();
        Self::with_selectors(rm_code, algorithm, max_iterations, selectors)
    }

    /// Builds the decoder with the given decode selectors.
    ///
    /// The builder selects a route for equivalence tests and benchmarks; decoded
    /// results and per-frame outcomes are identical on every route
    /// (`docs/reference/ldpc-decoder-numerical-contract.md`).
    ///
    /// Every frame decodes under `algorithm` with early termination and the
    /// iteration cap `max_iterations`; a cap of zero behaves as in
    /// [`Nr5gRateMatchedDecoder`](super::Nr5gRateMatchedDecoder)'s
    /// `decode_iterative`. Construction allocates the inner decoder of the
    /// mother code and the buffers of one wave.
    ///
    /// # Panics
    ///
    /// Panics if `algorithm` carries a parameter [`DecoderConfig::new`]
    /// rejects, and as [`LdpcBatchDecoder::with_selectors`].
    #[must_use]
    pub fn with_selectors(
        rm_code: Nr5gRateMatchedCode,
        algorithm: DecoderAlgorithm,
        max_iterations: usize,
        selectors: DecodeSelectors,
    ) -> Self {
        let inner = LdpcBatchDecoder::with_selectors(
            rm_code.mother_code().clone(),
            DecoderConfig::new(algorithm, true),
            max_iterations,
            DecodedWord::Codeword,
            selectors,
        );
        let full_n = rm_code.params().full_n;
        Self {
            inner,
            prepared: vec![Llr::zero(); LANES * full_n],
            codewords: vec![BitVec::with_capacity(full_n); LANES],
            rm_code,
        }
    }

    /// The rate-matched code.
    #[must_use]
    pub fn code(&self) -> &Nr5gRateMatchedCode {
        &self.rm_code
    }

    /// The iteration cap of every frame.
    #[must_use]
    pub fn max_iterations(&self) -> usize {
        self.inner.max_iterations()
    }

    /// The decode selectors the routes are decided from.
    #[must_use]
    pub fn selectors(&self) -> &DecodeSelectors {
        self.inner.selectors()
    }

    /// Holds the lane route on the portable backend, as
    /// [`LdpcBatchDecoder::with_portable_lanes`] does.
    ///
    /// The builder selects a route for equivalence tests and benchmarks; decoded
    /// results and per-frame outcomes are identical on every route
    /// (`docs/reference/ldpc-decoder-numerical-contract.md`).
    #[must_use]
    pub fn with_portable_lanes(mut self) -> Self {
        self.inner = self.inner.with_portable_lanes();
        self
    }

    /// The route of a wave of `frames` frames on the mother code, as
    /// [`LdpcBatchDecoder::batch_route`] gives it.
    #[must_use]
    pub fn batch_route(&self, frames: usize) -> BatchRoute {
        self.inner.batch_route(frames)
    }

    /// The mother-code wave route under configured concurrent decode `workers`.
    #[must_use]
    pub fn batch_route_with_workers(&self, frames: usize, workers: NonZeroUsize) -> BatchRoute {
        self.inner.batch_route_with_workers(frames, workers)
    }
}

impl BatchSoftDecoder for Nr5gRateMatchedBatchDecoder {
    /// `target_n`, the transmitted bits of one frame.
    fn frame_len(&self) -> usize {
        self.rm_code.n()
    }

    /// `target_k`, the message bits of one frame.
    fn decoded_len(&self) -> usize {
        self.rm_code.k()
    }

    /// The granularity of [`LdpcBatchDecoder`].
    fn batch_granularity(&self) -> usize {
        self.inner.batch_granularity()
    }

    /// # Panics
    ///
    /// Panics as [`BatchSoftDecoder::decode_batch`] does, before any frame
    /// decodes.
    fn decode_batch(
        &mut self,
        frames: &[&[Llr]],
        decoded: &mut [BitVec],
        outcomes: &mut [DecodeOutcome],
    ) {
        self.decode_batch_with_workers(frames, decoded, outcomes, NonZeroUsize::MIN);
    }

    fn decode_batch_with_workers(
        &mut self,
        frames: &[&[Llr]],
        decoded: &mut [BitVec],
        outcomes: &mut [DecodeOutcome],
        workers: NonZeroUsize,
    ) {
        let n = self.frame_len();
        assert_eq!(
            decoded.len(),
            frames.len(),
            "a batch needs one decoded word per frame"
        );
        assert_eq!(
            outcomes.len(),
            frames.len(),
            "a batch needs one outcome per frame"
        );
        for (index, llrs) in frames.iter().enumerate() {
            assert_eq!(
                llrs.len(),
                n,
                "frame {index} has {} LLRs, the decoder takes {n}",
                llrs.len()
            );
        }

        // The inner decoder cuts a batch into the same waves, so each wave
        // takes the route the whole batch would give it.
        let full_n = self.rm_code.params().full_n;
        for (index, wave) in frames.chunks(LANES).enumerate() {
            let first = index * LANES;
            let slots = first..first + wave.len();
            for (llrs, full) in wave.iter().zip(self.prepared.chunks_exact_mut(full_n)) {
                self.rm_code.prepare_llrs_into(llrs, full);
            }
            let mut views: [&[Llr]; LANES] = [&[]; LANES];
            for (view, full) in views.iter_mut().zip(self.prepared.chunks_exact(full_n)) {
                *view = full;
            }
            let codewords = &mut self.codewords[..wave.len()];
            self.inner.decode_batch_with_workers(
                &views[..wave.len()],
                codewords,
                &mut outcomes[slots.clone()],
                workers,
            );
            for (word, codeword) in decoded[slots].iter_mut().zip(codewords.iter()) {
                self.rm_code.natural_order_message_into(codeword, word);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::AwgnChannel;
    use crate::ldpc::nr_5g::Nr5gRateMatchedDecoder;
    use crate::ldpc::{LaneBackend, QuasiCyclicLdpc};
    use crate::test_support::ldpc_batch_routes as routes;
    use crate::traits::conformance::{batch_soft_decoder_contract, ConvergenceMix};
    use crate::traits::{BlockEncoder, IterativeSoftDecoder};
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    const SEED: u64 = 0x2b61_9d0e;
    const CAP: usize = 20;

    /// Frames of the test set: two full waves and a final wave of one frame.
    const FRAMES: usize = 2 * LANES + 1;

    /// Noise deviations of consecutive frames relative to a code's operating
    /// deviation, from clean to undecodable, so that a wave holds frames of
    /// each kind.
    const SIGMA_LADDER: [f64; 6] = [0.5, 1.0, 2.2, 0.9, 1.7, 1.1];

    const MIN_SUM_FAMILY: [DecoderAlgorithm; 3] = [
        DecoderAlgorithm::MinSum,
        DecoderAlgorithm::NormalizedMinSum(0.75),
        DecoderAlgorithm::OffsetMinSum(0.5),
    ];

    /// A rate-matched code with its operating noise deviation.
    struct Case {
        code: Nr5gRateMatchedCode,
        sigma: f64,
    }

    /// Codes of both base graphs with punctured systematic columns, truncated
    /// parity and filler bits.
    fn cases() -> Vec<Case> {
        [(1, 600, 200, 0.9), (2, 256, 49, 1.0)]
            .into_iter()
            .map(|(base_graph, n, k, sigma)| {
                let code = QuasiCyclicLdpc::nr_5g_rate_matched(base_graph, n, k);
                let params = code.params();
                assert!(params.num_punctured_systematic > 0, "bg{base_graph}");
                assert!(params.num_punctured_parity > 0, "bg{base_graph}");
                assert!(params.num_shortened > 0, "bg{base_graph}");
                Case { code, sigma }
            })
            .collect()
    }

    /// [`FRAMES`] random messages of `case` and their transmitted words
    /// through AWGN, frame `f` at the deviation its entry of [`SIGMA_LADDER`]
    /// scales.
    fn noisy_frames(case: &Case) -> (Vec<BitVec>, Vec<Vec<Llr>>) {
        let mut rng = StdRng::seed_from_u64(SEED);
        (0..FRAMES)
            .map(|f| {
                let message = BitVec::random(case.code.k(), &mut rng);
                let sent = case.code.encode(&message);
                let sigma = case.sigma * SIGMA_LADDER[f % SIGMA_LADDER.len()];
                let channel = AwgnChannel::from_variance(sigma * sigma);
                let scale = 2.0 / channel.variance();
                let llrs = (0..sent.len())
                    .map(|i| {
                        let symbol = if sent.get(i) { -1.0 } else { 1.0 };
                        Llr::new((scale * channel.transmit(symbol, &mut rng)) as f32)
                    })
                    .collect();
                (message, llrs)
            })
            .unzip()
    }

    /// A decoder of `case` whose lane route runs `backend`, one of the
    /// backends of [`routes`].
    fn decoder_on(
        backend: LaneBackend,
        case: &Case,
        algorithm: DecoderAlgorithm,
        cap: usize,
        selectors: DecodeSelectors,
    ) -> Nr5gRateMatchedBatchDecoder {
        let decoder = Nr5gRateMatchedBatchDecoder::with_selectors(
            case.code.clone(),
            algorithm,
            cap,
            selectors,
        );
        match backend {
            LaneBackend::Portable => decoder.with_portable_lanes(),
            LaneBackend::Avx2 => decoder,
        }
    }

    /// Runs the shared contract for `algorithm` under `selectors` on
    /// `backend`, against the single-frame decoder of the same algorithm.
    fn assert_batch_contract(
        case: &Case,
        frames: &[Vec<Llr>],
        algorithm: DecoderAlgorithm,
        (selectors, backend, full_wave): (DecodeSelectors, LaneBackend, BatchRoute),
    ) {
        let mut decoder = decoder_on(backend, case, algorithm, CAP, selectors);
        assert_eq!(decoder.frame_len(), case.code.n());
        assert_eq!(decoder.decoded_len(), case.code.k());
        assert_eq!(decoder.batch_granularity(), LANES);
        assert_eq!(decoder.max_iterations(), CAP);
        assert_eq!(decoder.batch_route(LANES), full_wave);

        let mut single = Nr5gRateMatchedDecoder::with_algorithm(case.code.clone(), algorithm);
        batch_soft_decoder_contract(
            &mut decoder,
            |llrs| single.decode_iterative(llrs, CAP),
            frames,
            ConvergenceMix::EarlyLateAndNonConverging,
        );
    }

    fn assert_min_sum_batch_contract(case: &Case) {
        let (_, frames) = noisy_frames(case);
        for algorithm in MIN_SUM_FAMILY {
            for route in routes() {
                assert_batch_contract(case, &frames, algorithm, route);
            }
        }
    }

    #[test]
    fn base_graph_1_min_sum_batches_satisfy_the_batch_contract_on_each_route() {
        assert_min_sum_batch_contract(&cases()[0]);
    }

    #[test]
    fn base_graph_2_min_sum_batches_satisfy_the_batch_contract_on_each_route() {
        assert_min_sum_batch_contract(&cases()[1]);
    }

    #[test]
    fn sum_product_batches_satisfy_the_batch_contract_on_the_single_frame_route() {
        for case in cases() {
            let (_, frames) = noisy_frames(&case);
            let (lane_selectors, backend, _) = routes().swap_remove(0);
            assert_batch_contract(
                &case,
                &frames,
                DecoderAlgorithm::SumProduct,
                (lane_selectors, backend, BatchRoute::SingleFrame),
            );
        }
    }

    #[test]
    fn a_zero_cap_equals_the_single_frame_decoder() {
        for case in cases() {
            let (_, frames) = noisy_frames(&case);
            for (selectors, backend, _) in routes() {
                let algorithm = DecoderAlgorithm::NormalizedMinSum(0.75);
                let mut decoder = decoder_on(backend, &case, algorithm, 0, selectors);
                let mut single =
                    Nr5gRateMatchedDecoder::with_algorithm(case.code.clone(), algorithm);
                batch_soft_decoder_contract(
                    &mut decoder,
                    |llrs| single.decode_iterative(llrs, 0),
                    &frames,
                    ConvergenceMix::Any,
                );
            }
        }
    }

    #[test]
    fn clean_frames_decode_to_their_messages() {
        for case in cases() {
            let mut rng = StdRng::seed_from_u64(SEED);
            let messages: Vec<BitVec> = (0..FRAMES)
                .map(|_| BitVec::random(case.code.k(), &mut rng))
                .collect();
            let frames: Vec<Vec<Llr>> = messages
                .iter()
                .map(|message| {
                    let sent = case.code.encode(message);
                    (0..sent.len())
                        .map(|i| Llr::new(if sent.get(i) { -10.0 } else { 10.0 }))
                        .collect()
                })
                .collect();
            let views: Vec<&[Llr]> = frames.iter().map(Vec::as_slice).collect();
            for (selectors, backend, _) in routes() {
                let algorithm = DecoderAlgorithm::NormalizedMinSum(0.75);
                let mut decoder = decoder_on(backend, &case, algorithm, CAP, selectors);
                let mut decoded = vec![BitVec::new(); FRAMES];
                let mut outcomes = vec![DecodeOutcome::default(); FRAMES];
                decoder.decode_batch(&views, &mut decoded, &mut outcomes);
                assert_eq!(decoded, messages, "{:?}", decoder.selectors());
                assert!(outcomes.iter().all(|outcome| outcome.converged));
            }
        }
    }

    #[test]
    fn the_default_constructor_reads_the_active_coding_section() {
        let code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 49);
        let decoder =
            Nr5gRateMatchedBatchDecoder::new(code, DecoderAlgorithm::NormalizedMinSum(0.75), CAP);
        assert_eq!(decoder.selectors(), tuning::active().section.decode());
    }

    fn decode_with_shapes(frame_len: usize, words: usize, outcomes: usize) {
        let mut decoder = Nr5gRateMatchedBatchDecoder::with_selectors(
            QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 49),
            DecoderAlgorithm::MinSum,
            CAP,
            DecodeSelectors::CONSERVATIVE,
        );
        let frame = vec![Llr::zero(); frame_len];
        decoder.decode_batch(
            &[&frame, &frame],
            &mut vec![BitVec::new(); words],
            &mut vec![DecodeOutcome::default(); outcomes],
        );
    }

    #[test]
    #[should_panic(expected = "frame 0 has 255 LLRs, the decoder takes 256")]
    fn a_frame_of_another_length_panics() {
        decode_with_shapes(255, 2, 2);
    }

    #[test]
    #[should_panic(expected = "a batch needs one decoded word per frame")]
    fn a_missing_decoded_word_panics() {
        decode_with_shapes(256, 1, 2);
    }

    #[test]
    #[should_panic(expected = "a batch needs one outcome per frame")]
    fn a_missing_outcome_panics() {
        decode_with_shapes(256, 2, 3);
    }
}
