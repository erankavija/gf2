//! Monte Carlo simulation framework for BER/FER performance analysis.
//!
//! This module provides reusable utilities for running communication system
//! simulations over AWGN channels, supporting both bit error rate (BER) and
//! frame error rate (FER) measurements.
//!
//! # Coded Simulation
//!
//! Use [`SimulationRunner::run_coded`] for end-to-end coded simulations:
//! encode → modulate → channel → demodulate → decode → count errors.
//! Results include BLER, BER, average decoder iterations, and average queries
//! per information bit.
//!
//! # Parallel Execution
//!
//! With the `parallel` feature enabled, [`SimulationRunner::run_coded_parallel`]
//! dispatches each SNR point to a separate rayon thread pool worker.

use crate::channel::{AwgnChannel, BpskModulator};
use crate::llr::Llr;
use crate::traits::{BlockEncoder, SoftDecoder};
use gf2_core::BitVec;
use rand::Rng;

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

/// Configuration for coded Monte Carlo simulations.
///
/// Provides all parameters needed to run a BER/BLER sweep over a range of
/// Eb/N0 values for a coded communication system.
#[derive(Debug, Clone)]
pub struct CodedSimulationConfig {
    /// Range of Eb/N0 values to simulate (in dB)
    pub eb_n0_range: Vec<f64>,

    /// Minimum number of **block** errors to collect before stopping at an SNR point.
    /// Typical value: 100 (statistical reliability at ~10% relative standard error).
    pub min_block_errors: usize,

    /// Maximum number of frames to transmit per SNR point.
    pub max_frames: usize,

    /// Maximum iterations for iterative decoders (e.g., LDPC belief propagation).
    /// Non-iterative decoders ignore this value.
    pub max_decoder_iterations: usize,
}

impl CodedSimulationConfig {
    /// Creates a quick-test configuration suitable for unit tests.
    ///
    /// Runs very few frames to keep test time short.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::CodedSimulationConfig;
    ///
    /// let config = CodedSimulationConfig::quick_test();
    /// assert!(config.max_frames > 0);
    /// assert!(config.min_block_errors > 0);
    /// ```
    pub fn quick_test() -> Self {
        CodedSimulationConfig {
            eb_n0_range: vec![3.0, 6.0],
            min_block_errors: 10,
            max_frames: 1_000,
            max_decoder_iterations: 50,
        }
    }

    /// Creates a standard simulation configuration.
    ///
    /// Collects at least 100 block errors per SNR point up to 1 million frames.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::CodedSimulationConfig;
    ///
    /// let config = CodedSimulationConfig::standard();
    /// assert_eq!(config.min_block_errors, 100);
    /// ```
    pub fn standard() -> Self {
        CodedSimulationConfig {
            eb_n0_range: (0..=10).map(|i| i as f64 * 0.5).collect(),
            min_block_errors: 100,
            max_frames: 1_000_000,
            max_decoder_iterations: 50,
        }
    }
}

/// Per-SNR-point statistics from a coded simulation.
///
/// Produced by [`SimulationRunner::run_coded`] for each Eb/N0 value.
#[derive(Debug, Clone)]
pub struct CodedSimulationResult {
    /// Eb/N0 in dB
    pub eb_n0_db: f64,

    /// Bit error rate (bit errors / total information bits)
    pub ber: f64,

    /// Block error rate (block errors / total blocks)
    pub bler: f64,

    /// Total information bits transmitted
    pub num_bits: usize,

    /// Total bit errors observed
    pub num_bit_errors: usize,

    /// Total frames transmitted
    pub num_frames: usize,

    /// Total frame (block) errors observed
    pub num_block_errors: usize,

    /// Average number of decoder iterations per frame.
    /// Returns 1.0 for non-iterative decoders.
    pub avg_iterations: f64,

    /// Whether the result reached the minimum block error target.
    /// If `false`, the simulation hit `max_frames` first.
    pub reached_target: bool,
}

impl CodedSimulationResult {
    /// Returns `true` if `num_block_errors >= min_block_errors`.
    ///
    /// # Arguments
    ///
    /// * `min_block_errors` - Minimum block errors threshold
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::CodedSimulationResult;
    ///
    /// let r = CodedSimulationResult {
    ///     eb_n0_db: 4.0,
    ///     ber: 0.001,
    ///     bler: 0.01,
    ///     num_bits: 10000,
    ///     num_bit_errors: 10,
    ///     num_frames: 100,
    ///     num_block_errors: 1,
    ///     avg_iterations: 5.0,
    ///     reached_target: false,
    /// };
    /// assert!(!r.is_complete(100));
    /// assert!(r.is_complete(1));
    /// ```
    pub fn is_complete(&self, min_block_errors: usize) -> bool {
        self.num_block_errors >= min_block_errors
    }

    /// Formats the result as a CSV row.
    ///
    /// Columns: `eb_n0_db,ber,bler,num_bits,num_bit_errors,num_frames,num_block_errors,avg_iterations`
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::CodedSimulationResult;
    ///
    /// let r = CodedSimulationResult {
    ///     eb_n0_db: 4.0,
    ///     ber: 0.001,
    ///     bler: 0.01,
    ///     num_bits: 10000,
    ///     num_bit_errors: 10,
    ///     num_frames: 100,
    ///     num_block_errors: 1,
    ///     avg_iterations: 5.0,
    ///     reached_target: false,
    /// };
    /// let row = r.to_csv_row();
    /// assert!(row.contains("4"));
    /// assert!(row.contains("0.001"));
    /// ```
    pub fn to_csv_row(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{}",
            self.eb_n0_db,
            self.ber,
            self.bler,
            self.num_bits,
            self.num_bit_errors,
            self.num_frames,
            self.num_block_errors,
            self.avg_iterations,
        )
    }

    /// Serialises the result as a JSON object string.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::CodedSimulationResult;
    ///
    /// let r = CodedSimulationResult {
    ///     eb_n0_db: 4.0,
    ///     ber: 0.001,
    ///     bler: 0.01,
    ///     num_bits: 10000,
    ///     num_bit_errors: 10,
    ///     num_frames: 100,
    ///     num_block_errors: 1,
    ///     avg_iterations: 5.0,
    ///     reached_target: false,
    /// };
    /// let json = r.to_json();
    /// assert!(json.contains("\"eb_n0_db\""));
    /// assert!(json.contains("\"ber\""));
    /// ```
    pub fn to_json(&self) -> String {
        format!(
            concat!(
                "{{",
                "\"eb_n0_db\":{},",
                "\"ber\":{},",
                "\"bler\":{},",
                "\"num_bits\":{},",
                "\"num_bit_errors\":{},",
                "\"num_frames\":{},",
                "\"num_block_errors\":{},",
                "\"avg_iterations\":{},",
                "\"reached_target\":{}",
                "}}"
            ),
            self.eb_n0_db,
            self.ber,
            self.bler,
            self.num_bits,
            self.num_bit_errors,
            self.num_frames,
            self.num_block_errors,
            self.avg_iterations,
            self.reached_target,
        )
    }
}

/// Monte Carlo simulation runner for communication systems.
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
        use crate::channel::BpskModulator;

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

    /// Runs a generic coded BER/BLER simulation sweep over AWGN.
    ///
    /// For each Eb/N0 point the loop:
    /// 1. Generates a random message of `encoder.k()` bits.
    /// 2. Encodes it to a codeword of `encoder.n()` bits.
    /// 3. Modulates with BPSK (+1 / -1).
    /// 4. Passes through AWGN at the given Eb/N0.
    /// 5. Converts received symbols to LLRs.
    /// 6. Decodes with the soft decoder.
    /// 7. Counts bit errors (decoded vs original message) and block errors.
    ///
    /// The loop terminates when `min_block_errors` block errors are collected or
    /// `max_frames` frames are transmitted, whichever comes first.
    ///
    /// Progress is reported to stderr every 1 000 frames via `eprintln!`.
    ///
    /// # Arguments
    ///
    /// * `encoder` - A [`BlockEncoder`] that maps k-bit messages to n-bit codewords.
    /// * `decoder` - A [`SoftDecoder`] that maps n LLRs to k decoded message bits.
    /// * `config`  - Simulation parameters (SNR range, stopping criteria, etc.).
    /// * `rng`     - Source of randomness.
    ///
    /// # Panics
    ///
    /// Panics if `encoder.k() != decoder.k()` or `encoder.n() != decoder.n()`.
    ///
    /// # Complexity
    ///
    /// O(max_frames × n) per SNR point.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::{SimulationRunner, CodedSimulationConfig};
    /// use gf2_coding::traits::{BlockEncoder, SoftDecoder, DecoderResult};
    /// use gf2_core::BitVec;
    /// use gf2_coding::llr::Llr;
    ///
    /// // Minimal identity encoder/decoder for doctest: n = k (rate-1, no coding)
    /// struct IdentityCode;
    /// impl BlockEncoder for IdentityCode {
    ///     fn k(&self) -> usize { 4 }
    ///     fn n(&self) -> usize { 4 }
    ///     fn encode(&self, msg: &BitVec) -> BitVec { msg.clone() }
    /// }
    /// impl SoftDecoder for IdentityCode {
    ///     fn k(&self) -> usize { 4 }
    ///     fn n(&self) -> usize { 4 }
    ///     fn decode_soft(&self, llrs: &[Llr]) -> BitVec {
    ///         let mut out = BitVec::new();
    ///         for &l in llrs { out.push_bit(l.hard_decision()); }
    ///         out
    ///     }
    /// }
    ///
    /// let mut config = CodedSimulationConfig::quick_test();
    /// config.eb_n0_range = vec![10.0];
    /// config.min_block_errors = 5;
    /// config.max_frames = 200;
    ///
    /// let encoder = IdentityCode;
    /// let decoder = IdentityCode;
    /// let mut rng = rand::thread_rng();
    /// let results = SimulationRunner::run_coded(&encoder, &decoder, &config, &mut rng);
    /// assert_eq!(results.len(), 1);
    /// ```
    pub fn run_coded<E, D, R>(
        encoder: &E,
        decoder: &D,
        config: &CodedSimulationConfig,
        rng: &mut R,
    ) -> Vec<CodedSimulationResult>
    where
        E: BlockEncoder,
        D: SoftDecoder,
        R: Rng,
    {
        assert_eq!(
            encoder.k(),
            decoder.k(),
            "encoder.k() must equal decoder.k()"
        );
        assert_eq!(
            encoder.n(),
            decoder.n(),
            "encoder.n() must equal decoder.n()"
        );

        let k = encoder.k();
        let n = encoder.n();
        let code_rate = k as f64 / n as f64;

        config
            .eb_n0_range
            .iter()
            .map(|&eb_n0_db| {
                let channel = AwgnChannel::from_eb_n0_db(eb_n0_db, code_rate);

                let mut num_frames: usize = 0;
                let mut num_block_errors: usize = 0;
                let mut num_bits: usize = 0;
                let mut num_bit_errors: usize = 0;
                let mut total_iterations: usize = 0;

                while num_block_errors < config.min_block_errors && num_frames < config.max_frames {
                    // Progress reporting every 1 000 frames
                    if num_frames > 0 && num_frames % 1_000 == 0 {
                        eprintln!(
                            "[sim] Eb/N0={:.2} dB  frames={}  block_errors={}",
                            eb_n0_db, num_frames, num_block_errors
                        );
                    }

                    // 1. Random message
                    let message = BitVec::random(k, rng);

                    // 2. Encode
                    let codeword = encoder.encode(&message);

                    // 3. Modulate (BPSK): false→+1, true→−1
                    let symbols: Vec<f64> = (0..n)
                        .map(|i| BpskModulator::modulate(codeword.get(i)))
                        .collect();

                    // 4. AWGN channel
                    let received = channel.transmit_symbols(&symbols, rng);

                    // 5. LLRs
                    let llrs: Vec<Llr> = channel.to_llrs(&received);

                    // 6. Decode
                    let result = decoder.decode_soft_with_result(&llrs);
                    let decoded = &result.decoded_bits;
                    total_iterations += result.iterations;

                    // 7. Count errors (message bits only)
                    let block_has_error = (0..k).any(|i| decoded.get(i) != message.get(i));
                    let frame_bit_errors =
                        (0..k).filter(|&i| decoded.get(i) != message.get(i)).count();

                    num_frames += 1;
                    num_bits += k;
                    num_bit_errors += frame_bit_errors;
                    if block_has_error {
                        num_block_errors += 1;
                    }
                }

                let ber = if num_bits > 0 {
                    num_bit_errors as f64 / num_bits as f64
                } else {
                    0.0
                };
                let bler = if num_frames > 0 {
                    num_block_errors as f64 / num_frames as f64
                } else {
                    0.0
                };
                let avg_iterations = if num_frames > 0 {
                    total_iterations as f64 / num_frames as f64
                } else {
                    0.0
                };
                let reached_target = num_block_errors >= config.min_block_errors;

                CodedSimulationResult {
                    eb_n0_db,
                    ber,
                    bler,
                    num_bits,
                    num_bit_errors,
                    num_frames,
                    num_block_errors,
                    avg_iterations,
                    reached_target,
                }
            })
            .collect()
    }

    /// Runs a coded simulation sweep in parallel across all SNR points.
    ///
    /// Each SNR point is processed by an independent rayon task with its own
    /// seeded RNG, so results are fully reproducible given the same `base_seed`.
    ///
    /// Requires the `parallel` feature.
    ///
    /// # Arguments
    ///
    /// * `encoder`   - [`BlockEncoder`] shared across threads (must be `Sync`).
    /// * `decoder`   - [`SoftDecoder`] shared across threads (must be `Sync`).
    /// * `config`    - Simulation parameters.
    /// * `base_seed` - Seed used to derive per-SNR-point RNG seeds deterministically.
    ///
    /// # Panics
    ///
    /// Panics if `encoder.k() != decoder.k()` or `encoder.n() != decoder.n()`.
    ///
    /// # Complexity
    ///
    /// O(max_frames × n) total work, parallelised over SNR points.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::{SimulationRunner, CodedSimulationConfig};
    /// use gf2_coding::traits::{BlockEncoder, SoftDecoder, DecoderResult};
    /// use gf2_core::BitVec;
    /// use gf2_coding::llr::Llr;
    ///
    /// struct IdentityCode;
    /// impl BlockEncoder for IdentityCode {
    ///     fn k(&self) -> usize { 4 }
    ///     fn n(&self) -> usize { 4 }
    ///     fn encode(&self, msg: &BitVec) -> BitVec { msg.clone() }
    /// }
    /// impl SoftDecoder for IdentityCode {
    ///     fn k(&self) -> usize { 4 }
    ///     fn n(&self) -> usize { 4 }
    ///     fn decode_soft(&self, llrs: &[Llr]) -> BitVec {
    ///         let mut out = BitVec::new();
    ///         for &l in llrs { out.push_bit(l.hard_decision()); }
    ///         out
    ///     }
    /// }
    ///
    /// let mut config = CodedSimulationConfig::quick_test();
    /// config.eb_n0_range = vec![6.0, 9.0];
    /// config.min_block_errors = 5;
    /// config.max_frames = 200;
    ///
    /// let encoder = IdentityCode;
    /// let decoder = IdentityCode;
    /// let results = SimulationRunner::run_coded_parallel(&encoder, &decoder, &config, 42);
    /// assert_eq!(results.len(), 2);
    /// ```
    #[cfg(feature = "parallel")]
    pub fn run_coded_parallel<E, D>(
        encoder: &E,
        decoder: &D,
        config: &CodedSimulationConfig,
        base_seed: u64,
    ) -> Vec<CodedSimulationResult>
    where
        E: BlockEncoder + Sync,
        D: SoftDecoder + Sync,
    {
        use rand::rngs::StdRng;
        use rand::SeedableRng;
        use rayon::prelude::*;

        config
            .eb_n0_range
            .par_iter()
            .enumerate()
            .map(|(idx, &eb_n0_db)| {
                // Derive a deterministic per-SNR-point seed
                let seed = base_seed.wrapping_add(idx as u64 * 6_364_136_223_846_793_005);
                let mut rng = StdRng::seed_from_u64(seed);

                // Run a single-point config
                let single_config = CodedSimulationConfig {
                    eb_n0_range: vec![eb_n0_db],
                    min_block_errors: config.min_block_errors,
                    max_frames: config.max_frames,
                    max_decoder_iterations: config.max_decoder_iterations,
                };

                let mut results =
                    SimulationRunner::run_coded(encoder, decoder, &single_config, &mut rng);
                results.remove(0)
            })
            .collect()
    }

    /// Exports coded simulation results to CSV format.
    ///
    /// Header: `eb_n0_db,ber,bler,num_bits,num_bit_errors,num_frames,num_block_errors,avg_iterations`
    ///
    /// # Arguments
    ///
    /// * `results`        - Slice of [`CodedSimulationResult`] to export.
    /// * `include_header` - If `true`, prepend the CSV header row.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::{SimulationRunner, CodedSimulationResult};
    ///
    /// let results = vec![CodedSimulationResult {
    ///     eb_n0_db: 4.0, ber: 0.001, bler: 0.01,
    ///     num_bits: 10000, num_bit_errors: 10,
    ///     num_frames: 100, num_block_errors: 1,
    ///     avg_iterations: 5.0, reached_target: false,
    /// }];
    /// let csv = SimulationRunner::coded_results_to_csv(&results, true);
    /// assert!(csv.contains("eb_n0_db"));
    /// assert!(csv.contains("avg_iterations"));
    /// ```
    pub fn coded_results_to_csv(results: &[CodedSimulationResult], include_header: bool) -> String {
        let mut csv = String::new();
        if include_header {
            csv.push_str(
                "eb_n0_db,ber,bler,num_bits,num_bit_errors,num_frames,num_block_errors,avg_iterations\n",
            );
        }
        for r in results {
            csv.push_str(&r.to_csv_row());
            csv.push('\n');
        }
        csv
    }

    /// Exports coded simulation results to a JSON array string.
    ///
    /// # Arguments
    ///
    /// * `results` - Slice of [`CodedSimulationResult`] to serialise.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::simulation::{SimulationRunner, CodedSimulationResult};
    ///
    /// let results = vec![CodedSimulationResult {
    ///     eb_n0_db: 4.0, ber: 0.001, bler: 0.01,
    ///     num_bits: 10000, num_bit_errors: 10,
    ///     num_frames: 100, num_block_errors: 1,
    ///     avg_iterations: 5.0, reached_target: false,
    /// }];
    /// let json = SimulationRunner::coded_results_to_json(&results);
    /// assert!(json.starts_with('['));
    /// assert!(json.ends_with(']'));
    /// assert!(json.contains("\"eb_n0_db\""));
    /// ```
    pub fn coded_results_to_json(results: &[CodedSimulationResult]) -> String {
        let items: Vec<String> = results.iter().map(|r| r.to_json()).collect();
        format!("[{}]", items.join(","))
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
}

#[cfg(test)]
mod tests {
    use super::*;

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

    // -------------------------------------------------------------------------
    // Helpers for coded simulation tests
    // -------------------------------------------------------------------------

    use crate::llr::Llr;
    use crate::traits::{BlockEncoder, SoftDecoder};

    /// Rate-1 pass-through encoder/decoder for unit testing.
    struct IdentityCode {
        k: usize,
    }

    impl BlockEncoder for IdentityCode {
        fn k(&self) -> usize {
            self.k
        }
        fn n(&self) -> usize {
            self.k
        }
        fn encode(&self, msg: &BitVec) -> BitVec {
            msg.clone()
        }
    }

    impl SoftDecoder for IdentityCode {
        fn k(&self) -> usize {
            self.k
        }
        fn n(&self) -> usize {
            self.k
        }
        fn decode_soft(&self, llrs: &[Llr]) -> BitVec {
            let mut out = BitVec::new();
            for &l in llrs {
                out.push_bit(l.hard_decision());
            }
            out
        }
    }

    // -------------------------------------------------------------------------
    // CodedSimulationConfig tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_coded_config_quick_test() {
        let config = CodedSimulationConfig::quick_test();
        assert!(config.min_block_errors > 0);
        assert!(config.max_frames > 0);
        assert!(config.max_decoder_iterations > 0);
        assert!(!config.eb_n0_range.is_empty());
    }

    #[test]
    fn test_coded_config_standard() {
        let config = CodedSimulationConfig::standard();
        assert_eq!(config.min_block_errors, 100);
        assert!(config.max_frames >= 100_000);
    }

    // -------------------------------------------------------------------------
    // CodedSimulationResult unit tests
    // -------------------------------------------------------------------------

    fn make_coded_result() -> CodedSimulationResult {
        CodedSimulationResult {
            eb_n0_db: 4.0,
            ber: 0.001,
            bler: 0.01,
            num_bits: 10_000,
            num_bit_errors: 10,
            num_frames: 100,
            num_block_errors: 1,
            avg_iterations: 5.0,
            reached_target: false,
        }
    }

    #[test]
    fn test_coded_result_is_complete() {
        let r = make_coded_result();
        assert!(r.is_complete(1));
        assert!(!r.is_complete(2));
    }

    #[test]
    fn test_coded_result_to_csv_row() {
        let r = make_coded_result();
        let row = r.to_csv_row();
        assert!(row.contains("4"), "missing eb_n0_db in row: {row}");
        assert!(row.contains("0.001"), "missing ber in row: {row}");
        assert!(row.contains("0.01"), "missing bler in row: {row}");
        assert!(row.contains("5"), "missing avg_iterations in row: {row}");
        // Eight comma-separated fields
        assert_eq!(
            row.split(',').count(),
            8,
            "expected 8 CSV fields, got: {row}"
        );
    }

    #[test]
    fn test_coded_result_to_json() {
        let r = make_coded_result();
        let json = r.to_json();
        assert!(json.contains("\"eb_n0_db\""), "missing key in json: {json}");
        assert!(json.contains("\"ber\""), "missing key in json: {json}");
        assert!(json.contains("\"bler\""), "missing key in json: {json}");
        assert!(
            json.contains("\"avg_iterations\""),
            "missing key in json: {json}"
        );
        assert!(
            json.contains("\"reached_target\""),
            "missing key in json: {json}"
        );
        assert!(json.starts_with('{'), "not a JSON object: {json}");
        assert!(json.ends_with('}'), "not a JSON object: {json}");
    }

    // -------------------------------------------------------------------------
    // run_coded integration tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_run_coded_result_count_matches_snr_range() {
        let mut config = CodedSimulationConfig::quick_test();
        config.eb_n0_range = vec![3.0, 6.0, 9.0];
        config.min_block_errors = 5;
        config.max_frames = 300;

        let code = IdentityCode { k: 8 };
        let decoder = IdentityCode { k: 8 };
        let mut rng = rand::thread_rng();

        let results = SimulationRunner::run_coded(&code, &decoder, &config, &mut rng);
        assert_eq!(results.len(), 3, "one result per SNR point");
    }

    #[test]
    fn test_run_coded_ber_is_non_negative() {
        let mut config = CodedSimulationConfig::quick_test();
        config.eb_n0_range = vec![6.0];
        config.min_block_errors = 5;
        config.max_frames = 300;

        let code = IdentityCode { k: 16 };
        let decoder = IdentityCode { k: 16 };
        let mut rng = rand::thread_rng();

        let results = SimulationRunner::run_coded(&code, &decoder, &config, &mut rng);
        assert!(results[0].ber >= 0.0, "BER must be non-negative");
        assert!(results[0].bler >= 0.0, "BLER must be non-negative");
    }

    #[test]
    fn test_run_coded_ber_bounded_above_by_one() {
        let mut config = CodedSimulationConfig::quick_test();
        // Very low SNR to maximise errors
        config.eb_n0_range = vec![-10.0];
        config.min_block_errors = 5;
        config.max_frames = 500;

        let code = IdentityCode { k: 8 };
        let decoder = IdentityCode { k: 8 };
        let mut rng = rand::thread_rng();

        let results = SimulationRunner::run_coded(&code, &decoder, &config, &mut rng);
        assert!(results[0].ber <= 1.0, "BER must be ≤ 1.0");
        assert!(results[0].bler <= 1.0, "BLER must be ≤ 1.0");
    }

    #[test]
    fn test_run_coded_high_snr_low_ber() {
        // At very high SNR the identity code (no error correction) should still
        // show low BER, matching the uncoded AWGN floor.
        let mut config = CodedSimulationConfig::quick_test();
        config.eb_n0_range = vec![12.0];
        config.min_block_errors = 1;
        config.max_frames = 2_000;

        let code = IdentityCode { k: 64 };
        let decoder = IdentityCode { k: 64 };
        let mut rng = rand::thread_rng();

        let results = SimulationRunner::run_coded(&code, &decoder, &config, &mut rng);
        // At 12 dB uncoded BPSK BER ≈ 1e-4; be generous with 0.01
        assert!(
            results[0].ber < 0.01,
            "expected low BER at 12 dB, got {}",
            results[0].ber
        );
    }

    #[test]
    fn test_run_coded_ber_decreases_with_snr() {
        // BER should be strictly decreasing with Eb/N0 for uncoded BPSK
        let mut config = CodedSimulationConfig::quick_test();
        config.eb_n0_range = vec![0.0, 6.0];
        config.min_block_errors = 10;
        config.max_frames = 2_000;

        let code_lo = IdentityCode { k: 32 };
        let dec_lo = IdentityCode { k: 32 };
        let code_hi = IdentityCode { k: 32 };
        let dec_hi = IdentityCode { k: 32 };
        let mut rng = rand::thread_rng();

        // Run separately to compare
        let cfg_lo = CodedSimulationConfig {
            eb_n0_range: vec![0.0],
            ..config.clone()
        };
        let cfg_hi = CodedSimulationConfig {
            eb_n0_range: vec![6.0],
            ..config
        };

        let res_lo = SimulationRunner::run_coded(&code_lo, &dec_lo, &cfg_lo, &mut rng);
        let res_hi = SimulationRunner::run_coded(&code_hi, &dec_hi, &cfg_hi, &mut rng);

        assert!(
            res_hi[0].ber < res_lo[0].ber,
            "BER at 6 dB ({}) should be lower than at 0 dB ({})",
            res_hi[0].ber,
            res_lo[0].ber
        );
    }

    #[test]
    fn test_run_coded_frame_and_bit_counts_consistent() {
        let mut config = CodedSimulationConfig::quick_test();
        config.eb_n0_range = vec![5.0];
        config.min_block_errors = 5;
        config.max_frames = 500;

        let k = 16usize;
        let code = IdentityCode { k };
        let decoder = IdentityCode { k };
        let mut rng = rand::thread_rng();

        let results = SimulationRunner::run_coded(&code, &decoder, &config, &mut rng);
        let r = &results[0];

        // num_bits must equal num_frames * k
        assert_eq!(
            r.num_bits,
            r.num_frames * k,
            "num_bits = num_frames * k invariant violated"
        );
        // num_bit_errors ≤ num_bits
        assert!(
            r.num_bit_errors <= r.num_bits,
            "bit errors exceed total bits"
        );
        // num_block_errors ≤ num_frames
        assert!(
            r.num_block_errors <= r.num_frames,
            "block errors exceed total frames"
        );
        // avg_iterations must be positive
        assert!(r.avg_iterations > 0.0, "avg_iterations must be > 0");
    }

    #[test]
    fn test_run_coded_early_termination_at_min_errors() {
        let mut config = CodedSimulationConfig::quick_test();
        // Very low SNR: many errors. Set a tiny min_block_errors.
        config.eb_n0_range = vec![-5.0];
        config.min_block_errors = 3;
        config.max_frames = 100_000;

        let code = IdentityCode { k: 8 };
        let decoder = IdentityCode { k: 8 };
        let mut rng = rand::thread_rng();

        let results = SimulationRunner::run_coded(&code, &decoder, &config, &mut rng);
        let r = &results[0];

        // Should have reached the block error target well before max_frames
        assert!(
            r.reached_target,
            "should have reached min_block_errors=3 at -5 dB"
        );
        assert!(
            r.num_frames < 100_000,
            "should terminate early: frames={}, max=100000",
            r.num_frames
        );
    }

    #[test]
    fn test_run_coded_max_frames_termination() {
        let mut config = CodedSimulationConfig::quick_test();
        // Very high SNR: almost no errors. min_block_errors large → max_frames triggers.
        config.eb_n0_range = vec![30.0];
        config.min_block_errors = 10_000;
        config.max_frames = 50;

        let code = IdentityCode { k: 8 };
        let decoder = IdentityCode { k: 8 };
        let mut rng = rand::thread_rng();

        let results = SimulationRunner::run_coded(&code, &decoder, &config, &mut rng);
        let r = &results[0];

        assert_eq!(
            r.num_frames, 50,
            "should have run exactly max_frames=50 frames"
        );
        assert!(
            !r.reached_target,
            "should not have reached block error target"
        );
    }

    // -------------------------------------------------------------------------
    // CSV / JSON export tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_coded_results_to_csv_header() {
        let results = vec![make_coded_result()];
        let csv = SimulationRunner::coded_results_to_csv(&results, true);
        assert!(csv.starts_with("eb_n0_db,"), "header missing: {csv}");
        assert!(
            csv.contains("avg_iterations"),
            "header missing avg_iterations"
        );
    }

    #[test]
    fn test_coded_results_to_csv_no_header() {
        let results = vec![make_coded_result()];
        let csv = SimulationRunner::coded_results_to_csv(&results, false);
        assert!(!csv.starts_with("eb_n0_db"), "should not have header");
    }

    #[test]
    fn test_coded_results_to_csv_multiple_rows() {
        let results = vec![make_coded_result(), make_coded_result()];
        let csv = SimulationRunner::coded_results_to_csv(&results, true);
        let lines: Vec<&str> = csv.trim().lines().collect();
        assert_eq!(lines.len(), 3, "header + 2 data rows");
    }

    #[test]
    fn test_coded_results_to_json_array() {
        let results = vec![make_coded_result(), make_coded_result()];
        let json = SimulationRunner::coded_results_to_json(&results);
        assert!(json.starts_with('['), "not a JSON array: {json}");
        assert!(json.ends_with(']'), "not a JSON array: {json}");
        // Two objects → one comma between them
        assert_eq!(json.matches("\"eb_n0_db\"").count(), 2);
    }

    #[test]
    fn test_coded_results_to_json_empty() {
        let json = SimulationRunner::coded_results_to_json(&[]);
        assert_eq!(json, "[]");
    }

    #[test]
    fn test_coded_results_to_csv_empty() {
        let csv = SimulationRunner::coded_results_to_csv(&[], true);
        // Just header
        assert!(csv.contains("eb_n0_db"));
        let lines: Vec<&str> = csv.trim().lines().collect();
        assert_eq!(lines.len(), 1, "only header line for empty results");
    }

    // -------------------------------------------------------------------------
    // Parallel tests (only compiled when feature = "parallel")
    // -------------------------------------------------------------------------

    #[cfg(feature = "parallel")]
    mod parallel_tests {
        use super::*;

        #[test]
        fn test_run_coded_parallel_result_count() {
            let mut config = CodedSimulationConfig::quick_test();
            config.eb_n0_range = vec![3.0, 6.0, 9.0];
            config.min_block_errors = 5;
            config.max_frames = 300;

            let code = IdentityCode { k: 8 };
            let decoder = IdentityCode { k: 8 };
            let results = SimulationRunner::run_coded_parallel(&code, &decoder, &config, 42);
            assert_eq!(results.len(), 3, "one result per SNR point");
        }

        #[test]
        fn test_run_coded_parallel_ber_non_negative() {
            let mut config = CodedSimulationConfig::quick_test();
            config.eb_n0_range = vec![6.0];
            config.min_block_errors = 3;
            config.max_frames = 200;

            let code = IdentityCode { k: 8 };
            let decoder = IdentityCode { k: 8 };
            let results = SimulationRunner::run_coded_parallel(&code, &decoder, &config, 0);
            assert!(results[0].ber >= 0.0);
            assert!(results[0].bler >= 0.0);
        }

        #[test]
        fn test_run_coded_parallel_deterministic() {
            let mut config = CodedSimulationConfig::quick_test();
            config.eb_n0_range = vec![5.0];
            config.min_block_errors = 10;
            config.max_frames = 500;

            let code1 = IdentityCode { k: 16 };
            let dec1 = IdentityCode { k: 16 };
            let code2 = IdentityCode { k: 16 };
            let dec2 = IdentityCode { k: 16 };

            let r1 = SimulationRunner::run_coded_parallel(&code1, &dec1, &config, 12345);
            let r2 = SimulationRunner::run_coded_parallel(&code2, &dec2, &config, 12345);

            assert_eq!(
                r1[0].num_frames, r2[0].num_frames,
                "parallel runs with same seed must produce identical frame counts"
            );
            assert_eq!(r1[0].num_bit_errors, r2[0].num_bit_errors);
        }
    }
}
