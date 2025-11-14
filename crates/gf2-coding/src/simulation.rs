//! Monte Carlo simulation framework for BER/FER performance analysis.
//!
//! This module provides reusable utilities for running communication system
//! simulations over AWGN channels, supporting both bit error rate (BER) and
//! frame error rate (FER) measurements.

use crate::channel::AwgnChannel;
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
}
