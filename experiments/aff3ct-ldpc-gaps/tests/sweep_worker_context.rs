//! The sweep passes the execution's worker count to a reused decoder.

use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use aff3ct_ldpc_gaps::bler_sweep::slice_errors;
use gf2_coding::traits::{BatchSoftDecoder, DecodeOutcome};
use gf2_coding::Llr;
use gf2_core::BitVec;

struct RecordingDecoder(Arc<Mutex<Vec<usize>>>);

impl BatchSoftDecoder for RecordingDecoder {
    fn frame_len(&self) -> usize {
        3
    }

    fn decoded_len(&self) -> usize {
        3
    }

    fn batch_granularity(&self) -> usize {
        1
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
        self.0.lock().unwrap().push(workers.get());
        for ((frame, word), outcome) in frames.iter().zip(decoded).zip(outcomes) {
            *word = BitVec::new();
            for llr in *frame {
                word.push_bit(llr.hard_decision());
            }
            *outcome = DecodeOutcome::default();
        }
    }
}

#[test]
fn sweep_passes_execution_workers_when_decoders_are_reused() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut decoder = RecordingDecoder(calls.clone());
    assert!(rayon::current_thread_index().is_none());
    let expected_errors = slice_errors(&mut decoder, 42, 1.0, 5);

    for width in [1, 2, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(width)
            .build()
            .unwrap();
        pool.install(|| assert_eq!(slice_errors(&mut decoder, 42, 1.0, 5), expected_errors));
    }
    assert_eq!(slice_errors(&mut decoder, 42, 1.0, 5), expected_errors);
    assert_eq!(*calls.lock().unwrap(), [1, 1, 2, 4, 1]);
}
