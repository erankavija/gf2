//! SNR-point checkpoint payload and resumable sweep execution over the
//! persistence mechanism of [`crate::checkpoint`]:
//! [`run_snr_point_checkpointed`] and [`run_sweep_checkpointed`] run frames in
//! heartbeat-sized chunks over [`run_snr_point_range`] and flush a checkpoint
//! after each chunk.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::checkpoint::{
    CheckpointLoadError, CheckpointPayload, CheckpointReader as GenericCheckpointReader,
    CheckpointWriter as GenericCheckpointWriter, ConfigHashProvider,
};
use crate::config::PipelineConfig;
use crate::error::{BuildError, FatalError, StageError};
use crate::parallel::{
    run_snr_point_range, worker_offset, FrameOutcome, WorkerCounters, WorkerCtx,
};

/// The schema version this module reads and writes. A loaded checkpoint with
/// any other value is rejected.
pub const SCHEMA_VERSION: u32 = 2;

impl ConfigHashProvider for PipelineConfig {
    fn config_hash(&self) -> String {
        config_hash(self)
    }
}

/// Per-worker resume state recorded in a [`CheckpointV2`], indexed by
/// `worker_idx`.
///
/// `rng_word_pos` is
/// [`worker_offset`]`(seed, snr_index, worker_idx, frames_in_worker)`. No
/// executor reads it back: the CPU within-SNR path resumes from the global
/// `frames_completed` and the hybrid executor restores per-worker progress
/// from `frames_in_worker`; both key each frame's RNG position on the global
/// frame index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerState {
    /// Zero-based worker partition index.
    pub worker_idx: usize,
    /// Number of frames this worker has completed so far (its next frame index
    /// within the partition).
    pub frames_in_worker: u64,
    /// Absolute ChaCha20 32-bit-word position for this worker's next frame.
    ///
    /// Serialised as a decimal string: a `u128` above `2^53` does not fit a
    /// JSON number.
    #[serde(with = "u128_string")]
    pub rng_word_pos: u128,
}

/// The checkpoint payload of one SNR point.
///
/// Written to `<checkpoint_dir>/snr_<NNNN>.json` (zero-padded to 4 digits) by
/// [`CheckpointWriter`] and loaded by [`CheckpointReader`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckpointV2 {
    /// Schema version. Always [`SCHEMA_VERSION`] (`2`) on write; the reader
    /// rejects any other value.
    pub schema_version: u32,
    /// Zero-based SNR-point index this checkpoint belongs to.
    pub snr_index: usize,
    /// The Es/N0 (dB) of this SNR point (diagnostic).
    pub esn0_db: f64,
    /// `"blake3:<hex>"` of the live [`PipelineConfig`] (see [`config_hash`]).
    pub config_hash: String,
    /// Target frame count for the point; equals `max_frames`.
    pub frames_target: u64,
    /// Target frame-error count for the point.
    pub errors_target: u64,
    /// Hard frame cap for the point.
    pub max_frames: u64,
    /// Frames completed across all workers so far.
    pub frames_completed: u64,
    /// Frame errors accumulated across all workers so far.
    pub errors_accumulated: u64,
    /// Sum of decoder iteration counts.
    pub total_iterations: u64,
    /// Sum of decoder queries (frames decoded).
    pub total_queries: u64,
    /// Sum of information bits compared.
    pub total_bits: u64,
    /// Sum of information-bit errors.
    pub total_bit_errors: u64,
    /// `true` once the point hit `frames_target` or `errors_target`; resume
    /// skips a completed point.
    pub completed: bool,
    /// Per-worker resume state, indexed by `worker_idx`.
    pub worker_states: Vec<WorkerState>,
    /// Microseconds-since-epoch at which the drain completed and the counters
    /// were latched (diagnostic).
    pub drain_committed_at_us_since_epoch: u128,
}

impl CheckpointPayload for CheckpointV2 {
    const IDENTITY: &'static str = "gf2-sim/snr-checkpoint-v2";
    const SCHEMA_VERSION: u32 = SCHEMA_VERSION;
}

/// `serde` adaptor (de)serialising a `u128` as a decimal string.
///
/// A `u128` above `2^53` cannot round-trip through a JSON number, so
/// [`WorkerState::rng_word_pos`] is stored as a string.
mod u128_string {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &u128, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u128, D::Error> {
        let s = String::deserialize(d)?;
        s.parse::<u128>().map_err(serde::de::Error::custom)
    }
}

/// Computes the config hash of a [`PipelineConfig`]:
/// `"blake3:<64 lowercase hex chars>"` over every field except
/// `checkpoint_dir`, `tracing_log_path`, `diagnostic_dump_dir` and
/// `inject_gpu_oom_modulus`.
///
/// `gpu_enabled` is hashed because the CPU executor and the hybrid executor
/// record differently shaped `worker_states[]` and path-specific
/// `total_iterations`, so a checkpoint written by one does not resume on the
/// other.
#[must_use]
pub fn config_hash(config: &PipelineConfig) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&config.seed.to_le_bytes());
    hasher.update(&(config.esn0_db_points.len() as u64).to_le_bytes());
    for &v in &config.esn0_db_points {
        hasher.update(&v.to_le_bytes());
    }
    hasher.update(&config.target_errors.to_le_bytes());
    hasher.update(&config.max_frames.to_le_bytes());
    hasher.update(&config.heartbeat_every_frames.to_le_bytes());
    hasher.update(&(config.parallelism.get() as u64).to_le_bytes());
    hasher.update(&[u8::from(config.gpu_enabled)]);
    hasher.update(&[u8::from(config.strict_gpu)]);
    format!("blake3:{}", hasher.finalize().to_hex())
}

/// The checkpoint file path for SNR point `index` (`<dir>/snr_<NNNN>.json`).
#[must_use]
pub fn checkpoint_path(dir: &Path, index: usize) -> PathBuf {
    dir.join(format!("snr_{index:04}.json"))
}

/// SNR checkpoint-directory adapter over the generic atomic writer.
///
/// Each [`write`](Self::write) selects `snr_<NNNN>.json` from the payload's
/// `snr_index`, then delegates envelope serialization and the complete
/// PID-temp/file-fsync/rename/directory-fsync sequence to
/// [`GenericCheckpointWriter`].
#[derive(Debug, Clone)]
pub struct CheckpointWriter {
    dir: PathBuf,
}

impl CheckpointWriter {
    /// Creates an SNR writer and its checkpoint directory.
    ///
    /// # Errors
    ///
    /// Returns the underlying I/O error if the directory cannot be created.
    pub fn new(dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    /// Returns the checkpoint directory.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Writes an SNR payload through the generic checkpoint mechanism.
    ///
    /// # Errors
    ///
    /// Propagates serialization and durability errors from the generic writer.
    pub fn write(&self, checkpoint: &CheckpointV2) -> std::io::Result<()> {
        self.write_with_fsync_hook(checkpoint, || {})
    }

    /// Writes with an instrumentation callback immediately before file fsync.
    ///
    /// The callback serves the kill-during-fsync test; production callers use
    /// [`write`](Self::write).
    ///
    /// # Errors
    ///
    /// The same errors as [`write`](Self::write).
    #[doc(hidden)]
    pub fn write_with_fsync_hook(
        &self,
        checkpoint: &CheckpointV2,
        on_pre_fsync: impl FnOnce(),
    ) -> std::io::Result<()> {
        GenericCheckpointWriter::<CheckpointV2, _>::for_payload(
            checkpoint_path(&self.dir, checkpoint.snr_index),
            checkpoint.config_hash.clone(),
        )?
        .write_payload_with_fsync_hook(checkpoint, on_pre_fsync)
    }
}

/// SNR checkpoint-directory adapter over the generic validated reader.
#[derive(Debug, Clone)]
pub struct CheckpointReader {
    dir: PathBuf,
    expected_hash: String,
}

impl CheckpointReader {
    /// Creates an SNR reader bound to the live configuration hash.
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>, expected_hash: String) -> Self {
        Self {
            dir: dir.into(),
            expected_hash,
        }
    }

    /// Loads the indexed SNR payload through the generic checkpoint mechanism.
    ///
    /// # Returns
    ///
    /// Returns `Ok(None)` when the file is absent and `Ok(Some(_))` only when
    /// the generic envelope and the payload's retained v2 metadata both match.
    ///
    /// # Errors
    ///
    /// Any present malformed or mismatched checkpoint is returned as
    /// [`BuildError::ConfigHashMismatch`].
    pub fn load(&self, index: usize) -> Result<Option<CheckpointV2>, FatalError> {
        let checkpoint = GenericCheckpointReader::<CheckpointV2, _>::for_payload(
            checkpoint_path(&self.dir, index),
            self.expected_hash.clone(),
        )
        .load_payload()
        .map_err(|error| self.load_error(error))?;

        if let Some(ref checkpoint) = checkpoint {
            if checkpoint.schema_version != SCHEMA_VERSION {
                return Err(FatalError::BuildError(BuildError::ConfigHashMismatch {
                    loaded: format!("schema_version:{}", checkpoint.schema_version),
                    expected: self.expected_hash.clone(),
                }));
            }
            if checkpoint.config_hash != self.expected_hash {
                return Err(FatalError::BuildError(BuildError::ConfigHashMismatch {
                    loaded: checkpoint.config_hash.clone(),
                    expected: self.expected_hash.clone(),
                }));
            }
        }
        Ok(checkpoint)
    }

    fn load_error(&self, error: CheckpointLoadError) -> FatalError {
        let loaded = match error {
            CheckpointLoadError::Io(error) => format!("io-error:{error}"),
            CheckpointLoadError::Invalid(_) => "schema:not-valid-v2".to_string(),
            CheckpointLoadError::SchemaVersionMismatch { loaded, .. } => {
                format!("schema_version:{loaded}")
            }
            CheckpointLoadError::PayloadIdentityMismatch { loaded, .. } => {
                format!("payload_identity:{loaded}")
            }
            CheckpointLoadError::ConfigHashMismatch { loaded, .. } => loaded,
        };
        FatalError::BuildError(BuildError::ConfigHashMismatch {
            loaded,
            expected: self.expected_hash.clone(),
        })
    }
}

/// Process-wide SIGINT/SIGTERM interrupt flag, lazily installing the `ctrlc`
/// handler on first access.
static INTERRUPTED: OnceLock<Arc<AtomicBool>> = OnceLock::new();

/// Returns the process-wide interrupt flag, installing the `ctrlc` handler on
/// first call.
///
/// A failure to install the handler is ignored; the runner then never
/// observes a signal.
fn interrupted_flag() -> &'static Arc<AtomicBool> {
    INTERRUPTED.get_or_init(|| {
        let flag = Arc::new(AtomicBool::new(false));
        let f2 = flag.clone();
        let _ = ctrlc::set_handler(move || {
            f2.store(true, Ordering::SeqCst);
        });
        flag
    })
}

/// Clears the interrupt flag. Call at the start of a campaign so a prior
/// request does not carry over.
pub fn clear_interrupt() {
    interrupted_flag().store(false, Ordering::SeqCst);
}

/// Returns `true` if SIGINT/SIGTERM was received since the last
/// [`clear_interrupt`].
#[must_use]
pub fn is_interrupted() -> bool {
    interrupted_flag().load(Ordering::SeqCst)
}

/// Requests a graceful interrupt, with the effect of a SIGINT/SIGTERM.
///
/// [`run_snr_point_checkpointed`] and [`run_sweep_checkpointed`] observe the
/// request at the next chunk boundary, after the chunk's checkpoint is
/// flushed, and return with `interrupted = true`. Pair with
/// [`clear_interrupt`] before a subsequent run.
pub fn request_interrupt() {
    interrupted_flag().store(true, Ordering::SeqCst);
}

/// Test-only hook to set the interrupt flag without delivering a real signal.
#[cfg(test)]
pub(crate) fn set_interrupted_for_test() {
    request_interrupt();
}

/// CPU counterpart of
/// [`Scheduler::drain_for_checkpoint`](crate::Scheduler::drain_for_checkpoint);
/// a no-op.
///
/// The rayon join inside [`run_snr_point_range`] completes every in-flight
/// frame before it returns, so the CPU path has nothing to synchronise.
#[inline]
pub fn drain_for_checkpoint() {}

/// Microseconds-since-epoch for the `drain_committed_at_us_since_epoch` stamp.
fn now_us() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros())
        .unwrap_or(0)
}

/// The outcome of a checkpointed SNR-point run.
///
/// Returned by [`run_snr_point_checkpointed`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointedRun {
    /// The aggregate counters for the point (resumed partial + freshly run).
    pub counters: WorkerCounters,
    /// `true` if the point reached `target_errors` or `max_frames`.
    pub completed: bool,
    /// `true` if the run stopped early on SIGINT/SIGTERM.
    pub interrupted: bool,
}

/// Runs one SNR point with heartbeat, SNR-boundary and interrupt
/// checkpointing.
///
/// Frames are dispatched over [`run_snr_point_range`] in chunks of
/// `config.heartbeat_every_frames` (one chunk when it is `0`). After each
/// chunk the runner adds the chunk's counters to the total, records each
/// worker's cumulative frame count in `worker_states[]`, and writes a
/// [`CheckpointV2`] through `writer`. It stops at `config.max_frames`, once
/// `errors >= config.target_errors` when that is non-zero, or when
/// [`is_interrupted`] holds at a chunk boundary.
///
/// With `resume = Some(ckpt)` the runner continues from
/// `ckpt.frames_completed` and folds the loaded counters into the result;
/// `tests::test_checkpointed_resume_byte_identical_smoke` checks that the
/// counters equal those of an uninterrupted run. A `resume` that is
/// `completed` or has `frames_completed >= max_frames` returns its loaded
/// counters immediately.
///
/// `on_heartbeat_flush(snr_index, frames_completed)` fires after each
/// checkpoint write that leaves the point incomplete.
///
/// # Errors
///
/// Returns a [`std::io::Error`] if a checkpoint write fails.
///
/// # Complexity
///
/// `O(frames_run)` frame closures across `config.parallelism` workers.
#[allow(clippy::too_many_arguments)]
pub fn run_snr_point_checkpointed<S, M, F, H>(
    config: &PipelineConfig,
    snr_index: usize,
    esn0_db: f64,
    writer: &CheckpointWriter,
    expected_hash: &str,
    resume: Option<CheckpointV2>,
    make_state: M,
    sim_frame: F,
    mut on_heartbeat_flush: H,
) -> std::io::Result<CheckpointedRun>
where
    M: Fn() -> S + Sync,
    F: Fn(usize, &mut WorkerCtx, &mut S) -> FrameOutcome + Sync,
    H: FnMut(usize, u64),
{
    let parallelism = config.parallelism;
    let num_workers = parallelism.get();
    let max_frames = config.max_frames as usize;
    let target_errors = config.target_errors;

    let chunk = if config.heartbeat_every_frames == 0 {
        max_frames
    } else {
        config.heartbeat_every_frames as usize
    };

    // `cumulative[w]` counts the frames worker `w` completed across all
    // chunks. It differs from a single `0..frames_completed` striding because
    // each chunk's striding restarts at the chunk's `start`.
    let mut start = 0usize;
    let mut total = WorkerCounters::default();
    let mut cumulative = vec![0u64; num_workers];
    if let Some(ref ck) = resume {
        if ck.completed || ck.frames_completed as usize >= max_frames {
            return Ok(CheckpointedRun {
                counters: loaded_counters(ck),
                completed: true,
                interrupted: false,
            });
        }
        start = ck.frames_completed as usize;
        total = loaded_counters(ck);
        // Loaded entries map by `worker_idx`; entries at or above
        // `num_workers` are ignored.
        for ws in &ck.worker_states {
            if ws.worker_idx < num_workers {
                cumulative[ws.worker_idx] = ws.frames_in_worker;
            }
        }
    }

    let mut completed = false;
    let mut interrupted = false;

    while start < max_frames {
        if is_interrupted() {
            interrupted = true;
            break;
        }

        let end = (start + chunk.max(1)).min(max_frames);
        let out = run_snr_point_range(
            config.seed,
            snr_index,
            start..end,
            parallelism,
            &make_state,
            &sim_frame,
        );
        drain_for_checkpoint();

        total = WorkerCounters::reduce_in_worker_order(&[total, out.counters]);
        for (w, &chunk_frames) in out.per_worker_frames.iter().enumerate() {
            cumulative[w] += chunk_frames;
        }
        start = end;

        let reached_target = target_errors > 0 && total.errors >= target_errors;
        completed = start >= max_frames || reached_target;

        let ckpt = build_checkpoint(
            config,
            snr_index,
            esn0_db,
            expected_hash,
            &total,
            &cumulative,
            completed,
        );
        writer.write(&ckpt)?;

        if completed {
            break;
        }
        on_heartbeat_flush(snr_index, total.frames);
    }

    Ok(CheckpointedRun {
        counters: total,
        completed,
        interrupted,
    })
}

/// The outcome of a full checkpointed SNR sweep ([`run_sweep_checkpointed`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SweepRun {
    /// One [`CheckpointedRun`] per SNR point that was run or loaded, in
    /// SNR-point order, ending at the interrupted point if any.
    pub per_point: Vec<CheckpointedRun>,
    /// `true` if the sweep stopped early on SIGINT/SIGTERM.
    pub interrupted: bool,
}

/// Runs an SNR sweep with per-point checkpoint/resume and interrupt-aware
/// early stop.
///
/// SNR points run serially. For point `idx` the sweep loads `snr_<idx>.json`
/// from the writer's directory when `resume` is set (a missing file is a
/// fresh point), obtains `(make_state, sim_frame)` from
/// `make_point(idx, esn0_db)`, and runs [`run_snr_point_checkpointed`]. An
/// interrupted point ends the sweep with `interrupted: true`.
///
/// `on_point_complete(snr_idx, esn0_db, &run)` fires once per point that was
/// not interrupted; `on_heartbeat_flush` is passed to
/// [`run_snr_point_checkpointed`].
///
/// # Errors
///
/// [`SweepError::Load`] for a present checkpoint that is malformed or
/// mismatched, [`SweepError::Io`] for a failed checkpoint write.
///
/// # Complexity
///
/// `O(sum over points of frames_run)` frame closures, `config.parallelism`
/// workers per point.
///
/// # Examples
///
/// ```no_run
/// use std::num::NonZeroUsize;
/// use gf2_sim::PipelineConfig;
/// use gf2_sim::snr_checkpoint::{config_hash, run_sweep_checkpointed, CheckpointWriter};
/// use gf2_sim::parallel::{FrameOutcome, WorkerCtx};
/// use rand::Rng as _;
///
/// let config = PipelineConfig {
///     seed: 7,
///     esn0_db_points: vec![3.0, 3.5, 4.0],
///     target_errors: 0,
///     max_frames: 8,
///     heartbeat_every_frames: 4,
///     checkpoint_dir: Some("/tmp/sweep".into()),
///     tracing_log_path: None,
///     parallelism: NonZeroUsize::new(2).unwrap(),
///     gpu_enabled: false,
///     strict_gpu: false,
///     diagnostic_dump_dir: None,
///     inject_gpu_oom_modulus: None,
/// };
/// let h = config_hash(&config);
/// let writer = CheckpointWriter::new("/tmp/sweep").unwrap();
/// let sweep = run_sweep_checkpointed(&config, &writer, &h, false,
///     |_idx, _esn0| {
///         (
///             || (),
///             |_g: usize, ctx: &mut WorkerCtx, _s: &mut ()| {
///                 let x: u64 = ctx.rng_mut().random();
///                 FrameOutcome { errored: x & 1 == 1, iterations: 1, info_bits: 8, bit_errors: x & 1 }
///             },
///         )
///     },
///     |_idx, _esn0, _run| {}, // per-point completion callback (unused here)
///     |_idx, _frames| {},     // per-heartbeat-flush callback (unused here)
/// )
/// .unwrap();
/// assert_eq!(sweep.per_point.len(), 3);
/// ```
pub fn run_sweep_checkpointed<S, M, F, P, C, H>(
    config: &PipelineConfig,
    writer: &CheckpointWriter,
    expected_hash: &str,
    resume: bool,
    make_point: P,
    mut on_point_complete: C,
    mut on_heartbeat_flush: H,
) -> Result<SweepRun, SweepError>
where
    M: Fn() -> S + Sync,
    F: Fn(usize, &mut WorkerCtx, &mut S) -> FrameOutcome + Sync,
    P: Fn(usize, f64) -> (M, F),
    C: FnMut(usize, f64, &CheckpointedRun),
    H: FnMut(usize, u64),
{
    let reader = CheckpointReader::new(writer.dir().to_path_buf(), expected_hash.to_string());
    let mut per_point = Vec::with_capacity(config.esn0_db_points.len());
    let mut interrupted = false;

    for (idx, &esn0_db) in config.esn0_db_points.iter().enumerate() {
        let loaded = if resume {
            reader.load(idx).map_err(SweepError::Load)?
        } else {
            None
        };

        let (make_state, sim_frame) = make_point(idx, esn0_db);
        let run = run_snr_point_checkpointed(
            config,
            idx,
            esn0_db,
            writer,
            expected_hash,
            loaded,
            make_state,
            sim_frame,
            &mut on_heartbeat_flush,
        )
        .map_err(SweepError::Io)?;

        let was_interrupted = run.interrupted;
        if !was_interrupted {
            on_point_complete(idx, esn0_db, &run);
        }
        per_point.push(run);
        if was_interrupted {
            interrupted = true;
            break;
        }
    }

    Ok(SweepRun {
        per_point,
        interrupted,
    })
}

/// Error from a checkpointed sweep.
#[derive(Debug)]
pub enum SweepError {
    /// A loaded checkpoint was invalid (see [`CheckpointReader::load`]).
    Load(FatalError),
    /// A checkpoint write failed.
    Io(std::io::Error),
    /// A pipeline stage faulted during the hybrid CPU+GPU sweep
    /// ([`Scheduler::run_sweep_checkpointed`](crate::Scheduler::run_sweep_checkpointed)):
    /// a GPU decode fault, a failed per-stream drain, or a config validation
    /// error (e.g. a missing `checkpoint_dir`).
    Stage(StageError),
}

impl std::fmt::Display for SweepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SweepError::Load(e) => write!(f, "checkpoint load failed: {e:?}"),
            SweepError::Io(e) => write!(f, "checkpoint write failed: {e}"),
            SweepError::Stage(e) => write!(f, "checkpointed run stage fault: {e}"),
        }
    }
}

impl std::error::Error for SweepError {}

/// Reconstructs the running [`WorkerCounters`] from a loaded checkpoint.
pub(crate) fn loaded_counters(ck: &CheckpointV2) -> WorkerCounters {
    WorkerCounters {
        frames: ck.frames_completed,
        errors: ck.errors_accumulated,
        total_iterations: ck.total_iterations,
        total_bits: ck.total_bits,
        total_bit_errors: ck.total_bit_errors,
    }
}

/// Builds a [`CheckpointV2`] from the running totals and the per-worker
/// cumulative completed-frame counts.
///
/// `per_worker_frames[w]` is recorded verbatim: under the CPU chunked dispatch
/// the striding restarts at each chunk's `start`, so the counts differ from a
/// single `0..frames_completed` striding
/// (`tests::test_worker_states_record_authoritative_chunked_distribution`).
/// Each `rng_word_pos` is
/// [`worker_offset`]`(seed, snr_index, worker_idx, frames_in_worker)`.
pub(crate) fn build_checkpoint(
    config: &PipelineConfig,
    snr_index: usize,
    esn0_db: f64,
    expected_hash: &str,
    total: &WorkerCounters,
    per_worker_frames: &[u64],
    completed: bool,
) -> CheckpointV2 {
    let worker_states: Vec<WorkerState> = per_worker_frames
        .iter()
        .enumerate()
        .map(|(w, &frames_in_worker)| {
            let rng_word_pos = worker_offset(config.seed, snr_index, w, frames_in_worker as usize);
            WorkerState {
                worker_idx: w,
                frames_in_worker,
                rng_word_pos,
            }
        })
        .collect();

    CheckpointV2 {
        schema_version: SCHEMA_VERSION,
        snr_index,
        esn0_db,
        config_hash: expected_hash.to_string(),
        frames_target: config.max_frames,
        errors_target: config.target_errors,
        max_frames: config.max_frames,
        frames_completed: total.frames,
        errors_accumulated: total.errors,
        total_iterations: total.total_iterations,
        total_queries: total.frames,
        total_bits: total.total_bits,
        total_bit_errors: total.total_bit_errors,
        completed,
        worker_states,
        drain_committed_at_us_since_epoch: now_us(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng as _;
    use std::num::NonZeroUsize;

    /// Serializes tests that touch the process-wide interrupt flag: `cargo
    /// test` runs a crate's unit tests multi-threaded in one process.
    static INTERRUPT_FLAG_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn interrupt_test_lock() -> std::sync::MutexGuard<'static, ()> {
        let guard = INTERRUPT_FLAG_GUARD
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        clear_interrupt();
        guard
    }

    fn test_config(parallelism: usize) -> PipelineConfig {
        PipelineConfig {
            seed: 0xC0FFEE,
            esn0_db_points: vec![6.25],
            target_errors: 0, // 0 ⇒ run all frames, no early stop
            max_frames: 40,
            heartbeat_every_frames: 7,
            checkpoint_dir: None,
            tracing_log_path: None,
            parallelism: NonZeroUsize::new(parallelism).unwrap(),
            gpu_enabled: false,
            strict_gpu: false,
            diagnostic_dump_dir: None,
            inject_gpu_oom_modulus: None,
        }
    }

    fn synth_frame(_g: usize, ctx: &mut WorkerCtx, _s: &mut ()) -> FrameOutcome {
        let u1: f64 = ctx.rng_mut().random();
        let u2: f64 = ctx.rng_mut().random();
        let errored = (u1 + u2) > 1.0;
        FrameOutcome {
            errored,
            iterations: if errored { 5 } else { 1 },
            info_bits: 32,
            bit_errors: u64::from(errored),
        }
    }

    #[test]
    fn test_config_hash_excludes_paths() {
        let cfg = test_config(4);
        let with_paths = PipelineConfig {
            checkpoint_dir: Some("/tmp/a".into()),
            tracing_log_path: Some("/tmp/b.jsonl".into()),
            ..cfg.clone()
        };
        assert_eq!(config_hash(&cfg), config_hash(&with_paths));
        let diff_seed = PipelineConfig {
            seed: 1,
            ..cfg.clone()
        };
        assert_ne!(config_hash(&cfg), config_hash(&diff_seed));
    }

    #[test]
    fn test_config_hash_includes_gpu_enabled() {
        let cfg = test_config(4);
        let gpu = PipelineConfig {
            gpu_enabled: true,
            ..cfg.clone()
        };
        assert_ne!(
            config_hash(&cfg),
            config_hash(&gpu),
            "gpu_enabled must be part of the config hash"
        );
    }

    #[test]
    fn test_checkpoint_v2_roundtrip_json() {
        let cfg = test_config(2);
        let total = WorkerCounters {
            frames: 14,
            errors: 3,
            total_iterations: 40,
            total_bits: 448,
            total_bit_errors: 3,
        };
        // 8/6: the 2-worker, 2-chunk distribution of 14 frames.
        let per_worker_frames = [8u64, 6u64];
        let ckpt = build_checkpoint(
            &cfg,
            0,
            6.25,
            "blake3:abc",
            &total,
            &per_worker_frames,
            false,
        );
        let json = serde_json::to_string(&ckpt).unwrap();
        let back: CheckpointV2 = serde_json::from_str(&json).unwrap();
        assert_eq!(ckpt, back);
        assert!(json.contains("\"rng_word_pos\":\""));
        assert_eq!(back.worker_states.len(), 2);
        assert_eq!(back.worker_states[0].frames_in_worker, 8);
        assert_eq!(back.worker_states[1].frames_in_worker, 6);
        let sum: u64 = back.worker_states.iter().map(|w| w.frames_in_worker).sum();
        assert_eq!(sum, back.frames_completed);
    }

    #[test]
    fn test_checkpoint_v2_is_a_generic_payload_instantiation() {
        let dir = tempdir();
        let path = dir.path().join("snr-payload.json");
        let cfg = test_config(1);
        let hash = config_hash(&cfg);
        let payload = build_checkpoint(
            &cfg,
            0,
            6.25,
            &hash,
            &WorkerCounters::default(),
            &[0],
            false,
        );
        GenericCheckpointWriter::<CheckpointV2, _>::for_payload(&path, hash.clone())
            .unwrap()
            .write_payload(&payload)
            .unwrap();
        let loaded = GenericCheckpointReader::<CheckpointV2, _>::for_payload(&path, hash)
            .load_payload()
            .unwrap();
        assert_eq!(loaded, Some(payload));
    }

    #[test]
    fn test_reader_rejects_non_v2_schema() {
        let dir = tempdir();
        let cfg = test_config(1);
        let h = config_hash(&cfg);
        let mut ckpt = build_checkpoint(&cfg, 0, 6.25, &h, &WorkerCounters::default(), &[0], false);
        ckpt.schema_version = 1;
        let writer = CheckpointWriter::new(dir.path()).unwrap();
        writer.write(&ckpt).unwrap();
        let reader = CheckpointReader::new(dir.path(), h);
        match reader.load(0) {
            Err(FatalError::BuildError(BuildError::ConfigHashMismatch { loaded, .. })) => {
                assert!(loaded.contains("schema_version:1"));
            }
            other => panic!("expected ConfigHashMismatch, got {other:?}"),
        }
    }

    #[test]
    fn test_reader_rejects_hash_mismatch() {
        let dir = tempdir();
        let cfg = test_config(1);
        let writer = CheckpointWriter::new(dir.path()).unwrap();
        let ckpt = build_checkpoint(
            &cfg,
            0,
            6.25,
            "blake3:STALE",
            &WorkerCounters::default(),
            &[0],
            false,
        );
        writer.write(&ckpt).unwrap();
        let reader = CheckpointReader::new(dir.path(), config_hash(&cfg));
        assert!(matches!(
            reader.load(0),
            Err(FatalError::BuildError(
                BuildError::ConfigHashMismatch { .. }
            ))
        ));
    }

    #[test]
    fn test_reader_missing_file_is_none() {
        let dir = tempdir();
        let reader = CheckpointReader::new(dir.path(), "blake3:x".to_string());
        assert_eq!(reader.load(3).unwrap(), None);
    }

    #[test]
    fn test_reader_rejects_non_v2_shaped_file() {
        let dir = tempdir();
        let not_v2 = r#"{ "snr_index": 0, "eb_n0_db": 1.99, "frames_completed": 100,
            "rng_word_pos": "13060800", "completed": true,
            "config_hash": "blake3:ef56" }"#;
        std::fs::write(checkpoint_path(dir.path(), 0), not_v2).unwrap();
        let reader = CheckpointReader::new(dir.path(), "blake3:ef56".to_string());
        assert!(matches!(
            reader.load(0),
            Err(FatalError::BuildError(
                BuildError::ConfigHashMismatch { .. }
            ))
        ));
    }

    #[test]
    fn test_atomic_write_no_partial_on_existing() {
        let dir = tempdir();
        let cfg = test_config(1);
        let h = config_hash(&cfg);
        let writer = CheckpointWriter::new(dir.path()).unwrap();
        let c1 = build_checkpoint(&cfg, 0, 6.25, &h, &WorkerCounters::default(), &[0], false);
        writer.write(&c1).unwrap();
        let total = WorkerCounters {
            frames: 10,
            ..Default::default()
        };
        let c2 = build_checkpoint(&cfg, 0, 6.25, &h, &total, &[10], true);
        writer.write(&c2).unwrap();
        let reader = CheckpointReader::new(dir.path(), h);
        let loaded = reader.load(0).unwrap().unwrap();
        assert_eq!(loaded.frames_completed, 10);
        assert!(loaded.completed);
        assert!(!dir.path().join("snr_0000.tmp").exists());
    }

    #[test]
    fn test_atomic_write_crash_mid_flush_leaves_previous_state() {
        let dir = tempdir();
        let cfg = test_config(1);
        let h = config_hash(&cfg);
        let writer = CheckpointWriter::new(dir.path()).unwrap();

        let prev = WorkerCounters {
            frames: 7,
            ..Default::default()
        };
        let c_prev = build_checkpoint(&cfg, 0, 6.25, &h, &prev, &[7], false);
        writer.write(&c_prev).unwrap();

        let tmp = dir
            .path()
            .join(format!("snr_0000.{}.tmp", std::process::id()));
        std::fs::write(
            &tmp,
            b"{ \"schema_version\": 2, \"snr_index\": 0, \"frames_comp",
        )
        .unwrap();

        let reader = CheckpointReader::new(dir.path(), h);
        let loaded = reader
            .load(0)
            .expect("canonical checkpoint must still be a valid v2 file")
            .expect("canonical checkpoint must exist");
        assert_eq!(loaded.frames_completed, 7);
        assert!(!loaded.completed);
        assert!(
            tmp.exists(),
            "the half-written tmp is still on disk (orphaned)"
        );
    }

    #[test]
    fn test_checkpointed_resume_byte_identical_smoke() {
        let _guard = interrupt_test_lock();
        let cfg = test_config(2);
        let h = config_hash(&cfg);

        let dir_ref = tempdir();
        let w_ref = CheckpointWriter::new(dir_ref.path()).unwrap();
        clear_interrupt();
        let reference = run_snr_point_checkpointed(
            &cfg,
            0,
            6.25,
            &w_ref,
            &h,
            None,
            || (),
            synth_frame,
            |_, _| {},
        )
        .unwrap();
        assert!(reference.completed);

        let dir = tempdir();
        let writer = CheckpointWriter::new(dir.path()).unwrap();
        let interrupt_cfg = PipelineConfig {
            max_frames: 7, // first chunk only
            ..cfg.clone()
        };
        clear_interrupt();
        let partial = run_snr_point_checkpointed(
            &interrupt_cfg,
            0,
            6.25,
            &writer,
            &h,
            None,
            || (),
            synth_frame,
            |_, _| {},
        )
        .unwrap();
        assert!(partial.completed); // hit its (reduced) max_frames

        let reader = CheckpointReader::new(dir.path(), h.clone());
        let mut loaded = reader.load(0).unwrap().unwrap();
        loaded.completed = false; // continue the point under the full budget
        let resumed = run_snr_point_checkpointed(
            &cfg,
            0,
            6.25,
            &writer,
            &h,
            Some(loaded),
            || (),
            synth_frame,
            |_, _| {},
        )
        .unwrap();

        assert_eq!(
            resumed.counters, reference.counters,
            "checkpoint resume must be byte-identical to the uninterrupted run"
        );
        assert!(resumed.completed);
    }

    #[test]
    fn test_worker_states_record_authoritative_chunked_distribution() {
        let _guard = interrupt_test_lock();
        // For 14 frames as chunks 0..7 then 7..14 with 2 workers, the per-chunk
        // striding restarts at each chunk's `start`:
        //   chunk 0..7  : worker0 = {0,2,4,6}   = 4, worker1 = {1,3,5}    = 3
        //   chunk 7..14 : worker0 = {7,9,11,13} = 4, worker1 = {8,10,12}  = 3
        //   cumulative  : worker0 = 8,             worker1 = 6
        // where a single dispatch gives 7/7.
        let cfg = PipelineConfig {
            max_frames: 14,
            heartbeat_every_frames: 7,
            target_errors: 0,
            ..test_config(2)
        };
        let h = config_hash(&cfg);
        let dir = tempdir();
        let writer = CheckpointWriter::new(dir.path()).unwrap();
        clear_interrupt();
        let run = run_snr_point_checkpointed(
            &cfg,
            0,
            6.25,
            &writer,
            &h,
            None,
            || (),
            synth_frame,
            |_, _| {},
        )
        .unwrap();
        assert!(run.completed);
        assert_eq!(run.counters.frames, 14);

        let loaded = CheckpointReader::new(dir.path(), h)
            .load(0)
            .unwrap()
            .unwrap();
        assert_eq!(loaded.worker_states.len(), 2);
        assert_eq!(
            loaded.worker_states[0].frames_in_worker, 8,
            "worker 0 must record its real chunked count (8), not the analytic 7"
        );
        assert_eq!(
            loaded.worker_states[1].frames_in_worker, 6,
            "worker 1 must record its real chunked count (6), not the analytic 7"
        );
        let sum: u64 = loaded
            .worker_states
            .iter()
            .map(|w| w.frames_in_worker)
            .sum();
        assert_eq!(sum, loaded.frames_completed);
        assert_eq!(sum, 14);
    }

    #[test]
    fn test_worker_states_rng_word_pos_uses_real_worker_idx() {
        let cfg = test_config(2);
        let h = config_hash(&cfg);
        let snr = 3usize;
        let per_worker = [8u64, 6u64];
        let total = WorkerCounters {
            frames: 14,
            ..Default::default()
        };
        let ckpt = build_checkpoint(&cfg, snr, 6.25, &h, &total, &per_worker, false);
        assert_eq!(ckpt.worker_states.len(), 2);
        assert_eq!(ckpt.worker_states[0].worker_idx, 0);
        assert_eq!(ckpt.worker_states[1].worker_idx, 1);
        assert_eq!(
            ckpt.worker_states[0].rng_word_pos,
            worker_offset(cfg.seed, snr, 0, 8)
        );
        assert_eq!(
            ckpt.worker_states[1].rng_word_pos,
            worker_offset(cfg.seed, snr, 1, 6),
            "worker 1 must be keyed on worker_idx=1, not 0"
        );
        assert_ne!(
            ckpt.worker_states[1].rng_word_pos,
            worker_offset(cfg.seed, snr, 0, 6),
            "rng_word_pos must NOT collapse worker 1 onto the worker_idx=0 axis"
        );
    }

    #[test]
    fn test_sigint_before_run_stops_immediately() {
        let _guard = interrupt_test_lock();
        let cfg = test_config(2);
        let h = config_hash(&cfg);
        let dir = tempdir();
        let writer = CheckpointWriter::new(dir.path()).unwrap();
        set_interrupted_for_test();
        let run = run_snr_point_checkpointed(
            &cfg,
            0,
            6.25,
            &writer,
            &h,
            None,
            || (),
            synth_frame,
            |_, _| {},
        )
        .unwrap();
        assert!(run.interrupted);
        assert!(!run.completed);
        assert_eq!(run.counters.frames, 0);
        clear_interrupt();
    }

    #[test]
    fn test_sigint_mid_run_flushes_resumable_checkpoint() {
        let _guard = interrupt_test_lock();
        clear_interrupt();
        let cfg = test_config(1); // single worker, heartbeat = 7, max = 40
        let h = config_hash(&cfg);
        let dir = tempdir();
        let writer = CheckpointWriter::new(dir.path()).unwrap();

        let trip_at = 6usize; // last frame of the first 7-frame chunk (0..7)
        let frame = move |g: usize, ctx: &mut WorkerCtx, s: &mut ()| {
            let out = synth_frame(g, ctx, s);
            if g == trip_at {
                set_interrupted_for_test();
            }
            out
        };

        let run =
            run_snr_point_checkpointed(&cfg, 0, 6.25, &writer, &h, None, || (), frame, |_, _| {})
                .unwrap();
        clear_interrupt();

        assert!(run.interrupted, "the mid-run SIGINT must stop the run");
        assert!(!run.completed);
        assert_eq!(run.counters.frames, 7);

        let loaded = CheckpointReader::new(dir.path(), h)
            .load(0)
            .unwrap()
            .expect("a checkpoint must have been flushed before the halt");
        assert_eq!(loaded.frames_completed, 7);
        assert!(!loaded.completed);
        assert_eq!(loaded.worker_states[0].frames_in_worker, 7);
    }

    fn tempdir() -> gf2_core::test_scratch::Scratch {
        gf2_core::test_scratch::scratch("gf2sim-ck")
    }
}
