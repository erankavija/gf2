//! Checkpoint resume tests on AWGN, Rayleigh and Rician channels: a
//! library-level or SIGINT-interrupted run resumes byte-identically to an
//! uninterrupted one, and a writer killed mid-flush leaves the canonical
//! checkpoint complete or absent.

use std::num::NonZeroUsize;
use std::path::Path;

use gf2_sim::batch::SymbolBatch;
use gf2_sim::channels::{Awgn, Rayleigh, Rician};
use gf2_sim::parallel::{FrameOutcome, WorkerCtx};
use gf2_sim::snr_checkpoint::{
    clear_interrupt, config_hash, run_snr_point_checkpointed, CheckpointReader, CheckpointV2,
    CheckpointWriter,
};
use gf2_sim::PipelineConfig;

fn tempdir(tag: &str) -> gf2_core::test_scratch::Scratch {
    gf2_core::test_scratch::scratch(&format!("gf2sim-ckcompat-{tag}"))
}

fn checkpoint_payload(bytes: &[u8]) -> serde_json::Result<CheckpointV2> {
    let envelope: serde_json::Value = serde_json::from_slice(bytes)?;
    serde_json::from_value(envelope["payload"].clone())
}

/// A scratch dir under `CARGO_TARGET_TMPDIR`, on a disk-backed filesystem, so
/// `sync_all` does real I/O for the SIGKILL to land in (a no-op on tmpfs).
#[cfg(unix)]
fn tempdir_real_fs(tag: &str) -> gf2_core::test_scratch::Scratch {
    gf2_core::test_scratch::scratch_in(
        Path::new(env!("CARGO_TARGET_TMPDIR")),
        &format!("gf2sim-ckcompat-{tag}"),
    )
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

/// The pattern is the same for every frame; per-frame variation comes only
/// from the channel's RNG draws.
fn signal_batch(n: usize) -> SymbolBatch {
    let i: Vec<f32> = (0..n)
        .map(|k| if k % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let q: Vec<f32> = vec![0.0; n];
    SymbolBatch::new(vec![i], vec![q])
}

const SYMS_PER_FRAME: usize = 64;

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
        iterations: 1 + bit_errors,
        info_bits: SYMS_PER_FRAME as u64,
        bit_errors,
    }
}

fn assert_resume_byte_identical<F>(tag: &str, parallelism: usize, frame: F)
where
    F: Fn(usize, &mut WorkerCtx, &mut ()) -> FrameOutcome + Sync,
{
    let full = cfg(parallelism, 40, 13);
    let h = config_hash(&full);

    let dir_ref = tempdir(&format!("{tag}-ref"));
    let w_ref = CheckpointWriter::new(dir_ref.path()).unwrap();
    clear_interrupt();
    let reference =
        run_snr_point_checkpointed(&full, 0, 6.25, &w_ref, &h, None, || (), &frame, |_, _| {})
            .unwrap();
    assert!(reference.completed);
    assert_eq!(reference.counters.frames, 40);

    let dir = tempdir(&format!("{tag}-resume"));
    let writer = CheckpointWriter::new(dir.path()).unwrap();
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

    let reader = CheckpointReader::new(dir.path(), h.clone());
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
    let ch = Awgn::new(0.5, 2);
    let c = cfg(2, 20, 7);
    let h = config_hash(&c);
    let dir = tempdir("nonzero");
    let w = CheckpointWriter::new(dir.path()).unwrap();
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

#[test]
fn test_snr_checkpoint_uses_generic_envelope() {
    let c = cfg(1, 1, 1);
    let h = config_hash(&c);
    let dir = tempdir("generic-envelope");
    let writer = CheckpointWriter::new(dir.path()).unwrap();
    let ch = Awgn::new(0.0, 1);
    run_snr_point_checkpointed(
        &c,
        0,
        6.25,
        &writer,
        &h,
        None,
        || (),
        awgn_frame(&ch),
        |_, _| {},
    )
    .unwrap();

    let stored: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("snr_0000.json")).unwrap()).unwrap();
    assert_eq!(
        stored["payload_identity"],
        serde_json::json!("gf2-sim/snr-checkpoint-v2")
    );
    assert!(stored["payload"].is_object());
}

/// Zeroes the volatile `drain_committed_at_us_since_epoch` so two runs compare
/// equal.
fn load_all_normalized(dir: &Path) -> Vec<gf2_sim::snr_checkpoint::CheckpointV2> {
    let mut v: Vec<gf2_sim::snr_checkpoint::CheckpointV2> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("snr_") && n.ends_with(".json"))
        })
        .map(|p| {
            let mut c: gf2_sim::snr_checkpoint::CheckpointV2 =
                checkpoint_payload(&std::fs::read(&p).unwrap()).unwrap();
            c.drain_committed_at_us_since_epoch = 0;
            c
        })
        .collect();
    v.sort_by_key(|c| c.snr_index);
    v
}

fn spawn_sweep(args: &[&str]) -> std::process::Child {
    std::process::Command::new(env!("CARGO_BIN_EXE_checkpoint_sweep"))
        .args(args)
        .spawn()
        .expect("checkpoint_sweep must spawn")
}

fn spawn_sweep_piped(args: &[&str]) -> std::process::Child {
    std::process::Command::new(env!("CARGO_BIN_EXE_checkpoint_sweep"))
        .args(args)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("checkpoint_sweep must spawn")
}

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

fn completed_count(dir: &Path) -> usize {
    load_all_normalized(dir)
        .iter()
        .filter(|c| c.completed)
        .count()
}

/// Sends SIGINT on the child's first `HEARTBEAT_<snr>_<frames>` marker, which
/// the binary prints on a within-point checkpoint flush, then resumes the
/// sweep to completion.
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

    // `--block-at-first-heartbeat` parks the child at its first within-point
    // flush until the signal lands, so it cannot finish the point first.
    let mut interrupt_args = base_args.to_vec();
    interrupt_args.push("--block-at-first-heartbeat");
    let mut child = spawn_sweep_piped(&interrupt_args);
    let pid = child.id();
    let stdout = child.stdout.take().expect("piped stdout");
    let mut reader = BufReader::new(stdout);

    let mut interrupted_snr: Option<usize> = None;
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
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

    let snr = interrupted_snr.unwrap_or_else(|| {
        panic!(
            "[{channel}] no HEARTBEAT_<snr>_<frames> marker before the child \
             exited; cannot guarantee a mid-point SIGINT"
        )
    });

    let status = child.wait().expect("wait interrupted sweep");
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
fn send_sigint(_pid: u32) {}

fn assert_sweep_resume_byte_identical(channel: &str, max_frames: u64) {
    let snr_points = 10;
    let heartbeat = max_frames / 4;

    let ref_dir = tempdir(&format!("sweep-ref-{channel}"));
    run_full_sweep(ref_dir.path(), channel, snr_points, max_frames, heartbeat);
    let reference = load_all_normalized(ref_dir.path());
    assert_eq!(reference.len(), snr_points);

    let res_dir = tempdir(&format!("sweep-res-{channel}"));
    interrupt_then_resume(res_dir.path(), channel, snr_points, max_frames, heartbeat);
    let resumed = load_all_normalized(res_dir.path());

    assert_eq!(
        resumed, reference,
        "[{channel}] SIGINT+resume sweep must be byte-identical (fer/frames/\
         errors/mean_iters via the v2 checkpoint counters) to the uninterrupted \
         reference at the same seed"
    );
}

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

/// Returns whether the canonical `snr_0000.json` is present.
fn assert_canonical_complete_or_absent(dir: &Path, ctx: &str) -> bool {
    let canon = dir.join("snr_0000.json");
    if !canon.exists() {
        return false;
    }
    let bytes = std::fs::read(&canon).unwrap();
    let parsed = checkpoint_payload(&bytes);
    assert!(
        parsed.is_ok(),
        "{ctx}: canonical snr_0000.json is torn/partial: {:?}",
        String::from_utf8_lossy(&bytes)
    );
    assert_eq!(parsed.unwrap().schema_version, 2);
    true
}

#[test]
#[cfg(unix)]
fn test_kill_during_fsync_deterministic() {
    // The child prints `BEGIN_FSYNC` after writing the tmp bytes of a large
    // checkpoint and before `sync_all`; a kill there lands before the atomic
    // rename, so the canonical file holds the prior checkpoint or is absent.
    use std::io::{BufRead, BufReader};

    let iterations = 2;
    let mut present = 0usize;
    let mut prior_state_survived = 0usize;
    for i in 0..iterations {
        let dir = tempdir_real_fs(&format!("fsynckill-{i}"));
        let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_checkpoint_sweep"))
            .args([
                "--checkpoint-dir",
                dir.path().to_str().unwrap(),
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

        let stdout = child.stdout.take().expect("piped stdout");
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
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

        if assert_canonical_complete_or_absent(dir.path(), &format!("iter {i}")) {
            present += 1;
            // The prior checkpoint has at most 2 worker states; the large
            // write has 700k.
            let c: CheckpointV2 =
                checkpoint_payload(&std::fs::read(dir.path().join("snr_0000.json")).unwrap())
                    .unwrap();
            if c.worker_states.len() <= 2 {
                prior_state_survived += 1;
            }
        }
    }

    assert!(
        present > 0,
        "expected the canonical checkpoint present in at least one of \
         {iterations} fsync-kill iterations"
    );
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
    // The kill delay sweeps 0.2-58 ms so that some iteration lets the child
    // finish a checkpoint; the complete-or-absent property holds at every
    // kill point.
    let iterations = 30;
    let mut observed_present = 0usize;
    for i in 0..iterations {
        let dir = tempdir(&format!("crashkill-{i}"));
        let mut child = spawn_sweep(&[
            "--checkpoint-dir",
            dir.path().to_str().unwrap(),
            "--channel",
            "awgn",
            "--snr-points",
            "1",
            "--seed",
            "1",
            "--crash-loop",
        ]);
        let micros = 200 + (i as u64 * 2000) % 60_000;
        std::thread::sleep(std::time::Duration::from_micros(micros));
        let _ = child.kill();
        let _ = child.wait();
        if assert_canonical_complete_or_absent(dir.path(), &format!("iter {i}")) {
            observed_present += 1;
        }
    }
    assert!(
        observed_present > 0,
        "expected the canonical checkpoint present in at least one of \
         {iterations} kill iterations"
    );
}
