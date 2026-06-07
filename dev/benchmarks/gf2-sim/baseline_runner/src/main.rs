//! Single-thread baseline measurement harness for the gf2-sim epic.
//!
//! Invokes `SimulationRunner::run_with_decoder` on the legacy
//! `crates/gf2-coding/src/simulation.rs` path (the path being migrated later)
//! and records per-cell throughput metrics as a CSV receipt.
//!
//! # Usage
//!
//! ```bash
//! # First build:
//! cargo build --release --manifest-path dev/benchmarks/gf2-sim/baseline_runner/Cargo.toml
//!
//! # Run the full matrix:
//! cargo run --release --manifest-path dev/benchmarks/gf2-sim/baseline_runner/Cargo.toml -- \
//!     --output dev/benchmarks/gf2-sim/baseline-single-thread.csv
//!
//! # Compare against a committed CSV (delta mode):
//! cargo run --release --manifest-path dev/benchmarks/gf2-sim/baseline_runner/Cargo.toml -- \
//!     --output /tmp/new_baseline.csv --compare dev/benchmarks/gf2-sim/baseline-single-thread.csv
//! ```
//!
//! # SNR points
//!
//! Three MODCODs are swept:
//!   - r1/2 16-QAM:  Es/N0 in {5.0, 6.25, 7.5} dB
//!   - r2/3 16-QAM:  Es/N0 in {7.9, 8.9, 10.4} dB
//!   - r1/2 64-QAM:  Es/N0 in {8.9, 9.9, 11.4} dB
//!
//! Three decoder x demap pairs per cell:
//!   - (SumProduct, ExactLogMap)
//!   - (NormalizedMinSum(0.75), ExactLogMap)
//!   - (MinSum, MaxLog)
//!
//! Fixed seed 42, 200 frames per cell.

#![deny(unsafe_code)]

use gf2_coding::dvb_t2_bicm_harness::{
    esn0_to_ebn0, mod_str, parse_baseline_csv, rate_display as rate_str, rate_f64,
    BaselineCellResult, BicmAwgnChannel, BicmFecEncoder, BASELINE_MATRIX_CELL_COUNT,
};
use gf2_coding::ldpc::dvb_t2::bit_interleaver::{
    DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation,
};
use gf2_coding::ldpc::dvb_t2::concat::{ConcatError, DvbT2Concat};
use gf2_coding::ldpc::dvb_t2::FrameSize;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::simulation::{SimulationConfig, SimulationRunner};
use gf2_coding::traits::{BlockEncoder, DecoderResult};
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn decoder_str(alg: &DecoderAlgorithm) -> String {
    match alg {
        DecoderAlgorithm::SumProduct => "SumProduct".to_string(),
        DecoderAlgorithm::MinSum => "MinSum".to_string(),
        DecoderAlgorithm::NormalizedMinSum(a) => format!("NormalizedMinSum({a:.2})"),
        DecoderAlgorithm::OffsetMinSum(b) => format!("OffsetMinSum({b:.2})"),
    }
}

fn demap_str(d: DemapMethod) -> &'static str {
    match d {
        DemapMethod::MaxLog => "MaxLog",
        DemapMethod::ExactLogMap => "ExactLogMap",
    }
}

// CellResult is provided by gf2_coding::dvb_t2_bicm_harness::BaselineCellResult.
// Use the shared type directly to avoid duplication.
type CellResult = BaselineCellResult;

// ---------------------------------------------------------------------------
// Matrix sweep configuration
// ---------------------------------------------------------------------------

/// A single cell in the sweep matrix.
struct Cell {
    rate: CodeRate,
    modulation: DvbT2Modulation,
    /// Es/N0 in dB (three points per MODCOD).
    esn0_db: f64,
    decoder_algo: DecoderAlgorithm,
    demap: DemapMethod,
}

/// Build the full 3 MODCODs × 3 SNR points × 3 decoder/demap pairs matrix.
fn build_matrix() -> Vec<Cell> {
    // SNR points per MODCOD (pre-waterfall, waterfall-mid, deep-waterfall).
    // Anchored to ETSI TS 102 831 Table 44 QEF C/N thresholds for SumProduct+ExactLogMap:
    //   r1/2 16-QAM: QEF ~6.0 dB  => {5.0, 6.25, 7.5}
    //   r2/3 16-QAM: QEF ~8.9 dB  => {7.9, 8.9, 10.4}
    //   r1/2 64-QAM: QEF ~9.9 dB  => {8.9, 9.9, 11.4}
    let modcods: &[(CodeRate, DvbT2Modulation, &[f64])] = &[
        (CodeRate::Rate1_2, DvbT2Modulation::Qam16, &[5.0, 6.25, 7.5]),
        (CodeRate::Rate2_3, DvbT2Modulation::Qam16, &[7.9, 8.9, 10.4]),
        (CodeRate::Rate1_2, DvbT2Modulation::Qam64, &[8.9, 9.9, 11.4]),
    ];

    let decoder_demap_pairs: &[(DecoderAlgorithm, DemapMethod)] = &[
        (DecoderAlgorithm::SumProduct, DemapMethod::ExactLogMap),
        (
            DecoderAlgorithm::NormalizedMinSum(0.75),
            DemapMethod::ExactLogMap,
        ),
        (DecoderAlgorithm::MinSum, DemapMethod::MaxLog),
    ];

    let mut cells = Vec::new();
    for &(rate, modulation, snrs) in modcods {
        for &esn0_db in snrs {
            for &(ref decoder_algo, demap) in decoder_demap_pairs {
                cells.push(Cell {
                    rate,
                    modulation,
                    esn0_db,
                    decoder_algo: decoder_algo.clone(),
                    demap,
                });
            }
        }
    }
    cells
}

// ---------------------------------------------------------------------------
// Git commit SHA helper
// ---------------------------------------------------------------------------

fn git_commit_sha() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short=10", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn today_date() -> String {
    // Try `date +%Y-%m-%d` first; fallback to a hand-rolled version via SystemTime.
    let out = std::process::Command::new("date")
        .arg("+%Y-%m-%d")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string());
    out.unwrap_or_else(|| "unknown".to_string())
}

// load_csv_results delegates to the shared parse_baseline_csv helper from
// gf2_coding::dvb_t2_bicm_harness. The file-IO wrapper stays here since it is
// thin CLI scaffolding.
fn load_csv_results(path: &Path) -> Vec<CellResult> {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    parse_baseline_csv(&content)
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut output_path: Option<PathBuf> = None;
    let mut compare_path: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--output" | "-o" => {
                i += 1;
                output_path = Some(PathBuf::from(&args[i]));
            }
            "--compare" | "-c" => {
                i += 1;
                compare_path = Some(PathBuf::from(&args[i]));
            }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: baseline_runner [--output <csv>] [--compare <baseline-csv>]\n\
                     \n\
                     Runs the single-thread DVB-T2 BICM baseline matrix and writes results to <csv>.\n\
                     If --compare is given, also prints a delta table vs the committed baseline.\n\
                     \n\
                     Default output: dev/benchmarks/gf2-sim/baseline-single-thread.csv\n"
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("Unknown argument: '{other}'");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let output_path = output_path
        .unwrap_or_else(|| PathBuf::from("dev/benchmarks/gf2-sim/baseline-single-thread.csv"));

    // Pin rayon to 1 thread so any incidental rayon usage (e.g., in the SIMD
    // dispatch layer) cannot inflate our single-thread measurement.
    rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build_global()
        .ok(); // Ignore error if already initialised.

    let cells = build_matrix();
    let total_cells = cells.len();
    debug_assert_eq!(
        total_cells, BASELINE_MATRIX_CELL_COUNT,
        "build_matrix() count must match the documented constant"
    );
    let commit_sha = git_commit_sha();
    let date = today_date();

    eprintln!(
        "Baseline matrix: {total_cells} cells (3 MODCODs × 3 SNR × 3 decoder/demap), \
         200 frames/cell, seed 42, 1 thread."
    );
    eprintln!("Output: {}", output_path.display());

    // Write the CSV header up front and append each cell's row as it completes,
    // so a SIGINT / kill mid-matrix still leaves a partial, valid CSV on disk
    // rather than losing the whole run.
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .unwrap_or_else(|e| panic!("Cannot create output dir: {e}"));
        }
    }
    {
        let mut hdr = std::fs::File::create(&output_path)
            .unwrap_or_else(|e| panic!("Cannot create {}: {e}", output_path.display()));
        writeln!(hdr, "{}", CellResult::csv_header()).unwrap();
    }

    let campaign_start = Instant::now();
    let mut results: Vec<CellResult> = Vec::with_capacity(total_cells);

    for (cell_idx, cell) in cells.iter().enumerate() {
        let bits_per_symbol: usize = match cell.modulation {
            DvbT2Modulation::Qam16 => 4,
            DvbT2Modulation::Qam64 => 6,
            _ => panic!("unsupported modulation"),
        };
        let code_rate_f64 = rate_f64(cell.rate);
        let ebn0_db = esn0_to_ebn0(cell.esn0_db, bits_per_symbol, code_rate_f64);

        let decoder_algo_str = decoder_str(&cell.decoder_algo);
        let demap_s = demap_str(cell.demap);

        eprintln!(
            "[{}/{total_cells}] {} {} Es/N0={:.2}dB decoder={decoder_algo_str} demap={demap_s}",
            cell_idx + 1,
            rate_str(cell.rate),
            mod_str(cell.modulation),
            cell.esn0_db,
        );

        // Build a fresh DvbT2Concat per cell (each cell may have a different
        // decoder config; the encoder/decoder share state in DvbT2Concat).
        let mut concat = DvbT2Concat::new(FrameSize::Normal, cell.rate)
            .unwrap_or_else(|e| panic!("DvbT2Concat::new failed: {e:?}"));
        concat.set_decoder_config(DecoderConfig::new(cell.decoder_algo.clone(), true));

        let modcod = DvbT2Modcod::new(FrameSize::Normal, cell.rate, cell.modulation);
        let interleaver = DvbT2BitInterleaver::new(modcod);

        let encoder = BicmFecEncoder::new(concat);
        let channel = BicmAwgnChannel::new(interleaver, bits_per_symbol, cell.demap);

        let sim_config = SimulationConfig {
            eb_n0_range_db: vec![ebn0_db],
            min_errors: usize::MAX, // do not stop early on errors; run all 200 frames
            max_frames: 200,
            max_decoder_iterations: 50,
            rng_seed: Some(42),
            output_path: None,
            checkpoint_dir: None,
            tracing_log_path: None,
            heartbeat_every_frames: None,
        };

        let cell_start = Instant::now();

        let sim_results = SimulationRunner::run_with_decoder(
            &encoder,
            |llrs| {
                let decode_result = encoder.concat.decode_soft(llrs);
                match decode_result {
                    Ok(bbframe) => DecoderResult::success(bbframe),
                    Err(ConcatError::LdpcDecodeFailed {
                        bbframe,
                        iterations,
                    }) => DecoderResult::new(bbframe, iterations, false, false),
                    Err(_) => {
                        DecoderResult::new(BitVec::with_capacity(encoder.k()), 50, false, false)
                    }
                }
            },
            &channel,
            &sim_config,
        );

        let cell_wall = cell_start.elapsed().as_secs_f64();

        // There is exactly one SNR point per cell.
        let pt = &sim_results.points[0];
        let frames_per_sec = if cell_wall > 0.0 {
            pt.num_frames as f64 / cell_wall
        } else {
            f64::INFINITY
        };
        let mean_iters = pt.avg_iterations.unwrap_or(0.0);

        eprintln!(
            "  -> {:.1} frames/s  FER={:.4}  BER={:.6}  mean_iters={:.2}  wall={:.1}s",
            frames_per_sec, pt.bler, pt.ber, mean_iters, cell_wall,
        );

        let cell_result = CellResult {
            rate: rate_str(cell.rate).to_string(),
            modulation: mod_str(cell.modulation).to_string(),
            es_n0_db: cell.esn0_db,
            decoder: decoder_algo_str,
            demap: demap_s.to_string(),
            frames: pt.num_frames,
            wall_seconds: cell_wall,
            frames_per_sec,
            mean_iters,
            ber: pt.ber,
            fer: pt.bler,
            commit_sha: commit_sha.clone(),
            date: date.clone(),
        };

        // Append this cell's row immediately so a kill mid-matrix preserves
        // every completed cell.
        {
            use std::io::Write as _;
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&output_path)
                .unwrap_or_else(|e| panic!("Cannot append to {}: {e}", output_path.display()));
            writeln!(f, "{}", cell_result.to_csv_row()).unwrap();
        }

        results.push(cell_result);
    }

    let total_wall = campaign_start.elapsed().as_secs_f64();
    eprintln!(
        "\nMatrix complete. Total wall-time: {:.1}s ({:.1} min).",
        total_wall,
        total_wall / 60.0
    );
    eprintln!("CSV written: {}", output_path.display());

    // Print canonical-config headline.
    let canonical = results.iter().find(|r| {
        r.rate == "1/2"
            && r.modulation == "16qam"
            && (r.es_n0_db - 6.25).abs() < 0.01
            && r.decoder.contains("SumProduct")
            && r.demap == "ExactLogMap"
    });
    if let Some(c) = canonical {
        eprintln!(
            "\nCanonical config (r1/2 16-QAM, Es/N0=6.25dB, SumProduct, ExactLogMap): \
             {:.3} frames/s  FER={:.4}  BER={:.6}  mean_iters={:.2}",
            c.frames_per_sec, c.fer, c.ber, c.mean_iters
        );
        eprintln!("Prior reference: 1.617 fps");
    }

    // Delta report.
    if let Some(ref cmp) = compare_path {
        let baseline = load_csv_results(cmp);
        if baseline.is_empty() {
            eprintln!(
                "Warning: could not load comparison baseline from {}",
                cmp.display()
            );
        } else {
            eprintln!("\n--- Delta vs {} ---", cmp.display());
            eprintln!(
                "{:<6} {:<8} {:>7} {:<28} {:<14} {:>9} {:>9} {:>9}",
                "rate", "mod", "Es/N0", "decoder", "demap", "fps_new", "fps_ref", "delta%"
            );
            for r in &results {
                let bline = baseline.iter().find(|b| {
                    b.rate == r.rate
                        && b.modulation == r.modulation
                        && (b.es_n0_db - r.es_n0_db).abs() < 0.01
                        && b.decoder == r.decoder
                        && b.demap == r.demap
                });
                if let Some(b) = bline {
                    let delta_pct =
                        (r.frames_per_sec - b.frames_per_sec) / b.frames_per_sec * 100.0;
                    eprintln!(
                        "{:<6} {:<8} {:>7.2} {:<28} {:<14} {:>9.3} {:>9.3} {:>+9.1}%",
                        r.rate,
                        r.modulation,
                        r.es_n0_db,
                        r.decoder,
                        r.demap,
                        r.frames_per_sec,
                        b.frames_per_sec,
                        delta_pct,
                    );
                } else {
                    eprintln!(
                        "{:<6} {:<8} {:>7.2} {:<28} {:<14} {:>9.3} {:>9} {:>9}",
                        r.rate,
                        r.modulation,
                        r.es_n0_db,
                        r.decoder,
                        r.demap,
                        r.frames_per_sec,
                        "(no ref)",
                        "N/A",
                    );
                }
            }
        }
    }
}
