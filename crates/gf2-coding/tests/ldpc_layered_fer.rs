//! Frame error rate of the layered quantized decoder against the canonical
//! flooding decoder over BPSK AWGN, on random codewords.
//!
//! `GF2_LAYERED_FER_FRAMES` bounds the frames per point (default 200 000),
//! `GF2_LAYERED_FER_ERRORS` the errors both decoders reach before a point
//! stops early (default 100), and `GF2_LAYERED_FER_GRID`, a comma-separated
//! list of Eb/N0 values in dB, replaces a test's grid.
use gf2_coding::channel::AwgnChannel;
use gf2_coding::ldpc::{
    DecoderAlgorithm, DecoderConfig, LayeredConfig, LayeredDecoder, LayeredRule, LdpcCode,
    LdpcDecoder, LdpcEncoder, QuasiCyclicLdpc,
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
    let rate = encoder.k() as f64 / code.n() as f64;
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
                            flooding.decode_codeword_into(&llrs, FLOODING_CAP, &mut word);
                            if word != codeword {
                                errors[0].fetch_add(1, Ordering::Relaxed);
                            }
                            layered.decode_codeword_into(&llrs, LAYERED_CAP, &mut word);
                            if word != codeword {
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

/// The Eb/N0 where log FER first falls below the target, linear between
/// grid points; the first grid point when the curve starts below it.
fn crossing(curve: &[(f64, f64)]) -> Option<f64> {
    if curve.first()?.1 < TARGET_FER {
        return Some(curve[0].0);
    }
    curve.windows(2).find_map(|pair| {
        let ((x0, f0), (x1, f1)) = (pair[0], pair[1]);
        (f0 >= TARGET_FER && f1 < TARGET_FER).then(|| {
            if f1 == 0.0 {
                return x1;
            }
            let (l0, l1, lt) = (f0.log10(), f1.log10(), TARGET_FER.log10());
            x0 + (x1 - x0) * (l0 - lt) / (l0 - l1)
        })
    })
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
    let flooding = crossing(&flooding_curve).expect("flooding curve reaches the target");
    let layered = crossing(&layered_curve).expect("layered curve reaches the target");
    eprintln!("{name}: Eb/N0 at FER 1e-3, flooding {flooding:.3} dB, layered {layered:.3} dB");
    assert!(
        layered - flooding <= MARGIN_DB,
        "{name}: layered loses {:.3} dB",
        layered - flooding
    );
}

#[test]
#[ignore = "sim: NR BG1 Z=384 mother code FER curves to 1e-3, flooding and layered"]
fn nr_bg1_z384_layered_is_within_a_tenth_of_a_db_of_flooding() {
    let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(1, 384));
    let layered = LayeredConfig::new(LayeredRule::Normalized { eighths: 6 }, 4.0, 31, 40, true);
    let grid: Vec<f64> = (0..16).map(|i| 0.1 * f64::from(i)).collect();
    compare("nr bg1 z384", &code, layered, &grid);
}

#[test]
#[ignore = "sim: DVB-T2 normal R1/2 FER curves to 1e-3, flooding and layered"]
fn dvb_t2_normal_r12_layered_is_within_a_tenth_of_a_db_of_flooding() {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
    let layered = LayeredConfig::new(LayeredRule::Normalized { eighths: 7 }, 4.0, 31, 40, true);
    let grid: Vec<f64> = (0..21).map(|i| 0.8 + 0.1 * f64::from(i)).collect();
    compare("dvb-t2 normal 1/2", &code, layered, &grid);
}
