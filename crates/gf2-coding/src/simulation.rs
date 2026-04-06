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
use crate::traits::{BlockEncoder, IterativeSoftDecoder, SoftDecoder};
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::path::PathBuf;

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
            csv.push_str("eb_n0_db,ber,bler,num_bits,num_bit_errors,num_frames,num_frame_errors,avg_iterations,avg_queries_per_bit\n");
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
/// Provides static methods for uncoded BER simulations. For coded
/// simulations, use the free functions [`run_coded`],
/// [`run_coded_iterative`], or [`run_coded_iterative_parallel`].
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

/// Reports simulation progress to stderr.
fn report_progress(eb_n0_db: f64, frames: usize, frame_errors: usize, min_errors: usize) {
    eprintln!(
        "[{:.1} dB] frames={}, frame_errors={}/{} ({:.1}%)",
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
}

impl SnrAccumulator {
    fn new(eb_n0_db: f64, k: usize) -> Self {
        Self {
            eb_n0_db,
            total_bit_errors: 0,
            total_bits: 0,
            total_frame_errors: 0,
            total_frames: 0,
            total_iterations: 0,
            total_queries: 0,
            k,
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

    fn into_result(self, min_errors: usize) -> SimulationResult {
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

        if self.total_frames > 0 {
            report_progress(
                self.eb_n0_db,
                self.total_frames,
                self.total_frame_errors,
                min_errors,
            );
        }

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

/// Counts bit errors between a decoded message and the original.
fn count_bit_errors(original: &BitVec, decoded: &BitVec) -> usize {
    let len = original.len().min(decoded.len());
    let mut errors = 0;
    for i in 0..len {
        if original.get(i) != decoded.get(i) {
            errors += 1;
        }
    }
    // Bits missing or extra in decoded count as errors
    errors += original.len().abs_diff(decoded.len());
    errors
}

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
/// ```ignore
/// use gf2_coding::simulation::{run_coded, BpskAwgnChannel, SimulationConfig};
///
/// let encoder = /* your BlockEncoder */;
/// let decoder = /* your SoftDecoder */;
/// let channel = BpskAwgnChannel;
/// let config = SimulationConfig::quick_test();
/// let results = run_coded(&encoder, &decoder, &channel, &config);
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
    let k = encoder.k();
    let n = encoder.n();
    let rate = k as f64 / n as f64;

    let mut rng = config.make_rng();
    let mut points = Vec::with_capacity(config.eb_n0_range_db.len());

    for &eb_n0_db in &config.eb_n0_range_db {
        let mut acc = SnrAccumulator::new(eb_n0_db, k);

        while !acc.should_stop(config.min_errors, config.max_frames) {
            let message = BitVec::random(k, &mut rng);
            let codeword = encoder.encode(&message);
            let llrs = channel.transmit_and_demodulate(&codeword, eb_n0_db, rate, &mut rng);
            let result = decoder.decode_soft_with_result(&llrs);
            let bit_errors = count_bit_errors(&message, &result.decoded_bits);
            acc.record_frame(bit_errors, result.iterations, result.queries);

            if acc.should_report() {
                report_progress(
                    eb_n0_db,
                    acc.total_frames,
                    acc.total_frame_errors,
                    config.min_errors,
                );
            }
        }

        points.push(acc.into_result(config.min_errors));
    }

    let results = SimulationResults { points };
    if let Some(ref path) = config.output_path {
        results.write_to(path);
    }
    results
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
/// ```ignore
/// use gf2_coding::simulation::{run_coded_iterative, BpskAwgnChannel, SimulationConfig};
///
/// let encoder = /* your BlockEncoder */;
/// let mut decoder = /* your IterativeSoftDecoder */;
/// let channel = BpskAwgnChannel;
/// let config = SimulationConfig::quick_test();
/// let results = run_coded_iterative(&encoder, &mut decoder, &channel, &config);
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
    let k = encoder.k();
    let n = encoder.n();
    let rate = k as f64 / n as f64;

    let mut rng = config.make_rng();
    let mut points = Vec::with_capacity(config.eb_n0_range_db.len());

    for &eb_n0_db in &config.eb_n0_range_db {
        let mut acc = SnrAccumulator::new(eb_n0_db, k);

        while !acc.should_stop(config.min_errors, config.max_frames) {
            let message = BitVec::random(k, &mut rng);
            let codeword = encoder.encode(&message);
            let llrs = channel.transmit_and_demodulate(&codeword, eb_n0_db, rate, &mut rng);

            decoder.reset();
            let result = decoder.decode_iterative(&llrs, config.max_decoder_iterations);
            let bit_errors = count_bit_errors(&message, &result.decoded_bits);
            acc.record_frame(bit_errors, result.iterations, result.queries);

            if acc.should_report() {
                report_progress(
                    eb_n0_db,
                    acc.total_frames,
                    acc.total_frame_errors,
                    config.min_errors,
                );
            }
        }

        points.push(acc.into_result(config.min_errors));
    }

    let results = SimulationResults { points };
    if let Some(ref path) = config.output_path {
        results.write_to(path);
    }
    results
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
/// ```ignore
/// use gf2_coding::simulation::{run_coded_iterative_parallel, BpskAwgnChannel, SimulationConfig};
///
/// let encoder = /* your BlockEncoder (Send + Sync) */;
/// let channel = BpskAwgnChannel;
/// let mut config = SimulationConfig::quick_test();
/// config.rng_seed = Some(42);
///
/// let results = run_coded_iterative_parallel(
///     &encoder,
///     || { /* create decoder */ },
///     &channel,
///     &config,
/// );
/// assert_eq!(results.points.len(), config.eb_n0_range_db.len());
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

    let simulate_point = |(idx, &eb_n0_db): (usize, &f64)| -> SimulationResult {
        let mut decoder = make_decoder();
        // Each SNR point gets a unique sub-seed derived from the config seed.
        // When no seed is provided, use a fixed per-point seed for consistency.
        let point_seed = config
            .rng_seed
            .unwrap_or(0xDEAD_BEEF)
            .wrapping_add(idx as u64);
        let mut rng = StdRng::seed_from_u64(point_seed);

        let mut acc = SnrAccumulator::new(eb_n0_db, k);

        while !acc.should_stop(config.min_errors, config.max_frames) {
            let message = BitVec::random(k, &mut rng);
            let codeword = encoder.encode(&message);
            let llrs = channel.transmit_and_demodulate(&codeword, eb_n0_db, rate, &mut rng);

            decoder.reset();
            let result = decoder.decode_iterative(&llrs, config.max_decoder_iterations);
            let bit_errors = count_bit_errors(&message, &result.decoded_bits);
            acc.record_frame(bit_errors, result.iterations, result.queries);

            if acc.should_report() {
                report_progress(
                    eb_n0_db,
                    acc.total_frames,
                    acc.total_frame_errors,
                    config.min_errors,
                );
            }
        }

        acc.into_result(config.min_errors)
    };

    #[cfg(feature = "parallel")]
    let points: Vec<SimulationResult> = {
        use rayon::prelude::*;
        config
            .eb_n0_range_db
            .par_iter()
            .enumerate()
            .map(|pair| simulate_point(pair))
            .collect()
    };
    #[cfg(not(feature = "parallel"))]
    let points: Vec<SimulationResult> = config
        .eb_n0_range_db
        .iter()
        .enumerate()
        .map(|pair| simulate_point(pair))
        .collect();

    let results = SimulationResults { points };
    if let Some(ref path) = config.output_path {
        results.write_to(path);
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::DecoderResult;

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

        let results = run_coded(&encoder, &decoder, &channel, &config);
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

        let results = run_coded_iterative(&encoder, &mut decoder, &channel, &config);
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

        let results = run_coded_iterative_parallel(
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

        let _results = run_coded(&encoder, &decoder, &channel, &config);
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

        let _results = run_coded_iterative(&encoder, &mut decoder, &channel, &config);
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

        let _results = run_coded_iterative_parallel(
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

        let results = run_coded(&encoder, &decoder, &channel, &config);
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

        let results = run_coded(&encoder, &decoder, &channel, &config);
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

        let results = run_coded(&encoder, &decoder, &channel, &config);
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
        let results1 = run_coded(&encoder, &decoder1, &channel, &config);

        let decoder2 = MockSoftDecoder;
        let results2 = run_coded(&encoder, &decoder2, &channel, &config);

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

        let results = run_coded(&encoder, &decoder, &channel, &config);
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
}
