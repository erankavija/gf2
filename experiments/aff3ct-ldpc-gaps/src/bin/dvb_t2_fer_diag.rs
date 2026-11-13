//! Frame error rates and per-frame failure diagnostics of gf2's canonical
//! flooding decoder and layered i8 decoder on DVB-T2 normal rate 1/2
//! (`@/citation/Etsi2015`) over BPSK AWGN, for the AFF3CT
//! (`@/citation/Cassagne2019`) comparison of @/issue/d5cd53e0.
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
//! `--frames` (default 20 000) and `--workers` (default 4) must be positive;
//! `--seed` defaults to 0. A decoder is `flooding:<alpha>:<cap>` (normalized
//! min-sum), `flooding-spa:<cap>`, `layered:<cap>` (`LayeredConfig::dvb_t2`),
//! or `layered:<cap>:<eighths>:<scale>:<channel limit>:<message limit>`
//! (normalized min-sum with the cap guard and early termination); output
//! names every decoder in its canonical form, the last for layered ones.
//!
//! The first line, `RUN`, gives the SHA-256 of the executable and the
//! effective arguments, seed included. Each point prints one `POINT` line per
//! decoder: frames, frame errors, the FER with its 95% Wilson score interval,
//! frames whose terminal word passes the syndrome but is wrong, mean
//! iterations and, for a layered decoder, the mean per frame of channel LLRs
//! quantized to the channel limit and of terminal beliefs at the i8 bound.
//! `--failures <decoder>` also prints one `FAIL` line per frame that decoder
//! loses, with its error weights, the column degrees of the wrong bits, the
//! terminal syndrome weight, for a layered decoder the frame scale and the
//! clipped and saturated counts, and the outcome of re-decoding the frame
//! under each [`Recheck`].

use std::sync::atomic::{AtomicUsize, Ordering};
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
use sha2::{Digest, Sha256};

const CHUNK: usize = 64;

/// The standard normal quantile of the two-sided 95% interval.
const Z95: f64 = 1.959_963_984_540_054;

/// A decoder under test, in the vocabulary of `--decoder`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Spec {
    Flooding {
        alpha: f32,
        cap: usize,
    },
    FloodingSpa {
        cap: usize,
    },
    /// Normalized min-sum at `eighths`/8 with the cap guard and early
    /// termination.
    Layered {
        cap: usize,
        eighths: u8,
        scale: f32,
        channel_limit: u8,
        message_limit: u8,
    },
}

impl Spec {
    /// `LayeredConfig::dvb_t2` at `cap`.
    fn dvb_t2_layered(cap: usize) -> Self {
        let config = LayeredConfig::dvb_t2();
        let LayeredRule::Normalized { eighths } = config.rule() else {
            unreachable!("the DVB-T2 setting is normalized min-sum")
        };
        Self::Layered {
            cap,
            eighths,
            scale: config.channel_scale(),
            channel_limit: config.channel_limit(),
            message_limit: config.message_limit(),
        }
    }

    fn parse(text: &str) -> Result<Self, String> {
        fn field<T: std::str::FromStr>(text: &str, value: &str) -> Result<T, String> {
            value.parse().map_err(|_| format!("bad decoder {text}"))
        }
        let parts: Vec<&str> = text.split(':').collect();
        match parts.as_slice() {
            ["flooding", alpha, cap] => Ok(Self::Flooding {
                alpha: field(text, alpha)?,
                cap: field(text, cap)?,
            }),
            ["flooding-spa", cap] => Ok(Self::FloodingSpa {
                cap: field(text, cap)?,
            }),
            ["layered", cap] => Ok(Self::dvb_t2_layered(field(text, cap)?)),
            ["layered", cap, eighths, scale, channel_limit, message_limit] => Ok(Self::Layered {
                cap: field(text, cap)?,
                eighths: field(text, eighths)?,
                scale: field(text, scale)?,
                channel_limit: field(text, channel_limit)?,
                message_limit: field(text, message_limit)?,
            }),
            _ => Err(format!("bad decoder {text}")),
        }
    }

    /// The canonical `--decoder` form, which [`Self::parse`] reads back.
    fn label(&self) -> String {
        match *self {
            Self::Flooding { alpha, cap } => format!("flooding:{alpha}:{cap}"),
            Self::FloodingSpa { cap } => format!("flooding-spa:{cap}"),
            Self::Layered {
                cap,
                eighths,
                scale,
                channel_limit,
                message_limit,
            } => format!("layered:{cap}:{eighths}:{scale}:{channel_limit}:{message_limit}"),
        }
    }

    fn layered_config(&self) -> Option<LayeredConfig> {
        match *self {
            Self::Layered {
                eighths,
                scale,
                channel_limit,
                message_limit,
                ..
            } => Some(LayeredConfig::new(
                LayeredRule::Normalized { eighths },
                scale,
                channel_limit,
                message_limit,
                true,
            )),
            _ => None,
        }
    }
}

/// An alternative each lost frame is re-decoded under by `--failures`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Recheck {
    LayeredCap200,
    LayeredNms6,
    LayeredScale2,
    LayeredScale2Cap200,
    LayeredMessage60,
    FloodingNms0875Cap50,
    FloodingNms0875Cap200,
    FloodingSpaCap50,
}

impl Recheck {
    const ALL: [Self; 8] = [
        Self::LayeredCap200,
        Self::LayeredNms6,
        Self::LayeredScale2,
        Self::LayeredScale2Cap200,
        Self::LayeredMessage60,
        Self::FloodingNms0875Cap50,
        Self::FloodingNms0875Cap200,
        Self::FloodingSpaCap50,
    ];

    fn spec(self) -> Spec {
        let dvb = Spec::dvb_t2_layered(25);
        let Spec::Layered {
            eighths,
            scale,
            channel_limit,
            message_limit,
            ..
        } = dvb
        else {
            unreachable!("dvb_t2_layered is layered")
        };
        let layered = |cap, eighths, scale, message_limit| Spec::Layered {
            cap,
            eighths,
            scale,
            channel_limit,
            message_limit,
        };
        match self {
            Self::LayeredCap200 => layered(200, eighths, scale, message_limit),
            Self::LayeredNms6 => layered(25, 6, scale, message_limit),
            Self::LayeredScale2 => layered(25, eighths, 2.0, message_limit),
            Self::LayeredScale2Cap200 => layered(200, eighths, 2.0, message_limit),
            Self::LayeredMessage60 => layered(25, eighths, scale, 60),
            Self::FloodingNms0875Cap50 => Spec::Flooding {
                alpha: 0.875,
                cap: 50,
            },
            Self::FloodingNms0875Cap200 => Spec::Flooding {
                alpha: 0.875,
                cap: 200,
            },
            Self::FloodingSpaCap50 => Spec::FloodingSpa { cap: 50 },
        }
    }
}

enum Decoder {
    Flooding(LdpcDecoder, usize),
    Layered(Box<LayeredDecoder>, usize),
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
            Spec::Flooding { alpha, cap } => {
                let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(alpha), true);
                Self::Flooding(LdpcDecoder::with_config(code.clone(), config), cap)
            }
            Spec::FloodingSpa { cap } => {
                let config = DecoderConfig::new(DecoderAlgorithm::SumProduct, true);
                Self::Flooding(LdpcDecoder::with_config(code.clone(), config), cap)
            }
            Spec::Layered { cap, .. } => {
                let config = spec.layered_config().expect("a layered spec");
                Self::Layered(
                    Box::new(
                        LayeredDecoder::new(code, config).expect("DVB-T2 retains QC structure"),
                    ),
                    cap,
                )
            }
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

    /// For a layered decoder after decoding `llrs`: channel LLRs quantized to
    /// the channel limit, and terminal beliefs at the i8 bound.
    fn saturation(&self, llrs: &[Llr]) -> Option<(usize, usize)> {
        let Self::Layered(decoder, _) = self else {
            return None;
        };
        let config = decoder.config();
        let scale = config.frame_scale(llrs);
        let limit = config.channel_limit() as i8;
        let clipped = llrs
            .iter()
            .filter(|&&l| config.quantize(l, scale).abs() == limit)
            .count();
        let saturated = decoder
            .posterior()
            .iter()
            .filter(|&&p| p.unsigned_abs() >= 127)
            .count();
        Some((clipped, saturated))
    }
}

/// The 95% Wilson score interval of `errors` successes in `frames` trials.
fn wilson95(errors: usize, frames: usize) -> (f64, f64) {
    let n = frames as f64;
    let p = errors as f64 / n;
    let z2 = Z95 * Z95;
    let centre = p + z2 / (2.0 * n);
    let half = Z95 * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt();
    let denominator = 1.0 + z2 / n;
    (
        ((centre - half) / denominator).max(0.0),
        ((centre + half) / denominator).min(1.0),
    )
}

#[derive(Default, Clone, Copy)]
struct Tally {
    frames: usize,
    errors: usize,
    undetected: usize,
    iterations: usize,
    clipped: usize,
    saturated: usize,
}

impl Tally {
    fn add(&mut self, other: &Self) {
        self.frames += other.frames;
        self.errors += other.errors;
        self.undetected += other.undetected;
        self.iterations += other.iterations;
        self.clipped += other.clipped;
        self.saturated += other.saturated;
    }
}

#[derive(Debug, PartialEq)]
struct Args {
    ebn0: Vec<f64>,
    frames: usize,
    seed: u64,
    workers: usize,
    decoders: Vec<Spec>,
    failures: Option<Spec>,
}

impl Args {
    /// The effective arguments, in the form [`parse_args`] reads.
    fn render(&self) -> String {
        let ebn0: Vec<String> = self.ebn0.iter().map(f64::to_string).collect();
        let mut text = format!(
            "--ebn0 {} --frames {} --seed {} --workers {}",
            ebn0.join(","),
            self.frames,
            self.seed,
            self.workers
        );
        for spec in &self.decoders {
            text += &format!(" --decoder {}", spec.label());
        }
        if let Some(spec) = self.failures {
            text += &format!(" --failures {}", spec.label());
        }
        text
    }
}

fn die(message: &str) -> ! {
    eprintln!("dvb_t2_fer_diag: {message}");
    std::process::exit(2)
}

fn parse_args(argv: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let mut args = Args {
        ebn0: Vec::new(),
        frames: 20_000,
        seed: 0,
        workers: 4,
        decoders: Vec::new(),
        failures: None,
    };
    let mut it = argv.into_iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        let number = |what: &str| format!("bad {what} {value}");
        match flag.as_str() {
            "--ebn0" => {
                args.ebn0 = value
                    .split(',')
                    .map(|v| v.trim().parse().map_err(|_| number("--ebn0")))
                    .collect::<Result<_, _>>()?;
            }
            "--frames" => args.frames = value.parse().map_err(|_| number("--frames"))?,
            "--seed" => args.seed = value.parse().map_err(|_| number("--seed"))?,
            "--workers" => args.workers = value.parse().map_err(|_| number("--workers"))?,
            "--decoder" => args.decoders.push(Spec::parse(&value)?),
            "--failures" => args.failures = Some(Spec::parse(&value)?),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if args.ebn0.is_empty() || args.decoders.is_empty() {
        return Err("--ebn0 and at least one --decoder are required".into());
    }
    if args.frames == 0 || args.workers == 0 {
        return Err("--frames and --workers must be positive".into());
    }
    Ok(args)
}

/// Column degree of every variable.
fn column_degrees(code: &LdpcCode) -> Vec<usize> {
    let decoder = LdpcDecoder::new(code.clone());
    let layout = decoder.edge_layout();
    (0..code.n()).map(|v| layout.var_range(v).len()).collect()
}

/// Describes one frame `decoder` (built from `spec`) lost: error weights,
/// degrees, syndrome, quantization and saturation, and the recheck outcomes.
#[allow(clippy::too_many_arguments)]
fn failure_line(
    code: &LdpcCode,
    degrees: &[usize],
    spec: Spec,
    decoder: &Decoder,
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
    if let (Some((clipped, saturated)), Decoder::Layered(layered, _)) =
        (decoder.saturation(llrs), decoder)
    {
        let posterior = layered.posterior();
        let wrong_magnitudes: Vec<u8> = wrong
            .iter()
            .take(16)
            .map(|&i| posterior[i].unsigned_abs())
            .collect();
        line += &format!(
            " frame_scale={:.4} channel_clipped={clipped} beliefs_saturated={saturated} \
             wrong_belief_magnitudes={wrong_magnitudes:?}",
            layered.config().frame_scale(llrs)
        );
    }
    for recheck in Recheck::ALL {
        let redo = Decoder::new(code, recheck.spec()).decode(llrs);
        let ok = (0..k).all(|i| redo.word.get(i) == codeword.get(i));
        line += &format!(
            " recheck[{}]={}/{}",
            recheck.spec().label(),
            if ok { "ok" } else { "lost" },
            redo.iterations
        );
    }
    line
}

/// The SHA-256 of the running executable, in hex.
fn executable_digest() -> String {
    let path = std::env::current_exe().unwrap_or_else(|e| die(&format!("current_exe: {e}")));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| die(&format!("read {path:?}: {e}")));
    Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn main() {
    let args = parse_args(std::env::args().skip(1)).unwrap_or_else(|e| die(&e));
    println!(
        "RUN harness=dvb_t2_fer_diag executable_sha256={} args={}",
        executable_digest(),
        args.render()
    );
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
    let encoder = LdpcEncoder::new(code.clone());
    let degrees = column_degrees(&code);
    let k = encoder.k();
    let rate = k as f64 / code.n() as f64;
    let chunks = args.frames.div_ceil(CHUNK);
    for &ebn0 in &args.ebn0 {
        let variance = 1.0 / (2.0 * rate * 10f64.powf(ebn0 / 10.0));
        let next = AtomicUsize::new(0);
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
                    let mut watched = args.failures.map(|s| (s, Decoder::new(&code, s)));
                    let channel = AwgnChannel::from_variance(variance);
                    let mut local = vec![Tally::default(); decoders.len()];
                    loop {
                        let chunk = next.fetch_add(1, Ordering::Relaxed);
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
                                tally.frames += 1;
                                tally.iterations += decoded.iterations;
                                if let Some((clipped, saturated)) = decoder.saturation(&llrs) {
                                    tally.clipped += clipped;
                                    tally.saturated += saturated;
                                }
                                if wrong(&decoded.word) {
                                    tally.errors += 1;
                                    tally.undetected += usize::from(decoded.syndrome_passed);
                                }
                            }
                            if let Some((spec, decoder)) = watched.as_mut() {
                                let decoded = decoder.decode(&llrs);
                                if wrong(&decoded.word) {
                                    let line = failure_line(
                                        &code,
                                        &degrees,
                                        *spec,
                                        decoder,
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
                        sum.add(part);
                    }
                });
            }
        });
        let seconds = started.elapsed().as_secs_f64();
        for (spec, tally) in args.decoders.iter().zip(tallies.into_inner().unwrap()) {
            if tally.frames != args.frames {
                die(&format!(
                    "decoded {} of {} frames at {ebn0} dB",
                    tally.frames, args.frames
                ));
            }
            let frames = tally.frames as f64;
            let (low, high) = wilson95(tally.errors, tally.frames);
            let mut line = format!(
                "POINT ebn0={ebn0:.2} sigma2={variance:.6} seed={} decoder={} frames={} \
                 errors={} fer={:.3e} fer_wilson95=[{low:.3e},{high:.3e}] undetected={} \
                 mean_iterations={:.2}",
                args.seed,
                spec.label(),
                tally.frames,
                tally.errors,
                tally.errors as f64 / frames,
                tally.undetected,
                tally.iterations as f64 / frames
            );
            if matches!(spec, Spec::Layered { .. }) {
                line += &format!(
                    " mean_channel_clipped={:.1} mean_beliefs_saturated={:.1}",
                    tally.clipped as f64 / frames,
                    tally.saturated as f64 / frames
                );
            }
            println!("{line} seconds={seconds:.0}");
        }
        let mut failures = failures.into_inner().unwrap();
        failures.sort_by_key(|(frame, _)| *frame);
        for (_, line) in failures {
            println!("{line}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(text: &str) -> Vec<String> {
        text.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn zero_workers_or_frames_are_rejected() {
        for bad in [
            "--ebn0 1.5 --decoder layered:25 --workers 0",
            "--ebn0 1.5 --decoder layered:25 --frames 0",
        ] {
            assert!(parse_args(argv(bad)).is_err(), "{bad}");
        }
        assert!(parse_args(argv("--ebn0 1.5 --decoder layered:25")).is_ok());
    }

    #[test]
    fn rendered_arguments_parse_back_to_the_same_run() {
        let args = parse_args(argv(
            "--ebn0 1.3,2 --frames 640 --seed 7 --workers 3 --decoder flooding:0.75:50 \
             --decoder flooding-spa:50 --decoder layered:25:7:2:31:40 --failures layered:25",
        ))
        .unwrap();
        assert_eq!(parse_args(argv(&args.render())).unwrap(), args);
        for recheck in Recheck::ALL {
            let spec = recheck.spec();
            assert_eq!(Spec::parse(&spec.label()).unwrap(), spec);
        }
    }

    #[test]
    fn layered_shorthand_is_the_dvb_t2_setting() {
        let spec = Spec::parse("layered:25").unwrap();
        assert_eq!(spec.layered_config(), Some(LayeredConfig::dvb_t2()));
    }

    #[test]
    fn wilson_interval_bounds_zero_and_matches_a_hand_value() {
        let (low, high) = wilson95(0, 5000);
        assert_eq!(low, 0.0);
        assert!((high - 7.677e-4).abs() < 1e-6, "{high}");
        // 10 of 100: (0.0552, 0.1744) to four places.
        let (low, high) = wilson95(10, 100);
        assert!((low - 0.0552).abs() < 1e-4 && (high - 0.1744).abs() < 1e-4);
    }
}
