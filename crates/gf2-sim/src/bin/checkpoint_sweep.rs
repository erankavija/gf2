//! Checkpointed SNR-sweep driver over the [`gf2_sim::channels`] models: runs
//! [`gf2_sim::snr_checkpoint::run_sweep_checkpointed`] with `--resume`, and
//! exits `130` when SIGINT interrupts the sweep. `--crash-loop` and
//! `--crash-during-fsync` rewrite the SNR-0 checkpoint forever so that a parent
//! test can SIGKILL the process mid-write.

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::process::ExitCode;

use gf2_sim::batch::SymbolBatch;
use gf2_sim::channels::{Awgn, Rayleigh, Rician};
use gf2_sim::parallel::{FrameOutcome, WorkerCtx};
use gf2_sim::snr_checkpoint::{
    config_hash, is_interrupted, run_sweep_checkpointed, CheckpointV2, CheckpointWriter,
    SweepError, WorkerState, SCHEMA_VERSION,
};
use gf2_sim::PipelineConfig;

/// 128 + SIGINT.
const EXIT_SIGINT: u8 = 130;

const SYMS_PER_FRAME: usize = 64;

struct Args {
    checkpoint_dir: PathBuf,
    resume: bool,
    channel: Channel,
    snr_points: usize,
    seed: u64,
    max_frames: u64,
    heartbeat: u64,
    crash_loop: bool,
    crash_during_fsync: bool,
    /// Milliseconds to pause after each completed SNR point, which widens the
    /// mid-sweep SIGINT window.
    point_delay_ms: u64,
    /// Test-only: the heartbeat callback blocks at the first within-point
    /// heartbeat flush until the interrupt flag is set, so a parent's SIGINT
    /// lands mid-point.
    block_at_first_heartbeat: bool,
}

#[derive(Clone, Copy)]
enum Channel {
    Awgn,
    Rayleigh,
    Rician,
}

const USAGE: &str = "checkpoint_sweep --checkpoint-dir <dir> [--resume] \
[--channel awgn|rayleigh|rician] [--snr-points N] [--seed S] \
[--max-frames N] [--heartbeat N] [--point-delay-ms N] \
[--block-at-first-heartbeat] [--crash-loop] [--crash-during-fsync]";

fn parse_args() -> Result<Args, String> {
    let mut checkpoint_dir: Option<PathBuf> = None;
    let mut resume = false;
    let mut channel = Channel::Awgn;
    let mut snr_points: usize = 10;
    let mut seed: u64 = 42;
    let mut max_frames: u64 = 8;
    let mut heartbeat: u64 = 4;
    let mut crash_loop = false;
    let mut crash_during_fsync = false;
    let mut point_delay_ms: u64 = 0;
    let mut block_at_first_heartbeat = false;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--checkpoint-dir" => {
                checkpoint_dir = Some(PathBuf::from(
                    it.next().ok_or("--checkpoint-dir needs a value")?,
                ));
            }
            "--resume" => resume = true,
            "--crash-loop" => crash_loop = true,
            "--crash-during-fsync" => crash_during_fsync = true,
            "--channel" => {
                channel = match it.next().ok_or("--channel needs a value")?.as_str() {
                    "awgn" => Channel::Awgn,
                    "rayleigh" => Channel::Rayleigh,
                    "rician" => Channel::Rician,
                    other => return Err(format!("unknown --channel {other}")),
                };
            }
            "--snr-points" => {
                snr_points = it
                    .next()
                    .ok_or("--snr-points needs a value")?
                    .parse()
                    .map_err(|e| format!("--snr-points: {e}"))?;
            }
            "--seed" => {
                seed = it
                    .next()
                    .ok_or("--seed needs a value")?
                    .parse()
                    .map_err(|e| format!("--seed: {e}"))?;
            }
            "--max-frames" => {
                max_frames = it
                    .next()
                    .ok_or("--max-frames needs a value")?
                    .parse()
                    .map_err(|e| format!("--max-frames: {e}"))?;
            }
            "--heartbeat" => {
                heartbeat = it
                    .next()
                    .ok_or("--heartbeat needs a value")?
                    .parse()
                    .map_err(|e| format!("--heartbeat: {e}"))?;
            }
            "--point-delay-ms" => {
                point_delay_ms = it
                    .next()
                    .ok_or("--point-delay-ms needs a value")?
                    .parse()
                    .map_err(|e| format!("--point-delay-ms: {e}"))?;
            }
            "--block-at-first-heartbeat" => block_at_first_heartbeat = true,
            "-h" | "--help" => return Err("help".to_string()),
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(Args {
        checkpoint_dir: checkpoint_dir.ok_or("--checkpoint-dir is required")?,
        resume,
        channel,
        snr_points,
        seed,
        max_frames,
        heartbeat,
        crash_loop,
        crash_during_fsync,
        point_delay_ms,
        block_at_first_heartbeat,
    })
}

fn build_config(args: &Args) -> PipelineConfig {
    let esn0_db_points: Vec<f64> = (0..args.snr_points).map(|i| 3.0 + 0.5 * i as f64).collect();
    PipelineConfig {
        seed: args.seed,
        esn0_db_points,
        target_errors: 0, // run the full frame budget at every point
        max_frames: args.max_frames,
        heartbeat_every_frames: args.heartbeat,
        checkpoint_dir: Some(args.checkpoint_dir.clone()),
        tracing_log_path: None,
        parallelism: NonZeroUsize::new(2).expect("2 is non-zero"),
        gpu_enabled: false,
        strict_gpu: false,
        diagnostic_dump_dir: None,
        inject_gpu_oom_modulus: None,
    }
}

/// Alternating ±1 on I, Q = 0; per-frame variation comes only from the channel
/// RNG draws.
fn signal_batch(n: usize) -> SymbolBatch {
    let i: Vec<f32> = (0..n)
        .map(|k| if k % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    SymbolBatch::new(vec![i], vec![vec![0.0; n]])
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

/// Per-point completion callback: the `SNR_<idx>_FLUSHED` line tells a parent
/// test that the point completed.
fn point_marker(
    point_delay_ms: u64,
) -> impl FnMut(usize, f64, &gf2_sim::snr_checkpoint::CheckpointedRun) {
    use std::io::Write as _;
    move |idx, _esn0, _run| {
        println!("SNR_{idx}_FLUSHED");
        let _ = std::io::stdout().flush();
        if point_delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(point_delay_ms));
        }
    }
}

/// Per-heartbeat callback: the `HEARTBEAT_<snr>_<frames>` line follows each
/// within-point checkpoint write.
fn heartbeat_marker(block_at_first: bool) -> impl FnMut(usize, u64) {
    use std::io::Write as _;
    let mut first = true;
    move |snr, frames| {
        println!("HEARTBEAT_{snr}_{frames}");
        let _ = std::io::stdout().flush();
        if block_at_first && first {
            first = false;
            while !is_interrupted() {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
    }
}

/// Returns whether the sweep was interrupted.
fn run(
    args: &Args,
    config: &PipelineConfig,
    writer: &CheckpointWriter,
) -> Result<bool, SweepError> {
    let hash = config_hash(config);
    let resume = args.resume;
    let delay = args.point_delay_ms;
    let sweep = match args.channel {
        Channel::Awgn => run_sweep_checkpointed(
            config,
            writer,
            &hash,
            resume,
            |_idx, esn0| {
                let ch = Awgn::new(esn0 as f32, 2);
                (
                    || (),
                    move |_g: usize, ctx: &mut WorkerCtx, _s: &mut ()| {
                        let mut b = signal_batch(SYMS_PER_FRAME);
                        ch.apply(&mut b, ctx.rng_mut());
                        verdict(&b)
                    },
                )
            },
            point_marker(delay),
            heartbeat_marker(args.block_at_first_heartbeat),
        )?,
        Channel::Rayleigh => run_sweep_checkpointed(
            config,
            writer,
            &hash,
            resume,
            |_idx, esn0| {
                let ch = Rayleigh::new(esn0 as f32, 2);
                (
                    || (),
                    move |_g: usize, ctx: &mut WorkerCtx, _s: &mut ()| {
                        let mut b = signal_batch(SYMS_PER_FRAME);
                        ch.apply(&mut b, ctx.rng_mut());
                        verdict(&b)
                    },
                )
            },
            point_marker(delay),
            heartbeat_marker(args.block_at_first_heartbeat),
        )?,
        Channel::Rician => run_sweep_checkpointed(
            config,
            writer,
            &hash,
            resume,
            |_idx, esn0| {
                let ch = Rician::new(esn0 as f32, 2, 4.0);
                (
                    || (),
                    move |_g: usize, ctx: &mut WorkerCtx, _s: &mut ()| {
                        let mut b = signal_batch(SYMS_PER_FRAME);
                        ch.apply(&mut b, ctx.rng_mut());
                        verdict(&b)
                    },
                )
            },
            point_marker(delay),
            heartbeat_marker(args.block_at_first_heartbeat),
        )?,
    };
    Ok(sweep.interrupted)
}

/// SNR-0 checkpoint padded with `extra_workers` synthetic `worker_states`
/// entries, which inflate its serialised size.
fn crash_checkpoint(
    config: &PipelineConfig,
    hash: &str,
    frames: u64,
    extra_workers: usize,
) -> CheckpointV2 {
    let mut worker_states = vec![
        WorkerState {
            worker_idx: 0,
            frames_in_worker: frames.div_ceil(2),
            rng_word_pos: frames as u128 * 4096,
        },
        WorkerState {
            worker_idx: 1,
            frames_in_worker: frames / 2,
            rng_word_pos: frames as u128 * 4096,
        },
    ];
    worker_states.extend((2..2 + extra_workers).map(|w| WorkerState {
        worker_idx: w,
        frames_in_worker: w as u64,
        rng_word_pos: (w as u128) * 4096,
    }));
    CheckpointV2 {
        schema_version: SCHEMA_VERSION,
        snr_index: 0,
        esn0_db: config.esn0_db_points.first().copied().unwrap_or(3.0),
        config_hash: hash.to_string(),
        frames_target: config.max_frames,
        errors_target: config.target_errors,
        max_frames: config.max_frames,
        frames_completed: frames,
        errors_accumulated: frames / 2,
        total_iterations: frames * 3,
        total_queries: frames,
        total_bits: frames * 64,
        total_bit_errors: frames,
        completed: false,
        worker_states,
        drain_committed_at_us_since_epoch: 0,
    }
}

/// `--crash-loop` mode: rewrites the SNR-0 checkpoint forever so that a parent
/// can SIGKILL mid-write.
fn crash_loop(config: &PipelineConfig, writer: &CheckpointWriter) -> ! {
    let hash = config_hash(config);
    let mut frames = 0u64;
    loop {
        frames = frames.wrapping_add(1) % 1000 + 1;
        let ckpt = crash_checkpoint(config, &hash, frames, 0);
        // Ignore write errors: the parent may SIGKILL us mid-syscall.
        let _ = writer.write(&ckpt);
    }
}

/// Synthetic `worker_states` entries in the `--crash-during-fsync` payload,
/// sized so that the parent's SIGKILL lands inside the tmp-file `sync_all`.
const FSYNC_CRASH_WORKERS: usize = 700_000;

/// `--crash-during-fsync` mode: writes one complete checkpoint, then rewrites
/// the padded payload forever through `CheckpointWriter::write_with_fsync_hook`.
/// The hook prints `BEGIN_FSYNC` immediately before the file fsync, the marker
/// on which the parent sends SIGKILL.
fn crash_during_fsync(config: &PipelineConfig, writer: &CheckpointWriter) -> ! {
    use std::io::Write as _;
    let hash = config_hash(config);

    let _ = writer.write(&crash_checkpoint(config, &hash, 7, 0));

    let big = crash_checkpoint(config, &hash, 9, FSYNC_CRASH_WORKERS);
    let mut frames = 9u64;
    loop {
        let mut ckpt = big.clone();
        frames = frames.wrapping_add(1);
        ckpt.frames_completed = frames; // vary so each write is distinct
        let _ = writer.write_with_fsync_hook(&ckpt, || {
            println!("BEGIN_FSYNC");
            let _ = std::io::stdout().flush();
        });
    }
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            if msg == "help" {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            eprintln!("error: {msg}\nusage: {USAGE}");
            return ExitCode::from(2);
        }
    };

    let config = build_config(&args);
    let writer = match CheckpointWriter::new(&args.checkpoint_dir) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("error: cannot open checkpoint dir: {e}");
            return ExitCode::FAILURE;
        }
    };

    if args.crash_during_fsync {
        crash_during_fsync(&config, &writer);
    }
    if args.crash_loop {
        crash_loop(&config, &writer);
    }

    match run(&args, &config, &writer) {
        Ok(true) => {
            eprintln!("interrupted: checkpoint flushed; exiting {EXIT_SIGINT}");
            ExitCode::from(EXIT_SIGINT)
        }
        Ok(false) => {
            println!("sweep complete: {} SNR points", config.esn0_db_points.len());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
