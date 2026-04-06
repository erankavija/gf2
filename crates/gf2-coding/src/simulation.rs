//! Monte Carlo simulation framework for BER/FER performance analysis.
//!
//! This module provides reusable utilities for running communication system
//! simulations over AWGN channels, supporting both bit error rate (BER) and
//! frame error rate (FER) measurements.
//!
//! # Overview
//!
//! The simulation harness supports two modes:
//!
//! - **Uncoded**: Direct BPSK transmission over AWGN (`run_uncoded_ber`).
//! - **Coded**: Full encode-modulate-channel-demodulate-decode loop generic
//!   over encoder, decoder, and channel model (`run_coded`, `run_coded_iterative`).
//!
//! # Channel abstraction
//!
//! The [`ChannelModel`] trait decouples the simulation loop from any specific
//! modulation/channel combination. A default [`BpskAwgnChannel`] implementation
//! is provided for BPSK over AWGN.
//!
//! # Parallel sweeps
//!
//! When the `parallel` feature is enabled, the non-iterative `run_coded` path
//! dispatches each SNR point to a separate rayon thread. The iterative path
//! (`run_coded_iterative`) requires `&mut self` on the decoder, so parallel
//! execution requires a decoder factory closure (see `run_coded_iterative`).
//!
//! # Output
//!
//! Results can be exported to CSV or JSON via [`SimulationRunner::results_to_csv`]
//! and [`SimulationRunner::coded_results_to_json`].

use crate::channel::{AwgnChannel, BpskModulator};
use crate::llr::Llr;
use crate::traits::{BlockEncoder, IterativeSoftDecoder, SoftDecoder};
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::path::PathBuf;

/// Configuration for Monte Carlo simulations.
#[derive(Debug, Clone)]
pub struct SimulationConfig {
    /// Range of Eb/N0 values to simulate (in dB)
    pub eb_n0_range: Vec<f64>,

    /// Minimum number of errors to collect before stopping at each SNR point
    pub min_errors: usize,

    /// Maximum number of trials (bits or frames) per SNR point
    pub max_trials: usize,

    /// Code rate (k/n) for computing SNR from Eb/N0
    pub code_rate: f64,

    /// Frame size in bits (for FER simulations)
    pub frame_size: Option<usize>,
}

impl SimulationConfig {
    /// Creates a default configuration for quick testing.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::SimulationConfig;
    ///
    /// let config = SimulationConfig::quick_test();
    /// assert_eq!(config.min_errors, 100);
    /// ```
    pub fn quick_test() -> Self {
        SimulationConfig {
            eb_n0_range: vec![0.0, 3.0, 6.0],
            min_errors: 100,
            max_trials: 100_000,
            code_rate: 1.0,
            frame_size: None,
        }
    }

    /// Creates a configuration for high-precision BER curves.
    pub fn high_precision() -> Self {
        SimulationConfig {
            eb_n0_range: (0..=10).map(|i| i as f64).collect(),
            min_errors: 1000,
            max_trials: 10_000_000,
            code_rate: 1.0,
            frame_size: None,
        }
    }
}

/// Results from a single SNR point simulation.
#[derive(Debug, Clone)]
pub struct SimulationResult {
    /// Eb/N0 in dB
    pub eb_n0_db: f64,

    /// Bit error rate (errors / total bits)
    pub ber: f64,

    /// Frame error rate (frame errors / total frames), if applicable
    pub fer: Option<f64>,

    /// Total number of bits transmitted
    pub num_bits: usize,

    /// Total number of bit errors observed
    pub num_errors: usize,

    /// Number of frames transmitted (for FER)
    pub num_frames: Option<usize>,

    /// Number of frames with errors (for FER)
    pub num_frame_errors: Option<usize>,
}

impl SimulationResult {
    /// Returns true if this result meets the minimum error requirement.
    pub fn is_complete(&self, min_errors: usize) -> bool {
        self.num_errors >= min_errors
    }

    /// Exports result as CSV row: "eb_n0_db,ber,num_bits,num_errors"
    pub fn to_csv_row(&self) -> String {
        if let (Some(fer), Some(num_frames), Some(num_frame_errors)) =
            (self.fer, self.num_frames, self.num_frame_errors)
        {
            format!(
                "{},{},{},{},{},{},{}",
                self.eb_n0_db,
                self.ber,
                self.num_bits,
                self.num_errors,
                fer,
                num_frames,
                num_frame_errors
            )
        } else {
            format!(
                "{},{},{},{}",
                self.eb_n0_db, self.ber, self.num_bits, self.num_errors
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Channel model abstraction
// ---------------------------------------------------------------------------

/// Abstraction over modulation and channel for coded simulations.
///
/// Implementations transform a codeword (as bits) into soft LLR values
/// suitable for soft-decision decoding, encapsulating modulation, noise
/// injection, and demodulation in a single method.
///
/// # Arguments (of `transmit_and_demodulate`)
///
/// * `codeword_bits` — the encoded bits to transmit
/// * `eb_n0_db` — energy-per-bit to noise spectral density in dB
/// * `code_rate` — k/n of the code
/// * `rng` — random number generator for noise sampling
///
/// # Examples
///
/// ```
/// use gf2_coding::simulation::{ChannelModel, BpskAwgnChannel};
/// use gf2_core::BitVec;
///
/// let channel = BpskAwgnChannel;
/// let codeword = BitVec::zeros(7);
/// let mut rng = rand::thread_rng();
/// let llrs = channel.transmit_and_demodulate(&codeword, 3.0, 0.5, &mut rng);
/// assert_eq!(llrs.len(), 7);
/// ```
pub trait ChannelModel {
    /// Modulates, transmits through the channel, and demodulates to LLRs.
    ///
    /// # Arguments
    ///
    /// * `codeword_bits` - Encoded codeword as a `BitVec`
    /// * `eb_n0_db` - Eb/N0 in dB
    /// * `code_rate` - Code rate k/n
    /// * `rng` - Random number generator
    ///
    /// # Returns
    ///
    /// Vector of LLRs, one per codeword bit.
    fn transmit_and_demodulate<R: Rng>(
        &self,
        codeword_bits: &BitVec,
        eb_n0_db: f64,
        code_rate: f64,
        rng: &mut R,
    ) -> Vec<Llr>;
}

/// BPSK modulation over an AWGN channel.
///
/// Maps `0 -> +1`, `1 -> -1`, adds Gaussian noise, and converts
/// received symbols to LLRs using `2r / sigma^2`.
///
/// # Examples
///
/// ```
/// use gf2_coding::simulation::{ChannelModel, BpskAwgnChannel};
/// use gf2_core::BitVec;
///
/// let ch = BpskAwgnChannel;
/// let bits = BitVec::zeros(4);
/// let mut rng = rand::thread_rng();
/// let llrs = ch.transmit_and_demodulate(&bits, 5.0, 1.0, &mut rng);
/// assert_eq!(llrs.len(), 4);
/// ```
pub struct BpskAwgnChannel;

impl ChannelModel for BpskAwgnChannel {
    fn transmit_and_demodulate<R: Rng>(
        &self,
        codeword_bits: &BitVec,
        eb_n0_db: f64,
        code_rate: f64,
        rng: &mut R,
    ) -> Vec<Llr> {
        let n = codeword_bits.len();
        let bits_vec: Vec<bool> = (0..n).map(|i| codeword_bits.get(i)).collect();
        let symbols = BpskModulator::modulate_bits(&bits_vec);
        let channel = AwgnChannel::from_eb_n0_db(eb_n0_db, code_rate);
        let received = channel.transmit_symbols(&symbols, rng);
        channel.to_llrs(&received)
    }
}

// ---------------------------------------------------------------------------
// Coded simulation configuration and results
// ---------------------------------------------------------------------------

/// Configuration for coded BER/BLER Monte Carlo sweeps.
///
/// Controls the SNR range, early-termination thresholds, and optional
/// output path for CSV export.
///
/// # Examples
///
/// ```
/// use gf2_coding::simulation::CodedSimulationConfig;
///
/// let config = CodedSimulationConfig {
///     eb_n0_range_db: vec![0.0, 1.0, 2.0, 3.0],
///     min_errors: 100,
///     max_frames: 10_000,
///     max_decoder_iterations: 50,
///     rng_seed: Some(42),
///     output_path: None,
/// };
/// assert_eq!(config.min_errors, 100);
/// ```
#[derive(Debug, Clone)]
pub struct CodedSimulationConfig {
    /// Eb/N0 values in dB to sweep.
    pub eb_n0_range_db: Vec<f64>,

    /// Minimum number of frame errors before stopping at each SNR point.
    /// Typical value is 100 for statistically meaningful results.
    pub min_errors: usize,

    /// Maximum number of frames to simulate per SNR point.
    pub max_frames: usize,

    /// Maximum decoder iterations (used for [`IterativeSoftDecoder`]).
    pub max_decoder_iterations: usize,

    /// Optional seed for deterministic RNG.
    /// When `Some(seed)`, each SNR point uses `seed ^ point_index` for
    /// reproducible results. When `None`, uses a fixed default seed
    /// (`0x5EED_CAFE ^ point_index`), which is still deterministic but
    /// not caller-controlled.
    pub rng_seed: Option<u64>,

    /// Optional path to write CSV results after simulation.
    pub output_path: Option<PathBuf>,
}

/// Per-SNR-point statistics from a coded simulation.
///
/// Contains BER, BLER, iteration counts, and query-per-bit metrics
/// collected during the Monte Carlo run at a single Eb/N0 point.
///
/// # Examples
///
/// ```
/// use gf2_coding::simulation::CodedSimulationResult;
///
/// let r = CodedSimulationResult {
///     eb_n0_db: 3.0,
///     ber: 1e-3,
///     bler: 0.05,
///     num_bit_errors: 40,
///     num_bits: 40_000,
///     num_block_errors: 5,
///     num_frames: 100,
///     avg_iterations: 12.5,
///     avg_queries_per_bit: 12.5,
/// };
/// assert!(r.ber < r.bler);
/// ```
#[derive(Debug, Clone)]
pub struct CodedSimulationResult {
    /// Eb/N0 in dB.
    pub eb_n0_db: f64,

    /// Bit error rate = num_bit_errors / num_bits.
    pub ber: f64,

    /// Block error rate = num_block_errors / num_frames.
    pub bler: f64,

    /// Total bit errors observed.
    pub num_bit_errors: usize,

    /// Total information bits transmitted.
    pub num_bits: usize,

    /// Total frame (block) errors observed.
    pub num_block_errors: usize,

    /// Total frames simulated.
    pub num_frames: usize,

    /// Average decoder iterations per frame.
    pub avg_iterations: f64,

    /// Average queries per information bit.
    ///
    /// Computed as `total_decoder_iterations / total_info_bits`. For standard
    /// iterative decoders (e.g., LDPC BP), `DecoderResult.iterations` counts
    /// BP iterations; for query-based decoders (e.g., GRAND), it counts
    /// noise pattern queries. This field normalizes per information bit in
    /// both cases.
    pub avg_queries_per_bit: f64,
}

/// Aggregated results from a full coded simulation sweep.
///
/// Wraps all per-SNR results and provides serialization helpers.
///
/// # Examples
///
/// ```
/// use gf2_coding::simulation::CodedSimulationResults;
///
/// let results = CodedSimulationResults { points: vec![] };
/// assert!(results.points.is_empty());
/// ```
#[derive(Debug, Clone)]
pub struct CodedSimulationResults {
    /// One entry per SNR point, in the order they were requested.
    pub points: Vec<CodedSimulationResult>,
}

/// Progress information reported during a coded simulation.
///
/// Passed to the optional progress callback so callers can display
/// ETA, logging, or progress bars.
///
/// # Examples
///
/// ```
/// use gf2_coding::simulation::ProgressReport;
///
/// let p = ProgressReport {
///     eb_n0_db: 3.0,
///     frames_done: 50,
///     max_frames: 1000,
///     block_errors_so_far: 2,
///     min_errors_target: 100,
/// };
/// assert_eq!(p.frames_done, 50);
/// ```
#[derive(Debug, Clone)]
pub struct ProgressReport {
    /// Current SNR point being simulated.
    pub eb_n0_db: f64,

    /// Number of frames completed so far at this SNR point.
    pub frames_done: usize,

    /// Maximum frames configured for this SNR point.
    pub max_frames: usize,

    /// Block errors collected so far at this SNR point.
    pub block_errors_so_far: usize,

    /// Target number of block errors for early termination.
    pub min_errors_target: usize,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Determines whether the simulation loop should terminate.
///
/// Terminates when either:
/// - `min_errors > 0` and `block_errors >= min_errors` (enough statistics collected)
/// - `frames >= max_frames` (budget exhausted)
///
/// When `min_errors == 0`, termination is based solely on `max_frames`.
fn early_terminate(
    block_errors: usize,
    min_errors: usize,
    frames: usize,
    max_frames: usize,
) -> bool {
    if frames >= max_frames {
        return true;
    }
    if min_errors > 0 && block_errors >= min_errors {
        return true;
    }
    false
}

/// Counts bit differences between two `BitVec`s of the same length.
///
/// # Arguments
///
/// * `a` - First bit vector
/// * `b` - Second bit vector
///
/// # Panics
///
/// Panics if the two vectors have different lengths.
///
/// # Complexity
///
/// O(n/64) where n is the bit length.
fn count_bit_errors(a: &BitVec, b: &BitVec) -> usize {
    assert_eq!(a.len(), b.len(), "BitVec lengths must match");
    a.words()
        .iter()
        .zip(b.words().iter())
        .map(|(&wa, &wb)| (wa ^ wb).count_ones() as usize)
        .sum()
}

/// Simulates a single SNR point for a non-iterative soft decoder.
///
/// # Arguments
///
/// * `encoder` - Block encoder for the code under test
/// * `decoder` - Soft-decision decoder
/// * `channel` - Channel model
/// * `config` - Simulation configuration
/// * `eb_n0_db` - The Eb/N0 value in dB
/// * `rng` - Random number generator
/// * `progress_cb` - Optional progress callback
///
/// # Complexity
///
/// O(max_frames * n) where n is the codeword length.
fn simulate_snr_point_soft<E, D, C, F>(
    encoder: &E,
    decoder: &D,
    channel: &C,
    config: &CodedSimulationConfig,
    eb_n0_db: f64,
    rng: &mut StdRng,
    progress_cb: &Option<F>,
) -> CodedSimulationResult
where
    E: BlockEncoder,
    D: SoftDecoder,
    C: ChannelModel,
    F: Fn(&ProgressReport),
{
    let k = encoder.k();
    let n = encoder.n();
    let code_rate = k as f64 / n as f64;

    let mut total_bit_errors = 0usize;
    let mut total_block_errors = 0usize;
    let mut total_frames = 0usize;
    let mut total_bits = 0usize;
    let mut total_iterations = 0usize;
    let mut total_queries = 0usize;

    while !early_terminate(
        total_block_errors,
        config.min_errors,
        total_frames,
        config.max_frames,
    ) {
        // Generate random message
        let message = BitVec::random(k, rng);

        // Encode
        let codeword = encoder.encode(&message);

        // Channel: modulate, add noise, demodulate to LLR
        let llrs = channel.transmit_and_demodulate(&codeword, eb_n0_db, code_rate, rng);

        // Decode
        let result = decoder.decode_soft_with_result(&llrs);

        // Count errors
        let bit_errors = count_bit_errors(&message, &result.decoded_bits);
        total_bit_errors += bit_errors;
        if bit_errors > 0 {
            total_block_errors += 1;
        }
        total_iterations += result.iterations;
        total_queries += result.queries.unwrap_or(result.iterations);
        total_frames += 1;
        total_bits += k;

        // Progress reporting (every 100 frames)
        if let Some(cb) = progress_cb {
            if total_frames % 100 == 0 {
                cb(&ProgressReport {
                    eb_n0_db,
                    frames_done: total_frames,
                    max_frames: config.max_frames,
                    block_errors_so_far: total_block_errors,
                    min_errors_target: config.min_errors,
                });
            }
        }
    }

    let ber = if total_bits > 0 {
        total_bit_errors as f64 / total_bits as f64
    } else {
        0.0
    };
    let bler = if total_frames > 0 {
        total_block_errors as f64 / total_frames as f64
    } else {
        0.0
    };
    let avg_iterations = if total_frames > 0 {
        total_iterations as f64 / total_frames as f64
    } else {
        0.0
    };
    let avg_queries_per_bit = if total_bits > 0 {
        total_queries as f64 / total_bits as f64
    } else {
        0.0
    };

    CodedSimulationResult {
        eb_n0_db,
        ber,
        bler,
        num_bit_errors: total_bit_errors,
        num_bits: total_bits,
        num_block_errors: total_block_errors,
        num_frames: total_frames,
        avg_iterations,
        avg_queries_per_bit,
    }
}

/// Simulates a single SNR point for an iterative soft decoder.
///
/// # Arguments
///
/// * `encoder` - Block encoder for the code under test
/// * `decoder` - Iterative soft-decision decoder
/// * `channel` - Channel model
/// * `config` - Simulation configuration
/// * `eb_n0_db` - The Eb/N0 value in dB
/// * `rng` - Random number generator
/// * `progress_cb` - Optional progress callback
///
/// # Complexity
///
/// O(max_frames * n * max_decoder_iterations) where n is the codeword length.
fn simulate_snr_point_iterative<E, D, C, F>(
    encoder: &E,
    decoder: &mut D,
    channel: &C,
    config: &CodedSimulationConfig,
    eb_n0_db: f64,
    rng: &mut StdRng,
    progress_cb: &Option<F>,
) -> CodedSimulationResult
where
    E: BlockEncoder,
    D: IterativeSoftDecoder,
    C: ChannelModel,
    F: Fn(&ProgressReport),
{
    let k = encoder.k();
    let n = encoder.n();
    let code_rate = k as f64 / n as f64;

    let mut total_bit_errors = 0usize;
    let mut total_block_errors = 0usize;
    let mut total_frames = 0usize;
    let mut total_bits = 0usize;
    let mut total_iterations = 0usize;
    let mut total_queries = 0usize;

    while !early_terminate(
        total_block_errors,
        config.min_errors,
        total_frames,
        config.max_frames,
    ) {
        // Generate random message
        let message = BitVec::random(k, rng);

        // Encode
        let codeword = encoder.encode(&message);

        // Channel: modulate, add noise, demodulate to LLR
        let llrs = channel.transmit_and_demodulate(&codeword, eb_n0_db, code_rate, rng);

        // Decode with iteration control
        decoder.reset();
        let result = decoder.decode_iterative(&llrs, config.max_decoder_iterations);

        // Count errors
        let bit_errors = count_bit_errors(&message, &result.decoded_bits);
        total_bit_errors += bit_errors;
        if bit_errors > 0 {
            total_block_errors += 1;
        }
        total_iterations += result.iterations;
        total_queries += result.queries.unwrap_or(result.iterations);
        total_frames += 1;
        total_bits += k;

        // Progress reporting (every 100 frames)
        if let Some(cb) = progress_cb {
            if total_frames % 100 == 0 {
                cb(&ProgressReport {
                    eb_n0_db,
                    frames_done: total_frames,
                    max_frames: config.max_frames,
                    block_errors_so_far: total_block_errors,
                    min_errors_target: config.min_errors,
                });
            }
        }
    }

    let ber = if total_bits > 0 {
        total_bit_errors as f64 / total_bits as f64
    } else {
        0.0
    };
    let bler = if total_frames > 0 {
        total_block_errors as f64 / total_frames as f64
    } else {
        0.0
    };
    let avg_iterations = if total_frames > 0 {
        total_iterations as f64 / total_frames as f64
    } else {
        0.0
    };
    let avg_queries_per_bit = if total_bits > 0 {
        total_queries as f64 / total_bits as f64
    } else {
        0.0
    };

    CodedSimulationResult {
        eb_n0_db,
        ber,
        bler,
        num_bit_errors: total_bit_errors,
        num_bits: total_bits,
        num_block_errors: total_block_errors,
        num_frames: total_frames,
        avg_iterations,
        avg_queries_per_bit,
    }
}

// ---------------------------------------------------------------------------
// SimulationRunner
// ---------------------------------------------------------------------------

/// Monte Carlo simulation runner for communication systems.
///
/// Provides methods for uncoded BER analysis and coded BER/BLER sweeps
/// with support for both single-shot and iterative soft decoders.
pub struct SimulationRunner;

impl SimulationRunner {
    /// Simulates uncoded transmission over AWGN and computes BER.
    ///
    /// # Arguments
    ///
    /// * `config` - Simulation configuration
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
    /// assert_eq!(results.len(), config.eb_n0_range.len());
    /// ```
    pub fn run_uncoded_ber<R: Rng>(
        config: &SimulationConfig,
        rng: &mut R,
    ) -> Vec<SimulationResult> {
        config
            .eb_n0_range
            .iter()
            .map(|&eb_n0_db| {
                let channel = AwgnChannel::from_eb_n0_db(eb_n0_db, config.code_rate);

                let mut total_bits = 0;
                let mut total_errors = 0;

                while total_errors < config.min_errors && total_bits < config.max_trials {
                    // Transmit batch of bits
                    let batch_size = 1000.min(config.max_trials - total_bits);
                    let bits = BitVec::random(batch_size, rng);

                    // Modulate and transmit
                    let bits_vec: Vec<bool> = (0..batch_size).map(|i| bits.get(i)).collect();
                    let symbols = BpskModulator::modulate_bits(&bits_vec);
                    let received = channel.transmit_symbols(&symbols, rng);

                    // Hard-decision demodulation
                    let decoded: Vec<bool> = received
                        .iter()
                        .map(|&r| BpskModulator::demodulate_hard(r))
                        .collect();

                    // Count errors
                    let errors = (0..batch_size)
                        .filter(|&i| bits.get(i) != decoded[i])
                        .count();

                    total_bits += batch_size;
                    total_errors += errors;
                }

                let ber = total_errors as f64 / total_bits as f64;

                SimulationResult {
                    eb_n0_db,
                    ber,
                    fer: None,
                    num_bits: total_bits,
                    num_errors: total_errors,
                    num_frames: None,
                    num_frame_errors: None,
                }
            })
            .collect()
    }

    /// Runs a coded BER/BLER sweep with a non-iterative [`SoftDecoder`].
    ///
    /// The simulation loop at each SNR point is:
    /// encode -> modulate -> channel -> demodulate -> decode -> count errors.
    ///
    /// Early termination fires when `min_errors` block errors are collected
    /// or `max_frames` frames have been simulated.
    ///
    /// When `config.output_path` is `Some(path)`, a CSV file is written at
    /// the end of the sweep.
    ///
    /// # Arguments
    ///
    /// * `encoder` - Block encoder producing codewords
    /// * `decoder` - Soft-decision decoder
    /// * `channel` - Channel model (e.g., [`BpskAwgnChannel`])
    /// * `config` - Coded simulation configuration
    ///
    /// # Returns
    ///
    /// Aggregated per-SNR results in a [`CodedSimulationResults`] struct.
    ///
    /// # Panics
    ///
    /// Panics if `encoder.n() != decoder.n()` or `encoder.k() != decoder.k()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::{
    ///     SimulationRunner, CodedSimulationConfig, BpskAwgnChannel,
    /// };
    /// use gf2_coding::LinearBlockCode;
    /// use gf2_coding::linear::SyndromeTableDecoder;
    /// use gf2_coding::traits::BlockEncoder;
    ///
    /// // Hamming(7,4) with a simple soft decoder wrapper is not directly
    /// // available, so this doc-test just shows config construction.
    /// let config = CodedSimulationConfig {
    ///     eb_n0_range_db: vec![4.0, 6.0],
    ///     min_errors: 10,
    ///     max_frames: 1_000,
    ///     max_decoder_iterations: 1,
    ///     rng_seed: Some(42),
    ///     output_path: None,
    /// };
    /// assert_eq!(config.eb_n0_range_db.len(), 2);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(|eb_n0_range| * max_frames * n) worst case, but early termination
    /// typically reduces this significantly at high SNR.
    pub fn run_coded<E, D, C>(
        encoder: &E,
        decoder: &D,
        channel: &C,
        config: &CodedSimulationConfig,
    ) -> CodedSimulationResults
    where
        E: BlockEncoder + Sync,
        D: SoftDecoder + Sync,
        C: ChannelModel + Sync,
    {
        Self::run_coded_with_progress(
            encoder,
            decoder,
            channel,
            config,
            None::<fn(&ProgressReport)>,
        )
    }

    /// Like [`run_coded`](Self::run_coded) but with an optional progress
    /// callback invoked every 100 frames per SNR point.
    ///
    /// # Arguments
    ///
    /// * `encoder` - Block encoder
    /// * `decoder` - Soft-decision decoder
    /// * `channel` - Channel model
    /// * `config` - Coded simulation configuration
    /// * `progress` - Optional callback receiving [`ProgressReport`]
    ///
    /// # Panics
    ///
    /// Panics if `encoder.n() != decoder.n()` or `encoder.k() != decoder.k()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::{
    ///     SimulationRunner, CodedSimulationConfig, BpskAwgnChannel, ProgressReport,
    /// };
    ///
    /// let config = CodedSimulationConfig {
    ///     eb_n0_range_db: vec![4.0],
    ///     min_errors: 10,
    ///     max_frames: 500,
    ///     max_decoder_iterations: 1,
    ///     rng_seed: Some(42),
    ///     output_path: None,
    /// };
    /// // progress callback example (no-op)
    /// let _cb = |_p: &ProgressReport| {};
    /// ```
    ///
    /// # Complexity
    ///
    /// O(|eb_n0_range| * max_frames * n).
    pub fn run_coded_with_progress<E, D, C, F>(
        encoder: &E,
        decoder: &D,
        channel: &C,
        config: &CodedSimulationConfig,
        progress: Option<F>,
    ) -> CodedSimulationResults
    where
        E: BlockEncoder + Sync,
        D: SoftDecoder + Sync,
        C: ChannelModel + Sync,
        F: Fn(&ProgressReport) + Sync,
    {
        assert_eq!(
            encoder.n(),
            decoder.n(),
            "Encoder n ({}) must match decoder n ({})",
            encoder.n(),
            decoder.n()
        );
        assert_eq!(
            encoder.k(),
            decoder.k(),
            "Encoder k ({}) must match decoder k ({})",
            encoder.k(),
            decoder.k()
        );

        let points = run_snr_sweep_soft(encoder, decoder, channel, config, &progress);

        let results = CodedSimulationResults { points };

        // Write CSV if output path is set
        if let Some(ref path) = config.output_path {
            let csv = Self::coded_results_to_csv(&results);
            std::fs::write(path, &csv).unwrap_or_else(|e| {
                eprintln!("Warning: failed to write CSV to {}: {}", path.display(), e);
            });
        }

        results
    }

    /// Runs a coded BER/BLER sweep with an [`IterativeSoftDecoder`].
    ///
    /// Identical to [`run_coded`](Self::run_coded) except the decoder
    /// receives `max_decoder_iterations` from the config, and decoder
    /// state is reset between frames.
    ///
    /// # Arguments
    ///
    /// * `encoder` - Block encoder
    /// * `decoder` - Iterative soft-decision decoder (requires `&mut`)
    /// * `channel` - Channel model
    /// * `config` - Coded simulation configuration
    ///
    /// # Returns
    ///
    /// Aggregated per-SNR results.
    ///
    /// # Panics
    ///
    /// Panics if `encoder.n() != decoder.n()` or `encoder.k() != decoder.k()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::{
    ///     SimulationRunner, CodedSimulationConfig, BpskAwgnChannel,
    /// };
    ///
    /// let config = CodedSimulationConfig {
    ///     eb_n0_range_db: vec![3.0],
    ///     min_errors: 10,
    ///     max_frames: 500,
    ///     max_decoder_iterations: 50,
    ///     rng_seed: Some(42),
    ///     output_path: None,
    /// };
    /// // To use: SimulationRunner::run_coded_iterative(&encoder, &mut decoder, &channel, &config)
    /// ```
    ///
    /// # Complexity
    ///
    /// O(|eb_n0_range| * max_frames * n * max_decoder_iterations).
    pub fn run_coded_iterative<E, D, C>(
        encoder: &E,
        decoder: &mut D,
        channel: &C,
        config: &CodedSimulationConfig,
    ) -> CodedSimulationResults
    where
        E: BlockEncoder,
        D: IterativeSoftDecoder,
        C: ChannelModel,
    {
        Self::run_coded_iterative_with_progress(
            encoder,
            decoder,
            channel,
            config,
            None::<fn(&ProgressReport)>,
        )
    }

    /// Like [`run_coded_iterative`](Self::run_coded_iterative) but with a
    /// progress callback.
    ///
    /// # Arguments
    ///
    /// * `encoder` - Block encoder
    /// * `decoder` - Iterative soft-decision decoder
    /// * `channel` - Channel model
    /// * `config` - Coded simulation configuration
    /// * `progress` - Optional callback receiving [`ProgressReport`]
    ///
    /// # Panics
    ///
    /// Panics if `encoder.n() != decoder.n()` or `encoder.k() != decoder.k()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::{
    ///     SimulationRunner, CodedSimulationConfig, BpskAwgnChannel, ProgressReport,
    /// };
    ///
    /// let config = CodedSimulationConfig {
    ///     eb_n0_range_db: vec![3.0],
    ///     min_errors: 10,
    ///     max_frames: 500,
    ///     max_decoder_iterations: 50,
    ///     rng_seed: Some(42),
    ///     output_path: None,
    /// };
    /// // progress callback example (no-op)
    /// let _cb = |_p: &ProgressReport| {};
    /// ```
    ///
    /// # Complexity
    ///
    /// O(|eb_n0_range| * max_frames * n * max_decoder_iterations).
    pub fn run_coded_iterative_with_progress<E, D, C, F>(
        encoder: &E,
        decoder: &mut D,
        channel: &C,
        config: &CodedSimulationConfig,
        progress: Option<F>,
    ) -> CodedSimulationResults
    where
        E: BlockEncoder,
        D: IterativeSoftDecoder,
        C: ChannelModel,
        F: Fn(&ProgressReport),
    {
        assert_eq!(
            encoder.n(),
            decoder.n(),
            "Encoder n ({}) must match decoder n ({})",
            encoder.n(),
            decoder.n()
        );
        assert_eq!(
            encoder.k(),
            decoder.k(),
            "Encoder k ({}) must match decoder k ({})",
            encoder.k(),
            decoder.k()
        );

        let points: Vec<CodedSimulationResult> = config
            .eb_n0_range_db
            .iter()
            .enumerate()
            .map(|(idx, &eb_n0_db)| {
                let seed = config.rng_seed.unwrap_or(0x5EED_CAFE) ^ (idx as u64);
                let mut rng = StdRng::seed_from_u64(seed);
                simulate_snr_point_iterative(
                    encoder, decoder, channel, config, eb_n0_db, &mut rng, &progress,
                )
            })
            .collect();

        let results = CodedSimulationResults { points };

        if let Some(ref path) = config.output_path {
            let csv = Self::coded_results_to_csv(&results);
            std::fs::write(path, &csv).unwrap_or_else(|e| {
                eprintln!("Warning: failed to write CSV to {}: {}", path.display(), e);
            });
        }

        results
    }

    /// Exports simulation results to CSV format.
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
        let mut csv = String::new();

        if include_header {
            // Determine if we have FER data
            let has_fer = results.iter().any(|r| r.fer.is_some());

            if has_fer {
                csv.push_str("eb_n0_db,ber,num_bits,num_errors,fer,num_frames,num_frame_errors\n");
            } else {
                csv.push_str("eb_n0_db,ber,num_bits,num_errors\n");
            }
        }

        for result in results {
            csv.push_str(&result.to_csv_row());
            csv.push('\n');
        }

        csv
    }

    /// Serializes coded simulation results to CSV.
    ///
    /// The CSV contains columns: `eb_n0_db`, `ber`, `bler`, `num_bit_errors`,
    /// `num_bits`, `num_block_errors`, `num_frames`, `avg_iterations`,
    /// `avg_queries_per_bit`.
    ///
    /// # Arguments
    ///
    /// * `results` - The coded simulation results to serialize
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::{SimulationRunner, CodedSimulationResults, CodedSimulationResult};
    ///
    /// let results = CodedSimulationResults {
    ///     points: vec![CodedSimulationResult {
    ///         eb_n0_db: 3.0,
    ///         ber: 0.001,
    ///         bler: 0.05,
    ///         num_bit_errors: 10,
    ///         num_bits: 10000,
    ///         num_block_errors: 5,
    ///         num_frames: 100,
    ///         avg_iterations: 10.0,
    ///         avg_queries_per_bit: 10.0,
    ///     }],
    /// };
    /// let csv = SimulationRunner::coded_results_to_csv(&results);
    /// assert!(csv.contains("eb_n0_db"));
    /// assert!(csv.contains("bler"));
    /// assert!(csv.contains("avg_queries_per_bit"));
    /// ```
    ///
    /// # Complexity
    ///
    /// O(n) where n is the number of SNR points.
    pub fn coded_results_to_csv(results: &CodedSimulationResults) -> String {
        let mut csv = String::from(
            "eb_n0_db,ber,bler,num_bit_errors,num_bits,num_block_errors,num_frames,avg_iterations,avg_queries_per_bit\n",
        );

        for p in &results.points {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{},{}\n",
                p.eb_n0_db,
                p.ber,
                p.bler,
                p.num_bit_errors,
                p.num_bits,
                p.num_block_errors,
                p.num_frames,
                p.avg_iterations,
                p.avg_queries_per_bit,
            ));
        }

        csv
    }

    /// Serializes coded simulation results to JSON.
    ///
    /// Returns a JSON array of objects, one per SNR point, with the same
    /// fields as the CSV output.
    ///
    /// # Arguments
    ///
    /// * `results` - The coded simulation results to serialize
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::{SimulationRunner, CodedSimulationResults, CodedSimulationResult};
    ///
    /// let results = CodedSimulationResults {
    ///     points: vec![CodedSimulationResult {
    ///         eb_n0_db: 3.0,
    ///         ber: 0.001,
    ///         bler: 0.05,
    ///         num_bit_errors: 10,
    ///         num_bits: 10000,
    ///         num_block_errors: 5,
    ///         num_frames: 100,
    ///         avg_iterations: 10.0,
    ///         avg_queries_per_bit: 10.0,
    ///     }],
    /// };
    /// let json = SimulationRunner::coded_results_to_json(&results);
    /// assert!(json.contains("\"eb_n0_db\""));
    /// assert!(json.contains("\"avg_queries_per_bit\""));
    /// ```
    ///
    /// # Complexity
    ///
    /// O(n) where n is the number of SNR points.
    pub fn coded_results_to_json(results: &CodedSimulationResults) -> String {
        let mut json = String::from("[\n");

        for (i, p) in results.points.iter().enumerate() {
            json.push_str(&format!(
                "  {{\n    \"eb_n0_db\": {},\n    \"ber\": {},\n    \"bler\": {},\n    \"num_bit_errors\": {},\n    \"num_bits\": {},\n    \"num_block_errors\": {},\n    \"num_frames\": {},\n    \"avg_iterations\": {},\n    \"avg_queries_per_bit\": {}\n  }}",
                p.eb_n0_db,
                p.ber,
                p.bler,
                p.num_bit_errors,
                p.num_bits,
                p.num_block_errors,
                p.num_frames,
                p.avg_iterations,
                p.avg_queries_per_bit,
            ));
            if i + 1 < results.points.len() {
                json.push(',');
            }
            json.push('\n');
        }

        json.push(']');
        json
    }
}

// ---------------------------------------------------------------------------
// Internal sweep dispatchers (sequential / parallel)
// ---------------------------------------------------------------------------

/// Sequential SNR sweep for non-iterative soft decoders.
#[cfg(not(feature = "parallel"))]
fn run_snr_sweep_soft<E, D, C, F>(
    encoder: &E,
    decoder: &D,
    channel: &C,
    config: &CodedSimulationConfig,
    progress_cb: &Option<F>,
) -> Vec<CodedSimulationResult>
where
    E: BlockEncoder + Sync,
    D: SoftDecoder + Sync,
    C: ChannelModel + Sync,
    F: Fn(&ProgressReport) + Sync,
{
    config
        .eb_n0_range_db
        .iter()
        .enumerate()
        .map(|(idx, &eb_n0_db)| {
            let seed = config.rng_seed.unwrap_or(0x5EED_CAFE) ^ (idx as u64);
            let mut rng = StdRng::seed_from_u64(seed);
            simulate_snr_point_soft(
                encoder,
                decoder,
                channel,
                config,
                eb_n0_db,
                &mut rng,
                progress_cb,
            )
        })
        .collect()
}

/// Parallel SNR sweep for non-iterative soft decoders.
#[cfg(feature = "parallel")]
fn run_snr_sweep_soft<E, D, C, F>(
    encoder: &E,
    decoder: &D,
    channel: &C,
    config: &CodedSimulationConfig,
    progress_cb: &Option<F>,
) -> Vec<CodedSimulationResult>
where
    E: BlockEncoder + Sync,
    D: SoftDecoder + Sync,
    C: ChannelModel + Sync,
    F: Fn(&ProgressReport) + Sync,
{
    use rayon::prelude::*;

    config
        .eb_n0_range_db
        .par_iter()
        .enumerate()
        .map(|(idx, &eb_n0_db)| {
            let seed = config.rng_seed.unwrap_or(0x5EED_CAFE) ^ (idx as u64);
            let mut rng = StdRng::seed_from_u64(seed);
            simulate_snr_point_soft(
                encoder,
                decoder,
                channel,
                config,
                eb_n0_db,
                &mut rng,
                progress_cb,
            )
        })
        .collect()
}

/// Parallel SNR sweep for iterative soft decoders.
///
/// Each SNR point creates a fresh decoder via `make_decoder()` so that
/// `&mut self` is available per-thread without shared mutable state.
#[cfg(feature = "parallel")]
fn run_snr_sweep_iterative<E, D, C, MkD, F>(
    encoder: &E,
    channel: &C,
    config: &CodedSimulationConfig,
    make_decoder: &MkD,
    progress_cb: &Option<F>,
) -> Vec<CodedSimulationResult>
where
    E: BlockEncoder + Sync,
    D: IterativeSoftDecoder,
    C: ChannelModel + Sync,
    MkD: Fn() -> D + Sync,
    F: Fn(&ProgressReport) + Sync,
{
    use rayon::prelude::*;

    config
        .eb_n0_range_db
        .par_iter()
        .enumerate()
        .map(|(idx, &eb_n0_db)| {
            let seed = config.rng_seed.unwrap_or(0x5EED_CAFE) ^ (idx as u64);
            let mut rng = StdRng::seed_from_u64(seed);
            let mut decoder = make_decoder();
            simulate_snr_point_iterative(
                encoder,
                &mut decoder,
                channel,
                config,
                eb_n0_db,
                &mut rng,
                progress_cb,
            )
        })
        .collect()
}

impl SimulationRunner {
    /// Runs a coded iterative simulation sweep in parallel across SNR points.
    ///
    /// Each SNR point gets a fresh decoder from `make_decoder()`, enabling
    /// parallel execution despite `IterativeSoftDecoder` requiring `&mut self`.
    ///
    /// Requires the `parallel` feature.
    ///
    /// # Arguments
    ///
    /// * `encoder` - Block encoder (shared, `Sync`)
    /// * `make_decoder` - Factory closure creating a fresh decoder per thread
    /// * `channel` - Channel model (shared, `Sync`)
    /// * `config` - Simulation configuration
    ///
    /// # Complexity
    ///
    /// O(max_frames * n) total work, parallelized over SNR points.
    #[cfg(feature = "parallel")]
    pub fn run_coded_iterative_parallel<E, D, C, MkD>(
        encoder: &E,
        make_decoder: MkD,
        channel: &C,
        config: &CodedSimulationConfig,
    ) -> CodedSimulationResults
    where
        E: BlockEncoder + Sync,
        D: IterativeSoftDecoder,
        C: ChannelModel + Sync,
        MkD: Fn() -> D + Sync,
    {
        let points = run_snr_sweep_iterative(
            encoder,
            channel,
            config,
            &make_decoder,
            &None::<fn(&ProgressReport)>,
        );
        let results = CodedSimulationResults { points };

        if let Some(ref path) = config.output_path {
            let csv = Self::coded_results_to_csv(&results);
            std::fs::write(path, csv).expect("Failed to write CSV output");
        }

        results
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{BlockEncoder, DecoderResult, IterativeSoftDecoder, SoftDecoder};

    // -----------------------------------------------------------------------
    // Existing uncoded tests (preserved)
    // -----------------------------------------------------------------------

    #[test]
    fn test_simulation_config_quick() {
        let config = SimulationConfig::quick_test();
        assert!(config.min_errors > 0);
        assert!(config.max_trials > config.min_errors);
        assert_eq!(config.code_rate, 1.0);
    }

    #[test]
    fn test_uncoded_ber_simulation() {
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range = vec![10.0]; // High SNR for fast test
        config.min_errors = 10;
        config.max_trials = 10_000;

        let mut rng = rand::thread_rng();
        let results = SimulationRunner::run_uncoded_ber(&config, &mut rng);

        assert_eq!(results.len(), 1);
        assert!(results[0].ber < 0.01); // Should be low at 10 dB
        assert!(results[0].ber >= 0.0);
    }

    #[test]
    fn test_ber_decreases_with_snr() {
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range = vec![0.0, 6.0];
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
        let results = vec![SimulationResult {
            eb_n0_db: 3.0,
            ber: 0.01,
            fer: None,
            num_bits: 10000,
            num_errors: 100,
            num_frames: None,
            num_frame_errors: None,
        }];

        let csv = SimulationRunner::results_to_csv(&results, true);
        eprintln!("CSV output:\n{}", csv);
        assert!(csv.contains("eb_n0_db"));
        assert!(csv.contains("3"));
        assert!(csv.contains("0.01"));
    }

    #[test]
    fn test_simulation_result_complete() {
        let result = SimulationResult {
            eb_n0_db: 3.0,
            ber: 0.01,
            fer: None,
            num_bits: 10000,
            num_errors: 100,
            num_frames: None,
            num_frame_errors: None,
        };

        assert!(result.is_complete(50));
        assert!(!result.is_complete(200));
    }

    // -----------------------------------------------------------------------
    // Mock encoder / decoder for coded tests
    // -----------------------------------------------------------------------

    /// Trivial (3,1) repetition code encoder: each bit is repeated 3 times.
    struct RepetitionEncoder;

    impl BlockEncoder for RepetitionEncoder {
        fn k(&self) -> usize {
            1
        }
        fn n(&self) -> usize {
            3
        }
        fn encode(&self, message: &BitVec) -> BitVec {
            assert_eq!(message.len(), 1, "Repetition(3,1) requires 1-bit message");
            let bit = message.get(0);
            let mut cw = BitVec::with_capacity(3);
            for _ in 0..3 {
                cw.push_bit(bit);
            }
            cw
        }
    }

    /// Majority-vote soft decoder for the (3,1) repetition code.
    struct RepetitionSoftDecoder;

    impl SoftDecoder for RepetitionSoftDecoder {
        fn k(&self) -> usize {
            1
        }
        fn n(&self) -> usize {
            3
        }
        fn decode_soft(&self, llrs: &[Llr]) -> BitVec {
            assert_eq!(llrs.len(), 3);
            // Sum LLRs; positive => bit 0, negative => bit 1
            let sum: f32 = llrs.iter().map(|l| l.value()).sum();
            let mut out = BitVec::with_capacity(1);
            out.push_bit(sum < 0.0);
            out
        }
        fn decode_soft_with_result(&self, llrs: &[Llr]) -> DecoderResult {
            let decoded = self.decode_soft(llrs);
            DecoderResult::new(decoded, 1, true, true)
        }
    }

    /// Hamming(7,4) soft decoder: hard-decision on LLRs then syndrome decode.
    struct Hamming74SoftDecoder {
        inner: crate::linear::SyndromeTableDecoder,
    }

    impl Hamming74SoftDecoder {
        fn new() -> Self {
            let code = crate::linear::LinearBlockCode::hamming(3);
            Self {
                inner: crate::linear::SyndromeTableDecoder::new(code),
            }
        }
    }

    impl SoftDecoder for Hamming74SoftDecoder {
        fn k(&self) -> usize {
            4
        }
        fn n(&self) -> usize {
            7
        }
        fn decode_soft(&self, llrs: &[Llr]) -> BitVec {
            assert_eq!(llrs.len(), 7);
            // Hard decision on each LLR
            let mut hard = BitVec::with_capacity(7);
            for &l in llrs {
                hard.push_bit(l.hard_decision());
            }
            use crate::traits::HardDecisionDecoder;
            self.inner.decode(&hard)
        }
        fn decode_soft_with_result(&self, llrs: &[Llr]) -> DecoderResult {
            let decoded = self.decode_soft(llrs);
            DecoderResult::new(decoded, 1, true, true)
        }
    }

    /// Mock iterative soft decoder for the (3,1) repetition code.
    struct RepetitionIterativeDecoder {
        last_iter: usize,
    }

    impl RepetitionIterativeDecoder {
        fn new() -> Self {
            Self { last_iter: 0 }
        }
    }

    impl SoftDecoder for RepetitionIterativeDecoder {
        fn k(&self) -> usize {
            1
        }
        fn n(&self) -> usize {
            3
        }
        fn decode_soft(&self, llrs: &[Llr]) -> BitVec {
            assert_eq!(llrs.len(), 3);
            let sum: f32 = llrs.iter().map(|l| l.value()).sum();
            let mut out = BitVec::with_capacity(1);
            out.push_bit(sum < 0.0);
            out
        }
    }

    impl IterativeSoftDecoder for RepetitionIterativeDecoder {
        fn decode_iterative(&mut self, llrs: &[Llr], max_iterations: usize) -> DecoderResult {
            assert_eq!(llrs.len(), 3);
            // "Converge" in min(max_iterations, 3) iterations
            let iters = max_iterations.min(3);
            self.last_iter = iters;
            let decoded = self.decode_soft(llrs);
            DecoderResult::new(decoded, iters, iters < max_iterations, true)
        }
        fn last_iteration_count(&self) -> usize {
            self.last_iter
        }
        fn reset(&mut self) {
            self.last_iter = 0;
        }
    }

    // -----------------------------------------------------------------------
    // Coded simulation tests: SoftDecoder
    // -----------------------------------------------------------------------

    #[test]
    fn test_run_coded_basic_ber_bler() {
        let encoder = RepetitionEncoder;
        let decoder = RepetitionSoftDecoder;
        let channel = BpskAwgnChannel;

        let config = CodedSimulationConfig {
            eb_n0_range_db: vec![0.0, 6.0],
            min_errors: 20,
            max_frames: 5_000,
            max_decoder_iterations: 1,
            rng_seed: Some(12345),
            output_path: None,
        };

        let results = SimulationRunner::run_coded(&encoder, &decoder, &channel, &config);

        assert_eq!(results.points.len(), 2);
        for p in &results.points {
            assert!(p.ber >= 0.0 && p.ber <= 1.0, "BER out of range: {}", p.ber);
            assert!(
                p.bler >= 0.0 && p.bler <= 1.0,
                "BLER out of range: {}",
                p.bler
            );
            assert!(p.num_frames > 0);
            assert!(p.num_bits > 0);
            assert!(p.avg_iterations >= 0.0);
            assert!(p.avg_queries_per_bit >= 0.0);
        }
    }

    #[test]
    fn test_run_coded_ber_decreases_with_snr() {
        let encoder = RepetitionEncoder;
        let decoder = RepetitionSoftDecoder;
        let channel = BpskAwgnChannel;

        let config = CodedSimulationConfig {
            eb_n0_range_db: vec![0.0, 8.0],
            min_errors: 30,
            max_frames: 50_000,
            max_decoder_iterations: 1,
            rng_seed: Some(99),
            output_path: None,
        };

        let results = SimulationRunner::run_coded(&encoder, &decoder, &channel, &config);
        assert!(
            results.points[1].ber < results.points[0].ber,
            "BER should decrease: {} vs {}",
            results.points[1].ber,
            results.points[0].ber
        );
    }

    #[test]
    fn test_run_coded_early_termination() {
        let encoder = RepetitionEncoder;
        let decoder = RepetitionSoftDecoder;
        let channel = BpskAwgnChannel;

        // Very low SNR, should collect min_errors quickly
        let config = CodedSimulationConfig {
            eb_n0_range_db: vec![-2.0],
            min_errors: 10,
            max_frames: 100_000,
            max_decoder_iterations: 1,
            rng_seed: Some(7),
            output_path: None,
        };

        let results = SimulationRunner::run_coded(&encoder, &decoder, &channel, &config);
        let p = &results.points[0];

        // Should have stopped well before max_frames
        assert!(
            p.num_block_errors >= 10,
            "Should have collected min_errors: got {}",
            p.num_block_errors
        );
        assert!(
            p.num_frames < 100_000,
            "Should have terminated early: {} frames",
            p.num_frames
        );
    }

    #[test]
    fn test_run_coded_max_frames_termination() {
        let encoder = RepetitionEncoder;
        let decoder = RepetitionSoftDecoder;
        let channel = BpskAwgnChannel;

        // Very high SNR, almost no errors -> should hit max_frames
        let config = CodedSimulationConfig {
            eb_n0_range_db: vec![20.0],
            min_errors: 1000,
            max_frames: 200,
            max_decoder_iterations: 1,
            rng_seed: Some(1),
            output_path: None,
        };

        let results = SimulationRunner::run_coded(&encoder, &decoder, &channel, &config);
        let p = &results.points[0];
        assert_eq!(p.num_frames, 200, "Should stop at max_frames");
    }

    // -----------------------------------------------------------------------
    // Coded simulation tests: IterativeSoftDecoder
    // -----------------------------------------------------------------------

    #[test]
    fn test_run_coded_iterative_basic() {
        let encoder = RepetitionEncoder;
        let mut decoder = RepetitionIterativeDecoder::new();
        let channel = BpskAwgnChannel;

        let config = CodedSimulationConfig {
            eb_n0_range_db: vec![0.0, 6.0],
            min_errors: 10,
            max_frames: 5_000,
            max_decoder_iterations: 50,
            rng_seed: Some(42),
            output_path: None,
        };

        let results =
            SimulationRunner::run_coded_iterative(&encoder, &mut decoder, &channel, &config);

        assert_eq!(results.points.len(), 2);
        for p in &results.points {
            assert!(p.ber >= 0.0);
            assert!(p.bler >= 0.0);
            // Iterative decoder should report >0 avg iterations
            assert!(p.avg_iterations > 0.0);
        }
    }

    #[test]
    fn test_run_coded_iterative_max_decoder_iters_wired() {
        let encoder = RepetitionEncoder;
        let mut decoder = RepetitionIterativeDecoder::new();
        let channel = BpskAwgnChannel;

        // Mock converges at 3 iters, so with max=2 it won't converge
        let config = CodedSimulationConfig {
            eb_n0_range_db: vec![6.0],
            min_errors: 5,
            max_frames: 1000,
            max_decoder_iterations: 2,
            rng_seed: Some(42),
            output_path: None,
        };

        let results =
            SimulationRunner::run_coded_iterative(&encoder, &mut decoder, &channel, &config);

        // avg_iterations should be <= 2 (max_decoder_iterations)
        assert!(
            results.points[0].avg_iterations <= 2.0,
            "avg_iterations should respect max: {}",
            results.points[0].avg_iterations
        );
    }

    // -----------------------------------------------------------------------
    // Progress reporting
    // -----------------------------------------------------------------------

    #[test]
    fn test_run_coded_with_progress_callback() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let encoder = RepetitionEncoder;
        let decoder = RepetitionSoftDecoder;
        let channel = BpskAwgnChannel;

        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = Arc::clone(&call_count);

        // Use high min_errors so the simulation runs well past 100 frames,
        // ensuring the progress callback (fired every 100 frames) triggers.
        let config = CodedSimulationConfig {
            eb_n0_range_db: vec![-2.0],
            min_errors: 200,
            max_frames: 50_000,
            max_decoder_iterations: 1,
            rng_seed: Some(7),
            output_path: None,
        };

        let _results = SimulationRunner::run_coded_with_progress(
            &encoder,
            &decoder,
            &channel,
            &config,
            Some(move |report: &ProgressReport| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                assert!(report.frames_done > 0);
                assert_eq!(report.min_errors_target, 200);
            }),
        );

        let count = call_count.load(Ordering::SeqCst);
        assert!(
            count >= 1,
            "Progress callback should have been called at least once, got {}",
            count
        );
    }

    // -----------------------------------------------------------------------
    // CSV / JSON output
    // -----------------------------------------------------------------------

    #[test]
    fn test_coded_results_to_csv() {
        let results = CodedSimulationResults {
            points: vec![
                CodedSimulationResult {
                    eb_n0_db: 0.0,
                    ber: 0.1,
                    bler: 0.5,
                    num_bit_errors: 100,
                    num_bits: 1000,
                    num_block_errors: 50,
                    num_frames: 100,
                    avg_iterations: 5.0,
                    avg_queries_per_bit: 5.0,
                },
                CodedSimulationResult {
                    eb_n0_db: 3.0,
                    ber: 0.01,
                    bler: 0.05,
                    num_bit_errors: 10,
                    num_bits: 1000,
                    num_block_errors: 5,
                    num_frames: 100,
                    avg_iterations: 3.0,
                    avg_queries_per_bit: 3.0,
                },
            ],
        };

        let csv = SimulationRunner::coded_results_to_csv(&results);
        assert!(csv.contains("eb_n0_db"));
        assert!(csv.contains("bler"));
        assert!(csv.contains("avg_iterations"));
        assert!(csv.contains("avg_queries_per_bit"));
        // Should have header + 2 data rows
        let lines: Vec<&str> = csv.trim().lines().collect();
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_coded_results_to_json() {
        let results = CodedSimulationResults {
            points: vec![CodedSimulationResult {
                eb_n0_db: 3.0,
                ber: 0.001,
                bler: 0.05,
                num_bit_errors: 10,
                num_bits: 10000,
                num_block_errors: 5,
                num_frames: 100,
                avg_iterations: 10.0,
                avg_queries_per_bit: 10.0,
            }],
        };

        let json = SimulationRunner::coded_results_to_json(&results);
        assert!(json.contains("\"eb_n0_db\""));
        assert!(json.contains("\"ber\""));
        assert!(json.contains("\"bler\""));
        assert!(json.contains("\"avg_queries_per_bit\""));
        assert!(json.starts_with('['));
        assert!(json.ends_with(']'));
    }

    // -----------------------------------------------------------------------
    // CSV file output
    // -----------------------------------------------------------------------

    #[test]
    fn test_coded_csv_output_to_file() {
        let dir = std::env::temp_dir().join("gf2_sim_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_output.csv");

        let encoder = RepetitionEncoder;
        let decoder = RepetitionSoftDecoder;
        let channel = BpskAwgnChannel;

        let config = CodedSimulationConfig {
            eb_n0_range_db: vec![0.0],
            min_errors: 5,
            max_frames: 5_000,
            max_decoder_iterations: 1,
            rng_seed: Some(42),
            output_path: Some(path.clone()),
        };

        let _results = SimulationRunner::run_coded(&encoder, &decoder, &channel, &config);

        assert!(path.exists(), "CSV file should have been created");
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("eb_n0_db"));
        assert!(contents.contains("bler"));

        // Cleanup
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    // -----------------------------------------------------------------------
    // Hand-calculated deterministic BER test
    // -----------------------------------------------------------------------

    #[test]
    fn test_hand_calculated_deterministic_ber() {
        // Use Hamming(7,4) with a fixed seed.
        // We independently replay the exact same RNG sequence to compute
        // expected bit errors, then verify the simulation matches exactly.

        let encoder = crate::linear::LinearBlockCode::hamming(3);
        let decoder = Hamming74SoftDecoder::new();
        let channel = BpskAwgnChannel;

        let eb_n0_db = 2.0;
        let k = 4;
        let n = 7;
        let code_rate = k as f64 / n as f64;
        let num_frames = 200;
        let seed = 0xDEAD_BEEF_u64;

        // --- Run the simulation ---
        let config = CodedSimulationConfig {
            eb_n0_range_db: vec![eb_n0_db],
            min_errors: 0, // don't terminate early; we want exactly num_frames
            max_frames: num_frames,
            max_decoder_iterations: 1,
            rng_seed: Some(seed),
            output_path: None,
        };

        let results = SimulationRunner::run_coded(&encoder, &decoder, &channel, &config);
        let sim_result = &results.points[0];

        // --- Independently replay the same RNG to compute expected errors ---
        // The seed for SNR index 0 is: seed ^ 0 = seed
        let mut rng = StdRng::seed_from_u64(seed);
        let mut expected_bit_errors = 0usize;
        let mut expected_block_errors = 0usize;

        for _ in 0..num_frames {
            // Same message generation as simulation
            let message = BitVec::random(k, &mut rng);
            let codeword = encoder.encode(&message);

            // Same channel realization
            let llrs = channel.transmit_and_demodulate(&codeword, eb_n0_db, code_rate, &mut rng);

            // Same decoding
            let result = decoder.decode_soft_with_result(&llrs);

            let bit_errors = count_bit_errors(&message, &result.decoded_bits);
            expected_bit_errors += bit_errors;
            if bit_errors > 0 {
                expected_block_errors += 1;
            }
        }

        // Exact match required since the RNG sequence is identical
        assert_eq!(
            sim_result.num_bit_errors, expected_bit_errors,
            "Bit errors mismatch: sim={} expected={}",
            sim_result.num_bit_errors, expected_bit_errors
        );
        assert_eq!(
            sim_result.num_block_errors, expected_block_errors,
            "Block errors mismatch: sim={} expected={}",
            sim_result.num_block_errors, expected_block_errors
        );
        assert_eq!(sim_result.num_frames, num_frames);
        assert_eq!(sim_result.num_bits, num_frames * k);

        // Verify BER computation
        let expected_ber = expected_bit_errors as f64 / (num_frames * k) as f64;
        assert!(
            (sim_result.ber - expected_ber).abs() < 1e-15,
            "BER mismatch: sim={} expected={}",
            sim_result.ber,
            expected_ber
        );
    }

    // -----------------------------------------------------------------------
    // Statistics accuracy: hand-computed avg_iterations / avg_queries_per_bit
    // -----------------------------------------------------------------------

    #[test]
    fn test_avg_iterations_and_queries_per_bit() {
        let encoder = RepetitionEncoder;
        let mut decoder = RepetitionIterativeDecoder::new();
        let channel = BpskAwgnChannel;

        let config = CodedSimulationConfig {
            eb_n0_range_db: vec![6.0],
            min_errors: 5,
            max_frames: 500,
            max_decoder_iterations: 50,
            rng_seed: Some(42),
            output_path: None,
        };

        let results =
            SimulationRunner::run_coded_iterative(&encoder, &mut decoder, &channel, &config);
        let p = &results.points[0];

        // Mock converges in 3 iters (min(50, 3) = 3)
        // So avg_iterations should be exactly 3.0
        assert!(
            (p.avg_iterations - 3.0).abs() < 1e-10,
            "Expected avg_iterations=3.0, got {}",
            p.avg_iterations
        );

        // k=1, so avg_queries_per_bit = total_iters / total_bits = 3 * frames / (1 * frames) = 3.0
        assert!(
            (p.avg_queries_per_bit - 3.0).abs() < 1e-10,
            "Expected avg_queries_per_bit=3.0, got {}",
            p.avg_queries_per_bit
        );
    }

    // -----------------------------------------------------------------------
    // Channel model is a parameter, not hardcoded
    // -----------------------------------------------------------------------

    /// Custom channel model for testing that the trait is respected.
    struct NoiselessChannel;

    impl ChannelModel for NoiselessChannel {
        fn transmit_and_demodulate<R: Rng>(
            &self,
            codeword_bits: &BitVec,
            _eb_n0_db: f64,
            _code_rate: f64,
            _rng: &mut R,
        ) -> Vec<Llr> {
            // Perfect channel: return high-confidence LLRs matching the codeword
            (0..codeword_bits.len())
                .map(|i| {
                    if codeword_bits.get(i) {
                        Llr::new(-10.0) // bit 1 -> negative LLR
                    } else {
                        Llr::new(10.0) // bit 0 -> positive LLR
                    }
                })
                .collect()
        }
    }

    #[test]
    fn test_custom_channel_model_noiseless() {
        let encoder = RepetitionEncoder;
        let decoder = RepetitionSoftDecoder;
        let channel = NoiselessChannel;

        let config = CodedSimulationConfig {
            eb_n0_range_db: vec![0.0],
            min_errors: 5,
            max_frames: 1000,
            max_decoder_iterations: 1,
            rng_seed: Some(42),
            output_path: None,
        };

        let results = SimulationRunner::run_coded(&encoder, &decoder, &channel, &config);
        let p = &results.points[0];

        // Noiseless channel -> zero errors, should hit max_frames
        assert_eq!(p.num_bit_errors, 0);
        assert_eq!(p.num_block_errors, 0);
        assert_eq!(p.num_frames, 1000);
        assert_eq!(p.ber, 0.0);
        assert_eq!(p.bler, 0.0);
    }

    // -----------------------------------------------------------------------
    // count_bit_errors helper
    // -----------------------------------------------------------------------

    #[test]
    fn test_count_bit_errors_identical() {
        let a = BitVec::zeros(64);
        let b = BitVec::zeros(64);
        assert_eq!(count_bit_errors(&a, &b), 0);
    }

    #[test]
    fn test_count_bit_errors_all_different() {
        let a = BitVec::zeros(8);
        let b = BitVec::ones(8);
        assert_eq!(count_bit_errors(&a, &b), 8);
    }

    #[test]
    fn test_count_bit_errors_one_bit() {
        let a = BitVec::zeros(4);
        let mut b = BitVec::zeros(4);
        b.set(2, true);
        assert_eq!(count_bit_errors(&a, &b), 1);
    }

    #[test]
    fn test_count_bit_errors_cross_word() {
        // 65 bits: crosses word boundary
        let a = BitVec::zeros(65);
        let b = BitVec::ones(65);
        assert_eq!(count_bit_errors(&a, &b), 65);
    }

    #[test]
    #[should_panic(expected = "BitVec lengths must match")]
    fn test_count_bit_errors_length_mismatch() {
        let a = BitVec::zeros(4);
        let b = BitVec::zeros(5);
        count_bit_errors(&a, &b);
    }

    // -----------------------------------------------------------------------
    // BpskAwgnChannel trait impl
    // -----------------------------------------------------------------------

    #[test]
    fn test_bpsk_awgn_channel_impl() {
        let channel = BpskAwgnChannel;
        let bits = BitVec::zeros(10);
        let mut rng = StdRng::seed_from_u64(42);
        let llrs = channel.transmit_and_demodulate(&bits, 10.0, 1.0, &mut rng);
        assert_eq!(llrs.len(), 10);
        // High SNR: all LLRs should be positive (all-zero codeword)
        for llr in &llrs {
            assert!(
                llr.value() > 0.0,
                "At 10dB, all-zero codeword should yield positive LLRs"
            );
        }
    }

    // -----------------------------------------------------------------------
    // CodedSimulationConfig construction
    // -----------------------------------------------------------------------

    #[test]
    fn test_coded_simulation_config_fields() {
        let config = CodedSimulationConfig {
            eb_n0_range_db: vec![0.0, 0.5, 1.0, 1.5, 2.0],
            min_errors: 100,
            max_frames: 100_000,
            max_decoder_iterations: 50,
            rng_seed: Some(42),
            output_path: None,
        };
        assert_eq!(config.eb_n0_range_db.len(), 5);
        assert_eq!(config.min_errors, 100);
        assert_eq!(config.max_decoder_iterations, 50);
    }

    // -----------------------------------------------------------------------
    // Verify JSON is parseable
    // -----------------------------------------------------------------------

    #[test]
    fn test_json_output_parseable() {
        let results = CodedSimulationResults {
            points: vec![
                CodedSimulationResult {
                    eb_n0_db: 0.0,
                    ber: 0.1,
                    bler: 0.5,
                    num_bit_errors: 100,
                    num_bits: 1000,
                    num_block_errors: 50,
                    num_frames: 100,
                    avg_iterations: 5.0,
                    avg_queries_per_bit: 5.0,
                },
                CodedSimulationResult {
                    eb_n0_db: 3.0,
                    ber: 0.01,
                    bler: 0.05,
                    num_bit_errors: 10,
                    num_bits: 1000,
                    num_block_errors: 5,
                    num_frames: 100,
                    avg_iterations: 3.0,
                    avg_queries_per_bit: 3.0,
                },
            ],
        };

        let json = SimulationRunner::coded_results_to_json(&results);

        // Basic structural validation
        assert!(json.starts_with('['));
        assert!(json.trim_end().ends_with(']'));

        // Count opening braces to verify we have 2 objects
        let brace_count = json.matches('{').count();
        assert_eq!(brace_count, 2);

        // All fields present
        for field in &[
            "eb_n0_db",
            "ber",
            "bler",
            "num_bit_errors",
            "num_bits",
            "num_block_errors",
            "num_frames",
            "avg_iterations",
            "avg_queries_per_bit",
        ] {
            assert!(
                json.contains(&format!("\"{}\"", field)),
                "JSON missing field: {}",
                field
            );
        }
    }
}
