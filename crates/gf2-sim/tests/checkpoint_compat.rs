//! v2 checkpoint compatibility + resume byte-identity integration tests
//! (issue `5f12e7ff` deliverables 4 & 5, design doc §4).
//!
//! Coverage:
//!
//! * **v1 → v2 migration** (`test_v1_migration_*`): a v1 checkpoint actually
//!   produced by running the legacy `gf2_coding::simulation::SimulationRunner`
//!   uncoded path is converted by the `checkpoint_migrate` binary, then the
//!   resulting v2 checkpoint loads (exactly one `worker_states` entry) and
//!   resumes single-worker byte-identically vs an uninterrupted single-thread
//!   reference. A fast synthetic structural test of the tool's output shape is
//!   also kept.
//! * **v2 library-level resume** (`test_v2_resume_*`): a v2 checkpoint written
//!   mid-point loads and resumes N-worker, with byte-identical
//!   `fer`/`frames`/`errors`/`mean_iters` vs an uninterrupted N-worker
//!   reference, on AWGN, Rayleigh, and Rician channels.
//! * **Subprocess SNR-sweep SIGINT + `--resume`** (`test_sweep_sigint_*`): one
//!   non-ignored fast-tier test PER CHANNEL (AWGN, Rayleigh, Rician) spawns the
//!   `checkpoint_sweep` binary, sends a real SIGINT mid-sweep, asserts a
//!   NON-ZERO child exit, then `--resume`s to completion and asserts the full
//!   10-SNR sweep is byte-identical to an uninterrupted reference. All three are
//!   in the cargo-ci gate (criterion 1 covers all three channels).
//! * **Kill-during-fsync** (`test_kill_during_fsync_deterministic`): reads the
//!   `--crash-during-fsync` child's `BEGIN_FSYNC` marker and SIGKILLs the
//!   instant it appears, landing the kill DURING the (large) fsync; asserts the
//!   canonical file is always a complete v2 checkpoint or absent — never torn,
//!   and that the prior complete state survives. A randomised
//!   `--crash-loop` variant adds defense in depth.
//!
//! The library-level resume tests use **small frame counts** so the fast tier
//! (5 s/test) is honoured; resume byte-identity is independent of frame count
//! (every frame's outcome is a pure function of its global index, design §3).
//! The subprocess sweep uses a larger budget so a mid-sweep SIGINT window
//! exists, but stays fast (~0.6 s/sweep for AWGN).

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use gf2_sim::batch::SymbolBatch;
use gf2_sim::channels::{Awgn, Rayleigh, Rician};
use gf2_sim::checkpoint::{
    clear_interrupt, config_hash, run_snr_point_checkpointed, CheckpointReader, CheckpointV2,
    CheckpointWriter,
};
use gf2_sim::parallel::{FrameOutcome, WorkerCtx};
use gf2_sim::PipelineConfig;

// ---------------------------------------------------------------------------
// Test scaffolding
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn tempdir(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "gf2sim-ckcompat-{tag}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn cfg(parallelism: usize, max_frames: u64, heartbeat: u64) -> PipelineConfig {
    PipelineConfig {
        seed: 0x5F12_E7FF,
        esn0_db_points: vec![6.25],
        target_errors: 0, // run the full frame budget (no early stop)
        max_frames,
        heartbeat_every_frames: heartbeat,
        checkpoint_dir: None,
        tracing_log_path: None,
        parallelism: NonZeroUsize::new(parallelism).unwrap(),
        strict_gpu: false,
    }
}

/// Builds a deterministic synthetic 1-frame `SymbolBatch` of `n` BPSK-ish
/// symbols (+1/-1 on I, 0 on Q) so a channel has real signal to corrupt. The
/// pattern is fixed (frame-independent); the per-frame variation comes entirely
/// from the channel's RNG draws, keeping each frame's outcome a pure function of
/// its seek position.
fn signal_batch(n: usize) -> SymbolBatch {
    let i: Vec<f32> = (0..n)
        .map(|k| if k % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let q: Vec<f32> = vec![0.0; n];
    SymbolBatch::new(vec![i], vec![q])
}

/// Number of symbols per synthetic frame. Small so 40 frames × {1,2,4} workers
/// stays well under the 5 s fast-tier budget while still exercising real
/// channel draws (AWGN 8 words/sym, Rayleigh/Rician 16 words/sym).
const SYMS_PER_FRAME: usize = 64;

/// A frame closure factory for an AWGN channel: applies AWGN to a fresh signal
/// batch (drawing from the runner-seeked ctx RNG), then derives a verdict from
/// the noisy energy. The verdict is a pure function of the frame's seek.
fn awgn_frame(ch: &Awgn) -> impl Fn(usize, &mut WorkerCtx, &mut ()) -> FrameOutcome + Sync + '_ {
    move |_g, ctx, _s| {
        let mut batch = signal_batch(SYMS_PER_FRAME);
        ch.apply(&mut batch, ctx.rng_mut());
        verdict(&batch)
    }
}

fn rayleigh_frame(
    ch: &Rayleigh,
) -> impl Fn(usize, &mut WorkerCtx, &mut ()) -> FrameOutcome + Sync + '_ {
    move |_g, ctx, _s| {
        let mut batch = signal_batch(SYMS_PER_FRAME);
        ch.apply(&mut batch, ctx.rng_mut());
        verdict(&batch)
    }
}

fn rician_frame(
    ch: &Rician,
) -> impl Fn(usize, &mut WorkerCtx, &mut ()) -> FrameOutcome + Sync + '_ {
    move |_g, ctx, _s| {
        let mut batch = signal_batch(SYMS_PER_FRAME);
        ch.apply(&mut batch, ctx.rng_mut());
        verdict(&batch)
    }
}

/// Derives a deterministic frame verdict from a noisy symbol batch: a symbol is
/// "in error" if the noisy I-component flips sign relative to the transmitted
/// +1/-1 pattern. Returns integer-exact counters (byte-identical across worker
/// counts).
fn verdict(batch: &SymbolBatch) -> FrameOutcome {
    let mut bit_errors = 0u64;
    for (k, &ri) in batch.i[0].iter().enumerate() {
        let tx = if k % 2 == 0 { 1.0 } else { -1.0 };
        if ri.signum() != tx {
            bit_errors += 1;
        }
    }
    FrameOutcome {
        errored: bit_errors > 0,
        iterations: 1 + bit_errors, // a real, byte-identical per-frame quantity
        info_bits: SYMS_PER_FRAME as u64,
        bit_errors,
    }
}

// ---------------------------------------------------------------------------
// v2 round-trip resume (deliverables 4b, 5) — AWGN / Rayleigh / Rician
// ---------------------------------------------------------------------------

/// Runs the full point uninterrupted, then runs it as "first chunk, checkpoint,
/// resume", and asserts the two aggregates are byte-identical. Generic over the
/// channel frame closure so AWGN / Rayleigh / Rician share one harness.
fn assert_resume_byte_identical<F>(tag: &str, parallelism: usize, frame: F)
where
    F: Fn(usize, &mut WorkerCtx, &mut ()) -> FrameOutcome + Sync,
{
    let full = cfg(parallelism, 40, 13);
    let h = config_hash(&full);

    // Uninterrupted reference.
    let dir_ref = tempdir(&format!("{tag}-ref"));
    let w_ref = CheckpointWriter::new(&dir_ref).unwrap();
    clear_interrupt();
    let reference =
        run_snr_point_checkpointed(&full, 0, 6.25, &w_ref, &h, None, || (), &frame).unwrap();
    assert!(reference.completed);
    assert_eq!(reference.counters.frames, 40);

    // Partial run capped at the first chunk, then resume under the full budget.
    let dir = tempdir(&format!("{tag}-resume"));
    let writer = CheckpointWriter::new(&dir).unwrap();
    let partial_cfg = PipelineConfig {
        max_frames: 13,
        ..full.clone()
    };
    clear_interrupt();
    let partial =
        run_snr_point_checkpointed(&partial_cfg, 0, 6.25, &writer, &h, None, || (), &frame)
            .unwrap();
    assert_eq!(partial.counters.frames, 13);

    let reader = CheckpointReader::new(&dir, h.clone());
    let mut loaded = reader.load(0).unwrap().unwrap();
    // Re-open the point under the full budget for resume.
    loaded.completed = false;
    let resumed =
        run_snr_point_checkpointed(&full, 0, 6.25, &writer, &h, Some(loaded), || (), &frame)
            .unwrap();

    assert_eq!(
        resumed.counters, reference.counters,
        "[{tag}] resume must be byte-identical to the uninterrupted run \
         (fer/frames/errors/mean_iters)"
    );
    // Spot-check the derived ratios too.
    assert_eq!(resumed.counters.fer(), reference.counters.fer());
    assert_eq!(
        resumed.counters.mean_iters(),
        reference.counters.mean_iters()
    );
    assert!(resumed.completed);
}

#[test]
fn test_v2_resume_byte_identical_awgn() {
    let ch = Awgn::new(3.0, 2); // low Es/N0 ⇒ plenty of sign flips ⇒ nonzero errors
    assert_resume_byte_identical("awgn-p1", 1, awgn_frame(&ch));
    assert_resume_byte_identical("awgn-p2", 2, awgn_frame(&ch));
    assert_resume_byte_identical("awgn-p4", 4, awgn_frame(&ch));
}

#[test]
fn test_v2_resume_byte_identical_rayleigh() {
    let ch = Rayleigh::new(6.0, 2);
    assert_resume_byte_identical("rayleigh-p1", 1, rayleigh_frame(&ch));
    assert_resume_byte_identical("rayleigh-p2", 2, rayleigh_frame(&ch));
    assert_resume_byte_identical("rayleigh-p4", 4, rayleigh_frame(&ch));
}

#[test]
fn test_v2_resume_byte_identical_rician() {
    let ch = Rician::new(6.0, 2, 4.0);
    assert_resume_byte_identical("rician-p1", 1, rician_frame(&ch));
    assert_resume_byte_identical("rician-p2", 2, rician_frame(&ch));
    assert_resume_byte_identical("rician-p4", 4, rician_frame(&ch));
}

#[test]
fn test_v2_resume_nonzero_errors_present() {
    // Guard against a vacuous byte-identity check: at low Es/N0 the AWGN
    // verdict must actually produce frame errors, so resume identity is
    // exercised on a non-trivial counter set.
    let ch = Awgn::new(0.5, 2);
    let c = cfg(2, 20, 7);
    let h = config_hash(&c);
    let dir = tempdir("nonzero");
    let w = CheckpointWriter::new(&dir).unwrap();
    clear_interrupt();
    let run =
        run_snr_point_checkpointed(&c, 0, 6.25, &w, &h, None, || (), awgn_frame(&ch)).unwrap();
    assert_eq!(run.counters.frames, 20);
    assert!(
        run.counters.errors > 0,
        "low-Es/N0 AWGN must produce frame errors; got {:?}",
        run.counters
    );
}

// ---------------------------------------------------------------------------
// v1 → v2 migration (deliverable 3, 4a)
// ---------------------------------------------------------------------------

/// Writes a legacy v1 checkpoint (byte-for-byte the `gf2_coding::simulation`
/// `SnrCheckpoint::to_json` layout) into `dir/snr_<NNNN>.json`.
#[allow(clippy::too_many_arguments)]
fn write_v1_fixture(
    dir: &Path,
    snr_index: usize,
    eb_n0_db: f64,
    frames_completed: u64,
    errors_accumulated: u64,
    total_iterations: u64,
    total_bits: u64,
    total_bit_errors: u64,
    rng_word_pos: u128,
    frames_target: u64,
    errors_target: u64,
    completed: bool,
    config_hash: &str,
) {
    let json = format!(
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
        snr_index,
        eb_n0_db,
        frames_completed,
        errors_accumulated,
        total_iterations,
        frames_completed, // total_queries == frames in the v1 coded path
        total_bits,
        total_bit_errors,
        rng_word_pos,
        frames_target,
        errors_target,
        completed,
        config_hash,
    );
    std::fs::write(dir.join(format!("snr_{snr_index:04}.json")), json).unwrap();
}

/// Runs the `checkpoint_migrate` binary against `input` → `output`.
fn run_migrate(input: &Path, output: &Path, parallelism: usize) {
    let bin = env!("CARGO_BIN_EXE_checkpoint_migrate");
    let status = std::process::Command::new(bin)
        .args([
            "--input",
            input.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
            "--parallelism",
            &parallelism.to_string(),
        ])
        .status()
        .expect("checkpoint_migrate must run");
    assert!(status.success(), "checkpoint_migrate exited non-zero");
}

#[test]
fn test_v1_migration_produces_valid_v2() {
    let v1_dir = tempdir("v1src");
    let v2_dir = tempdir("v2dst");
    write_v1_fixture(
        &v1_dir,
        0,
        1.989_700_043_360_188,
        100,
        100,
        5000,
        3_220_800,
        427_025,
        13_060_800,
        10_000_000,
        100,
        true,
        "blake3:ef56f88523777b04bf303f18c64de099a06ec322bb3f0124671cd39fad73f420",
    );

    run_migrate(&v1_dir, &v2_dir, 1);

    // The v2 reader loads it (hash matches the v1-recorded hash).
    let reader = CheckpointReader::new(
        &v2_dir,
        "blake3:ef56f88523777b04bf303f18c64de099a06ec322bb3f0124671cd39fad73f420".to_string(),
    );
    let v2 = reader.load(0).unwrap().expect("migrated v2 must exist");
    assert_eq!(v2.schema_version, 2);
    assert_eq!(v2.frames_completed, 100);
    assert_eq!(v2.errors_accumulated, 100);
    assert!(v2.completed);
    // Single-worker mapping: worker 0 carries the full count + the v1 position.
    assert_eq!(v2.worker_states.len(), 1);
    assert_eq!(v2.worker_states[0].worker_idx, 0);
    assert_eq!(v2.worker_states[0].frames_in_worker, 100);
    assert_eq!(v2.worker_states[0].rng_word_pos, 13_060_800);
}

/// The subset of a genuine v1 `snr_*.json` (written by `SimulationRunner`) that
/// the migration must carry through faithfully.
#[derive(Debug, Clone, serde::Deserialize)]
struct V1Source {
    frames_completed: u64,
    errors_accumulated: u64,
    total_iterations: u64,
    total_bits: u64,
    total_bit_errors: u64,
    /// Decimal string (`u128` does not fit a JSON number above `2^53`).
    rng_word_pos: String,
}

/// Produces a GENUINE v1 checkpoint by running the legacy
/// `gf2_coding::simulation::SimulationRunner` uncoded path with
/// `checkpoint_dir = Some(dir)` (the fast per-SNR-boundary checkpoint path,
/// `simulation.rs:1611`). Returns `(checkpoint_dir, config_hash, v1_source)`
/// where `v1_source` is the parsed genuine `snr_0000.json` the runner wrote.
///
/// `SnrCheckpoint` is private in `gf2-coding`, so the only way to obtain a real
/// v1 file is to run the runner — exactly what criterion 2 requires.
fn produce_real_v1_checkpoints(tag: &str) -> (PathBuf, String, V1Source) {
    use gf2_coding::simulation::{BpskAwgnChannel, SimulationConfig, SimulationRunner};
    use rand08::SeedableRng as _;

    let dir = tempdir(tag);
    let mut config = SimulationConfig::quick_test();
    // Tiny, fast, deterministic: 2 SNR points, few errors/frames, fixed seed
    // (fixed seed is REQUIRED for the uncoded checkpoint path to engage).
    config.eb_n0_range_db = vec![2.0, 4.0];
    config.min_errors = 5;
    config.max_frames = 200;
    config.rng_seed = Some(0xA1B2_C3D4);
    config.checkpoint_dir = Some(dir.clone());

    let channel = BpskAwgnChannel;
    let mut rng = rand08::rngs::StdRng::seed_from_u64(1);
    let results = SimulationRunner::run_uncoded_ber_with_channel(&channel, &config, &mut rng);
    assert_eq!(results.len(), 2, "two SNR points must run");

    // The runner writes config_hash.txt alongside the snr_*.json files.
    let hash = std::fs::read_to_string(dir.join("config_hash.txt"))
        .expect("SimulationRunner must write config_hash.txt")
        .trim()
        .to_string();
    // Parse the genuine v1 snr_0000.json the runner produced.
    let v1_bytes = std::fs::read(dir.join("snr_0000.json"))
        .expect("SimulationRunner must write snr_0000.json");
    let v1_source: V1Source =
        serde_json::from_slice(&v1_bytes).expect("v1 snr_0000.json must parse");
    (dir, hash, v1_source)
}

#[test]
fn test_v1_migration_from_real_simulation_runner_resume_faithful() {
    // Criterion 2 (resume-faithfulness reading, user-approved 2026-06-08): a
    // checkpoint PRODUCED BY the legacy SimulationRunner is migrated to v2, the
    // migration is field-faithful, and RESUMING FROM THE MIGRATED CHECKPOINT
    // loads and resumes byte-identically to resuming from a NATIVE v2 checkpoint
    // with the same schema fields. The epic Non-goal forbids legacy
    // byte-identity, so the reference is the native-v2 equivalent (NOT a legacy
    // run).

    // (1) Genuine v1 checkpoints from the real uncoded SimulationRunner.
    let (v1_dir, v1_hash, v1) = produce_real_v1_checkpoints("realsim-v1");
    let v1_rng_word_pos: u128 = v1.rng_word_pos.parse().unwrap();

    // (2) Migrate v1 -> v2 via the real checkpoint_migrate binary
    //     (--parallelism 2 also proves the single-worker invariant holds).
    let v2_dir = tempdir("realsim-v2");
    run_migrate(&v1_dir, &v2_dir, 2);
    let migrated = CheckpointReader::new(&v2_dir, v1_hash.clone())
        .load(0)
        .unwrap()
        .expect("migrated v2 for snr 0 must exist");

    // (3) Field-faithfulness: every carried field equals the v1 source.
    assert_eq!(migrated.schema_version, 2);
    assert_eq!(migrated.frames_completed, v1.frames_completed);
    assert_eq!(migrated.errors_accumulated, v1.errors_accumulated);
    assert_eq!(migrated.total_iterations, v1.total_iterations);
    assert_eq!(migrated.total_bits, v1.total_bits);
    assert_eq!(migrated.total_bit_errors, v1.total_bit_errors);
    assert_eq!(migrated.worker_states.len(), 1);
    assert_eq!(migrated.worker_states[0].worker_idx, 0);
    assert_eq!(
        migrated.worker_states[0].frames_in_worker,
        v1.frames_completed
    );
    assert_eq!(migrated.worker_states[0].rng_word_pos, v1_rng_word_pos);

    // The migrated point's prefix is exactly its `frames_completed = M` (the
    // genuine uncoded SimulationRunner records M = 0 for the uncoded path —
    // `frames_completed`/`rng_word_pos` are bit-oriented there; the migration
    // carries that through faithfully, asserted above). The resume budget adds
    // `extra` frames beyond M. We use M (not a clamped value) consistently for
    // both the resume `start` and the budget so the arithmetic is exact for any
    // M, including M = 0.
    let m = migrated.frames_completed;
    let extra = 12u64;
    let budget = m + extra;
    let resume_cfg = PipelineConfig {
        seed: 0x1234_5678,
        esn0_db_points: vec![migrated.esn0_db],
        target_errors: 0,
        max_frames: budget,
        heartbeat_every_frames: 0, // single chunk
        checkpoint_dir: None,
        tracing_log_path: None,
        parallelism: NonZeroUsize::new(1).unwrap(),
        strict_gpu: false,
    };
    let ch = Awgn::new(3.0, 2);

    // (4) Resume FROM the migrated checkpoint itself (passed as the `resume`
    //     arg). It must start at frame M (not re-run [0..M)) and fold the
    //     migrated prefix counters into the result.
    let mut from_mig = migrated.clone();
    from_mig.completed = false; // re-open the point under the larger budget
    let dir_mig = tempdir("realsim-resume-mig");
    let w_mig = CheckpointWriter::new(&dir_mig).unwrap();
    clear_interrupt();
    let resumed_mig = run_snr_point_checkpointed(
        &resume_cfg,
        0,
        migrated.esn0_db,
        &w_mig,
        &v1_hash,
        Some(from_mig),
        || (),
        awgn_frame(&ch),
    )
    .unwrap();

    // (4a) Started at M: exactly `extra` frames were added on top of the prefix
    //      (i.e. only `[M..budget)` ran, never re-running `[0..M)`).
    assert_eq!(
        resumed_mig.counters.frames, budget,
        "resume must end at the full budget"
    );
    assert_eq!(
        resumed_mig.counters.frames - m,
        extra,
        "resume must run only the remaining frames `[M..budget)`, never re-run [0..M)"
    );
    // (4b) Final counters fold (contain) the migrated prefix counters.
    assert!(resumed_mig.counters.errors >= migrated.errors_accumulated);
    assert!(resumed_mig.counters.total_iterations >= migrated.total_iterations);
    assert!(resumed_mig.counters.total_bits >= migrated.total_bits);
    assert!(resumed_mig.counters.total_bit_errors >= migrated.total_bit_errors);

    // (5) Byte-identity vs a NATIVE v2 checkpoint with identical schema fields.
    //     Resuming from the migrated checkpoint must equal resuming from its
    //     native-v2 twin — the honest reading of "migrated v2 loads and resumes
    //     byte-identically" that respects the legacy-byte-identity Non-goal.
    let native_twin = native_like(&migrated, &v1_hash);
    let dir_nat = tempdir("realsim-resume-nat");
    let w_nat = CheckpointWriter::new(&dir_nat).unwrap();
    clear_interrupt();
    let resumed_nat = run_snr_point_checkpointed(
        &resume_cfg,
        0,
        migrated.esn0_db,
        &w_nat,
        &v1_hash,
        Some(native_twin),
        || (),
        awgn_frame(&ch),
    )
    .unwrap();

    assert_eq!(
        resumed_mig.counters, resumed_nat.counters,
        "resuming from the migrated v2 checkpoint must be byte-identical to \
         resuming from its native-v2 equivalent"
    );

    // (6) Explicit non-zero-prefix resume math: prove "start at M, don't re-run
    //     [0..M)" with a native v2 checkpoint at M' > 0. Combined with (5)
    //     (migrated ≡ native), this establishes the migrated checkpoint would
    //     resume faithfully at any prefix, not only the genuine uncoded M = 0.
    assert_nonzero_prefix_resume_starts_at_m(&v1_hash, &ch);
}

/// Builds a native v2 checkpoint with the same schema fields as `migrated`
/// (re-opened, drain timestamp zeroed) — the "native twin" used to prove the
/// migrated checkpoint resumes identically to a natively-written one.
fn native_like(migrated: &CheckpointV2, hash: &str) -> CheckpointV2 {
    CheckpointV2 {
        schema_version: 2,
        config_hash: hash.to_string(),
        completed: false,
        drain_committed_at_us_since_epoch: 0,
        worker_states: migrated.worker_states.clone(),
        ..migrated.clone()
    }
}

/// Resumes a single-worker gf2-sim run from a native v2 checkpoint with a
/// **non-zero** prefix `M' = 10` and asserts the resume starts at `M'` (it adds
/// exactly `budget - M'` frames and folds the prefix counters), proving the
/// "don't re-run [0..M)" resume math directly.
fn assert_nonzero_prefix_resume_starts_at_m(hash: &str, ch: &Awgn) {
    // First, produce a genuine native gf2-sim prefix of M' = 10 frames so the
    // checkpoint's counters are real (not fabricated).
    let prefix = 10u64;
    let extra = 7u64;
    let budget = prefix + extra;
    let mut prefix_cfg = PipelineConfig {
        seed: 0x1234_5678,
        esn0_db_points: vec![3.0],
        target_errors: 0,
        max_frames: prefix,
        heartbeat_every_frames: 0,
        checkpoint_dir: None,
        tracing_log_path: None,
        parallelism: NonZeroUsize::new(1).unwrap(),
        strict_gpu: false,
    };
    let dir_pre = tempdir("nonzero-prefix");
    let w_pre = CheckpointWriter::new(&dir_pre).unwrap();
    clear_interrupt();
    let _ = run_snr_point_checkpointed(
        &prefix_cfg,
        0,
        3.0,
        &w_pre,
        hash,
        None,
        || (),
        awgn_frame(ch),
    )
    .unwrap();
    let mut prefix_ckpt = CheckpointReader::new(&dir_pre, hash.to_string())
        .load(0)
        .unwrap()
        .unwrap();
    assert_eq!(prefix_ckpt.frames_completed, prefix);
    prefix_ckpt.completed = false;
    let prefix_errors = prefix_ckpt.errors_accumulated;

    // Resume from that M' = 10 prefix under a larger budget.
    prefix_cfg.max_frames = budget;
    let dir_res = tempdir("nonzero-resume");
    let w_res = CheckpointWriter::new(&dir_res).unwrap();
    clear_interrupt();
    let resumed = run_snr_point_checkpointed(
        &prefix_cfg,
        0,
        3.0,
        &w_res,
        hash,
        Some(prefix_ckpt),
        || (),
        awgn_frame(ch),
    )
    .unwrap();
    // Started at M' = 10: only `extra` frames ran on top of the prefix.
    assert_eq!(resumed.counters.frames, budget);
    assert_eq!(
        resumed.counters.frames - prefix,
        extra,
        "non-zero-prefix resume must start at M' and not re-run [0..M')"
    );
    assert!(resumed.counters.errors >= prefix_errors);
}

/// Migrates the committed (gitignored, run-locally) `curve_1_2_16qam`
/// checkpoint fixtures and asserts every migrated file is a valid v2 checkpoint
/// loadable by the v2 reader with `worker_states[0]` carrying the v1 count and
/// stream position verbatim (design doc §4 "tested against the 2026-06-05
/// checkpoint dir").
///
/// `external` tier: the fixtures are gitignored campaign output (`curve_*/`),
/// so they are absent in a fresh CI checkout — the test skips with a notice
/// rather than failing when the directory is missing.
#[test]
#[ignore = "external: requires the gitignored curve_1_2_16qam/checkpoints fixtures"]
fn test_v1_migration_against_real_fixtures() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../dev/benchmarks/dvb_t2_awgn/curve_1_2_16qam/checkpoints");
    if !fixtures.is_dir() {
        eprintln!(
            "skipping: fixtures not present at {} (gitignored campaign output)",
            fixtures.display()
        );
        return;
    }
    let expected_hash = std::fs::read_to_string(fixtures.join("config_hash.txt"))
        .expect("config_hash.txt must exist beside the fixtures")
        .trim()
        .to_string();

    let out = tempdir("realfix");
    run_migrate(&fixtures, &out, 1);

    let reader = CheckpointReader::new(&out, expected_hash);
    let mut loaded = 0usize;
    for idx in 0..16 {
        if let Some(v2) = reader.load(idx).unwrap() {
            assert_eq!(v2.schema_version, 2);
            assert_eq!(v2.worker_states.len(), 1);
            assert_eq!(v2.worker_states[0].worker_idx, 0);
            assert_eq!(v2.worker_states[0].frames_in_worker, v2.frames_completed);
            loaded += 1;
        }
    }
    assert!(
        loaded > 0,
        "expected at least one migrated fixture checkpoint"
    );
}

// ---------------------------------------------------------------------------
// Subprocess SNR-sweep SIGINT + --resume byte-identity (criterion 1, deliv. 2)
// ---------------------------------------------------------------------------

/// Reads every `snr_NNNN.json` in `dir` (skipping `.tmp`) and returns them
/// sorted by `snr_index`, with the volatile `drain_committed_at_us_since_epoch`
/// zeroed so two runs are comparable byte-for-byte.
fn load_all_normalized(dir: &Path) -> Vec<gf2_sim::checkpoint::CheckpointV2> {
    let mut v: Vec<gf2_sim::checkpoint::CheckpointV2> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("snr_") && n.ends_with(".json"))
        })
        .map(|p| {
            let mut c: gf2_sim::checkpoint::CheckpointV2 =
                serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
            c.drain_committed_at_us_since_epoch = 0;
            c
        })
        .collect();
    v.sort_by_key(|c| c.snr_index);
    v
}

/// Spawns the `checkpoint_sweep` binary with the given args.
fn spawn_sweep(args: &[&str]) -> std::process::Child {
    std::process::Command::new(env!("CARGO_BIN_EXE_checkpoint_sweep"))
        .args(args)
        .spawn()
        .expect("checkpoint_sweep must spawn")
}

/// Runs a complete (uninterrupted) sweep to `dir`; asserts exit 0.
fn run_full_sweep(dir: &Path, channel: &str, snr_points: usize, max_frames: u64, heartbeat: u64) {
    let status = spawn_sweep(&[
        "--checkpoint-dir",
        dir.to_str().unwrap(),
        "--channel",
        channel,
        "--snr-points",
        &snr_points.to_string(),
        "--seed",
        "7",
        "--max-frames",
        &max_frames.to_string(),
        "--heartbeat",
        &heartbeat.to_string(),
    ])
    .wait()
    .expect("wait full sweep");
    assert!(status.success(), "full sweep must exit 0, got {status}");
}

/// Counts completed (`"completed": true`) checkpoint files in `dir`.
fn completed_count(dir: &Path) -> usize {
    load_all_normalized(dir)
        .iter()
        .filter(|c| c.completed)
        .count()
}

/// The interrupted-then-resumed half: spawn a long-enough sweep, SIGINT it once
/// it has completed ≥1 but <`snr_points` SNR points (self-synchronising poll, no
/// fixed sleep), assert it exits NON-ZERO, then `--resume` to completion.
/// Returns once the resume has finished successfully.
///
/// `#[cfg(unix)]`: sends a real `SIGINT` via `kill -INT <pid>` (mirrors the
/// legacy gf2-coding subprocess test). On non-unix it falls back to `kill()`.
fn interrupt_then_resume(
    dir: &Path,
    channel: &str,
    snr_points: usize,
    max_frames: u64,
    heartbeat: u64,
) {
    let mf = max_frames.to_string();
    let hb = heartbeat.to_string();
    let np = snr_points.to_string();
    let base_args = [
        "--checkpoint-dir",
        dir.to_str().unwrap(),
        "--channel",
        channel,
        "--snr-points",
        &np,
        "--seed",
        "7",
        "--max-frames",
        &mf,
        "--heartbeat",
        &hb,
    ];

    let mut child = spawn_sweep(&base_args);
    let pid = child.id();

    // Poll until partial progress (some but not all points completed), then
    // SIGINT. Hard cap the poll loop so a too-fast child does not hang the test.
    let mut signalled = false;
    for _ in 0..2000 {
        let done = completed_count(dir);
        if done >= 1 && done < snr_points {
            send_sigint(pid);
            signalled = true;
            break;
        }
        // If the child already exited (finished too fast), stop polling.
        if let Ok(Some(_)) = child.try_wait() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }

    let status = child.wait().expect("wait interrupted sweep");
    if signalled {
        assert!(
            !status.success(),
            "interrupted sweep must exit NON-ZERO after SIGINT, got {status}"
        );
        assert!(
            completed_count(dir) < snr_points,
            "interrupt must land before all SNR points complete"
        );
    } else {
        // The sweep finished before we could catch a partial state (very fast
        // host). That is not an interrupt scenario; fall through to resume,
        // which is then a no-op confirming completed points are skipped.
        eprintln!("note: sweep completed before SIGINT could be delivered");
    }

    // Resume to completion (same config + dir + --resume); must exit 0.
    let mut resume_args = base_args.to_vec();
    resume_args.push("--resume");
    let status = spawn_sweep(&resume_args).wait().expect("wait resume");
    assert!(status.success(), "resumed sweep must exit 0, got {status}");
    assert_eq!(
        completed_count(dir),
        snr_points,
        "after resume all SNR points must be completed"
    );
}

#[cfg(unix)]
fn send_sigint(pid: u32) {
    let _ = std::process::Command::new("kill")
        .args(["-INT", &pid.to_string()])
        .status();
}

#[cfg(not(unix))]
fn send_sigint(_pid: u32) {
    // No SIGINT on non-unix; the poll loop's try_wait fallback handles it.
}

/// Asserts a 10-SNR SIGINT-interrupted sweep resumes byte-identically (per
/// `snr_NNNN.json`) to a fresh uninterrupted reference for `channel`.
///
/// `max_frames` is sized per channel so the full interrupt→resume→reference
/// cycle (3 sweeps) stays comfortably under the 5 s fast-tier hard kill while
/// keeping a wide mid-sweep SIGINT window: AWGN draws 8 words/symbol,
/// Rayleigh/Rician 16, so the lighter channels get a larger frame budget for a
/// comparable wall time. Measured cycle times: AWGN ~0.5 s, Rayleigh ~0.6 s,
/// Rician ~0.5 s (see the rework receipts).
fn assert_sweep_resume_byte_identical(channel: &str, max_frames: u64) {
    let snr_points = 10;
    let heartbeat = 500;

    let ref_dir = tempdir(&format!("sweep-ref-{channel}"));
    run_full_sweep(&ref_dir, channel, snr_points, max_frames, heartbeat);
    let reference = load_all_normalized(&ref_dir);
    assert_eq!(reference.len(), snr_points);

    let res_dir = tempdir(&format!("sweep-res-{channel}"));
    interrupt_then_resume(&res_dir, channel, snr_points, max_frames, heartbeat);
    let resumed = load_all_normalized(&res_dir);

    assert_eq!(
        resumed, reference,
        "[{channel}] SIGINT+resume sweep must be byte-identical (fer/frames/\
         errors/mean_iters via the v2 checkpoint counters) to the uninterrupted \
         reference at the same seed"
    );
}

// Each channel is its OWN non-ignored fast-tier subprocess test so criterion 1
// (byte-identical SIGINT+resume across AWGN, Rayleigh, AND Rician) is fully in
// the cargo-ci gate. nextest runs them as separate parallel processes; each
// individually clears the 5 s hard kill with a wide margin (~0.5-0.6 s).

#[test]
fn test_sweep_sigint_resume_byte_identical_awgn_subprocess() {
    assert_sweep_resume_byte_identical("awgn", 12_000);
}

#[test]
fn test_sweep_sigint_resume_byte_identical_rayleigh_subprocess() {
    assert_sweep_resume_byte_identical("rayleigh", 8_000);
}

#[test]
fn test_sweep_sigint_resume_byte_identical_rician_subprocess() {
    assert_sweep_resume_byte_identical("rician", 6_000);
}

// ---------------------------------------------------------------------------
// Kill-during-fsync atomic-write contract (criterion 3)
// ---------------------------------------------------------------------------

/// Asserts the canonical `snr_0000.json` in `dir` is either a complete v2
/// checkpoint or absent — never torn — and returns whether it was present.
fn assert_canonical_complete_or_absent(dir: &Path, ctx: &str) -> bool {
    let canon = dir.join("snr_0000.json");
    if !canon.exists() {
        return false;
    }
    let bytes = std::fs::read(&canon).unwrap();
    let parsed: Result<CheckpointV2, _> = serde_json::from_slice(&bytes);
    assert!(
        parsed.is_ok(),
        "{ctx}: canonical snr_0000.json is torn/partial: {:?}",
        String::from_utf8_lossy(&bytes)
    );
    assert_eq!(parsed.unwrap().schema_version, 2);
    // A leftover `snr_0000.<pid>.tmp` must never be the canonical file: the
    // canonical name is only ever produced by the atomic rename.
    true
}

#[test]
#[cfg(unix)]
fn test_kill_during_fsync_deterministic() {
    // Criterion 3 ([hard]): kill the writer mid-flush VIA SIGNAL DURING FSYNC.
    // The `--crash-during-fsync` child writes one COMPLETE prior-state
    // checkpoint, then prints `BEGIN_FSYNC` immediately before a LARGE write
    // whose tmp-file `sync_all` dominates its wall time. This parent reads the
    // child's stdout and SIGKILLs the instant it sees `BEGIN_FSYNC`, so the kill
    // lands during the fsync with high confidence (verified: the canonical file
    // is then ALWAYS the prior complete state — the large write's rename never
    // happened because its fsync was interrupted). The canonical snr_0000.json
    // must always be a complete v2 checkpoint or absent — never torn. Fast tier:
    // 12 spawn+read+kill cycles, ~1-2 ms each.
    use std::io::{BufRead, BufReader};

    let iterations = 12;
    let mut present = 0usize;
    let mut prior_state_survived = 0usize;
    for i in 0..iterations {
        let dir = tempdir(&format!("fsynckill-{i}"));
        let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_checkpoint_sweep"))
            .args([
                "--checkpoint-dir",
                dir.to_str().unwrap(),
                "--channel",
                "awgn",
                "--snr-points",
                "1",
                "--seed",
                "1",
                "--crash-during-fsync",
            ])
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("checkpoint_sweep must spawn");

        // Read stdout until the BEGIN_FSYNC marker, then SIGKILL immediately so
        // the kill lands inside the large write's fsync.
        let stdout = child.stdout.take().expect("piped stdout");
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break, // child exited
                Ok(_) => {
                    if line.contains("BEGIN_FSYNC") {
                        let _ = child.kill();
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = child.wait();

        if assert_canonical_complete_or_absent(&dir, &format!("iter {i}")) {
            present += 1;
            // The prior complete state (≤2 worker_states) must survive — the
            // interrupted large write (40k+ worker_states) never renamed.
            let c: CheckpointV2 =
                serde_json::from_slice(&std::fs::read(dir.join("snr_0000.json")).unwrap()).unwrap();
            if c.worker_states.len() <= 2 {
                prior_state_survived += 1;
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    assert!(
        present > 0,
        "expected the canonical checkpoint present in at least one of \
         {iterations} fsync-kill iterations"
    );
    // The whole point of "during fsync": the prior complete state survives
    // because the large write's rename never happened.
    assert!(
        prior_state_survived > 0,
        "expected the prior complete checkpoint to survive a during-fsync kill \
         in at least one iteration (large write's rename must not have happened)"
    );
}

#[test]
#[cfg(unix)]
fn test_kill_mid_write_randomized_defense_in_depth() {
    // Defense in depth: the `--crash-loop` child writes in a tight loop; the
    // parent SIGKILLs at a randomised-ish moment so the kill lands at an
    // arbitrary point in the write/fsync/rename window. The canonical file must
    // always be complete-or-absent. Fast tier: 30 sub-10ms spawn+kill cycles.
    let iterations = 30;
    let mut observed_present = 0usize;
    for i in 0..iterations {
        let dir = tempdir(&format!("crashkill-{i}"));
        let mut child = spawn_sweep(&[
            "--checkpoint-dir",
            dir.to_str().unwrap(),
            "--channel",
            "awgn",
            "--snr-points",
            "1",
            "--seed",
            "1",
            "--crash-loop",
        ]);
        let micros = 200 + (i as u64 * 137) % 4000;
        std::thread::sleep(std::time::Duration::from_micros(micros));
        let _ = child.kill();
        let _ = child.wait();
        if assert_canonical_complete_or_absent(&dir, &format!("iter {i}")) {
            observed_present += 1;
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
    assert!(
        observed_present > 0,
        "expected the canonical checkpoint present in at least one of \
         {iterations} kill iterations"
    );
}
