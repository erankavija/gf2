//! The batch soft-decoder contract over each soft decoder of the crate:
//! through the per-frame path every `SoftDecoder` has, and through
//! `IterativeBatchDecoder` for the iterative ones.

use gf2_coding::channel::AwgnChannel;
use gf2_coding::gldpc::{self, GldpcDecoder, QcGldpcCode};
use gf2_coding::grand::{OrbGrand, OrbGrandConfig};
use gf2_coding::ldpc::nr_5g::{self, Nr5gRateMatchedCode, Nr5gRateMatchedDecoder};
use gf2_coding::ldpc::{LdpcCode, LdpcDecoder, LdpcEncoder, QuasiCyclicLdpc};
use gf2_coding::linear::LinearBlockCode;
use gf2_coding::llr::Llr;
use gf2_coding::osd::{GeneratorMatrixOsdDecoder, OsdConfig};
use gf2_coding::traits::block::ParityCheckMatrixAccess;
use gf2_coding::traits::conformance::{batch_soft_decoder_contract, ConvergenceMix};
use gf2_coding::traits::{
    BatchSoftDecoder, BlockEncoder, DecodeOutcome, IterativeBatchDecoder, IterativeSoftDecoder,
    SoftDecoder,
};
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::SeedableRng;

const SEED: u64 = 0x2438_c16f;

/// `count` random codewords of `encoder`.
fn random_codewords(encoder: &impl BlockEncoder, count: usize) -> Vec<BitVec> {
    let mut rng = StdRng::seed_from_u64(SEED);
    (0..count)
        .map(|_| encoder.encode(&BitVec::random(encoder.k(), &mut rng)))
        .collect()
}

/// The LLRs of BPSK-modulated `codeword` observed through `noise`.
fn llrs(codeword: &BitVec, scale: f64, mut noise: impl FnMut() -> f64) -> Vec<Llr> {
    (0..codeword.len())
        .map(|i| {
            let symbol = if codeword.get(i) { -1.0 } else { 1.0 };
            Llr::new((scale * (symbol + noise())) as f32)
        })
        .collect()
}

fn noiseless_frame(codeword: &BitVec) -> Vec<Llr> {
    llrs(codeword, 1.0, || 0.0)
}

/// Each codeword observed through AWGN of the standard deviation at its
/// index in `sigmas`.
fn noisy_frames(codewords: &[BitVec], sigmas: &[f64]) -> Vec<Vec<Llr>> {
    let mut rng = StdRng::seed_from_u64(SEED);
    codewords
        .iter()
        .zip(sigmas)
        .map(|(codeword, &sigma)| {
            let channel = AwgnChannel::from_variance(sigma * sigma);
            llrs(codeword, 2.0 / channel.variance(), || {
                channel.transmit(0.0, &mut rng)
            })
        })
        .collect()
}

/// One noisy frame per entry of [`SIGMAS`], each of another codeword.
fn frame_set(encoder: &impl BlockEncoder) -> Vec<Vec<Llr>> {
    noisy_frames(&random_codewords(encoder, SIGMAS.len()), &SIGMAS)
}

/// Asserts the contract of the per-frame path of `decoder`, with `reference`
/// an equally built decoder.
fn assert_default_path<D: SoftDecoder>(mut decoder: D, reference: D, frames: &[Vec<Llr>]) {
    batch_soft_decoder_contract(
        &mut decoder,
        |llrs| reference.decode_soft_with_result(llrs),
        frames,
        ConvergenceMix::Any,
    );
}

/// Asserts the contract of the adapter over `decoder` at `cap`, with
/// `reference` an equally built decoder, on a frame set mixing early, late
/// and non-converging frames.
fn assert_adapter<D: IterativeSoftDecoder>(
    decoder: D,
    mut reference: D,
    cap: usize,
    frames: &[Vec<Llr>],
) {
    batch_soft_decoder_contract(
        &mut IterativeBatchDecoder::new(decoder, cap),
        |llrs| reference.decode_iterative(llrs, cap),
        frames,
        ConvergenceMix::EarlyLateAndNonConverging,
    );
}

/// Noise deviations from clean to undecodable, interleaved so that frames of
/// each kind are neighbours.
const SIGMAS: [f64; 12] = [
    0.45, 0.9, 1.6, 0.8, 0.5, 1.4, 0.85, 0.95, 0.4, 1.8, 0.75, 1.0,
];

const CAP: usize = 20;

/// A (96, 48) quasi-cyclic code of column weight three.
fn ldpc_code() -> LdpcCode {
    let base = vec![
        vec![0, 3, 7, 1, 0, -1],
        vec![5, 0, 11, -1, 2, 9],
        vec![9, 13, 0, 6, -1, 4],
    ];
    LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::new(base, 16))
}

fn gldpc_code() -> QcGldpcCode {
    QcGldpcCode::lentmaier(7, 4, 1)
}

fn nr_code() -> Nr5gRateMatchedCode {
    QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 49)
}

#[test]
fn osd_default_path_satisfies_the_batch_contract() {
    let code = LinearBlockCode::hamming(4);
    let decoder = || GeneratorMatrixOsdDecoder::new(code.clone(), OsdConfig::new(1));
    assert_default_path(decoder(), decoder(), &frame_set(&code));
}

#[test]
fn orbgrand_default_path_satisfies_the_batch_contract() {
    let code = LinearBlockCode::hamming(4);
    let decoder = || {
        let config = OrbGrandConfig {
            max_queries: 6,
            ..OrbGrandConfig::default()
        };
        OrbGrand::new(code.parity_check_matrix().unwrap(), config)
    };
    let frames = frame_set(&code);
    // The query budget separates found codewords from exhausted searches.
    let found = |llrs: &Vec<Llr>| decoder().decode_soft_with_result(llrs).converged;
    assert!(frames.iter().any(found) && !frames.iter().all(found));
    assert_default_path(decoder(), decoder(), &frames);
}

#[test]
fn ldpc_default_path_satisfies_the_batch_contract() {
    let frames = frame_set(&LdpcEncoder::new(ldpc_code()));
    let decoder = || LdpcDecoder::new(ldpc_code());
    assert_default_path(decoder(), decoder(), &frames);
}

#[test]
fn gldpc_default_path_satisfies_the_batch_contract() {
    let frames = frame_set(&gldpc_code());
    let decoder = || GldpcDecoder::new(gldpc_code());
    assert_default_path(decoder(), decoder(), &frames);
}

#[test]
fn rate_matched_nr_default_path_satisfies_the_batch_contract() {
    let code = nr_code();
    let frames = frame_set(&code);
    assert_default_path(code.clone(), code.clone(), &frames);
    let decoder = || Nr5gRateMatchedDecoder::new(code.clone());
    assert_default_path(decoder(), decoder(), &frames);
}

/// Asserts that the per-frame path of `decoder` reports the word and metadata
/// of `iterative` at `cap`, over the frames of `encoder` at one converging and
/// one capped noise deviation.
fn assert_default_outcome_is_the_iterative_result<D: BatchSoftDecoder, I: IterativeSoftDecoder>(
    encoder: &impl BlockEncoder,
    mut decoder: D,
    mut iterative: I,
    cap: usize,
) {
    let codewords = random_codewords(encoder, 2);
    let frames = noisy_frames(&codewords, &[0.5, 3.0]);
    let mut verdicts = Vec::new();
    for frame in &frames {
        let expected = iterative.decode_iterative(frame, cap);
        let (word, outcome) = decode_one(&mut decoder, frame);
        assert_eq!(word, expected.decoded_bits);
        assert_eq!(outcome.iterations, expected.iterations);
        assert_eq!(outcome.converged, expected.converged);
        assert_eq!(
            outcome.syndrome_check_passed,
            expected.syndrome_check_passed
        );
        if !expected.converged {
            assert_eq!(expected.iterations, cap);
        }
        verdicts.push(expected.converged);
    }
    assert_eq!(verdicts, [true, false]);
}

#[test]
fn rate_matched_nr_code_default_path_outcome_is_the_iterative_result() {
    let code = nr_code();
    assert_default_outcome_is_the_iterative_result(
        &code,
        code.clone(),
        Nr5gRateMatchedDecoder::new(code.clone()),
        nr_5g::SOFT_DECODE_MAX_ITERATIONS,
    );
}

#[test]
fn rate_matched_nr_decoder_default_path_outcome_is_the_iterative_result() {
    let code = nr_code();
    let decoder = || Nr5gRateMatchedDecoder::new(code.clone());
    assert_default_outcome_is_the_iterative_result(
        &code,
        decoder(),
        decoder(),
        nr_5g::SOFT_DECODE_MAX_ITERATIONS,
    );
}

#[test]
fn gldpc_default_path_outcome_is_the_iterative_result() {
    let decoder = || GldpcDecoder::new(gldpc_code());
    assert_default_outcome_is_the_iterative_result(
        &gldpc_code(),
        decoder(),
        decoder(),
        gldpc::SOFT_DECODE_MAX_ITERATIONS,
    );
}

/// Asserts that `SoftDecoder::decode_soft` of a decoder contracted to decode
/// agrees with `decode_soft_with_result` on every frame of `encoder`, and
/// returns the word decoded from the noiseless frame of the codeword for at
/// least one frame whose hard decision (the word at an iteration cap of zero)
/// is another word.
fn assert_decode_soft_decodes<D: SoftDecoder + IterativeSoftDecoder>(
    encoder: &impl BlockEncoder,
    decoder: impl Fn() -> D,
) {
    let codewords = random_codewords(encoder, SIGMAS.len());
    let mut corrected = 0;
    for (frame, codeword) in noisy_frames(&codewords, &SIGMAS).iter().zip(&codewords) {
        let word = decoder().decode_soft(frame);
        assert_eq!(word, decoder().decode_soft_with_result(frame).decoded_bits);
        let sent = decoder().decode_soft(&noiseless_frame(codeword));
        let hard = decoder().decode_iterative(frame, 0);
        if hard.decoded_bits != sent && word == sent {
            corrected += 1;
        }
    }
    assert!(corrected > 0);
}

#[test]
fn gldpc_decode_soft_corrects_a_frame_its_hard_decision_gets_wrong() {
    assert_decode_soft_decodes(&gldpc_code(), || GldpcDecoder::new(gldpc_code()));
}

#[test]
fn rate_matched_nr_decode_soft_corrects_a_frame_its_hard_decision_gets_wrong() {
    let code = nr_code();
    assert_decode_soft_decodes(&code, || Nr5gRateMatchedDecoder::new(code.clone()));
}

/// The per-frame batch path returns the sent message of a frame whose hard
/// decision at the systematic positions is another word.
#[test]
fn gldpc_default_path_corrects_a_frame_its_hard_decision_gets_wrong() {
    let code = gldpc_code();
    let positions = code.systematic_positions();
    let codewords = random_codewords(&code, SIGMAS.len());
    let corrected = noisy_frames(&codewords, &SIGMAS)
        .iter()
        .zip(&codewords)
        .filter(|(frame, codeword)| {
            let sent = hard_decisions(&noiseless_frame(codeword), positions);
            let (word, outcome) = decode_one(&mut GldpcDecoder::new(gldpc_code()), frame);
            hard_decisions(frame, positions) != sent
                && word == sent
                && outcome.converged
                && outcome.iterations > 0
        })
        .count();
    assert!(corrected > 0);
}

#[test]
fn ldpc_adapter_satisfies_the_batch_contract() {
    let frames = frame_set(&LdpcEncoder::new(ldpc_code()));
    let decoder = || LdpcDecoder::new(ldpc_code());
    assert_adapter(decoder(), decoder(), CAP, &frames);
}

#[test]
fn gldpc_adapter_satisfies_the_batch_contract() {
    let frames = frame_set(&gldpc_code());
    let decoder = || GldpcDecoder::new(gldpc_code());
    assert_adapter(decoder(), decoder(), CAP, &frames);
}

#[test]
fn rate_matched_nr_adapter_satisfies_the_batch_contract() {
    let code = nr_code();
    let frames = frame_set(&code);
    let decoder = || Nr5gRateMatchedDecoder::new(code.clone());
    assert_adapter(decoder(), decoder(), CAP, &frames);
}

fn decode_one<D: BatchSoftDecoder>(decoder: &mut D, frame: &[Llr]) -> (BitVec, DecodeOutcome) {
    let mut decoded = [BitVec::new()];
    let mut outcomes = [DecodeOutcome::default()];
    decoder.decode_batch(&[frame], &mut decoded, &mut outcomes);
    let [word] = decoded;
    (word, outcomes[0])
}

fn hard_decisions(frame: &[Llr], positions: &[usize]) -> BitVec {
    let mut bits = BitVec::with_capacity(positions.len());
    for &position in positions {
        bits.push_bit(frame[position].hard_decision());
    }
    bits
}

/// Asserts that the per-frame path of a decoder writes the hard decisions of
/// the LLRs at `positions` where the adapter iterates to another word. The
/// frame is the first whose word without iteration is wrong and which the
/// adapter decodes.
fn assert_default_path_is_the_hard_decision<D: IterativeSoftDecoder>(
    encoder: &impl BlockEncoder,
    decoder: impl Fn() -> D,
    positions: &[usize],
) {
    let adapter =
        |frame: &[Llr], cap| decode_one(&mut IterativeBatchDecoder::new(decoder(), cap), frame);
    let codewords = random_codewords(encoder, SIGMAS.len());
    let (frame, (uniterated, at_zero)) = noisy_frames(&codewords, &SIGMAS)
        .into_iter()
        .zip(&codewords)
        .find_map(|(frame, codeword)| {
            let (sent, _) = adapter(&noiseless_frame(codeword), CAP);
            let uniterated = adapter(&frame, 0);
            (uniterated.0 != sent && adapter(&frame, CAP).0 == sent).then_some((frame, uniterated))
        })
        .expect("a frame that needs decoding and is decodable");
    assert_eq!(at_zero.iterations, 0);

    let (word, _) = decode_one(&mut decoder(), &frame);
    assert_eq!(word, hard_decisions(&frame, positions));

    let (iterated, at_cap) = adapter(&frame, CAP);
    assert!(at_cap.converged && at_cap.iterations > 0);
    assert_ne!(iterated, uniterated);
}

#[test]
fn ldpc_default_path_is_the_hard_decision_and_the_adapter_iterates() {
    let code = ldpc_code();
    let positions: Vec<usize> = (0..code.n()).collect();
    assert_default_path_is_the_hard_decision(
        &LdpcEncoder::new(code.clone()),
        || LdpcDecoder::new(code.clone()),
        &positions,
    );
}

/// Asserts that the per-frame path reports no iteration and the parity test
/// of the hard decisions of every frame, over a set holding frames on both
/// sides of the test.
fn assert_default_outcome_is_the_parity_test<D: SoftDecoder>(
    encoder: &impl BlockEncoder,
    decoder: impl Fn() -> D,
    is_codeword: impl Fn(&BitVec) -> bool,
    n: usize,
) {
    let positions: Vec<usize> = (0..n).collect();
    let mut frames = frame_set(encoder);
    frames.push(noiseless_frame(&random_codewords(encoder, 1)[0]));
    let mut verdicts = Vec::new();
    for frame in &frames {
        let expected = is_codeword(&hard_decisions(frame, &positions));
        let (_, outcome) = decode_one(&mut decoder(), frame);
        assert_eq!(outcome.iterations, 0);
        assert_eq!(outcome.converged, expected);
        assert_eq!(outcome.syndrome_check_passed, expected);
        verdicts.push(expected);
    }
    assert!(verdicts.contains(&true) && verdicts.contains(&false));
}

#[test]
fn ldpc_default_path_outcome_is_the_parity_test_of_the_hard_decision() {
    let code = ldpc_code();
    assert_default_outcome_is_the_parity_test(
        &LdpcEncoder::new(code.clone()),
        || LdpcDecoder::new(code.clone()),
        |word| code.is_valid_codeword(word),
        code.n(),
    );
}
