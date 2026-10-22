//! Decode [`Stage`] over the `gf2-coding` [`BatchSoftDecoder`] trait:
//! [`BatchDecode`] hands a whole [`LlrBatch`] to one decoder held in its
//! [`BatchDecodeScratch`].

use std::num::NonZeroUsize;

use gf2_coding::traits::{BatchSoftDecoder, DecodeOutcome};
use gf2_coding::Llr;
use gf2_core::BitVec;

use crate::batch::{HardDecisionBatch, LlrBatch};
use crate::error::StageError;
use crate::stage::{ExecutionClass, Stage};

/// Per-stage scratch for [`BatchDecode`]: the decoder, built from the stage's
/// factory by the first `process` call that uses this scratch, and the
/// per-frame iteration counts of the most recent call.
///
/// The scratch type is the same for every decoder type, so a consumer of an
/// erased stage downcasts to it without naming the decoder.
#[derive(Default)]
pub struct BatchDecodeScratch {
    /// Per-frame [`DecodeOutcome::iterations`] of the most recent
    /// [`BatchDecode::process`] call, in input-frame order.
    pub iterations: Vec<u64>,
    decoder: Option<Box<dyn BatchSoftDecoder + Send + Sync>>,
    outcomes: Vec<DecodeOutcome>,
    /// Always empty between calls; holds the allocation of the frame views.
    frame_views: Vec<&'static [Llr]>,
}

/// Empties `views` and returns its allocation for views of any lifetime.
fn recycle(mut views: Vec<&[Llr]>) -> Vec<&'static [Llr]> {
    views.clear();
    // Collecting a mapped `vec::IntoIter` into a `Vec` of the same element
    // layout reuses the source allocation.
    views
        .into_iter()
        .map(|_| -> &'static [Llr] { &[] })
        .collect()
}

/// Soft-decode stage: frames of LLRs → decoded words, through any
/// [`BatchSoftDecoder`].
///
/// One `process` call passes every frame of the batch to
/// [`BatchSoftDecoder::decode_batch_with_workers`] in input order, without
/// copying LLRs. Decode concurrency is the active Rayon pool width, or one
/// when called outside a Rayon worker.
/// Each input frame is [`frame_len`](BatchSoftDecoder::frame_len) LLRs; each
/// output frame is the [`decoded_len`](BatchSoftDecoder::decoded_len)-bit word
/// the decoder writes. The per-frame iteration counts are recorded into the
/// [`BatchDecodeScratch`].
///
/// A frame that does not converge yields the decoder's estimate and no stage
/// error.
///
/// # Bounds
///
/// The factory is `Fn() -> D + Send + Sync + 'static`: the stage is shared
/// across executor threads and calls it once per scratch. The decoder is
/// `D: BatchSoftDecoder + Send + Sync + 'static`, as [`Stage::Scratch`]
/// requires of the scratch that owns it.
///
/// # Allocation
///
/// A call allocates the output batch and what the decoder allocates. The
/// scratch keeps its outcome, iteration-count and frame-view buffers across
/// calls.
///
/// # Panics
///
/// `process` panics as [`BatchSoftDecoder::decode_batch`] does on a frame
/// whose length differs from the decoder's `frame_len`.
pub struct BatchDecode<F> {
    factory: F,
}

impl<D, F> BatchDecode<F>
where
    D: BatchSoftDecoder + Send + Sync + 'static,
    F: Fn() -> D + Send + Sync + 'static,
{
    /// Builds a decode stage whose scratches each hold one decoder returned
    /// by `factory`.
    pub fn new(factory: F) -> Self {
        Self { factory }
    }
}

impl<D, F> Stage<LlrBatch, HardDecisionBatch> for BatchDecode<F>
where
    D: BatchSoftDecoder + Send + Sync + 'static,
    F: Fn() -> D + Send + Sync + 'static,
{
    type Scratch = BatchDecodeScratch;
    type CpuFallback = Self;

    fn process(
        &self,
        input: &LlrBatch,
        scratch: &mut BatchDecodeScratch,
    ) -> Result<HardDecisionBatch, StageError> {
        let decoder = scratch
            .decoder
            .get_or_insert_with(|| Box::new((self.factory)()));

        let mut views: Vec<&[Llr]> = std::mem::take(&mut scratch.frame_views);
        views.extend(input.frames.iter().map(Vec::as_slice));
        let mut decoded = vec![BitVec::new(); views.len()];
        scratch.outcomes.clear();
        scratch
            .outcomes
            .resize(views.len(), DecodeOutcome::default());

        let workers = NonZeroUsize::new(
            rayon::current_thread_index().map_or(1, |_| rayon::current_num_threads()),
        )
        .expect("Rayon pools have at least one worker");
        decoder.decode_batch_with_workers(&views, &mut decoded, &mut scratch.outcomes, workers);

        scratch.frame_views = recycle(views);
        scratch.iterations.clear();
        scratch.iterations.extend(
            scratch
                .outcomes
                .iter()
                .map(|outcome| outcome.iterations as u64),
        );
        Ok(HardDecisionBatch::new(decoded))
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder;
    use gf2_coding::ldpc::{DecoderAlgorithm, QuasiCyclicLdpc};
    use gf2_coding::traits::{
        DecoderResult, IterativeBatchDecoder, IterativeSoftDecoder, SoftDecoder,
    };

    use super::*;
    use crate::stage::erase;
    use crate::stages::nr_5g::Nr5gDecode;

    const K: usize = 3;
    const N: usize = 5;

    /// Decodes the hard decisions of the first `K` LLRs. Both paths need one
    /// iteration more than the frame has negative LLRs, so frames differ in
    /// decoded word and iteration count.
    #[derive(Clone)]
    struct MockDecoder;

    fn hard_bits(llrs: &[Llr]) -> BitVec {
        let mut bits = BitVec::new();
        for llr in &llrs[..K] {
            bits.push_bit(llr.hard_decision());
        }
        bits
    }

    fn needed_iterations(llrs: &[Llr]) -> usize {
        1 + llrs.iter().filter(|llr| llr.hard_decision()).count()
    }

    impl SoftDecoder for MockDecoder {
        fn k(&self) -> usize {
            K
        }

        fn n(&self) -> usize {
            N
        }

        fn decode_soft(&self, llrs: &[Llr]) -> BitVec {
            hard_bits(llrs)
        }

        fn decode_soft_with_result(&self, llrs: &[Llr]) -> DecoderResult {
            DecoderResult::new(hard_bits(llrs), needed_iterations(llrs), true, true)
        }
    }

    impl IterativeSoftDecoder for MockDecoder {
        fn decode_iterative(&mut self, llrs: &[Llr], max_iterations: usize) -> DecoderResult {
            let needed = needed_iterations(llrs);
            let converged = needed <= max_iterations;
            DecoderResult::new(
                hard_bits(llrs),
                needed.min(max_iterations),
                converged,
                converged,
            )
        }

        fn last_iteration_count(&self) -> usize {
            0
        }

        fn reset(&mut self) {}
    }

    /// Frame `f` has its first `f % (N + 1)` LLRs negative.
    fn frames(count: usize) -> Vec<Vec<Llr>> {
        (0..count)
            .map(|f| {
                (0..N)
                    .map(|i| Llr::new(if i < f % (N + 1) { -1.0 } else { 1.0 }))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn default_path_stage_matches_per_frame_decoding() {
        let stage = BatchDecode::new(|| MockDecoder);
        let above_granularity = BatchSoftDecoder::batch_granularity(&MockDecoder) + 3;
        let mut scratch = BatchDecodeScratch::default();

        for count in [0, 1, above_granularity] {
            let frames = frames(count);
            let out = stage
                .process(&LlrBatch::new(frames.clone()), &mut scratch)
                .unwrap();

            assert_eq!(out.frames.len(), count);
            assert_eq!(scratch.iterations.len(), count);
            for (f, frame) in frames.iter().enumerate() {
                let single = MockDecoder.decode_soft_with_result(frame);
                assert_eq!(
                    out.frames[f], single.decoded_bits,
                    "{count} frames, frame {f}"
                );
                assert_eq!(
                    scratch.iterations[f], single.iterations as u64,
                    "{count} frames, frame {f}"
                );
            }
        }
    }

    #[test]
    fn scratch_iterations_describe_the_most_recent_call() {
        let stage = BatchDecode::new(|| MockDecoder);
        let mut scratch = BatchDecodeScratch::default();
        let frames = frames(4);

        stage
            .process(&LlrBatch::new(frames.clone()), &mut scratch)
            .unwrap();
        let last = frames[3].clone();
        stage
            .process(&LlrBatch::new(vec![last.clone()]), &mut scratch)
            .unwrap();

        assert_eq!(scratch.iterations, [needed_iterations(&last) as u64]);
    }

    #[test]
    fn nonconverging_frame_yields_the_estimate_and_no_error() {
        let cap = 2;
        let stage = BatchDecode::new(move || IterativeBatchDecoder::new(MockDecoder, cap));
        let frames = frames(N + 1);
        let mut scratch = BatchDecodeScratch::default();

        let out = stage
            .process(&LlrBatch::new(frames.clone()), &mut scratch)
            .unwrap();

        let mut nonconverged = 0;
        for (f, frame) in frames.iter().enumerate() {
            let single = MockDecoder.decode_iterative(frame, cap);
            nonconverged += usize::from(!single.converged);
            assert_eq!(out.frames[f], single.decoded_bits, "frame {f}");
            assert_eq!(scratch.iterations[f], single.iterations as u64, "frame {f}");
        }
        assert!(nonconverged > 0 && nonconverged < frames.len());
    }

    #[test]
    fn decoder_is_built_once_per_scratch_on_first_use() {
        let built = Arc::new(AtomicUsize::new(0));
        let counter = built.clone();
        let stage = BatchDecode::new(move || {
            counter.fetch_add(1, Ordering::Relaxed);
            MockDecoder
        });
        let batch = LlrBatch::new(frames(4));

        let mut first = BatchDecodeScratch::default();
        assert_eq!(built.load(Ordering::Relaxed), 0);
        stage.process(&batch, &mut first).unwrap();
        stage.process(&batch, &mut first).unwrap();
        assert_eq!(built.load(Ordering::Relaxed), 1);

        let mut second = BatchDecodeScratch::default();
        stage.process(&batch, &mut second).unwrap();
        assert_eq!(built.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn scratch_keeps_its_frame_view_allocation() {
        let stage = BatchDecode::new(|| MockDecoder);
        let mut scratch = BatchDecodeScratch::default();
        let count = 9;

        stage
            .process(&LlrBatch::new(frames(count)), &mut scratch)
            .unwrap();

        assert!(scratch.frame_views.is_empty());
        assert!(scratch.frame_views.capacity() >= count);
    }

    #[test]
    fn stage_erases_to_llr_in_hard_decisions_out() {
        use std::any::TypeId;

        let erased = erase(BatchDecode::new(|| MockDecoder));
        assert_eq!(erased.input_type(), TypeId::of::<LlrBatch>());
        assert_eq!(erased.output_type(), TypeId::of::<HardDecisionBatch>());
    }

    #[test]
    fn iterative_nr_decoder_matches_the_nr_decode_stage() {
        let code = Arc::new(QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121));
        let algorithm = DecoderAlgorithm::NormalizedMinSum(0.75);
        let cap = 3;
        // Frame `f` has every `(f + 2)`-th LLR weakly negative on an all-zero
        // codeword, so the frames differ in how hard they are to decode.
        let frames: Vec<Vec<Llr>> = (0..4)
            .map(|f| {
                (0..code.n())
                    .map(|i| Llr::new(if i % (f + 2) == 0 { -1.5 } else { 2.0 }))
                    .collect()
            })
            .collect();
        let batch = LlrBatch::new(frames);

        let reference = Nr5gDecode::with_algorithm(code.clone(), algorithm, cap);
        let mut reference_scratch = BatchDecodeScratch::default();
        let expected = reference.process(&batch, &mut reference_scratch).unwrap();

        let stage = BatchDecode::new(move || {
            IterativeBatchDecoder::new(
                Nr5gRateMatchedDecoder::with_algorithm((*code).clone(), algorithm),
                cap,
            )
        });
        let mut scratch = BatchDecodeScratch::default();
        let out = stage.process(&batch, &mut scratch).unwrap();

        assert_eq!(out.frames, expected.frames);
        assert_eq!(scratch.iterations, reference_scratch.iterations);
    }
}
