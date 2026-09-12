//! Profile driver for the steady-state LDPC arms (jit:3be770d5).
//!
//! Runs exactly the per-worker decode of a throughput cell outside the
//! benchmark runner so a profiler can observe it: the same recorded bundle,
//! pool, decoder construction, batch and decision check. It resolves the core
//! arm itself with the protocol's resolver, performs one untimed warm
//! dispatch, then `--passes` dispatches inside the profiled region. With
//! `--perf-control CTL,ACK` it enables and disables a `perf` session started
//! with `-D -1 --control fifo:CTL,ACK` around exactly that region, so setup
//! and the warm pass stay outside the samples.
//!
//! Prints one JSON record to standard output; the placement report goes to
//! standard error. The wall-clock figures are profile-session observations,
//! not protocol timing windows.
//!
//! Usage:
//!   ldpc-profile --arm gf2|aff3ct --bundle DIR --code NAME --core-arm ARM
//!       [--batch 16] [--passes 1] [--iteration-cap 50] [--norm 0.75]
//!       [--perf-control CTL,ACK]

use ldpc_survey::arm::{self, DecoderCase};
use ldpc_throughput::aff3ct::{Aff3ctWorker, Handle, Selection};
use ldpc_throughput::gf2::{config, Gf2Worker};
use ldpc_throughput::pool::{self, process_threads, Finished};
use ldpc_throughput::workload::{verify_workers, Threads, Workload};
use serde_json::json;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::process::ExitCode;
use std::str::FromStr;
use std::time::Instant;
use tuning_campaign_support::host::{resolve_core_arm, CoreArm, CpuAffinity, CpuTopology};

const SCHEMA: &str = "ldpc-profile-record-v1";

struct Options {
    arm: String,
    bundle: String,
    code: String,
    core_arm: String,
    batch: u32,
    passes: u64,
    iteration_cap: u32,
    norm: f32,
    perf_control: Option<(String, String)>,
}

fn parse() -> Result<Options, String> {
    let mut args = std::env::args().skip(1);
    let mut options = Options {
        arm: String::new(),
        bundle: String::new(),
        code: String::new(),
        core_arm: String::new(),
        batch: 16,
        passes: 1,
        iteration_cap: 50,
        norm: 0.75,
        perf_control: None,
    };
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or(format!("{flag} needs a value"));
        fn number<T: FromStr>(text: String, flag: &str) -> Result<T, String> {
            text.parse().map_err(|_| format!("{flag}: not a number"))
        }
        match flag.as_str() {
            "--arm" => options.arm = value()?,
            "--bundle" => options.bundle = value()?,
            "--code" => options.code = value()?,
            "--core-arm" => options.core_arm = value()?,
            "--batch" => options.batch = number(value()?, &flag)?,
            "--passes" => options.passes = number(value()?, &flag)?,
            "--iteration-cap" => options.iteration_cap = number(value()?, &flag)?,
            "--norm" => options.norm = number(value()?, &flag)?,
            "--perf-control" => {
                let text = value()?;
                let (ctl, ack) = text.split_once(',').ok_or("--perf-control takes CTL,ACK")?;
                options.perf_control = Some((ctl.to_owned(), ack.to_owned()));
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if options.arm.is_empty() || options.bundle.is_empty() || options.code.is_empty() {
        return Err("--arm, --bundle and --code are required".to_owned());
    }
    if options.passes == 0 {
        return Err("--passes must be positive".to_owned());
    }
    Ok(options)
}

/// A `perf` control channel: commands go to CTL, acknowledgements come on ACK.
struct PerfControl {
    ctl: File,
    ack: BufReader<File>,
}

impl PerfControl {
    fn open(ctl: &str, ack: &str) -> Result<Self, String> {
        let ctl = OpenOptions::new()
            .write(true)
            .open(ctl)
            .map_err(|e| format!("{ctl}: {e}"))?;
        let ack = BufReader::new(File::open(ack).map_err(|e| format!("{ack}: {e}"))?);
        Ok(Self { ctl, ack })
    }

    fn command(&mut self, command: &str) -> Result<(), String> {
        writeln!(self.ctl, "{command}").map_err(|e| e.to_string())?;
        self.ctl.flush().map_err(|e| e.to_string())?;
        let mut line = String::new();
        self.ack.read_line(&mut line).map_err(|e| e.to_string())?;
        if line.trim() != "ack" {
            return Err(format!("perf answered {line:?} to {command}"));
        }
        Ok(())
    }
}

/// Timings of the profiled region.
struct Region {
    setup_ns: u64,
    decode_ns: u64,
    threads: Threads,
    calls: u64,
}

fn profiled<S>(
    options: &Options,
    cpus: &[u32],
    setup_start: Instant,
    init: impl Fn(usize) -> Result<S, String> + Sync,
    body: impl Fn(&mut S) + Sync,
) -> Result<(Region, Vec<Finished<S>>), String>
where
    S: Send,
{
    let (region, finished) = pool::run(
        cpus,
        init,
        |_, state| body(state),
        |dispatch| -> Result<Region, String> {
            let setup_ns = setup_start.elapsed().as_nanos() as u64;
            let ready = process_threads();
            dispatch.call();
            let mut control = match &options.perf_control {
                Some((ctl, ack)) => Some(PerfControl::open(ctl, ack)?),
                None => None,
            };
            if let Some(control) = control.as_mut() {
                control.command("enable")?;
            }
            let start = Instant::now();
            for _ in 0..options.passes {
                dispatch.call();
            }
            let decode_ns = start.elapsed().as_nanos() as u64;
            if let Some(control) = control.as_mut() {
                control.command("disable")?;
            }
            Ok(Region {
                setup_ns,
                decode_ns,
                threads: Threads {
                    ready,
                    after: process_threads(),
                },
                calls: options.passes + 1,
            })
        },
    )?;
    Ok((region?, finished))
}

fn run() -> Result<(), String> {
    let options = parse()?;
    let quality = arm::prepared_quality()?.ok_or(
        "the profile checks decisions against prepared quality; GF2_LDPC_QUALITY is unset",
    )?;
    let case = DecoderCase {
        bundle: options.bundle.clone(),
        code: options.code.clone(),
        iteration_cap: options.iteration_cap,
        normalization_factor: options.norm,
        syndrome_stopping: true,
        batch_size: options.batch,
        quality_frames: 0,
        decisions_out: None,
    };
    let workload = Workload::load(&case)?;
    let core_arm: CoreArm =
        serde_json::from_value(json!(options.core_arm)).map_err(|e| format!("--core-arm: {e}"))?;
    let topology = CpuTopology::observe().map_err(|e| e.to_string())?;
    let affinity = CpuAffinity::observe().map_err(|e| e.to_string())?;
    let cpus = resolve_core_arm(core_arm, &topology, &affinity)?;
    let batch = options.batch as usize;
    if batch == 0 || batch > workload.manifest.frames {
        return Err(format!(
            "--batch must lie in 1..={}",
            workload.manifest.frames
        ));
    }
    let llrs = workload.frames(0, batch);
    let (n, k) = (workload.manifest.n, workload.manifest.k);
    let setup_start = Instant::now();
    let (region, workers, selected, iterations) = match options.arm.as_str() {
        "gf2" => {
            let code =
                ldpc_survey::read_alist_code(&workload.alist()).map_err(|e| e.to_string())?;
            let decoder_config = config(options.norm, true);
            let cap = options.iteration_cap as usize;
            let (region, finished) = profiled(
                &options,
                &cpus,
                setup_start,
                |_| Ok(Gf2Worker::new(&code, decoder_config, batch, cap)),
                |worker: &mut Gf2Worker| worker.decode_batch(llrs),
            )?;
            let iterations: Vec<u32> = finished[0].state.iterations.clone();
            let workers = verify_workers(
                "gf2",
                &finished,
                region.calls,
                region.threads,
                |worker: &Gf2Worker| &worker.decisions,
                &workload,
                &quality,
            )?;
            (
                region,
                workers,
                "gf2-coding LdpcDecoder f32 flooding NMS".to_owned(),
                Some(iterations),
            )
        }
        "aff3ct" => {
            let selection = Selection::from_environment()?;
            let prototype = Handle::build(
                &workload.alist(),
                k,
                n,
                options.iteration_cap,
                options.norm,
                true,
                &selection,
            )?;
            let name = prototype.name();
            let (region, finished) = profiled(
                &options,
                &cpus,
                setup_start,
                |_| Aff3ctWorker::new(&prototype, batch, k),
                |worker: &mut Aff3ctWorker| worker.decode_batch(llrs),
            )?;
            let workers = verify_workers(
                "aff3ct",
                &finished,
                region.calls,
                region.threads,
                |worker: &Aff3ctWorker| &worker.decisions,
                &workload,
                &quality,
            )?;
            let selected = format!(
                "{name} simd={:?} precision={}",
                selection.simd, selection.precision
            );
            (region, workers, selected, None)
        }
        other => return Err(format!("--arm {other} is not gf2 or aff3ct")),
    };
    let frames_per_worker = batch as u64 * options.passes;
    let record = json!({
        "schema": SCHEMA,
        "arm": options.arm,
        "selected_path": selected,
        "code": options.code,
        "bundle_llr_sha256": workload.manifest.llrs_sha256,
        "h_sha256": workload.manifest.h_sha256,
        "core_arm": options.core_arm,
        "cpus": cpus,
        "workers": workers,
        "threads": region.threads,
        "batch": batch,
        "passes": options.passes,
        "frames_per_worker": frames_per_worker,
        "frames": frames_per_worker * u64::from(workers),
        "iteration_cap": options.iteration_cap,
        "normalization_factor": options.norm,
        "batch_iterations": iterations,
        "setup_ns": region.setup_ns,
        "decode_ns": region.decode_ns,
        "ns_per_frame_per_worker": region.decode_ns as f64 / frames_per_worker as f64,
        "perf_controlled": options.perf_control.is_some(),
    });
    println!("{record}");
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ldpc-profile: {error}");
            ExitCode::FAILURE
        }
    }
}
