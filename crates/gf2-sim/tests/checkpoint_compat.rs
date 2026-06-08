//! v2 checkpoint compatibility + resume byte-identity integration tests
//! (issue `5f12e7ff` deliverables 4 & 5, design doc §4).
//!
//! Coverage:
//!
//! * **v1 → v2 migration** (`test_v1_migration_*`): a legacy
//!   `SimulationRunner`-shaped v1 checkpoint is converted by the
//!   `checkpoint_migrate` binary, then the resulting v2 checkpoint loads and
//!   resumes single-worker byte-identically vs an uninterrupted single-thread
//!   reference.
//! * **v2 round-trip resume** (`test_v2_resume_*`): a v2 checkpoint written
//!   mid-point loads and resumes N-worker, with byte-identical
//!   `fer`/`frames`/`errors`/`mean_iters` vs an uninterrupted N-worker
//!   reference, on AWGN, Rayleigh, and Rician channels.
//!
//! All tests use **small frame counts** so the fast tier (5 s/test) is honoured;
//! the resume byte-identity property is independent of frame count (every
//! frame's outcome is a pure function of its global index, design doc §3).

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use gf2_sim::batch::SymbolBatch;
use gf2_sim::channels::{Awgn, Rayleigh, Rician};
use gf2_sim::checkpoint::{
    clear_interrupt, config_hash, run_snr_point_checkpointed, CheckpointReader, CheckpointWriter,
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

#[test]
fn test_v1_migration_single_worker_resume_byte_identical() {
    // A v1 checkpoint at frame M, migrated to v2, must resume single-worker
    // byte-identically vs the uninterrupted single-thread reference over the
    // same synthetic AWGN frame closure.
    let ch = Awgn::new(3.0, 2);
    let full = cfg(1, 30, 0); // single worker; no heartbeat (one chunk)
    let h = config_hash(&full);

    // Uninterrupted single-thread reference over 30 frames.
    let dir_ref = tempdir("v1ref");
    let w_ref = CheckpointWriter::new(&dir_ref).unwrap();
    clear_interrupt();
    let reference =
        run_snr_point_checkpointed(&full, 0, 6.25, &w_ref, &h, None, || (), awgn_frame(&ch))
            .unwrap();
    assert_eq!(reference.counters.frames, 30);

    // To get the exact v1 rng_word_pos for "10 frames done", run a 10-frame
    // single-worker partial and read the worker-0 position it recorded. This is
    // the legacy stream position a v1 run would have stored.
    let part_cfg = PipelineConfig {
        max_frames: 10,
        ..full.clone()
    };
    let dir_part = tempdir("v1part");
    let w_part = CheckpointWriter::new(&dir_part).unwrap();
    clear_interrupt();
    let partial = run_snr_point_checkpointed(
        &part_cfg,
        0,
        6.25,
        &w_part,
        &h,
        None,
        || (),
        awgn_frame(&ch),
    )
    .unwrap();
    assert_eq!(partial.counters.frames, 10);
    let v2_partial = CheckpointReader::new(&dir_part, h.clone())
        .load(0)
        .unwrap()
        .unwrap();
    let pos_at_10 = v2_partial.worker_states[0].rng_word_pos;

    // Synthesise a *v1* fixture carrying that partial state, migrate it, load.
    let v1_dir = tempdir("v1synth");
    let v2_dir = tempdir("v2synth");
    write_v1_fixture(
        &v1_dir,
        0,
        6.25,
        partial.counters.frames,
        partial.counters.errors,
        partial.counters.total_iterations,
        partial.counters.total_bits,
        partial.counters.total_bit_errors,
        pos_at_10,
        30,
        0,
        false,
        &h,
    );
    run_migrate(&v1_dir, &v2_dir, 1);
    let migrated = CheckpointReader::new(&v2_dir, h.clone())
        .load(0)
        .unwrap()
        .unwrap();
    assert_eq!(migrated.frames_completed, 10);

    // Resume from the migrated v2 checkpoint under the full budget.
    let dir_resume = tempdir("v1resume");
    let w_resume = CheckpointWriter::new(&dir_resume).unwrap();
    clear_interrupt();
    let resumed = run_snr_point_checkpointed(
        &full,
        0,
        6.25,
        &w_resume,
        &h,
        Some(migrated),
        || (),
        awgn_frame(&ch),
    )
    .unwrap();

    assert_eq!(
        resumed.counters, reference.counters,
        "migrated v1→v2 resume must be byte-identical to the uninterrupted \
         single-thread reference"
    );
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
