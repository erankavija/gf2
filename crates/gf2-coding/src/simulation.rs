//! Monte Carlo BER/BLER simulation over a [`ChannelModel`]: uncoded sweeps and
//! coded sweeps for [`SoftDecoder`], [`IterativeSoftDecoder`] and closure
//! decoders, run through [`SimulationRunner`], with CSV/JSON export through
//! [`SimulationResults`].

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

/// Serializes every JSONL append (progress and `point_complete`) across
/// parallel simulation workers.
static JSONL_WRITE_LOCK: Mutex<()> = Mutex::new(());
use std::time::{Duration, Instant};

/// Set by the `ctrlc` handler on SIGINT or SIGTERM; the simulation loops poll
/// it and exit after flushing a checkpoint.
#[cfg(feature = "sim-observability")]
static INTERRUPTED: OnceLock<Arc<AtomicBool>> = OnceLock::new();

/// The process-wide interrupt flag; the first call registers the `ctrlc` handler.
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

/// Called at the start of each campaign so an earlier interrupt does not end it.
#[cfg(feature = "sim-observability")]
fn clear_interrupt() {
    interrupted_flag().store(false, Ordering::SeqCst);
}

#[cfg(feature = "sim-observability")]
fn is_interrupted() -> bool {
    interrupted_flag().load(Ordering::SeqCst)
}

/// Per-SNR-point checkpoint stored at `<checkpoint_dir>/snr_<index>.json`.
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
    /// ChaCha20 word position; serialized as a decimal string because `u128`
    /// exceeds JSON's safe integer range (2^53).
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

    /// Returns `None` if a field is missing or fails to parse.
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

/// BLAKE3 hash, as `"blake3:<64 lowercase hex chars>"`, over the fields that
/// determine simulation results: SNR range, stopping criteria, decoder
/// iteration limit and RNG seed. Output and observability paths are excluded,
/// so changing them keeps a checkpoint directory valid.
#[cfg(feature = "sim-observability")]
fn compute_config_hash(config: &SimulationConfig) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&(config.eb_n0_range_db.len() as u64).to_le_bytes());
    for &v in &config.eb_n0_range_db {
        hasher.update(&v.to_le_bytes());
    }
    hasher.update(&(config.min_errors as u64).to_le_bytes());
    hasher.update(&(config.max_frames as u64).to_le_bytes());
    hasher.update(&(config.max_decoder_iterations as u64).to_le_bytes());
    // The tag byte distinguishes `None` from `Some(0)`.
    let seed_tag: u8 = if config.rng_seed.is_some() { 1 } else { 0 };
    hasher.update(&[seed_tag]);
    hasher.update(&config.rng_seed.unwrap_or(0).to_le_bytes());
    let hash = hasher.finalize();
    format!("blake3:{}", hash.to_hex())
}

#[cfg(feature = "sim-observability")]
fn checkpoint_path(dir: &Path, index: usize) -> PathBuf {
    dir.join(format!("snr_{:04}.json", index))
}

#[cfg(feature = "sim-observability")]
fn config_hash_path(dir: &Path) -> PathBuf {
    dir.join("config_hash.txt")
}

/// Writes to a `.tmp` file then renames, so an interrupt leaves no torn checkpoint.
#[cfg(feature = "sim-observability")]
fn write_checkpoint_atomic(path: &Path, ckpt: &SnrCheckpoint) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, ckpt.to_json())?;
    std::fs::rename(&tmp, path)
}

/// Returns `None` if the file does not exist or cannot be parsed.
///
/// # Panics
///
/// Panics if the stored `config_hash` differs from `expected_hash`: the
/// directory holds checkpoints of a different configuration, and skipping
/// them would overwrite valid results.
#[cfg(feature = "sim-observability")]
fn load_checkpoint(path: &Path, expected_hash: &str) -> Option<SnrCheckpoint> {
    let s = std::fs::read_to_string(path).ok()?;
    let ckpt = SnrCheckpoint::from_json(&s)?;
    if ckpt.config_hash != expected_hash {
        panic!(
            "Per-checkpoint config hash mismatch at {}.\n  \
             stored:   {}\n  \
             expected: {}\n  \
             The checkpoint was written by a different campaign config. \
             Delete the checkpoint directory or use a matching config.",
            path.display(),
            ckpt.config_hash,
            expected_hash,
        );
    }
    Some(ckpt)
}

/// Compares `config_hash.txt` in `dir` against `current_hash`, creating the
/// directory and the file when absent. Returns an error message on mismatch.
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

/// Installs a JSON-lines tracing subscriber appending to
/// `config.tracing_log_path` as the thread-local default and returns the guard
/// that restores the previous subscriber. Returns `None` when no path is set
/// or the file cannot be opened.
///
/// The subscriber is thread-local: rayon workers re-enter it through a cloned
/// `Dispatch`.
#[cfg(feature = "sim-observability")]
fn setup_tracing_guard(config: &SimulationConfig) -> Option<tracing::subscriber::DefaultGuard> {
    use tracing_subscriber::{fmt, prelude::*, registry};

    let path = config.tracing_log_path.as_ref()?;

    // Keep one extra dispatcher registered for the lifetime of the process.
    // With at most one registered dispatcher, tracing-core (0.1.36) computes a
    // callsite's cached `Interest` on its first hit from the hitting thread's
    // current dispatch, so a thread without a subscriber can cache
    // `Interest::never` for a shared callsite and drop that event for every
    // other thread. A second, no-op dispatcher makes interest computation fold
    // over the registered list (`never.and(always) == sometimes`).
    static ANTI_JUSTONE_DISPATCH: std::sync::OnceLock<tracing::Dispatch> =
        std::sync::OnceLock::new();
    ANTI_JUSTONE_DISPATCH
        .get_or_init(|| tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default()));
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

/// Per-SNR-point `ChaCha20Rng`: seeded with
/// `base_seed ^ (snr_index as u64).rotate_left(13)` and positioned at
/// `word_pos`, the stream position a checkpoint recorded (0 for a fresh point).
#[cfg(feature = "sim-observability")]
fn make_chacha_rng(base_seed: u64, snr_index: usize, word_pos: u128) -> ChaCha20Rng {
    use rand::SeedableRng as _;
    let seed = base_seed ^ (snr_index as u64).rotate_left(13);
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    rng.set_word_pos(word_pos);
    rng
}

const CSV_HEADER: &str =
    "eb_n0_db,ber,bler,num_bits,num_bit_errors,num_frames,num_frame_errors,avg_iterations,avg_queries_per_bit";

/// Modulation, channel noise and demodulation combined: maps transmitted bits
/// to received LLRs.
pub trait ChannelModel {
    /// Returns one LLR per bit of `bits` after modulation, the channel at
    /// `eb_n0_db` for code rate `rate` (k/n), and demodulation.
    fn transmit_and_demodulate<R: Rng>(
        &self,
        bits: &BitVec,
        eb_n0_db: f64,
        rate: f64,
        rng: &mut R,
    ) -> Vec<Llr>;

    /// Required divisor of `bits.len()` in
    /// [`ChannelModel::transmit_and_demodulate`]; the uncoded runners round
    /// each batch down to a multiple of it. Defaults to `1`.
    fn batch_alignment(&self) -> usize {
        1
    }

    /// The [`crate::modem::DemapMethod`] whose LLRs this channel produces.
    /// [`SimulationRunner::run_uncoded_ber_with_analysis`] requires it to
    /// equal [`crate::modem::AnalysisCapture::demap_method`]. Defaults to
    /// [`crate::modem::DemapMethod::MaxLog`].
    fn demap_method(&self) -> crate::modem::DemapMethod {
        crate::modem::DemapMethod::MaxLog
    }
}

/// BPSK over AWGN: maps bits to +/-1, adds Gaussian noise of the variance set
/// by Eb/N0 and code rate, and returns the LLRs `2r / sigma^2`.
///
/// Noise is drawn once per symbol on the I axis only;
/// [`crate::modem::ModemChannelAdapter`] is the 2-D pipeline for arbitrary
/// constellations.
///
/// # Panics
///
/// [`ChannelModel::transmit_and_demodulate`] panics if `rate` is not in `(0, 1]`.
pub struct BpskAwgnChannel;

/// Process-wide BPSK reference mapper shared by every frame.
fn bpsk_mapper_f64() -> &'static ReferenceMapper<f64> {
    static MAPPER: OnceLock<ReferenceMapper<f64>> = OnceLock::new();
    MAPPER.get_or_init(|| ReferenceMapper::new(ModemSpec::<f64>::bpsk_with_scalar()))
}

/// Process-wide BPSK reference soft demapper; with `noise_var = 2 * sigma^2`
/// its closed form is `LLR = 2 y / sigma^2`.
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
        // Noise is I-axis only (1-D); tests rely on that RNG-stream shape.
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

    /// BPSK exact log-MAP equals the closed form `2r / sigma^2`.
    fn demap_method(&self) -> DemapMethod {
        DemapMethod::ExactLogMap
    }
}

/// Configuration of a Monte Carlo sweep.
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

    /// RNG seed of the coded runners; the uncoded runners draw from the
    /// caller's RNG. When `None`, the sequential runners seed from
    /// `rand::thread_rng()` and the parallel runner uses a fixed default.
    pub rng_seed: Option<u64>,

    /// Result file of the coded runners. A `.json` path is written once at
    /// the end of the sweep; any other path is CSV, appended per completed SNR
    /// point, and its completed rows are reused on a later run.
    pub output_path: Option<PathBuf>,

    /// Directory for per-SNR checkpoint files (`sim-observability` feature).
    ///
    /// When set, the runner writes `<dir>/snr_<index>.json` after each SNR
    /// point and skips completed points on startup. `config_hash.txt` in the
    /// directory records the BLAKE3 hash of this configuration; the runner
    /// panics when the directory cannot be created or a stored hash differs.
    ///
    /// The sequential coded runners write checkpoints only when
    /// [`rng_seed`](Self::rng_seed) is `Some`.
    pub checkpoint_dir: Option<PathBuf>,

    /// Path for JSON-lines tracing output (`sim-observability` feature),
    /// opened in append mode.
    ///
    /// Each campaign writes a `campaign_start` record, an `snr_completed`
    /// record per SNR point, and `heartbeat` records at the cadence of
    /// [`heartbeat_every_frames`](Self::heartbeat_every_frames).
    pub tracing_log_path: Option<PathBuf>,

    /// Within-SNR heartbeat cadence in frames (`sim-observability` feature).
    ///
    /// When `Some(n)`, the sequential coded runners with a set
    /// [`rng_seed`](Self::rng_seed) and a set
    /// [`tracing_log_path`](Self::tracing_log_path) or
    /// [`checkpoint_dir`](Self::checkpoint_dir) emit a `heartbeat` tracing
    /// event every `n` frames and, with `checkpoint_dir`, write an intermediate
    /// checkpoint holding the RNG word position, from which an interrupted SNR
    /// point resumes. The uncoded and parallel runners ignore it.
    pub heartbeat_every_frames: Option<usize>,
}

impl SimulationConfig {
    /// A small sweep for quick tests.
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

    /// A dense sweep with tight stopping criteria for BER curves.
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

    /// Seeds from `rng_seed`, or from `thread_rng()` when it is `None`.
    fn make_rng(&self) -> StdRng {
        match self.rng_seed {
            Some(seed) => StdRng::seed_from_u64(seed),
            None => StdRng::seed_from_u64(rand::thread_rng().gen()),
        }
    }
}

/// Results at one Eb/N0 point.
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
    pub fn is_complete(&self, min_errors: usize) -> bool {
        self.num_frame_errors >= min_errors
    }

    /// Exports result as a CSV row.
    ///
    /// Format: `eb_n0_db,ber,bler,num_bits,num_bit_errors,num_frames,num_frame_errors,avg_iterations,avg_queries_per_bit`
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

    /// Parses a row produced by [`to_csv_row`](Self::to_csv_row); the two
    /// trailing optional columns may be absent. Returns `None` if the row
    /// cannot be parsed.
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

    /// Appends this result as one CSV row to `path`, writing the header first
    /// when the file is missing or empty.
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

/// Per-SNR-point results of a sweep, with CSV and JSON export.
#[derive(Debug, Clone)]
pub struct SimulationResults {
    /// One result per SNR point, in the order of [`SimulationConfig::eb_n0_range_db`].
    pub points: Vec<SimulationResult>,
}

impl SimulationResults {
    /// CSV with one row per point; `include_header` prepends the header row.
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

    /// JSON array with one object per SNR point.
    pub fn to_json(&self) -> String {
        let entries: Vec<String> = self.points.iter().map(|p| p.to_json()).collect();
        format!("[{}]", entries.join(","))
    }

    /// Writes JSON when `path` ends in `.json`, otherwise CSV with a header.
    ///
    /// # Panics
    ///
    /// Panics if the file cannot be created or written.
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

/// The uncoded-BER Monte Carlo loop behind
/// [`SimulationRunner::run_uncoded_ber_with_channel`] and
/// [`SimulationRunner::run_uncoded_ber_with_analysis`].
#[inline]
fn run_uncoded_ber_with_channel_impl<C: ChannelModel, R: Rng>(
    channel: &C,
    config: &SimulationConfig,
    mut capture: Option<&mut AnalysisCapture<'_>>,
    rng: &mut R,
) -> Vec<SimulationResult> {
    #[cfg(feature = "sim-observability")]
    let _tracing_guard = setup_tracing_guard(config);

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

    // Checkpoints are written at SNR-point boundaries only.
    #[cfg(feature = "sim-observability")]
    let config_hash = compute_config_hash(config);
    #[cfg(feature = "sim-observability")]
    if let Some(ref ckpt_dir) = config.checkpoint_dir {
        if let Err(e) = validate_checkpoint_dir(ckpt_dir, &config_hash) {
            panic!("{e}");
        }
    }

    #[cfg(feature = "sim-observability")]
    clear_interrupt();

    /// A multiple of every common `bits_per_symbol` (1, 2, 3, 4, 5, 6, 8, 10,
    /// 12, 15, 16).
    const UNCODED_MODEM_BATCH_BITS: usize = 960;

    let alignment = channel.batch_alignment().max(1);

    // `PerBitLlrStats::accumulate` checks lengths only, so a capture whose
    // `bits_per_symbol` differs from the channel alignment would accumulate
    // meaningless per-position statistics unnoticed.
    if let Some(cap) = capture.as_deref() {
        assert_eq!(
            cap.bits_per_symbol() as usize,
            alignment,
            "AnalysisCapture bits_per_symbol ({}) must equal channel.batch_alignment() \
             ({}) — a mismatched capture would silently collapse per-position statistics",
            cap.bits_per_symbol(),
            alignment,
        );
        // Per-bit MI / GMI semantics differ between exact log-MAP and max-log
        // (see `crate::modem::analysis::gmi_bits`), so one accumulator must not
        // mix methods.
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

    // Allocated only on the analysis-enabled path and reused across batches.
    let mut truth_scratch: Option<Vec<bool>> = if capture.is_some() {
        Some(Vec::with_capacity(UNCODED_MODEM_BATCH_BITS))
    } else {
        None
    };

    let mut results = Vec::with_capacity(config.eb_n0_range_db.len());
    for (snr_idx, &eb_n0_db) in config.eb_n0_range_db.iter().enumerate() {
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

        #[cfg(feature = "sim-observability")]
        let _snr_guard = tracing::info_span!(
            "snr_point",
            eb_n0_db = eb_n0_db,
            es_n0_db = eb_n0_db, // uncoded: rate=1, so es_n0_db == eb_n0_db
            frames_target = config.max_frames,
            errors_target = config.min_errors,
        )
        .entered();

        #[cfg(not(feature = "sim-observability"))]
        let _ = snr_idx;

        #[cfg_attr(not(feature = "sim-observability"), allow(unused_variables))]
        let point_start = Instant::now();
        let mut total_bits = 0usize;
        let mut total_errors = 0usize;

        while total_errors < config.min_errors && total_bits < config.max_frames {
            let remaining = config.max_frames - total_bits;
            let mut batch_size = UNCODED_MODEM_BATCH_BITS.min(remaining);
            batch_size -= batch_size % alignment;
            if batch_size == 0 {
                break;
            }
            let bits = BitVec::random(batch_size, rng);
            // Uncoded => rate = 1.0.
            let llrs = channel.transmit_and_demodulate(&bits, eb_n0_db, 1.0, rng);
            debug_assert_eq!(llrs.len(), batch_size);

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

        // Checked after the SNR-boundary checkpoint is on disk.
        #[cfg(feature = "sim-observability")]
        if is_interrupted() {
            tracing::info!(
                name: "campaign_interrupted",
                event_type = "campaign_interrupted",
                last_snr_index = snr_idx,
                last_eb_n0_db = eb_n0_db,
            );
            eprintln!(
                "Interrupted after uncoded SNR point {} ({:.1} dB). \
                 Resume by re-running with the same config.",
                snr_idx, eb_n0_db
            );
            std::process::exit(1);
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

/// Entry points for uncoded and coded Monte Carlo sweeps.
pub struct SimulationRunner;

impl SimulationRunner {
    /// Simulates uncoded BPSK over AWGN; equals
    /// [`SimulationRunner::run_uncoded_ber_with_channel`] with [`BpskAwgnChannel`].
    ///
    /// # Panics
    ///
    /// As [`SimulationRunner::run_uncoded_ber_with_channel`].
    pub fn run_uncoded_ber<R: Rng>(
        config: &SimulationConfig,
        rng: &mut R,
    ) -> Vec<SimulationResult> {
        Self::run_uncoded_ber_with_channel(&BpskAwgnChannel, config, rng)
    }

    /// Uncoded BER sweep through any [`ChannelModel`].
    ///
    /// Hard decision on the returned LLRs: positive LLR => bit 0, negative =>
    /// bit 1, a tie => bit 0. The channel is called with `rate = 1.0`, and
    /// `rng` feeds both bit generation and channel noise.
    ///
    /// Each SNR point stops at `config.min_errors` bit errors or
    /// `config.max_frames` transmitted bits; `max_decoder_iterations` and
    /// `heartbeat_every_frames` are unused. With `sim-observability`, a
    /// checkpoint is written per completed SNR point, completed points are
    /// skipped on resume, and SIGINT or SIGTERM exits the process at the next
    /// SNR boundary.
    ///
    /// Returns one [`SimulationResult`] per entry of `config.eb_n0_range_db`
    /// with `num_bits` and `num_bit_errors` populated; the frame counts stay 0.
    ///
    /// # Panics
    ///
    /// Panics under the conditions stated at [`SimulationConfig::checkpoint_dir`].
    ///
    /// # Complexity
    ///
    /// O(SNR_points * max_frames * channel_time).
    pub fn run_uncoded_ber_with_channel<C: ChannelModel, R: Rng>(
        channel: &C,
        config: &SimulationConfig,
        rng: &mut R,
    ) -> Vec<SimulationResult> {
        run_uncoded_ber_with_channel_impl::<C, R>(channel, config, None, rng)
    }

    /// [`SimulationRunner::run_uncoded_ber_with_channel`] with per-bit LLR
    /// analysis capture: when `capture` is `Some`, each post-demap
    /// `(llrs, truth_bits)` batch is forwarded to it before errors are counted.
    ///
    /// One capture accumulates over every entry of `config.eb_n0_range_db`;
    /// for per-SNR statistics run one SNR point per call with a fresh
    /// [`crate::modem::analysis::PerBitLlrStats`].
    ///
    /// # Panics
    ///
    /// Panics before the first batch if `capture` is `Some` and its
    /// `bits_per_symbol()` differs from `channel.batch_alignment()` or its
    /// `demap_method()` differs from `channel.demap_method()`, and under the
    /// conditions of [`SimulationRunner::run_uncoded_ber_with_channel`].
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
    /// That of [`SimulationRunner::run_uncoded_ber_with_channel`] plus O(bits)
    /// accumulator work when capturing.
    pub fn run_uncoded_ber_with_analysis<C: ChannelModel, R: Rng>(
        channel: &C,
        config: &SimulationConfig,
        capture: Option<&mut AnalysisCapture<'_>>,
        rng: &mut R,
    ) -> Vec<SimulationResult> {
        run_uncoded_ber_with_channel_impl::<C, R>(channel, config, capture, rng)
    }

    /// CSV for a slice of results; `include_header` prepends the header row.
    pub fn results_to_csv(results: &[SimulationResult], include_header: bool) -> String {
        let wrapper = SimulationResults {
            points: results.to_vec(),
        };
        wrapper.to_csv(include_header)
    }
}

/// Frames between stderr progress reports.
const PROGRESS_INTERVAL: usize = 1000;

/// Replaces the extension of `csv_path` with `progress.jsonl`.
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

/// Reports progress to stderr with elapsed time and an ETA for the current
/// SNR point: none at 0 errors, flagged "rough" at 1-5 errors.
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

/// Estimates the remaining sweep time: BLER at each remaining point from a
/// log-linear fit over completed points with errors, frames as
/// `min_errors / bler` capped at `max_frames`, duration from the frame rate of
/// the nearest completed point. Returns `None` with fewer than 2 such points.
fn estimate_sweep_eta(
    completed: &[CompletedPointInfo],
    remaining_snr_points: &[f64],
    min_errors: usize,
    max_frames: usize,
) -> Option<std::time::Duration> {
    if remaining_snr_points.is_empty() {
        return None;
    }

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

    let mut total_eta_secs = 0.0f64;
    for &snr in remaining_snr_points {
        let ln_bler_est = slope * snr + intercept;
        let bler_est = ln_bler_est.exp().clamp(1e-12, 1.0);

        let frames_needed = if min_errors > 0 {
            ((min_errors as f64 / bler_est).ceil() as usize).min(max_frames)
        } else {
            max_frames
        };

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

/// Reports a completed point to stderr with elapsed time and the sweep ETA.
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
        .take(4)
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

/// Loads completed results from a CSV file for resuming, keyed by Eb/N0
/// formatted to 6 decimal places. Rows with fewer than `min_errors` frame
/// errors are dropped; an unreadable file gives an empty map.
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

    /// Appends a JSONL progress entry under [`JSONL_WRITE_LOCK`]; a write
    /// failure warns on stderr once.
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

    fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

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

/// The one `point_complete` JSONL schema, shared by the sequential and parallel paths.
fn append_point_complete_jsonl(
    path: &Path,
    result: &SimulationResult,
    elapsed: Duration,
) -> std::io::Result<()> {
    use std::io::Write;
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

/// Counts differing bits over the common prefix plus the length difference of
/// the two vectors.
pub fn count_bit_errors(original: &BitVec, decoded: &BitVec) -> usize {
    if original.len() == decoded.len() {
        let mut diff = original.clone();
        diff.bit_xor_into(decoded);
        diff.count_ones()
    } else {
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

/// Parameters of one SNR point shared by the simulation entry points.
struct SnrPointContext<'a> {
    eb_n0_db: f64,
    rate: f64,
    config: &'a SimulationConfig,
    existing: &'a HashMap<String, SimulationResult>,
    output_path: Option<&'a Path>,
    progress_path: Option<&'a Path>,
    remaining_snr_points: &'a [f64],
    completed_points: &'a [CompletedPointInfo],
    /// Suppresses completion reporting and CSV/JSONL writes; the parallel
    /// runner's `ParallelResultCollector` performs them.
    suppress_completion_side_effects: bool,
}

/// Simulates one SNR point: the frame loop, stopping rule, CSV-resume check,
/// progress reporting and CSV/JSONL output. `decode_frame` maps one frame's
/// channel LLRs to a [`DecoderResult`].
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
        if let Some(pp) = ctx.progress_path {
            let mut acc_for_jsonl = SnrAccumulator::new(eb_n0_db, k);
            // Reuse the start time from the original accumulator via elapsed.
            acc_for_jsonl.start_time = Instant::now() - point_elapsed;
            acc_for_jsonl.write_point_complete_entry(pp, &sim_result);
        }

        // JSON output is written once at the end of the sweep.
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

/// Collects results of parallel SNR-point workers; its `Mutex` serializes
/// result storage, CSV append, the `point_complete` JSONL entry and the stderr
/// completion report.
#[derive(Debug)]
struct ParallelResultCollector {
    /// Results indexed by SNR point index; `None` until completed.
    results: Vec<Option<SimulationResult>>,
    /// Path for CSV output (if configured and not JSON).
    output_path: Option<PathBuf>,
    progress_path: Option<PathBuf>,
    completed_count: usize,
    completed_points: Vec<CompletedPointInfo>,
    all_snr_points: Vec<f64>,
    min_errors: usize,
    max_frames: usize,
    /// Set to `true` after the first JSONL write failure so we only warn once.
    progress_write_warned: bool,
}

impl ParallelResultCollector {
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

    /// Stores the result and performs the completion I/O; called under the collector's `Mutex`.
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

        let completed_snrs: Vec<f64> = self.completed_points.iter().map(|p| p.eb_n0_db).collect();
        let remaining_snr: Vec<f64> = self
            .all_snr_points
            .iter()
            .filter(|snr| !completed_snrs.iter().any(|c| (c - **snr).abs() < 1e-9))
            .copied()
            .collect();

        if let Some(ref path) = self.output_path {
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                result.append_csv_row_to(path);
            }
        }

        if let Some(pp) = self.progress_path.clone() {
            self.write_point_complete_entry(&pp, &result, point_elapsed);
        }

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

/// Sequential SNR sweep behind `run_coded`, `run_coded_iterative` and
/// `run_with_decoder`: CSV resume, progress, incremental CSV, JSONL logging
/// and the final file write around a per-frame `decode_frame` closure.
///
/// With `sim-observability`, a set [`SimulationConfig::rng_seed`] and a set
/// `checkpoint_dir` or `tracing_log_path`, each point runs through
/// `simulate_single_point_observable`.
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

    #[cfg(feature = "sim-observability")]
    let config_hash = compute_config_hash(config);
    #[cfg(feature = "sim-observability")]
    if let Some(ref ckpt_dir) = config.checkpoint_dir {
        if let Err(e) = validate_checkpoint_dir(ckpt_dir, &config_hash) {
            panic!("{e}");
        }
    }

    #[cfg(feature = "sim-observability")]
    clear_interrupt();

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

        // A completed checkpoint takes precedence over CSV-based resume.
        #[cfg(feature = "sim-observability")]
        let ckpt_resume: Option<SnrCheckpoint> = config
            .checkpoint_dir
            .as_ref()
            .and_then(|dir| load_checkpoint(&checkpoint_path(dir, point_idx), &config_hash));

        #[cfg(feature = "sim-observability")]
        if let Some(ref ckpt) = ckpt_resume {
            if ckpt.completed {
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

        // The per-SNR span gives heartbeat and snr_completed events their SNR fields.
        #[cfg(feature = "sim-observability")]
        let _snr_span_guard = {
            let es_n0_db = crate::info_theory::ebn0_to_esn0(eb_n0_db, 1, k as f64 / n as f64);
            tracing::info_span!(
                "snr_point",
                eb_n0_db = eb_n0_db,
                es_n0_db = es_n0_db,
                frames_target = config.max_frames,
                errors_target = config.min_errors,
            )
            .entered()
        };

        // Checkpoint and heartbeat resume need a seeded, seekable per-SNR ChaCha20Rng.
        #[cfg(feature = "sim-observability")]
        let sim_result = if config.checkpoint_dir.is_some() || config.tracing_log_path.is_some() {
            if let Some(base_seed) = config.rng_seed {
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
                // No seed: the plain path, without checkpoints or heartbeats.
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
    if let Some(ref path) = config.output_path {
        results.write_to(path);
    }
    results
}

/// `simulate_single_point` with checkpoints, heartbeats, tracing events and
/// interrupt handling.
///
/// Takes a `ChaCha20Rng` so each checkpoint stores its word position, from
/// which a resumed run continues the same stream. The `resume_*` parameters
/// carry the totals of a partial checkpoint.
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

    // CSV-based resume applies only when no partial checkpoint exists.
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
    acc.total_frames = resume_frames;
    acc.total_frame_errors = resume_errors;
    acc.total_iterations = resume_iters;
    acc.total_queries = resume_queries;
    acc.total_bits = resume_bits;
    acc.total_bit_errors = resume_bit_errors;

    while !acc.should_stop(config.min_errors, config.max_frames) {
        if is_interrupted() {
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

        if let Some(pp) = progress_path {
            if acc.should_write_progress() {
                acc.write_progress_entry(pp);
            }
        }

        if let Some(every) = config.heartbeat_every_frames {
            if acc.should_heartbeat(every) {
                let word_pos = rng.get_word_pos();
                let elapsed_s = acc.elapsed().as_secs_f64();

                tracing::info!(
                    name: "heartbeat",
                    event_type = "heartbeat",
                    snr_index = snr_index,
                    eb_n0_db = eb_n0_db,
                    frames_completed = acc.total_frames,
                    errors_so_far = acc.total_frame_errors,
                    elapsed_seconds = elapsed_s,
                );

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

    if let Some(pp) = progress_path {
        if let Err(e) = append_point_complete_jsonl(pp, &sim_result, point_elapsed) {
            eprintln!("Warning: failed to write JSONL progress: {e}");
        }
    }

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
    /// Coded sweep with an immutable [`SoftDecoder`]: per SNR point, frames
    /// are encoded, sent through `channel` and decoded until
    /// `config.min_errors` frame errors or `config.max_frames` frames.
    ///
    /// # Panics
    ///
    /// Panics if `output_path` is set and the file cannot be written, and under
    /// the conditions stated at [`SimulationConfig::checkpoint_dir`].
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

    /// [`run_coded`](Self::run_coded) for an [`IterativeSoftDecoder`], which
    /// is reset before each frame and run for at most
    /// `config.max_decoder_iterations` iterations.
    ///
    /// # Panics
    ///
    /// As [`run_coded`](Self::run_coded).
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

    /// Coded iterative sweep with one worker per SNR point: rayon threads with
    /// the `parallel` feature, sequential otherwise. `make_decoder` is called
    /// once per SNR point.
    ///
    /// With `sim-observability`, a checkpoint is written per completed SNR
    /// point and completed points are skipped on resume;
    /// `heartbeat_every_frames` is unused, since the per-worker
    /// [`rand::rngs::StdRng`] has no stream seek
    /// ([`SimulationRunner::run_coded_iterative`] resumes within a point). On
    /// SIGINT or SIGTERM, workers that have not started skip their point and
    /// the process exits non-zero once the running ones finish.
    ///
    /// Results are in the order of `config.eb_n0_range_db`.
    ///
    /// # Panics
    ///
    /// As [`run_coded`](Self::run_coded).
    ///
    /// # Complexity
    ///
    /// O(SNR_points * max_frames * (encode_time + channel_time + decode_time))
    /// work.
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

        #[cfg(feature = "sim-observability")]
        let _tracing_guard = setup_tracing_guard(config);

        // Checkpoints are written at SNR-point boundaries only, after a worker
        // completes its point.
        #[cfg(feature = "sim-observability")]
        let config_hash = compute_config_hash(config);
        #[cfg(feature = "sim-observability")]
        if let Some(ref ckpt_dir) = config.checkpoint_dir {
            if let Err(e) = validate_checkpoint_dir(ckpt_dir, &config_hash) {
                panic!("{e}");
            }
        }

        #[cfg(feature = "sim-observability")]
        clear_interrupt();

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

        // Rayon workers re-enter the current thread-local Dispatch.
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

        let csv_output = config.output_path.as_ref().and_then(|p| {
            if p.extension().and_then(|e| e.to_str()) == Some("json") {
                None
            } else {
                Some(p.clone())
            }
        });
        let progress_path = config.output_path.as_ref().map(|p| progress_path_for(p));
        let worker_progress_path = progress_path.clone();

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

        // Checkpointed points enter the collector so the results cover every SNR point.
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

        let simulate_and_record = |(idx, eb_n0_db): (usize, f64)| {
            // Workers that have not started skip their point; the flag is
            // checked again after all workers return.
            #[cfg(feature = "sim-observability")]
            if is_interrupted() {
                return;
            }

            // `set_default` is thread-local; skipped without a JSON subscriber.
            #[cfg(feature = "sim-observability")]
            let _dispatch_guard = (!worker_dispatch.is::<tracing::subscriber::NoSubscriber>())
                .then(|| tracing::dispatcher::set_default(&worker_dispatch));

            #[cfg(feature = "sim-observability")]
            let _snr_guard = tracing::info_span!(
                "snr_point",
                eb_n0_db = eb_n0_db,
                es_n0_db = crate::info_theory::ebn0_to_esn0(eb_n0_db, 1, k as f64 / n as f64),
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
                // Entries carry eb_n0_db so readers can demultiplex concurrent
                // SNR points.
                progress_path: worker_progress_path.as_deref(),
                remaining_snr_points: &[],
                completed_points: &[],
                suppress_completion_side_effects: true,
            };

            let point_start = Instant::now();
            let result = simulate_single_point(encoder, channel, &mut rng, &ctx, |llrs| {
                decoder.reset();
                decoder.decode_iterative(llrs, max_iter)
            });
            let point_elapsed = point_start.elapsed();

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

        #[cfg(feature = "sim-observability")]
        if is_interrupted() {
            eprintln!(
                "Interrupted during parallel sweep — exiting. \
                 Resume by re-running with the same config."
            );
            std::process::exit(1);
        }

        let points = Arc::try_unwrap(collector)
            .expect("ParallelResultCollector Arc has outstanding references")
            .into_inner()
            .expect("ParallelResultCollector Mutex poisoned")
            .into_results();

        let results = SimulationResults { points };
        // Final overwrite with a complete file; JSON has no incremental form.
        if let Some(ref path) = config.output_path {
            results.write_to(path);
        }
        results
    }

    /// [`run_coded`](Self::run_coded) with a decode closure, for decoders
    /// outside the decoder traits (e.g.
    /// [`TurboDecoder`](crate::product::TurboDecoder)).
    ///
    /// # Panics
    ///
    /// As [`run_coded`](Self::run_coded).
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
}

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

    /// Encodes [m0, m1] -> [m0, m0, m1, m1].
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

    /// Decides each message bit from the LLR sum of its repeated pair.
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
            for pair in 0..2 {
                let combined = llrs[2 * pair].value() + llrs[2 * pair + 1].value();
                result.push_bit(combined < 0.0);
            }
            result
        }
    }

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

    /// Returns LLR +10 for bit 0 and -10 for bit 1, sign-flipped at `flip_positions`.
    struct DeterministicChannel {
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
                    let correct_llr = if bits.get(i) { -10.0 } else { 10.0 };
                    if self.flip_positions.contains(&i) {
                        Llr::new(-correct_llr)
                    } else {
                        Llr::new(correct_llr)
                    }
                })
                .collect()
        }
    }

    #[test]
    fn test_hand_calculated_deterministic_ber() {
        // Flipping positions 0 and 1 (both copies of m0) makes every frame
        // decode m0 wrong and m1 right: 1 bit error per k=2 message bits.
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

        let expected_ber = 10.0 / 20.0;
        assert!(
            (point.ber - expected_ber).abs() < 1e-10,
            "BER must be exactly 0.5, got {}",
            point.ber
        );

        let expected_bler = 10.0 / 10.0;
        assert!(
            (point.bler - expected_bler).abs() < 1e-10,
            "BLER must be exactly 1.0, got {}",
            point.bler
        );
    }

    #[test]
    fn test_deterministic_no_errors() {
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

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert!(lines[0].contains("eb_n0_db"), "Header must be present");
        assert_eq!(lines.len(), 3, "CSV must have header + 2 data rows");
        assert_eq!(results.points.len(), 2);
    }

    #[test]
    fn test_resume_skips_completed_points() {
        let tmpdir = tempfile::tempdir().unwrap();
        let dir = tmpdir.path();
        let path = dir.join("resume.csv");

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
        assert_eq!(results.points[0].num_frame_errors, 50);
        assert!((results.points[0].eb_n0_db - 8.0).abs() < 1e-10);
        assert!(results.points[1].num_frames > 0);
        assert!((results.points[1].eb_n0_db - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_run_with_decoder_produces_same_results() {
        let encoder = MockEncoder;
        let channel = BpskAwgnChannel;
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![6.0];
        config.min_errors = 5;
        config.max_frames = 500;
        config.rng_seed = Some(42);

        let mut decoder = MockIterativeDecoder { last_iterations: 0 };
        let results_trait =
            SimulationRunner::run_coded_iterative(&encoder, &mut decoder, &channel, &config);

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

        // The wall-clock threshold in should_write_progress keeps short runs
        // from writing entries, so they are written through the accumulator.
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

        acc.write_progress_entry(&jsonl_path);
        let sim_result = acc.into_result();
        let mut acc2 = SnrAccumulator::new(5.0, k);
        acc2.write_point_complete_entry(&jsonl_path, &sim_result);

        assert!(jsonl_path.exists(), "JSONL progress file must exist");
        let content = std::fs::read_to_string(&jsonl_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert!(
            lines.len() >= 2,
            "JSONL must have at least 2 lines (progress + point_complete), got {}",
            lines.len()
        );

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

        for &snr in &[6.0_f64, 8.0, 10.0] {
            let snr_str = format!("\"eb_n0_db\":{}", snr);
            assert!(
                jsonl_lines.iter().any(|l| l.contains(&snr_str)),
                "JSONL must have point_complete for {snr} dB"
            );
        }

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

    #[test]
    fn test_run_uncoded_ber_with_channel_matches_legacy_bpsk() {
        let mut config = SimulationConfig::quick_test();
        config.eb_n0_range_db = vec![5.0];
        config.min_errors = 200;
        config.max_frames = 200_000;
        config.rng_seed = Some(0xC0FFEE_u64);

        let mut rng_legacy = StdRng::seed_from_u64(config.rng_seed.unwrap());
        let legacy = SimulationRunner::run_uncoded_ber(&config, &mut rng_legacy);

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
        let ratio = ber_modem / ber_legacy;
        assert!(
            ratio > 0.6 && ratio < 1.4,
            "modem BER should track legacy BER within ~40% (got ratio={ratio}: legacy={ber_legacy}, modem={ber_modem})",
        );
    }

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

    /// `QpskRicianChannelModel::transmit_and_demodulate` asserts an even
    /// codeword length.
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
        let results = SimulationRunner::run_uncoded_ber_with_channel(&channel, &config, &mut rng);
        assert_eq!(results.len(), 1);
        let r = &results[0];
        assert!(r.ber.is_finite());
        assert!(
            r.num_bits.is_multiple_of(2),
            "transmitted bits must stay aligned to the QPSK fading channel's 2-bit requirement"
        );
    }

    /// `ModemChannelAdapter::transmit_and_demodulate` requires
    /// `bits.len() % bits_per_symbol == 0`.
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
        let results = SimulationRunner::run_uncoded_ber_with_channel(&adapter, &config, &mut rng);
        assert_eq!(results.len(), 1);
        let r = &results[0];
        assert!(r.ber.is_finite());
        assert!(
            r.num_bits.is_multiple_of(2),
            "transmitted bits must stay aligned"
        );
    }

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

    #[cfg(feature = "sim-observability")]
    mod observability_tests {
        use super::*;

        /// Serializes the tests in this module that install a JSON subscriber:
        /// each registration rebuilds tracing-core's global callsite-interest
        /// cache, and this keeps those rebuilds out of another such test's
        /// emit window. Poisoning is tolerated so a panicking test does not
        /// cascade.
        static TRACING_SUBSCRIBER_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

            let decoder = MockSoftDecoder;
            let results1 = SimulationRunner::run_coded(&encoder, &decoder, &channel, &config);
            assert_eq!(results1.points.len(), 3);

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

            let decoder2 = MockSoftDecoder;
            let results2 = SimulationRunner::run_coded(&encoder, &decoder2, &channel, &config);
            assert_eq!(results2.points.len(), 3);

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

            let decoder = MockSoftDecoder;
            let _ = SimulationRunner::run_coded(&encoder, &decoder, &channel, &config);

            // `min_errors` is part of the config hash.
            let mut config2 = config.clone();
            config2.min_errors = 99;

            let decoder2 = MockSoftDecoder;
            let _ = SimulationRunner::run_coded(&encoder, &decoder2, &channel, &config2);
        }

        #[test]
        #[should_panic(expected = "Per-checkpoint config hash mismatch")]
        fn test_per_file_hash_mismatch_aborts() {
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

            let decoder = MockSoftDecoder;
            let _ = SimulationRunner::run_coded(&encoder, &decoder, &channel, &config);

            let ckpt_file = ckpt_dir.join("snr_0000.json");
            assert!(
                ckpt_file.exists(),
                "checkpoint must be written after first run"
            );

            let content = std::fs::read_to_string(&ckpt_file).unwrap();
            let tampered = content.replace("blake3:", "blake3:aaaa_tampered_");
            assert_ne!(content, tampered, "tampering must change the file content");
            std::fs::write(&ckpt_file, tampered).unwrap();

            let decoder2 = MockSoftDecoder;
            let _ = SimulationRunner::run_coded(&encoder, &decoder2, &channel, &config);
        }

        /// `DeterministicChannel` gives one frame error per frame, so
        /// `min_errors = 5` runs exactly 5 frames and cadence 2 fires after
        /// frames 2 and 4.
        #[test]
        fn test_heartbeat_cadence() {
            let _serial = TRACING_SUBSCRIBER_GUARD
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
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

        #[test]
        fn test_tracing_log_valid_jsonl() {
            let _serial = TRACING_SUBSCRIBER_GUARD
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
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

            // The tracing-subscriber JSON formatter omits the callsite name;
            // the explicit `event_type` field tells events apart.
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

        #[test]
        #[ignore = "slow: subprocess SIGINT timing may exceed 5 s on slow hosts"]
        fn test_resume_after_interrupt() {
            use std::process::Command;
            use std::time::{Duration, Instant};

            // cargo places the helper in target/<profile>/, one level above deps/.
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

            let mut child = Command::new(&helper_bin)
                .arg(&resume_dir)
                .spawn()
                .unwrap_or_else(|e| panic!("failed to spawn helper: {e}"));

            let pid = child.id();

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

            let kill_status = Command::new("kill")
                .args(["-INT", &pid.to_string()])
                .status()
                .unwrap_or_else(|e| panic!("failed to send SIGINT: {e}"));
            assert!(
                kill_status.success(),
                "kill -INT returned non-zero: {kill_status}"
            );

            let interrupted_status = child.wait().expect("failed to wait for interrupted helper");
            assert_eq!(
                interrupted_status.code(),
                Some(1),
                "interrupted helper must exit with code 1, got: {interrupted_status}"
            );

            assert!(
                ckpt_file.exists(),
                "partial checkpoint must persist after SIGINT"
            );

            let resume_status = Command::new(&helper_bin)
                .arg(&resume_dir)
                .status()
                .unwrap_or_else(|e| panic!("failed to spawn resumed helper: {e}"));
            assert!(
                resume_status.success(),
                "resumed run must exit with code 0, got: {resume_status}"
            );

            let resume_csv = std::fs::read(resume_dir.join("results.csv"))
                .expect("resume results.csv must exist after resumed run");
            assert_eq!(
                ref_csv, resume_csv,
                "resumed results.csv must be byte-identical to the reference run"
            );
        }

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

        #[test]
        fn test_chacha_rng_deterministic_seek() {
            use rand::RngCore;

            let seed = 0xDEAD_BEEF_u64;
            let mut rng1 = make_chacha_rng(seed, 3, 0);

            let mut buf = [0u8; 400];
            rng1.fill_bytes(&mut buf);
            let pos = rng1.get_word_pos();

            let mut rng2 = make_chacha_rng(seed, 3, pos);

            let mut out1 = [0u8; 4];
            let mut out2 = [0u8; 4];
            rng1.fill_bytes(&mut out1);
            rng2.fill_bytes(&mut out2);
            assert_eq!(
                out1, out2,
                "ChaCha20 seek must restore the exact stream position"
            );
        }

        #[test]
        fn test_uncoded_tracing_events() {
            let _serial = TRACING_SUBSCRIBER_GUARD
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
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

            let mut rng1 = rand::rngs::StdRng::seed_from_u64(88);
            let results1 =
                SimulationRunner::run_uncoded_ber_with_channel(&channel, &config, &mut rng1);
            assert_eq!(results1.len(), 2);

            for i in 0..2_usize {
                let ckpt_file = ckpt_dir.join(format!("snr_{:04}.json", i));
                assert!(ckpt_file.exists(), "Uncoded checkpoint {i} must exist");
                let content = std::fs::read_to_string(&ckpt_file).unwrap();
                assert!(
                    content.contains("\"completed\": true"),
                    "Uncoded checkpoint {i} must be marked completed"
                );
            }

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

        #[test]
        fn test_parallel_tracing_events() {
            let _serial = TRACING_SUBSCRIBER_GUARD
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
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

            let results1 = SimulationRunner::run_coded_iterative_parallel(
                &encoder,
                || MockIterativeDecoder { last_iterations: 0 },
                &channel,
                &config,
            );
            assert_eq!(results1.points.len(), 2);

            for i in 0..2_usize {
                let ckpt_file = ckpt_dir.join(format!("snr_{:04}.json", i));
                assert!(ckpt_file.exists(), "Parallel checkpoint {i} must exist");
                let content = std::fs::read_to_string(&ckpt_file).unwrap();
                assert!(
                    content.contains("\"completed\": true"),
                    "Parallel checkpoint {i} must be marked completed"
                );
            }

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
    }
}
