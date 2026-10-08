//! Slice and point logic of the LDPC BLER sweep: seeded all-zero-codeword
//! frames from [`AwgnLlrSource`], decoded per slice through one
//! [`BatchSoftDecoder`]. Compiled under `cfg(test)` or the `test-support`
//! feature with [`testutil`](crate::testutil).

use gf2_core::BitVec;
use rayon::prelude::*;

use gf2_coding::ldpc::{DecodedWord, DecoderAlgorithm, DecoderConfig, LdpcBatchDecoder};
use gf2_coding::traits::{BatchSoftDecoder, DecodeOutcome};
use gf2_coding::{LdpcCode, Llr};

use crate::testutil::AwgnLlrSource;

/// Normalization factor of the min-sum decoder of the sweep.
pub const NMS_SCALE: f32 = 0.75;

/// Frames per slice; small slices bound the overshoot past the error target
/// to one wave.
pub const SLICE_FRAMES: u64 = 32;

/// Returns the batch decoder of the sweep: normalized min-sum with early
/// termination, decoding message words, with the active decode selectors.
///
/// # Panics
///
/// Panics if the parity-check matrix of `code` is rank deficient.
#[must_use]
pub fn sweep_decoder(code: LdpcCode, max_iter: usize) -> Box<dyn BatchSoftDecoder> {
    let cfg = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(NMS_SCALE), true);
    Box::new(LdpcBatchDecoder::new(
        code,
        cfg,
        max_iter,
        DecodedWord::Message,
    ))
}

/// Counts the frame errors among `frames` all-zero-codeword frames drawn from
/// `AwgnLlrSource::new(seed)` at noise std `sigma` and decoded as one batch by
/// `dec`. A frame error is a nonzero bit in the decoded message.
///
/// # Panics
///
/// Panics if `frames` does not fit in `usize` or `dec` rejects frames of
/// `frame_len` LLRs.
pub fn slice_errors(dec: &mut dyn BatchSoftDecoder, seed: u64, sigma: f64, frames: u64) -> u64 {
    let n = dec.frame_len();
    let mut src = AwgnLlrSource::new(seed);
    let llrs: Vec<Vec<Llr>> = (0..frames).map(|_| src.frame_all_zero(n, sigma)).collect();
    let views: Vec<&[Llr]> = llrs.iter().map(Vec::as_slice).collect();
    let mut decoded = vec![BitVec::new(); views.len()];
    let mut outcomes = vec![DecodeOutcome::default(); views.len()];
    dec.decode_batch(&views, &mut decoded, &mut outcomes);
    decoded
        .iter()
        .filter(|word| (0..word.len()).any(|i| word.get(i)))
        .count() as u64
}

/// Decodes one Es/N0 point until `target_errors` frame errors or `max_frames`
/// frames, whichever first, and returns `(frames, errors)`.
///
/// Each [`SLICE_FRAMES`]-frame slice owns an [`AwgnLlrSource`] seeded from
/// `point_seed` and the slice's first frame index, and one [`sweep_decoder`].
/// Slices run on the rayon pool; the error budget is checked between waves of
/// one slice per worker, so the counts for a fixed seed do not depend on the
/// worker count unless the error target stops the point early.
pub fn run_point(
    code: &LdpcCode,
    point_seed: u64,
    sigma: f64,
    max_frames: u64,
    target_errors: u64,
    max_iter: usize,
) -> (u64, u64) {
    let workers = rayon::current_num_threads().max(1) as u64;
    let wave = SLICE_FRAMES * workers;
    let mut frames_done: u64 = 0;
    let mut errors: u64 = 0;

    while frames_done < max_frames && errors < target_errors {
        let wave_end = (frames_done + wave).min(max_frames);
        let slice_starts: Vec<u64> = (frames_done..wave_end)
            .step_by(SLICE_FRAMES as usize)
            .collect();
        errors += slice_starts
            .par_iter()
            .map(|&slice_start| {
                let slice_end = (slice_start + SLICE_FRAMES).min(wave_end);
                let mut dec = sweep_decoder(code.clone(), max_iter);
                let seed = point_seed ^ slice_start.wrapping_mul(0x9E37);
                slice_errors(&mut *dec, seed, sigma, slice_end - slice_start)
            })
            .sum::<u64>();
        frames_done = wave_end;
    }
    (frames_done, errors)
}
