//! Frame error rates and per-frame failure diagnostics of gf2's canonical
//! flooding decoder and layered i8 decoder on DVB-T2 normal rate 1/2 over
//! BPSK AWGN, for the AFF3CT comparison of @/issue/d5cd53e0.
//!
//! Frames are those of `crates/gf2-coding/tests/ldpc_layered_fer.rs`: random
//! messages, `LdpcEncoder` codewords, bit 0 sent as +1, noise variance
//! `1 / (2 R Eb/N0)` with `R = k / n`, channel LLRs `2y/σ²`, and chunk `c` of
//! 64 frames drawn from `StdRng::seed_from_u64(seed ^ (c << 20))`. A frame
//! error is a wrong decision among the first k positions.
//!
//! ```text
//! dvb_t2_fer_diag --ebn0 1.5,2.0 --frames 20000 --seed 0 --workers 4 \
//!     --decoder flooding:0.75:50 --decoder layered:25 [--failures layered:25]
//! ```
//!
//! A decoder is `flooding:<alpha>:<cap>` (normalized min-sum),
//! `flooding-spa:<cap>`, `layered:<cap>` (`LayeredConfig::dvb_t2`), or
//! `layered:<cap>:<eighths>:<scale>:<channel limit>:<message limit>`
//! (normalized min-sum with the cap guard and early termination). Each
//! point prints one line per decoder: frames, frame errors, frames whose
//! terminal word passes the syndrome but is wrong, and mean iterations.
//! `--failures <decoder>` also prints one line per frame that decoder loses,
//! with its error weights, the column degrees of the wrong bits, the terminal
//! syndrome weight and, for the layered decoder, the frame scale, saturated
//! channel and belief counts, and the outcome of re-decoding the frame under
//! the alternatives of [`RECHECKS`].

use std::sync::Mutex;

use gf2_coding::channel::AwgnChannel;
use gf2_coding::ldpc::{
    DecoderAlgorithm, DecoderConfig, LayeredConfig, LayeredDecoder, LayeredRule, LdpcCode,
    LdpcDecoder, LdpcEncoder,
};
use gf2_coding::llr::Llr;
use gf2_coding::traits::BlockEncoder;
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use rand::{rngs::StdRng, SeedableRng};

const CHUNK: usize = 64;

#[derive(Clone, Copy, Debug)]
enum Spec {
    Flooding(f32, usize),
    FloodingSpa(usize),
    Layered(LayeredConfig, usize),
}

impl Spec {
    fn parse(text: &str) -> Self {
        let parts: Vec<&str> = text.split(':').collect();
        fn parsed<T: std::str::FromStr>(text: &str, field: &str) -> T {
            field
                .parse()
                .unwrap_or_else(|_| die(&format!("bad decoder {text}")))
        }
        let number = |field: &str| parsed::<usize>(text, field);
        match parts.as_slice() {
            ["flooding", alpha, cap] => Self::Flooding(parsed(text, alpha), number(cap)),
            ["flooding-spa", cap] => Self::FloodingSpa(number(cap)),
            ["layered", cap] => Self::Layered(LayeredConfig::dvb_t2(), number(cap)),
            ["layered", cap, eighths, scale, channel_limit, message_limit] => Self::Layered(
                LayeredConfig::new(
                    LayeredRule::Normalized {
                        eighths: parsed(text, eighths),
                    },
                    parsed(text, scale),
                    parsed(text, channel_limit),
                    parsed(text, message_limit),
                    true,
                ),
                number(cap),
            ),
            _ => die(&format!("bad decoder {text}")),
        }
    }

    fn label(&self) -> String {
        match self {
            Self::Flooding(alpha, cap) => format!("flooding-nms{alpha}-cap{cap}"),
            Self::FloodingSpa(cap) => format!("flooding-spa-cap{cap}"),
            Self::Layered(config, cap) => format!(
                "layered-{:?}-s{}-c{}-m{}{}-cap{cap}",
                config.rule(),
                config.channel_scale(),
                config.channel_limit(),
                config.message_limit(),
                if config.cap_guard() { "" } else { "-fixed" }
            ),
        }
    }
}

/// Alternatives each lost frame is re-decoded under by `--failures`.
const RECHECKS: &[&str] = &[
    "layered-cap200",
    "layered-nms6",
    "layered-scale2",
    "layered-scale2-cap200",
    "layered-msg60",
    "flooding-nms0.875-cap50",
    "flooding-nms0.875-cap200",
    "flooding-spa-cap50",
];

fn recheck_spec(name: &str) -> Spec {
    let dvb = LayeredConfig::dvb_t2();
    let seven = LayeredRule::Normalized { eighths: 7 };
    match name {
        "layered-cap200" => Spec::Layered(dvb, 200),
        "layered-nms6" => Spec::Layered(
            LayeredConfig::new(LayeredRule::Normalized { eighths: 6 }, 4.0, 31, 40, true),
            25,
        ),
        "layered-scale2" => Spec::Layered(LayeredConfig::new(seven, 2.0, 31, 40, true), 25),
        "layered-scale2-cap200" => Spec::Layered(LayeredConfig::new(seven, 2.0, 31, 40, true), 200),
        "layered-msg60" => Spec::Layered(LayeredConfig::new(seven, 4.0, 31, 60, true), 25),
        "flooding-nms0.875-cap50" => Spec::Flooding(0.875, 50),
        "flooding-nms0.875-cap200" => Spec::Flooding(0.875, 200),
        "flooding-spa-cap50" => Spec::FloodingSpa(50),
        _ => unreachable!("recheck names are fixed"),
    }
}

enum Decoder {
    Flooding(LdpcDecoder, usize),
    Layered(LayeredDecoder, usize),
}

/// The terminal state of one decode.
struct Decoded {
    word: BitVec,
    syndrome_passed: bool,
    iterations: usize,
}

impl Decoder {
    fn new(code: &LdpcCode, spec: Spec) -> Self {
        match spec {
            Spec::Flooding(alpha, cap) => {
                let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(alpha), true);
                Self::Flooding(LdpcDecoder::with_config(code.clone(), config), cap)
            }
            Spec::FloodingSpa(cap) => {
                let config = DecoderConfig::new(DecoderAlgorithm::SumProduct, true);
                Self::Flooding(LdpcDecoder::with_config(code.clone(), config), cap)
            }
            Spec::Layered(config, cap) => Self::Layered(
                LayeredDecoder::new(code, config).expect("DVB-T2 retains QC structure"),
                cap,
            ),
        }
    }

    fn decode(&mut self, llrs: &[Llr]) -> Decoded {
        let mut word = BitVec::new();
        let outcome = match self {
            Self::Flooding(decoder, cap) => decoder.decode_codeword_into(llrs, *cap, &mut word),
            Self::Layered(decoder, cap) => decoder.decode_codeword_into(llrs, *cap, &mut word),
        };
        Decoded {
            word,
            syndrome_passed: outcome.syndrome_check_passed,
            iterations: outcome.iterations,
        }
    }
}

#[derive(Default, Clone, Copy)]
struct Tally {
    errors: usize,
    undetected: usize,
    iterations: usize,
}

struct Args {
    ebn0: Vec<f64>,
    frames: usize,
    seed: u64,
    workers: usize,
    decoders: Vec<Spec>,
    failures: Option<Spec>,
}

fn die(message: &str) -> ! {
    eprintln!("dvb_t2_fer_diag: {message}");
    std::process::exit(2)
}

fn parse_args() -> Args {
    let mut args = Args {
        ebn0: Vec::new(),
        frames: 20_000,
        seed: 0,
        workers: 4,
        decoders: Vec::new(),
        failures: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || {
            it.next()
                .unwrap_or_else(|| die(&format!("{flag} needs a value")))
        };
        match flag.as_str() {
            "--ebn0" => {
                args.ebn0 = value()
                    .split(',')
                    .map(|v| v.trim().parse().unwrap_or_else(|_| die("bad --ebn0")))
                    .collect();
            }
            "--frames" => args.frames = value().parse().unwrap_or_else(|_| die("bad --frames")),
            "--seed" => args.seed = value().parse().unwrap_or_else(|_| die("bad --seed")),
            "--workers" => args.workers = value().parse().unwrap_or_else(|_| die("bad --workers")),
            "--decoder" => args.decoders.push(Spec::parse(&value())),
            "--failures" => args.failures = Some(Spec::parse(&value())),
            other => die(&format!("unknown argument {other}")),
        }
    }
    if args.ebn0.is_empty() || args.decoders.is_empty() {
        die("--ebn0 and at least one --decoder are required");
    }
    args
}

/// Column degree of every variable.
fn column_degrees(code: &LdpcCode) -> Vec<usize> {
    let decoder = LdpcDecoder::new(code.clone());
    let layout = decoder.edge_layout();
    (0..code.n()).map(|v| layout.var_range(v).len()).collect()
}

/// Describes one frame `spec` lost: error weights, degrees, syndrome,
/// quantization and saturation, and the recheck outcomes.
fn failure_line(
    code: &LdpcCode,
    degrees: &[usize],
    spec: Spec,
    frame: (f64, usize),
    codeword: &BitVec,
    llrs: &[Llr],
    lost: &Decoded,
) -> String {
    let k = code.k();
    let wrong: Vec<usize> = (0..code.n())
        .filter(|&i| lost.word.get(i) != codeword.get(i))
        .collect();
    let info = wrong.iter().filter(|&&i| i < k).count();
    let mut by_degree = std::collections::BTreeMap::<usize, usize>::new();
    for &i in &wrong {
        *by_degree.entry(degrees[i]).or_default() += 1;
    }
    let syndrome = code.syndrome(&lost.word).count_ones();
    let mut line = format!(
        "FAIL ebn0={:.2} frame={} decoder={} iterations={} syndrome_passed={} \
         syndrome_weight={} wrong_info={} wrong_total={} wrong_by_degree={:?}",
        frame.0,
        frame.1,
        spec.label(),
        lost.iterations,
        lost.syndrome_passed,
        syndrome,
        info,
        wrong.len(),
        by_degree
    );
    if let Spec::Layered(config, cap) = spec {
        let mut decoder = LayeredDecoder::new(code, config).expect("QC structure");
        let mut word = BitVec::new();
        decoder.decode_codeword_into(llrs, cap, &mut word);
        let scale = config.frame_scale(llrs);
        let limit = config.channel_limit() as i8;
        let clipped = llrs
            .iter()
            .filter(|&&l| config.quantize(l, scale).abs() == limit)
            .count();
        let posterior = decoder.posterior();
        let saturated = posterior
            .iter()
            .filter(|&&p| p.unsigned_abs() >= 127)
            .count();
        let wrong_magnitudes: Vec<u8> = wrong
            .iter()
            .take(16)
            .map(|&i| posterior[i].unsigned_abs())
            .collect();
        line += &format!(
            " frame_scale={scale:.4} channel_clipped={clipped} beliefs_saturated={saturated} \
             wrong_belief_magnitudes={wrong_magnitudes:?}"
        );
    }
    for name in RECHECKS {
        let redo = Decoder::new(code, recheck_spec(name)).decode(llrs);
        let ok = (0..k).all(|i| redo.word.get(i) == codeword.get(i));
        line += &format!(
            " {name}={}/{}",
            if ok { "ok" } else { "lost" },
            redo.iterations
        );
    }
    line
}

fn main() {
    let args = parse_args();
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
    let encoder = LdpcEncoder::new(code.clone());
    let degrees = column_degrees(&code);
    let k = encoder.k();
    let rate = k as f64 / code.n() as f64;
    let chunks = args.frames.div_ceil(CHUNK);
    for &ebn0 in &args.ebn0 {
        let variance = 1.0 / (2.0 * rate * 10f64.powf(ebn0 / 10.0));
        let next = std::sync::atomic::AtomicUsize::new(0);
        let tallies = Mutex::new(vec![Tally::default(); args.decoders.len()]);
        let failures = Mutex::new(Vec::<(usize, String)>::new());
        let started = std::time::Instant::now();
        std::thread::scope(|scope| {
            for _ in 0..args.workers {
                scope.spawn(|| {
                    let mut decoders: Vec<Decoder> = args
                        .decoders
                        .iter()
                        .map(|&s| Decoder::new(&code, s))
                        .collect();
                    let mut watched = args.failures.map(|s| Decoder::new(&code, s));
                    let channel = AwgnChannel::from_variance(variance);
                    let mut local = vec![Tally::default(); decoders.len()];
                    loop {
                        let chunk = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if chunk >= chunks {
                            break;
                        }
                        let mut rng = StdRng::seed_from_u64(args.seed ^ ((chunk as u64) << 20));
                        for offset in 0..CHUNK {
                            let frame = chunk * CHUNK + offset;
                            let message = BitVec::random(k, &mut rng);
                            let codeword = encoder.encode(&message);
                            let llrs: Vec<Llr> = (0..codeword.len())
                                .map(|i| {
                                    let symbol = if codeword.get(i) { -1.0 } else { 1.0 };
                                    let y = channel.transmit(symbol, &mut rng);
                                    Llr::new((2.0 * y / variance) as f32)
                                })
                                .collect();
                            if frame >= args.frames {
                                continue;
                            }
                            let wrong = |w: &BitVec| (0..k).any(|i| w.get(i) != codeword.get(i));
                            for (decoder, tally) in decoders.iter_mut().zip(local.iter_mut()) {
                                let decoded = decoder.decode(&llrs);
                                tally.iterations += decoded.iterations;
                                if wrong(&decoded.word) {
                                    tally.errors += 1;
                                    tally.undetected += usize::from(decoded.syndrome_passed);
                                }
                            }
                            if let (Some(spec), Some(decoder)) = (args.failures, watched.as_mut()) {
                                let decoded = decoder.decode(&llrs);
                                if wrong(&decoded.word) {
                                    let line = failure_line(
                                        &code,
                                        &degrees,
                                        spec,
                                        (ebn0, frame),
                                        &codeword,
                                        &llrs,
                                        &decoded,
                                    );
                                    failures.lock().unwrap().push((frame, line));
                                }
                            }
                        }
                    }
                    let mut shared = tallies.lock().unwrap();
                    for (sum, part) in shared.iter_mut().zip(&local) {
                        sum.errors += part.errors;
                        sum.undetected += part.undetected;
                        sum.iterations += part.iterations;
                    }
                });
            }
        });
        let seconds = started.elapsed().as_secs_f64();
        for (spec, tally) in args.decoders.iter().zip(tallies.into_inner().unwrap()) {
            println!(
                "POINT ebn0={ebn0:.2} sigma2={variance:.6} decoder={} frames={} errors={} \
                 fer={:.3e} undetected={} mean_iterations={:.2} seconds={seconds:.0}",
                spec.label(),
                args.frames,
                tally.errors,
                tally.errors as f64 / args.frames as f64,
                tally.undetected,
                tally.iterations as f64 / args.frames as f64
            );
        }
        let mut failures = failures.into_inner().unwrap();
        failures.sort_by_key(|(frame, _)| *frame);
        for (_, line) in failures {
            println!("{line}");
        }
    }
}
