//! LDPC-only AWGN-BPSK BLER sweep for comparison against aff3ct
//! (`@/citation/Cassagne2019`). The isolated LDPC decoder (normalized min-sum,
//! early termination) decodes noisy all-zero codewords of the `--code`
//! [`ComparisonCode`] drawn from [`gf2_sim::testutil::AwgnLlrSource`]; a frame
//! error is a nonzero bit among the K message bits. The output CSV has header
//! `es_n0_db,gf2_sim_bler,gf2_sim_fps` and one row per Es/N0 point.

use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use rayon::prelude::*;

use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig, LdpcDecoder};
use gf2_coding::traits::IterativeSoftDecoder;
use gf2_coding::LdpcCode;
use gf2_sim::testutil::{AwgnLlrSource, ComparisonCode};

const NMS_SCALE: f32 = 0.75;

struct Cfg {
    code: ComparisonCode,
    esn0: Vec<f64>,
    max_frames: u64,
    target_errors: u64,
    max_iter: usize,
    seed: u64,
    output: PathBuf,
}

/// Parses `start:stop:step` into an inclusive range.
fn parse_range(s: &str) -> Result<Vec<f64>, String> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return Err(format!("--esn0-range '{s}' must be 'start:stop:step'"));
    }
    let start: f64 = parts[0].parse().map_err(|_| "bad start".to_string())?;
    let stop: f64 = parts[1].parse().map_err(|_| "bad stop".to_string())?;
    let step: f64 = parts[2].parse().map_err(|_| "bad step".to_string())?;
    if step <= 0.0 {
        return Err("step must be positive".to_string());
    }
    let n = ((stop - start) / step).round() as i64;
    Ok((0..=n)
        .map(|i| start + (i as f64) * step)
        .filter(|&v| v <= stop + step * 1e-3)
        .collect())
}

fn parse_args() -> Cfg {
    let mut code: Option<ComparisonCode> = None;
    let mut esn0: Option<Vec<f64>> = None;
    let mut max_frames: u64 = 20000;
    let mut target_errors: u64 = 200;
    let mut max_iter: usize = 50;
    let mut seed: u64 = 42;
    let mut output: Option<PathBuf> = None;

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut next = || {
            args.next()
                .unwrap_or_else(|| die(&format!("{a} requires a value")))
        };
        match a.as_str() {
            "--code" => {
                code = Some(ComparisonCode::parse(&next()).unwrap_or_else(|e| die(&e)));
            }
            "--esn0-range" => {
                esn0 = Some(parse_range(&next()).unwrap_or_else(|e| die(&e)));
            }
            "--max-frames" => {
                max_frames = next().parse().unwrap_or_else(|_| die("bad --max-frames"))
            }
            "--target-errors" => {
                target_errors = next()
                    .parse()
                    .unwrap_or_else(|_| die("bad --target-errors"))
            }
            "--max-iter" => max_iter = next().parse().unwrap_or_else(|_| die("bad --max-iter")),
            "--seed" => seed = next().parse().unwrap_or_else(|_| die("bad --seed")),
            "--output" => output = Some(PathBuf::from(next())),
            "-h" | "--help" => {
                println!(
                    "ldpc_bler_sweep --code <dvb-t2-r12|nr-bg1-r12> \\\n\
                     \t--esn0-range start:stop:step --max-frames N --target-errors N \\\n\
                     \t--max-iter N --seed S --output file.csv"
                );
                std::process::exit(0);
            }
            other => die(&format!("unknown argument '{other}'")),
        }
    }

    Cfg {
        code: code.unwrap_or_else(|| die("--code is required")),
        esn0: esn0.unwrap_or_else(|| die("--esn0-range is required")),
        max_frames,
        target_errors,
        max_iter,
        seed,
        output: output.unwrap_or_else(|| die("--output is required")),
    }
}

fn die(msg: &str) -> ! {
    eprintln!("error: {msg}");
    std::process::exit(2);
}

/// Es/N0 (dB) -> AWGN noise std for unit-energy BPSK (`Es = 1`):
/// `sigma = sqrt(1 / (2 · 10^(EsN0/10)))`. Equivalent to `N0 = 1/10^(EsN0/10)`
/// and `sigma² = N0/2`.
fn esn0_db_to_sigma(esn0_db: f64) -> f64 {
    let esn0_lin = 10f64.powf(esn0_db / 10.0);
    (1.0 / (2.0 * esn0_lin)).sqrt()
}

/// Runs one Es/N0 point: decodes frames until `target_errors` frame errors or
/// `max_frames` frames, whichever first. Returns `(bler, frames, errors, fps)`.
///
/// Each `chunk`-frame slice owns an `AwgnLlrSource` seeded from `point_seed`
/// and the slice's first frame index. The error budget is checked between
/// waves of `chunk · workers` frames.
fn run_point(
    code: &LdpcCode,
    point_seed: u64,
    sigma: f64,
    max_frames: u64,
    target_errors: u64,
    max_iter: usize,
) -> (f64, u64, u64, f64) {
    let n = code.n();
    let k = code.k();
    // Small slices bound the overshoot past `target_errors` to one wave.
    let chunk: u64 = 32;

    let start = Instant::now();
    let mut frames_done: u64 = 0;
    let mut errors: u64 = 0;

    let workers = rayon::current_num_threads().max(1) as u64;
    let wave = chunk * workers;

    while frames_done < max_frames && errors < target_errors {
        let wave_start = frames_done;
        let wave_end = (wave_start + wave).min(max_frames);
        let wave_len = wave_end - wave_start;

        let slice_starts: Vec<u64> = (wave_start..wave_end).step_by(chunk as usize).collect();
        let wave_errors: u64 = slice_starts
            .par_iter()
            .map(|&slice_start| {
                let slice_end = (slice_start + chunk).min(wave_end);
                let mut src = AwgnLlrSource::new(point_seed ^ slice_start.wrapping_mul(0x9E37));
                let cfg = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(NMS_SCALE), true);
                let mut dec = LdpcDecoder::with_config(code.clone(), cfg);
                let mut local_err = 0u64;
                for _ in slice_start..slice_end {
                    let llrs = src.frame_all_zero(n, sigma);
                    let res = dec.decode_iterative(&llrs, max_iter);
                    debug_assert_eq!(res.decoded_bits.len(), k);
                    if (0..k).any(|i| res.decoded_bits.get(i)) {
                        local_err += 1;
                    }
                }
                local_err
            })
            .sum();

        frames_done += wave_len;
        errors += wave_errors;
    }

    let secs = start.elapsed().as_secs_f64();
    let fps = if secs > 0.0 {
        frames_done as f64 / secs
    } else {
        0.0
    };
    let bler = if frames_done > 0 {
        errors as f64 / frames_done as f64
    } else {
        0.0
    };
    (bler, frames_done, errors, fps)
}

fn main() {
    let cfg = parse_args();
    let code = cfg.code.build();
    eprintln!(
        "ldpc_bler_sweep: code N={} K={} (rate {:.4}), {} Es/N0 points, \
         max_frames={} target_errors={} max_iter={} seed={}",
        code.n(),
        code.k(),
        code.k() as f64 / code.n() as f64,
        cfg.esn0.len(),
        cfg.max_frames,
        cfg.target_errors,
        cfg.max_iter,
        cfg.seed,
    );

    let mut rows: Vec<(f64, f64, f64)> = Vec::with_capacity(cfg.esn0.len());
    for (idx, &esn0_db) in cfg.esn0.iter().enumerate() {
        let sigma = esn0_db_to_sigma(esn0_db);
        let point_seed = cfg.seed ^ (idx as u64).wrapping_mul(0x1234_5678_9ABC_DEF1);
        let (bler, frames, errors, fps) = run_point(
            &code,
            point_seed,
            sigma,
            cfg.max_frames,
            cfg.target_errors,
            cfg.max_iter,
        );
        eprintln!(
            "  Es/N0={esn0_db:>5.2} dB  BLER={bler:.3e}  frames={frames}  \
             errors={errors}  fps={fps:.1}"
        );
        rows.push((esn0_db, bler, fps));
    }

    let mut f = std::fs::File::create(&cfg.output)
        .unwrap_or_else(|e| die(&format!("cannot create {}: {e}", cfg.output.display())));
    writeln!(f, "es_n0_db,gf2_sim_bler,gf2_sim_fps").unwrap();
    for (esn0_db, bler, fps) in &rows {
        writeln!(f, "{esn0_db},{bler:.6e},{fps:.3}").unwrap();
    }
    eprintln!("wrote {}", cfg.output.display());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_range_inclusive_endpoints() {
        let pts = parse_range("0:1:0.5").unwrap();
        assert_eq!(pts.len(), 3);
        assert!((pts[0] - 0.0).abs() < 1e-12);
        assert!((pts[1] - 0.5).abs() < 1e-12);
        assert!((pts[2] - 1.0).abs() < 1e-12);
    }

    /// f64 step drift must not drop the stop endpoint.
    #[test]
    fn test_parse_range_negative_float_step() {
        let pts = parse_range("-1.8:-0.8:0.2").unwrap();
        assert_eq!(pts.len(), 6);
        assert!((pts[0] - (-1.8)).abs() < 1e-9);
        assert!((pts[5] - (-0.8)).abs() < 1e-9);
    }

    #[test]
    fn test_parse_range_single_point() {
        let pts = parse_range("2.5:2.5:1.0").unwrap();
        assert_eq!(pts.len(), 1);
        assert!((pts[0] - 2.5).abs() < 1e-12);
    }

    #[test]
    fn test_parse_range_rejects_malformed() {
        assert!(parse_range("1:2").is_err());
        assert!(parse_range("1:2:3:4").is_err());
        assert!(parse_range("a:2:1").is_err());
        assert!(parse_range("1:b:1").is_err());
        assert!(parse_range("1:2:c").is_err());
        assert!(parse_range("1:2:0").is_err());
        assert!(parse_range("1:2:-0.5").is_err());
    }

    #[test]
    fn test_esn0_db_to_sigma_known_values() {
        assert!((esn0_db_to_sigma(0.0) - 0.5f64.sqrt()).abs() < 1e-12);
        assert!((esn0_db_to_sigma(10.0) - 0.05f64.sqrt()).abs() < 1e-12);
        assert!((esn0_db_to_sigma(-10.0) - 5.0f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn test_esn0_db_to_sigma_n0_relation() {
        for &db in &[-4.3, -1.4, 0.0, 3.7] {
            let sigma = esn0_db_to_sigma(db);
            let n0 = 2.0 * sigma * sigma;
            assert!((n0 - 10f64.powf(-db / 10.0)).abs() < 1e-12, "EsN0={db} dB");
        }
    }
}
