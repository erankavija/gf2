//! v2 checkpoint resume byte-identity integration tests
//! (issue `5f12e7ff`, design doc §4).
//!
//! The checkpoint format is v2-only (a single schema). Coverage:
//!
//! * **v2 library-level resume** (`test_v2_resume_*`): a v2 checkpoint written
//!   mid-point loads and resumes N-worker, with byte-identical
//!   `fer`/`frames`/`errors`/`mean_iters` vs an uninterrupted N-worker
//!   reference, on AWGN, Rayleigh, and Rician channels.
//! * **Subprocess SNR-sweep MID-POINT SIGINT + `--resume`**
//!   (`test_sweep_sigint_*`): one non-ignored fast-tier test PER CHANNEL (AWGN,
//!   Rayleigh, Rician) spawns the `checkpoint_sweep` binary and sends a real
//!   SIGINT on the FIRST within-point `HEARTBEAT_<snr>_<frames>` marker (so the
//!   signal lands while a point is still simulating, triggering a within-point
//!   heartbeat flush + drain + per-worker-state latch — not a between-points
//!   boundary flush). It asserts a NON-ZERO (130) child exit, that the
//!   interrupted point's checkpoint is genuinely mid-point
//!   (`0 < frames_completed < max_frames`, `!completed`, per-worker state
//!   summing to `frames_completed`), then `--resume`s to completion and asserts
//!   the full 10-SNR sweep is byte-identical to an uninterrupted reference. All
//!   three are in the cargo-ci gate (criterion 1 covers all three channels).
//! * **Kill-mid-flush** (`test_kill_during_fsync_deterministic`): reads the
//!   `--crash-during-fsync` child's `BEGIN_FSYNC` marker and SIGKILLs it, so the
//!   kill lands mid-flush (after the tmp bytes are written, before the atomic
//!   rename — the amended criterion-3 durability contract); asserts the
//!   canonical file is always a complete v2 checkpoint or absent — never torn,
//!   and that the prior complete state survives. A randomised
//!   `--crash-loop` variant adds defense in depth.
//!
//! The library-level resume tests use **small frame counts** so the fast tier
//! (5 s/test) is honoured; resume byte-identity is independent of frame count
//! (every frame's outcome is a pure function of its global index, design §3).
//! The subprocess sweep uses several heartbeat chunks/point so a within-point
//! SIGINT window exists, but stays fast (~1 s/test).

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

/// A tempdir under Cargo's `CARGO_TARGET_TMPDIR` (inside `target/`), which lives
/// on the build's real backing filesystem — NOT a RAM-backed `tmpfs` like
/// `/tmp` (where `fsync`/`sync_all` is a no-op). The kill-during-fsync test
/// needs `sync_all` to do real, slow disk I/O so the SIGKILL can land inside it,
/// so it uses this instead of [`tempdir`].
#[cfg(unix)]
fn tempdir_real_fs(tag: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
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
        gpu_enabled: false,
        strict_gpu: false,
        diagnostic_dump_dir: None,
        inject_gpu_oom_modulus: None,
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
        run_snr_point_checkpointed(&full, 0, 6.25, &w_ref, &h, None, || (), &frame, |_, _| {})
            .unwrap();
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
    let partial = run_snr_point_checkpointed(
        &partial_cfg,
        0,
        6.25,
        &writer,
        &h,
        None,
        || (),
        &frame,
        |_, _| {},
    )
    .unwrap();
    assert_eq!(partial.counters.frames, 13);

    let reader = CheckpointReader::new(&dir, h.clone());
    let mut loaded = reader.load(0).unwrap().unwrap();
    // Re-open the point under the full budget for resume.
    loaded.completed = false;
    let resumed = run_snr_point_checkpointed(
        &full,
        0,
        6.25,
        &writer,
        &h,
        Some(loaded),
        || (),
        &frame,
        |_, _| {},
    )
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
        run_snr_point_checkpointed(&c, 0, 6.25, &w, &h, None, || (), awgn_frame(&ch), |_, _| {})
            .unwrap();
    assert_eq!(run.counters.frames, 20);
    assert!(
        run.counters.errors > 0,
        "low-Es/N0 AWGN must produce frame errors; got {:?}",
        run.counters
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

/// Spawns the `checkpoint_sweep` binary with stdout piped (so the parent can
/// read the `HEARTBEAT_<snr>_<frames>` and `SNR_<idx>_FLUSHED` progress
/// markers).
fn spawn_sweep_piped(args: &[&str]) -> std::process::Child {
    std::process::Command::new(env!("CARGO_BIN_EXE_checkpoint_sweep"))
        .args(args)
        .stdout(std::process::Stdio::piped())
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

/// The interrupted-then-resumed half. GUARANTEES the SIGINT lands MID-POINT
/// (while a point is still simulating), not between points:
///
/// 1. Spawn the sweep with NO `--point-delay-ms` (so the signal cannot land in
///    a between-point sleep), and a small `heartbeat_every_frames` so each SNR
///    point performs SEVERAL within-point heartbeat flushes.
/// 2. Read the child's stdout for the FIRST `HEARTBEAT_<snr>_<frames>` marker —
///    emitted only on a WITHIN-point (non-final) checkpoint flush, so the point
///    is provably mid-simulation — then SIGINT immediately.
/// 3. HARD-FAIL (panic) if: no `HEARTBEAT_` marker appears before the child
///    exits, OR the child exits successfully (status 0) instead of via the 130
///    interrupt path, OR the interrupted point's checkpoint shows
///    `frames_completed == 0` or `== max_frames` (not genuinely mid-point).
/// 4. ASSERT the interrupted point's checkpoint has
///    `0 < frames_completed < max_frames` (a mid-point heartbeat flush, with
///    per-worker state, triggered by the SIGINT) and `completed == false`.
/// 5. `--resume` to completion; assert exit 0 and that all points are complete.
///
/// There is NO log-and-continue fallback: an undelivered or between-points
/// interrupt fails the test.
///
/// `#[cfg(unix)]`: sends a real `SIGINT` via `kill -INT <pid>`.
fn interrupt_then_resume(
    dir: &Path,
    channel: &str,
    snr_points: usize,
    max_frames: u64,
    heartbeat: u64,
) {
    use std::io::{BufRead, BufReader};

    let mf = max_frames.to_string();
    let hb = heartbeat.to_string();
    let np = snr_points.to_string();
    // No `--point-delay-ms`: the only wide window is the within-point
    // simulation, so the SIGINT lands mid-point. `max_frames / heartbeat >= 2`
    // guarantees at least one within-point (non-final) heartbeat flush.
    assert!(
        max_frames / heartbeat >= 2,
        "[{channel}] need >=2 heartbeat chunks/point for a within-point flush"
    );
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

    // The interrupted child ALSO gets `--block-at-first-heartbeat`: it parks at
    // its first within-point heartbeat flush (snr 0) until the signal lands, so a
    // fast/idle host cannot finish the point before the parent delivers the
    // SIGINT (without it the parent reads buffered stdout while the child races
    // ahead, making the interrupted point nondeterministic). The flag is NOT in
    // `base_args` because the `--resume` child below must run to completion, not
    // block.
    let mut interrupt_args = base_args.to_vec();
    interrupt_args.push("--block-at-first-heartbeat");
    let mut child = spawn_sweep_piped(&interrupt_args);
    let pid = child.id();
    let stdout = child.stdout.take().expect("piped stdout");
    let mut reader = BufReader::new(stdout);

    // Read until the FIRST `HEARTBEAT_<snr>_<frames>` marker (a within-point
    // flush ⇒ provably mid-simulation), then SIGINT immediately. Capture the
    // interrupted point's snr index.
    let mut interrupted_snr: Option<usize> = None;
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break, // child closed stdout / exited
            Ok(_) => {
                if let Some(rest) = line.trim().strip_prefix("HEARTBEAT_") {
                    let snr: usize = rest
                        .split('_')
                        .next()
                        .and_then(|s| s.parse().ok())
                        .expect("HEARTBEAT_<snr>_<frames> marker");
                    send_sigint(pid);
                    interrupted_snr = Some(snr);
                    break;
                }
            }
            Err(_) => break,
        }
    }

    // HARD requirement: a within-point heartbeat marker must have appeared. If
    // not, the child never flushed mid-point — cannot guarantee a mid-point
    // SIGINT.
    let snr = interrupted_snr.unwrap_or_else(|| {
        panic!(
            "[{channel}] no HEARTBEAT_<snr>_<frames> marker before the child \
             exited; cannot guarantee a mid-point SIGINT"
        )
    });

    let status = child.wait().expect("wait interrupted sweep");
    // HARD requirement: the child must have exited via the interrupt path
    // (non-zero / 130), never a clean success.
    assert!(
        !status.success(),
        "[{channel}] interrupted sweep must exit NON-ZERO after a mid-point \
         SIGINT, got {status}"
    );
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        assert_eq!(
            status.code(),
            Some(130),
            "[{channel}] interrupted sweep must exit 130 (SIGINT path); \
             got code={:?} signal={:?}",
            status.code(),
            status.signal()
        );
    }

    // HARD requirement: the interrupted point's checkpoint is a genuine MID-POINT
    // flush: 0 < frames_completed < max_frames, not completed, with per-worker
    // state. This proves the SIGINT triggered a within-point heartbeat flush +
    // drain + per-worker-state latching (criterion 1, design §4) — NOT just a
    // between-points SNR-boundary flush.
    let ck = load_all_normalized(dir)
        .into_iter()
        .find(|c| c.snr_index == snr)
        .unwrap_or_else(|| panic!("[{channel}] interrupted point snr {snr} has no checkpoint"));
    assert!(
        ck.frames_completed > 0 && ck.frames_completed < max_frames,
        "[{channel}] interrupted point snr {snr} must be mid-point: \
         0 < frames_completed ({}) < max_frames ({max_frames})",
        ck.frames_completed
    );
    assert!(
        !ck.completed,
        "[{channel}] interrupted point snr {snr} must not be completed"
    );
    assert!(
        !ck.worker_states.is_empty(),
        "[{channel}] mid-point checkpoint must carry per-worker state"
    );
    let ws_sum: u64 = ck.worker_states.iter().map(|w| w.frames_in_worker).sum();
    assert_eq!(
        ws_sum, ck.frames_completed,
        "[{channel}] per-worker frames_in_worker must sum to frames_completed"
    );

    // Resume to completion (same config + dir + --resume); exit 0.
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
    // No SIGINT on non-unix; the unix-only assertions in interrupt_then_resume
    // are `#[cfg(unix)]`, and the marker read still drives the flow.
}

/// Asserts a 10-SNR MID-POINT-SIGINT-interrupted sweep resumes byte-identically
/// (per `snr_NNNN.json`) to a fresh uninterrupted reference for `channel`.
///
/// The SIGINT lands WITHIN a point's simulation: `heartbeat = max_frames / 4`
/// gives 4 chunks/point, so each point performs within-point heartbeat flushes;
/// [`interrupt_then_resume`] signals on the first one. `max_frames` is small and
/// uniform, keeping all three sweeps (reference, interrupt, resume) fast.
/// Measured per-test cycle: ~0.9-1.5 s.
fn assert_sweep_resume_byte_identical(channel: &str, max_frames: u64) {
    let snr_points = 10;
    let heartbeat = max_frames / 4;

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
// individually clears the 5 s hard kill with a wide margin (~0.7-0.9 s).

#[test]
fn test_sweep_sigint_resume_byte_identical_awgn_subprocess() {
    assert_sweep_resume_byte_identical("awgn", 2_000);
}

#[test]
fn test_sweep_sigint_resume_byte_identical_rayleigh_subprocess() {
    assert_sweep_resume_byte_identical("rayleigh", 2_000);
}

#[test]
fn test_sweep_sigint_resume_byte_identical_rician_subprocess() {
    assert_sweep_resume_byte_identical("rician", 2_000);
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
    // Criterion 3 ([hard], amended 2026-06-08): the durability contract is "kill
    // the writer MID-FLUSH — after the tmp bytes are written and BEFORE the
    // atomic rename". The canonical checkpoint must then be either the complete
    // prior-state checkpoint or absent — NEVER a torn/partial JSON.
    //
    // This test demonstrates the strongest form of that window: the
    // `--crash-during-fsync` child writes one COMPLETE prior-state checkpoint,
    // then for a >=64 MiB write fires `CheckpointWriter`'s pre-fsync hook AFTER
    // the tmp bytes are written and immediately before `sync_all`: it prints
    // `BEGIN_FSYNC` and returns at once (no sleep). The parent reads the child's
    // stdout and SIGKILLs the instant it sees `BEGIN_FSYNC`. The checkpoint dir
    // is on a REAL filesystem (`CARGO_TARGET_TMPDIR` under `target/`), not
    // RAM-backed `tmpfs` (`/tmp`, where `sync_all` is a no-op), so the >=64 MiB
    // `sync_all` does real, hundreds-of-ms disk I/O — the kill therefore lands
    // within that real `sync_all`, which is squarely inside the amended window
    // (after the tmp bytes, before the rename).
    //
    // The assertion is correct for ANY kill before the atomic rename: the
    // canonical snr_0000.json is the prior complete v2 checkpoint or absent —
    // never torn, and never the large write (whose rename happens only after a
    // successful `sync_all`, which the kill interrupts). The large fsync simply
    // pins the kill into the after-write/before-rename window robustly.
    // Few iterations: each does a 64 MiB write + a real (hundreds-of-ms)
    // sync_all, so 3 keeps the test well under the 5 s fast-tier limit.
    use std::io::{BufRead, BufReader};

    let iterations = 3;
    let mut present = 0usize;
    let mut prior_state_survived = 0usize;
    for i in 0..iterations {
        let dir = tempdir_real_fs(&format!("fsynckill-{i}"));
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
        // the kill lands inside the large write's real `sync_all`.
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
            // The prior complete state (<=2 worker_states) must survive — the
            // interrupted large write (700k worker_states) never renamed.
            let c: CheckpointV2 =
                serde_json::from_slice(&std::fs::read(dir.join("snr_0000.json")).unwrap()).unwrap();
            if c.worker_states.len() <= 2 {
                prior_state_survived += 1;
            }
        }
        // Clean up the large tmp/canonical files so disk isn't bloated.
        let _ = std::fs::remove_dir_all(&dir);
    }

    assert!(
        present > 0,
        "expected the canonical checkpoint present in at least one of \
         {iterations} fsync-kill iterations"
    );
    // The whole point of "during fsync": the prior complete state survives
    // because the large write's rename never happened (the kill interrupted its
    // `sync_all`). Every present iteration must show the prior state (never the
    // large write's payload).
    assert_eq!(
        prior_state_survived, present,
        "every during-fsync kill must leave the prior complete checkpoint \
         (the large write's rename must not have happened): \
         prior_state_survived={prior_state_survived} present={present}"
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
