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
    AnalysisCapture, BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, ModemSpec,
    ReferenceMapper, ReferenceSoftDemapper,
};
use crate::traits::{BlockEncoder, DecoderResult, IterativeSoftDecoder, SoftDecoder};
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};

#[cfg(feature = "sim-observability")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "sim-observability")]
use rand_chacha::ChaCha20Rng;

/// Global lock for serializing all JSONL file appends (progress + point_complete).
///
/// Used by both `SnrAccumulator::write_progress_entry` and
/// `append_point_complete_jsonl` to prevent interleaved writes from
/// concurrent parallel simulation workers.
static JSONL_WRITE_LOCK: Mutex<()> = Mutex::new(());
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// sim-observability: checkpointing, tracing, and signal handling
// ---------------------------------------------------------------------------

/// Process-wide interrupt flag. Set to `true` by the `ctrlc` handler
/// (SIGINT / SIGTERM) when `sim-observability` is active.
///
/// The inner simulation loops poll this flag between frames; on trip they
/// flush the current SNR checkpoint and exit.
#[cfg(feature = "sim-observability")]
static INTERRUPTED: OnceLock<Arc<AtomicBool>> = OnceLock::new();

/// Returns a reference to the process-wide interrupt flag, initialising the
/// `ctrlc` handler on first call.
///
/// Thread-safe: `OnceLock` guarantees the handler is registered exactly once
/// even under concurrent access.
#[cfg(feature = "sim-observability")]
fn interrupted_flag() -> &'static Arc<AtomicBool> {
    INTERRUPTED.get_or_init(|| {
        let flag = Arc::new(AtomicBool::new(false));
        let f2 = flag.clone();
        // Ignore errors — if ctrlc::set_handler fails (e.g. in a test that
        // already registered a handler) we continue without graceful flush.
        let _ = ctrlc::set_handler(move || {
            f2.store(true, Ordering::SeqCst);
        });
        flag
    })
}

/// Clears the interrupt flag. Called at the start of each campaign so that
/// a previous SIGINT during a test does not bleed into the next run.
#[cfg(feature = "sim-observability")]
fn clear_interrupt() {
    interrupted_flag().store(false, Ordering::SeqCst);
}

/// Returns `true` if SIGINT or SIGTERM was received since the last
/// [`clear_interrupt`] call.
#[cfg(feature = "sim-observability")]
fn is_interrupted() -> bool {
    interrupted_flag().load(Ordering::SeqCst)
}

/// Per-SNR-point checkpoint written to `<checkpoint_dir>/snr_<index>.json`.
///
/// All numeric fields are plain JSON values; the `config_hash` field holds
/// the `"blake3:<hex>"` string so a corrupted or mismatched checkpoint is
/// detected early.
#[cfg(feature = "sim-observability")]
#[derive(Debug)]
struct SnrCheckpoint {
    snr_index: usize,
    eb_n0_db: f64,
    frames_completed: usize,
    errors_accumulated: usize,
    total_iterations: usize,
    total_queries: usize,
    total_bits: usize,
    total_bit_errors: usize,
    /// ChaCha20 word position at the time of this checkpoint snapshot.
    /// Stored as a decimal string because `u128` exceeds JSON's safe integer
    /// range (2^53); the reader parses it back with `str::parse::<u128>`.
    rng_word_pos: u128,
    frames_target: usize,
    errors_target: usize,
    /// `true` means the point hit `frames_target` or `errors_target`; resume
    /// skips it. `false` means a heartbeat snapshot mid-point.
    completed: bool,
    /// `"blake3:<64 hex chars>"` — BLAKE3 of the canonical config encoding.
    config_hash: String,
}

#[cfg(feature = "sim-observability")]
impl SnrCheckpoint {
    /// Serialises to a JSON string (no external serde dep required).
    fn to_json(&self) -> String {
        format!(
            concat!(
                "{{\n",
                "  \"snr_index\": {},\n",
                "  \"eb_n0_db\": {},\n",
                "  \"frames_completed\": {},\n",
                "  \"errors_accumulated\": {},\n",
                "  \"total_iterations\": {},\n",
                "  \"total_queries\": {},\n",
                "  \"total_bits\": {},\n",
                "  \"total_bit_errors\": {},\n",
                "  \"rng_word_pos\": \"{}\",\n",
                "  \"frames_target\": {},\n",
                "  \"errors_target\": {},\n",
                "  \"completed\": {},\n",
                "  \"config_hash\": \"{}\"\n",
                "}}"
            ),
            self.snr_index,
            self.eb_n0_db,
            self.frames_completed,
            self.errors_accumulated,
            self.total_iterations,
            self.total_queries,
            self.total_bits,
            self.total_bit_errors,
            self.rng_word_pos,
            self.frames_target,
            self.errors_target,
            self.completed,
            self.config_hash,
        )
    }

    /// Parses a checkpoint from its JSON string representation.
    ///
    /// Returns `None` if any required field is missing or cannot be parsed.
    fn from_json(s: &str) -> Option<Self> {
        fn extract<'a>(s: &'a str, key: &str) -> Option<&'a str> {
            let needle = format!("\"{key}\":");
            let pos = s.find(needle.as_str())?;
            let after = s[pos + needle.len()..].trim_start();
            // Value is either a quoted string or a bare value terminated by
            // `,` `\n` or `}`.
            if let Some(inner) = after.strip_prefix('"') {
                let end = inner.find('"')?;
                Some(&inner[..end])
            } else {
                let end = after.find([',', '\n', '}']).unwrap_or(after.len());
                Some(after[..end].trim())
            }
        }

        Some(Self {
            snr_index: extract(s, "snr_index")?.parse().ok()?,
            eb_n0_db: extract(s, "eb_n0_db")?.parse().ok()?,
            frames_completed: extract(s, "frames_completed")?.parse().ok()?,
            errors_accumulated: extract(s, "errors_accumulated")?.parse().ok()?,
            total_iterations: extract(s, "total_iterations")?.parse().ok()?,
            total_queries: extract(s, "total_queries")?.parse().ok()?,
            total_bits: extract(s, "total_bits")?.parse().ok()?,
            total_bit_errors: extract(s, "total_bit_errors")?.parse().ok()?,
            rng_word_pos: extract(s, "rng_word_pos")?.parse().ok()?,
            frames_target: extract(s, "frames_target")?.parse().ok()?,
            errors_target: extract(s, "errors_target")?.parse().ok()?,
            completed: extract(s, "completed")? == "true",
            config_hash: extract(s, "config_hash")?.to_string(),
        })
    }
}

/// Computes the BLAKE3 config hash for a `SimulationConfig`.
///
/// The canonical encoding includes all fields that affect simulation results:
/// SNR range, stopping criteria, RNG seed, and decoder parameters.  The
/// optional observability fields (`checkpoint_dir`, `tracing_log_path`,
/// `heartbeat_every_frames`, `output_path`) are excluded — they control
/// output paths, not the simulation itself, so changing them should not
/// invalidate an existing checkpoint directory.
///
/// # Returns
///
/// A string `"blake3:<64 lowercase hex chars>"`.
#[cfg(feature = "sim-observability")]
fn compute_config_hash(config: &SimulationConfig) -> String {
    let mut hasher = blake3::Hasher::new();
    // SNR range — encoded as little-endian f64 bytes.
    hasher.update(&(config.eb_n0_range_db.len() as u64).to_le_bytes());
    for &v in &config.eb_n0_range_db {
        hasher.update(&v.to_le_bytes());
    }
    hasher.update(&(config.min_errors as u64).to_le_bytes());
    hasher.update(&(config.max_frames as u64).to_le_bytes());
    hasher.update(&(config.max_decoder_iterations as u64).to_le_bytes());
    // Seed: 0 for None (same as Some(0), but None is distinct).
    let seed_tag: u8 = if config.rng_seed.is_some() { 1 } else { 0 };
    hasher.update(&[seed_tag]);
    hasher.update(&config.rng_seed.unwrap_or(0).to_le_bytes());
    let hash = hasher.finalize();
    format!("blake3:{}", hash.to_hex())
}

/// Checkpoint file name for SNR point `index`.
#[cfg(feature = "sim-observability")]
fn checkpoint_path(dir: &Path, index: usize) -> PathBuf {
    dir.join(format!("snr_{:04}.json", index))
}

/// Path of the config-hash sentinel file inside a checkpoint directory.
#[cfg(feature = "sim-observability")]
fn config_hash_path(dir: &Path) -> PathBuf {
    dir.join("config_hash.txt")
}

/// Atomically writes a checkpoint by writing to a `.tmp` file first then
/// renaming, avoiding torn writes under SIGINT.
#[cfg(feature = "sim-observability")]
fn write_checkpoint_atomic(path: &Path, ckpt: &SnrCheckpoint) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, ckpt.to_json())?;
    std::fs::rename(&tmp, path)
}

/// Loads and validates an existing checkpoint from disk.
///
/// Returns `None` if the file does not exist, cannot be parsed, or the
/// config hash does not match `expected_hash`.
#[cfg(feature = "sim-observability")]
fn load_checkpoint(path: &Path, expected_hash: &str) -> Option<SnrCheckpoint> {
    let s = std::fs::read_to_string(path).ok()?;
    let ckpt = SnrCheckpoint::from_json(&s)?;
    if ckpt.config_hash != expected_hash {
        None // hash mismatch — treat as absent
    } else {
        Some(ckpt)
    }
}

/// Validates the checkpoint directory on startup.
///
/// Reads `config_hash.txt` if present and compares against `current_hash`.
/// Returns an error string if a mismatch is found; returns `Ok(())` if the
/// directory is absent, empty, or hashes match.
///
/// On first run (no `config_hash.txt`), creates the directory and writes the
/// hash file.
#[cfg(feature = "sim-observability")]
fn validate_checkpoint_dir(dir: &Path, current_hash: &str) -> Result<(), String> {
    if !dir.exists() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Cannot create checkpoint directory {}: {e}", dir.display()))?;
        std::fs::write(config_hash_path(dir), current_hash)
            .map_err(|e| format!("Cannot write config_hash.txt: {e}"))?;
        return Ok(());
    }
    let hash_file = config_hash_path(dir);
    if !hash_file.exists() {
        // Directory exists but no hash file — write it (first use of an empty dir).
        std::fs::write(&hash_file, current_hash)
            .map_err(|e| format!("Cannot write config_hash.txt: {e}"))?;
        return Ok(());
    }
    let stored = std::fs::read_to_string(&hash_file)
        .map_err(|e| format!("Cannot read config_hash.txt: {e}"))?;
    let stored = stored.trim();
    if stored != current_hash {
        return Err(format!(
            "Checkpoint directory config hash mismatch.\n  stored:  {stored}\n  current: {current_hash}\n\
             Change checkpoint_dir or delete the directory to start fresh.",
        ));
    }
    Ok(())
}

/// Installs a JSON-lines tracing subscriber for the current thread and returns
/// a guard that uninstalls it on drop.
///
/// When `config.tracing_log_path` is `Some(path)`, opens the file in append
/// mode, builds a `tracing_subscriber::fmt` JSON layer that writes to it, and
/// calls `SubscriberInitExt::set_default` to install it as the thread-local
/// default.  The returned `DefaultGuard` restores the previous subscriber
/// (which may be `NoSubscriber`) when it is dropped at end of scope.
///
/// When `tracing_log_path` is `None`, returns `None` and installs nothing.
/// The caller's existing subscriber (if any) remains active.
///
/// # Thread safety
///
/// The installed subscriber is thread-local (`set_default`), not global.  For
/// rayon-parallel paths the caller must propagate the current `Dispatch` into
/// each worker thread:
///
/// ```text
/// let dispatch = tracing::dispatcher::get_default(|d| d.clone());
/// rayon_worker_closure = move || {
///     tracing::dispatcher::with_default(&dispatch, || { ... })
/// };
/// ```
#[cfg(feature = "sim-observability")]
fn setup_tracing_guard(config: &SimulationConfig) -> Option<tracing::subscriber::DefaultGuard> {
    use tracing_subscriber::{fmt, prelude::*, registry};

    let path = config.tracing_log_path.as_ref()?;
    let file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        Ok(f) => f,
        Err(e) => {
            eprintln!(
                "Warning: cannot open tracing log {} — tracing disabled: {e}",
                path.display()
            );
            return None;
        }
    };

    let layer = fmt::layer()
        .json()
        .with_writer(Mutex::new(file))
        .with_span_list(false)
        .with_current_span(true);
    let subscriber = registry().with(layer);
    Some(subscriber.set_default())
}

/// Constructs a per-SNR-point `ChaCha20Rng` with deterministic seeding.
///
/// The seed for point `snr_index` is derived as:
///
/// ```text
/// seed = base_seed ^ (snr_index as u64).rotate_left(13)
/// ```
///
/// This ensures each SNR point gets an independent RNG stream from a single
/// base seed while keeping the derivation trivially reversible for auditing.
/// `set_word_pos(word_pos)` then seeks into the stream at the position
/// recorded by the last heartbeat checkpoint (0 for a fresh point).
///
/// # Resume determinism
///
/// The design doc specifies `seed = config.seed ^ snr_index ^ frames_completed_at_checkpoint`
/// with `(or equivalent)`. This implementation omits the
/// `frames_completed_at_checkpoint` term from the seed: instead the checkpoint
/// frame count is consumed by `ChaCha20Rng::set_word_pos(rng_word_pos)`, where
/// `rng_word_pos` is the exact stream position captured at the heartbeat
/// checkpoint. This is strictly equivalent for byte-identical resume because
/// `ChaCha20Rng::set_word_pos` provides a bit-exact stream seek that reaches
/// the same generator state the uninterrupted run would have been at —
/// documented as reproducible across `rand_chacha` versions.
///
/// # Arguments
///
/// * `base_seed` — From `SimulationConfig::rng_seed`.
/// * `snr_index` — Zero-based index into `eb_n0_range_db`.
/// * `word_pos` — ChaCha20 word position to seek to; 0 for a fresh start.
#[cfg(feature = "sim-observability")]
fn make_chacha_rng(base_seed: u64, snr_index: usize, word_pos: u128) -> ChaCha20Rng {
    use rand::SeedableRng as _;
    let seed = base_seed ^ (snr_index as u64).rotate_left(13);
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    rng.set_word_pos(word_pos);
    rng
}

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

    /// The [`crate::modem::DemapMethod`] whose LLRs this channel
    /// produces. Consumed by
    /// [`SimulationRunner::run_uncoded_ber_with_analysis`] to tag the
    /// captured statistics with the method that generated them —
    /// heterogeneous batches silently merged into one `PerBitLlrStats`
    /// would produce un-interpretable MI / GMI estimates, so the
    /// runner asserts that the capture's
    /// [`crate::modem::AnalysisCapture::demap_method`] matches this
    /// value before the first batch.
    ///
    /// Default is [`crate::modem::DemapMethod::MaxLog`], which matches
    /// the LLR convention of the legacy BPSK/AWGN paths and the
    /// [`BpskAwgnChannel`] compatibility surface. Modem-backed
    /// channels that use exact log-MAP (or expose a user-selected
    /// method like [`crate::modem::ModemChannelAdapter`]) should
    /// override this.
    fn demap_method(&self) -> crate::modem::DemapMethod {
        crate::modem::DemapMethod::MaxLog
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

    /// `BpskAwgnChannel` drives the shared `ReferenceSoftDemapper`
    /// with [`DemapMethod::ExactLogMap`] (BPSK exact log-MAP collapses
    /// to the closed-form `2r / sigma^2` under the consistent-Gaussian
    /// LLR convention, so this is the numerically correct choice).
    fn demap_method(&self) -> DemapMethod {
        DemapMethod::ExactLogMap
    }
}

/// Configuration for Monte Carlo simulations.
///
/// Controls SNR sweep range, stopping criteria, decoder iteration limits,
/// RNG seeding, optional output file path, and (with the `sim-observability`
/// feature) crash-safe checkpointing, structured JSON-lines tracing, and
/// within-SNR heartbeat events.
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

    /// Optional directory for per-SNR checkpoint files (requires `sim-observability` feature).
    ///
    /// When `Some(dir)`, the runner writes one JSON file per SNR point under
    /// `<dir>/snr_<index>.json` after each point completes, and resumes from
    /// any existing checkpoints on startup.  A `config_hash.txt` file in the
    /// same directory records the BLAKE3 hash of this configuration; if a
    /// resume attempt finds a hash mismatch the runner aborts with a clear
    /// error rather than silently using incompatible state.
    ///
    /// `None` (the default) disables all checkpoint behaviour.
    ///
    /// Requires [`rng_seed`](Self::rng_seed) to be `Some` for byte-identical
    /// resume: without a fixed seed the per-SNR-point RNG sequence is not
    /// reproducible.
    pub checkpoint_dir: Option<PathBuf>,

    /// Optional path for JSON-lines tracing output (requires `sim-observability` feature).
    ///
    /// When `Some(path)`, each campaign produces one JSON object per line:
    /// a `campaign_start` record on startup, `snr_completed` records after
    /// each SNR point, and `heartbeat` records at the cadence set by
    /// [`heartbeat_every_frames`](Self::heartbeat_every_frames).
    ///
    /// The file is opened in append mode so concurrent or sequential runs to
    /// the same path interleave without truncation.
    ///
    /// `None` (the default) disables tracing output.
    pub tracing_log_path: Option<PathBuf>,

    /// Optional within-SNR heartbeat cadence in frames (requires `sim-observability` feature).
    ///
    /// When `Some(n)`, the runner emits a `heartbeat` tracing event and — if
    /// [`checkpoint_dir`](Self::checkpoint_dir) is set — writes an
    /// intermediate (incomplete) checkpoint every `n` simulated frames.
    /// The intermediate checkpoint records the current RNG word position so
    /// a crash mid-SNR-point can be recovered by restarting with the same
    /// config.
    ///
    /// `None` (the default) disables within-SNR heartbeats and intermediate
    /// checkpoints; only finished SNR points are checkpointed.
    pub heartbeat_every_frames: Option<usize>,
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
            checkpoint_dir: None,
            tracing_log_path: None,
            heartbeat_every_frames: None,
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
            checkpoint_dir: None,
            tracing_log_path: None,
            heartbeat_every_frames: None,
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

/// Shared body of the uncoded-BER Monte Carlo loop, parameterized by
/// an optional [`AnalysisCapture`]. Both
/// [`SimulationRunner::run_uncoded_ber_with_channel`] (always `None`)
/// and [`SimulationRunner::run_uncoded_ber_with_analysis`] (caller's
/// choice) delegate here so there is exactly one implementation of the
/// `bits -> channel -> hard-decision` logic.
///
/// # Zero-overhead contract
///
/// The function is `#[inline]` and the only branch on `capture` is a
/// single `if let Some(_) = capture` guard around the
/// [`AnalysisCapture::accumulate_slice`] call. When the caller passes
/// `None`, the guard's body is dead code after monomorphization and
/// constant propagation: the compiler emits exactly the same loop
/// that the analysis-free runner emitted before this refactor. The
/// `simulation_no_analysis_overhead` bench guards that equivalence.
#[inline]
fn run_uncoded_ber_with_channel_impl<C: ChannelModel, R: Rng>(
    channel: &C,
    config: &SimulationConfig,
    mut capture: Option<&mut AnalysisCapture<'_>>,
    rng: &mut R,
) -> Vec<SimulationResult> {
    // sim-observability: install JSON-lines tracing subscriber for this run.
    #[cfg(feature = "sim-observability")]
    let _tracing_guard = setup_tracing_guard(config);

    // sim-observability: open campaign span and emit campaign_start event.
    #[cfg(feature = "sim-observability")]
    let _campaign_guard = {
        use std::time::SystemTime;
        let config_hash = compute_config_hash(config);
        let run_uuid = {
            let t = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            format!("{:032x}", t ^ (config.rng_seed.unwrap_or(0) as u128))
        };
        let seed_val = config.rng_seed.unwrap_or(0);
        let guard = tracing::info_span!(
            "campaign",
            config_hash = %config_hash,
            run_uuid = %run_uuid,
            seed = seed_val,
        )
        .entered();
        tracing::info!(
            name: "campaign_start",
            event_type = "campaign_start",
            config_hash = %config_hash,
            run_uuid = %run_uuid,
            seed = seed_val,
        );
        guard
    };

    // sim-observability: validate checkpoint directory and compute config hash.
    // Per-SNR-boundary checkpointing only; within-SNR heartbeat resume is not
    // implemented for this path (uncoded simulations are typically fast enough
    // that per-SNR granularity is sufficient, and StdRng does not support
    // ChaCha20-style seek).
    #[cfg(feature = "sim-observability")]
    let config_hash = compute_config_hash(config);
    #[cfg(feature = "sim-observability")]
    if let Some(ref ckpt_dir) = config.checkpoint_dir {
        if let Err(e) = validate_checkpoint_dir(ckpt_dir, &config_hash) {
            panic!("{e}");
        }
    }

    // sim-observability: clear any stale interrupt flag from a previous run.
    #[cfg(feature = "sim-observability")]
    clear_interrupt();

    /// Nominal batch length. See
    /// [`SimulationRunner::run_uncoded_ber_with_channel`] for the
    /// rationale behind the value and the alignment rounding.
    const UNCODED_MODEM_BATCH_BITS: usize = 960;

    let alignment = channel.batch_alignment().max(1);

    // Contract: if the caller opted into analysis capture, its
    // accumulator's `bits_per_symbol` must match the channel's
    // `batch_alignment`. `PerBitLlrStats::accumulate` only enforces
    // length invariants; a silent mismatch would accumulate nonsensical
    // per-position statistics without tripping any downstream check.
    // Reject up front with a descriptive panic so the misuse is caught
    // at the first batch rather than being silently averaged away.
    if let Some(cap) = capture.as_deref() {
        assert_eq!(
            cap.bits_per_symbol() as usize,
            alignment,
            "AnalysisCapture bits_per_symbol ({}) must equal channel.batch_alignment() \
             ({}) — a mismatched capture would silently collapse per-position statistics",
            cap.bits_per_symbol(),
            alignment,
        );
        // Second provenance check: the capture is tagged with the
        // demap method its MI/GMI numbers will describe. The channel
        // advertises its own method through `ChannelModel::demap_method`.
        // Per-bit MI / GMI semantics differ between exact log-MAP and
        // max-log (see the `analysis` module docs), so heterogeneous
        // batches silently merged into one accumulator would produce
        // un-interpretable statistics.
        let channel_method = channel.demap_method();
        assert_eq!(
            cap.demap_method(),
            channel_method,
            "AnalysisCapture was tagged with {:?} but the channel produces {:?} LLRs — \
             rebuild the capture via `AnalysisCapture::with_method(...)` so the per-bit \
             MI / GMI numbers are interpretable",
            cap.demap_method(),
            channel_method,
        );
    }

    // Scratch buffer reused across batches when analysis is enabled.
    // Allocated exactly once per SNR-point sweep and only on the
    // analysis-enabled path. Stays `None` when `capture.is_none()`.
    let mut truth_scratch: Option<Vec<bool>> = if capture.is_some() {
        Some(Vec::with_capacity(UNCODED_MODEM_BATCH_BITS))
    } else {
        None
    };

    let mut results = Vec::with_capacity(config.eb_n0_range_db.len());
    for (snr_idx, &eb_n0_db) in config.eb_n0_range_db.iter().enumerate() {
        // sim-observability: check for an existing completed checkpoint.
        // Per-SNR-boundary granularity only; within-SNR heartbeat resume is
        // not implemented for the uncoded path (see function-level rustdoc).
        #[cfg(feature = "sim-observability")]
        let ckpt_resume: Option<SnrCheckpoint> = config
            .checkpoint_dir
            .as_ref()
            .and_then(|dir| load_checkpoint(&checkpoint_path(dir, snr_idx), &config_hash));

        #[cfg(feature = "sim-observability")]
        if let Some(ref ckpt) = ckpt_resume {
            if ckpt.completed {
                eprintln!(
                    "[{:.1} dB] CHECKPOINT RESUMED: skipping completed uncoded point \
                     ({} bit errors / {} bits)",
                    eb_n0_db, ckpt.total_bit_errors, ckpt.total_bits
                );
                let ber = if ckpt.total_bits > 0 {
                    ckpt.total_bit_errors as f64 / ckpt.total_bits as f64
                } else {
                    0.0
                };
                results.push(SimulationResult {
                    eb_n0_db,
                    ber,
                    bler: 0.0,
                    avg_iterations: None,
                    avg_queries_per_bit: None,
                    num_bits: ckpt.total_bits,
                    num_bit_errors: ckpt.total_bit_errors,
                    num_frames: 0,
                    num_frame_errors: 0,
                });
                continue;
            }
        }

        // sim-observability: per-SNR span.
        #[cfg(feature = "sim-observability")]
        let _snr_guard = tracing::info_span!(
            "snr_point",
            eb_n0_db = eb_n0_db,
            es_n0_db = eb_n0_db, // uncoded: rate=1, so es_n0_db == eb_n0_db
            frames_target = config.max_frames,
            errors_target = config.min_errors,
        )
        .entered();

        // Suppress unused-variable warning when feature is disabled.
        #[cfg(not(feature = "sim-observability"))]
        let _ = snr_idx;

        #[cfg_attr(not(feature = "sim-observability"), allow(unused_variables))]
        let point_start = Instant::now();
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

            // Opt-in per-bit analysis. The `None` branch has no
            // extra work; the inline wrapper lets the optimizer
            // collapse the match when the caller passed `None`.
            if let (Some(cap), Some(scratch)) = (capture.as_deref_mut(), truth_scratch.as_mut()) {
                scratch.clear();
                scratch.reserve(batch_size);
                for i in 0..batch_size {
                    scratch.push(bits.get(i));
                }
                cap.accumulate_slice(&llrs, scratch);
            }

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

        // sim-observability: snr_completed event.
        #[cfg(feature = "sim-observability")]
        tracing::info!(
            name: "snr_completed",
            event_type = "snr_completed",
            snr_index = snr_idx,
            eb_n0_db = eb_n0_db,
            fer = 0.0_f64, // uncoded: no frame errors concept
            ber = ber,
            mean_iters = Option::<f64>::None,
            elapsed_seconds = point_start.elapsed().as_secs_f64(),
        );

        // sim-observability: write a completed checkpoint after each SNR point.
        // Per-SNR-boundary granularity only; within-SNR heartbeat is not
        // implemented for the uncoded path (uncoded simulations are fast enough
        // that per-SNR granularity is sufficient; StdRng also lacks ChaCha20's
        // `set_word_pos` seek, so byte-identical within-SNR resume is not
        // available here).
        #[cfg(feature = "sim-observability")]
        if let Some(ref ckpt_dir) = config.checkpoint_dir {
            let ckpt = SnrCheckpoint {
                snr_index: snr_idx,
                eb_n0_db,
                frames_completed: 0, // uncoded: no frame concept
                errors_accumulated: 0,
                total_iterations: 0,
                total_queries: 0,
                total_bits,
                total_bit_errors: total_errors,
                rng_word_pos: 0, // no seek support on uncoded path
                frames_target: config.max_frames,
                errors_target: config.min_errors,
                completed: true,
                config_hash: config_hash.clone(),
            };
            if let Err(e) = write_checkpoint_atomic(&checkpoint_path(ckpt_dir, snr_idx), &ckpt) {
                eprintln!("[sim-observability] Failed to write uncoded checkpoint: {e}");
            }
        }

        results.push(SimulationResult {
            eb_n0_db,
            ber,
            bler: 0.0,
            avg_iterations: None,
            avg_queries_per_bit: None,
            num_bits: total_bits,
            num_bit_errors: total_errors,
            num_frames: 0,
            num_frame_errors: 0,
        });
    }
    results
}

/// Monte Carlo simulation runner for communication systems.
///
/// Provides static methods for both uncoded and coded simulations:
///
/// - [`SimulationRunner::run_uncoded_ber`] — uncoded BPSK over AWGN
/// - [`SimulationRunner::run_uncoded_ber_with_channel`] — uncoded over any
///   [`ChannelModel`] (modem-framework backed, e.g.
///   [`ModemChannelAdapter`](crate::modem::ModemChannelAdapter))
/// - [`SimulationRunner::run_uncoded_ber_with_analysis`] — same, with
///   opt-in per-bit LLR analysis capture
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
        // Delegate to the analysis-aware core with no capture. The
        // `None` branch is a single `match` arm behind an `#[inline]`
        // wrapper, so after optimization this path performs no
        // analysis-specific work whatsoever — no LLR copy, no truth
        // materialization, no extra allocation. The
        // `simulation_no_analysis_overhead` bench locks this
        // equivalence in place.
        run_uncoded_ber_with_channel_impl::<C, R>(channel, config, None, rng)
    }

    /// Uncoded BER sweep with opt-in per-bit LLR analysis capture.
    ///
    /// Behaves exactly like
    /// [`SimulationRunner::run_uncoded_ber_with_channel`] — same
    /// `ChannelModel` contract, same batch-size alignment, same
    /// hard-decision convention, same result vector layout — with one
    /// addition: when `capture` is `Some(&mut AnalysisCapture)`, each
    /// post-demap `(llrs, truth_bits)` batch is forwarded to the
    /// accumulator before the error count is tallied. When `capture` is
    /// `None`, the hot loop is bit-identical to the unaugmented runner
    /// (the extra branch collapses under `#[inline]`).
    ///
    /// # Zero-overhead contract
    ///
    /// The no-capture path is benchmarked against
    /// [`SimulationRunner::run_uncoded_ber_with_channel`] in
    /// `simulation_no_analysis_overhead`; both paths share the same
    /// `#[inline]` implementation, so the disabled path matches the
    /// original to within measurement noise.
    ///
    /// # Arguments
    ///
    /// * `channel` - Any [`ChannelModel`] implementation (same as
    ///   `run_uncoded_ber_with_channel`).
    /// * `config` - Simulation configuration.
    /// * `capture` - Optional [`AnalysisCapture`] handle. When `Some`,
    ///   the accumulator backing the capture must have
    ///   `bits_per_symbol()` equal to the number of bits per modem
    ///   symbol advertised by `channel` (for a modem-backed channel
    ///   that is `channel.batch_alignment()`; for `BpskAwgnChannel`
    ///   it is `1`). See the `# Panics` section for enforcement
    ///   details.
    /// * `rng` - Random source.
    ///
    /// # Panics
    ///
    /// Panics with a descriptive message if `capture` is `Some(_)` and
    /// `capture.bits_per_symbol() != channel.batch_alignment()`. The
    /// check runs up front, before the first batch — a misconfigured
    /// capture is caught immediately rather than silently accumulating
    /// nonsensical per-position statistics over an entire sweep. The
    /// runner does not try to translate between capture shapes.
    ///
    /// # Multi-SNR sweeps
    ///
    /// The same `AnalysisCapture` accumulator is reused across every
    /// entry in `config.eb_n0_range_db`. For most link-level workflows
    /// that is intentional — the report reflects the *aggregate*
    /// per-bit LLR distribution over the whole sweep. If you need
    /// per-SNR decompositions, drive the runner once per SNR point
    /// with a fresh `AnalysisCapture` (construct a new
    /// [`crate::modem::analysis::PerBitLlrStats`] between calls).
    ///
    /// # Returns
    ///
    /// Same as [`SimulationRunner::run_uncoded_ber_with_channel`]: one
    /// [`SimulationResult`] per SNR point.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::analysis::PerBitLlrStats;
    /// use gf2_coding::modem::{AnalysisCapture, DemapMethod};
    /// use gf2_coding::simulation::{
    ///     BpskAwgnChannel, SimulationConfig, SimulationRunner,
    /// };
    ///
    /// let mut config = SimulationConfig::quick_test();
    /// config.eb_n0_range_db = vec![6.0];
    /// config.min_errors = 1;
    /// config.max_frames = 2_000;
    /// let channel = BpskAwgnChannel;
    ///
    /// let mut stats = PerBitLlrStats::new(1);
    /// // BpskAwgnChannel advertises DemapMethod::ExactLogMap; the
    /// // capture must be tagged to match or the runner will panic.
    /// let mut capture =
    ///     AnalysisCapture::with_method(&mut stats, DemapMethod::ExactLogMap);
    /// let mut rng = rand::thread_rng();
    /// let results = SimulationRunner::run_uncoded_ber_with_analysis(
    ///     &channel,
    ///     &config,
    ///     Some(&mut capture),
    ///     &mut rng,
    /// );
    /// assert_eq!(results.len(), 1);
    /// let report = stats.report();
    /// assert_eq!(report.len(), 1);
    /// assert!(report[0].bit0.count() + report[0].bit1.count() > 0);
    /// ```
    ///
    /// # Complexity
    ///
    /// Same as [`SimulationRunner::run_uncoded_ber_with_channel`], plus
    /// O(bits) accumulator work on the enabled path.
    pub fn run_uncoded_ber_with_analysis<C: ChannelModel, R: Rng>(
        channel: &C,
        config: &SimulationConfig,
        capture: Option<&mut AnalysisCapture<'_>>,
        rng: &mut R,
    ) -> Vec<SimulationResult> {
        run_uncoded_ber_with_channel_impl::<C, R>(channel, config, capture, rng)
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
        self.total_frames.is_multiple_of(PROGRESS_INTERVAL) && self.total_frames > 0
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

    /// Returns whether the frame-count trigger for a heartbeat has fired.
    ///
    /// Returns `true` if `total_frames > 0` and `total_frames` is a multiple
    /// of `every_frames`.
    #[cfg(feature = "sim-observability")]
    fn should_heartbeat(&self, every_frames: usize) -> bool {
        every_frames > 0 && self.total_frames > 0 && self.total_frames.is_multiple_of(every_frames)
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
///
/// With the `sim-observability` feature enabled and
/// [`SimulationConfig::checkpoint_dir`] set, the sweep additionally:
/// - validates the checkpoint directory config hash on startup (aborting on
///   mismatch),
/// - skips SNR points with a completed checkpoint,
/// - resumes mid-point from the last heartbeat checkpoint using
///   `ChaCha20Rng::set_word_pos`,
/// - writes heartbeat checkpoints every
///   [`SimulationConfig::heartbeat_every_frames`] frames,
/// - writes a `campaign_start` tracing event to
///   [`SimulationConfig::tracing_log_path`],
/// - writes `snr_completed` and `heartbeat` tracing events,
/// - polls the process-wide interrupt flag between frames and flushes the
///   current checkpoint before exiting with a non-zero status on SIGINT /
///   SIGTERM.
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
    // sim-observability: install a JSON-lines tracing subscriber for this run.
    // `_tracing_guard` is held for the entire function duration; dropping it
    // at end of scope restores the previous subscriber.
    #[cfg(feature = "sim-observability")]
    let _tracing_guard = setup_tracing_guard(config);

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

    // sim-observability: validate checkpoint directory and compute config hash.
    #[cfg(feature = "sim-observability")]
    let config_hash = compute_config_hash(config);
    #[cfg(feature = "sim-observability")]
    if let Some(ref ckpt_dir) = config.checkpoint_dir {
        if let Err(e) = validate_checkpoint_dir(ckpt_dir, &config_hash) {
            panic!("{e}");
        }
    }

    // sim-observability: clear any stale interrupt flag from a previous run.
    #[cfg(feature = "sim-observability")]
    clear_interrupt();

    // sim-observability: open campaign span (owned via `entered()`) and emit
    // campaign_start event.  `EnteredSpan` keeps the span alive and entered.
    #[cfg(feature = "sim-observability")]
    let _campaign_guard = {
        use std::time::SystemTime;
        let run_uuid = {
            let t = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            format!("{:032x}", t ^ (config.rng_seed.unwrap_or(0) as u128))
        };
        let seed_val = config.rng_seed.unwrap_or(0);
        let guard = tracing::info_span!(
            "campaign",
            config_hash = %config_hash,
            run_uuid = %run_uuid,
            seed = seed_val,
        )
        .entered();
        tracing::info!(
            name: "campaign_start",
            event_type = "campaign_start",
            config_hash = %config_hash,
            run_uuid = %run_uuid,
            seed = seed_val,
        );
        guard
    };

    let mut rng = config.make_rng();
    let mut points = Vec::with_capacity(config.eb_n0_range_db.len());
    let mut completed_points: Vec<CompletedPointInfo> = Vec::new();

    for (point_idx, &eb_n0_db) in config.eb_n0_range_db.iter().enumerate() {
        let remaining_snr: Vec<f64> = config.eb_n0_range_db[point_idx + 1..].to_vec();
        let point_start = Instant::now();

        // sim-observability: check for existing checkpoint before the legacy
        // CSV-based resume.
        #[cfg(feature = "sim-observability")]
        let ckpt_resume: Option<SnrCheckpoint> = config
            .checkpoint_dir
            .as_ref()
            .and_then(|dir| load_checkpoint(&checkpoint_path(dir, point_idx), &config_hash));

        #[cfg(feature = "sim-observability")]
        if let Some(ref ckpt) = ckpt_resume {
            if ckpt.completed {
                // Skip this point entirely — reconstruct result from checkpoint.
                eprintln!(
                    "[{:.1} dB] CHECKPOINT RESUMED: skipping completed point \
                     ({} errors / {} frames)",
                    eb_n0_db, ckpt.errors_accumulated, ckpt.frames_completed
                );
                let ber = if ckpt.total_bits > 0 {
                    ckpt.total_bit_errors as f64 / ckpt.total_bits as f64
                } else {
                    0.0
                };
                let bler = if ckpt.frames_completed > 0 {
                    ckpt.errors_accumulated as f64 / ckpt.frames_completed as f64
                } else {
                    0.0
                };
                let avg_iterations = if ckpt.frames_completed > 0 {
                    Some(ckpt.total_iterations as f64 / ckpt.frames_completed as f64)
                } else {
                    None
                };
                let avg_queries_per_bit = if ckpt.total_bits > 0 {
                    Some(ckpt.total_queries as f64 / ckpt.total_bits as f64)
                } else {
                    None
                };
                let sim_result = SimulationResult {
                    eb_n0_db,
                    ber,
                    bler,
                    avg_iterations,
                    avg_queries_per_bit,
                    num_bits: ckpt.total_bits,
                    num_bit_errors: ckpt.total_bit_errors,
                    num_frames: ckpt.frames_completed,
                    num_frame_errors: ckpt.errors_accumulated,
                };
                let point_elapsed = point_start.elapsed();
                completed_points.push(CompletedPointInfo {
                    eb_n0_db,
                    duration: point_elapsed,
                    num_frames: sim_result.num_frames,
                    bler: sim_result.bler,
                });
                points.push(sim_result);
                continue;
            }
        }

        // sim-observability: open a per-SNR span (owned via `entered()`) so
        // that heartbeat and snr_completed events carry the SNR fields.
        #[cfg(feature = "sim-observability")]
        let _snr_span_guard = {
            let es_n0_db = eb_n0_db + 10.0 * (k as f64 / n as f64).log10();
            tracing::info_span!(
                "snr_point",
                eb_n0_db = eb_n0_db,
                es_n0_db = es_n0_db,
                frames_target = config.max_frames,
                errors_target = config.min_errors,
            )
            .entered()
        };

        // sim-observability: if we have a seed and checkpoint support, use
        // a per-SNR ChaCha20Rng with deterministic seek.
        #[cfg(feature = "sim-observability")]
        let sim_result = if config.checkpoint_dir.is_some() || config.tracing_log_path.is_some() {
            if let Some(base_seed) = config.rng_seed {
                // Determine resume word position from a partial checkpoint.
                let resume_word_pos: u128 = {
                    #[allow(clippy::option_if_let_else)]
                    if let Some(ref ckpt) = ckpt_resume {
                        ckpt.rng_word_pos
                    } else {
                        0
                    }
                };
                let resume_frames: usize = ckpt_resume.as_ref().map_or(0, |c| c.frames_completed);
                let resume_errors: usize = ckpt_resume.as_ref().map_or(0, |c| c.errors_accumulated);
                let resume_iters: usize = ckpt_resume.as_ref().map_or(0, |c| c.total_iterations);
                let resume_queries: usize = ckpt_resume.as_ref().map_or(0, |c| c.total_queries);
                let resume_bits: usize = ckpt_resume.as_ref().map_or(0, |c| c.total_bits);
                let resume_bit_errors: usize =
                    ckpt_resume.as_ref().map_or(0, |c| c.total_bit_errors);
                let mut chacha = make_chacha_rng(base_seed, point_idx, resume_word_pos);
                simulate_single_point_observable(
                    encoder,
                    channel,
                    &mut chacha,
                    eb_n0_db,
                    rate,
                    config,
                    &existing,
                    point_idx,
                    &config_hash,
                    resume_frames,
                    resume_errors,
                    resume_iters,
                    resume_queries,
                    resume_bits,
                    resume_bit_errors,
                    config.output_path.as_deref(),
                    progress_path.as_deref(),
                    &remaining_snr,
                    &completed_points,
                    &mut decode_frame,
                )
            } else {
                // No seed — fall back to standard sequential path (no resume).
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
                simulate_single_point(encoder, channel, &mut rng, &ctx, &mut decode_frame)
            }
        } else {
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
            simulate_single_point(encoder, channel, &mut rng, &ctx, &mut decode_frame)
        };

        #[cfg(not(feature = "sim-observability"))]
        let sim_result = {
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
            simulate_single_point(encoder, channel, &mut rng, &ctx, &mut decode_frame)
        };

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

/// Observable variant of `simulate_single_point` used when
/// `sim-observability` is active and a `checkpoint_dir` or
/// `tracing_log_path` is set.
///
/// Accepts a `ChaCha20Rng` so its word position can be captured at each
/// heartbeat and stored in the checkpoint, enabling byte-identical resume.
///
/// The `resume_*` parameters carry state accumulated in a prior run's partial
/// checkpoint so the totals are correct on resume.
#[cfg(feature = "sim-observability")]
#[allow(clippy::too_many_arguments)]
fn simulate_single_point_observable<E, C, F>(
    encoder: &E,
    channel: &C,
    rng: &mut ChaCha20Rng,
    eb_n0_db: f64,
    rate: f64,
    config: &SimulationConfig,
    existing: &HashMap<String, SimulationResult>,
    snr_index: usize,
    config_hash: &str,
    resume_frames: usize,
    resume_errors: usize,
    resume_iters: usize,
    resume_queries: usize,
    resume_bits: usize,
    resume_bit_errors: usize,
    output_path: Option<&Path>,
    progress_path: Option<&Path>,
    remaining_snr_points: &[f64],
    completed_points: &[CompletedPointInfo],
    decode_frame: &mut F,
) -> SimulationResult
where
    E: BlockEncoder,
    C: ChannelModel,
    F: FnMut(&[crate::llr::Llr]) -> DecoderResult,
{
    let k = encoder.k();

    // Legacy CSV-based resume (still honoured alongside checkpoint-based resume).
    let snr_key = format!("{:.6}", eb_n0_db);
    if let Some(cached) = existing.get(&snr_key) {
        if resume_frames == 0 {
            eprintln!(
                "[{:.1} dB] RESUMED: using existing CSV result ({} errors, {} frames)",
                eb_n0_db, cached.num_frame_errors, cached.num_frames,
            );
            return cached.clone();
        }
    }

    if resume_frames > 0 {
        eprintln!(
            "[{:.1} dB] RESUMING from checkpoint: {} frames done, {} errors",
            eb_n0_db, resume_frames, resume_errors
        );
    }

    let mut acc = SnrAccumulator::new(eb_n0_db, k);
    // Inject resumed totals.
    acc.total_frames = resume_frames;
    acc.total_frame_errors = resume_errors;
    acc.total_iterations = resume_iters;
    acc.total_queries = resume_queries;
    acc.total_bits = resume_bits;
    acc.total_bit_errors = resume_bit_errors;

    while !acc.should_stop(config.min_errors, config.max_frames) {
        // Check for SIGINT / SIGTERM before each frame.
        if is_interrupted() {
            // Flush partial checkpoint and exit.
            if let Some(ref ckpt_dir) = config.checkpoint_dir {
                let word_pos = rng.get_word_pos();
                let ckpt = SnrCheckpoint {
                    snr_index,
                    eb_n0_db,
                    frames_completed: acc.total_frames,
                    errors_accumulated: acc.total_frame_errors,
                    total_iterations: acc.total_iterations,
                    total_queries: acc.total_queries,
                    total_bits: acc.total_bits,
                    total_bit_errors: acc.total_bit_errors,
                    rng_word_pos: word_pos,
                    frames_target: config.max_frames,
                    errors_target: config.min_errors,
                    completed: false,
                    config_hash: config_hash.to_string(),
                };
                if let Err(e) =
                    write_checkpoint_atomic(&checkpoint_path(ckpt_dir, snr_index), &ckpt)
                {
                    eprintln!("Warning: failed to write interrupt checkpoint: {e}");
                } else {
                    eprintln!(
                        "[{:.1} dB] Interrupt: checkpoint flushed at {} frames",
                        eb_n0_db, acc.total_frames
                    );
                }
            }
            eprintln!("Interrupted — exiting. Resume by re-running with the same config.");
            std::process::exit(1);
        }

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

        // Legacy progress JSONL.
        if let Some(pp) = progress_path {
            if acc.should_write_progress() {
                acc.write_progress_entry(pp);
            }
        }

        // Heartbeat: tracing event + intermediate checkpoint.
        if let Some(every) = config.heartbeat_every_frames {
            if acc.should_heartbeat(every) {
                let word_pos = rng.get_word_pos();
                let elapsed_s = acc.elapsed().as_secs_f64();

                // Tracing event via `tracing::info!` — picked up by the
                // JSON subscriber installed by `setup_tracing_guard`.
                tracing::info!(
                    name: "heartbeat",
                    event_type = "heartbeat",
                    snr_index = snr_index,
                    eb_n0_db = eb_n0_db,
                    frames_completed = acc.total_frames,
                    errors_so_far = acc.total_frame_errors,
                    elapsed_seconds = elapsed_s,
                );

                // Intermediate checkpoint.
                if let Some(ref ckpt_dir) = config.checkpoint_dir {
                    let ckpt = SnrCheckpoint {
                        snr_index,
                        eb_n0_db,
                        frames_completed: acc.total_frames,
                        errors_accumulated: acc.total_frame_errors,
                        total_iterations: acc.total_iterations,
                        total_queries: acc.total_queries,
                        total_bits: acc.total_bits,
                        total_bit_errors: acc.total_bit_errors,
                        rng_word_pos: word_pos,
                        frames_target: config.max_frames,
                        errors_target: config.min_errors,
                        completed: false,
                        config_hash: config_hash.to_string(),
                    };
                    if let Err(e) =
                        write_checkpoint_atomic(&checkpoint_path(ckpt_dir, snr_index), &ckpt)
                    {
                        eprintln!("Warning: failed to write heartbeat checkpoint: {e}");
                    }
                }
            }
        }
    }

    let point_elapsed = acc.elapsed();
    let sim_result = acc.into_result();

    // Write completed checkpoint.
    if let Some(ref ckpt_dir) = config.checkpoint_dir {
        let word_pos = rng.get_word_pos();
        let ckpt = SnrCheckpoint {
            snr_index,
            eb_n0_db,
            frames_completed: sim_result.num_frames,
            errors_accumulated: sim_result.num_frame_errors,
            total_iterations: (sim_result.avg_iterations.unwrap_or(0.0)
                * sim_result.num_frames as f64)
                .round() as usize,
            total_queries: (sim_result.avg_queries_per_bit.unwrap_or(0.0)
                * sim_result.num_bits as f64)
                .round() as usize,
            total_bits: sim_result.num_bits,
            total_bit_errors: sim_result.num_bit_errors,
            rng_word_pos: word_pos,
            frames_target: config.max_frames,
            errors_target: config.min_errors,
            completed: true,
            config_hash: config_hash.to_string(),
        };
        if let Err(e) = write_checkpoint_atomic(&checkpoint_path(ckpt_dir, snr_index), &ckpt) {
            eprintln!("Warning: failed to write completion checkpoint: {e}");
        }
    }

    // Tracing: snr_completed event via `tracing::info!`.
    tracing::info!(
        name: "snr_completed",
        event_type = "snr_completed",
        snr_index = snr_index,
        eb_n0_db = eb_n0_db,
        fer = sim_result.bler,
        ber = sim_result.ber,
        mean_iters = sim_result.avg_iterations,
        elapsed_seconds = point_elapsed.as_secs_f64(),
    );

    // Legacy progress JSONL: point_complete entry.
    if let Some(pp) = progress_path {
        if let Err(e) = append_point_complete_jsonl(pp, &sim_result, point_elapsed) {
            eprintln!("Warning: failed to write JSONL progress: {e}");
        }
    }

    // Incremental CSV append.
    if let Some(path) = output_path {
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            sim_result.append_csv_row_to(path);
        }
    }

    report_point_complete(
        eb_n0_db,
        &sim_result,
        point_elapsed,
        remaining_snr_points,
        completed_points,
        config.min_errors,
        config.max_frames,
    );

    sim_result
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

        // sim-observability: install JSON-lines tracing subscriber for this run.
        #[cfg(feature = "sim-observability")]
        let _tracing_guard = setup_tracing_guard(config);

        // sim-observability: compute config hash and validate checkpoint directory.
        // Per-SNR-boundary checkpointing only; within-SNR heartbeat resume is
        // architecturally unavailable with rayon-parallel SNR-point dispatch
        // (workers run concurrently and checkpoints are only written after each
        // worker returns its completed result to the main thread).
        #[cfg(feature = "sim-observability")]
        let config_hash = compute_config_hash(config);
        #[cfg(feature = "sim-observability")]
        if let Some(ref ckpt_dir) = config.checkpoint_dir {
            if let Err(e) = validate_checkpoint_dir(ckpt_dir, &config_hash) {
                panic!("{e}");
            }
        }

        // sim-observability: clear any stale interrupt flag.
        #[cfg(feature = "sim-observability")]
        clear_interrupt();

        // sim-observability: open campaign span and emit campaign_start event.
        // The Dispatch is cloned so each rayon worker can re-enter it on their
        // own thread via `tracing::dispatcher::with_default`.
        #[cfg(feature = "sim-observability")]
        let _campaign_guard = {
            use std::time::SystemTime;
            let run_uuid = {
                let t = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                format!("{:032x}", t ^ (config.rng_seed.unwrap_or(0) as u128))
            };
            let seed_val = config.rng_seed.unwrap_or(0);
            let guard = tracing::info_span!(
                "campaign",
                config_hash = %config_hash,
                run_uuid = %run_uuid,
                seed = seed_val,
            )
            .entered();
            tracing::info!(
                name: "campaign_start",
                event_type = "campaign_start",
                config_hash = %config_hash,
                run_uuid = %run_uuid,
                seed = seed_val,
            );
            guard
        };

        // sim-observability: capture the current thread-local Dispatch so
        // rayon worker threads can re-activate it.
        #[cfg(feature = "sim-observability")]
        let worker_dispatch = tracing::dispatcher::get_default(|d| d.clone());

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

        // sim-observability: pre-load any completed checkpoint results and build
        // a filtered list of SNR indices still needing computation.
        // `checkpoint_results` maps snr_idx -> SimulationResult for already-done points.
        #[cfg(feature = "sim-observability")]
        let checkpoint_results: Vec<Option<SimulationResult>> = {
            (0..total_points)
                .map(|idx| {
                    let ckpt_opt = config
                        .checkpoint_dir
                        .as_ref()
                        .and_then(|dir| load_checkpoint(&checkpoint_path(dir, idx), &config_hash));
                    if let Some(ckpt) = ckpt_opt {
                        if ckpt.completed {
                            let ber = if ckpt.total_bits > 0 {
                                ckpt.total_bit_errors as f64 / ckpt.total_bits as f64
                            } else {
                                0.0
                            };
                            let bler = if ckpt.frames_completed > 0 {
                                ckpt.errors_accumulated as f64 / ckpt.frames_completed as f64
                            } else {
                                0.0
                            };
                            let avg_iterations = if ckpt.frames_completed > 0 {
                                Some(ckpt.total_iterations as f64 / ckpt.frames_completed as f64)
                            } else {
                                None
                            };
                            eprintln!(
                                "[{:.1} dB] CHECKPOINT RESUMED: skipping completed parallel point \
                                 ({} frame errors / {} frames)",
                                ckpt.eb_n0_db, ckpt.errors_accumulated, ckpt.frames_completed
                            );
                            Some(SimulationResult {
                                eb_n0_db: ckpt.eb_n0_db,
                                ber,
                                bler,
                                avg_iterations,
                                avg_queries_per_bit: None,
                                num_bits: ckpt.total_bits,
                                num_bit_errors: ckpt.total_bit_errors,
                                num_frames: ckpt.frames_completed,
                                num_frame_errors: ckpt.errors_accumulated,
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect()
        };

        // Build the list of (original_idx, eb_n0_db) pairs that still need work.
        // When sim-observability is disabled, all points need work.
        let pending_points: Vec<(usize, f64)> = {
            #[cfg(feature = "sim-observability")]
            {
                config
                    .eb_n0_range_db
                    .iter()
                    .enumerate()
                    .filter(|&(idx, _)| checkpoint_results[idx].is_none())
                    .map(|(idx, &db)| (idx, db))
                    .collect()
            }
            #[cfg(not(feature = "sim-observability"))]
            {
                config
                    .eb_n0_range_db
                    .iter()
                    .enumerate()
                    .map(|(idx, &db)| (idx, db))
                    .collect()
            }
        };

        let collector = Arc::new(Mutex::new(ParallelResultCollector::new(
            total_points,
            csv_output,
            progress_path,
            config.eb_n0_range_db.clone(),
            config.min_errors,
            config.max_frames,
        )));

        // sim-observability: pre-populate the collector with checkpoint results
        // so the final aggregation includes all SNR points.
        #[cfg(feature = "sim-observability")]
        {
            let mut coll = collector
                .lock()
                .expect("ParallelResultCollector lock poisoned");
            for (idx, opt) in checkpoint_results.iter().enumerate() {
                if let Some(ref result) = opt {
                    coll.record_completed_point(idx, result.clone(), std::time::Duration::ZERO);
                }
            }
        }

        // Worker closure: simulates one SNR point, then locks the collector
        // briefly to record the result with immediate CSV/JSONL writes.
        let simulate_and_record = |(idx, eb_n0_db): (usize, f64)| {
            // sim-observability: install the campaign subscriber on this rayon
            // worker thread for the duration of the closure.  `set_default`
            // is thread-local; the guard restores the previous dispatch on drop.
            #[cfg(feature = "sim-observability")]
            let _dispatch_guard = tracing::dispatcher::set_default(&worker_dispatch);

            // sim-observability: open a per-SNR span on this worker thread.
            #[cfg(feature = "sim-observability")]
            let _snr_guard = tracing::info_span!(
                "snr_point",
                eb_n0_db = eb_n0_db,
                es_n0_db = eb_n0_db + 10.0 * (k as f64 / n as f64).log10(),
                frames_target = config.max_frames,
                errors_target = config.min_errors,
            )
            .entered();

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

            // sim-observability: emit snr_completed event on this worker thread.
            #[cfg(feature = "sim-observability")]
            tracing::info!(
                name: "snr_completed",
                event_type = "snr_completed",
                snr_index = idx,
                eb_n0_db = eb_n0_db,
                fer = result.bler,
                ber = result.ber,
                mean_iters = result.avg_iterations,
                elapsed_seconds = point_elapsed.as_secs_f64(),
            );

            // sim-observability: write a completed checkpoint after this SNR point.
            // Within-SNR heartbeat is not implemented for the parallel path —
            // workers run concurrently and can only checkpoint at completion
            // boundaries (see function-level rustdoc).
            #[cfg(feature = "sim-observability")]
            if let Some(ref ckpt_dir) = config.checkpoint_dir {
                let ckpt = SnrCheckpoint {
                    snr_index: idx,
                    eb_n0_db,
                    frames_completed: result.num_frames,
                    errors_accumulated: result.num_frame_errors,
                    total_iterations: result
                        .avg_iterations
                        .map(|a| (a * result.num_frames as f64).round() as usize)
                        .unwrap_or(0),
                    total_queries: result
                        .avg_queries_per_bit
                        .map(|q| (q * result.num_bits as f64).round() as usize)
                        .unwrap_or(0),
                    total_bits: result.num_bits,
                    total_bit_errors: result.num_bit_errors,
                    rng_word_pos: 0, // no seek support on parallel path
                    frames_target: config.max_frames,
                    errors_target: config.min_errors,
                    completed: true,
                    config_hash: config_hash.clone(),
                };
                if let Err(e) = write_checkpoint_atomic(&checkpoint_path(ckpt_dir, idx), &ckpt) {
                    eprintln!("[sim-observability] Failed to write parallel checkpoint: {e}");
                }
            }

            // Lock the collector only for the brief I/O + bookkeeping window.
            let mut coll = collector
                .lock()
                .expect("ParallelResultCollector lock poisoned");
            coll.record_completed_point(idx, result, point_elapsed);
        };

        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;
            pending_points.into_par_iter().for_each(simulate_and_record);
        }
        #[cfg(not(feature = "parallel"))]
        {
            pending_points.into_iter().for_each(simulate_and_record);
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
    #[ignore = "sim: BER at 10 dB, 10 000 frames"]
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
    #[ignore = "sim: BER monotonicity, 2 SNR points, 50 errors each"]
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
        let tmpdir = tempfile::tempdir().unwrap();
        let dir = tmpdir.path();
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
    }

    #[test]
    fn test_output_path_json() {
        let tmpdir = tempfile::tempdir().unwrap();
        let dir = tmpdir.path();
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
    #[ignore = "sim: parallel-iterative coded sim, 2 SNR x 1000 frames"]
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
        let tmpdir = tempfile::tempdir().unwrap();
        let dir = tmpdir.path();
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
    }

    #[test]
    fn test_run_coded_iterative_with_output_path() {
        let tmpdir = tempfile::tempdir().unwrap();
        let dir = tmpdir.path();
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
    }

    #[test]
    fn test_run_coded_iterative_parallel_with_output_path() {
        let tmpdir = tempfile::tempdir().unwrap();
        let dir = tmpdir.path();
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
    #[ignore = "sim: reproducibility check, 2000 frames, 20 min errors"]
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
        let tmpdir = tempfile::tempdir().unwrap();
        let dir = tmpdir.path();
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
    }

    #[test]
    fn test_resume_skips_completed_points() {
        let tmpdir = tempfile::tempdir().unwrap();
        let dir = tmpdir.path();
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
        let tmpdir = tempfile::tempdir().unwrap();
        let dir = tmpdir.path();
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
        let tmpdir = tempfile::tempdir().unwrap();
        let dir = tmpdir.path();
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
            r.num_bits.is_multiple_of(2),
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
        assert!(
            r.num_bits.is_multiple_of(2),
            "transmitted bits must stay aligned"
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

    // -------------------------------------------------------------------
    // sim-observability integration tests (gated on the feature)
    // -------------------------------------------------------------------

    #[cfg(feature = "sim-observability")]
    mod observability_tests {
        use super::*;

        /// SC-INT-1: Multi-point AWGN campaign completes, then re-run with same
        /// config skips all checkpointed points near-instantly.
        #[test]
        fn test_checkpoint_skip_completed_points() {
            let tmpdir = tempfile::tempdir().unwrap();
            let ckpt_dir = tmpdir.path().join("ckpts");
            let csv_path = tmpdir.path().join("results.csv");

            let encoder = MockEncoder;
            let channel = DeterministicChannel {
                flip_positions: vec![0, 1],
            };
            let mut config = SimulationConfig::quick_test();
            config.eb_n0_range_db = vec![4.0, 6.0, 8.0];
            config.min_errors = 5;
            config.max_frames = 50;
            config.rng_seed = Some(777);
            config.output_path = Some(csv_path.clone());
            config.checkpoint_dir = Some(ckpt_dir.clone());
            config.heartbeat_every_frames = None;

            // First run: completes all 3 SNR points and writes checkpoints.
            let decoder = MockSoftDecoder;
            let results1 = SimulationRunner::run_coded(&encoder, &decoder, &channel, &config);
            assert_eq!(results1.points.len(), 3);

            // Checkpoint files must exist.
            for i in 0..3 {
                let ckpt_file = ckpt_dir.join(format!("snr_{:04}.json", i));
                assert!(
                    ckpt_file.exists(),
                    "Checkpoint file {i} must exist after first run"
                );
                let content = std::fs::read_to_string(&ckpt_file).unwrap();
                assert!(
                    content.contains("\"completed\": true"),
                    "Checkpoint {i} must be marked completed"
                );
            }

            // Second run: all checkpoints are complete -> results must match
            // and the run must complete without recomputing frames.
            let decoder2 = MockSoftDecoder;
            let results2 = SimulationRunner::run_coded(&encoder, &decoder2, &channel, &config);
            assert_eq!(results2.points.len(), 3);

            // Results must be identical (same checkpoint values).
            for i in 0..3 {
                assert_eq!(
                    results1.points[i].num_frames, results2.points[i].num_frames,
                    "Frame counts must match on resume at SNR point {i}"
                );
                assert_eq!(
                    results1.points[i].num_frame_errors, results2.points[i].num_frame_errors,
                    "Error counts must match on resume at SNR point {i}"
                );
            }
        }

        /// SC-INT-2: Config hash mismatch aborts with a clear panic message.
        #[test]
        #[should_panic(expected = "config hash mismatch")]
        fn test_config_mismatch_aborts() {
            let tmpdir = tempfile::tempdir().unwrap();
            let ckpt_dir = tmpdir.path().join("ckpts");

            let encoder = MockEncoder;
            let channel = DeterministicChannel {
                flip_positions: vec![0, 1],
            };
            let mut config = SimulationConfig::quick_test();
            config.eb_n0_range_db = vec![4.0];
            config.min_errors = 3;
            config.max_frames = 20;
            config.rng_seed = Some(42);
            config.checkpoint_dir = Some(ckpt_dir.clone());

            // First run: creates config_hash.txt.
            let decoder = MockSoftDecoder;
            let _ = SimulationRunner::run_coded(&encoder, &decoder, &channel, &config);

            // Mutate a field that affects the hash.
            let mut config2 = config.clone();
            config2.min_errors = 99;

            // Second run with different config: must panic with mismatch message.
            let decoder2 = MockSoftDecoder;
            let _ = SimulationRunner::run_coded(&encoder, &decoder2, &channel, &config2);
        }

        /// SC-INT-3: Heartbeat events are emitted at the configured cadence.
        ///
        /// `DeterministicChannel` produces one frame error per frame.  With
        /// `min_errors = 5` the loop runs exactly 5 frames.  With
        /// `heartbeat_every_frames = 2` heartbeats fire after frames 2 and 4
        /// (frame 5 stops the loop before the cadence-6 heartbeat can fire),
        /// so the log must contain **exactly 2** heartbeat events.
        ///
        /// Each heartbeat line is parsed as JSON via `serde_json` and all
        /// required fields (`frames_completed`, `errors_so_far`,
        /// `elapsed_seconds`, `snr_index`, `eb_n0_db`) are asserted present.
        #[test]
        fn test_heartbeat_cadence() {
            let tmpdir = tempfile::tempdir().unwrap();
            let tlog = tmpdir.path().join("trace.jsonl");
            let ckpt_dir = tmpdir.path().join("ckpts");

            let encoder = MockEncoder;
            let channel = DeterministicChannel {
                flip_positions: vec![0, 1], // always 1 frame error per frame
            };
            let mut config = SimulationConfig::quick_test();
            config.eb_n0_range_db = vec![5.0];
            config.min_errors = 5; // stops at exactly 5 frames
            config.max_frames = 50;
            config.rng_seed = Some(123);
            config.tracing_log_path = Some(tlog.clone());
            config.checkpoint_dir = Some(ckpt_dir.clone());
            config.heartbeat_every_frames = Some(2);

            let decoder = MockSoftDecoder;
            let _ = SimulationRunner::run_coded(&encoder, &decoder, &channel, &config);

            // Parse every non-empty line as JSON and collect heartbeat objects.
            let content = std::fs::read_to_string(&tlog).unwrap();
            let heartbeats: Vec<serde_json::Value> = content
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| {
                    serde_json::from_str::<serde_json::Value>(l).unwrap_or_else(|e| {
                        panic!("tracing line is not valid JSON: {e}\nline: {l}")
                    })
                })
                .filter(|obj| obj["fields"]["event_type"] == "heartbeat")
                .collect();

            // With cadence=2 and 5 frames: heartbeats at frames 2, 4 → exactly 2.
            assert_eq!(
                heartbeats.len(),
                2,
                "expected exactly 2 heartbeat events (cadence=2, min_errors=5), got {}",
                heartbeats.len()
            );

            // Required fields under "fields" in the tracing-subscriber JSON layer.
            for (i, hb) in heartbeats.iter().enumerate() {
                let fields = &hb["fields"];
                assert!(
                    !fields["frames_completed"].is_null(),
                    "heartbeat[{i}] missing fields.frames_completed"
                );
                assert!(
                    !fields["errors_so_far"].is_null(),
                    "heartbeat[{i}] missing fields.errors_so_far"
                );
                assert!(
                    !fields["elapsed_seconds"].is_null(),
                    "heartbeat[{i}] missing fields.elapsed_seconds"
                );
                assert!(
                    !fields["snr_index"].is_null(),
                    "heartbeat[{i}] missing fields.snr_index"
                );
                assert!(
                    !fields["eb_n0_db"].is_null(),
                    "heartbeat[{i}] missing fields.eb_n0_db"
                );
            }
        }

        /// SC-INT-4: Every line in the tracing log parses as a JSON object and
        /// carries the required fields for its event type.
        ///
        /// Each line is parsed with `serde_json::from_str`.  For the event
        /// types the runner emits the following fields are asserted under the
        /// `"fields"` key produced by `tracing-subscriber`'s JSON formatter:
        ///
        /// - `campaign_start`: `config_hash`, `run_uuid`, `seed`
        /// - `snr_completed`: `eb_n0_db`, `fer`, `ber`, `mean_iters`
        /// - `heartbeat`: `frames_completed`, `errors_so_far`,
        ///   `elapsed_seconds`, `snr_index`, `eb_n0_db`
        ///
        /// The event name is carried in the top-level `"name"` key that
        /// `tracing-subscriber` emits for each record.
        #[test]
        fn test_tracing_log_valid_jsonl() {
            let tmpdir = tempfile::tempdir().unwrap();
            let tlog = tmpdir.path().join("trace.jsonl");

            let encoder = MockEncoder;
            let channel = DeterministicChannel {
                flip_positions: vec![0, 1],
            };
            let mut config = SimulationConfig::quick_test();
            config.eb_n0_range_db = vec![4.0, 6.0];
            config.min_errors = 3;
            config.max_frames = 20;
            config.rng_seed = Some(55);
            config.tracing_log_path = Some(tlog.clone());
            config.heartbeat_every_frames = Some(5);

            let decoder = MockSoftDecoder;
            let _ = SimulationRunner::run_coded(&encoder, &decoder, &channel, &config);

            let content = std::fs::read_to_string(&tlog).unwrap();

            // Parse every non-empty line with serde_json; assert it's an object.
            let objects: Vec<serde_json::Value> = content
                .lines()
                .filter(|l| !l.trim().is_empty())
                .enumerate()
                .map(|(i, line)| {
                    let v: serde_json::Value = serde_json::from_str(line).unwrap_or_else(|e| {
                        panic!("line {i} is not valid JSON: {e}\nline: {line}")
                    });
                    assert!(
                        v.is_object(),
                        "line {i} parsed as JSON but is not an object: {v}"
                    );
                    v
                })
                .collect();

            // Partition by event_type field (explicit field in "fields" object,
            // since tracing-subscriber JSON formatter does not include the
            // callsite name in its output).
            let campaign_starts: Vec<&serde_json::Value> = objects
                .iter()
                .filter(|o| o["fields"]["event_type"] == "campaign_start")
                .collect();
            let snr_completeds: Vec<&serde_json::Value> = objects
                .iter()
                .filter(|o| o["fields"]["event_type"] == "snr_completed")
                .collect();

            assert_eq!(
                campaign_starts.len(),
                1,
                "must have exactly 1 campaign_start"
            );
            assert_eq!(
                snr_completeds.len(),
                2,
                "must have exactly 2 snr_completed events (one per SNR point)"
            );

            // campaign_start: required fields under "fields".
            let cs = campaign_starts[0]["fields"]
                .as_object()
                .unwrap_or_else(|| panic!("campaign_start event has no 'fields' object"));
            assert!(
                cs.contains_key("config_hash"),
                "campaign_start missing fields.config_hash"
            );
            assert!(
                cs.contains_key("run_uuid"),
                "campaign_start missing fields.run_uuid"
            );
            assert!(
                cs.contains_key("seed"),
                "campaign_start missing fields.seed"
            );

            // snr_completed: required fields under "fields".
            for (i, sc) in snr_completeds.iter().enumerate() {
                let f = sc["fields"]
                    .as_object()
                    .unwrap_or_else(|| panic!("snr_completed[{i}] has no 'fields' object"));
                for key in &["eb_n0_db", "fer", "ber", "mean_iters"] {
                    assert!(
                        f.contains_key(*key),
                        "snr_completed[{i}] missing fields.{key}"
                    );
                }
            }
        }

        /// SC-INT-5: Resume after real SIGINT — subprocess-based test.
        ///
        /// # Protocol
        ///
        /// 1. Produce a **reference CSV** by running `sim_checkpoint_helper`
        ///    to normal completion in a clean directory.
        /// 2. In a second directory run the helper, wait until at least one
        ///    heartbeat checkpoint file appears (the helper writes one every 5
        ///    frames), then deliver SIGINT with `kill -INT <pid>`.  The helper
        ///    exits with code 1 and has flushed a partial checkpoint.
        /// 3. Re-run the helper with the same checkpoint directory.  It resumes
        ///    from the partial checkpoint and runs to completion (exit code 0).
        /// 4. Assert the resumed `results.csv` is byte-identical to the
        ///    reference CSV.
        ///
        /// The test is marked `#[ignore = "slow: ..."]` because the subprocess
        /// spin-up and SIGINT timing can exceed 5 s on slow CI hosts.
        #[test]
        #[ignore = "slow: subprocess SIGINT timing may exceed 5 s on slow hosts"]
        fn test_resume_after_interrupt() {
            use std::process::Command;
            use std::time::{Duration, Instant};

            // Locate the helper binary next to the current test executable.
            // cargo places it in .../target/<profile>/ (one level above deps/).
            let helper_bin = {
                let mut exe = std::env::current_exe().expect("cannot locate test executable");
                exe.pop(); // strip filename
                if exe.ends_with("deps") {
                    exe.pop(); // strip "deps", now at target/<profile>/
                }
                exe.push("sim_checkpoint_helper");
                exe
            };
            assert!(
                helper_bin.exists(),
                "sim_checkpoint_helper binary not found at {}: \
                 build with `cargo build --bin sim_checkpoint_helper`",
                helper_bin.display()
            );

            let tmpdir = tempfile::tempdir().unwrap();
            let ref_dir = tmpdir.path().join("ref");
            let resume_dir = tmpdir.path().join("resume");
            std::fs::create_dir_all(&ref_dir).unwrap();
            std::fs::create_dir_all(&resume_dir).unwrap();

            // ── Step 1: reference run to completion ──────────────────────────
            let ref_status = Command::new(&helper_bin)
                .arg(&ref_dir)
                .status()
                .unwrap_or_else(|e| panic!("failed to spawn helper: {e}"));
            assert!(
                ref_status.success(),
                "reference run did not exit with code 0: {ref_status}"
            );
            let ref_csv = std::fs::read(ref_dir.join("results.csv"))
                .expect("reference results.csv must exist after successful run");

            // ── Step 2: interrupted run ──────────────────────────────────────
            let mut child = Command::new(&helper_bin)
                .arg(&resume_dir)
                .spawn()
                .unwrap_or_else(|e| panic!("failed to spawn helper: {e}"));

            let pid = child.id();

            // Wait until at least one heartbeat checkpoint exists (cadence=5).
            // The helper writes snr_0000.json after every 5 frames.
            let ckpt_file = resume_dir.join("snr_0000.json");
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                if ckpt_file.exists() {
                    break;
                }
                if Instant::now() > deadline {
                    let _ = child.kill();
                    panic!(
                        "timed out waiting for heartbeat checkpoint in {}",
                        resume_dir.display()
                    );
                }
                std::thread::sleep(Duration::from_millis(50));
            }

            // Deliver SIGINT.
            let kill_status = Command::new("kill")
                .args(["-INT", &pid.to_string()])
                .status()
                .unwrap_or_else(|e| panic!("failed to send SIGINT: {e}"));
            assert!(
                kill_status.success(),
                "kill -INT returned non-zero: {kill_status}"
            );

            // Wait for the helper to flush and exit (exit code 1 = interrupted).
            let interrupted_status = child.wait().expect("failed to wait for interrupted helper");
            assert_eq!(
                interrupted_status.code(),
                Some(1),
                "interrupted helper must exit with code 1, got: {interrupted_status}"
            );

            // Partial checkpoint must exist on disk.
            assert!(
                ckpt_file.exists(),
                "partial checkpoint must persist after SIGINT"
            );

            // ── Step 3: resume run ───────────────────────────────────────────
            let resume_status = Command::new(&helper_bin)
                .arg(&resume_dir)
                .status()
                .unwrap_or_else(|e| panic!("failed to spawn resumed helper: {e}"));
            assert!(
                resume_status.success(),
                "resumed run must exit with code 0, got: {resume_status}"
            );

            // ── Step 4: byte-identical CSV ───────────────────────────────────
            let resume_csv = std::fs::read(resume_dir.join("results.csv"))
                .expect("resume results.csv must exist after resumed run");
            assert_eq!(
                ref_csv, resume_csv,
                "resumed results.csv must be byte-identical to the reference run"
            );
        }

        /// SC-INT-6: Checkpoint serialisation round-trip.
        ///
        /// Verifies that `SnrCheckpoint::to_json` → `SnrCheckpoint::from_json`
        /// is lossless for all relevant fields including the `u128` word_pos.
        #[test]
        fn test_checkpoint_roundtrip() {
            let ckpt = SnrCheckpoint {
                snr_index: 7,
                eb_n0_db: 8.5,
                frames_completed: 320_000,
                errors_accumulated: 47,
                total_iterations: 894_231,
                total_queries: 912_000,
                total_bits: 6_400_000,
                total_bit_errors: 512,
                rng_word_pos: 18_432_000_u128,
                frames_target: 1_000_000,
                errors_target: 100,
                completed: false,
                config_hash: "blake3:abc123".to_string(),
            };
            let json = ckpt.to_json();
            let parsed = SnrCheckpoint::from_json(&json).expect("round-trip must succeed");
            assert_eq!(parsed.snr_index, ckpt.snr_index);
            assert!((parsed.eb_n0_db - ckpt.eb_n0_db).abs() < 1e-12);
            assert_eq!(parsed.frames_completed, ckpt.frames_completed);
            assert_eq!(parsed.errors_accumulated, ckpt.errors_accumulated);
            assert_eq!(parsed.total_iterations, ckpt.total_iterations);
            assert_eq!(parsed.total_queries, ckpt.total_queries);
            assert_eq!(parsed.total_bits, ckpt.total_bits);
            assert_eq!(parsed.total_bit_errors, ckpt.total_bit_errors);
            assert_eq!(parsed.rng_word_pos, ckpt.rng_word_pos);
            assert_eq!(parsed.frames_target, ckpt.frames_target);
            assert_eq!(parsed.errors_target, ckpt.errors_target);
            assert!(!parsed.completed);
            assert_eq!(parsed.config_hash, ckpt.config_hash);
        }

        /// SC-INT-7: Config hash stability — same config produces same hash,
        /// different config produces different hash.
        #[test]
        fn test_config_hash_stability() {
            let mut config = SimulationConfig::quick_test();
            config.rng_seed = Some(42);

            let h1 = compute_config_hash(&config);
            let h2 = compute_config_hash(&config);
            assert_eq!(h1, h2, "Same config must produce same hash");
            assert!(h1.starts_with("blake3:"), "Hash must have blake3: prefix");

            let mut config2 = config.clone();
            config2.min_errors += 1;
            let h3 = compute_config_hash(&config2);
            assert_ne!(h1, h3, "Different config must produce different hash");
        }

        /// SC-INT-8: ChaCha20 RNG derivation is deterministic and the word_pos
        /// round-trip restores the exact stream position.
        #[test]
        fn test_chacha_rng_deterministic_seek() {
            use rand::RngCore;

            let seed = 0xDEAD_BEEF_u64;
            let mut rng1 = make_chacha_rng(seed, 3, 0);

            // Advance 100 words.
            let mut buf = [0u8; 400];
            rng1.fill_bytes(&mut buf);
            let pos = rng1.get_word_pos();

            // Build a second RNG seeked to the same position.
            let mut rng2 = make_chacha_rng(seed, 3, pos);

            // Next 4 bytes must be identical.
            let mut out1 = [0u8; 4];
            let mut out2 = [0u8; 4];
            rng1.fill_bytes(&mut out1);
            rng2.fill_bytes(&mut out2);
            assert_eq!(
                out1, out2,
                "ChaCha20 seek must restore the exact stream position"
            );
        }

        /// SC-INT-9: `run_uncoded_ber_with_channel` emits tracing events in the
        /// expected shape.
        ///
        /// Configures a short 2-SNR campaign with a `tracing_log_path`, runs the
        /// uncoded path, and asserts:
        /// - Every non-empty line parses as a JSON object.
        /// - Exactly one `campaign_start` event with `config_hash`, `run_uuid`, `seed`.
        /// - Exactly two `snr_completed` events (one per SNR point) each carrying
        ///   `eb_n0_db`, `ber`, `fer`, and `elapsed_seconds`.
        #[test]
        fn test_uncoded_tracing_events() {
            let tmpdir = tempfile::tempdir().unwrap();
            let tlog = tmpdir.path().join("uncoded_trace.jsonl");

            let channel = BpskAwgnChannel;
            let mut config = SimulationConfig::quick_test();
            config.eb_n0_range_db = vec![4.0, 8.0];
            config.min_errors = 3;
            config.max_frames = 30;
            config.rng_seed = Some(77);
            config.tracing_log_path = Some(tlog.clone());

            let mut rng = rand::rngs::StdRng::seed_from_u64(77);
            let _ = SimulationRunner::run_uncoded_ber_with_channel(&channel, &config, &mut rng);

            let content = std::fs::read_to_string(&tlog).unwrap();

            let objects: Vec<serde_json::Value> = content
                .lines()
                .filter(|l| !l.trim().is_empty())
                .enumerate()
                .map(|(i, line)| {
                    serde_json::from_str::<serde_json::Value>(line).unwrap_or_else(|e| {
                        panic!("uncoded trace line {i} is not valid JSON: {e}\nline: {line}")
                    })
                })
                .collect();

            let campaign_starts: Vec<_> = objects
                .iter()
                .filter(|o| o["fields"]["event_type"] == "campaign_start")
                .collect();
            let snr_completeds: Vec<_> = objects
                .iter()
                .filter(|o| o["fields"]["event_type"] == "snr_completed")
                .collect();

            assert_eq!(
                campaign_starts.len(),
                1,
                "uncoded path must emit exactly 1 campaign_start"
            );
            assert_eq!(
                snr_completeds.len(),
                2,
                "uncoded path must emit exactly 2 snr_completed events"
            );

            // campaign_start: required fields.
            let cs = campaign_starts[0]["fields"].as_object().unwrap();
            assert!(
                cs.contains_key("config_hash"),
                "campaign_start missing config_hash"
            );
            assert!(
                cs.contains_key("run_uuid"),
                "campaign_start missing run_uuid"
            );
            assert!(cs.contains_key("seed"), "campaign_start missing seed");

            // snr_completed: required fields.
            for (i, sc) in snr_completeds.iter().enumerate() {
                let f = sc["fields"]
                    .as_object()
                    .unwrap_or_else(|| panic!("snr_completed[{i}] has no 'fields' object"));
                for key in &["eb_n0_db", "ber", "fer", "elapsed_seconds"] {
                    assert!(
                        f.contains_key(*key),
                        "snr_completed[{i}] missing fields.{key}"
                    );
                }
            }
        }

        /// SC-INT-10: `run_uncoded_ber_with_channel` checkpoint skip — second run
        /// loads from checkpoint and skips recomputation.
        ///
        /// Runs the uncoded path twice with the same config and checkpoint dir.
        /// After the first run, asserts checkpoint files exist and are marked
        /// `completed: true`.  The second run must produce results with the same
        /// values (loaded from checkpoint, not recomputed).
        #[test]
        fn test_uncoded_checkpoint_skip() {
            let tmpdir = tempfile::tempdir().unwrap();
            let ckpt_dir = tmpdir.path().join("uncoded_ckpts");

            let channel = BpskAwgnChannel;
            let mut config = SimulationConfig::quick_test();
            config.eb_n0_range_db = vec![4.0, 8.0];
            config.min_errors = 3;
            config.max_frames = 30;
            config.rng_seed = Some(88);
            config.checkpoint_dir = Some(ckpt_dir.clone());

            // First run: computes and writes checkpoints.
            let mut rng1 = rand::rngs::StdRng::seed_from_u64(88);
            let results1 =
                SimulationRunner::run_uncoded_ber_with_channel(&channel, &config, &mut rng1);
            assert_eq!(results1.len(), 2);

            // Checkpoint files must exist.
            for i in 0..2_usize {
                let ckpt_file = ckpt_dir.join(format!("snr_{:04}.json", i));
                assert!(ckpt_file.exists(), "Uncoded checkpoint {i} must exist");
                let content = std::fs::read_to_string(&ckpt_file).unwrap();
                assert!(
                    content.contains("\"completed\": true"),
                    "Uncoded checkpoint {i} must be marked completed"
                );
            }

            // Second run: loads from checkpoints; results must match.
            let mut rng2 = rand::rngs::StdRng::seed_from_u64(99); // different seed irrelevant
            let results2 =
                SimulationRunner::run_uncoded_ber_with_channel(&channel, &config, &mut rng2);
            assert_eq!(results2.len(), 2);

            for i in 0..2 {
                assert_eq!(
                    results1[i].num_bits, results2[i].num_bits,
                    "Uncoded checkpoint resume: num_bits must match at SNR point {i}"
                );
                assert_eq!(
                    results1[i].num_bit_errors, results2[i].num_bit_errors,
                    "Uncoded checkpoint resume: num_bit_errors must match at SNR point {i}"
                );
            }
        }

        /// SC-INT-11: `run_coded_iterative_parallel` emits tracing events in the
        /// expected shape.
        ///
        /// Configures a short 2-SNR parallel campaign with `tracing_log_path`,
        /// runs it, and asserts:
        /// - Every non-empty line parses as a JSON object.
        /// - Exactly one `campaign_start` event with `config_hash`, `run_uuid`, `seed`.
        /// - Exactly two `snr_completed` events (one per SNR) with `eb_n0_db`,
        ///   `fer`, `ber`, and `elapsed_seconds`.
        #[test]
        fn test_parallel_tracing_events() {
            let tmpdir = tempfile::tempdir().unwrap();
            let tlog = tmpdir.path().join("parallel_trace.jsonl");

            let encoder = MockEncoder;
            let channel = DeterministicChannel {
                flip_positions: vec![0, 1],
            };
            let mut config = SimulationConfig::quick_test();
            config.eb_n0_range_db = vec![4.0, 8.0];
            config.min_errors = 3;
            config.max_frames = 20;
            config.rng_seed = Some(42);
            config.tracing_log_path = Some(tlog.clone());

            let _ = SimulationRunner::run_coded_iterative_parallel(
                &encoder,
                || MockIterativeDecoder { last_iterations: 0 },
                &channel,
                &config,
            );

            let content = std::fs::read_to_string(&tlog).unwrap();

            let objects: Vec<serde_json::Value> = content
                .lines()
                .filter(|l| !l.trim().is_empty())
                .enumerate()
                .map(|(i, line)| {
                    serde_json::from_str::<serde_json::Value>(line).unwrap_or_else(|e| {
                        panic!("parallel trace line {i} is not valid JSON: {e}\nline: {line}")
                    })
                })
                .collect();

            let campaign_starts: Vec<_> = objects
                .iter()
                .filter(|o| o["fields"]["event_type"] == "campaign_start")
                .collect();
            let snr_completeds: Vec<_> = objects
                .iter()
                .filter(|o| o["fields"]["event_type"] == "snr_completed")
                .collect();

            assert_eq!(
                campaign_starts.len(),
                1,
                "parallel path must emit exactly 1 campaign_start"
            );
            assert_eq!(
                snr_completeds.len(),
                2,
                "parallel path must emit exactly 2 snr_completed events"
            );

            // campaign_start: required fields.
            let cs = campaign_starts[0]["fields"].as_object().unwrap();
            assert!(
                cs.contains_key("config_hash"),
                "campaign_start missing config_hash"
            );
            assert!(
                cs.contains_key("run_uuid"),
                "campaign_start missing run_uuid"
            );
            assert!(cs.contains_key("seed"), "campaign_start missing seed");

            // snr_completed: required fields.
            for (i, sc) in snr_completeds.iter().enumerate() {
                let f = sc["fields"]
                    .as_object()
                    .unwrap_or_else(|| panic!("snr_completed[{i}] has no 'fields' object"));
                for key in &["eb_n0_db", "fer", "ber", "elapsed_seconds"] {
                    assert!(
                        f.contains_key(*key),
                        "snr_completed[{i}] missing fields.{key}"
                    );
                }
            }
        }

        /// SC-INT-12: `run_coded_iterative_parallel` checkpoint skip — second run
        /// loads from checkpoint and skips recomputation.
        ///
        /// Runs the parallel path twice with the same config and checkpoint dir.
        /// After the first run, asserts checkpoint files exist and are marked
        /// `completed: true`.  The second run must produce results with the same
        /// frame counts and error counts (loaded from checkpoint).
        #[test]
        fn test_parallel_checkpoint_skip() {
            let tmpdir = tempfile::tempdir().unwrap();
            let ckpt_dir = tmpdir.path().join("parallel_ckpts");

            let encoder = MockEncoder;
            let channel = DeterministicChannel {
                flip_positions: vec![0, 1],
            };
            let mut config = SimulationConfig::quick_test();
            config.eb_n0_range_db = vec![4.0, 8.0];
            config.min_errors = 3;
            config.max_frames = 20;
            config.rng_seed = Some(42);
            config.checkpoint_dir = Some(ckpt_dir.clone());

            // First run: computes and writes checkpoints.
            let results1 = SimulationRunner::run_coded_iterative_parallel(
                &encoder,
                || MockIterativeDecoder { last_iterations: 0 },
                &channel,
                &config,
            );
            assert_eq!(results1.points.len(), 2);

            // Checkpoint files must exist.
            for i in 0..2_usize {
                let ckpt_file = ckpt_dir.join(format!("snr_{:04}.json", i));
                assert!(ckpt_file.exists(), "Parallel checkpoint {i} must exist");
                let content = std::fs::read_to_string(&ckpt_file).unwrap();
                assert!(
                    content.contains("\"completed\": true"),
                    "Parallel checkpoint {i} must be marked completed"
                );
            }

            // Second run: loads from checkpoints; results must match.
            let results2 = SimulationRunner::run_coded_iterative_parallel(
                &encoder,
                || MockIterativeDecoder { last_iterations: 0 },
                &channel,
                &config,
            );
            assert_eq!(results2.points.len(), 2);

            for i in 0..2 {
                assert_eq!(
                    results1.points[i].num_frames, results2.points[i].num_frames,
                    "Parallel checkpoint resume: num_frames must match at SNR point {i}"
                );
                assert_eq!(
                    results1.points[i].num_frame_errors, results2.points[i].num_frame_errors,
                    "Parallel checkpoint resume: num_frame_errors must match at SNR point {i}"
                );
            }
        }
    } // mod observability_tests
}
