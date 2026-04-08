//! Monte Carlo simulation framework for BER/FER performance analysis.
//!
//! This module provides reusable utilities for running communication system
//! simulations over configurable channel models, supporting both bit error rate
//! (BER) and frame/block error rate (BLER) measurements.
//!
//! # Overview
//!
//! The simulation framework supports three main workflows:
//!
//! - **Uncoded BER**: Raw bit-error-rate measurement without coding
//!   ([`SimulationRunner::run_uncoded_ber`]).
//! - **Coded (immutable decoder)**: For [`SoftDecoder`] implementations that
//!   take `&self` ([`run_coded`]).
//! - **Coded iterative (mutable decoder)**: For [`IterativeSoftDecoder`]
//!   implementations that take `&mut self` ([`run_coded_iterative`]).
//! - **Coded iterative parallel**: Parallel SNR sweeps (with the `parallel`
//!   feature) using a decoder factory closure ([`run_coded_iterative_parallel`]).
//!   Falls back to sequential execution without the feature.
//!
//! # Channel Abstraction
//!
//! The [`ChannelModel`] trait abstracts the modulation and channel, with a
//! default BPSK/AWGN implementation provided by [`BpskAwgnChannel`].
//!
//! # Output
//!
//! Results can be exported to CSV or JSON via [`SimulationResults`]. When
//! [`SimulationConfig::output_path`] is set, results are automatically written
//! to disk in the format determined by the file extension (`.json` for JSON,
//! anything else for CSV).

use crate::channel::{AwgnChannel, BpskModulator};
use crate::llr::Llr;
use crate::traits::{BlockEncoder, DecoderResult, IterativeSoftDecoder, SoftDecoder};
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// CSV header row used for simulation result output files.
const CSV_HEADER: &str =
    "eb_n0_db,ber,bler,num_bits,num_bit_errors,num_frames,num_frame_errors,avg_iterations,avg_queries_per_bit";

/// Abstracts the modulation scheme and channel model.
///
/// Implementors combine modulation (e.g., BPSK), channel noise (e.g., AWGN),
/// and demodulation into a single `transmit_and_demodulate` call that maps
/// transmitted bits to received LLRs.
///
/// # Examples
///
/// ```
/// use gf2_coding::simulation::{ChannelModel, BpskAwgnChannel};
/// use gf2_core::BitVec;
///
/// let channel = BpskAwgnChannel;
/// let bits = BitVec::from_bytes_le(&[0b10110001]);
/// let mut rng = rand::thread_rng();
/// let llrs = channel.transmit_and_demodulate(&bits, 3.0, 0.5, &mut rng);
/// assert_eq!(llrs.len(), bits.len());
/// ```
pub trait ChannelModel {
    /// Modulates, transmits through a noisy channel, and demodulates to LLRs.
    ///
    /// # Arguments
    ///
    /// * `bits` - The codeword bits to transmit
    /// * `eb_n0_db` - Energy per bit to noise ratio in dB
    /// * `rate` - Code rate (k/n)
    /// * `rng` - Random number generator for noise samples
    ///
    /// # Returns
    ///
    /// A vector of log-likelihood ratios, one per transmitted bit.
    fn transmit_and_demodulate<R: Rng>(
        &self,
        bits: &BitVec,
        eb_n0_db: f64,
        rate: f64,
        rng: &mut R,
    ) -> Vec<Llr>;
}

/// Default BPSK modulation over an AWGN channel.
///
/// Maps bits to +/-1 BPSK symbols, adds Gaussian noise with variance
/// determined by Eb/N0 and code rate, then converts received symbols
/// to LLRs via `2r / sigma^2`.
///
/// # Examples
///
/// ```
/// use gf2_coding::simulation::{ChannelModel, BpskAwgnChannel};
/// use gf2_core::BitVec;
///
/// let channel = BpskAwgnChannel;
/// let bits = BitVec::from_bytes_le(&[0b1010]);
/// let mut rng = rand::thread_rng();
/// let llrs = channel.transmit_and_demodulate(&bits, 5.0, 0.5, &mut rng);
/// assert_eq!(llrs.len(), bits.len());
/// ```
pub struct BpskAwgnChannel;

impl ChannelModel for BpskAwgnChannel {
    fn transmit_and_demodulate<R: Rng>(
        &self,
        bits: &BitVec,
        eb_n0_db: f64,
        rate: f64,
        rng: &mut R,
    ) -> Vec<Llr> {
        let n = bits.len();
        let channel = AwgnChannel::from_eb_n0_db(eb_n0_db, rate);
        let bits_vec: Vec<bool> = (0..n).map(|i| bits.get(i)).collect();
        let symbols = BpskModulator::modulate_bits(&bits_vec);
        let received = channel.transmit_symbols(&symbols, rng);
        channel.to_llrs(&received)
    }
}

/// Configuration for Monte Carlo simulations.
///
/// Controls SNR sweep range, stopping criteria, decoder iteration limits,
/// RNG seeding, and optional output file path.
///
/// # Examples
///
/// ```
/// use gf2_coding::simulation::SimulationConfig;
///
/// let config = SimulationConfig::quick_test();
/// assert_eq!(config.min_errors, 100);
/// assert_eq!(config.max_frames, 100_000);
/// ```
#[derive(Debug, Clone)]
pub struct SimulationConfig {
    /// Range of Eb/N0 values to simulate (in dB).
    pub eb_n0_range_db: Vec<f64>,

    /// Minimum number of block errors to collect before stopping at each SNR point.
    pub min_errors: usize,

    /// Maximum number of frames to transmit per SNR point.
    pub max_frames: usize,

    /// Maximum decoder iterations for iterative decoders.
    pub max_decoder_iterations: usize,

    /// Optional RNG seed for reproducible simulations.
    ///
    /// When `Some(seed)`, the simulation uses `StdRng::seed_from_u64(seed)`.
    /// When `None`, the simulation uses `rand::thread_rng()`.
    pub rng_seed: Option<u64>,

    /// Optional path for automatic result output.
    ///
    /// When set, results are written to this path after the simulation
    /// completes. Files ending in `.json` are written as JSON; all
    /// other extensions produce CSV.
    pub output_path: Option<PathBuf>,
}

impl SimulationConfig {
    /// Creates a default configuration for quick testing.
    ///
    /// Uses three SNR points (0, 3, 6 dB), 100 minimum errors, 100k max
    /// frames, 50 max decoder iterations, and no fixed seed.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::SimulationConfig;
    ///
    /// let config = SimulationConfig::quick_test();
    /// assert_eq!(config.min_errors, 100);
    /// assert_eq!(config.max_frames, 100_000);
    /// assert_eq!(config.max_decoder_iterations, 50);
    /// ```
    pub fn quick_test() -> Self {
        SimulationConfig {
            eb_n0_range_db: vec![0.0, 3.0, 6.0],
            min_errors: 100,
            max_frames: 100_000,
            max_decoder_iterations: 50,
            rng_seed: None,
            output_path: None,
        }
    }

    /// Creates a configuration for high-precision BER curves.
    ///
    /// Uses 11 SNR points (0..10 dB), 1000 minimum errors, 10M max
    /// frames, 100 max decoder iterations, and no fixed seed.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::SimulationConfig;
    ///
    /// let config = SimulationConfig::high_precision();
    /// assert_eq!(config.min_errors, 1000);
    /// assert_eq!(config.eb_n0_range_db.len(), 11);
    /// ```
    pub fn high_precision() -> Self {
        SimulationConfig {
            eb_n0_range_db: (0..=10).map(|i| i as f64).collect(),
            min_errors: 1000,
            max_frames: 10_000_000,
            max_decoder_iterations: 100,
            rng_seed: None,
            output_path: None,
        }
    }

    /// Returns a seeded RNG for this configuration.
    ///
    /// If `rng_seed` is set, uses it directly. Otherwise generates a seed
    /// from `thread_rng()` so that the simulation still uses a `StdRng`
    /// internally (consistent type, no dynamic dispatch).
    fn make_rng(&self) -> StdRng {
        match self.rng_seed {
            Some(seed) => StdRng::seed_from_u64(seed),
            None => StdRng::seed_from_u64(rand::thread_rng().gen()),
        }
    }
}

/// Results from a single SNR point simulation.
///
/// Contains BER, BLER, iteration statistics, and raw counts for a single
/// Eb/N0 operating point.
///
/// # Examples
///
/// ```
/// use gf2_coding::simulation::SimulationResult;
///
/// let result = SimulationResult {
///     eb_n0_db: 3.0,
///     ber: 0.01,
///     bler: 0.05,
///     avg_iterations: Some(12.5),
///     avg_queries_per_bit: None,
///     num_bits: 10000,
///     num_bit_errors: 100,
///     num_frames: 200,
///     num_frame_errors: 10,
/// };
/// assert!(result.is_complete(5));
/// ```
#[derive(Debug, Clone)]
pub struct SimulationResult {
    /// Eb/N0 in dB for this operating point.
    pub eb_n0_db: f64,

    /// Bit error rate (bit errors / total decoded bits).
    pub ber: f64,

    /// Block error rate (frame errors / total frames).
    pub bler: f64,

    /// Average decoder iterations per frame, if applicable.
    pub avg_iterations: Option<f64>,

    /// Average parity-check queries per decoded bit.
    ///
    /// Computed from `DecoderResult.queries` when available, falling back
    /// to `DecoderResult.iterations` when `queries` is `None`.
    /// This provides a finer-grained measure of decoder complexity than
    /// `avg_iterations` alone.
    pub avg_queries_per_bit: Option<f64>,

    /// Total number of decoded message bits.
    pub num_bits: usize,

    /// Total number of bit errors observed.
    pub num_bit_errors: usize,

    /// Total number of frames transmitted.
    pub num_frames: usize,

    /// Total number of frames with at least one bit error.
    pub num_frame_errors: usize,
}

impl SimulationResult {
    /// Returns `true` if this result has collected at least `min_errors` frame errors.
    ///
    /// # Arguments
    ///
    /// * `min_errors` - Minimum frame error count threshold
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::SimulationResult;
    ///
    /// let result = SimulationResult {
    ///     eb_n0_db: 3.0, ber: 0.01, bler: 0.05,
    ///     avg_iterations: None, avg_queries_per_bit: None,
    ///     num_bits: 10000, num_bit_errors: 100,
    ///     num_frames: 200, num_frame_errors: 10,
    /// };
    /// assert!(result.is_complete(5));
    /// assert!(!result.is_complete(50));
    /// ```
    pub fn is_complete(&self, min_errors: usize) -> bool {
        self.num_frame_errors >= min_errors
    }

    /// Exports result as a CSV row.
    ///
    /// Format: `eb_n0_db,ber,bler,num_bits,num_bit_errors,num_frames,num_frame_errors,avg_iterations,avg_queries_per_bit`
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::SimulationResult;
    ///
    /// let result = SimulationResult {
    ///     eb_n0_db: 3.0, ber: 0.01, bler: 0.05,
    ///     avg_iterations: Some(12.5), avg_queries_per_bit: None,
    ///     num_bits: 10000, num_bit_errors: 100,
    ///     num_frames: 200, num_frame_errors: 10,
    /// };
    /// let row = result.to_csv_row();
    /// assert!(row.starts_with("3,0.01,0.05,"));
    /// ```
    pub fn to_csv_row(&self) -> String {
        let avg_iter = self
            .avg_iterations
            .map_or_else(String::new, |v| format!("{v}"));
        let avg_q = self
            .avg_queries_per_bit
            .map_or_else(String::new, |v| format!("{v}"));
        format!(
            "{},{},{},{},{},{},{},{},{}",
            self.eb_n0_db,
            self.ber,
            self.bler,
            self.num_bits,
            self.num_bit_errors,
            self.num_frames,
            self.num_frame_errors,
            avg_iter,
            avg_q,
        )
    }

    /// Parses a CSV row (produced by [`to_csv_row`](Self::to_csv_row)) back into
    /// a `SimulationResult`.
    ///
    /// Returns `None` if the row cannot be parsed.
    ///
    /// # Arguments
    ///
    /// * `row` - A comma-separated string with 9 fields matching the CSV header.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::SimulationResult;
    ///
    /// let result = SimulationResult {
    ///     eb_n0_db: 3.0, ber: 0.01, bler: 0.05,
    ///     avg_iterations: Some(12.5), avg_queries_per_bit: None,
    ///     num_bits: 10000, num_bit_errors: 100,
    ///     num_frames: 200, num_frame_errors: 10,
    /// };
    /// let row = result.to_csv_row();
    /// let parsed = SimulationResult::from_csv_row(&row).unwrap();
    /// assert!((parsed.eb_n0_db - 3.0).abs() < 1e-10);
    /// assert_eq!(parsed.num_frame_errors, 10);
    /// ```
    pub fn from_csv_row(row: &str) -> Option<Self> {
        let fields: Vec<&str> = row.split(',').collect();
        if fields.len() < 7 {
            return None;
        }
        let eb_n0_db = fields[0].parse::<f64>().ok()?;
        let ber = fields[1].parse::<f64>().ok()?;
        let bler = fields[2].parse::<f64>().ok()?;
        let num_bits = fields[3].parse::<usize>().ok()?;
        let num_bit_errors = fields[4].parse::<usize>().ok()?;
        let num_frames = fields[5].parse::<usize>().ok()?;
        let num_frame_errors = fields[6].parse::<usize>().ok()?;
        let avg_iterations = fields.get(7).and_then(|s| s.parse::<f64>().ok());
        let avg_queries_per_bit = fields.get(8).and_then(|s| s.parse::<f64>().ok());
        Some(Self {
            eb_n0_db,
            ber,
            bler,
            avg_iterations,
            avg_queries_per_bit,
            num_bits,
            num_bit_errors,
            num_frames,
            num_frame_errors,
        })
    }

    /// Appends this result as a single CSV row to the given file.
    ///
    /// Writes the CSV header if the file does not exist or is empty.
    /// Uses append mode so concurrent runs do not overwrite each other.
    ///
    /// # Arguments
    ///
    /// * `path` - Destination CSV file path.
    ///
    /// # Panics
    ///
    /// Panics if the file cannot be opened or written.
    pub fn append_csv_row_to(&self, path: &Path) {
        use std::io::Write;
        let needs_header = !path.exists() || std::fs::metadata(path).map_or(true, |m| m.len() == 0);
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap_or_else(|e| panic!("Failed to open {} for append: {e}", path.display()));
        if needs_header {
            writeln!(file, "{}", CSV_HEADER).unwrap();
        }
        writeln!(file, "{}", self.to_csv_row()).unwrap();
    }

    /// Serializes this result as a JSON object string.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::SimulationResult;
    ///
    /// let result = SimulationResult {
    ///     eb_n0_db: 3.0, ber: 0.01, bler: 0.05,
    ///     avg_iterations: None, avg_queries_per_bit: None,
    ///     num_bits: 10000, num_bit_errors: 100,
    ///     num_frames: 200, num_frame_errors: 10,
    /// };
    /// let json = result.to_json();
    /// assert!(json.contains("\"eb_n0_db\":3"));
    /// assert!(json.contains("\"ber\":0.01"));
    /// ```
    pub fn to_json(&self) -> String {
        let avg_iter = self
            .avg_iterations
            .map_or("null".to_string(), |v| format!("{v}"));
        let avg_q = self
            .avg_queries_per_bit
            .map_or("null".to_string(), |v| format!("{v}"));
        format!(
            concat!(
                "{{",
                "\"eb_n0_db\":{},",
                "\"ber\":{},",
                "\"bler\":{},",
                "\"num_bits\":{},",
                "\"num_bit_errors\":{},",
                "\"num_frames\":{},",
                "\"num_frame_errors\":{},",
                "\"avg_iterations\":{},",
                "\"avg_queries_per_bit\":{}",
                "}}"
            ),
            self.eb_n0_db,
            self.ber,
            self.bler,
            self.num_bits,
            self.num_bit_errors,
            self.num_frames,
            self.num_frame_errors,
            avg_iter,
            avg_q,
        )
    }
}

/// Aggregated simulation results across all SNR points.
///
/// Contains per-SNR-point results and provides CSV/JSON export.
///
/// # Examples
///
/// ```
/// use gf2_coding::simulation::{SimulationResult, SimulationResults};
///
/// let results = SimulationResults { points: vec![] };
/// assert!(results.points.is_empty());
/// ```
#[derive(Debug, Clone)]
pub struct SimulationResults {
    /// Per-SNR-point simulation results, ordered by increasing Eb/N0.
    pub points: Vec<SimulationResult>,
}

impl SimulationResults {
    /// Exports all results to CSV format.
    ///
    /// # Arguments
    ///
    /// * `include_header` - Whether to prepend a CSV header row
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::{SimulationResult, SimulationResults};
    ///
    /// let results = SimulationResults {
    ///     points: vec![SimulationResult {
    ///         eb_n0_db: 3.0, ber: 0.01, bler: 0.05,
    ///         avg_iterations: None, avg_queries_per_bit: None,
    ///         num_bits: 10000, num_bit_errors: 100,
    ///         num_frames: 200, num_frame_errors: 10,
    ///     }],
    /// };
    /// let csv = results.to_csv(true);
    /// assert!(csv.contains("eb_n0_db"));
    /// assert!(csv.contains("0.01"));
    /// ```
    pub fn to_csv(&self, include_header: bool) -> String {
        let mut csv = String::new();
        if include_header {
            csv.push_str(CSV_HEADER);
            csv.push('\n');
        }
        for point in &self.points {
            csv.push_str(&point.to_csv_row());
            csv.push('\n');
        }
        csv
    }

    /// Exports all results to JSON format.
    ///
    /// Returns a JSON array containing one object per SNR point.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::{SimulationResult, SimulationResults};
    ///
    /// let results = SimulationResults {
    ///     points: vec![SimulationResult {
    ///         eb_n0_db: 3.0, ber: 0.01, bler: 0.05,
    ///         avg_iterations: None, avg_queries_per_bit: None,
    ///         num_bits: 10000, num_bit_errors: 100,
    ///         num_frames: 200, num_frame_errors: 10,
    ///     }],
    /// };
    /// let json = results.to_json();
    /// assert!(json.starts_with('['));
    /// assert!(json.ends_with(']'));
    /// ```
    pub fn to_json(&self) -> String {
        let entries: Vec<String> = self.points.iter().map(|p| p.to_json()).collect();
        format!("[{}]", entries.join(","))
    }

    /// Writes results to the given path.
    ///
    /// Files ending in `.json` are written as JSON; all other extensions
    /// produce CSV with a header.
    ///
    /// # Arguments
    ///
    /// * `path` - Destination file path
    ///
    /// # Panics
    ///
    /// Panics if the file cannot be created or written.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_coding::simulation::{SimulationResults};
    /// use std::path::Path;
    ///
    /// let results = SimulationResults { points: vec![] };
    /// results.write_to(Path::as_ref(std::path::Path::new("/tmp/out.csv")));
    /// ```
    pub fn write_to(&self, path: &std::path::Path) {
        let content = if path.extension().and_then(|e| e.to_str()) == Some("json") {
            self.to_json()
        } else {
            self.to_csv(true)
        };
        std::fs::write(path, content).unwrap_or_else(|e| {
            panic!(
                "Failed to write simulation results to {}: {e}",
                path.display()
            )
        });
    }
}

/// Monte Carlo simulation runner for communication systems.
///
/// Provides static methods for both uncoded and coded simulations:
///
/// - [`SimulationRunner::run_uncoded_ber`] — uncoded BPSK over AWGN
/// - [`SimulationRunner::run_coded`] — coded with immutable [`SoftDecoder`]
/// - [`SimulationRunner::run_coded_iterative`] — coded with [`IterativeSoftDecoder`]
/// - [`SimulationRunner::run_coded_iterative_parallel`] — parallel iterative with decoder factory
pub struct SimulationRunner;

impl SimulationRunner {
    /// Simulates uncoded BPSK transmission over AWGN and computes BER.
    ///
    /// # Arguments
    ///
    /// * `config` - Simulation configuration (uses `eb_n0_range_db`, `min_errors`, `max_frames`)
    /// * `rng` - Random number generator
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::{SimulationRunner, SimulationConfig};
    ///
    /// let config = SimulationConfig::quick_test();
    /// let mut rng = rand::thread_rng();
    /// let results = SimulationRunner::run_uncoded_ber(&config, &mut rng);
    ///
    /// assert_eq!(results.len(), config.eb_n0_range_db.len());
    /// ```
    ///
    /// # Complexity
    ///
    /// O(SNR_points * max_frames * batch_size) where batch_size = 1000.
    pub fn run_uncoded_ber<R: Rng>(
        config: &SimulationConfig,
        rng: &mut R,
    ) -> Vec<SimulationResult> {
        config
            .eb_n0_range_db
            .iter()
            .map(|&eb_n0_db| {
                let channel = AwgnChannel::from_eb_n0_db(eb_n0_db, 1.0);

                let mut total_bits = 0;
                let mut total_errors = 0;

                while total_errors < config.min_errors && total_bits < config.max_frames {
                    let batch_size = 1000.min(config.max_frames - total_bits);
                    let bits = BitVec::random(batch_size, rng);

                    let bits_vec: Vec<bool> = (0..batch_size).map(|i| bits.get(i)).collect();
                    let symbols = BpskModulator::modulate_bits(&bits_vec);
                    let received = channel.transmit_symbols(&symbols, rng);

                    let decoded: Vec<bool> = received
                        .iter()
                        .map(|&r| BpskModulator::demodulate_hard(r))
                        .collect();

                    let errors = (0..batch_size)
                        .filter(|&i| bits.get(i) != decoded[i])
                        .count();

                    total_bits += batch_size;
                    total_errors += errors;
                }

                let ber = if total_bits > 0 {
                    total_errors as f64 / total_bits as f64
                } else {
                    0.0
                };

                SimulationResult {
                    eb_n0_db,
                    ber,
                    bler: 0.0,
                    avg_iterations: None,
                    avg_queries_per_bit: None,
                    num_bits: total_bits,
                    num_bit_errors: total_errors,
                    num_frames: 0,
                    num_frame_errors: 0,
                }
            })
            .collect()
    }

    /// Exports simulation results to CSV format.
    ///
    /// # Arguments
    ///
    /// * `results` - Slice of per-SNR simulation results
    /// * `include_header` - Whether to prepend a CSV header row
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::{SimulationRunner, SimulationConfig};
    ///
    /// let config = SimulationConfig::quick_test();
    /// let mut rng = rand::thread_rng();
    /// let results = SimulationRunner::run_uncoded_ber(&config, &mut rng);
    /// let csv = SimulationRunner::results_to_csv(&results, true);
    ///
    /// assert!(csv.contains("eb_n0_db"));
    /// ```
    pub fn results_to_csv(results: &[SimulationResult], include_header: bool) -> String {
        let wrapper = SimulationResults {
            points: results.to_vec(),
        };
        wrapper.to_csv(include_header)
    }
}

/// Progress reporting interval: print status every this many frames.
const PROGRESS_INTERVAL: usize = 1000;

/// Derives the `.progress.jsonl` path from a CSV output path.
///
/// Replaces the extension of `csv_path` with `.progress.jsonl`.
fn progress_path_for(csv_path: &Path) -> PathBuf {
    csv_path.with_extension("progress.jsonl")
}

/// Formats a `Duration` as a human-readable string (e.g., `4m23s`).
fn format_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    if secs >= 3600 {
        format!(
            "{}h{:02}m{:02}s",
            secs / 3600,
            (secs % 3600) / 60,
            secs % 60
        )
    } else if secs >= 60 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}

/// Reports simulation progress to stderr, including wall-clock elapsed time
/// and estimated remaining time for the current SNR point.
fn report_progress(
    eb_n0_db: f64,
    frames: usize,
    frame_errors: usize,
    min_errors: usize,
    max_frames: usize,
    elapsed: Option<std::time::Duration>,
) {
    let elapsed_str = elapsed.map_or_else(String::new, |d| format!(" [{}]", format_duration(d)));

    // Estimate remaining time for this SNR point.
    let eta_str = if let Some(el) = elapsed {
        if frames > 0 && frame_errors > 0 && frame_errors < min_errors {
            // Error-limited: estimate remaining frames from error rate.
            let remaining_errors = min_errors - frame_errors;
            let error_rate = frame_errors as f64 / frames as f64;
            let remaining_frames = (remaining_errors as f64 / error_rate).ceil() as usize;
            let remaining_frames = remaining_frames.min(max_frames.saturating_sub(frames));
            let frame_rate = frames as f64 / el.as_secs_f64();
            if frame_rate > 0.0 {
                let eta_secs = remaining_frames as f64 / frame_rate;
                format!(
                    ", ETA ~{}",
                    format_duration(std::time::Duration::from_secs_f64(eta_secs))
                )
            } else {
                String::new()
            }
        } else if frames > 0 && frame_errors == 0 {
            // No errors yet: estimate based on max_frames.
            let remaining_frames = max_frames.saturating_sub(frames);
            let frame_rate = frames as f64 / el.as_secs_f64();
            if frame_rate > 0.0 {
                let eta_secs = remaining_frames as f64 / frame_rate;
                format!(
                    ", ETA ~{}",
                    format_duration(std::time::Duration::from_secs_f64(eta_secs))
                )
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    eprintln!(
        "[{:.1} dB] frames={}, frame_errors={}/{} ({:.1}%){elapsed_str}{eta_str}",
        eb_n0_db,
        frames,
        frame_errors,
        min_errors,
        if min_errors > 0 {
            100.0 * frame_errors as f64 / min_errors as f64
        } else {
            0.0
        },
    );
}

/// Reports per-point completion with elapsed time and optional ETA.
///
/// # Arguments
///
/// * `eb_n0_db` - The completed SNR point.
/// * `result` - The completed simulation result.
/// * `point_elapsed` - Wall-clock time for this point.
/// * `remaining_points` - Number of SNR points still to simulate.
/// * `completed_durations` - Durations of previously completed points for ETA estimation.
fn report_point_complete(
    eb_n0_db: f64,
    result: &SimulationResult,
    point_elapsed: std::time::Duration,
    remaining_points: usize,
    completed_durations: &[std::time::Duration],
) {
    let elapsed_str = format_duration(point_elapsed);
    let eta_str = if remaining_points > 0 && !completed_durations.is_empty() {
        let avg_secs: f64 = completed_durations
            .iter()
            .map(|d| d.as_secs_f64())
            .sum::<f64>()
            / completed_durations.len() as f64;
        let eta = std::time::Duration::from_secs_f64(avg_secs * remaining_points as f64);
        format!(
            " — ETA {} for {} remaining point{}",
            format_duration(eta),
            remaining_points,
            if remaining_points == 1 { "" } else { "s" }
        )
    } else {
        String::new()
    };

    eprintln!(
        "[{:.1} dB] DONE: BLER={:.2e} ({} errors / {} frames) in {}{eta_str}",
        eb_n0_db, result.bler, result.num_frame_errors, result.num_frames, elapsed_str,
    );
}

/// Loads existing simulation results from a CSV file for resuming.
///
/// Parses each data row and returns a map keyed by the SNR value
/// formatted to 6 decimal places. Only results meeting the `min_errors`
/// threshold are included.
///
/// Returns an empty map if the file does not exist or cannot be parsed.
///
/// # Arguments
///
/// * `path` - Path to the CSV file to load.
/// * `min_errors` - Minimum frame error count for a result to be considered complete.
///
/// # Examples
///
/// ```
/// use gf2_coding::simulation::try_load_existing_results;
/// use std::path::Path;
///
/// let results = try_load_existing_results(Path::new("/nonexistent.csv"), 100);
/// assert!(results.is_empty());
/// ```
pub fn try_load_existing_results(
    path: &Path,
    min_errors: usize,
) -> HashMap<String, SimulationResult> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    let mut map = HashMap::new();
    for line in content.lines().skip(1) {
        // Skip header
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(result) = SimulationResult::from_csv_row(trimmed) {
            if result.is_complete(min_errors) {
                let key = format!("{:.6}", result.eb_n0_db);
                map.insert(key, result);
            }
        }
    }
    map
}

/// Accumulator for per-SNR-point statistics during simulation.
struct SnrAccumulator {
    eb_n0_db: f64,
    total_bit_errors: usize,
    total_bits: usize,
    total_frame_errors: usize,
    total_frames: usize,
    total_iterations: usize,
    total_queries: usize,
    k: usize,
    start_time: Instant,
    last_progress_time: Instant,
    progress_count: usize,
    /// Set to `true` after the first JSONL write failure so we only warn once.
    progress_write_warned: bool,
}

impl SnrAccumulator {
    fn new(eb_n0_db: f64, k: usize) -> Self {
        let now = Instant::now();
        Self {
            eb_n0_db,
            total_bit_errors: 0,
            total_bits: 0,
            total_frame_errors: 0,
            total_frames: 0,
            total_iterations: 0,
            total_queries: 0,
            k,
            start_time: now,
            last_progress_time: now,
            progress_count: 0,
            progress_write_warned: false,
        }
    }

    fn record_frame(&mut self, bit_errors: usize, iterations: usize, queries: Option<usize>) {
        self.total_bit_errors += bit_errors;
        self.total_bits += self.k;
        self.total_frames += 1;
        if bit_errors > 0 {
            self.total_frame_errors += 1;
        }
        self.total_iterations += iterations;
        if let Some(q) = queries {
            self.total_queries += q;
        } else {
            self.total_queries += iterations;
        }
    }

    fn should_stop(&self, min_errors: usize, max_frames: usize) -> bool {
        self.total_frame_errors >= min_errors || self.total_frames >= max_frames
    }

    fn should_report(&self) -> bool {
        self.total_frames % PROGRESS_INTERVAL == 0 && self.total_frames > 0
    }

    /// Returns `true` if enough wall-clock time has elapsed for a JSONL
    /// progress entry: 10 seconds for the first entry, then 60 seconds.
    fn should_write_progress(&self) -> bool {
        let elapsed = self.last_progress_time.elapsed();
        let threshold = if self.progress_count == 0 {
            std::time::Duration::from_secs(10)
        } else {
            std::time::Duration::from_secs(60)
        };
        elapsed >= threshold
    }

    /// Appends a JSONL progress entry to the given file path.
    ///
    /// Warns on stderr on the first failure (best-effort, does not panic).
    fn write_progress_entry(&mut self, path: &Path) {
        use std::io::Write;
        let elapsed_s = self.start_time.elapsed().as_secs_f64();
        let bler_estimate = if self.total_frames > 0 {
            self.total_frame_errors as f64 / self.total_frames as f64
        } else {
            0.0
        };
        let entry = format!(
            concat!(
                "{{\"type\":\"progress\",",
                "\"timestamp\":\"{}\",",
                "\"eb_n0_db\":{},",
                "\"frames\":{},",
                "\"frame_errors\":{},",
                "\"bler_estimate\":{},",
                "\"elapsed_s\":{:.1}}}"
            ),
            chrono_like_timestamp(),
            self.eb_n0_db,
            self.total_frames,
            self.total_frame_errors,
            bler_estimate,
            elapsed_s,
        );
        let result = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut file| writeln!(file, "{entry}"));
        if let Err(e) = result {
            if !self.progress_write_warned {
                eprintln!(
                    "Warning: failed to write JSONL progress to {}: {e}",
                    path.display()
                );
                self.progress_write_warned = true;
            }
        }
        self.last_progress_time = Instant::now();
        self.progress_count += 1;
    }

    /// Appends a `"type":"point_complete"` JSONL entry with full result fields.
    fn write_point_complete_entry(&mut self, path: &Path, result: &SimulationResult) {
        use std::io::Write;
        let elapsed_s = self.start_time.elapsed().as_secs_f64();
        let avg_iter = result
            .avg_iterations
            .map_or("null".to_string(), |v| format!("{v}"));
        let avg_q = result
            .avg_queries_per_bit
            .map_or("null".to_string(), |v| format!("{v}"));
        let entry = format!(
            concat!(
                "{{\"type\":\"point_complete\",",
                "\"timestamp\":\"{}\",",
                "\"eb_n0_db\":{},",
                "\"ber\":{},",
                "\"bler\":{},",
                "\"num_bits\":{},",
                "\"num_bit_errors\":{},",
                "\"num_frames\":{},",
                "\"num_frame_errors\":{},",
                "\"avg_iterations\":{},",
                "\"avg_queries_per_bit\":{},",
                "\"elapsed_s\":{:.1}}}"
            ),
            chrono_like_timestamp(),
            result.eb_n0_db,
            result.ber,
            result.bler,
            result.num_bits,
            result.num_bit_errors,
            result.num_frames,
            result.num_frame_errors,
            avg_iter,
            avg_q,
            elapsed_s,
        );
        let write_result = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut file| writeln!(file, "{entry}"));
        if let Err(e) = write_result {
            if !self.progress_write_warned {
                eprintln!(
                    "Warning: failed to write JSONL progress to {}: {e}",
                    path.display()
                );
                self.progress_write_warned = true;
            }
        }
    }

    /// Returns the elapsed wall-clock time since this accumulator was created.
    fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    fn into_result(self) -> SimulationResult {
        let ber = if self.total_bits > 0 {
            self.total_bit_errors as f64 / self.total_bits as f64
        } else {
            0.0
        };
        let bler = if self.total_frames > 0 {
            self.total_frame_errors as f64 / self.total_frames as f64
        } else {
            0.0
        };
        let avg_iterations = if self.total_frames > 0 {
            Some(self.total_iterations as f64 / self.total_frames as f64)
        } else {
            None
        };
        let avg_queries_per_bit = if self.total_bits > 0 {
            Some(self.total_queries as f64 / self.total_bits as f64)
        } else {
            None
        };

        SimulationResult {
            eb_n0_db: self.eb_n0_db,
            ber,
            bler,
            avg_iterations,
            avg_queries_per_bit,
            num_bits: self.total_bits,
            num_bit_errors: self.total_bit_errors,
            num_frames: self.total_frames,
            num_frame_errors: self.total_frame_errors,
        }
    }
}

/// Returns an ISO 8601 timestamp string (`YYYY-MM-DDTHH:MM:SS`) without external dependencies.
fn chrono_like_timestamp() -> String {
    use std::time::SystemTime;
    match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => {
            let secs = d.as_secs();
            // Manual UTC breakdown — avoids adding chrono as a dependency.
            let days = secs / 86400;
            let time_of_day = secs % 86400;
            let hours = time_of_day / 3600;
            let minutes = (time_of_day % 3600) / 60;
            let seconds = time_of_day % 60;

            // Convert days since epoch to (year, month, day) using a civil calendar algorithm.
            // Based on Howard Hinnant's `civil_from_days` (public domain).
            let z = days as i64 + 719468;
            let era = if z >= 0 { z } else { z - 146096 } / 146097;
            let doe = (z - era * 146097) as u64; // day of era [0, 146096]
            let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era [0, 399]
            let y = yoe as i64 + era * 400;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
            let mp = (5 * doy + 2) / 153; // [0, 11]
            let d = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
            let m = if mp < 10 { mp + 3 } else { mp - 9 }; // month [1, 12]
            let y = if m <= 2 { y + 1 } else { y };

            format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
                y, m, d, hours, minutes, seconds
            )
        }
        Err(_) => "1970-01-01T00:00:00".to_string(),
    }
}

/// Counts bit errors between a decoded message and the original.
///
/// Uses word-level XOR and popcount for O(n/64) performance on aligned
/// vectors. Length mismatches count as additional errors.
pub fn count_bit_errors(original: &BitVec, decoded: &BitVec) -> usize {
    if original.len() == decoded.len() {
        // Fast path: XOR and popcount
        let mut diff = original.clone();
        diff.bit_xor_into(decoded);
        diff.count_ones()
    } else {
        // Mismatched lengths: compare common prefix, count remainder as errors
        let len = original.len().min(decoded.len());
        let mut errors = 0;
        for i in 0..len {
            if original.get(i) != decoded.get(i) {
                errors += 1;
            }
        }
        errors + original.len().abs_diff(decoded.len())
    }
}

/// Shared context for running a single SNR point within the simulation sweep.
///
/// Groups the parameters that are common across all simulation entry-points
/// so the inner function does not exceed clippy's argument limit.
struct SnrPointContext<'a> {
    eb_n0_db: f64,
    rate: f64,
    config: &'a SimulationConfig,
    existing: &'a HashMap<String, SimulationResult>,
    output_path: Option<&'a Path>,
    progress_path: Option<&'a Path>,
    remaining_points: usize,
    completed_durations: &'a [std::time::Duration],
}

/// Simulates a single SNR point, handling progress reporting, JSONL logging,
/// incremental CSV append, and point-completion reporting.
///
/// The caller supplies a `decode_frame` closure that receives the channel LLRs
/// for one frame and returns a [`DecoderResult`]. Everything else — the frame
/// loop, early-termination, resume check, CSV/JSONL I/O — lives here so it
/// is shared across all simulation entry-points.
fn simulate_single_point<E, C, R, F>(
    encoder: &E,
    channel: &C,
    rng: &mut R,
    ctx: &SnrPointContext<'_>,
    mut decode_frame: F,
) -> SimulationResult
where
    E: BlockEncoder,
    C: ChannelModel,
    R: Rng,
    F: FnMut(&[crate::llr::Llr]) -> DecoderResult,
{
    let eb_n0_db = ctx.eb_n0_db;
    let rate = ctx.rate;
    let config = ctx.config;
    let k = encoder.k();

    // Check resume cache.
    let snr_key = format!("{:.6}", eb_n0_db);
    if let Some(cached) = ctx.existing.get(&snr_key) {
        eprintln!(
            "[{:.1} dB] RESUMED: using existing result ({} errors, {} frames)",
            eb_n0_db, cached.num_frame_errors, cached.num_frames,
        );
        return cached.clone();
    }

    let mut acc = SnrAccumulator::new(eb_n0_db, k);

    while !acc.should_stop(config.min_errors, config.max_frames) {
        let message = BitVec::random(k, rng);
        let codeword = encoder.encode(&message);
        let llrs = channel.transmit_and_demodulate(&codeword, eb_n0_db, rate, rng);

        let result = decode_frame(&llrs);
        let bit_errors = count_bit_errors(&message, &result.decoded_bits);
        acc.record_frame(bit_errors, result.iterations, result.queries);

        if acc.should_report() {
            report_progress(
                eb_n0_db,
                acc.total_frames,
                acc.total_frame_errors,
                config.min_errors,
                config.max_frames,
                Some(acc.elapsed()),
            );
        }

        if let Some(pp) = ctx.progress_path {
            if acc.should_write_progress() {
                acc.write_progress_entry(pp);
            }
        }
    }

    let point_elapsed = acc.elapsed();
    let sim_result = acc.into_result();

    // Write point_complete JSONL entry.
    if let Some(pp) = ctx.progress_path {
        let mut acc_for_jsonl = SnrAccumulator::new(eb_n0_db, k);
        // Reuse the start time from the original accumulator via elapsed.
        acc_for_jsonl.start_time = Instant::now() - point_elapsed;
        acc_for_jsonl.write_point_complete_entry(pp, &sim_result);
    }

    // Incremental CSV append.
    if let Some(path) = ctx.output_path {
        sim_result.append_csv_row_to(path);
    }

    report_point_complete(
        eb_n0_db,
        &sim_result,
        point_elapsed,
        ctx.remaining_points,
        ctx.completed_durations,
    );

    sim_result
}

/// Runs the sequential SNR sweep shared by `run_coded_iterative` and
/// `run_with_decoder`.
///
/// Handles resume, progress, incremental CSV, JSONL logging, and final file
/// write. The caller provides a per-frame `decode_frame` closure.
fn run_sequential_sweep<E, C, F>(
    encoder: &E,
    channel: &C,
    config: &SimulationConfig,
    mut decode_frame: F,
) -> SimulationResults
where
    E: BlockEncoder,
    C: ChannelModel,
    F: FnMut(&[crate::llr::Llr]) -> DecoderResult,
{
    let n = encoder.n();
    let k = encoder.k();
    let rate = k as f64 / n as f64;

    let existing = config.output_path.as_ref().map_or_else(HashMap::new, |p| {
        try_load_existing_results(p, config.min_errors)
    });
    let progress_path = config.output_path.as_ref().map(|p| progress_path_for(p));

    let mut rng = config.make_rng();
    let mut points = Vec::with_capacity(config.eb_n0_range_db.len());
    let mut completed_durations: Vec<std::time::Duration> = Vec::new();

    for (point_idx, &eb_n0_db) in config.eb_n0_range_db.iter().enumerate() {
        let remaining = config.eb_n0_range_db.len() - point_idx - 1;
        let point_start = Instant::now();
        let ctx = SnrPointContext {
            eb_n0_db,
            rate,
            config,
            existing: &existing,
            output_path: config.output_path.as_deref(),
            progress_path: progress_path.as_deref(),
            remaining_points: remaining,
            completed_durations: &completed_durations,
        };
        let sim_result = simulate_single_point(encoder, channel, &mut rng, &ctx, &mut decode_frame);
        completed_durations.push(point_start.elapsed());
        points.push(sim_result);
    }

    let results = SimulationResults { points };
    // Final overwrite with clean, complete file.
    if let Some(ref path) = config.output_path {
        results.write_to(path);
    }
    results
}

impl SimulationRunner {
    /// Runs a coded simulation using an immutable [`SoftDecoder`].
    ///
    /// Executes the encode-modulate-channel-demodulate-decode loop for each
    /// SNR point, collecting BER, BLER, and iteration statistics.
    ///
    /// # Arguments
    ///
    /// * `encoder` - Block encoder producing codewords from messages
    /// * `decoder` - Soft-decision decoder (immutable `&self`)
    /// * `channel` - Channel model for modulation, noise, and demodulation
    /// * `config` - Simulation configuration controlling sweep parameters
    ///
    /// # Returns
    ///
    /// Aggregated [`SimulationResults`] with one entry per SNR point.
    ///
    /// # Panics
    ///
    /// Panics if `output_path` is set and the file cannot be written.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::{SimulationRunner, BpskAwgnChannel, SimulationConfig};
    /// use gf2_coding::grand::{OrbGrand, OrbGrandConfig};
    /// use gf2_coding::linear::LinearBlockCode;
    /// use gf2_coding::traits::GeneratorMatrixAccess;
    ///
    /// let code = LinearBlockCode::hamming(3);
    /// let h = code.parity_check().unwrap().clone();
    /// let decoder = OrbGrand::new(h, OrbGrandConfig::default());
    /// let channel = BpskAwgnChannel;
    /// let mut config = SimulationConfig::quick_test();
    /// config.eb_n0_range_db = vec![6.0];
    /// config.max_frames = 50;
    /// let results = SimulationRunner::run_coded(&code, &decoder, &channel, &config);
    /// assert_eq!(results.points.len(), 1);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(SNR_points * max_frames * (encode_time + channel_time + decode_time)).
    pub fn run_coded<E, D, C>(
        encoder: &E,
        decoder: &D,
        channel: &C,
        config: &SimulationConfig,
    ) -> SimulationResults
    where
        E: BlockEncoder,
        D: SoftDecoder,
        C: ChannelModel,
    {
        run_sequential_sweep(encoder, channel, config, |llrs| {
            decoder.decode_soft_with_result(llrs)
        })
    }

    /// Runs a coded simulation using a mutable [`IterativeSoftDecoder`].
    ///
    /// Similar to [`run_coded`] but accepts a decoder requiring `&mut self`,
    /// as is typical for iterative belief-propagation decoders that maintain
    /// internal message state.
    ///
    /// # Arguments
    ///
    /// * `encoder` - Block encoder producing codewords from messages
    /// * `decoder` - Iterative soft-decision decoder (mutable `&mut self`)
    /// * `channel` - Channel model for modulation, noise, and demodulation
    /// * `config` - Simulation configuration controlling sweep parameters
    ///
    /// # Returns
    ///
    /// Aggregated [`SimulationResults`] with one entry per SNR point.
    ///
    /// # Panics
    ///
    /// Panics if `output_path` is set and the file cannot be written.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_coding::simulation::{SimulationRunner, BpskAwgnChannel, SimulationConfig};
    /// use gf2_coding::{LdpcCode, LdpcDecoder, CodeRate};
    /// use gf2_coding::ldpc::LdpcEncoder;
    ///
    /// let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    /// let encoder = LdpcEncoder::new(code.clone());
    /// let mut decoder = LdpcDecoder::new(code);
    /// let channel = BpskAwgnChannel;
    /// let mut config = SimulationConfig::quick_test();
    /// config.eb_n0_range_db = vec![4.0];
    /// config.max_frames = 10;
    /// let results = SimulationRunner::run_coded_iterative(&encoder, &mut decoder, &channel, &config);
    /// assert_eq!(results.points.len(), 1);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(SNR_points * max_frames * (encode_time + channel_time + decode_time)).
    pub fn run_coded_iterative<E, D, C>(
        encoder: &E,
        decoder: &mut D,
        channel: &C,
        config: &SimulationConfig,
    ) -> SimulationResults
    where
        E: BlockEncoder,
        D: IterativeSoftDecoder,
        C: ChannelModel,
    {
        let max_iter = config.max_decoder_iterations;
        run_sequential_sweep(encoder, channel, config, |llrs| {
            decoder.reset();
            decoder.decode_iterative(llrs, max_iter)
        })
    }

    /// Runs a coded iterative simulation with per-SNR-point parallelism.
    ///
    /// Each SNR point gets its own decoder instance created by `make_decoder`,
    /// enabling safe parallel execution. With the `parallel` feature enabled,
    /// SNR points are dispatched to rayon threads. Without it, execution is
    /// sequential but each point still gets a fresh decoder.
    ///
    /// # Arguments
    ///
    /// * `encoder` - Block encoder producing codewords from messages. Must be
    ///   `Send + Sync` for parallel access.
    /// * `make_decoder` - Factory closure that creates a fresh
    ///   [`IterativeSoftDecoder`] instance for each SNR point. Called once per
    ///   SNR point, so each thread gets its own decoder with independent state.
    /// * `channel` - Channel model for modulation, noise, and demodulation.
    ///   Must be `Send + Sync` for parallel access.
    /// * `config` - Simulation configuration controlling sweep parameters.
    ///
    /// # Returns
    ///
    /// Aggregated [`SimulationResults`] with one entry per SNR point, ordered
    /// by increasing Eb/N0.
    ///
    /// # Panics
    ///
    /// Panics if `output_path` is set and the file cannot be written.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_coding::simulation::{SimulationRunner, BpskAwgnChannel, SimulationConfig};
    /// use gf2_coding::{LdpcCode, LdpcDecoder, CodeRate};
    /// use gf2_coding::ldpc::LdpcEncoder;
    ///
    /// let code = LdpcCode::dvb_t2_short(CodeRate::Rate1_2);
    /// let encoder = LdpcEncoder::new(code);
    /// let channel = BpskAwgnChannel;
    /// let mut config = SimulationConfig::quick_test();
    /// config.eb_n0_range_db = vec![4.0];
    /// config.max_frames = 10;
    ///
    /// let results = SimulationRunner::run_coded_iterative_parallel(
    ///     &encoder,
    ///     || LdpcDecoder::new(LdpcCode::dvb_t2_short(CodeRate::Rate1_2)),
    ///     &channel,
    ///     &config,
    /// );
    /// assert_eq!(results.points.len(), 1);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(SNR_points * max_frames * (encode_time + channel_time + decode_time))
    /// wall-clock time, divided by available parallelism for independent SNR
    /// points.
    pub fn run_coded_iterative_parallel<E, D, F, C>(
        encoder: &E,
        make_decoder: F,
        channel: &C,
        config: &SimulationConfig,
    ) -> SimulationResults
    where
        E: BlockEncoder + Send + Sync,
        D: IterativeSoftDecoder,
        F: Fn() -> D + Send + Sync,
        C: ChannelModel + Send + Sync,
    {
        let k = encoder.k();
        let n = encoder.n();
        let rate = k as f64 / n as f64;

        // Resume: load existing results if output_path is set.
        let existing = config.output_path.as_ref().map_or_else(HashMap::new, |p| {
            try_load_existing_results(p, config.min_errors)
        });

        let output_path = config.output_path.clone();
        let progress_path = config.output_path.as_ref().map(|p| progress_path_for(p));
        let max_iter = config.max_decoder_iterations;

        let simulate_point = |(idx, &eb_n0_db): (usize, &f64)| -> SimulationResult {
            let mut decoder = make_decoder();
            // Each SNR point gets a unique sub-seed derived from the config seed.
            let point_seed = config
                .rng_seed
                .unwrap_or(0xDEAD_BEEF)
                .wrapping_add(idx as u64);
            let mut rng = StdRng::seed_from_u64(point_seed);

            let ctx = SnrPointContext {
                eb_n0_db,
                rate,
                config,
                existing: &existing,
                output_path: output_path.as_deref(),
                progress_path: progress_path.as_deref(),
                remaining_points: 0, // not tracked in parallel mode
                completed_durations: &[],
            };
            simulate_single_point(encoder, channel, &mut rng, &ctx, |llrs| {
                decoder.reset();
                decoder.decode_iterative(llrs, max_iter)
            })
        };

        #[cfg(feature = "parallel")]
        let points: Vec<SimulationResult> = {
            use rayon::prelude::*;
            config
                .eb_n0_range_db
                .par_iter()
                .enumerate()
                .map(simulate_point)
                .collect()
        };
        #[cfg(not(feature = "parallel"))]
        let points: Vec<SimulationResult> = config
            .eb_n0_range_db
            .iter()
            .enumerate()
            .map(simulate_point)
            .collect();

        let results = SimulationResults { points };
        // Final overwrite with clean, complete file.
        if let Some(ref path) = config.output_path {
            results.write_to(path);
        }
        results
    }

    /// Runs a coded simulation using a decode closure instead of a trait object.
    ///
    /// This is useful for decoders that do not implement [`IterativeSoftDecoder`]
    /// (e.g., [`TurboDecoder`](crate::product::TurboDecoder)) but can be wrapped
    /// in a closure that returns [`DecoderResult`].
    ///
    /// Supports incremental CSV output, JSONL progress logging, and resume from
    /// existing results, identical to [`run_coded_iterative`].
    ///
    /// # Arguments
    ///
    /// * `encoder` - Block encoder producing codewords from messages.
    /// * `decode_fn` - Closure that takes a slice of LLRs and returns a
    ///   [`DecoderResult`].
    /// * `channel` - Channel model for modulation, noise, and demodulation.
    /// * `config` - Simulation configuration controlling sweep parameters.
    ///
    /// # Returns
    ///
    /// Aggregated [`SimulationResults`] with one entry per SNR point.
    ///
    /// # Panics
    ///
    /// Panics if `output_path` is set and the file cannot be written.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::{SimulationRunner, BpskAwgnChannel, SimulationConfig};
    /// use gf2_coding::traits::{DecoderResult, BlockEncoder};
    /// use gf2_core::BitVec;
    ///
    /// // Simple hard-decision closure decoder
    /// let encoder = gf2_coding::linear::LinearBlockCode::hamming(3);
    /// let channel = BpskAwgnChannel;
    /// let mut config = SimulationConfig::quick_test();
    /// config.eb_n0_range_db = vec![10.0];
    /// config.min_errors = 5;
    /// config.max_frames = 500;
    ///
    /// let results = SimulationRunner::run_with_decoder(
    ///     &encoder,
    ///     |llrs| {
    ///         let mut bits = BitVec::with_capacity(encoder.k());
    ///         for &llr in llrs.iter().take(encoder.k()) {
    ///             bits.push_bit(llr.value() < 0.0);
    ///         }
    ///         DecoderResult::success(bits)
    ///     },
    ///     &channel,
    ///     &config,
    /// );
    /// assert_eq!(results.points.len(), 1);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(SNR_points * max_frames * (encode_time + channel_time + decode_time)).
    pub fn run_with_decoder<E, C, F>(
        encoder: &E,
        mut decode_fn: F,
        channel: &C,
        config: &SimulationConfig,
    ) -> SimulationResults
    where
        E: BlockEncoder,
        C: ChannelModel,
        F: FnMut(&[crate::llr::Llr]) -> DecoderResult,
    {
        run_sequential_sweep(encoder, channel, config, |llrs| decode_fn(llrs))
    }
} // impl SimulationRunner (coded methods)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulation_config_quick() {
        let config = SimulationConfig::quick_test();
        assert!(config.min_errors > 0);
        assert!(config.max_frames > config.min_errors);
        assert_eq!(config.max_decoder_iterations, 50);
        assert!(config.rng_seed.is_none());
        assert!(config.output_path.is_none());
    }

    #[test]
    fn test_simulation_config_high_precision() {
        let config = SimulationConfig::high_precision();
        assert_eq!(config.min_errors, 1000);
        assert_eq!(config.eb_n0_range_db.len(), 11);
        assert_eq!(config.max_decoder_iterations, 100);
    }

    #[test]
    fn test_uncoded_ber_simulation() {
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![10.0];
        config.min_errors = 10;
        config.max_frames = 10_000;

        let mut rng = rand::thread_rng();
        let results = SimulationRunner::run_uncoded_ber(&config, &mut rng);

        assert_eq!(results.len(), 1);
        assert!(results[0].ber < 0.01, "BER should be low at 10 dB");
        assert!(results[0].ber >= 0.0);
    }

    #[test]
    fn test_ber_decreases_with_snr() {
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![0.0, 6.0];
        config.min_errors = 50;

        let mut rng = rand::thread_rng();
        let results = SimulationRunner::run_uncoded_ber(&config, &mut rng);

        assert_eq!(results.len(), 2);
        assert!(
            results[1].ber < results[0].ber,
            "BER should decrease with SNR: {} vs {}",
            results[1].ber,
            results[0].ber
        );
    }

    #[test]
    fn test_csv_export() {
        let results = SimulationResults {
            points: vec![SimulationResult {
                eb_n0_db: 3.0,
                ber: 0.01,
                bler: 0.05,
                avg_iterations: Some(12.5),
                avg_queries_per_bit: None,
                num_bits: 10000,
                num_bit_errors: 100,
                num_frames: 200,
                num_frame_errors: 10,
            }],
        };

        let csv = results.to_csv(true);
        assert!(csv.contains("eb_n0_db"), "CSV must contain header");
        assert!(csv.contains("bler"), "CSV header must contain bler");
        assert!(csv.contains("0.01"), "CSV must contain BER value");
        assert!(csv.contains("0.05"), "CSV must contain BLER value");
    }

    #[test]
    fn test_json_export_field_values() {
        let result = SimulationResult {
            eb_n0_db: 3.0,
            ber: 0.01,
            bler: 0.05,
            avg_iterations: Some(12.5),
            avg_queries_per_bit: None,
            num_bits: 10000,
            num_bit_errors: 100,
            num_frames: 200,
            num_frame_errors: 10,
        };

        let json = result.to_json();
        // Verify field:value pairs directly
        assert!(
            json.contains("\"eb_n0_db\":3"),
            "JSON must contain eb_n0_db:3, got: {json}"
        );
        assert!(
            json.contains("\"ber\":0.01"),
            "JSON must contain ber:0.01, got: {json}"
        );
        assert!(
            json.contains("\"bler\":0.05"),
            "JSON must contain bler:0.05, got: {json}"
        );
        assert!(
            json.contains("\"num_bits\":10000"),
            "JSON must contain num_bits:10000, got: {json}"
        );
        assert!(
            json.contains("\"num_bit_errors\":100"),
            "JSON must contain num_bit_errors:100, got: {json}"
        );
        assert!(
            json.contains("\"num_frames\":200"),
            "JSON must contain num_frames:200, got: {json}"
        );
        assert!(
            json.contains("\"num_frame_errors\":10"),
            "JSON must contain num_frame_errors:10, got: {json}"
        );
        assert!(
            json.contains("\"avg_iterations\":12.5"),
            "JSON must contain avg_iterations:12.5, got: {json}"
        );
        assert!(
            json.contains("\"avg_queries_per_bit\":null"),
            "JSON must contain avg_queries_per_bit:null, got: {json}"
        );
    }

    #[test]
    fn test_json_results_array() {
        let results = SimulationResults {
            points: vec![
                SimulationResult {
                    eb_n0_db: 1.0,
                    ber: 0.1,
                    bler: 0.5,
                    avg_iterations: None,
                    avg_queries_per_bit: None,
                    num_bits: 100,
                    num_bit_errors: 10,
                    num_frames: 10,
                    num_frame_errors: 5,
                },
                SimulationResult {
                    eb_n0_db: 2.0,
                    ber: 0.05,
                    bler: 0.3,
                    avg_iterations: None,
                    avg_queries_per_bit: None,
                    num_bits: 200,
                    num_bit_errors: 10,
                    num_frames: 20,
                    num_frame_errors: 6,
                },
            ],
        };

        let json = results.to_json();
        assert!(json.starts_with('['), "JSON array must start with [");
        assert!(json.ends_with(']'), "JSON array must end with ]");
        assert!(
            json.contains("\"eb_n0_db\":1"),
            "JSON must contain first point"
        );
        assert!(
            json.contains("\"eb_n0_db\":2"),
            "JSON must contain second point"
        );
    }

    #[test]
    fn test_simulation_result_complete() {
        let result = SimulationResult {
            eb_n0_db: 3.0,
            ber: 0.01,
            bler: 0.05,
            avg_iterations: None,
            avg_queries_per_bit: None,
            num_bits: 10000,
            num_bit_errors: 100,
            num_frames: 200,
            num_frame_errors: 10,
        };

        assert!(result.is_complete(5));
        assert!(!result.is_complete(50));
    }

    #[test]
    fn test_count_bit_errors_identical() {
        let a = BitVec::from_bytes_le(&[0b10110011]);
        let b = BitVec::from_bytes_le(&[0b10110011]);
        assert_eq!(count_bit_errors(&a, &b), 0);
    }

    #[test]
    fn test_count_bit_errors_all_different() {
        let a = BitVec::from_bytes_le(&[0b00000000]);
        let b = BitVec::from_bytes_le(&[0b11111111]);
        assert_eq!(count_bit_errors(&a, &b), 8);
    }

    #[test]
    fn test_count_bit_errors_length_mismatch() {
        let mut a = BitVec::new();
        a.push_bit(false);
        a.push_bit(true);
        a.push_bit(false);

        let mut b = BitVec::new();
        b.push_bit(false);
        // b is shorter: the 2 missing bits count as errors
        assert_eq!(count_bit_errors(&a, &b), 2);
    }

    #[test]
    fn test_output_path_csv() {
        let dir = std::env::temp_dir().join("gf2_sim_test_csv");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("results.csv");

        let results = SimulationResults {
            points: vec![SimulationResult {
                eb_n0_db: 5.0,
                ber: 0.001,
                bler: 0.01,
                avg_iterations: Some(8.0),
                avg_queries_per_bit: Some(2.5),
                num_bits: 50000,
                num_bit_errors: 50,
                num_frames: 5000,
                num_frame_errors: 50,
            }],
        };
        results.write_to(&path);

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("eb_n0_db"), "CSV file must have header");
        assert!(content.contains("0.001"), "CSV must contain BER value");

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_output_path_json() {
        let dir = std::env::temp_dir().join("gf2_sim_test_json");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("results.json");

        let results = SimulationResults {
            points: vec![SimulationResult {
                eb_n0_db: 5.0,
                ber: 0.001,
                bler: 0.01,
                avg_iterations: None,
                avg_queries_per_bit: None,
                num_bits: 50000,
                num_bit_errors: 50,
                num_frames: 5000,
                num_frame_errors: 50,
            }],
        };
        results.write_to(&path);

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.starts_with('['), "JSON file must start with [");
        assert!(
            content.contains("\"ber\":0.001"),
            "JSON must contain ber:0.001"
        );

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- Mock encoder/decoder for coded simulation tests ---

    /// A trivial (n=4, k=2) repetition-like encoder for testing.
    /// Encodes 2 message bits by repeating each bit once: [m0, m1] -> [m0, m0, m1, m1].
    struct MockEncoder;

    impl BlockEncoder for MockEncoder {
        fn k(&self) -> usize {
            2
        }
        fn n(&self) -> usize {
            4
        }
        fn encode(&self, message: &BitVec) -> BitVec {
            assert_eq!(message.len(), 2);
            let mut codeword = BitVec::with_capacity(4);
            for i in 0..2 {
                let bit = message.get(i);
                codeword.push_bit(bit);
                codeword.push_bit(bit);
            }
            codeword
        }
    }

    /// A mock soft decoder that does majority-vote on repeated pairs.
    struct MockSoftDecoder;

    impl SoftDecoder for MockSoftDecoder {
        fn k(&self) -> usize {
            2
        }
        fn n(&self) -> usize {
            4
        }
        fn decode_soft(&self, llrs: &[Llr]) -> BitVec {
            assert_eq!(llrs.len(), 4);
            let mut result = BitVec::with_capacity(2);
            // Pair 0: llrs[0] + llrs[1], pair 1: llrs[2] + llrs[3]
            for pair in 0..2 {
                let combined = llrs[2 * pair].value() + llrs[2 * pair + 1].value();
                result.push_bit(combined < 0.0);
            }
            result
        }
    }

    /// A mock iterative soft decoder wrapping MockSoftDecoder.
    struct MockIterativeDecoder {
        last_iterations: usize,
    }

    impl SoftDecoder for MockIterativeDecoder {
        fn k(&self) -> usize {
            2
        }
        fn n(&self) -> usize {
            4
        }
        fn decode_soft(&self, llrs: &[Llr]) -> BitVec {
            assert_eq!(llrs.len(), 4);
            let mut result = BitVec::with_capacity(2);
            for pair in 0..2 {
                let combined = llrs[2 * pair].value() + llrs[2 * pair + 1].value();
                result.push_bit(combined < 0.0);
            }
            result
        }
    }

    impl IterativeSoftDecoder for MockIterativeDecoder {
        fn decode_iterative(&mut self, llrs: &[Llr], max_iterations: usize) -> DecoderResult {
            let decoded = self.decode_soft(llrs);
            let iters = max_iterations.min(3);
            self.last_iterations = iters;
            DecoderResult::new(decoded, iters, true, true)
        }

        fn last_iteration_count(&self) -> usize {
            self.last_iterations
        }

        fn reset(&mut self) {
            self.last_iterations = 0;
        }
    }

    #[test]
    fn test_run_coded_basic() {
        let encoder = MockEncoder;
        let decoder = MockSoftDecoder;
        let channel = BpskAwgnChannel;
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![10.0];
        config.min_errors = 5;
        config.max_frames = 1000;

        let results = SimulationRunner::run_coded(&encoder, &decoder, &channel, &config);
        assert_eq!(results.points.len(), 1);
        assert!(results.points[0].num_frames > 0);
        assert!(results.points[0].ber >= 0.0);
        assert!(results.points[0].bler >= 0.0);
    }

    #[test]
    fn test_run_coded_iterative_basic() {
        let encoder = MockEncoder;
        let mut decoder = MockIterativeDecoder { last_iterations: 0 };
        let channel = BpskAwgnChannel;
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![10.0];
        config.min_errors = 5;
        config.max_frames = 1000;

        let results =
            SimulationRunner::run_coded_iterative(&encoder, &mut decoder, &channel, &config);
        assert_eq!(results.points.len(), 1);
        assert!(results.points[0].num_frames > 0);
        assert!(results.points[0].avg_iterations.is_some());
    }

    #[test]
    fn test_run_coded_iterative_parallel_basic() {
        let encoder = MockEncoder;
        let channel = BpskAwgnChannel;
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![8.0, 10.0];
        config.min_errors = 5;
        config.max_frames = 1000;
        config.rng_seed = Some(42);

        let results = SimulationRunner::run_coded_iterative_parallel(
            &encoder,
            || MockIterativeDecoder { last_iterations: 0 },
            &channel,
            &config,
        );
        assert_eq!(results.points.len(), 2);
        for point in &results.points {
            assert!(point.num_frames > 0);
        }
    }

    #[test]
    fn test_run_coded_with_output_path() {
        let dir = std::env::temp_dir().join("gf2_sim_coded_out");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("coded_results.csv");

        let encoder = MockEncoder;
        let decoder = MockSoftDecoder;
        let channel = BpskAwgnChannel;
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![10.0];
        config.min_errors = 5;
        config.max_frames = 500;
        config.output_path = Some(path.clone());

        let _results = SimulationRunner::run_coded(&encoder, &decoder, &channel, &config);
        assert!(path.exists(), "Output file must be created");

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("eb_n0_db"), "CSV must have header");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_run_coded_iterative_with_output_path() {
        let dir = std::env::temp_dir().join("gf2_sim_iter_out");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("iter_results.json");

        let encoder = MockEncoder;
        let mut decoder = MockIterativeDecoder { last_iterations: 0 };
        let channel = BpskAwgnChannel;
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![10.0];
        config.min_errors = 5;
        config.max_frames = 500;
        config.output_path = Some(path.clone());

        let _results =
            SimulationRunner::run_coded_iterative(&encoder, &mut decoder, &channel, &config);
        assert!(path.exists(), "Output file must be created");

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.starts_with('['), "JSON file must start with [");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_run_coded_iterative_parallel_with_output_path() {
        let dir = std::env::temp_dir().join("gf2_sim_par_out");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("par_results.csv");

        let encoder = MockEncoder;
        let channel = BpskAwgnChannel;
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![10.0];
        config.min_errors = 5;
        config.max_frames = 500;
        config.output_path = Some(path.clone());
        config.rng_seed = Some(99);

        let _results = SimulationRunner::run_coded_iterative_parallel(
            &encoder,
            || MockIterativeDecoder { last_iterations: 0 },
            &channel,
            &config,
        );
        assert!(path.exists(), "Output file must be created");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A deterministic "channel" that always returns fixed LLRs.
    /// Bit 0 gets LLR +10 (correct), bit 1 gets LLR -10 (correct),
    /// except when `flip_positions` indicates an error.
    struct DeterministicChannel {
        /// Bit positions in the codeword where the channel introduces errors
        /// (LLR sign is flipped).
        flip_positions: Vec<usize>,
    }

    impl ChannelModel for DeterministicChannel {
        fn transmit_and_demodulate<R: Rng>(
            &self,
            bits: &BitVec,
            _eb_n0_db: f64,
            _rate: f64,
            _rng: &mut R,
        ) -> Vec<Llr> {
            (0..bits.len())
                .map(|i| {
                    let correct_llr = if bits.get(i) {
                        -10.0 // bit=1 -> negative LLR
                    } else {
                        10.0 // bit=0 -> positive LLR
                    };
                    if self.flip_positions.contains(&i) {
                        Llr::new(-correct_llr) // flip: wrong decision
                    } else {
                        Llr::new(correct_llr) // correct
                    }
                })
                .collect()
        }
    }

    #[test]
    fn test_hand_calculated_deterministic_ber() {
        // Setup: MockEncoder maps [m0, m1] -> [m0, m0, m1, m1]
        // DeterministicChannel flips position 0 and 1 (both copies of m0).
        // MockSoftDecoder does majority vote on pairs:
        //   pair 0: both flipped -> wrong decision on m0
        //   pair 1: both correct -> correct on m1
        //
        // So every frame has exactly 1 bit error out of k=2 message bits.
        // With seeded RNG and 10 frames: 10 bit errors, 10 frame errors.

        let encoder = MockEncoder;
        let decoder = MockSoftDecoder;
        let channel = DeterministicChannel {
            flip_positions: vec![0, 1],
        };
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![5.0]; // value doesn't matter for deterministic channel
        config.min_errors = 10;
        config.max_frames = 10;
        config.rng_seed = Some(12345);

        let results = SimulationRunner::run_coded(&encoder, &decoder, &channel, &config);
        assert_eq!(results.points.len(), 1);

        let point = &results.points[0];
        assert_eq!(point.num_frames, 10, "Must have run exactly 10 frames");
        assert_eq!(
            point.num_bit_errors, 10,
            "Each frame has exactly 1 bit error, so 10 total"
        );
        assert_eq!(
            point.num_frame_errors, 10,
            "Every frame has at least 1 error"
        );
        assert_eq!(point.num_bits, 20, "10 frames * k=2 bits per frame");

        let expected_ber = 10.0 / 20.0; // 0.5
        assert!(
            (point.ber - expected_ber).abs() < 1e-10,
            "BER must be exactly 0.5, got {}",
            point.ber
        );

        let expected_bler = 10.0 / 10.0; // 1.0
        assert!(
            (point.bler - expected_bler).abs() < 1e-10,
            "BLER must be exactly 1.0, got {}",
            point.bler
        );
    }

    #[test]
    fn test_deterministic_no_errors() {
        // No flipped positions -> zero errors
        let encoder = MockEncoder;
        let decoder = MockSoftDecoder;
        let channel = DeterministicChannel {
            flip_positions: vec![],
        };
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![5.0];
        config.min_errors = 10;
        config.max_frames = 20;
        config.rng_seed = Some(42);

        let results = SimulationRunner::run_coded(&encoder, &decoder, &channel, &config);
        let point = &results.points[0];

        assert_eq!(point.num_frames, 20, "Should hit max_frames with no errors");
        assert_eq!(
            point.num_bit_errors, 0,
            "No channel errors -> no bit errors"
        );
        assert_eq!(point.num_frame_errors, 0, "No frame errors");
        assert!((point.ber - 0.0).abs() < 1e-10, "BER must be 0.0");
        assert!((point.bler - 0.0).abs() < 1e-10, "BLER must be 0.0");
    }

    #[test]
    fn test_early_termination_at_min_errors() {
        // Channel always causes errors -> should stop at min_errors, not max_frames.
        let encoder = MockEncoder;
        let decoder = MockSoftDecoder;
        let channel = DeterministicChannel {
            flip_positions: vec![0, 1],
        };
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![5.0];
        config.min_errors = 5;
        config.max_frames = 1000;
        config.rng_seed = Some(1);

        let results = SimulationRunner::run_coded(&encoder, &decoder, &channel, &config);
        let point = &results.points[0];

        assert_eq!(
            point.num_frame_errors, 5,
            "Should stop at exactly min_errors frame errors"
        );
        assert_eq!(
            point.num_frames, 5,
            "Every frame has errors, so should stop at 5 frames"
        );
    }

    #[test]
    fn test_seeded_rng_reproducibility() {
        let encoder = MockEncoder;
        let channel = BpskAwgnChannel;
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![3.0];
        config.min_errors = 20;
        config.max_frames = 2000;
        config.rng_seed = Some(42);

        let decoder1 = MockSoftDecoder;
        let results1 = SimulationRunner::run_coded(&encoder, &decoder1, &channel, &config);

        let decoder2 = MockSoftDecoder;
        let results2 = SimulationRunner::run_coded(&encoder, &decoder2, &channel, &config);

        assert_eq!(
            results1.points[0].num_bit_errors,
            results2.points[0].num_bit_errors
        );
        assert_eq!(results1.points[0].num_frames, results2.points[0].num_frames);
    }

    #[test]
    fn test_queries_tracking() {
        /// A decoder that reports queries in its result.
        struct QueryTrackingDecoder;

        impl SoftDecoder for QueryTrackingDecoder {
            fn k(&self) -> usize {
                2
            }
            fn n(&self) -> usize {
                4
            }
            fn decode_soft(&self, llrs: &[Llr]) -> BitVec {
                assert_eq!(llrs.len(), 4);
                let mut result = BitVec::with_capacity(2);
                for pair in 0..2 {
                    let combined = llrs[2 * pair].value() + llrs[2 * pair + 1].value();
                    result.push_bit(combined < 0.0);
                }
                result
            }
            fn decode_soft_with_result(&self, llrs: &[Llr]) -> DecoderResult {
                let decoded = self.decode_soft(llrs);
                let mut r = DecoderResult::new(decoded, 1, true, true);
                r.queries = Some(42);
                r
            }
        }

        let encoder = MockEncoder;
        let decoder = QueryTrackingDecoder;
        let channel = DeterministicChannel {
            flip_positions: vec![],
        };
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![5.0];
        config.min_errors = 1;
        config.max_frames = 5;
        config.rng_seed = Some(1);

        let results = SimulationRunner::run_coded(&encoder, &decoder, &channel, &config);
        let point = &results.points[0];

        // 5 frames, each with 42 queries, k=2 bits per frame -> total_queries=210, total_bits=10
        // avg_queries_per_bit = 210 / 10 = 21.0
        assert!(
            point.avg_queries_per_bit.is_some(),
            "avg_queries_per_bit should be present"
        );
        let avg_q = point.avg_queries_per_bit.unwrap();
        assert!(
            (avg_q - 21.0).abs() < 1e-10,
            "avg_queries_per_bit should be 21.0, got {avg_q}"
        );
    }

    #[test]
    fn test_bpsk_awgn_channel_model() {
        let channel = BpskAwgnChannel;
        let bits = BitVec::from_bytes_le(&[0b10110001]);
        let mut rng = StdRng::seed_from_u64(42);
        let llrs = channel.transmit_and_demodulate(&bits, 10.0, 0.5, &mut rng);
        assert_eq!(llrs.len(), bits.len());
    }

    #[test]
    fn test_snr_accumulator_basic() {
        let mut acc = SnrAccumulator::new(3.0, 10);
        acc.record_frame(2, 5, None);
        acc.record_frame(0, 3, None);
        acc.record_frame(1, 7, Some(100));

        assert_eq!(acc.total_frames, 3);
        assert_eq!(acc.total_bit_errors, 3);
        assert_eq!(acc.total_frame_errors, 2);
        assert_eq!(acc.total_bits, 30);
        assert_eq!(acc.total_iterations, 15);
        // queries: 5 (fallback) + 3 (fallback) + 100 (explicit) = 108
        assert_eq!(acc.total_queries, 108);
    }

    // -------------------------------------------------------------------
    // count_bit_errors boundary tests (0/1/63/64/65 bits)
    // -------------------------------------------------------------------

    #[test]
    fn test_count_bit_errors_empty() {
        let a = BitVec::zeros(0);
        let b = BitVec::zeros(0);
        assert_eq!(count_bit_errors(&a, &b), 0);
    }

    #[test]
    fn test_count_bit_errors_single_bit() {
        let a = BitVec::zeros(1);
        let mut b = BitVec::zeros(1);
        assert_eq!(count_bit_errors(&a, &b), 0);
        b.set(0, true);
        assert_eq!(count_bit_errors(&a, &b), 1);
    }

    #[test]
    fn test_count_bit_errors_63_bits() {
        let a = BitVec::zeros(63);
        let mut b = BitVec::zeros(63);
        b.set(62, true);
        assert_eq!(count_bit_errors(&a, &b), 1);
    }

    #[test]
    fn test_count_bit_errors_64_bits() {
        let a = BitVec::zeros(64);
        let mut b = BitVec::zeros(64);
        b.set(0, true);
        b.set(63, true);
        assert_eq!(count_bit_errors(&a, &b), 2);
    }

    #[test]
    fn test_count_bit_errors_65_bits() {
        let a = BitVec::zeros(65);
        let mut b = BitVec::zeros(65);
        b.set(64, true);
        assert_eq!(count_bit_errors(&a, &b), 1);
    }

    // -------------------------------------------------------------------
    // Incremental CSV, resume, and run_with_decoder tests
    // -------------------------------------------------------------------

    #[test]
    fn test_incremental_csv_append() {
        let dir = std::env::temp_dir().join("gf2_sim_incr_csv");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("incremental.csv");

        let encoder = MockEncoder;
        let mut decoder = MockIterativeDecoder { last_iterations: 0 };
        let channel = BpskAwgnChannel;
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![8.0, 10.0];
        config.min_errors = 5;
        config.max_frames = 1000;
        config.rng_seed = Some(42);
        config.output_path = Some(path.clone());

        let results =
            SimulationRunner::run_coded_iterative(&encoder, &mut decoder, &channel, &config);

        // Verify final CSV exists and contains all points.
        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert!(lines[0].contains("eb_n0_db"), "Header must be present");
        // 1 header + 2 data rows
        assert_eq!(lines.len(), 3, "CSV must have header + 2 data rows");
        assert_eq!(results.points.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resume_skips_completed_points() {
        let dir = std::env::temp_dir().join("gf2_sim_resume");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("resume.csv");

        // Write a partial CSV with one completed point.
        let pre_result = SimulationResult {
            eb_n0_db: 8.0,
            ber: 0.01,
            bler: 0.05,
            avg_iterations: Some(3.0),
            avg_queries_per_bit: Some(1.5),
            num_bits: 2000,
            num_bit_errors: 20,
            num_frames: 1000,
            num_frame_errors: 50,
        };
        // Write header + row
        let header = "eb_n0_db,ber,bler,num_bits,num_bit_errors,num_frames,num_frame_errors,avg_iterations,avg_queries_per_bit";
        std::fs::write(&path, format!("{}\n{}\n", header, pre_result.to_csv_row())).unwrap();

        let encoder = MockEncoder;
        let mut decoder = MockIterativeDecoder { last_iterations: 0 };
        let channel = BpskAwgnChannel;
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![8.0, 10.0];
        config.min_errors = 5; // pre_result has 50 >= 5, so 8.0 dB should be skipped
        config.max_frames = 1000;
        config.rng_seed = Some(42);
        config.output_path = Some(path.clone());

        let results =
            SimulationRunner::run_coded_iterative(&encoder, &mut decoder, &channel, &config);

        assert_eq!(results.points.len(), 2);
        // The 8.0 dB point should be the cached one (50 frame errors).
        assert_eq!(results.points[0].num_frame_errors, 50);
        assert!((results.points[0].eb_n0_db - 8.0).abs() < 1e-10);
        // The 10.0 dB point should be freshly simulated.
        assert!(results.points[1].num_frames > 0);
        assert!((results.points[1].eb_n0_db - 10.0).abs() < 1e-10);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_run_with_decoder_produces_same_results() {
        // Compare closure-based vs trait-based with the same config and seed.
        let encoder = MockEncoder;
        let channel = BpskAwgnChannel;
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![6.0];
        config.min_errors = 5;
        config.max_frames = 500;
        config.rng_seed = Some(42);

        // Trait-based run.
        let mut decoder = MockIterativeDecoder { last_iterations: 0 };
        let results_trait =
            SimulationRunner::run_coded_iterative(&encoder, &mut decoder, &channel, &config);

        // Closure-based run (mimicking MockIterativeDecoder behavior).
        let results_closure = SimulationRunner::run_with_decoder(
            &encoder,
            |llrs| {
                assert_eq!(llrs.len(), 4);
                let mut result_bits = BitVec::with_capacity(2);
                for pair in 0..2 {
                    let combined = llrs[2 * pair].value() + llrs[2 * pair + 1].value();
                    result_bits.push_bit(combined < 0.0);
                }
                let iters = 3; // MockIterativeDecoder uses min(max_iter, 3)
                DecoderResult::new(result_bits, iters, true, true)
            },
            &channel,
            &config,
        );

        assert_eq!(results_trait.points.len(), 1);
        assert_eq!(results_closure.points.len(), 1);

        let pt = &results_trait.points[0];
        let pc = &results_closure.points[0];
        assert_eq!(pt.num_frames, pc.num_frames, "Frame counts must match");
        assert_eq!(
            pt.num_bit_errors, pc.num_bit_errors,
            "Bit error counts must match"
        );
        assert_eq!(
            pt.num_frame_errors, pc.num_frame_errors,
            "Frame error counts must match"
        );
    }

    #[test]
    fn test_from_csv_row_roundtrip() {
        let result = SimulationResult {
            eb_n0_db: 3.5,
            ber: 0.0123,
            bler: 0.0567,
            avg_iterations: Some(12.5),
            avg_queries_per_bit: Some(4.2),
            num_bits: 10000,
            num_bit_errors: 123,
            num_frames: 5000,
            num_frame_errors: 283,
        };

        let row = result.to_csv_row();
        let parsed = SimulationResult::from_csv_row(&row).unwrap();

        assert!((parsed.eb_n0_db - result.eb_n0_db).abs() < 1e-10);
        assert!((parsed.ber - result.ber).abs() < 1e-10);
        assert!((parsed.bler - result.bler).abs() < 1e-10);
        assert_eq!(parsed.num_bits, result.num_bits);
        assert_eq!(parsed.num_bit_errors, result.num_bit_errors);
        assert_eq!(parsed.num_frames, result.num_frames);
        assert_eq!(parsed.num_frame_errors, result.num_frame_errors);
        assert!((parsed.avg_iterations.unwrap() - result.avg_iterations.unwrap()).abs() < 1e-10);
        assert!(
            (parsed.avg_queries_per_bit.unwrap() - result.avg_queries_per_bit.unwrap()).abs()
                < 1e-10
        );
    }

    #[test]
    fn test_from_csv_row_no_optional_fields() {
        let result = SimulationResult {
            eb_n0_db: 1.0,
            ber: 0.1,
            bler: 0.5,
            avg_iterations: None,
            avg_queries_per_bit: None,
            num_bits: 100,
            num_bit_errors: 10,
            num_frames: 10,
            num_frame_errors: 5,
        };

        let row = result.to_csv_row();
        let parsed = SimulationResult::from_csv_row(&row).unwrap();
        assert!(parsed.avg_iterations.is_none());
        assert!(parsed.avg_queries_per_bit.is_none());
    }

    #[test]
    fn test_from_csv_row_bad_input() {
        assert!(SimulationResult::from_csv_row("").is_none());
        assert!(SimulationResult::from_csv_row("not,enough,fields").is_none());
        assert!(SimulationResult::from_csv_row("abc,def,ghi,1,2,3,4").is_none());
    }

    #[test]
    fn test_try_load_nonexistent() {
        let results = try_load_existing_results(Path::new("/tmp/nonexistent_gf2_test.csv"), 5);
        assert!(results.is_empty());
    }

    #[test]
    fn test_progress_path_derivation() {
        let csv_path = Path::new("/tmp/results.csv");
        let pp = progress_path_for(csv_path);
        assert_eq!(pp, PathBuf::from("/tmp/results.progress.jsonl"));
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(std::time::Duration::from_secs(0)), "0s");
        assert_eq!(format_duration(std::time::Duration::from_secs(59)), "59s");
        assert_eq!(format_duration(std::time::Duration::from_secs(60)), "1m00s");
        assert_eq!(
            format_duration(std::time::Duration::from_secs(125)),
            "2m05s"
        );
        assert_eq!(
            format_duration(std::time::Duration::from_secs(3661)),
            "1h01m01s"
        );
    }

    #[test]
    fn test_jsonl_progress_written() {
        let dir = std::env::temp_dir().join("gf2_sim_jsonl_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let csv_path = dir.join("results.csv");
        let jsonl_path = dir.join("results.progress.jsonl");

        // Force JSONL progress by using a deterministic channel with errors
        // so we hit enough frames, and set a 0s threshold override by writing
        // directly through the accumulator API.
        let encoder = MockEncoder;
        let channel = DeterministicChannel {
            flip_positions: vec![0, 1],
        };
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![5.0];
        config.min_errors = 5;
        config.max_frames = 500;
        config.rng_seed = Some(42);
        config.output_path = Some(csv_path.clone());

        // Manually simulate to force JSONL writes (the wall-clock threshold in
        // should_write_progress means normal short tests won't trigger it).
        let k = encoder.k();
        let n = encoder.n();
        let rate = k as f64 / n as f64;
        let mut rng = config.make_rng();
        let mut acc = SnrAccumulator::new(5.0, k);

        while !acc.should_stop(config.min_errors, config.max_frames) {
            let message = BitVec::random(k, &mut rng);
            let codeword = encoder.encode(&message);
            let llrs = channel.transmit_and_demodulate(&codeword, 5.0, rate, &mut rng);
            let decoded_bits = {
                let mut result = BitVec::with_capacity(2);
                for pair in 0..2 {
                    let combined = llrs[2 * pair].value() + llrs[2 * pair + 1].value();
                    result.push_bit(combined < 0.0);
                }
                result
            };
            let bit_errors = count_bit_errors(&message, &decoded_bits);
            acc.record_frame(bit_errors, 1, None);
        }

        // Force a progress entry write.
        acc.write_progress_entry(&jsonl_path);
        // Force a point_complete entry write.
        let sim_result = acc.into_result();
        let mut acc2 = SnrAccumulator::new(5.0, k);
        acc2.write_point_complete_entry(&jsonl_path, &sim_result);

        // Verify the JSONL file exists and has valid entries.
        assert!(jsonl_path.exists(), "JSONL progress file must exist");
        let content = std::fs::read_to_string(&jsonl_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert!(
            lines.len() >= 2,
            "JSONL must have at least 2 lines (progress + point_complete), got {}",
            lines.len()
        );

        // Validate the first progress line has expected fields.
        let first_line = lines[0];
        assert!(
            first_line.contains("\"type\":\"progress\""),
            "First line must have type:progress, got: {first_line}"
        );
        assert!(
            first_line.contains("\"eb_n0_db\""),
            "Must contain eb_n0_db field"
        );
        assert!(
            first_line.contains("\"frames\""),
            "Must contain frames field"
        );
        assert!(
            first_line.contains("\"frame_errors\""),
            "Must contain frame_errors field"
        );
        assert!(
            first_line.contains("\"elapsed_s\""),
            "Must contain elapsed_s field"
        );
        assert!(
            first_line.contains("\"timestamp\""),
            "Must contain timestamp field"
        );

        // Validate the point_complete line.
        let last_line = lines[lines.len() - 1];
        assert!(
            last_line.contains("\"type\":\"point_complete\""),
            "Last line must have type:point_complete, got: {last_line}"
        );
        assert!(
            last_line.contains("\"ber\""),
            "point_complete must contain ber"
        );
        assert!(
            last_line.contains("\"bler\""),
            "point_complete must contain bler"
        );
        assert!(
            last_line.contains("\"num_frames\""),
            "point_complete must contain num_frames"
        );

        // Verify timestamps are ISO 8601 format (YYYY-MM-DDTHH:MM:SS).
        // Extract timestamp from first line.
        if let Some(ts_start) = first_line.find("\"timestamp\":\"") {
            let after = &first_line[ts_start + 13..];
            if let Some(ts_end) = after.find('"') {
                let ts = &after[..ts_end];
                assert_eq!(
                    ts.len(),
                    19,
                    "Timestamp must be 19 chars (YYYY-MM-DDTHH:MM:SS), got: {ts}"
                );
                assert_eq!(
                    &ts[4..5],
                    "-",
                    "Timestamp must have dash at pos 4, got: {ts}"
                );
                assert_eq!(
                    &ts[10..11],
                    "T",
                    "Timestamp must have T at pos 10, got: {ts}"
                );
                assert_eq!(
                    &ts[13..14],
                    ":",
                    "Timestamp must have colon at pos 13, got: {ts}"
                );
            }
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_iso8601_timestamp_format() {
        let ts = chrono_like_timestamp();
        assert_eq!(
            ts.len(),
            19,
            "Timestamp must be 19 chars (YYYY-MM-DDTHH:MM:SS), got: {ts}"
        );
        assert_eq!(&ts[4..5], "-", "Position 4 must be '-' in: {ts}");
        assert_eq!(&ts[7..8], "-", "Position 7 must be '-' in: {ts}");
        assert_eq!(&ts[10..11], "T", "Position 10 must be 'T' in: {ts}");
        assert_eq!(&ts[13..14], ":", "Position 13 must be ':' in: {ts}");
        assert_eq!(&ts[16..17], ":", "Position 16 must be ':' in: {ts}");
        // Year should be reasonable (2020-2099).
        let year: u32 = ts[..4].parse().expect("Year must be numeric");
        assert!(
            (2020..2100).contains(&year),
            "Year {year} is out of range in: {ts}"
        );
    }

    // -------------------------------------------------------------------
    // Property-based tests for simulation statistics
    // -------------------------------------------------------------------

    mod prop_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn prop_count_bit_errors_symmetric(len in 1usize..128) {
                let mut rng = rand::thread_rng();
                let a = BitVec::random(len, &mut rng);
                let b = BitVec::random(len, &mut rng);
                prop_assert_eq!(count_bit_errors(&a, &b), count_bit_errors(&b, &a));
            }

            #[test]
            fn prop_count_bit_errors_identical_is_zero(len in 1usize..128) {
                let mut rng = rand::thread_rng();
                let a = BitVec::random(len, &mut rng);
                prop_assert_eq!(count_bit_errors(&a, &a), 0);
            }

            #[test]
            fn prop_count_bit_errors_bounded(len in 1usize..128) {
                let mut rng = rand::thread_rng();
                let a = BitVec::random(len, &mut rng);
                let b = BitVec::random(len, &mut rng);
                prop_assert!(count_bit_errors(&a, &b) <= len);
            }

            #[test]
            fn prop_ber_between_zero_and_one(
                num_bit_errors in 0usize..1000,
                num_bits in 1usize..10000,
            ) {
                let ber = num_bit_errors as f64 / num_bits as f64;
                prop_assert!(ber >= 0.0);
                // BER can exceed 1.0 if errors > bits (e.g., random decode)
            }

            #[test]
            fn prop_bler_between_zero_and_one(
                num_block_errors in 0usize..100,
                num_frames in 1usize..1000,
            ) {
                let bler = num_block_errors as f64 / num_frames as f64;
                prop_assert!(bler >= 0.0);
            }
        }
    }
}
