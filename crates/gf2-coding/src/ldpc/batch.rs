//! LDPC implementer of [`BatchSoftDecoder`]: a batch decodes in waves of
//! [`LANES`] frames, each wave in the lanes of the
//! [`gf2_kernels_simd::ldpc_lanes`] kernels or frame by frame on
//! [`LdpcDecoder`], as [`decide_batch_route`] decides.

use super::route::detected_lanes;
use super::{
    decide_batch_route, BatchRoute, DecoderConfig, EdgeLayout, LaneBackend, LaneCapability,
    LdpcCode, LdpcDecoder,
};
use crate::llr::Llr;
use crate::traits::{BatchSoftDecoder, DecodeOutcome};
use crate::tuning::{self, DecodeSelectors};
use gf2_core::BitVec;
use gf2_kernels_simd::ldpc_lanes::{F32x8, LaneMask, LaneMinSumRule, LdpcLaneFns, LANES};
use std::num::NonZeroUsize;

/// The word an [`LdpcBatchDecoder`] writes for each frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodedWord {
    /// All `n` hard-decided codeword bits, the word of
    /// [`LdpcDecoder::decode_to_codeword`].
    Codeword,
    /// The `k` message bits, the word of [`LdpcDecoder`]'s
    /// [`decode_iterative`](crate::traits::IterativeSoftDecoder::decode_iterative).
    Message,
}

/// Batch decoder of one LDPC code under one configuration and iteration cap.
///
/// A batch is decoded in consecutive waves of [`LANES`] frames; the last wave
/// holds the remaining frames. [`decide_batch_route`] gives each wave its
/// route from the decoder's [`DecodeSelectors`], the algorithm, the number of
/// frames in the wave, the edge count of the code and configured decode
/// concurrency:
///
/// - [`BatchRoute::Lane`]: the frames occupy one lane each of the flooding
///   min-sum lane kernels. A frame stops as the single-frame decoder stops
///   it, and the wave runs until each of its frames has stopped or the cap is
///   reached. The route names the [`LaneBackend`] whose kernels run.
/// - [`BatchRoute::SingleFrame`]: each frame decodes on an owned
///   [`LdpcDecoder`]. [`DecoderAlgorithm::SumProduct`](super::DecoderAlgorithm::SumProduct)
///   takes this route for every wave.
///
/// The lane backend is [`LaneBackend::detected`], decided at construction:
/// the AVX2 kernels when `gf2-coding` is built with its `simd` cargo feature
/// and AVX2 is detected at run time, and the portable kernels otherwise.
/// [`with_portable_lanes`](Self::with_portable_lanes) holds a decoder on the
/// portable kernels.
///
/// On either route and backend a frame's decoded word, outcome and posterior
/// equal those of [`LdpcDecoder`] on the same LLRs, configuration and cap,
/// bit for bit, under the batch clauses of
/// `docs/reference/ldpc-decoder-numerical-contract.md`.
///
/// The decoder owns its lane workspace, sized at construction from the edge
/// layout ([`workspace_bytes`](Self::workspace_bytes)). After a first decode,
/// a decode into words that already hold
/// [`decoded_len`](BatchSoftDecoder::decoded_len) bits of storage allocates
/// nothing.
///
/// # Examples
///
/// Decoding a batch through the trait, with selectors that open the lane
/// route for every wave:
///
/// ```
/// use gf2_coding::ldpc::{
///     BatchRoute, DecodedWord, DecoderConfig, LdpcBatchDecoder, LdpcCode, LdpcEncoder,
///     QuasiCyclicLdpc,
/// };
/// use gf2_coding::llr::Llr;
/// use gf2_coding::traits::{BatchSoftDecoder, BlockEncoder, DecodeOutcome};
/// use gf2_coding::tuning::DecodeSelectors;
/// use gf2_core::BitVec;
///
/// let base = vec![
///     vec![0, 3, 7, 1, 0, -1],
///     vec![5, 0, 11, -1, 2, 9],
///     vec![9, 13, 0, 6, -1, 4],
/// ];
/// let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::new(base, 16));
/// let encoder = LdpcEncoder::new(code.clone());
///
/// // Eleven frames: one full wave and a final wave of three.
/// let codewords: Vec<BitVec> = (0..11)
///     .map(|seed| encoder.encode(&BitVec::random_seeded(encoder.k(), seed)))
///     .collect();
/// let frames: Vec<Vec<Llr>> = codewords
///     .iter()
///     .map(|codeword| {
///         (0..codeword.len())
///             .map(|i| Llr::new(if codeword.get(i) { -2.0 } else { 2.0 }))
///             .collect()
///     })
///     .collect();
///
/// let selectors = DecodeSelectors::try_new(1, usize::MAX, 1, 1).unwrap();
/// let decoder = LdpcBatchDecoder::with_selectors(
///     code,
///     DecoderConfig::default(),
///     20,
///     DecodedWord::Codeword,
///     selectors,
/// );
/// assert!(matches!(decoder.batch_route(frames.len()), BatchRoute::Lane(_)));
///
/// let mut decoder: Box<dyn BatchSoftDecoder> = Box::new(decoder);
/// let views: Vec<&[Llr]> = frames.iter().map(Vec::as_slice).collect();
/// let mut decoded = vec![BitVec::new(); views.len()];
/// let mut outcomes = vec![DecodeOutcome::default(); views.len()];
/// decoder.decode_batch(&views, &mut decoded, &mut outcomes);
///
/// assert_eq!(decoded, codewords);
/// assert!(outcomes.iter().all(|outcome| outcome.converged));
/// ```
pub struct LdpcBatchDecoder {
    single: LdpcDecoder,
    config: DecoderConfig,
    max_iterations: usize,
    selectors: DecodeSelectors,
    lane_backend: LaneBackend,
    lane_fns: LdpcLaneFns,
    /// Codeword position of each message bit; `None` when the decoded words
    /// are codewords.
    message_cols: Option<Vec<usize>>,
    /// Codeword of a single-frame decode whose message bits are extracted.
    codeword: BitVec,
    /// Channel LLRs and beliefs, by variable and then by lane.
    channel: Vec<F32x8>,
    beliefs: Vec<F32x8>,
    /// Messages of each direction, by canonical edge id and then by lane.
    var_to_check: Vec<F32x8>,
    check_to_var: Vec<F32x8>,
    /// Hard decisions by variable; bit `l` is the decision of lane `l`.
    hard: Vec<u8>,
}

impl LdpcBatchDecoder {
    /// Builds the decoder with the decode selectors of the process-wide
    /// coding tuning section ([`tuning::active`]), read once here.
    ///
    /// # Panics
    ///
    /// As [`Self::with_selectors`], and as [`tuning::active`].
    #[must_use]
    pub fn new(
        code: LdpcCode,
        config: DecoderConfig,
        max_iterations: usize,
        word: DecodedWord,
    ) -> Self {
        let selectors = tuning::active().section.decode().clone();
        Self::with_selectors(code, config, max_iterations, word, selectors)
    }

    /// Builds the decoder with the given decode selectors.
    ///
    /// The builder selects a route for equivalence tests and benchmarks; decoded
    /// results and per-frame outcomes are identical on every route
    /// (`docs/reference/ldpc-decoder-numerical-contract.md`).
    ///
    /// Every frame decodes under `config` with the iteration cap
    /// `max_iterations`; a cap of zero behaves as in
    /// [`LdpcDecoder::decode_codeword_into`]. Construction allocates the
    /// single-frame decoder and the lane workspace and, for
    /// [`DecodedWord::Message`], resolves the code's systematic columns.
    ///
    /// # Panics
    ///
    /// Panics for [`DecodedWord::Message`] if the parity-check matrix is rank
    /// deficient, and as [`EdgeLayout::from_parity_check`].
    #[must_use]
    pub fn with_selectors(
        code: LdpcCode,
        config: DecoderConfig,
        max_iterations: usize,
        word: DecodedWord,
        selectors: DecodeSelectors,
    ) -> Self {
        let message_cols = match word {
            DecodedWord::Codeword => None,
            DecodedWord::Message => Some(code.systematic_cols()),
        };
        let single = LdpcDecoder::with_config(code, config);
        let layout = single.edge_layout();
        let (n, edges) = (layout.n(), layout.edges());
        let (lane_backend, lane_fns) = detected_lanes();
        Self {
            config,
            max_iterations,
            selectors,
            lane_backend,
            lane_fns,
            message_cols,
            codeword: BitVec::with_capacity(n),
            channel: vec![F32x8::ZERO; n],
            beliefs: vec![F32x8::ZERO; n],
            var_to_check: vec![F32x8::ZERO; edges],
            check_to_var: vec![F32x8::ZERO; edges],
            hard: vec![0; n],
            single,
        }
    }

    /// Holds the lane route on [`LaneBackend::Portable`], whatever the build
    /// and the host provide.
    ///
    /// The builder selects a route for equivalence tests and benchmarks; decoded
    /// results and per-frame outcomes are identical on every route
    /// (`docs/reference/ldpc-decoder-numerical-contract.md`).
    #[must_use]
    pub fn with_portable_lanes(mut self) -> Self {
        self.lane_backend = LaneBackend::Portable;
        self.lane_fns = LdpcLaneFns::portable();
        self
    }

    /// The decode selectors the routes are decided from.
    #[must_use]
    pub fn selectors(&self) -> &DecodeSelectors {
        &self.selectors
    }

    /// The iteration cap of every frame.
    #[must_use]
    pub fn max_iterations(&self) -> usize {
        self.max_iterations
    }

    /// The route of a wave of `frames` frames at one configured worker, without decoding.
    ///
    /// A batch of at most [`LANES`] frames is one wave and takes this route.
    /// A longer batch takes `batch_route(LANES)`, the value returned for it,
    /// in each full wave, and `batch_route(r)` in a final wave of `r` frames.
    #[must_use]
    pub fn batch_route(&self, frames: usize) -> BatchRoute {
        self.batch_route_with_workers(frames, NonZeroUsize::MIN)
    }

    /// The wave route under configured concurrent decode `workers`.
    #[must_use]
    pub fn batch_route_with_workers(&self, frames: usize, workers: NonZeroUsize) -> BatchRoute {
        decide_batch_route(
            &self.selectors,
            self.config.algorithm(),
            LaneCapability::Present(self.lane_backend),
            frames.min(LANES),
            self.single.edge_layout().edges(),
            workers,
        )
    }

    /// Bytes of the lane workspace: both message directions per edge, channel
    /// LLRs and beliefs per variable, each of [`LANES`] `f32`, and one
    /// hard-decision byte per variable. The storage of the owned single-frame
    /// decoder is not counted.
    #[must_use]
    pub fn workspace_bytes(&self) -> usize {
        std::mem::size_of_val(self.channel.as_slice())
            + std::mem::size_of_val(self.beliefs.as_slice())
            + std::mem::size_of_val(self.var_to_check.as_slice())
            + std::mem::size_of_val(self.check_to_var.as_slice())
            + std::mem::size_of_val(self.hard.as_slice())
    }

    /// Decodes as [`BatchSoftDecoder::decode_batch`] and writes the posterior
    /// LLRs of `frames[i]` to `posteriors[i * n..(i + 1) * n]`, with `n` the
    /// frame length. Uses one configured worker.
    ///
    /// A posterior is the frame's beliefs at its own last iteration, the
    /// slice [`LdpcDecoder::posterior_llrs`] holds after decoding that frame;
    /// a frame that stops before its wave ends keeps them.
    ///
    /// # Panics
    ///
    /// Panics as [`BatchSoftDecoder::decode_batch`], and if
    /// `posteriors.len() != frames.len() * n`.
    pub fn decode_batch_with_posteriors(
        &mut self,
        frames: &[&[Llr]],
        decoded: &mut [BitVec],
        outcomes: &mut [DecodeOutcome],
        posteriors: &mut [Llr],
    ) {
        self.decode_batch_with_posteriors_and_workers(
            frames,
            decoded,
            outcomes,
            posteriors,
            NonZeroUsize::MIN,
        );
    }

    /// Decodes with posterior output and configured concurrent decode `workers`.
    ///
    /// # Panics
    ///
    /// Panics as [`Self::decode_batch_with_posteriors`] does.
    pub fn decode_batch_with_posteriors_and_workers(
        &mut self,
        frames: &[&[Llr]],
        decoded: &mut [BitVec],
        outcomes: &mut [DecodeOutcome],
        posteriors: &mut [Llr],
        workers: NonZeroUsize,
    ) {
        assert_eq!(
            posteriors.len(),
            frames.len() * self.frame_len(),
            "a batch needs one posterior LLR per frame position"
        );
        self.decode_waves(frames, decoded, outcomes, Some(posteriors), workers);
    }

    fn decode_waves(
        &mut self,
        frames: &[&[Llr]],
        decoded: &mut [BitVec],
        outcomes: &mut [DecodeOutcome],
        mut posteriors: Option<&mut [Llr]>,
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

        let rule = self.config.algorithm().min_sum_rule();
        for (index, wave) in frames.chunks(LANES).enumerate() {
            let first = index * LANES;
            let slots = first..first + wave.len();
            let decoded = &mut decoded[slots.clone()];
            let outcomes = &mut outcomes[slots.clone()];
            let posteriors = posteriors
                .as_deref_mut()
                .map(|all| &mut all[slots.start * n..slots.end * n]);
            match (self.batch_route_with_workers(wave.len(), workers), rule) {
                (BatchRoute::Lane(_), Some(rule)) => {
                    self.decode_wave_in_lanes(rule.into(), wave, decoded, outcomes, posteriors);
                }
                _ => self.decode_wave_per_frame(wave, decoded, outcomes, posteriors),
            }
        }
    }

    fn decode_wave_per_frame(
        &mut self,
        wave: &[&[Llr]],
        decoded: &mut [BitVec],
        outcomes: &mut [DecodeOutcome],
        mut posteriors: Option<&mut [Llr]>,
    ) {
        let n = self.frame_len();
        for (slot, &llrs) in wave.iter().enumerate() {
            let word = &mut decoded[slot];
            outcomes[slot] = match &self.message_cols {
                None => self
                    .single
                    .decode_codeword_into(llrs, self.max_iterations, word),
                Some(cols) => {
                    let outcome = self.single.decode_codeword_into(
                        llrs,
                        self.max_iterations,
                        &mut self.codeword,
                    );
                    word.clear();
                    for &col in cols {
                        word.push_bit(self.codeword.get(col));
                    }
                    outcome
                }
            };
            if let Some(posteriors) = posteriors.as_deref_mut() {
                posteriors[slot * n..(slot + 1) * n].copy_from_slice(self.single.posterior_llrs());
            }
        }
    }

    /// Decodes one wave with frame `l` in lane `l`; the lanes from
    /// `wave.len()` up are idle at zero LLRs.
    fn decode_wave_in_lanes(
        &mut self,
        rule: LaneMinSumRule,
        wave: &[&[Llr]],
        decoded: &mut [BitVec],
        outcomes: &mut [DecodeOutcome],
        mut posteriors: Option<&mut [Llr]>,
    ) {
        let layout = self.single.edge_layout();
        let n = layout.n();

        for (var, channel) in self.channel.iter_mut().enumerate() {
            let mut lanes = F32x8::ZERO;
            let mut decided = 0u8;
            for (lane, llrs) in wave.iter().enumerate() {
                lanes.0[lane] = llrs[var].value();
                decided |= u8::from(llrs[var].hard_decision()) << lane;
            }
            *channel = lanes;
            self.hard[var] = decided;
        }
        // The first iteration overwrites the belief of every occupied lane;
        // a zero cap and the idle lanes read the channel LLRs.
        self.beliefs.copy_from_slice(&self.channel);
        self.check_to_var.fill(F32x8::ZERO);
        for (message, &var) in self.var_to_check.iter_mut().zip(layout.check_edge_var()) {
            *message = self.channel[var as usize];
        }

        let occupied = u8::MAX >> (LANES - wave.len());
        let mut active = occupied;
        let mut iterations = [0usize; LANES];
        for iteration in 1..=self.max_iterations {
            if active == 0 {
                break;
            }
            (self.lane_fns.check_step_fn)(
                rule,
                layout.check_offsets(),
                &self.var_to_check,
                &mut self.check_to_var,
            );
            (self.lane_fns.variable_step_fn)(
                LaneMask::from_bits(active),
                layout.var_offsets(),
                layout.var_edge_to_check_edge(),
                &self.channel,
                &self.check_to_var,
                &mut self.beliefs,
                &mut self.var_to_check,
                &mut self.hard,
            );
            for (lane, count) in iterations.iter_mut().enumerate() {
                if active >> lane & 1 == 1 {
                    *count = iteration;
                }
            }
            if self.config.early_termination() {
                active = failing_lanes(layout, &self.hard, active);
            }
        }

        // A lane stops early only on a passing syndrome and holds its hard
        // decisions from then on, so the terminal test is also `converged`.
        let failing = failing_lanes(layout, &self.hard, occupied);
        for (lane, word) in decoded.iter_mut().enumerate() {
            word.clear();
            match &self.message_cols {
                None => {
                    for &decisions in &self.hard {
                        word.push_bit(decisions >> lane & 1 == 1);
                    }
                }
                Some(cols) => {
                    for &col in cols {
                        word.push_bit(self.hard[col] >> lane & 1 == 1);
                    }
                }
            }
            let passed = failing >> lane & 1 == 0;
            outcomes[lane] = DecodeOutcome {
                iterations: iterations[lane],
                converged: passed,
                syndrome_check_passed: passed,
                queries: None,
            };
            if let Some(posteriors) = posteriors.as_deref_mut() {
                let frame = &mut posteriors[lane * n..(lane + 1) * n];
                for (posterior, belief) in frame.iter_mut().zip(&self.beliefs) {
                    *posterior = Llr::new(belief.0[lane]);
                }
            }
        }
    }
}

/// The lanes of `lanes` whose hard decisions leave a check unsatisfied.
fn failing_lanes(layout: &EdgeLayout, hard: &[u8], lanes: u8) -> u8 {
    let edge_var = layout.check_edge_var();
    let mut failing = 0u8;
    for bounds in layout.check_offsets().windows(2) {
        let parity = edge_var[bounds[0] as usize..bounds[1] as usize]
            .iter()
            .fold(0u8, |parity, &var| parity ^ hard[var as usize]);
        failing |= parity & lanes;
        if failing == lanes {
            break;
        }
    }
    failing
}

impl BatchSoftDecoder for LdpcBatchDecoder {
    fn frame_len(&self) -> usize {
        self.single.edge_layout().n()
    }

    fn decoded_len(&self) -> usize {
        self.message_cols
            .as_ref()
            .map_or(self.frame_len(), Vec::len)
    }

    /// [`LANES`], the frames of one wave.
    fn batch_granularity(&self) -> usize {
        LANES
    }

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
        self.decode_waves(frames, decoded, outcomes, None, workers);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ldpc::{DecoderAlgorithm, LdpcEncoder};
    use crate::test_support::{
        ldpc_batch_code as code, ldpc_batch_frames as frames, ldpc_batch_routes as routes,
        ldpc_lane_backends, LDPC_BATCH_CAP as CAP, LDPC_BATCH_FRAMES as FRAMES,
    };
    use crate::traits::conformance::{batch_soft_decoder_contract, ConvergenceMix};
    use crate::traits::{BlockEncoder, DecoderResult, IterativeSoftDecoder};
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    const SEED: u64 = 0x5cb7_4f81;

    const MIN_SUM_FAMILY: [DecoderAlgorithm; 3] = [
        DecoderAlgorithm::MinSum,
        DecoderAlgorithm::NormalizedMinSum(0.75),
        DecoderAlgorithm::OffsetMinSum(0.5),
    ];

    /// [`FRAMES`] random messages and the noiseless LLRs of their codewords.
    fn noiseless_frames() -> (Vec<BitVec>, Vec<Vec<Llr>>) {
        let encoder = LdpcEncoder::new(code());
        let mut rng = StdRng::seed_from_u64(SEED);
        (0..FRAMES)
            .map(|_| {
                let message = BitVec::random(encoder.k(), &mut rng);
                let codeword = encoder.encode(&message);
                let llrs = (0..codeword.len())
                    .map(|i| Llr::new(if codeword.get(i) { -10.0 } else { 10.0 }))
                    .collect();
                (message, llrs)
            })
            .unzip()
    }

    /// Selectors that give every wave the lane route.
    fn lane_selectors() -> DecodeSelectors {
        DecodeSelectors::try_new(1, usize::MAX, 1, 1).unwrap()
    }

    /// Selectors that give a wave of fewer than four frames the single-frame
    /// route and the lane route otherwise.
    fn mixed_selectors() -> DecodeSelectors {
        DecodeSelectors::try_new(4, usize::MAX, 1, 1).unwrap()
    }

    /// A decoder whose lane route runs `backend`, one of
    /// [`ldpc_lane_backends`].
    fn batch_decoder_on(
        backend: LaneBackend,
        config: DecoderConfig,
        cap: usize,
        word: DecodedWord,
        selectors: DecodeSelectors,
    ) -> LdpcBatchDecoder {
        let decoder = LdpcBatchDecoder::with_selectors(code(), config, cap, word, selectors);
        match backend {
            LaneBackend::Portable => decoder.with_portable_lanes(),
            LaneBackend::Avx2 => decoder,
        }
    }

    /// A decoder on the backend of the build and host.
    fn batch_decoder(
        config: DecoderConfig,
        cap: usize,
        word: DecodedWord,
        selectors: DecodeSelectors,
    ) -> LdpcBatchDecoder {
        batch_decoder_on(LaneBackend::detected(), config, cap, word, selectors)
    }

    fn views(frames: &[Vec<Llr>]) -> Vec<&[Llr]> {
        frames.iter().map(Vec::as_slice).collect()
    }

    #[test]
    fn codeword_words_satisfy_the_batch_contract_on_each_route() {
        let frames = frames();
        for (selectors, backend, full_wave) in routes() {
            let config = DecoderConfig::default();
            let mut decoder =
                batch_decoder_on(backend, config, CAP, DecodedWord::Codeword, selectors);
            assert_eq!(decoder.batch_granularity(), LANES);
            assert_eq!(decoder.decoded_len(), code().n());
            assert_eq!(decoder.batch_route(LANES), full_wave);

            let mut single = LdpcDecoder::with_config(code(), config);
            batch_soft_decoder_contract(
                &mut decoder,
                |llrs| single.decode_to_codeword(llrs, CAP),
                &frames,
                ConvergenceMix::EarlyLateAndNonConverging,
            );
        }
    }

    #[test]
    fn message_words_satisfy_the_batch_contract_on_each_route() {
        let frames = frames();
        let (messages, clean) = noiseless_frames();
        for (selectors, backend, full_wave) in routes() {
            let config = DecoderConfig::default();
            let mut decoder =
                batch_decoder_on(backend, config, CAP, DecodedWord::Message, selectors);
            assert_eq!(decoder.decoded_len(), code().k());
            assert_eq!(decoder.batch_route(LANES), full_wave);

            let mut single = LdpcDecoder::with_config(code(), config);
            batch_soft_decoder_contract(
                &mut decoder,
                |llrs| single.decode_iterative(llrs, CAP),
                &frames,
                ConvergenceMix::EarlyLateAndNonConverging,
            );

            let mut decoded = vec![BitVec::new(); FRAMES];
            let mut outcomes = vec![DecodeOutcome::default(); FRAMES];
            decoder.decode_batch(&views(&clean), &mut decoded, &mut outcomes);
            assert_eq!(decoded, messages, "{:?}", decoder.selectors());
        }
    }

    /// The single-frame result and posterior bit patterns of each frame.
    fn single_frame_results(
        config: DecoderConfig,
        cap: usize,
        frames: &[Vec<Llr>],
    ) -> Vec<(DecoderResult, Vec<u32>)> {
        let mut single = LdpcDecoder::with_config(code(), config);
        frames
            .iter()
            .map(|llrs| {
                let result = single.decode_to_codeword(llrs, cap);
                let posterior = single.posterior_llrs();
                (
                    result,
                    posterior.iter().map(|p| p.value().to_bits()).collect(),
                )
            })
            .collect()
    }

    /// Asserts that one batch of `frames` through a decoder of `selectors`
    /// on `backend` equals the single-frame decoder per frame, posteriors
    /// included.
    fn assert_equals_single_frame(
        backend: LaneBackend,
        config: DecoderConfig,
        cap: usize,
        selectors: DecodeSelectors,
        frames: &[Vec<Llr>],
    ) {
        assert_equals_single_frame_with_workers(
            backend,
            config,
            cap,
            selectors,
            frames,
            NonZeroUsize::MIN,
        );
    }

    fn assert_equals_single_frame_with_workers(
        backend: LaneBackend,
        config: DecoderConfig,
        cap: usize,
        selectors: DecodeSelectors,
        frames: &[Vec<Llr>],
        workers: NonZeroUsize,
    ) {
        let n = code().n();
        let expected = single_frame_results(config, cap, frames);
        let mut decoder = batch_decoder_on(
            backend,
            config,
            cap,
            DecodedWord::Codeword,
            selectors.clone(),
        );
        let mut decoded = vec![BitVec::new(); frames.len()];
        let mut outcomes = vec![DecodeOutcome::default(); frames.len()];
        let mut posteriors = vec![Llr::new(f32::NAN); frames.len() * n];
        decoder.decode_batch_with_posteriors_and_workers(
            &views(frames),
            &mut decoded,
            &mut outcomes,
            &mut posteriors,
            workers,
        );

        for (f, (result, posterior)) in expected.iter().enumerate() {
            let context = format!("frame {f}, {config:?}, cap {cap}, {selectors:?}, {backend:?}");
            assert_eq!(decoded[f], result.decoded_bits, "{context}");
            assert_eq!(outcomes[f], result.outcome(), "{context}");
            let got: Vec<u32> = posteriors[f * n..(f + 1) * n]
                .iter()
                .map(|p| p.value().to_bits())
                .collect();
            assert_eq!(&got, posterior, "posterior of {context}");
        }
    }

    #[test]
    fn explicit_worker_context_preserves_posteriors_on_selected_and_forced_routes() {
        let frames = frames();
        for backend in ldpc_lane_backends() {
            for algorithm in MIN_SUM_FAMILY {
                let config = DecoderConfig::new(algorithm, true);
                for max_workers in [1, 24] {
                    let selectors =
                        DecodeSelectors::try_new(4, usize::MAX, max_workers, 1).unwrap();
                    for workers in [1, 6, 12, 24] {
                        let workers = NonZeroUsize::new(workers).unwrap();
                        let decoder = batch_decoder_on(
                            backend,
                            config,
                            CAP,
                            DecodedWord::Codeword,
                            selectors.clone(),
                        );
                        let expected = if workers.get() <= max_workers {
                            BatchRoute::Lane(backend)
                        } else {
                            BatchRoute::SingleFrame
                        };
                        assert_eq!(decoder.batch_route_with_workers(LANES, workers), expected);
                        assert_eq!(
                            decoder.batch_route_with_workers(1, workers),
                            BatchRoute::SingleFrame
                        );
                        assert_equals_single_frame_with_workers(
                            backend,
                            config,
                            CAP,
                            selectors.clone(),
                            &frames,
                            workers,
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn frames_equal_the_single_frame_decoder_to_the_posterior_bit() {
        let frames = frames();
        for algorithm in MIN_SUM_FAMILY {
            for early_termination in [true, false] {
                let config = DecoderConfig::new(algorithm, early_termination);
                for (selectors, backend, _) in routes() {
                    for cap in [0, 1, CAP] {
                        assert_equals_single_frame(
                            backend,
                            config,
                            cap,
                            selectors.clone(),
                            &frames,
                        );
                    }
                }
            }
        }
    }

    /// The posterior of a frame that stops early is read after its wave has
    /// run on: every full wave of the set holds a frame that converges before
    /// another frame of the wave stops.
    #[test]
    fn a_terminated_lane_keeps_its_posterior_while_its_wave_continues() {
        let frames = frames();
        for algorithm in MIN_SUM_FAMILY {
            let config = DecoderConfig::new(algorithm, true);
            let expected = single_frame_results(config, CAP, &frames);
            for wave in expected.chunks_exact(LANES) {
                let stops = || wave.iter().map(|(result, _)| result.iterations);
                assert!(
                    stops().min() < stops().max(),
                    "{algorithm:?}: a wave needs frames that stop at different iterations"
                );
            }
            for backend in ldpc_lane_backends() {
                assert_equals_single_frame(backend, config, CAP, lane_selectors(), &frames);
            }
        }
    }

    #[test]
    fn sum_product_takes_the_single_frame_route() {
        let frames = frames();
        for early_termination in [true, false] {
            let config = DecoderConfig::new(DecoderAlgorithm::SumProduct, early_termination);
            let decoder = batch_decoder(config, CAP, DecodedWord::Codeword, lane_selectors());
            for size in 0..=FRAMES {
                assert_eq!(decoder.batch_route(size), BatchRoute::SingleFrame);
            }
            let backend = LaneBackend::detected();
            assert_equals_single_frame(backend, config, CAP, lane_selectors(), &frames);
        }
    }

    /// A decoder takes the detected backend unless it is held on the
    /// portable one.
    #[test]
    fn the_lane_route_names_the_backend_it_runs() {
        let decoder = |selectors| {
            batch_decoder(
                DecoderConfig::default(),
                CAP,
                DecodedWord::Codeword,
                selectors,
            )
        };
        assert_eq!(
            decoder(lane_selectors()).batch_route(LANES),
            BatchRoute::Lane(LaneBackend::detected())
        );
        assert_eq!(
            decoder(lane_selectors())
                .with_portable_lanes()
                .batch_route(LANES),
            BatchRoute::Lane(LaneBackend::Portable)
        );
        assert_eq!(
            decoder(DecodeSelectors::CONSERVATIVE)
                .with_portable_lanes()
                .batch_route(LANES),
            BatchRoute::SingleFrame
        );
        #[cfg(not(feature = "simd"))]
        assert_eq!(LaneBackend::detected(), LaneBackend::Portable);
    }

    #[test]
    fn explicit_selectors_give_the_route_of_each_wave_size() {
        let edges = EdgeLayout::from_parity_check(code().parity_check_matrix()).edges();
        let decoder = |selectors| {
            batch_decoder(
                DecoderConfig::default(),
                CAP,
                DecodedWord::Codeword,
                selectors,
            )
        };

        let lane = BatchRoute::Lane(LaneBackend::detected());
        let by_frames = decoder(mixed_selectors());
        let min_frames = by_frames.selectors().lane_min_frames();
        for size in 0..=FRAMES {
            let expected = if size >= min_frames {
                lane
            } else {
                BatchRoute::SingleFrame
            };
            assert_eq!(by_frames.batch_route(size), expected, "{size} frames");
        }

        let at_bound = decoder(DecodeSelectors::try_new(1, edges, 1, 1).unwrap());
        let below_bound = decoder(DecodeSelectors::try_new(1, edges - 1, 1, 1).unwrap());
        for size in 1..=FRAMES {
            assert_eq!(at_bound.batch_route(size), lane);
            assert_eq!(below_bound.batch_route(size), BatchRoute::SingleFrame);
        }
    }

    #[test]
    fn the_default_constructor_reads_the_active_coding_section() {
        let active = tuning::active().section.decode();
        let decoder =
            LdpcBatchDecoder::new(code(), DecoderConfig::default(), CAP, DecodedWord::Codeword);
        assert_eq!(decoder.selectors(), active);

        let explicit = batch_decoder(
            DecoderConfig::default(),
            CAP,
            DecodedWord::Codeword,
            active.clone(),
        );
        for size in 0..=FRAMES {
            assert_eq!(decoder.batch_route(size), explicit.batch_route(size));
        }
    }

    #[test]
    fn workspace_bytes_follow_the_edge_layout() {
        let layout = EdgeLayout::from_parity_check(code().parity_check_matrix());
        let lane_vector = std::mem::size_of::<F32x8>();
        let decoder = batch_decoder(
            DecoderConfig::default(),
            CAP,
            DecodedWord::Codeword,
            lane_selectors(),
        );
        assert_eq!(
            decoder.workspace_bytes(),
            2 * layout.edges() * lane_vector + 2 * layout.n() * lane_vector + layout.n()
        );
    }

    fn decode_with_shapes(frame_len: usize, words: usize, outcomes: usize) {
        let mut decoder = batch_decoder(
            DecoderConfig::default(),
            CAP,
            DecodedWord::Codeword,
            lane_selectors(),
        );
        let frame = vec![Llr::zero(); frame_len];
        decoder.decode_batch(
            &[&frame, &frame],
            &mut vec![BitVec::new(); words],
            &mut vec![DecodeOutcome::default(); outcomes],
        );
    }

    #[test]
    #[should_panic(expected = "frame 0 has 95 LLRs, the decoder takes 96")]
    fn a_frame_of_another_length_panics() {
        decode_with_shapes(code().n() - 1, 2, 2);
    }

    #[test]
    #[should_panic(expected = "a batch needs one decoded word per frame")]
    fn a_missing_decoded_word_panics() {
        decode_with_shapes(code().n(), 1, 2);
    }

    #[test]
    #[should_panic(expected = "a batch needs one outcome per frame")]
    fn a_missing_outcome_panics() {
        decode_with_shapes(code().n(), 2, 3);
    }

    #[test]
    #[should_panic(expected = "a batch needs one posterior LLR per frame position")]
    fn a_short_posterior_buffer_panics() {
        let mut decoder = batch_decoder(
            DecoderConfig::default(),
            CAP,
            DecodedWord::Codeword,
            lane_selectors(),
        );
        let frame = vec![Llr::zero(); code().n()];
        decoder.decode_batch_with_posteriors(
            &[&frame],
            &mut [BitVec::new()],
            &mut [DecodeOutcome::default()],
            &mut vec![Llr::zero(); code().n() - 1],
        );
    }
}
