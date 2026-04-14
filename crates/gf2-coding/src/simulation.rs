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
//!   ([`SimulationRunner::run_uncoded_ber`]). The legacy entry point is
//!   hard-coded to BPSK; the modem-backed counterpart
//!   [`SimulationRunner::run_uncoded_ber_with_channel`] takes any
//!   [`ChannelModel`] (e.g., the
//!   [`ModemChannelAdapter`](crate::modem::ModemChannelAdapter) built on the
//!   shared modem framework) and performs hard decisions directly on the
//!   LLRs it returns.
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
//! default BPSK/AWGN implementation provided by [`BpskAwgnChannel`]. The
//! modem-framework adapter
//! [`ModemChannelAdapter`](crate::modem::ModemChannelAdapter) also implements
//! [`ChannelModel`], so any validated
//! [`ModemSpec`](crate::modem::ModemSpec) (BPSK, QPSK, 16-/64-/256-QAM, ...)
//! can be plugged into every `*_with_channel` runner entry point as well as
//! into [`run_coded`], [`run_coded_iterative`], and
//! [`run_coded_iterative_parallel`], which already accept any
//! `C: ChannelModel`.
//!
//! # Output
//!
//! Results can be exported to CSV or JSON via [`SimulationResults`]. When
//! [`SimulationConfig::output_path`] is set, results are automatically written
//! to disk in the format determined by the file extension (`.json` for JSON,
//! anything else for CSV).

use crate::channel::AwgnChannel;
use crate::llr::Llr;
use crate::modem::{
    BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, ModemSpec, ReferenceMapper,
    ReferenceSoftDemapper,
};
use crate::traits::{BlockEncoder, DecoderResult, IterativeSoftDecoder, SoftDecoder};
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};

/// Global lock for serializing all JSONL file appends (progress + point_complete).
///
/// Used by both `SnrAccumulator::write_progress_entry` and
/// `append_point_complete_jsonl` to prevent interleaved writes from
/// concurrent parallel simulation workers.
static JSONL_WRITE_LOCK: Mutex<()> = Mutex::new(());
use std::time::{Duration, Instant};

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

    /// Minimum required alignment for `bits.len()` calls into
    /// [`ChannelModel::transmit_and_demodulate`].
    ///
    /// Returns `1` for channels that accept any length (the BPSK legacy
    /// path). Modem-backed channels that internally require
    /// `bits.len() % bits_per_symbol == 0` return their `bits_per_symbol`
    /// here so callers like
    /// [`SimulationRunner::run_uncoded_ber_with_channel`] can round their
    /// batch lengths down to a multiple rather than panicking on a
    /// ragged tail.
    ///
    /// Default is `1`.
    fn batch_alignment(&self) -> usize {
        1
    }
}

/// Default BPSK modulation over an AWGN channel.
///
/// Maps bits to +/-1 BPSK symbols, adds Gaussian noise with variance
/// determined by Eb/N0 and code rate, then converts received symbols
/// to LLRs via `2r / sigma^2`.
///
/// # Framework-backed implementation
///
/// All bit-to-symbol mapping and LLR conversion route through the shared
/// modem framework ([`crate::modem::ReferenceMapper`] and
/// [`crate::modem::ReferenceSoftDemapper`] over
/// [`crate::modem::ModemSpec::bpsk_with_scalar`]). The only call that is
/// intrinsically AWGN-shaped (and therefore not modem business) is the
/// noise application via [`AwgnChannel::transmit_symbols`].
///
/// Noise is drawn once per BPSK symbol on the I axis (no Q-axis draw),
/// matching the 1-D noise convention used by BPSK reference simulations.
/// Callers that want the generic 2-D modem pipeline (with Q-axis noise
/// and arbitrary constellation order) should use
/// [`crate::modem::ModemChannelAdapter`] instead.
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

/// Lazily-initialised, process-wide BPSK reference mapper over `f64`,
/// used by [`BpskAwgnChannel::transmit_and_demodulate`] so every frame
/// shares the same constant preset without reallocating.
fn bpsk_mapper_f64() -> &'static ReferenceMapper<f64> {
    static MAPPER: OnceLock<ReferenceMapper<f64>> = OnceLock::new();
    MAPPER.get_or_init(|| ReferenceMapper::new(ModemSpec::<f64>::bpsk_with_scalar()))
}

/// Lazily-initialised, process-wide BPSK reference soft demapper over
/// `f64`. Matches the convention `noise_var = 2 * sigma^2` for the BPSK
/// closed form `LLR = 2 y / sigma^2`.
fn bpsk_demapper_f64() -> &'static ReferenceSoftDemapper<f64> {
    static DEMAP: OnceLock<ReferenceSoftDemapper<f64>> = OnceLock::new();
    DEMAP.get_or_init(|| ReferenceSoftDemapper::new(ModemSpec::<f64>::bpsk_with_scalar()))
}

impl ChannelModel for BpskAwgnChannel {
    fn transmit_and_demodulate<R: Rng>(
        &self,
        bits: &BitVec,
        eb_n0_db: f64,
        rate: f64,
        rng: &mut R,
    ) -> Vec<Llr> {
        // All modem-side math runs through the shared framework:
        //   bits -> ReferenceMapper<f64> (BPSK preset) -> I-axis symbols
        //   -> AwgnChannel::transmit_symbols (1-D noise) -> received I
        //   -> ReferenceSoftDemapper<f64> with noise_var = 2 * sigma^2
        //      (the framework's BPSK closed form recovers LLR = 2 y/sigma^2)
        //
        // The I-only (1-D) noise application is deliberate: legacy
        // `BpskAwgnChannel` never drew Q-axis noise, and downstream tests
        // rely on that RNG-stream shape.
        let n = bits.len();
        let channel = AwgnChannel::from_eb_n0_db(eb_n0_db, rate);
        let bits_vec: Vec<bool> = (0..n).map(|i| bits.get(i)).collect();

        let mut tx_i = vec![0.0_f64; n];
        let mut tx_q = vec![0.0_f64; n];
        bpsk_mapper_f64().map_bits(&bits_vec, &mut tx_i, &mut tx_q);

        let received = channel.transmit_symbols(&tx_i, rng);

        let n0 = vec![2.0 * channel.variance(); n];
        let mut llrs = vec![Llr::new(0.0); n];
        let input = DemapInput::<f64> {
            rx_i: &received,
            rx_q: &tx_q, // all zeros — BPSK is I-axis only
            gain_i: None,
            gain_q: None,
            noise_var: &n0,
            method: DemapMethod::ExactLogMap,
        };
        bpsk_demapper_f64().demap_llrs(input, &mut llrs);
        llrs
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
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_coding::simulation::SimulationResult;
    /// use std::path::Path;
    ///
    /// let result = SimulationResult {
    ///     eb_n0_db: 3.0, ber: 0.001, bler: 0.01,
    ///     avg_iterations: Some(5.0), avg_queries_per_bit: None,
    ///     num_bits: 12100, num_bit_errors: 12,
    ///     num_frames: 100, num_frame_errors: 1,
    /// };
    /// result.append_csv_row_to(Path::new("/tmp/test_results.csv"));
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1) per call (single file open + write).
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
/// - [`SimulationRunner::run_uncoded_ber_with_channel`] — uncoded over any
///   [`ChannelModel`] (modem-framework backed, e.g.
///   [`ModemChannelAdapter`](crate::modem::ModemChannelAdapter))
/// - [`SimulationRunner::run_coded`] — coded with immutable [`SoftDecoder`]
/// - [`SimulationRunner::run_coded_iterative`] — coded with [`IterativeSoftDecoder`]
/// - [`SimulationRunner::run_coded_iterative_parallel`] — parallel iterative with decoder factory
pub struct SimulationRunner;

impl SimulationRunner {
    /// Simulates uncoded BPSK transmission over AWGN and computes BER.
    ///
    /// Thin wrapper that delegates to
    /// [`SimulationRunner::run_uncoded_ber_with_channel`] with
    /// [`BpskAwgnChannel`] as the channel, so the BPSK/AWGN Monte Carlo
    /// loop has a single source of truth.
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
    /// O(SNR_points * max_frames) in transmitted bits. Internal batching
    /// is governed by `run_uncoded_ber_with_channel` (currently 960 bits
    /// per inner call, rounded down to `BpskAwgnChannel::batch_alignment()`
    /// which is `1`).
    pub fn run_uncoded_ber<R: Rng>(
        config: &SimulationConfig,
        rng: &mut R,
    ) -> Vec<SimulationResult> {
        // Delegate to the shared modem-backed runner so the legacy BPSK
        // entry point no longer reimplements the modulate/transmit/
        // demodulate Monte Carlo loop. `BpskAwgnChannel` is the canonical
        // `ChannelModel` for the legacy path; running through it keeps
        // the noise-variance, RNG consumption, and hard-decision
        // convention identical to the previous implementation while
        // eliminating duplication with `run_uncoded_ber_with_channel`.
        Self::run_uncoded_ber_with_channel(&BpskAwgnChannel, config, rng)
    }

    /// Modem-backed counterpart to [`SimulationRunner::run_uncoded_ber`].
    ///
    /// Routes the uncoded sweep through the supplied [`ChannelModel`]
    /// and applies a hard decision to the returned LLRs (convention:
    /// positive LLR => bit 0, negative LLR => bit 1; this matches the
    /// modem framework's
    /// [`BatchSoftDemapper`](crate::modem::BatchSoftDemapper) output
    /// sign). All modulation, noise generation, and demapping live
    /// inside [`ChannelModel::transmit_and_demodulate`] — this method
    /// never reimplements a BPSK LLR formula or a noise draw itself.
    ///
    /// Passing [`crate::simulation::BpskAwgnChannel`] gives the BPSK/AWGN
    /// reference path (at equal `StdRng` seeds it converges to the same
    /// BER as the legacy uncoded runner). Passing
    /// [`crate::modem::ModemChannelAdapter`] runs any validated
    /// [`ModemSpec`](crate::modem::ModemSpec) (BPSK, QPSK, 16-/64-/256-QAM)
    /// over AWGN with the shared [`BatchMapper`](crate::modem::BatchMapper)
    /// and [`BatchSoftDemapper`](crate::modem::BatchSoftDemapper) surfaces.
    ///
    /// # Arguments
    ///
    /// * `channel` - Any [`ChannelModel`] implementation. `rate = 1.0` is
    ///   passed for every call because this is an uncoded sweep.
    /// * `config` - Simulation configuration. `eb_n0_range_db`, `min_errors`,
    ///   and `max_frames` are consumed; the decoder-iteration and output
    ///   fields are ignored (this is an uncoded runner).
    /// * `rng` - Random source used for both bit generation and channel
    ///   noise. A single RNG feeds both so deterministic seeding is
    ///   honoured end-to-end.
    ///
    /// # Batch sizing
    ///
    /// Bits are generated in batches of `UNCODED_MODEM_BATCH_BITS` so that
    /// the batch size is a multiple of every common `bits_per_symbol`
    /// (1, 2, 3, 4, 5, 6, 8, 10, 12, 15, 16), satisfying the
    /// [`ModemChannelAdapter`](crate::modem::ModemChannelAdapter)
    /// precondition `bits.len() % bits_per_symbol == 0` for all standard
    /// modulations.
    ///
    /// # Returns
    ///
    /// One [`SimulationResult`] per SNR point in `config.eb_n0_range_db`.
    /// `num_bits` / `num_bit_errors` are populated; `num_frames` stays 0
    /// because uncoded streaming has no natural frame boundary.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::{
    ///     BpskAwgnChannel, SimulationConfig, SimulationRunner,
    /// };
    ///
    /// let mut config = SimulationConfig::quick_test();
    /// config.eb_n0_range_db = vec![6.0];
    /// config.min_errors = 1;
    /// config.max_frames = 10_000;
    /// let channel = BpskAwgnChannel;
    /// let mut rng = rand::thread_rng();
    /// let results = SimulationRunner::run_uncoded_ber_with_channel(
    ///     &channel, &config, &mut rng,
    /// );
    /// assert_eq!(results.len(), 1);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(SNR_points * max_frames * channel_time).
    pub fn run_uncoded_ber_with_channel<C: ChannelModel, R: Rng>(
        channel: &C,
        config: &SimulationConfig,
        rng: &mut R,
    ) -> Vec<SimulationResult> {
        /// Nominal batch length. The loop rounds each call down to a
        /// multiple of `channel.batch_alignment()` so that modem-backed
        /// [`ChannelModel`] implementations like
        /// [`ModemChannelAdapter`](crate::modem::ModemChannelAdapter) --
        /// which require `bits.len() % bits_per_symbol == 0` -- never
        /// see a ragged-tail batch, regardless of how `max_frames` is
        /// configured.
        const UNCODED_MODEM_BATCH_BITS: usize = 960;

        let alignment = channel.batch_alignment().max(1);

        config
            .eb_n0_range_db
            .iter()
            .map(|&eb_n0_db| {
                let mut total_bits = 0usize;
                let mut total_errors = 0usize;

                while total_errors < config.min_errors && total_bits < config.max_frames {
                    let remaining = config.max_frames - total_bits;
                    let mut batch_size = UNCODED_MODEM_BATCH_BITS.min(remaining);
                    // Round down to the channel's required alignment so
                    // modem-backed channels with bits_per_symbol > 1 do
                    // not panic on a ragged tail.
                    batch_size -= batch_size % alignment;
                    if batch_size == 0 {
                        break;
                    }
                    let bits = BitVec::random(batch_size, rng);
                    // Uncoded => rate = 1.0. The channel owns modulation,
                    // noise, and demapping end-to-end; we only consume LLRs.
                    let llrs = channel.transmit_and_demodulate(&bits, eb_n0_db, 1.0, rng);
                    debug_assert_eq!(llrs.len(), batch_size);

                    let errors = (0..batch_size)
                        .filter(|&i| {
                            // Positive LLR => bit 0, negative => bit 1.
                            // Ties (0.0) map to bit 0, matching the
                            // framework hard-decision convention.
                            let decoded_bit = llrs[i].value() < 0.0;
                            bits.get(i) != decoded_bit
                        })
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
///
/// Uses a tiered approach based on error count:
/// - 0 errors: show frame progress as percentage of max_frames, no ETA guess
/// - 1-5 errors: show cautious ETA flagged as "rough"
/// - >5 errors: confident ETA from current BLER and frame rate
fn report_progress(
    eb_n0_db: f64,
    frames: usize,
    frame_errors: usize,
    min_errors: usize,
    max_frames: usize,
    elapsed: Option<std::time::Duration>,
) {
    let elapsed_str = elapsed.map_or_else(String::new, |d| format!(" [{}]", format_duration(d)));

    let eta_str = if let Some(el) = elapsed {
        if frames == 0 || el.as_secs_f64() == 0.0 {
            String::new()
        } else if frame_errors == 0 {
            // No errors yet: show progress toward max_frames, no ETA guess.
            let pct = 100.0 * frames as f64 / max_frames as f64;
            format!(", {frames}/{max_frames} ({pct:.1}%), no errors yet")
        } else if frame_errors < min_errors {
            let remaining_errors = min_errors - frame_errors;
            let error_rate = frame_errors as f64 / frames as f64;
            let remaining_frames = (remaining_errors as f64 / error_rate).ceil() as usize;
            let remaining_frames = remaining_frames.min(max_frames.saturating_sub(frames));
            let frame_rate = frames as f64 / el.as_secs_f64();
            if frame_rate > 0.0 {
                let eta_secs = remaining_frames as f64 / frame_rate;
                let eta_dur = std::time::Duration::from_secs_f64(eta_secs);
                if frame_errors <= 5 {
                    // Few errors: ETA is unreliable, flag it.
                    format!(
                        ", ETA ~{} (rough, {} errors)",
                        format_duration(eta_dur),
                        frame_errors
                    )
                } else {
                    format!(", ETA ~{}", format_duration(eta_dur))
                }
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

/// Data about a completed SNR point for sweep ETA estimation.
#[derive(Clone, Debug)]
struct CompletedPointInfo {
    eb_n0_db: f64,
    duration: std::time::Duration,
    num_frames: usize,
    bler: f64,
}

/// Estimates the remaining sweep time using log-linear BLER extrapolation.
///
/// For each remaining SNR point, estimates the BLER from the log-linear trend
/// of completed points that had errors, then estimates frames needed as
/// `min_errors / estimated_bler` (capped at `max_frames`), and estimates
/// duration from the frame rate of the nearest completed point.
///
/// Returns `None` if insufficient data (fewer than 2 completed points with
/// errors) to extrapolate.
fn estimate_sweep_eta(
    completed: &[CompletedPointInfo],
    remaining_snr_points: &[f64],
    min_errors: usize,
    max_frames: usize,
) -> Option<std::time::Duration> {
    if remaining_snr_points.is_empty() {
        return None;
    }

    // Collect points with measurable BLER for log-linear fit.
    let data_points: Vec<(f64, f64)> = completed
        .iter()
        .filter(|p| p.bler > 0.0 && p.num_frames > 0)
        .map(|p| (p.eb_n0_db, p.bler.ln()))
        .collect();

    if data_points.len() < 2 {
        return None;
    }

    // Simple linear regression: ln(BLER) = a * snr + b
    let n = data_points.len() as f64;
    let sum_x: f64 = data_points.iter().map(|(x, _)| x).sum();
    let sum_y: f64 = data_points.iter().map(|(_, y)| y).sum();
    let sum_xy: f64 = data_points.iter().map(|(x, y)| x * y).sum();
    let sum_xx: f64 = data_points.iter().map(|(x, _)| x * x).sum();

    let denom = n * sum_xx - sum_x * sum_x;
    if denom.abs() < 1e-15 {
        return None;
    }

    let slope = (n * sum_xy - sum_x * sum_y) / denom;
    let intercept = (sum_y - slope * sum_x) / n;

    // Estimate frame rate from the nearest completed point to each remaining point.
    let mut total_eta_secs = 0.0f64;
    for &snr in remaining_snr_points {
        // Extrapolate BLER.
        let ln_bler_est = slope * snr + intercept;
        let bler_est = ln_bler_est.exp().clamp(1e-12, 1.0);

        // Estimate frames needed.
        let frames_needed = if min_errors > 0 {
            ((min_errors as f64 / bler_est).ceil() as usize).min(max_frames)
        } else {
            max_frames
        };

        // Find nearest completed point by SNR for frame rate estimation.
        let nearest = completed
            .iter()
            .filter(|p| p.num_frames > 0 && p.duration.as_secs_f64() > 0.0)
            .min_by(|a, b| {
                let da = (a.eb_n0_db - snr).abs();
                let db = (b.eb_n0_db - snr).abs();
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            });

        if let Some(ref_point) = nearest {
            let frame_rate = ref_point.num_frames as f64 / ref_point.duration.as_secs_f64();
            if frame_rate > 0.0 {
                total_eta_secs += frames_needed as f64 / frame_rate;
            }
        }
    }

    if total_eta_secs > 0.0 {
        Some(std::time::Duration::from_secs_f64(total_eta_secs))
    } else {
        None
    }
}

/// Reports per-point completion with elapsed time and optional ETA.
///
/// Uses log-linear BLER extrapolation for sweep ETA when sufficient data
/// (2+ completed points with errors) is available. Falls back to "unknown"
/// otherwise.
///
/// # Arguments
///
/// * `eb_n0_db` - The completed SNR point.
/// * `result` - The completed simulation result.
/// * `point_elapsed` - Wall-clock time for this point.
/// * `remaining_snr_points` - SNR values still to simulate.
/// * `completed_points` - Info about previously completed points for ETA estimation.
/// * `min_errors` - Minimum errors per point (for frame count estimation).
/// * `max_frames` - Maximum frames per point (cap for estimation).
fn report_point_complete(
    eb_n0_db: f64,
    result: &SimulationResult,
    point_elapsed: std::time::Duration,
    remaining_snr_points: &[f64],
    completed_points: &[CompletedPointInfo],
    min_errors: usize,
    max_frames: usize,
) {
    let elapsed_str = format_duration(point_elapsed);
    let remaining = remaining_snr_points.len();
    let eta_str = if remaining > 0 {
        match estimate_sweep_eta(
            completed_points,
            remaining_snr_points,
            min_errors,
            max_frames,
        ) {
            Some(eta) => {
                let mut detail = format!(
                    " -- ETA ~{} for {} remaining point{}",
                    format_duration(eta),
                    remaining,
                    if remaining == 1 { "" } else { "s" }
                );
                // Show per-point breakdown for the next few points
                if let Some(breakdown) = estimate_per_point_eta(
                    completed_points,
                    remaining_snr_points,
                    min_errors,
                    max_frames,
                ) {
                    detail.push_str(&format!(" ({})", breakdown));
                }
                detail
            }
            None => format!(
                " -- {} remaining point{}, ETA unknown",
                remaining,
                if remaining == 1 { "" } else { "s" }
            ),
        }
    } else {
        String::new()
    };

    eprintln!(
        "[{:.1} dB] DONE: BLER={:.2e} ({} errors / {} frames) in {}{eta_str}",
        eb_n0_db, result.bler, result.num_frame_errors, result.num_frames, elapsed_str,
    );
}

/// Produces a compact per-point ETA breakdown string, e.g. "3.0dB~12m, 3.5dB~2h, 4.0dB~cap".
fn estimate_per_point_eta(
    completed: &[CompletedPointInfo],
    remaining_snr_points: &[f64],
    min_errors: usize,
    max_frames: usize,
) -> Option<String> {
    let data_points: Vec<(f64, f64)> = completed
        .iter()
        .filter(|p| p.bler > 0.0 && p.num_frames > 0)
        .map(|p| (p.eb_n0_db, p.bler.ln()))
        .collect();
    if data_points.len() < 2 {
        return None;
    }

    let n = data_points.len() as f64;
    let sum_x: f64 = data_points.iter().map(|(x, _)| x).sum();
    let sum_y: f64 = data_points.iter().map(|(_, y)| y).sum();
    let sum_xy: f64 = data_points.iter().map(|(x, y)| x * y).sum();
    let sum_xx: f64 = data_points.iter().map(|(x, _)| x * x).sum();
    let denom = n * sum_xx - sum_x * sum_x;
    if denom.abs() < 1e-15 {
        return None;
    }
    let slope = (n * sum_xy - sum_x * sum_y) / denom;
    let intercept = (sum_y - slope * sum_x) / n;

    let parts: Vec<String> = remaining_snr_points
        .iter()
        .take(4) // show at most 4 points
        .map(|&snr| {
            let bler_est = (slope * snr + intercept).exp().clamp(1e-12, 1.0);
            let frames_needed = if min_errors > 0 {
                ((min_errors as f64 / bler_est).ceil() as usize).min(max_frames)
            } else {
                max_frames
            };
            let nearest = completed
                .iter()
                .filter(|p| p.num_frames > 0 && p.duration.as_secs_f64() > 0.0)
                .min_by(|a, b| {
                    (a.eb_n0_db - snr)
                        .abs()
                        .partial_cmp(&(b.eb_n0_db - snr).abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            if let Some(ref_point) = nearest {
                let frame_rate = ref_point.num_frames as f64 / ref_point.duration.as_secs_f64();
                if frame_rate > 0.0 {
                    let secs = frames_needed as f64 / frame_rate;
                    if frames_needed >= max_frames {
                        format!("{:.1}dB~cap", snr)
                    } else {
                        format!(
                            "{:.1}dB~{}",
                            snr,
                            format_duration(std::time::Duration::from_secs_f64(secs))
                        )
                    }
                } else {
                    format!("{:.1}dB~?", snr)
                }
            } else {
                format!("{:.1}dB~?", snr)
            }
        })
        .collect();

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

/// Loads existing simulation results from a CSV file for resuming.
///
/// Parses each data row and returns a map keyed by the SNR value
/// formatted to 6 decimal places. Only results meeting the `min_errors`
/// threshold are included.
///
/// Returns an empty map if the file does not exist or cannot be parsed.
///
/// # Panics
///
/// Does not panic. I/O and parse errors are handled gracefully by
/// returning an empty map.
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
///
/// # Complexity
///
/// O(n) where n is the number of rows in the CSV file.
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
    /// Thread-safe: uses [`JSONL_WRITE_LOCK`] to serialize concurrent writes
    /// from parallel simulation workers.
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
        let _guard = JSONL_WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let result = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut file| writeln!(file, "{entry}"));
        drop(_guard);
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
        if let Err(e) = append_point_complete_jsonl(path, result, self.start_time.elapsed()) {
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

/// Formats and appends a `"type":"point_complete"` JSONL entry.
///
/// Shared implementation used by both `SnrAccumulator` (sequential) and
/// `ParallelResultCollector` (parallel) to avoid duplicating the schema.
fn append_point_complete_jsonl(
    path: &Path,
    result: &SimulationResult,
    elapsed: Duration,
) -> std::io::Result<()> {
    use std::io::Write;
    // Use the module-level JSONL_WRITE_LOCK to serialize all JSONL appends.
    let _guard = JSONL_WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let elapsed_s = elapsed.as_secs_f64();
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
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{entry}")
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
    remaining_snr_points: &'a [f64],
    completed_points: &'a [CompletedPointInfo],
    /// When `true`, `simulate_single_point` suppresses per-point completion
    /// reporting and CSV/JSONL writes because the caller (e.g., the parallel
    /// runner's `ParallelResultCollector`) handles them instead.
    suppress_completion_side_effects: bool,
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

    if !ctx.suppress_completion_side_effects {
        // Write point_complete JSONL entry.
        if let Some(pp) = ctx.progress_path {
            let mut acc_for_jsonl = SnrAccumulator::new(eb_n0_db, k);
            // Reuse the start time from the original accumulator via elapsed.
            acc_for_jsonl.start_time = Instant::now() - point_elapsed;
            acc_for_jsonl.write_point_complete_entry(pp, &sim_result);
        }

        // Incremental CSV append (only for CSV outputs; JSON is written at the end).
        if let Some(path) = ctx.output_path {
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                sim_result.append_csv_row_to(path);
            }
        }

        report_point_complete(
            eb_n0_db,
            &sim_result,
            point_elapsed,
            ctx.remaining_snr_points,
            ctx.completed_points,
            ctx.config.min_errors,
            ctx.config.max_frames,
        );
    }

    sim_result
}

/// Collects results from parallel SNR-point workers with synchronized I/O.
///
/// Each parallel worker calls [`record_completed_point`] when it finishes an SNR
/// point. The collector holds the `Mutex` only briefly to:
/// 1. Store the result at the correct index (preserving config ordering).
/// 2. Append a CSV row (if CSV output is configured).
/// 3. Write a `point_complete` JSONL entry (if output_path is set).
/// 4. Print per-point completion with ETA to stderr.
///
/// Intra-point JSONL progress entries are written directly by parallel workers
/// via append-mode file I/O (each `writeln!` is atomic for short lines on POSIX).
/// Entries include `eb_n0_db` so readers can demultiplex interleaved progress
/// from concurrent SNR points.
#[derive(Debug)]
struct ParallelResultCollector {
    /// Results indexed by SNR point index; `None` until completed.
    results: Vec<Option<SimulationResult>>,
    /// Path for CSV output (if configured and not JSON).
    output_path: Option<PathBuf>,
    /// Path for JSONL progress output (derived from output_path).
    progress_path: Option<PathBuf>,
    /// Number of completed SNR points so far.
    completed_count: usize,
    /// Info about completed points for ETA estimation.
    completed_points: Vec<CompletedPointInfo>,
    /// All SNR points in the sweep (for computing remaining points).
    all_snr_points: Vec<f64>,
    /// Minimum errors per point (for ETA estimation).
    min_errors: usize,
    /// Maximum frames per point (for ETA estimation).
    max_frames: usize,
    /// Set to `true` after the first JSONL write failure so we only warn once.
    progress_write_warned: bool,
}

impl ParallelResultCollector {
    /// Creates a new collector for the given number of SNR points.
    fn new(
        total_points: usize,
        output_path: Option<PathBuf>,
        progress_path: Option<PathBuf>,
        all_snr_points: Vec<f64>,
        min_errors: usize,
        max_frames: usize,
    ) -> Self {
        Self {
            results: vec![None; total_points],
            output_path,
            progress_path,
            completed_count: 0,
            completed_points: Vec::with_capacity(total_points),
            all_snr_points,
            min_errors,
            max_frames,
            progress_write_warned: false,
        }
    }

    /// Records a completed SNR point, writing CSV/JSONL and printing progress.
    ///
    /// Called by each parallel worker after `simulate_single_point` returns.
    /// The Mutex is held only for this brief I/O + bookkeeping window.
    fn record_completed_point(
        &mut self,
        index: usize,
        result: SimulationResult,
        point_elapsed: Duration,
    ) {
        self.results[index] = Some(result.clone());
        self.completed_count += 1;
        self.completed_points.push(CompletedPointInfo {
            eb_n0_db: result.eb_n0_db,
            duration: point_elapsed,
            num_frames: result.num_frames,
            bler: result.bler,
        });

        // Compute remaining SNR points (those not yet completed).
        let completed_snrs: Vec<f64> = self.completed_points.iter().map(|p| p.eb_n0_db).collect();
        let remaining_snr: Vec<f64> = self
            .all_snr_points
            .iter()
            .filter(|snr| !completed_snrs.iter().any(|c| (c - **snr).abs() < 1e-9))
            .copied()
            .collect();

        // Append CSV row immediately (if CSV output).
        if let Some(ref path) = self.output_path {
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                result.append_csv_row_to(path);
            }
        }

        // Write point_complete JSONL entry.
        if let Some(pp) = self.progress_path.clone() {
            self.write_point_complete_entry(&pp, &result, point_elapsed);
        }

        // Print per-point completion with ETA.
        report_point_complete(
            result.eb_n0_db,
            &result,
            point_elapsed,
            &remaining_snr,
            &self.completed_points,
            self.min_errors,
            self.max_frames,
        );
    }

    /// Appends a `"type":"point_complete"` JSONL entry with full result fields.
    fn write_point_complete_entry(
        &mut self,
        path: &Path,
        result: &SimulationResult,
        elapsed: Duration,
    ) {
        if let Err(e) = append_point_complete_jsonl(path, result, elapsed) {
            if !self.progress_write_warned {
                eprintln!(
                    "Warning: failed to write JSONL progress to {}: {e}",
                    path.display()
                );
                self.progress_write_warned = true;
            }
        }
    }

    /// Collects all results in index order. Panics if any slot is still `None`.
    fn into_results(self) -> Vec<SimulationResult> {
        self.results
            .into_iter()
            .enumerate()
            .map(|(i, opt)| opt.unwrap_or_else(|| panic!("SNR point {i} was never completed")))
            .collect()
    }
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

    // Resume from existing CSV results (not applicable for JSON outputs).
    let existing = config
        .output_path
        .as_ref()
        .filter(|p| p.extension().and_then(|e| e.to_str()) != Some("json"))
        .map_or_else(HashMap::new, |p| {
            try_load_existing_results(p, config.min_errors)
        });
    let progress_path = config.output_path.as_ref().map(|p| progress_path_for(p));

    let mut rng = config.make_rng();
    let mut points = Vec::with_capacity(config.eb_n0_range_db.len());
    let mut completed_points: Vec<CompletedPointInfo> = Vec::new();

    for (point_idx, &eb_n0_db) in config.eb_n0_range_db.iter().enumerate() {
        let remaining_snr: Vec<f64> = config.eb_n0_range_db[point_idx + 1..].to_vec();
        let point_start = Instant::now();
        let ctx = SnrPointContext {
            eb_n0_db,
            rate,
            config,
            existing: &existing,
            output_path: config.output_path.as_deref(),
            progress_path: progress_path.as_deref(),
            remaining_snr_points: &remaining_snr,
            completed_points: &completed_points,
            suppress_completion_side_effects: false,
        };
        let sim_result = simulate_single_point(encoder, channel, &mut rng, &ctx, &mut decode_frame);
        let point_elapsed = point_start.elapsed();
        completed_points.push(CompletedPointInfo {
            eb_n0_db,
            duration: point_elapsed,
            num_frames: sim_result.num_frames,
            bler: sim_result.bler,
        });
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
        let n = encoder.n();
        let k = encoder.k();
        let rate = k as f64 / n as f64;

        // Resume from existing CSV results (not applicable for JSON outputs).
        let existing = config
            .output_path
            .as_ref()
            .filter(|p| p.extension().and_then(|e| e.to_str()) != Some("json"))
            .map_or_else(HashMap::new, |p| {
                try_load_existing_results(p, config.min_errors)
            });

        let max_iter = config.max_decoder_iterations;
        let total_points = config.eb_n0_range_db.len();

        // Derive CSV-only output path (None for JSON outputs).
        let csv_output = config.output_path.as_ref().and_then(|p| {
            if p.extension().and_then(|e| e.to_str()) == Some("json") {
                None
            } else {
                Some(p.clone())
            }
        });
        let progress_path = config.output_path.as_ref().map(|p| progress_path_for(p));
        let worker_progress_path = progress_path.clone();

        let collector = Arc::new(Mutex::new(ParallelResultCollector::new(
            total_points,
            csv_output,
            progress_path,
            config.eb_n0_range_db.clone(),
            config.min_errors,
            config.max_frames,
        )));

        // Worker closure: simulates one SNR point, then locks the collector
        // briefly to record the result with immediate CSV/JSONL writes.
        let simulate_and_record = |(idx, &eb_n0_db): (usize, &f64)| {
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
                // CSV writes handled by ParallelResultCollector under Mutex.
                output_path: None,
                // JSONL progress: enabled for parallel workers. All JSONL
                // writes are serialized via the module-level JSONL_WRITE_LOCK
                // mutex. Entries include eb_n0_db so readers can demultiplex
                // interleaved progress from concurrent SNR points.
                progress_path: worker_progress_path.as_deref(),
                remaining_snr_points: &[],
                completed_points: &[],
                // The ParallelResultCollector handles CSV append and
                // per-point completion reporting with proper ETA tracking.
                suppress_completion_side_effects: true,
            };

            let point_start = Instant::now();
            let result = simulate_single_point(encoder, channel, &mut rng, &ctx, |llrs| {
                decoder.reset();
                decoder.decode_iterative(llrs, max_iter)
            });
            let point_elapsed = point_start.elapsed();

            // Lock the collector only for the brief I/O + bookkeeping window.
            let mut coll = collector
                .lock()
                .expect("ParallelResultCollector lock poisoned");
            coll.record_completed_point(idx, result, point_elapsed);
        };

        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;
            config
                .eb_n0_range_db
                .par_iter()
                .enumerate()
                .for_each(simulate_and_record);
        }
        #[cfg(not(feature = "parallel"))]
        {
            config
                .eb_n0_range_db
                .iter()
                .enumerate()
                .for_each(simulate_and_record);
        }

        let points = Arc::try_unwrap(collector)
            .expect("ParallelResultCollector Arc has outstanding references")
            .into_inner()
            .expect("ParallelResultCollector Mutex poisoned")
            .into_results();

        let results = SimulationResults { points };
        // Final overwrite with clean, complete file (CSV gets a sorted, header-
        // included version; JSON is written here since incremental JSON is not
        // supported).
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

    #[test]
    fn test_parallel_incremental_csv_append() {
        let dir = std::env::temp_dir().join("gf2_sim_par_incr_csv");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let csv_path = dir.join("par_incremental.csv");
        let jsonl_path = dir.join("par_incremental.progress.jsonl");

        let encoder = MockEncoder;
        let channel = BpskAwgnChannel;
        let mut config = SimulationConfig::quick_test();
        // Use 3 SNR points to test parallel incremental writes.
        config.eb_n0_range_db = vec![6.0, 8.0, 10.0];
        config.min_errors = 5;
        config.max_frames = 1000;
        config.rng_seed = Some(77);
        config.output_path = Some(csv_path.clone());

        let results = SimulationRunner::run_coded_iterative_parallel(
            &encoder,
            || MockIterativeDecoder { last_iterations: 0 },
            &channel,
            &config,
        );

        assert_eq!(results.points.len(), 3, "Must have 3 result points");

        // Verify final CSV has header + 3 data rows.
        let content = std::fs::read_to_string(&csv_path).unwrap();
        let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
        assert!(
            lines[0].contains("eb_n0_db"),
            "CSV header must be present, got: {}",
            lines[0]
        );
        assert_eq!(
            lines.len(),
            4,
            "CSV must have header + 3 data rows, got {} lines",
            lines.len()
        );

        // Verify JSONL has point_complete entries for each SNR point.
        assert!(
            jsonl_path.exists(),
            "JSONL progress file must exist at {}",
            jsonl_path.display()
        );
        let jsonl_content = std::fs::read_to_string(&jsonl_path).unwrap();
        let jsonl_lines: Vec<&str> = jsonl_content
            .lines()
            .filter(|l| l.contains("\"type\":\"point_complete\""))
            .collect();
        assert_eq!(
            jsonl_lines.len(),
            3,
            "JSONL must have 3 point_complete entries, got {}",
            jsonl_lines.len()
        );

        // Verify all 3 SNR values appear in point_complete entries.
        for &snr in &[6.0_f64, 8.0, 10.0] {
            let snr_str = format!("\"eb_n0_db\":{}", snr);
            assert!(
                jsonl_lines.iter().any(|l| l.contains(&snr_str)),
                "JSONL must have point_complete for {snr} dB"
            );
        }

        // Verify result ordering matches config order.
        assert!(
            (results.points[0].eb_n0_db - 6.0).abs() < 1e-10,
            "First point must be 6.0 dB"
        );
        assert!(
            (results.points[1].eb_n0_db - 8.0).abs() < 1e-10,
            "Second point must be 8.0 dB"
        );
        assert!(
            (results.points[2].eb_n0_db - 10.0).abs() < 1e-10,
            "Third point must be 10.0 dB"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // -------------------------------------------------------------------
    // Modem-backed uncoded runner (run_uncoded_ber_with_channel) tests
    // -------------------------------------------------------------------

    /// SC3(a): at matched Eb/N0 and the same `StdRng` seed, the modem-backed
    /// path through [`BpskAwgnChannel`] converges to the same BER as the
    /// legacy hard-coded BPSK path, within a small Monte Carlo tolerance.
    ///
    /// The two paths differ slightly in interleaving order (legacy draws all
    /// payload bits then all noise samples in one contiguous block; the
    /// generic runner uses 960-bit batches) so we do not require bit-exact
    /// match — we require BER closeness at a moderately-high Eb/N0 where
    /// both paths have settled.
    #[test]
    fn test_run_uncoded_ber_with_channel_matches_legacy_bpsk() {
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![5.0];
        config.min_errors = 200;
        config.max_frames = 200_000;
        config.rng_seed = Some(0xC0FFEE_u64);

        // Legacy BPSK path.
        let mut rng_legacy = StdRng::seed_from_u64(config.rng_seed.unwrap());
        let legacy = SimulationRunner::run_uncoded_ber(&config, &mut rng_legacy);

        // Modem-backed path through the same BpskAwgnChannel contract.
        let mut rng_modem = StdRng::seed_from_u64(config.rng_seed.unwrap());
        let channel = BpskAwgnChannel;
        let modem_path =
            SimulationRunner::run_uncoded_ber_with_channel(&channel, &config, &mut rng_modem);

        assert_eq!(legacy.len(), 1);
        assert_eq!(modem_path.len(), 1);

        let ber_legacy = legacy[0].ber;
        let ber_modem = modem_path[0].ber;

        assert!(
            ber_legacy.is_finite() && ber_modem.is_finite(),
            "both BERs must be finite: legacy={ber_legacy}, modem={ber_modem}",
        );
        assert!(
            ber_legacy > 0.0 && ber_modem > 0.0,
            "at 5 dB both paths should observe some errors: legacy={ber_legacy}, modem={ber_modem}",
        );
        // Tolerance: relative error below 40% covers Monte Carlo variance
        // at min_errors=200 between two independently interleaved paths
        // without hiding a systematic noise-convention drift (which would
        // produce >2x or >0.5x relative deviations).
        let ratio = ber_modem / ber_legacy;
        assert!(
            ratio > 0.6 && ratio < 1.4,
            "modem BER should track legacy BER within ~40% (got ratio={ratio}: legacy={ber_legacy}, modem={ber_modem})",
        );
    }

    /// SC3(b): [`ModemChannelAdapter`] built on BPSK (the smallest modem
    /// spec) plugs into [`SimulationRunner::run_uncoded_ber_with_channel`]
    /// and returns a sane result (non-zero frame count, finite BER).
    ///
    /// Covers the reusability requirement that the new runner accepts any
    /// `ChannelModel` — BPSK here, but the type is
    /// `ModemChannelAdapter<GrayQamMapper<f32>, ReferenceSoftDemapper<f32>>`,
    /// the exact surface coded runners (`run_coded`, `run_coded_iterative`)
    /// already accept.
    #[test]
    fn test_run_uncoded_ber_with_channel_supports_modem_adapter() {
        use crate::modem::{
            DemapMethod, GrayQamMapper, ModemChannelAdapter, ModemSpec, ReferenceSoftDemapper,
        };

        let mapper = GrayQamMapper::<f32>::from_preset_order(2); // BPSK
        let demap = ReferenceSoftDemapper::new(ModemSpec::<f32>::bpsk());
        let adapter = ModemChannelAdapter::new(mapper, demap, DemapMethod::ExactLogMap);

        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![3.0];
        config.min_errors = 50;
        config.max_frames = 50_000;
        config.rng_seed = Some(0xA5A5_A5A5_u64);

        let mut rng = StdRng::seed_from_u64(config.rng_seed.unwrap());
        let results = SimulationRunner::run_uncoded_ber_with_channel(&adapter, &config, &mut rng);

        assert_eq!(results.len(), 1);
        let r = &results[0];
        assert!(
            r.num_bits > 0,
            "modem-adapter uncoded sweep must transmit at least one batch of bits",
        );
        assert!(r.ber.is_finite(), "BER must be finite, got {}", r.ber);
        assert!(
            (0.0..=1.0).contains(&r.ber),
            "uncoded BER must lie in [0, 1], got {}",
            r.ber,
        );
    }

    /// Regression locking in that the uncoded runner honours the
    /// `batch_alignment()` contract for the existing QPSK Rician fading
    /// path as well as for modem-framework channels.
    ///
    /// `QpskRicianChannelModel::transmit_and_demodulate` asserts an
    /// even codeword length. Before the batch-alignment fix the runner
    /// could feed it an odd tail and panic; with
    /// `QpskRicianChannelModel::batch_alignment() -> 2` the runner must
    /// round each batch down and never panic. `max_frames = 963` is
    /// intentionally not divisible by 2.
    #[test]
    fn test_run_uncoded_ber_with_channel_handles_ragged_tail_for_qpsk_rician() {
        use crate::fading::{QpskRicianChannelModel, RicianConfig};

        let channel = QpskRicianChannelModel::new(RicianConfig::fig8());
        assert_eq!(
            channel.batch_alignment(),
            2,
            "QpskRicianChannelModel must declare alignment 2"
        );

        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![3.0];
        config.min_errors = 1;
        config.max_frames = 963; // intentionally not divisible by 2
        config.rng_seed = Some(0xFADE_CAFE_u64);

        let mut rng = StdRng::seed_from_u64(config.rng_seed.unwrap());
        // Must not panic even though max_frames is odd.
        let results = SimulationRunner::run_uncoded_ber_with_channel(&channel, &config, &mut rng);
        assert_eq!(results.len(), 1);
        let r = &results[0];
        assert!(r.ber.is_finite());
        assert!(
            r.num_bits % 2 == 0,
            "transmitted bits must stay aligned to the QPSK fading channel's 2-bit requirement"
        );
    }

    /// Regression for ragged-tail safety on modem-backed channels with
    /// `bits_per_symbol > 1`.
    ///
    /// Earlier code computed `batch_size = UNCODED_MODEM_BATCH_BITS.min(remaining)`
    /// without respecting the channel's required alignment, which would
    /// panic inside `ModemChannelAdapter::transmit_and_demodulate` on
    /// the final batch when `max_frames` was not a multiple of
    /// `bits_per_symbol`. The runner now honours
    /// `ChannelModel::batch_alignment()` and rounds every batch down,
    /// exercised here with a QPSK adapter and `max_frames = 963` (not
    /// divisible by 2).
    #[test]
    fn test_run_uncoded_ber_with_channel_handles_ragged_tail_for_qpsk() {
        use crate::modem::{
            DemapMethod, GrayQamMapper, ModemChannelAdapter, ModemSpec, ReferenceSoftDemapper,
        };

        let mapper = GrayQamMapper::<f32>::from_preset_order(4); // QPSK
        let demap = ReferenceSoftDemapper::new(ModemSpec::<f32>::gray_square_qam(4));
        let adapter = ModemChannelAdapter::new(mapper, demap, DemapMethod::ExactLogMap);
        assert_eq!(
            adapter.batch_alignment(),
            2,
            "QPSK adapter must require alignment 2"
        );

        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![3.0];
        config.min_errors = 1; // terminate quickly
        config.max_frames = 963; // intentionally not divisible by 2
        config.rng_seed = Some(0xCAFE_F00D_u64);

        let mut rng = StdRng::seed_from_u64(config.rng_seed.unwrap());
        // Must not panic: each inner batch must be pre-aligned to 2.
        let results = SimulationRunner::run_uncoded_ber_with_channel(&adapter, &config, &mut rng);
        assert_eq!(results.len(), 1);
        let r = &results[0];
        assert!(r.ber.is_finite());
        assert!(r.num_bits % 2 == 0, "transmitted bits must stay aligned");
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
