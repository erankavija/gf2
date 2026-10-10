//! Checkpoint resume tests on AWGN, Rayleigh and Rician channels: a
//! library-level or SIGINT-interrupted run resumes byte-identically to an
//! uninterrupted one, and a writer killed mid-flush leaves the canonical
//! checkpoint complete or absent.

use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use gf2_coding::ldpc::{
    DecodedWord, DecoderAlgorithm, DecoderConfig, LdpcBatchDecoder, QuasiCyclicLdpc,
};
use gf2_coding::traits::BatchSoftDecoder;
use gf2_coding::{LdpcCode, Llr};
use gf2_sim::batch::{LlrBatch, SymbolBatch};
use gf2_sim::channels::{Awgn, Rayleigh, Rician};
use gf2_sim::parallel::{worker_index_partition, FrameOutcome, WorkerCtx};
use gf2_sim::snr_checkpoint::{
    clear_interrupt, config_hash, request_interrupt, run_snr_point_checkpointed, CheckpointReader,
    CheckpointV2, CheckpointWriter, CheckpointedRun,
};
use gf2_sim::stage::Stage;
use gf2_sim::stages::batch_decode::{BatchDecode, BatchDecodeScratch};
use gf2_sim::testutil::AwgnLlrSource;
use gf2_sim::PipelineConfig;

fn tempdir(tag: &str) -> gf2_core::test_scratch::Scratch {
    gf2_core::test_scratch::scratch(&format!("gf2sim-ckcompat-{tag}"))
}

/// Serialises the in-process runs of this binary: the interrupt flag is
/// process-wide, so a request one test raises must not reach another's run.
static INTERRUPT_MUTEX: Mutex<()> = Mutex::new(());

fn interrupt_guard() -> MutexGuard<'static, ()> {
    INTERRUPT_MUTEX
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
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
    let _guard = interrupt_guard();
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

const BATCH_SEED: u64 = 0xB47C_4DEC;

/// Noise deviations cycling over the frames, from clean to undecodable, so
/// that a decode wave holds converging and non-converging lanes.
const BATCH_SIGMAS: [f64; 6] = [0.5, 0.8, 1.6, 0.7, 1.2, 0.9];

const BATCH_MAX_ITER: usize = 12;

const BATCH_FRAMES: u64 = 100;

/// At one, two and four workers no worker's share of a heartbeat chunk is a
/// multiple of either batch size under test.
const BATCH_HEARTBEAT: u64 = 43;

fn batch_code() -> LdpcCode {
    QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121)
        .mother_code()
        .clone()
}

fn batch_decoder_factory(code: LdpcCode) -> impl Fn() -> LdpcBatchDecoder + Send + Sync + 'static {
    move || {
        let cfg = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
        LdpcBatchDecoder::new(code.clone(), cfg, BATCH_MAX_ITER, DecodedWord::Message)
    }
}

/// End of the dispatch holding frame `g`: the runner dispatches
/// `heartbeat_every_frames`-sized chunks from frame 0 or from a heartbeat
/// checkpoint.
fn dispatch_end(config: &PipelineConfig, g: usize) -> usize {
    let max_frames = config.max_frames as usize;
    match config.heartbeat_every_frames as usize {
        0 => max_frames,
        chunk => ((g / chunk + 1) * chunk).min(max_frames),
    }
}

/// Per-worker state of a batch-decoding campaign: the outcomes of the batch
/// in progress, in the order the worker's frames arrive.
#[derive(Default)]
struct BatchWorker {
    scratch: BatchDecodeScratch,
    pending: VecDeque<(usize, FrameOutcome)>,
}

/// Frame kernel of a batch-decoding campaign. A worker without pending
/// outcomes decodes its next `batch` frames of the current dispatch, fewer at
/// the dispatch end, in one [`BatchDecode`] call and appends their indices to
/// `decoded`. Each frame's LLRs depend on its index alone.
fn batch_decode_frame<'a>(
    code: &'a LdpcCode,
    stage: &'a BatchDecode<impl Fn() -> LdpcBatchDecoder + Send + Sync + 'static>,
    config: &'a PipelineConfig,
    batch: usize,
    decoded: &'a Mutex<Vec<Vec<usize>>>,
) -> impl Fn(usize, &mut WorkerCtx, &mut BatchWorker) -> FrameOutcome + Sync + 'a {
    move |g, _ctx, worker| {
        if worker.pending.is_empty() {
            // The worker's remaining frames of the dispatch stride from `g`.
            let frames: Vec<usize> = worker_index_partition(
                g as u64..dispatch_end(config, g) as u64,
                0,
                config.parallelism,
            )
            .take(batch)
            .map(|f| f as usize)
            .collect();
            let llrs: Vec<Vec<Llr>> = frames
                .iter()
                .map(|&f| {
                    let sigma = BATCH_SIGMAS[f % BATCH_SIGMAS.len()];
                    AwgnLlrSource::new(BATCH_SEED ^ (f as u64).wrapping_mul(0x9E37))
                        .frame_all_zero(code.n(), sigma)
                })
                .collect();
            let words = stage
                .process(&LlrBatch::new(llrs), &mut worker.scratch)
                .expect("the batch decode stage does not fail");
            for ((&f, word), &iterations) in frames
                .iter()
                .zip(&words.frames)
                .zip(&worker.scratch.iterations)
            {
                let bit_errors = (0..word.len()).filter(|&i| word.get(i)).count() as u64;
                worker.pending.push_back((
                    f,
                    FrameOutcome {
                        errored: bit_errors > 0,
                        iterations,
                        info_bits: word.len() as u64,
                        bit_errors,
                    },
                ));
            }
            decoded.lock().unwrap().push(frames);
        }
        let (frame, outcome) = worker.pending.pop_front().unwrap();
        assert_eq!(frame, g, "a worker's frames follow its strided partition");
        outcome
    }
}

/// Runs the batch-decoding campaign under `config` and returns the run with
/// the frame indices of every decoded batch. The caller holds
/// [`interrupt_guard`].
fn run_batch_campaign(
    batch: usize,
    config: &PipelineConfig,
    dir: &Path,
    resume: Option<CheckpointV2>,
    on_heartbeat_flush: impl FnMut(usize, u64),
) -> (CheckpointedRun, Vec<Vec<usize>>) {
    let code = batch_code();
    let stage = BatchDecode::new(batch_decoder_factory(code.clone()));
    let decoded = Mutex::new(Vec::new());
    let run = run_snr_point_checkpointed(
        config,
        0,
        6.25,
        &CheckpointWriter::new(dir).unwrap(),
        &config_hash(config),
        resume,
        BatchWorker::default,
        batch_decode_frame(&code, &stage, config, batch, &decoded),
        on_heartbeat_flush,
    )
    .unwrap();
    (run, decoded.into_inner().unwrap())
}

fn sorted_frames(batches: &[Vec<usize>]) -> Vec<usize> {
    let mut frames: Vec<usize> = batches.iter().flatten().copied().collect();
    frames.sort_unstable();
    frames
}

/// Interrupts a campaign decoding `batch`-frame batches at its first heartbeat
/// checkpoint, resumes it, and asserts at each worker count that the counters
/// equal those of two uninterrupted runs, one of which decodes a wave across
/// the checkpoint boundary.
fn assert_batch_decode_resume(batch: usize) {
    let lanes = batch_decoder_factory(batch_code())().batch_granularity();
    // A batch the dispatch end cuts short, leaving a lane wave partly filled.
    let cut_short = |batches: &[Vec<usize>]| {
        batches
            .iter()
            .filter(|b| b.len() < batch && !b.len().is_multiple_of(lanes))
            .count()
    };
    for parallelism in [1, 2, 4] {
        let tag = format!("batch{batch}-p{parallelism}");
        let _guard = interrupt_guard();
        let full = cfg(parallelism, BATCH_FRAMES, BATCH_HEARTBEAT);

        let dir = tempdir(&format!("{tag}-resume"));
        clear_interrupt();
        let mut heartbeats = 0;
        let (interrupted, before) = run_batch_campaign(batch, &full, dir.path(), None, |_, _| {
            heartbeats += 1;
            request_interrupt();
        });
        clear_interrupt();
        assert!(
            interrupted.interrupted && !interrupted.completed,
            "[{tag}] the run must stop on the interrupt: {interrupted:?}"
        );
        assert_eq!(
            heartbeats, 1,
            "[{tag}] the run stops at its first heartbeat"
        );
        let boundary = interrupted.counters.frames as usize;
        assert_eq!(
            sorted_frames(&before),
            (0..boundary).collect::<Vec<_>>(),
            "[{tag}] the interrupted run decodes the frames below the checkpoint"
        );
        assert_eq!(
            cut_short(&before),
            parallelism,
            "[{tag}] the checkpoint must cut each worker's last batch short: {before:?}"
        );

        let loaded = CheckpointReader::new(dir.path(), config_hash(&full))
            .load(0)
            .unwrap()
            .unwrap();
        assert!(!loaded.completed, "[{tag}] the checkpoint is resumable");
        assert_eq!(loaded.frames_completed as usize, boundary);
        let (resumed, after) =
            run_batch_campaign(batch, &full, dir.path(), Some(loaded), |_, _| {});
        assert!(resumed.completed && !resumed.interrupted);
        assert_eq!(
            sorted_frames(&after),
            (boundary..BATCH_FRAMES as usize).collect::<Vec<_>>(),
            "[{tag}] the resumed run decodes the remaining frames"
        );
        assert!(
            resumed.counters.errors > 0 && resumed.counters.errors < resumed.counters.frames,
            "[{tag}] the campaign must mix frame errors and successes: {:?}",
            resumed.counters
        );

        // One dispatch over the whole range, so no checkpoint bounds a batch.
        let undivided = PipelineConfig {
            heartbeat_every_frames: 0,
            ..full.clone()
        };
        for (name, config) in [("checkpointed", &full), ("undivided", &undivided)] {
            let dir_ref = tempdir(&format!("{tag}-{name}"));
            let (reference, batches) =
                run_batch_campaign(batch, config, dir_ref.path(), None, |_, _| {});
            assert!(reference.completed && !reference.interrupted);
            assert_eq!(
                resumed.counters, reference.counters,
                "[{tag}] resume must be byte-identical to the {name} uninterrupted run"
            );
            if config.heartbeat_every_frames == 0 {
                let across_boundary = batches
                    .iter()
                    .filter(|b| b[0] < boundary && boundary <= b[b.len() - 1])
                    .count();
                assert_eq!(
                    across_boundary, parallelism,
                    "[{tag}] each worker of the undivided run must decode one \
                     batch across the checkpoint boundary: {batches:?}"
                );
            }
        }
    }
}

#[test]
fn test_v2_resume_byte_identical_batch_decode_one_wave_batches() {
    let lanes = batch_decoder_factory(batch_code())().batch_granularity();
    assert_batch_decode_resume(lanes);
}

#[test]
fn test_v2_resume_byte_identical_batch_decode_partial_wave_batches() {
    // A batch of 1.5 waves decodes a full wave and a half-filled one.
    let lanes = batch_decoder_factory(batch_code())().batch_granularity();
    assert_batch_decode_resume(lanes + lanes / 2);
}

#[test]
fn test_v2_resume_nonzero_errors_present() {
    let ch = Awgn::new(0.5, 2);
    let c = cfg(2, 20, 7);
    let h = config_hash(&c);
    let dir = tempdir("nonzero");
    let _guard = interrupt_guard();
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
    let _guard = interrupt_guard();
    clear_interrupt();
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

const SWEEP_CHANNELS: [&str; 3] = ["awgn", "rayleigh", "rician"];

fn sweep_args<'a>(dir: &'a str, channel: &'a str, resume: bool) -> Vec<&'a str> {
    let mut args = vec![
        "--checkpoint-dir",
        dir,
        "--channel",
        channel,
        "--snr-points",
        "2",
        "--seed",
        "7",
        "--max-frames",
        "8",
        "--heartbeat",
        "4",
    ];
    if resume {
        args.push("--resume");
    }
    args
}

fn sweep_checkpoint_identity(channel: &str) -> String {
    let dir = tempdir(&format!("sweep-identity-{channel}"));
    run_full_sweep(dir.path(), channel, 2, 8, 4);
    let bytes = std::fs::read(dir.path().join("snr_0000.json")).unwrap();
    checkpoint_payload(&bytes).unwrap().config_hash
}

#[test]
fn test_sweep_checkpoint_identity_differs_per_channel_pair() {
    let identities: Vec<String> = SWEEP_CHANNELS
        .iter()
        .map(|c| sweep_checkpoint_identity(c))
        .collect();
    for a in 0..identities.len() {
        for b in a + 1..identities.len() {
            assert_ne!(
                identities[a], identities[b],
                "{} and {} sweeps must have distinct checkpoint identities",
                SWEEP_CHANNELS[a], SWEEP_CHANNELS[b]
            );
        }
    }
}

#[test]
fn test_sweep_resume_rejects_checkpoints_of_another_channel() {
    for written in SWEEP_CHANNELS {
        for resumed in SWEEP_CHANNELS.into_iter().filter(|c| *c != written) {
            let dir = tempdir(&format!("sweep-xchannel-{written}-{resumed}"));
            let dir_str = dir.path().to_str().unwrap();
            run_full_sweep(dir.path(), written, 2, 8, 4);
            let before = load_all_normalized(dir.path());

            let out = std::process::Command::new(env!("CARGO_BIN_EXE_checkpoint_sweep"))
                .args(sweep_args(dir_str, resumed, true))
                .output()
                .expect("checkpoint_sweep must spawn");

            let ctx = format!("{written} checkpoints resumed as {resumed}");
            assert!(!out.status.success(), "{ctx}: resume must fail");
            assert!(
                String::from_utf8_lossy(&out.stderr).contains("ConfigHashMismatch"),
                "{ctx}: stderr must report the mismatch, got {}",
                String::from_utf8_lossy(&out.stderr)
            );
            assert!(
                !String::from_utf8_lossy(&out.stdout).contains("FLUSHED"),
                "{ctx}: no point may be completed or reused"
            );
            assert_eq!(
                load_all_normalized(dir.path()),
                before,
                "{ctx}: checkpoints must be left untouched"
            );
        }
    }
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
