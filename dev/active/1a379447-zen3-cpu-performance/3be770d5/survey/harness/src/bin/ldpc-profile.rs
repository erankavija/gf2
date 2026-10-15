//! Profile driver for the steady-state LDPC arms (jit:3be770d5).
//!
//! Runs exactly the per-worker decode of a throughput cell outside the
//! benchmark runner so a profiler can observe it: the same recorded bundle,
//! pool, decoder construction, batch and decision check, in the
//! [`profile`] region. Prints one JSON record to standard
//! output; the placement report goes to standard error.
//!
//! Usage:
//!   ldpc-profile --arm gf2|aff3ct --bundle DIR --code NAME --core-arm ARM
//!       [--batch 16] [--passes 1] [--iteration-cap 50] [--norm 0.75]
//!       [--perf-control CTL,ACK]

use ldpc_survey::arm;
use ldpc_throughput::aff3ct::{Aff3ctWorker, Handle, Selection};
use ldpc_throughput::gf2::{config, Gf2Worker};
use ldpc_throughput::workload::verify_workers;
use std::process::ExitCode;
use std::time::Instant;

#[path = "../profile.rs"]
mod profile;
use profile::{profiled, record, ProfileOptions};

fn run() -> Result<(), String> {
    let options = ProfileOptions::parse(std::env::args().skip(1))?;
    let quality = arm::prepared_quality()?.ok_or(
        "the profile checks decisions against prepared quality; GF2_LDPC_QUALITY is unset",
    )?;
    let (workload, cpus) = options.load()?;
    let batch = options.batch as usize;
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
                &cpus,
                options.passes,
                options.perf_control.as_ref(),
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
                &cpus,
                options.passes,
                options.perf_control.as_ref(),
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
    let record = record(
        &options,
        &workload,
        &cpus,
        workers,
        &region,
        &selected,
        iterations.as_deref(),
    );
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
