//! v2 heartbeat-checkpoint schema, atomic writer, v2-only reader, and the
//! checkpointed SNR-point runner (design doc §4).
//!
//! Owned by `5f12e7ff`. The checkpoint format is **v2-only**: there is a single
//! schema and no backward-compatibility layer. A loaded file whose
//! `schema_version` is not `2`, whose `config_hash` does not match the live
//! config, or which does not parse as v2 JSON, is rejected as a hard load error
//! ([`FatalError::BuildError`] wrapping [`BuildError::ConfigHashMismatch`]) —
//! i.e. it is treated as not-a-valid-v2-checkpoint (corruption / wrong config),
//! never silently accepted.
//!
//! # What this module provides
//!
//! * [`CheckpointV2`] / [`WorkerState`] — the v2 schema (`worker_states[]` is
//!   **required**, `schema_version: 2`).
//! * [`config_hash`] — the blake3 hash of the serialised [`PipelineConfig`]
//!   **excluding** the path-dependent `checkpoint_dir` / `tracing_log_path`
//!   fields. A loaded checkpoint whose hash differs aborts the resume.
//! * [`CheckpointWriter`] — atomic per-SNR JSON writer (tmp + fsync + rename +
//!   directory fsync, crash-safe under SIGINT during the write).
//! * [`CheckpointReader`] — v2-only loader that restores the per-worker
//!   `rng_word_pos` and rejects non-v2 / hash-mismatched files.
//! * [`run_snr_point_checkpointed`] — the executor-facing runner: it dispatches
//!   heartbeat-sized chunks of frames over [`run_snr_point_range`], settles the
//!   rayon workers on a frame boundary (the CPU drain), latches the per-worker
//!   counters into a [`CheckpointV2`], and flushes it at the heartbeat cadence,
//!   at the SNR boundary, and on SIGINT.
//!
//! # The CPU drain (design doc §4 "Drain commit contract")
//!
//! This module is the CPU half (Phase A). For the CPU path, "drain" means:
//! dispatch a bounded chunk of frames via [`run_snr_point_range`] (whose rayon
//! `join` is the natural settle point — every in-flight frame completes and
//! increments its worker's count before the call returns), then latch the
//! per-worker counts as the SSOT for each worker's next
//! [`worker_offset`](crate::parallel::worker_offset) seek. No partial frames
//! are ever recorded mid-chunk. The GPU-stream drain
//! ([`Scheduler::drain_for_checkpoint`](crate::Scheduler::drain_for_checkpoint),
//! per-stream `hipStreamSynchronize`) and the checkpointed **hybrid** CPU+GPU
//! sweep ([`Pipeline::run_checkpointed`](crate::Pipeline::run_checkpointed))
//! landed with Phase C task `571c11c4` in `executor::drain`; see
//! [`drain_for_checkpoint`] for how the two halves correspond.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::config::PipelineConfig;
use crate::error::{BuildError, FatalError, StageError};
use crate::parallel::{
    run_snr_point_range, worker_offset, FrameOutcome, WorkerCounters, WorkerCtx,
};

/// The fixed schema version this module reads and writes. A loaded checkpoint
/// with any other value is rejected (design doc §4: v2-only reader).
pub const SCHEMA_VERSION: u32 = 2;

/// Per-worker resume state recorded in a [`CheckpointV2`] (design doc §4).
///
/// Indexed by `worker_idx`. `rng_word_pos` is
/// `worker_offset(seed, snr_index, worker_idx, frames_in_worker)` — the
/// per-worker stream position per design-doc §4's drain contract.
///
/// **Resume semantics (design-doc §4, amended 2026-06-08 and 2026-06-10):**
/// every executor keys every frame on the *global* frame index (§3,
/// [`worker_offset`](crate::parallel::worker_offset)`(seed, snr_idx, 0, g)`) —
/// the **CPU within-SNR path** (Phase A, `5f12e7ff`/`3fcb7025`) resumes via the
/// global `frames_completed`, and the **Phase C hybrid executor** (`75c22fa8`,
/// strided partitions; resume `571c11c4`) restores per-worker *progress* from
/// `frames_in_worker` and re-derives per-frame RNG positions from the global
/// index. **No executor reads `rng_word_pos` back**; it is recorded for v2
/// schema fidelity only (the per-worker-stream restore §4 originally
/// anticipated was retired by the 2026-06-10 amendment). The global keying is
/// what guarantees byte-identity across worker counts, per `3fcb7025`.
///
/// `rng_word_pos` is serialised as a **decimal string** because a `u128` does
/// not fit a JSON number above `2^53`.
///
/// # Examples
///
/// ```
/// use gf2_sim::checkpoint::WorkerState;
///
/// let ws = WorkerState { worker_idx: 1, frames_in_worker: 1563, rng_word_pos: 6_402_048 };
/// assert_eq!(ws.rng_word_pos, 6_402_048);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerState {
    /// Zero-based worker partition index.
    pub worker_idx: usize,
    /// Number of frames this worker has completed so far (its next frame index
    /// within the partition).
    pub frames_in_worker: u64,
    /// Absolute ChaCha20 32-bit-word position for this worker's next frame.
    ///
    /// Serialised as a decimal string (`u128` > `2^53` does not fit a JSON
    /// number); see the module/type docs.
    #[serde(with = "u128_string")]
    pub rng_word_pos: u128,
}

/// A v2 per-SNR-point checkpoint (design doc §4).
///
/// Written to `<checkpoint_dir>/snr_<NNNN>.json` (zero-padded to 4 digits) by
/// [`CheckpointWriter`] and loaded by [`CheckpointReader`]. Field order and
/// names match the design-doc §4 schema verbatim. `worker_states[]` is required
/// (not optional): per-SNR-boundary checkpoints set each worker's
/// `frames_in_worker` from the executor's authoritative counter.
///
/// # Examples
///
/// ```
/// use gf2_sim::checkpoint::{CheckpointV2, WorkerState};
///
/// let ckpt = CheckpointV2 {
///     schema_version: gf2_sim::checkpoint::SCHEMA_VERSION,
///     snr_index: 5,
///     esn0_db: 6.25,
///     config_hash: "blake3:dead".to_string(),
///     frames_target: 100_000,
///     errors_target: 100,
///     max_frames: 10_000_000,
///     frames_completed: 37_555,
///     errors_accumulated: 0,
///     total_iterations: 65_082,
///     total_queries: 37_555,
///     total_bits: 1_185_735_384,
///     total_bit_errors: 0,
///     completed: false,
///     worker_states: vec![WorkerState { worker_idx: 0, frames_in_worker: 37_555, rng_word_pos: 0 }],
///     drain_committed_at_us_since_epoch: 1_717_891_200_000_000,
/// };
/// assert_eq!(ckpt.schema_version, 2);
/// ```
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
    /// Target frame count for the point (`max_frames` cap or the configured
    /// `frames_target`).
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
    /// Per-worker resume state, indexed by `worker_idx`. **Required** in v2.
    pub worker_states: Vec<WorkerState>,
    /// Microseconds-since-epoch at which the drain completed and the counters
    /// were latched (diagnostic).
    pub drain_committed_at_us_since_epoch: u128,
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

/// Computes the v2 config hash for a [`PipelineConfig`] (design doc §4).
///
/// The hash is `"blake3:<64 lowercase hex chars>"` over a canonical encoding of
/// every field that affects simulation results: `seed`, the Es/N0 points,
/// `target_errors`, `max_frames`, `heartbeat_every_frames`, `parallelism`,
/// `gpu_enabled`, and `strict_gpu`. The path-dependent fields `checkpoint_dir`
/// and `tracing_log_path` are **excluded** — they control where output lands,
/// not the simulation itself, so changing them must not invalidate a checkpoint
/// directory (design doc §4).
///
/// `gpu_enabled` **is** hashed (added by `571c11c4`): the CPU within-SNR
/// executor and the hybrid strided-partition executor record *differently
/// shaped* `worker_states[]` (chunk-restart striding vs fixed strided-partition
/// prefixes) and accumulate path-specific `total_iterations` (design doc §11
/// excludes `mean_iters` from CPU-vs-GPU byte-identity), so a checkpoint
/// written by one path must not silently resume on the other.
///
/// # Arguments
///
/// * `config` — the live pipeline configuration.
///
/// # Returns
///
/// A `"blake3:<hex>"` string.
///
/// # Examples
///
/// ```
/// use std::num::NonZeroUsize;
/// use gf2_sim::PipelineConfig;
/// use gf2_sim::checkpoint::config_hash;
///
/// let cfg = PipelineConfig {
///     seed: 42,
///     esn0_db_points: vec![6.25],
///     target_errors: 100,
///     max_frames: 1000,
///     heartbeat_every_frames: 10,
///     checkpoint_dir: None,
///     tracing_log_path: None,
///     parallelism: NonZeroUsize::new(4).unwrap(),
///     gpu_enabled: false,
///     strict_gpu: false,
/// };
/// let h = config_hash(&cfg);
/// assert!(h.starts_with("blake3:"));
/// // The path-dependent fields do not change the hash.
/// let cfg2 = PipelineConfig { checkpoint_dir: Some("/tmp/x".into()), ..cfg.clone() };
/// assert_eq!(config_hash(&cfg), config_hash(&cfg2));
/// ```
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

/// The checkpoint file name for SNR point `index` (`snr_<NNNN>.json`).
///
/// # Arguments
///
/// * `dir` — the checkpoint directory.
/// * `index` — the zero-based SNR-point index.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use gf2_sim::checkpoint::checkpoint_path;
///
/// let p = checkpoint_path(Path::new("/tmp/ck"), 5);
/// assert!(p.ends_with("snr_0005.json"));
/// ```
#[must_use]
pub fn checkpoint_path(dir: &Path, index: usize) -> PathBuf {
    dir.join(format!("snr_{index:04}.json"))
}

/// Atomic, crash-safe writer for v2 per-SNR checkpoints (design doc §4).
///
/// Each [`write`](Self::write) serialises a [`CheckpointV2`] to
/// `<dir>/snr_<NNNN>.json` via a tmp-file + fsync + rename + directory-fsync
/// sequence, so a crash (or SIGINT) at any point leaves either the complete
/// previous checkpoint or no new file — never a partially-written JSON. The
/// rename is atomic on POSIX; the directory fsync durably persists the rename
/// itself.
///
/// # Examples
///
/// ```no_run
/// use gf2_sim::checkpoint::{CheckpointWriter, CheckpointV2};
/// let writer = CheckpointWriter::new("/tmp/ck").unwrap();
/// // writer.write(&ckpt)?;  // ckpt: CheckpointV2
/// ```
#[derive(Debug, Clone)]
pub struct CheckpointWriter {
    dir: PathBuf,
}

impl CheckpointWriter {
    /// Creates a writer for `dir`, creating the directory if it does not exist.
    ///
    /// # Arguments
    ///
    /// * `dir` — the target checkpoint directory.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`std::io::Error`] if the directory cannot be
    /// created.
    pub fn new(dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    /// The checkpoint directory this writer targets.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Atomically writes `ckpt` to `<dir>/snr_<NNNN>.json`.
    ///
    /// Sequence (design doc §4 step 4, hardened): serialise to JSON, write to a
    /// unique `.tmp` sibling, fsync the tmp file, rename over the target, then
    /// fsync the directory so the rename is durable. A crash before the rename
    /// leaves the old checkpoint intact; a crash after leaves the new one fully
    /// written.
    ///
    /// # Arguments
    ///
    /// * `ckpt` — the checkpoint to persist; its `snr_index` selects the file.
    ///
    /// # Errors
    ///
    /// Returns a [`std::io::Error`] if serialisation or any filesystem step
    /// fails — including the directory `fsync`, which is a hard part of the
    /// durability contract (not best-effort).
    pub fn write(&self, ckpt: &CheckpointV2) -> std::io::Result<()> {
        self.write_with_fsync_hook(ckpt, || {})
    }

    /// Atomic write with an instrumentation callback fired immediately before
    /// the tmp-file `fsync` (the single source of truth for [`write`](Self::write)).
    ///
    /// This is identical to [`write`](Self::write) except that `on_pre_fsync` is
    /// invoked **after the tmp file's bytes are fully written but before
    /// `sync_all`** is called on it. It exists solely as an instrumentation
    /// seam: it lets a test deterministically land an external signal (e.g.
    /// SIGKILL) *during* the `fsync`, which is the precise window criterion 3
    /// requires the atomic-write contract to survive ("kill the writer mid-flush
    /// via signal during fsync"). [`write`](Self::write) is just this method with
    /// an empty hook, so there is a single write path (no duplicated
    /// tmp+fsync+rename+dir-fsync logic).
    ///
    /// It is `pub` (and `#[doc(hidden)]`) because the `checkpoint_sweep` binary
    /// — a separate crate target — drives the deterministic kill-during-fsync
    /// proof through it. Production code should call [`write`](Self::write).
    ///
    /// # Arguments
    ///
    /// * `ckpt` — the checkpoint to persist; its `snr_index` selects the file.
    /// * `on_pre_fsync` — called once, immediately before the tmp `sync_all`.
    ///
    /// # Errors
    ///
    /// Same as [`write`](Self::write): any serialisation or filesystem step,
    /// including the directory `fsync`.
    #[doc(hidden)]
    pub fn write_with_fsync_hook(
        &self,
        ckpt: &CheckpointV2,
        on_pre_fsync: impl FnOnce(),
    ) -> std::io::Result<()> {
        let path = checkpoint_path(&self.dir, ckpt.snr_index);
        let json = serde_json::to_vec_pretty(ckpt).map_err(std::io::Error::other)?;

        // Unique tmp name (pid-tagged) so concurrent writers for the same SNR
        // never clobber each other's in-progress file before the rename.
        let tmp = self.dir.join(format!(
            "snr_{:04}.{}.tmp",
            ckpt.snr_index,
            std::process::id()
        ));

        // Write the tmp bytes, fire the pre-fsync hook, then fsync the tmp file
        // before renaming. The hook runs *after* all bytes are on the page cache
        // but *before* `sync_all`, so an external kill fired from the hook lands
        // during the fsync.
        {
            use std::io::Write as _;
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&json)?;
            on_pre_fsync();
            f.sync_all()?;
        }

        // Atomic rename over the destination.
        std::fs::rename(&tmp, &path)?;

        // fsync the directory so the rename itself is durable across a crash.
        // This is part of the hard tmp+fsync+rename+dir-fsync contract (design
        // doc §4 step 4): a dir-open or dir-fsync failure is propagated as a
        // hard `io::Error` from `write`, exactly like the tmp-file fsync above.
        {
            let dir = std::fs::File::open(&self.dir)?;
            dir.sync_all()?;
        }
        Ok(())
    }
}

/// v2-only reader for per-SNR checkpoints (design doc §4).
///
/// [`load`](Self::load) deserialises `<dir>/snr_<NNNN>.json` and validates it:
/// a `schema_version` other than [`SCHEMA_VERSION`] **or** a `config_hash` that
/// differs from the live config is a hard error
/// ([`FatalError::BuildError`]`(`[`BuildError::ConfigHashMismatch`]`)`). The
/// format is v2-only: any file that is not a valid v2 checkpoint (corrupt,
/// truncated, or written under a different config) is rejected, never silently
/// accepted.
///
/// # Examples
///
/// ```no_run
/// use gf2_sim::checkpoint::CheckpointReader;
/// let reader = CheckpointReader::new("/tmp/ck", "blake3:dead".to_string());
/// // let maybe = reader.load(5)?;  // Ok(None) if the file is absent.
/// ```
#[derive(Debug, Clone)]
pub struct CheckpointReader {
    dir: PathBuf,
    expected_hash: String,
}

impl CheckpointReader {
    /// Creates a reader bound to `dir` that requires `expected_hash`.
    ///
    /// # Arguments
    ///
    /// * `dir` — the checkpoint directory to read from.
    /// * `expected_hash` — the live config's [`config_hash`]; any loaded file
    ///   with a different hash is rejected.
    pub fn new(dir: impl Into<PathBuf>, expected_hash: String) -> Self {
        Self {
            dir: dir.into(),
            expected_hash,
        }
    }

    /// Loads and validates the checkpoint for SNR point `index`.
    ///
    /// # Arguments
    ///
    /// * `index` — the zero-based SNR-point index.
    ///
    /// # Returns
    ///
    /// `Ok(None)` if no checkpoint file exists for `index` (a fresh point);
    /// `Ok(Some(ckpt))` for a valid v2 checkpoint whose hash matches.
    ///
    /// # Errors
    ///
    /// * [`FatalError::BuildError`]`(`[`BuildError::ConfigHashMismatch`]`)` if
    ///   the file's `schema_version` is not [`SCHEMA_VERSION`] (the loaded hash
    ///   is reported as `"schema_version:<n>"` so the mismatch is legible), or
    ///   if its `config_hash` differs from the expected hash.
    /// * [`FatalError::BuildError`]`(`[`BuildError::ConfigHashMismatch`]`)` if
    ///   the file exists but cannot be parsed as v2 JSON (a corrupt or
    ///   truncated file).
    pub fn load(&self, index: usize) -> Result<Option<CheckpointV2>, FatalError> {
        let path = checkpoint_path(&self.dir, index);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                // An unreadable-but-present file is a configuration fault, not a
                // fresh point; surface it as a hard error rather than silently
                // restarting the point.
                return Err(FatalError::BuildError(BuildError::ConfigHashMismatch {
                    loaded: format!("io-error:{e}"),
                    expected: self.expected_hash.clone(),
                }));
            }
        };

        let ckpt: CheckpointV2 = serde_json::from_slice(&bytes).map_err(|_| {
            // A parse failure on a present file means it is not a valid v2
            // checkpoint (corruption / truncation). The v2-only reader rejects
            // it rather than guessing any other layout.
            FatalError::BuildError(BuildError::ConfigHashMismatch {
                loaded: "schema:not-valid-v2".to_string(),
                expected: self.expected_hash.clone(),
            })
        })?;

        if ckpt.schema_version != SCHEMA_VERSION {
            return Err(FatalError::BuildError(BuildError::ConfigHashMismatch {
                loaded: format!("schema_version:{}", ckpt.schema_version),
                expected: self.expected_hash.clone(),
            }));
        }

        if ckpt.config_hash != self.expected_hash {
            return Err(FatalError::BuildError(BuildError::ConfigHashMismatch {
                loaded: ckpt.config_hash,
                expected: self.expected_hash.clone(),
            }));
        }

        Ok(Some(ckpt))
    }
}

/// Process-wide SIGINT/SIGTERM interrupt flag, lazily installing the `ctrlc`
/// handler on first access (the design-doc-mandated `ctrlc` crate).
static INTERRUPTED: OnceLock<Arc<AtomicBool>> = OnceLock::new();

/// Returns the process-wide interrupt flag, installing the `ctrlc` handler on
/// first call.
///
/// `OnceLock` guarantees the handler is registered exactly once even under
/// concurrent access. A failure to install (e.g. a handler already registered
/// in a test) is ignored — the runner then simply never observes an interrupt.
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

/// Clears the interrupt flag so a prior SIGINT does not bleed into a new run.
///
/// Call at the start of a campaign. Exposed for tests that drive the
/// interrupt-flush path deterministically.
pub fn clear_interrupt() {
    interrupted_flag().store(false, Ordering::SeqCst);
}

/// Returns `true` if SIGINT/SIGTERM was received since the last
/// [`clear_interrupt`].
#[must_use]
pub fn is_interrupted() -> bool {
    interrupted_flag().load(Ordering::SeqCst)
}

/// Programmatically requests a graceful interrupt, identical in effect to a
/// SIGINT/SIGTERM.
///
/// The next chunk boundary in [`run_snr_point_checkpointed`] (or
/// [`run_sweep_checkpointed`]) observes the request via [`is_interrupted`],
/// having already flushed the in-progress heartbeat checkpoint with
/// `completed = false`, and returns with `interrupted = true`. This is the
/// programmatic equivalent of the `ctrlc` signal handler: it lets an embedder
/// (a watchdog, a wall-clock budget, a supervising harness) drive the same
/// checkpoint-and-stop path without an OS signal, and lets tests exercise the
/// SIGINT-flush path deterministically. Pair with [`clear_interrupt`] before a
/// subsequent run so the request does not bleed across runs.
///
/// # Examples
///
/// ```
/// use gf2_sim::checkpoint::{clear_interrupt, is_interrupted, request_interrupt};
///
/// clear_interrupt();
/// assert!(!is_interrupted());
/// request_interrupt();
/// assert!(is_interrupted());
/// clear_interrupt();
/// ```
pub fn request_interrupt() {
    interrupted_flag().store(true, Ordering::SeqCst);
}

/// Test-only hook to set the interrupt flag without delivering a real signal.
#[cfg(test)]
pub(crate) fn set_interrupted_for_test() {
    request_interrupt();
}

/// CPU "drain" seam for the checkpoint commit contract (design doc §4).
///
/// On the CPU path a drain is implicit: [`run_snr_point_range`] dispatches a
/// bounded chunk of frames and its rayon `join` is the settle point — every
/// in-flight frame completes and increments its worker's count before the call
/// returns, so there is nothing further to synchronise. This function therefore
/// returns immediately; it is the **named CPU counterpart** of the GPU drain
/// [`Scheduler::drain_for_checkpoint`](crate::Scheduler::drain_for_checkpoint)
/// (Phase C `571c11c4`, `executor::drain`), which iterates each worker's owned
/// HIP stream and calls the per-stream `hipStreamSynchronize()` (not
/// `hipDeviceSynchronize()`) before the counters are latched. It is
/// intentionally a no-op here, not a fake GPU implementation.
#[inline]
pub fn drain_for_checkpoint() {
    // CPU path: the rayon join in `run_snr_point_range` already settled all
    // in-flight frames on a frame boundary. The GPU per-stream sync lives in
    // `Scheduler::drain_for_checkpoint` (`571c11c4`).
}

/// Microseconds-since-epoch for the `drain_committed_at_us_since_epoch` stamp.
fn now_us() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros())
        .unwrap_or(0)
}

/// The outcome of a checkpointed SNR-point run.
///
/// Returned by [`run_snr_point_checkpointed`]. `interrupted` is `true` when the
/// run stopped early because SIGINT/SIGTERM tripped mid-point (the final
/// heartbeat checkpoint was flushed before returning).
///
/// # Examples
///
/// ```
/// use gf2_sim::checkpoint::CheckpointedRun;
/// use gf2_sim::parallel::WorkerCounters;
///
/// let run = CheckpointedRun {
///     counters: WorkerCounters::default(),
///     completed: true,
///     interrupted: false,
/// };
/// assert!(run.completed);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointedRun {
    /// The aggregate counters for the point (resumed partial + freshly run).
    pub counters: WorkerCounters,
    /// `true` if the point reached `target_errors` or `max_frames`.
    pub completed: bool,
    /// `true` if the run stopped early on SIGINT/SIGTERM.
    pub interrupted: bool,
}

/// Runs one SNR point with heartbeat + SNR-boundary + SIGINT checkpointing
/// (design doc §4 deliverable 2).
///
/// This is the executor-facing runner. It dispatches the point's frames in
/// heartbeat-sized chunks over [`run_snr_point_range`], and after **each chunk
/// settles** (the CPU drain — see [`drain_for_checkpoint`]) it:
///
/// 1. accumulates the chunk's counters into the running total,
/// 2. latches per-worker `worker_states[]` from the authoritative per-worker
///    frame counts (each worker's `rng_word_pos =`
///    [`worker_offset`](crate::parallel::worker_offset)`(seed, snr_index,
///    worker_idx, frames_in_worker)`),
/// 3. writes a [`CheckpointV2`] atomically.
///
/// On resume, the caller passes the loaded checkpoint as `resume`; the runner
/// continues from `resume.frames_completed` and folds the loaded counters into
/// the result, so the final aggregate is byte-identical to an uninterrupted
/// run (every frame's outcome is a pure function of its global index — see
/// [`run_snr_point_range`]).
///
/// Early stop: after each chunk the runner checks `target_errors` (stop once
/// `errors_accumulated >= target_errors`) and SIGINT. On either it flushes a
/// final checkpoint and returns.
///
/// # Arguments
///
/// * `config` — the live pipeline config (supplies `seed`, `parallelism`,
///   `heartbeat_every_frames`, `max_frames`, `target_errors`).
/// * `snr_index` — zero-based SNR-point index.
/// * `esn0_db` — the point's Es/N0 (dB), recorded in the checkpoint.
/// * `writer` — the atomic checkpoint writer.
/// * `expected_hash` — the live [`config_hash`]; recorded in each checkpoint.
/// * `resume` — `Some(ckpt)` to continue an interrupted point, `None` to start
///   fresh. A `resume` whose `frames_completed >= max_frames` (or `completed`)
///   returns immediately with the loaded counters.
/// * `make_state` / `sim_frame` — the per-worker state factory and per-frame
///   closure (see [`run_snr_point_range`]).
/// * `on_heartbeat_flush` — callback `(snr_index, frames_completed)` fired
///   **after each WITHIN-point (non-final) heartbeat checkpoint write** — i.e.
///   the flushes where the point has not yet completed. It is **not** fired for
///   the final SNR-boundary flush (that is the point-complete event the sweep
///   reports). A CLI driver can use it to emit a mid-point progress marker so a
///   parent can deliver a SIGINT while the point is still simulating; pass
///   `|_, _| {}` if unused.
///
/// # Returns
///
/// A [`CheckpointedRun`] with the aggregate counters and the completed /
/// interrupted flags.
///
/// # Errors
///
/// Returns a [`std::io::Error`] if a checkpoint write fails.
///
/// # Complexity
///
/// `O(frames_run)` frame closures across `config.parallelism` workers.
///
/// # Examples
///
/// ```no_run
/// use std::num::NonZeroUsize;
/// use gf2_sim::PipelineConfig;
/// use gf2_sim::checkpoint::{config_hash, run_snr_point_checkpointed, CheckpointWriter};
/// use gf2_sim::parallel::{FrameOutcome, WorkerCtx};
/// use rand::Rng as _;
///
/// let config = PipelineConfig {
///     seed: 7,
///     esn0_db_points: vec![3.0],
///     target_errors: 0,
///     max_frames: 8,
///     heartbeat_every_frames: 4,
///     checkpoint_dir: Some("/tmp/snr".into()),
///     tracing_log_path: None,
///     parallelism: NonZeroUsize::new(2).unwrap(),
///     gpu_enabled: false,
///     strict_gpu: false,
/// };
/// let h = config_hash(&config);
/// let writer = CheckpointWriter::new("/tmp/snr").unwrap();
/// let run = run_snr_point_checkpointed(
///     &config, 0, 3.0, &writer, &h, None,
///     || (),
///     |_g: usize, ctx: &mut WorkerCtx, _s: &mut ()| {
///         let x: u64 = ctx.rng_mut().random();
///         FrameOutcome { errored: x & 1 == 1, iterations: 1, info_bits: 8, bit_errors: x & 1 }
///     },
///     |_snr, _frames| {}, // per-heartbeat-flush callback (unused here)
/// )
/// .unwrap();
/// assert!(run.completed);
/// ```
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

    // Heartbeat chunk size (0 disables within-SNR heartbeats: one chunk to the
    // end, with only the SNR-boundary flush).
    let chunk = if config.heartbeat_every_frames == 0 {
        max_frames
    } else {
        config.heartbeat_every_frames as usize
    };

    // Resume state: start frame, folded-in counters, and the authoritative
    // per-worker cumulative frame counts (the SSOT for `worker_states[]`,
    // design doc §4 step 3). `cumulative[w]` is the number of frames worker `w`
    // has completed across all chunks so far; it is NOT the analytic
    // `0..frames_completed` distribution, because the chunked dispatch restarts
    // its striding at each chunk's `start` (see `run_snr_point_range`).
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
        // Carry forward the pre-interruption per-worker counts so the resumed
        // checkpoint's `worker_states[]` include them. A resumed run under a
        // different worker count maps positionally by `worker_idx` (any extra
        // workers start at 0; surplus loaded entries are ignored).
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
        // CPU drain: rayon join above already settled every in-flight frame on
        // a frame boundary; this is the named seam for the Phase C GPU drain.
        drain_for_checkpoint();

        total = WorkerCounters::reduce_in_worker_order(&[total, out.counters]);
        // Accumulate the authoritative per-worker counts this chunk reported.
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
        // A within-point (non-final) heartbeat flush just hit disk with
        // `0 < frames_completed < max_frames` and per-worker state. Notify the
        // driver so it can emit a mid-point progress marker.
        on_heartbeat_flush(snr_index, total.frames);
    }

    Ok(CheckpointedRun {
        counters: total,
        completed,
        interrupted,
    })
}

/// The outcome of a full checkpointed SNR sweep ([`run_sweep_checkpointed`]).
///
/// `per_point` holds one [`CheckpointedRun`] per SNR point actually run (a
/// resumed sweep that skips already-completed leading points still records them
/// from their loaded checkpoints). `interrupted` is `true` if a SIGINT/SIGTERM
/// stopped the sweep before every point completed; in that case `per_point` is
/// truncated at the interrupted point (whose checkpoint was already flushed).
///
/// # Examples
///
/// ```
/// use gf2_sim::checkpoint::SweepRun;
///
/// let sweep = SweepRun { per_point: vec![], interrupted: false };
/// assert!(!sweep.interrupted);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SweepRun {
    /// One [`CheckpointedRun`] per SNR point that was run or loaded, in
    /// SNR-point order.
    pub per_point: Vec<CheckpointedRun>,
    /// `true` if the sweep stopped early on SIGINT/SIGTERM.
    pub interrupted: bool,
}

/// Runs a full **SNR sweep** with per-point checkpoint/resume and SIGINT-aware
/// early stop (design doc §4; issue criterion 1, deliverable 2).
///
/// This is the sweep-level driver over [`run_snr_point_checkpointed`]. SNR
/// points are processed **serially** (cross-SNR parallelism is deliberately not
/// in the contract — the within-SNR frame parallelism, design doc §3, is the
/// concurrency model). For each point `idx` with Es/N0 `config.esn0_db_points[idx]`:
///
/// 1. If `resume` is set, load `<checkpoint_dir>/snr_<idx>.json` via
///    [`CheckpointReader`] (a missing file ⇒ fresh point; a non-v2 or
///    hash-mismatched file ⇒ the load error propagates as
///    [`FatalError`](crate::error::FatalError)).
/// 2. Build the point's per-worker state factory and per-frame closure from
///    `make_point(idx, esn0_db)`.
/// 3. Run [`run_snr_point_checkpointed`], which flushes a v2 checkpoint at the
///    heartbeat cadence, at the SNR boundary, and on SIGINT.
///
/// If a point returns `interrupted`, the sweep stops immediately (its
/// checkpoint was already flushed by `run_snr_point_checkpointed`) and the
/// returned [`SweepRun`] carries `interrupted: true`. Resuming the sweep
/// (`resume = true` with the same `checkpoint_dir`) continues byte-identically:
/// completed points are skipped via their loaded checkpoints, the interrupted
/// point resumes from its recorded `frames_completed`, and the per-frame
/// outcome is a pure function of the global frame index (design doc §3), so the
/// aggregate matches an uninterrupted run at the same seed.
///
/// # Arguments
///
/// * `config` — the live pipeline config (supplies `esn0_db_points`, `seed`,
///   `parallelism`, `heartbeat_every_frames`, `max_frames`, `target_errors`).
/// * `writer` — the atomic checkpoint writer (its directory must match the
///   reader directory for resume to find prior checkpoints).
/// * `expected_hash` — the live [`config_hash`]; recorded in each checkpoint and
///   required of any loaded checkpoint.
/// * `resume` — `true` to load and continue from existing checkpoints in the
///   writer's directory; `false` to start every point fresh.
/// * `make_point` — factory `(snr_idx, esn0_db) -> (make_state, sim_frame)`
///   producing the point's per-worker state factory and per-frame closure. It
///   is called once per SNR point, so each point can bind channel parameters
///   derived from its Es/N0.
/// * `on_point_complete` — callback `(snr_idx, esn0_db, &CheckpointedRun)` fired
///   once per SNR point **after** its SNR-boundary checkpoint has been flushed
///   and the point finished (it is not called for an interrupted point, whose
///   run stops before completion). A CLI driver can use it to emit a
///   progress marker; pass `|_, _, _| {}` if unused.
/// * `on_heartbeat_flush` — callback `(snr_idx, frames_completed)` threaded into
///   [`run_snr_point_checkpointed`]; fired after each within-point (non-final)
///   heartbeat checkpoint write. A CLI driver can use it to emit a *mid-point*
///   progress marker (so a parent can deliver a SIGINT while a point is still
///   simulating); pass `|_, _| {}` if unused.
///
/// # Returns
///
/// A [`SweepRun`] with the per-point runs and the aggregate `interrupted` flag.
///
/// # Errors
///
/// Propagates a [`FatalError`](crate::error::FatalError) from a checkpoint load
/// (non-v2 schema or `config_hash` mismatch), or a [`std::io::Error`] from a
/// checkpoint write, boxed as [`SweepError`].
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
/// use gf2_sim::checkpoint::{config_hash, run_sweep_checkpointed, CheckpointWriter};
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
        // Fire the completion callback only for a finished point (its
        // SNR-boundary checkpoint is on disk); never for an interrupted point.
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

/// Error from [`run_sweep_checkpointed`].
///
/// Either a fatal checkpoint-load error (non-v2 schema or `config_hash`
/// mismatch) or an I/O error from a checkpoint write.
///
/// # Examples
///
/// ```
/// use gf2_sim::checkpoint::SweepError;
/// fn is_io(e: &SweepError) -> bool { matches!(e, SweepError::Io(_)) }
/// ```
#[derive(Debug)]
pub enum SweepError {
    /// A loaded checkpoint was invalid (see [`CheckpointReader::load`]).
    Load(FatalError),
    /// A checkpoint write failed.
    Io(std::io::Error),
    /// A pipeline stage faulted during a checkpointed run (the hybrid CPU+GPU
    /// sweep, [`Scheduler::run_sweep_checkpointed`](crate::Scheduler::run_sweep_checkpointed),
    /// `571c11c4`): a GPU decode fault, a failed per-stream drain, or a config
    /// validation error (e.g. a missing `checkpoint_dir`).
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
///
/// `pub(crate)`: the hybrid checkpointed runner (`executor::drain`, `571c11c4`)
/// folds loaded counters with this same helper so the projection stays SSOT.
pub(crate) fn loaded_counters(ck: &CheckpointV2) -> WorkerCounters {
    WorkerCounters {
        frames: ck.frames_completed,
        errors: ck.errors_accumulated,
        total_iterations: ck.total_iterations,
        total_bits: ck.total_bits,
        total_bit_errors: ck.total_bit_errors,
    }
}

/// Builds a [`CheckpointV2`] from the running totals, latching per-worker
/// `worker_states[]` from the **authoritative** per-worker frame counts.
///
/// `per_worker_frames[w]` is worker `w`'s cumulative completed-frame count —
/// the executor's authoritative counter (design doc §4 "Drain commit contract",
/// step 3): on the CPU path, as reported by [`run_snr_point_range`] across
/// every chunk so far; on the hybrid path (`executor::drain`, `571c11c4`), the
/// worker's strided-partition progress latched after the GPU drain. It is
/// recorded verbatim and is **not** recomputed from an analytic
/// `0..frames_completed` striding, which would be wrong under the CPU chunked
/// dispatch (the per-chunk striding restarts at each chunk's `start`, so the
/// real per-worker distribution differs from a single-dispatch distribution).
///
/// Each worker's recorded `rng_word_pos` is `worker_offset(seed, snr_index,
/// worker_idx, frames_in_worker)` — the per-worker stream position per design
/// doc §4's drain contract. The **CPU** within-SNR path (Phase A) does not
/// itself seek to this position on resume: it keys every frame on the *global*
/// frame index (§3, `worker_offset(seed, snr_idx, 0, g)`) and resumes via the
/// global `frames_completed` (§4 step 5, amended 2026-06-08). `rng_word_pos` is
/// recorded per the v2 schema; per the 2026-06-10 §4 amendment **no executor
/// reads it back** — the hybrid executor (`571c11c4`) restores per-worker
/// *progress* from `frames_in_worker` and re-derives per-frame RNG positions
/// from the global frame index.
///
/// `pub(crate)`: the hybrid checkpointed runner (`executor::drain`, `571c11c4`)
/// latches its post-drain `worker_states[]` through this same constructor so
/// the v2 schema projection stays SSOT.
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
            // Record the per-worker stream position per design-doc §4's drain
            // contract: `worker_offset(seed, snr_idx, worker_idx, frames_in_worker)`
            // with the *physical* `worker_idx = w`. NO executor reads it back
            // (§4 amendment 2026-06-10): every executor keys every frame on the
            // global index (§3, `worker_offset(.., 0, g)`) — the CPU path resumes
            // via the global `frames_completed`, the hybrid executor (`571c11c4`)
            // via the per-worker `frames_in_worker` progress. The position is
            // recorded for v2 schema fidelity only.
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

    /// Serializes unit tests that touch the process-wide interrupt flag (the
    /// SIGINT path read inside [`run_snr_point_checkpointed`]). Bare `cargo test`
    /// runs a crate's unit tests multi-threaded in **one process**, so without
    /// this guard a test that trips the global flag can bleed it into a
    /// concurrent run that expects to complete (observed as spurious
    /// `assertion failed: run.completed`). `nextest` runs one process per test
    /// and is immune, but the `tests` gate uses `cargo test`.
    static INTERRUPT_FLAG_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Acquires [`INTERRUPT_FLAG_GUARD`] (recovering from poisoning so a single
    /// panicking test does not cascade) and clears any stale interrupt request.
    /// Hold the returned guard for the whole test so no other flag-touching test
    /// runs concurrently (`MutexGuard` is itself `#[must_use]`, so a dropped
    /// return is caught by the compiler).
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
        }
    }

    /// A synthetic stateless per-frame closure whose outcome is a pure function
    /// of the global frame index (draws two f64s like the real channel).
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
        // A result-affecting field flips the hash.
        let diff_seed = PipelineConfig {
            seed: 1,
            ..cfg.clone()
        };
        assert_ne!(config_hash(&cfg), config_hash(&diff_seed));
    }

    #[test]
    fn test_config_hash_includes_gpu_enabled() {
        // `gpu_enabled` IS result-affecting (`571c11c4`): the CPU and hybrid
        // executors record differently shaped `worker_states[]` and
        // path-specific `total_iterations`, so a checkpoint written by one path
        // must not resume on the other. Flipping the flag must flip the hash.
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
        // The authoritative per-worker counts (8/6 for a 2-worker, 2-chunk
        // dispatch of 14 frames — NOT the analytic 7/7); see
        // `test_worker_states_record_authoritative_chunked_distribution`.
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
        // rng_word_pos serialised as a string.
        assert!(json.contains("\"rng_word_pos\":\""));
        // worker_states required and recorded verbatim from the authoritative
        // counts (8/6), not recomputed.
        assert_eq!(back.worker_states.len(), 2);
        assert_eq!(back.worker_states[0].frames_in_worker, 8);
        assert_eq!(back.worker_states[1].frames_in_worker, 6);
        // Invariant: per-worker counts sum to frames_completed.
        let sum: u64 = back.worker_states.iter().map(|w| w.frames_in_worker).sum();
        assert_eq!(sum, back.frames_completed);
    }

    #[test]
    fn test_reader_rejects_non_v2_schema() {
        let dir = tempdir();
        let cfg = test_config(1);
        let h = config_hash(&cfg);
        let mut ckpt = build_checkpoint(&cfg, 0, 6.25, &h, &WorkerCounters::default(), &[0], false);
        ckpt.schema_version = 1;
        let writer = CheckpointWriter::new(&dir).unwrap();
        writer.write(&ckpt).unwrap();
        let reader = CheckpointReader::new(&dir, h);
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
        let writer = CheckpointWriter::new(&dir).unwrap();
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
        let reader = CheckpointReader::new(&dir, config_hash(&cfg));
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
        let reader = CheckpointReader::new(&dir, "blake3:x".to_string());
        assert_eq!(reader.load(3).unwrap(), None);
    }

    #[test]
    fn test_reader_rejects_non_v2_shaped_file() {
        let dir = tempdir();
        // A present file whose JSON does not match the v2 schema (missing
        // worker_states / schema_version) is not a valid v2 checkpoint and must
        // be rejected, never silently accepted.
        let not_v2 = r#"{ "snr_index": 0, "eb_n0_db": 1.99, "frames_completed": 100,
            "rng_word_pos": "13060800", "completed": true,
            "config_hash": "blake3:ef56" }"#;
        std::fs::write(checkpoint_path(&dir, 0), not_v2).unwrap();
        let reader = CheckpointReader::new(&dir, "blake3:ef56".to_string());
        assert!(matches!(
            reader.load(0),
            Err(FatalError::BuildError(
                BuildError::ConfigHashMismatch { .. }
            ))
        ));
    }

    #[test]
    fn test_atomic_write_no_partial_on_existing() {
        // Writing twice leaves a complete, parseable file each time (no .tmp
        // residue under the canonical name).
        let dir = tempdir();
        let cfg = test_config(1);
        let h = config_hash(&cfg);
        let writer = CheckpointWriter::new(&dir).unwrap();
        let c1 = build_checkpoint(&cfg, 0, 6.25, &h, &WorkerCounters::default(), &[0], false);
        writer.write(&c1).unwrap();
        let total = WorkerCounters {
            frames: 10,
            ..Default::default()
        };
        let c2 = build_checkpoint(&cfg, 0, 6.25, &h, &total, &[10], true);
        writer.write(&c2).unwrap();
        let reader = CheckpointReader::new(&dir, h);
        let loaded = reader.load(0).unwrap().unwrap();
        assert_eq!(loaded.frames_completed, 10);
        assert!(loaded.completed);
        // No leftover canonical-name .tmp.
        assert!(!dir.join("snr_0000.tmp").exists());
    }

    #[test]
    fn test_atomic_write_crash_mid_flush_leaves_previous_state() {
        // Models a crash (or SIGINT) *during* the next write: the tmp file is
        // half-written and never renamed. The canonical checkpoint must still be
        // the complete previous state, and the v2 reader must load it cleanly —
        // never the torn tmp. This is the structural guarantee of the
        // tmp+fsync+rename sequence (the canonical name is only ever replaced by
        // an atomic rename).
        let dir = tempdir();
        let cfg = test_config(1);
        let h = config_hash(&cfg);
        let writer = CheckpointWriter::new(&dir).unwrap();

        // First, a complete previous-state checkpoint.
        let prev = WorkerCounters {
            frames: 7,
            ..Default::default()
        };
        let c_prev = build_checkpoint(&cfg, 0, 6.25, &h, &prev, &[7], false);
        writer.write(&c_prev).unwrap();

        // Simulate a crash mid-flush of the *next* checkpoint: a half-written
        // tmp sibling that was never renamed (truncated JSON).
        let tmp = dir.join(format!("snr_0000.{}.tmp", std::process::id()));
        std::fs::write(
            &tmp,
            b"{ \"schema_version\": 2, \"snr_index\": 0, \"frames_comp",
        )
        .unwrap();

        // The canonical file is untouched and still loads as the previous state.
        let reader = CheckpointReader::new(&dir, h);
        let loaded = reader
            .load(0)
            .expect("canonical checkpoint must still be a valid v2 file")
            .expect("canonical checkpoint must exist");
        assert_eq!(loaded.frames_completed, 7);
        assert!(!loaded.completed);
        // The torn tmp never masquerades as the checkpoint.
        assert!(
            tmp.exists(),
            "the half-written tmp is still on disk (orphaned)"
        );
    }

    #[test]
    fn test_checkpointed_resume_byte_identical_smoke() {
        let _guard = interrupt_test_lock();
        // Uninterrupted reference vs checkpoint-at-chunk-boundary resume must be
        // byte-identical. Small frame counts keep the fast tier under 5 s.
        let cfg = test_config(2);
        let h = config_hash(&cfg);

        let dir_ref = tempdir();
        let w_ref = CheckpointWriter::new(&dir_ref).unwrap();
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

        // Interrupted run: stop after the first heartbeat chunk, then resume
        // from the written checkpoint.
        let dir = tempdir();
        let writer = CheckpointWriter::new(&dir).unwrap();
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

        // Load the partial checkpoint, then resume under the full config.
        // The partial checkpoint was written by `interrupt_cfg`; rewrite the
        // recorded frames_completed under the full config for resume.
        let reader = CheckpointReader::new(&dir, h.clone());
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
        // The recorded worker_states[].frames_in_worker must be the AUTHORITATIVE
        // per-worker counter (design doc §4 step 3), not an analytic recompute.
        // For 14 frames as chunks 0..7 then 7..14 with 2 workers, the per-chunk
        // striding restarts at each chunk's `start`:
        //   chunk 0..7  : worker0 = {0,2,4,6}   = 4, worker1 = {1,3,5}    = 3
        //   chunk 7..14 : worker0 = {7,9,11,13} = 4, worker1 = {8,10,12}  = 3
        //   cumulative  : worker0 = 8,             worker1 = 6
        // => 8/6, NOT the single-dispatch analytic 7/7.
        let cfg = PipelineConfig {
            max_frames: 14,
            heartbeat_every_frames: 7,
            target_errors: 0,
            ..test_config(2)
        };
        let h = config_hash(&cfg);
        let dir = tempdir();
        let writer = CheckpointWriter::new(&dir).unwrap();
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

        let loaded = CheckpointReader::new(&dir, h).load(0).unwrap().unwrap();
        assert_eq!(loaded.worker_states.len(), 2);
        // Authoritative chunked distribution: 8/6, not the analytic 7/7.
        assert_eq!(
            loaded.worker_states[0].frames_in_worker, 8,
            "worker 0 must record its real chunked count (8), not the analytic 7"
        );
        assert_eq!(
            loaded.worker_states[1].frames_in_worker, 6,
            "worker 1 must record its real chunked count (6), not the analytic 7"
        );
        // Invariant: per-worker counts sum to frames_completed.
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
        // Regression guard (design-doc §4 formula): `rng_word_pos` must be keyed
        // on the PHYSICAL `worker_idx`, NOT hardcoded to 0. Worker 1's recorded
        // position must equal `worker_offset(seed, snr, 1, frames_in_worker[1])`
        // and must DIFFER from the `worker_idx = 0` projection — otherwise the
        // prior bug (every worker collapsed onto the worker-0 axis) could
        // silently reappear. The CPU within-SNR path itself resumes via the
        // global `frames_completed`; this field is for the Phase C executor.
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
        // Each position is the §4-formula offset keyed on the physical worker_idx.
        assert_eq!(
            ckpt.worker_states[0].rng_word_pos,
            worker_offset(cfg.seed, snr, 0, 8)
        );
        assert_eq!(
            ckpt.worker_states[1].rng_word_pos,
            worker_offset(cfg.seed, snr, 1, 6),
            "worker 1 must be keyed on worker_idx=1, not 0"
        );
        // The old worker_idx=0 bug would have made this hold; assert it does NOT.
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
        let writer = CheckpointWriter::new(&dir).unwrap();
        // Trip the interrupt before the run: the first chunk check stops it with
        // no chunk run and no checkpoint file (start == 0, nothing committed).
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
        // A SIGINT *after* the first chunk completes must (1) stop the run and
        // (2) leave a flushed, resumable v2 checkpoint on disk carrying the
        // first chunk's committed frames. The interrupt is tripped from inside
        // the sim closure once the chunk's last global frame has been seen, so
        // the next chunk-boundary check halts the loop — but only after the
        // first chunk's checkpoint was already written.
        clear_interrupt();
        let cfg = test_config(1); // single worker, heartbeat = 7, max = 40
        let h = config_hash(&cfg);
        let dir = tempdir();
        let writer = CheckpointWriter::new(&dir).unwrap();

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
        // Exactly the first chunk's frames were committed before the halt.
        assert_eq!(run.counters.frames, 7);

        // A resumable checkpoint was flushed: it loads, is not completed, and
        // records the 7 committed frames.
        let loaded = CheckpointReader::new(&dir, h)
            .load(0)
            .unwrap()
            .expect("a checkpoint must have been flushed before the halt");
        assert_eq!(loaded.frames_completed, 7);
        assert!(!loaded.completed);
        assert_eq!(loaded.worker_states[0].frames_in_worker, 7);
    }

    /// Minimal tempdir helper (avoids a dev-dependency on `tempfile`).
    fn tempdir() -> PathBuf {
        let mut p = std::env::temp_dir();
        let unique = format!(
            "gf2sim-ck-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        p.push(unique);
        std::fs::create_dir_all(&p).unwrap();
        p
    }
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
}
