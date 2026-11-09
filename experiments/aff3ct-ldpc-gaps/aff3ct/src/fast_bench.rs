//! Throughput of one AFF3CT NMS decoder configuration over a recorded bundle.
//!
//! `aff3ct-fast-bench BUNDLE CODE KIND SIMD PRECISION WORKERS BATCH CALLS`
//! prints one JSON line. `SIMD` is `none` or `INTER`; worker `i` pins to CPU
//! `i`. Every worker decodes the bundle's first `BATCH` frames `CALLS` times
//! after two warm calls; the quality pass decodes the whole bundle untimed.

use aff3ct_ldpc_arm::{Aff3ctWorker, Handle, Selection};
use aff3ct_ldpc_gaps::arm::DecoderCase;
use aff3ct_ldpc_gaps::pool::pin_current_thread;
use aff3ct_ldpc_gaps::workload::{median_u64, Workload};
use std::process::ExitCode;
use std::sync::{Barrier, Mutex};
use std::time::Instant;

const ITERATION_CAP: u32 = 50;
const NORMALIZATION: f32 = 0.75;
const WARM_CALLS: usize = 2;

fn number(text: &str, what: &str) -> Result<usize, String> {
    text.parse()
        .ok()
        .filter(|&value| value > 0)
        .ok_or(format!("{what} is a positive integer, not {text}"))
}

fn run(args: &[String]) -> Result<(), String> {
    let [bundle, code, kind, simd, precision, workers, batch, calls] = args else {
        return Err("usage: BUNDLE CODE KIND SIMD PRECISION WORKERS BATCH CALLS".to_owned());
    };
    let workers = number(workers, "WORKERS")?;
    let batch = number(batch, "BATCH")?;
    let calls = number(calls, "CALLS")?;
    let simd = if simd == "none" { "" } else { simd.as_str() };
    let case = DecoderCase {
        bundle: bundle.clone(),
        code: code.clone(),
        iteration_cap: ITERATION_CAP,
        normalization_factor: NORMALIZATION,
        syndrome_stopping: true,
        batch_size: batch as u32,
        quality_frames: 0,
        decisions_out: None,
    };
    let workload = Workload::load(&case)?;
    let (n, k) = (workload.manifest.n, workload.manifest.k);
    let selection = Selection::fast(kind, simd, precision);
    let prototype = Handle::build(
        &workload.alist(),
        k,
        n,
        ITERATION_CAP,
        NORMALIZATION,
        true,
        &selection,
    )?;
    let wave = prototype.wave();
    let name = prototype.name();
    if batch > workload.manifest.frames {
        return Err(format!("the bundle holds fewer than {batch} frames"));
    }

    let quality_frames = workload.manifest.frames / wave * wave;
    let mut quality = Aff3ctWorker::new(&prototype, quality_frames, k)?;
    quality.decode_batch(workload.frames(0, quality_frames));
    let frame_errors = workload
        .frame_errors(0, &quality.decisions)
        .iter()
        .filter(|&&errors| errors > 0)
        .count();

    let llrs = workload.frames(0, batch);
    let prototype = Mutex::new(prototype);
    let start = Barrier::new(workers);
    let medians = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|cpu| {
                let (prototype, start) = (&prototype, &start);
                scope.spawn(move || -> Result<u64, String> {
                    pin_current_thread(cpu as u32)?;
                    let mut worker = {
                        let prototype = prototype.lock().map_err(|e| e.to_string())?;
                        Aff3ctWorker::new(&prototype, batch, k)?
                    };
                    for _ in 0..WARM_CALLS {
                        worker.decode_batch(llrs);
                    }
                    start.wait();
                    let nanoseconds = (0..calls)
                        .map(|_| {
                            let call = Instant::now();
                            worker.decode_batch(llrs);
                            call.elapsed().as_nanos() as u64
                        })
                        .collect();
                    Ok(median_u64(nanoseconds))
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().map_err(|_| "a worker panicked".to_owned())?)
            .collect::<Result<Vec<u64>, String>>()
    })?;

    let median_call_ns = median_u64(medians);
    let frames_per_second = (workers * batch) as f64 / (median_call_ns as f64 * 1e-9);
    println!(
        "{}",
        serde_json::json!({
            "decoder": name,
            "kind": kind,
            "simd": simd,
            "precision": precision,
            "code": code,
            "workers": workers,
            "batch": batch,
            "calls": calls,
            "median_call_ns": median_call_ns,
            "frames_per_second": frames_per_second,
            "info_mbps": frames_per_second * k as f64 / 1e6,
            "quality_frames": quality_frames,
            "frame_errors": frame_errors,
        })
    );
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("aff3ct-fast-bench: {error}");
            ExitCode::FAILURE
        }
    }
}
