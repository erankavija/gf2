//! Frame error rate of the layered quantized decoder against the canonical
//! flooding decoder over BPSK AWGN, on random codewords. A frame error is a
//! wrong decision among the first k positions, the information columns of
//! both codes' standards and the window of the AFF3CT comparison harness.
//!
//! `GF2_LAYERED_FER_FRAMES` bounds the frames per point (default 200 000),
//! `GF2_LAYERED_FER_ERRORS` the errors both decoders reach before a point
//! stops early (default 100), and `GF2_LAYERED_FER_GRID`, a comma-separated
//! list of Eb/N0 values in dB, replaces a test's grid. A full curve takes
//! hours, beyond the slow tier, so the curve tests run only under
//! `GF2_BENCH=1`; the unit tests of the comparator run in the fast tier.

mod common;

use gf2_coding::channel::AwgnChannel;
use gf2_coding::ldpc::{
    DecoderAlgorithm, DecoderConfig, LayeredConfig, LayeredDecoder, LdpcCode, LdpcDecoder,
    LdpcEncoder, QuasiCyclicLdpc,
};
use gf2_coding::llr::Llr;
use gf2_coding::traits::BlockEncoder;
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use rand::{rngs::StdRng, SeedableRng};
use std::sync::atomic::{AtomicUsize, Ordering};

const TARGET_FER: f64 = 1e-3;
const FLOODING: DecoderAlgorithm = DecoderAlgorithm::NormalizedMinSum(0.75);
const FLOODING_CAP: usize = 50;
const LAYERED_CAP: usize = 25;
const MARGIN_DB: f64 = 0.1;
const CHUNK: usize = 64;
/// Chunks per round; the stopping rule reads counts only between rounds, so
/// every count is independent of the worker count and scheduling.
const ROUND: usize = 48;

fn setting(name: &str, fallback: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(fallback)
}

/// Frame errors of (flooding, layered) on the same frames, and frames run.
fn point(
    code: &LdpcCode,
    encoder: &LdpcEncoder,
    layered: LayeredConfig,
    ebn0_db: f64,
    seed: u64,
) -> (usize, usize, usize) {
    let max_frames = setting("GF2_LAYERED_FER_FRAMES", 200_000);
    let target_errors = setting("GF2_LAYERED_FER_ERRORS", 100);
    let k = encoder.k();
    let rate = k as f64 / code.n() as f64;
    let variance = 1.0 / (2.0 * rate * 10f64.powf(ebn0_db / 10.0));
    let errors = [AtomicUsize::new(0), AtomicUsize::new(0)];
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let mut frames = 0;
    while frames < max_frames
        && errors
            .iter()
            .any(|e| e.load(Ordering::Relaxed) < target_errors)
    {
        let first = frames / CHUNK;
        let round = ROUND.min((max_frames - frames).div_ceil(CHUNK));
        let next = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| {
                    let config = DecoderConfig::new(FLOODING, true);
                    let mut flooding = LdpcDecoder::with_config(code.clone(), config);
                    let mut layered = LayeredDecoder::new(code, layered).unwrap();
                    let channel = AwgnChannel::from_variance(variance);
                    let mut word = BitVec::new();
                    loop {
                        let chunk = next.fetch_add(1, Ordering::Relaxed);
                        if chunk >= round {
                            break;
                        }
                        let stream = ((first + chunk) as u64) << 20;
                        let mut rng = StdRng::seed_from_u64(seed ^ stream);
                        for _ in 0..CHUNK {
                            let message = BitVec::random(encoder.k(), &mut rng);
                            let codeword = encoder.encode(&message);
                            let llrs: Vec<Llr> = (0..codeword.len())
                                .map(|i| {
                                    let symbol = if codeword.get(i) { -1.0 } else { 1.0 };
                                    let y = channel.transmit(symbol, &mut rng);
                                    Llr::new((2.0 * y / variance) as f32)
                                })
                                .collect();
                            let wrong =
                                |word: &BitVec| (0..k).any(|i| word.get(i) != codeword.get(i));
                            flooding.decode_codeword_into(&llrs, FLOODING_CAP, &mut word);
                            if wrong(&word) {
                                errors[0].fetch_add(1, Ordering::Relaxed);
                            }
                            layered.decode_codeword_into(&llrs, LAYERED_CAP, &mut word);
                            if wrong(&word) {
                                errors[1].fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                });
            }
        });
        frames += round * CHUNK;
    }
    (
        errors[0].load(Ordering::Relaxed),
        errors[1].load(Ordering::Relaxed),
        frames,
    )
}

/// Bounds on the Eb/N0 where FER first falls below the target, from the first
/// pair of adjacent grid points at and below it: the log-linear interpolation
/// when both have errors, otherwise the pair itself. `None` unless grid points
/// bracket the crossing on both sides.
fn crossing(curve: &[(f64, f64)]) -> Option<(f64, f64)> {
    curve.windows(2).find_map(|pair| {
        let ((x0, f0), (x1, f1)) = (pair[0], pair[1]);
        (f0 >= TARGET_FER && f1 < TARGET_FER).then(|| {
            if f1 == 0.0 {
                return (x0, x1);
            }
            let (l0, l1, lt) = (f0.log10(), f1.log10(), TARGET_FER.log10());
            let x = x0 + (x1 - x0) * (l0 - lt) / (l0 - l1);
            (x, x)
        })
    })
}

/// The largest loss in dB of layered against flooding that the curves admit.
fn loss_bound(flooding: &[(f64, f64)], layered: &[(f64, f64)]) -> Result<f64, String> {
    let (flooding_low, _) =
        crossing(flooding).ok_or("grid points do not bracket the flooding crossing")?;
    let (_, layered_high) =
        crossing(layered).ok_or("grid points do not bracket the layered crossing")?;
    Ok(layered_high - flooding_low)
}

fn compare(name: &str, code: &LdpcCode, layered: LayeredConfig, grid: &[f64]) {
    let encoder = LdpcEncoder::new(code.clone());
    let grid: Vec<f64> = std::env::var("GF2_LAYERED_FER_GRID").map_or_else(
        |_| grid.to_vec(),
        |list| {
            list.split(',')
                .map(|v| v.trim().parse().expect("Eb/N0 in dB"))
                .collect()
        },
    );
    let (mut flooding_curve, mut layered_curve) = (Vec::new(), Vec::new());
    for (index, &ebn0) in grid.iter().enumerate() {
        let (flooding, layered_errors, frames) = point(code, &encoder, layered, ebn0, index as u64);
        let rate = |e: usize| e as f64 / frames as f64;
        eprintln!(
            "{name} Eb/N0 {ebn0:.2} dB: frames {frames}, flooding FER {:.3e}, layered FER {:.3e}",
            rate(flooding),
            rate(layered_errors)
        );
        flooding_curve.push((ebn0, rate(flooding)));
        layered_curve.push((ebn0, rate(layered_errors)));
        if rate(flooding) < TARGET_FER && rate(layered_errors) < TARGET_FER {
            break;
        }
    }
    let loss =
        loss_bound(&flooding_curve, &layered_curve).unwrap_or_else(|e| panic!("{name}: {e}"));
    eprintln!(
        "{name}: Eb/N0 at FER 1e-3, flooding {:?} dB, layered {:?} dB",
        crossing(&flooding_curve),
        crossing(&layered_curve)
    );
    assert!(
        loss <= MARGIN_DB,
        "{name}: layered loses up to {loss:.3} dB"
    );
}

#[test]
#[ignore = "sim: NR BG1 Z=384 mother code FER curves to 1e-3, flooding and layered"]
fn nr_bg1_z384_layered_is_within_a_tenth_of_a_db_of_flooding() {
    skip_unless_bench!(
        "nr_bg1_z384_layered_is_within_a_tenth_of_a_db_of_flooding",
        "FER curves to 1e-3 take hours"
    );
    let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(1, 384));
    let layered = LayeredConfig::nr_5g();
    let grid: Vec<f64> = (0..16).map(|i| 0.1 * f64::from(i)).collect();
    compare("nr bg1 z384", &code, layered, &grid);
}

#[test]
#[ignore = "sim: DVB-T2 normal R1/2 FER curves to 1e-3, flooding and layered"]
fn dvb_t2_normal_r12_layered_is_within_a_tenth_of_a_db_of_flooding() {
    skip_unless_bench!(
        "dvb_t2_normal_r12_layered_is_within_a_tenth_of_a_db_of_flooding",
        "FER curves to 1e-3 take hours"
    );
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
    let layered = LayeredConfig::dvb_t2();
    let grid: Vec<f64> = (0..21).map(|i| 0.8 + 0.1 * f64::from(i)).collect();
    compare("dvb-t2 normal 1/2", &code, layered, &grid);
}

#[test]
fn an_unbracketed_crossing_fails_the_comparison() {
    // Both decoders error-free at one high-SNR point.
    let error_free = [(1.5, 0.0)];
    assert_eq!(crossing(&error_free), None);
    assert!(loss_bound(&error_free, &error_free).is_err());
    // Below the target from the first point, and never reaching it.
    let below = [(1.0, 1e-4), (1.1, 0.0)];
    let above = [(1.0, 0.5), (1.1, 1e-2)];
    assert_eq!(crossing(&below), None);
    assert_eq!(crossing(&above), None);
    let bracketed = [(0.9, 0.1), (1.0, 1e-2), (1.1, 1e-4)];
    assert!(loss_bound(&below, &bracketed).is_err());
    assert!(loss_bound(&bracketed, &above).is_err());
}

#[test]
fn a_bracketed_crossing_interpolates_log_fer_or_bounds_by_the_pair() {
    let (low, high) = crossing(&[(0.9, 0.1), (1.0, 1e-2), (1.1, 1e-4)]).unwrap();
    assert_eq!(low, high);
    assert!((low - 1.05).abs() < 1e-12);
    let zero = [(0.9, 1e-2), (1.0, 0.0)];
    assert_eq!(crossing(&zero), Some((0.9, 1.0)));
    // A zero-error upper point bounds the loss by the full grid step.
    let estimate = [(0.9, 1e-2), (1.0, 1e-4)];
    let loss = loss_bound(&estimate, &zero).unwrap();
    assert!((loss - 0.05).abs() < 1e-12);
    let loss = loss_bound(&zero, &estimate).unwrap();
    assert!((loss - 0.05).abs() < 1e-12);
}
