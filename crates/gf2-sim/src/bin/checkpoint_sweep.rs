//! Checkpointed DVB-T2-style SNR-sweep driver with SIGINT-flush + `--resume`
//! (issue `5f12e7ff`, design doc §4; criterion 1, deliverable 2).
//!
//! Runs a serial sweep over `--snr-points` Es/N0 points, each simulated with a
//! per-frame closure driven by one of the [`gf2_sim::channels`] models
//! (`awgn` / `rayleigh` / `rician`), checkpointing per heartbeat / SNR boundary
//! / SIGINT via [`gf2_sim::snr_checkpoint::run_sweep_checkpointed`]. On SIGINT the
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
//! The `--crash-loop` mode writes the SNR-0 checkpoint in a tight loop forever
//! so a parent can SIGKILL it at a random moment (randomised defense-in-depth).
//! The `--crash-during-fsync` mode does a LARGE checkpoint write through
//! `CheckpointWriter::write_with_fsync_hook`, whose hook prints `BEGIN_FSYNC`
//! *immediately before* the tmp-file `sync_all`; a parent that SIGKILLs on the
//! marker lands the kill DURING the fsync (not during the byte write) — both
//! used by the kill-during-fsync atomic-write test.

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
    crash_during_fsync: bool,
    /// Milliseconds to pause after each completed SNR point. Widens the
    /// mid-sweep SIGINT window so the interrupt deterministically lands before
    /// the remaining points finish; `0` (the default) keeps reference/resume
    /// runs fast.
    point_delay_ms: u64,
    /// Test-only: when set, the per-heartbeat callback **blocks** at the FIRST
    /// within-point heartbeat flush until the interrupt flag is set, so a parent
    /// can deliver a real SIGINT that deterministically lands mid-point (snr 0,
    /// `0 < frames < max_frames`) regardless of host speed. This removes the
    /// parent-reads-buffered-stdout-vs-child-runs race that otherwise lets a
    /// fast/idle host finish the point before the SIGINT arrives.
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
        gpu_enabled: false,
        strict_gpu: false,
        diagnostic_dump_dir: None,
        inject_gpu_oom_modulus: None,
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

/// Per-point completion callback: prints `SNR_<idx>_FLUSHED` (so a parent can
/// detect that an SNR point COMPLETED), flushes stdout, then optionally pauses
/// `point_delay_ms`.
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

/// Per-heartbeat-flush callback: prints `HEARTBEAT_<snr>_<frames>` after each
/// WITHIN-point (non-final) checkpoint write, so a parent can deliver a SIGINT
/// while the point is still simulating (`0 < frames < point_max`). Flushes
/// stdout so the parent observes it without buffering delay.
fn heartbeat_marker(block_at_first: bool) -> impl FnMut(usize, u64) {
    use std::io::Write as _;
    let mut first = true;
    move |snr, frames| {
        println!("HEARTBEAT_{snr}_{frames}");
        let _ = std::io::stdout().flush();
        // Deterministic mid-point SIGINT (test-only): park at the FIRST
        // within-point flush until the interrupt flag is set by the parent's
        // real SIGINT. The checkpoint at `frames` is already on disk, so once
        // the signal lands the next chunk-boundary check stops the run here,
        // mid-point — independent of host speed. Without this, a fast/idle host
        // finishes the point before the parent (reading buffered stdout) can
        // deliver the signal.
        if block_at_first && first {
            first = false;
            while !is_interrupted() {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
    }
}

/// Runs the sweep for the selected channel and returns the aggregate
/// `interrupted` flag (or a `SweepError`).
///
/// Each channel arm monomorphises `run_sweep_checkpointed` with its own
/// per-frame closure type; the `make_point` factory binds the point's Es/N0 to
/// a freshly constructed channel. The per-point callback emits the
/// `SNR_<idx>_FLUSHED` progress marker (and the optional inter-point delay).
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

/// Builds a SNR-0 checkpoint whose serialised size grows with `extra_workers`
/// (each pretty-printed `worker_states` entry is ~110 B of JSON). With a large
/// `extra_workers` (e.g. [`FSYNC_CRASH_WORKERS`] ⇒ ≥64 MiB) the tmp file is big
/// enough that its `sync_all` does real, hundreds-of-ms disk I/O — the lever the
/// `--crash-during-fsync` mode uses to land a kill inside the real fsync.
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
    // Pad with synthetic workers to inflate the serialised payload.
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

/// `--crash-loop` mode: writes the SNR-0 checkpoint in a tight infinite loop so
/// a parent can SIGKILL mid-write and verify the atomic-write contract. Never
/// returns normally (the parent kills it).
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

/// Number of synthetic `worker_states` entries in the `--crash-during-fsync`
/// payload. Each pretty-printed entry is ~110 B, so 700 000 entries serialise to
/// a ≥64 MiB checkpoint — large enough that the tmp-file `sync_all` does real,
/// measurable disk I/O (hundreds of ms on a real filesystem) and the parent's
/// sub-millisecond read→kill lands *inside* that `sync_all`.
const FSYNC_CRASH_WORKERS: usize = 700_000;

/// `--crash-during-fsync` mode: lands a parent SIGKILL **mid-flush — after the
/// tmp bytes are written and before the atomic rename** (criterion 3, amended
/// 2026-06-08). It (1) writes one COMPLETE prior-state checkpoint, then in a
/// loop (2) calls `CheckpointWriter::write_with_fsync_hook` with a ≥64 MiB
/// payload, passing a hook that fires **after the tmp bytes are fully written
/// and immediately before `sync_all`** and only prints the `BEGIN_FSYNC` marker
/// (NO sleep/park). The parent SIGKILLs the instant it reads the marker; because
/// the subsequent real `sync_all` on a ≥64 MiB freshly-written file runs for
/// hundreds of milliseconds (on a real filesystem) while the parent's read→kill
/// is sub-millisecond, the SIGKILL lands within that real `sync_all` — squarely
/// inside the after-tmp-write / before-rename window.
///
/// The canonical `snr_0000.json` is only ever replaced by the atomic rename,
/// which happens *after* a successful `sync_all`. So for **any** kill before the
/// rename — including one during `sync_all` — the canonical stays the prior
/// complete state (or absent), never torn, which is the durability contract.
/// The large fsync simply pins the kill robustly into that window. Never returns
/// normally (the parent kills it).
fn crash_during_fsync(config: &PipelineConfig, writer: &CheckpointWriter) -> ! {
    use std::io::Write as _;
    let hash = config_hash(config);

    // (1) Establish one complete prior-state checkpoint so the "complete
    //     previous state survives" arm is exercised.
    let _ = writer.write(&crash_checkpoint(config, &hash, 7, 0));

    // (2) A ≥64 MiB payload so the tmp-file `sync_all` is a wide window.
    let big = crash_checkpoint(config, &hash, 9, FSYNC_CRASH_WORKERS);
    let mut frames = 9u64;
    loop {
        let mut ckpt = big.clone();
        frames = frames.wrapping_add(1);
        ckpt.frames_completed = frames; // vary so each write is distinct
                                        // The hook fires AFTER the tmp bytes are fully written and IMMEDIATELY
                                        // BEFORE `sync_all`. It prints the marker and returns at once — NO sleep
                                        // — so the kill the parent sends on the marker lands inside the real
                                        // `sync_all` that runs next (the payload is ≥64 MiB, so that syscall is
                                        // hundreds of ms wide). Ignore write errors: the parent may SIGKILL us
                                        // mid-syscall.
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
