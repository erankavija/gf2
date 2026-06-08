//! Checkpointed DVB-T2-style SNR-sweep driver with SIGINT-flush + `--resume`
//! (issue `5f12e7ff`, design doc §4; criterion 1, deliverable 2).
//!
//! Runs a serial sweep over `--snr-points` Es/N0 points, each simulated with a
//! per-frame closure driven by one of the [`gf2_sim::channels`] models
//! (`awgn` / `rayleigh` / `rician`), checkpointing per heartbeat / SNR boundary
//! / SIGINT via [`gf2_sim::checkpoint::run_sweep_checkpointed`]. On SIGINT the
//! library runner flushes the in-progress checkpoint; this binary then prints a
//! diagnostic and **exits with a non-zero status (130 = 128 + SIGINT)** — the
//! `exit non-zero` half of deliverable 2 that must NOT live in a library fn.
//!
//! # Usage
//!
//! ```bash
//! # Fresh 10-SNR AWGN sweep, checkpointing under /tmp/ck:
//! cargo run -p gf2-sim --release --bin checkpoint_sweep -- \
//!     --checkpoint-dir /tmp/ck --channel awgn --snr-points 10 \
//!     --seed 42 --max-frames 8 --heartbeat 4
//!
//! # Resume an interrupted sweep (same dir + config):
//! cargo run -p gf2-sim --release --bin checkpoint_sweep -- \
//!     --checkpoint-dir /tmp/ck --channel awgn --snr-points 10 \
//!     --seed 42 --max-frames 8 --heartbeat 4 --resume
//! ```
//!
//! The `--crash-loop` mode (used by the kill-during-fsync test) writes the
//! SNR-0 checkpoint in a tight loop forever so a parent can SIGKILL it mid-write
//! and verify the atomic-write contract.

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::process::ExitCode;

use gf2_sim::batch::SymbolBatch;
use gf2_sim::channels::{Awgn, Rayleigh, Rician};
use gf2_sim::checkpoint::{
    config_hash, run_sweep_checkpointed, CheckpointV2, CheckpointWriter, SweepError, WorkerState,
    SCHEMA_VERSION,
};
use gf2_sim::parallel::{FrameOutcome, WorkerCtx};
use gf2_sim::PipelineConfig;

/// Process exit code on SIGINT (128 + signal number; SIGINT = 2).
const EXIT_SIGINT: u8 = 130;

/// Symbols per synthetic frame. Small so the sweep stays fast; large enough that
/// a low-Es/N0 channel produces real sign-flip errors.
const SYMS_PER_FRAME: usize = 64;

/// Parsed CLI arguments.
struct Args {
    checkpoint_dir: PathBuf,
    resume: bool,
    channel: Channel,
    snr_points: usize,
    seed: u64,
    max_frames: u64,
    heartbeat: u64,
    crash_loop: bool,
}

#[derive(Clone, Copy)]
enum Channel {
    Awgn,
    Rayleigh,
    Rician,
}

const USAGE: &str = "checkpoint_sweep --checkpoint-dir <dir> [--resume] \
[--channel awgn|rayleigh|rician] [--snr-points N] [--seed S] \
[--max-frames N] [--heartbeat N] [--crash-loop]";

fn parse_args() -> Result<Args, String> {
    let mut checkpoint_dir: Option<PathBuf> = None;
    let mut resume = false;
    let mut channel = Channel::Awgn;
    let mut snr_points: usize = 10;
    let mut seed: u64 = 42;
    let mut max_frames: u64 = 8;
    let mut heartbeat: u64 = 4;
    let mut crash_loop = false;

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
    })
}

/// Builds the sweep config: `snr_points` Es/N0 points stepping 0.5 dB from 3.0.
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
        strict_gpu: false,
    }
}

/// A fixed +1/-1 (I) BPSK-ish signal batch (Q = 0); per-frame variation comes
/// entirely from the channel RNG draws, keeping each frame a pure function of
/// its seek position.
fn signal_batch(n: usize) -> SymbolBatch {
    let i: Vec<f32> = (0..n)
        .map(|k| if k % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    SymbolBatch::new(vec![i], vec![vec![0.0; n]])
}

/// Frame verdict: a symbol is in error if its noisy I-component flips sign.
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

/// Runs the sweep for the selected channel and returns the aggregate
/// `interrupted` flag (or a `SweepError`).
///
/// Each channel arm monomorphises `run_sweep_checkpointed` with its own
/// per-frame closure type; the `make_point` factory binds the point's Es/N0 to
/// a freshly constructed channel.
fn run(
    args: &Args,
    config: &PipelineConfig,
    writer: &CheckpointWriter,
) -> Result<bool, SweepError> {
    let hash = config_hash(config);
    let resume = args.resume;
    let sweep = match args.channel {
        Channel::Awgn => run_sweep_checkpointed(config, writer, &hash, resume, |_idx, esn0| {
            let ch = Awgn::new(esn0 as f32, 2);
            (
                || (),
                move |_g: usize, ctx: &mut WorkerCtx, _s: &mut ()| {
                    let mut b = signal_batch(SYMS_PER_FRAME);
                    ch.apply(&mut b, ctx.rng_mut());
                    verdict(&b)
                },
            )
        })?,
        Channel::Rayleigh => {
            run_sweep_checkpointed(config, writer, &hash, resume, |_idx, esn0| {
                let ch = Rayleigh::new(esn0 as f32, 2);
                (
                    || (),
                    move |_g: usize, ctx: &mut WorkerCtx, _s: &mut ()| {
                        let mut b = signal_batch(SYMS_PER_FRAME);
                        ch.apply(&mut b, ctx.rng_mut());
                        verdict(&b)
                    },
                )
            })?
        }
        Channel::Rician => run_sweep_checkpointed(config, writer, &hash, resume, |_idx, esn0| {
            let ch = Rician::new(esn0 as f32, 2, 4.0);
            (
                || (),
                move |_g: usize, ctx: &mut WorkerCtx, _s: &mut ()| {
                    let mut b = signal_batch(SYMS_PER_FRAME);
                    ch.apply(&mut b, ctx.rng_mut());
                    verdict(&b)
                },
            )
        })?,
    };
    Ok(sweep.interrupted)
}

/// `--crash-loop` mode: writes the SNR-0 checkpoint in a tight infinite loop so
/// a parent can SIGKILL mid-write and verify the atomic-write contract. Never
/// returns normally (the parent kills it).
fn crash_loop(config: &PipelineConfig, writer: &CheckpointWriter) -> ! {
    let hash = config_hash(config);
    let mut frames = 0u64;
    loop {
        frames = frames.wrapping_add(1) % 1000 + 1;
        let ckpt = CheckpointV2 {
            schema_version: SCHEMA_VERSION,
            snr_index: 0,
            esn0_db: config.esn0_db_points.first().copied().unwrap_or(3.0),
            config_hash: hash.clone(),
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
            worker_states: vec![
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
            ],
            drain_committed_at_us_since_epoch: 0,
        };
        // Ignore write errors: the parent may SIGKILL us mid-syscall.
        let _ = writer.write(&ckpt);
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

    if args.crash_loop {
        crash_loop(&config, &writer);
    }

    match run(&args, &config, &writer) {
        Ok(true) => {
            // SIGINT tripped mid-sweep; run_sweep_checkpointed already flushed
            // the in-progress checkpoint. Exit NON-ZERO (128 + SIGINT).
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
