//! Frame error counts of the BLER sweep equal those of an independent
//! per-frame `decode_iterative` loop, for one worker and for the default pool.

use aff3ct_ldpc_gaps::bler_sweep::{run_point, NMS_SCALE, SLICE_FRAMES};
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig, LdpcDecoder, QuasiCyclicLdpc};
use gf2_coding::traits::IterativeSoftDecoder;
use gf2_coding::LdpcCode;
use gf2_sim::testutil::AwgnLlrSource;

const MAX_ITER: usize = 20;
const FRAMES: u64 = 3 * SLICE_FRAMES + 5;

fn reference_errors(code: &LdpcCode, point_seed: u64, sigma: f64) -> u64 {
    let cfg = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(NMS_SCALE), true);
    let mut dec = LdpcDecoder::with_config(code.clone(), cfg);
    let mut errors = 0;
    for slice_start in (0..FRAMES).step_by(SLICE_FRAMES as usize) {
        let slice_end = (slice_start + SLICE_FRAMES).min(FRAMES);
        let mut src = AwgnLlrSource::new(point_seed ^ slice_start.wrapping_mul(0x9E37));
        for _ in slice_start..slice_end {
            let llrs = src.frame_all_zero(code.n(), sigma);
            let res = dec.decode_iterative(&llrs, MAX_ITER);
            errors += u64::from((0..res.decoded_bits.len()).any(|i| res.decoded_bits.get(i)));
        }
    }
    errors
}

#[test]
fn sweep_errors_match_single_frame_path_across_workers() {
    let code = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121)
        .mother_code()
        .clone();
    let mut seen_errors = false;
    for (point, sigma) in [1.6, 1.2, 0.9].into_iter().enumerate() {
        let seed = 0xB1E5_u64 ^ (point as u64).wrapping_mul(0x1234_5678_9ABC_DEF1);
        let expected = reference_errors(&code, seed, sigma);
        seen_errors |= expected > 0;

        let one = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap();
        let (frames, errors) =
            one.install(|| run_point(&code, seed, sigma, FRAMES, u64::MAX, MAX_ITER));
        assert_eq!(
            (frames, errors),
            (FRAMES, expected),
            "one worker, point {point}"
        );

        let (frames, errors) = run_point(&code, seed, sigma, FRAMES, u64::MAX, MAX_ITER);
        assert_eq!(
            (frames, errors),
            (FRAMES, expected),
            "pool default, point {point}"
        );
    }
    assert!(seen_errors, "no point produced a frame error");
}
