//! Untimed validation of the steady-state decode paths (jit:3be770d5).
//!
//! Replays every recorded frame of a bundle through the pool the throughput
//! arms use: `--workers` pinned workers, each with its own reused gf2 decoder
//! or AFF3CT clone, decode consecutive `--batch`-frame chunks in turn. The
//! per-frame information-window error vector must equal the prepared quality
//! evidence exactly; for gf2 the iteration distribution must equal it as
//! well. Prints one JSON verdict and exits nonzero on any difference. Nothing
//! is timed.
//!
//! Usage:
//!   ldpc-throughput-validate --arm gf2|aff3ct --bundle DIR --code NAME
//!       [--workers 2] [--batch 16] [--iteration-cap 50] [--norm 0.75]

use ldpc_survey::arm::{self, quantile, DecoderCase};
use ldpc_throughput::aff3ct::{Aff3ctWorker, Handle, Selection};
use ldpc_throughput::gf2::{config, Gf2Worker};
use ldpc_throughput::pool::{self, Finished};
use ldpc_throughput::workload::{check_prepared, Workload};
use serde_json::json;
use std::process::ExitCode;
use tuning_campaign_support::host::CpuAffinity;

/// A worker's decoder plus the chunks it decoded.
struct Chunked<W> {
    worker: W,
    /// (first frame, decisions, iterations when observed) per chunk.
    chunks: Vec<(usize, Vec<u8>, Vec<u32>)>,
    next: usize,
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (mut arm_name, mut bundle, mut code) = (String::new(), String::new(), String::new());
    let (mut workers, mut batch, mut cap, mut norm) = (2usize, 16usize, 50u32, 0.75f32);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or(format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--arm" => arm_name = value,
            "--bundle" => bundle = value,
            "--code" => code = value,
            "--workers" => workers = value.parse().map_err(|_| format!("{flag}: not a number"))?,
            "--batch" => batch = value.parse().map_err(|_| format!("{flag}: not a number"))?,
            "--iteration-cap" => cap = value.parse().map_err(|_| format!("{flag}: not a number"))?,
            "--norm" => norm = value.parse().map_err(|_| format!("{flag}: not a number"))?,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let quality = arm::prepared_quality()?.ok_or("GF2_LDPC_QUALITY is unset")?;
    let case = DecoderCase {
        bundle,
        code: code.clone(),
        iteration_cap: cap,
        normalization_factor: norm,
        syndrome_stopping: true,
        batch_size: batch as u32,
        quality_frames: 0,
        decisions_out: None,
    };
    let workload = Workload::load(&case)?;
    let frames = workload.manifest.frames;
    let k = workload.manifest.k;
    if batch == 0 || frames % (batch * workers) != 0 {
        return Err(format!(
            "{workers} workers of {batch}-frame chunks must tile {frames} frames"
        ));
    }
    let cpus: Vec<u32> = CpuAffinity::observe()
        .map_err(|e| e.to_string())?
        .cpus()
        .iter()
        .copied()
        .take(workers)
        .collect();
    if cpus.len() != workers {
        return Err(format!("the affinity mask holds fewer than {workers} CPUs"));
    }
    let rounds = frames / (batch * workers);
    let chunk_start = move |worker: usize, round: usize| (round * workers + worker) * batch;
    let dispatch_rounds = |dispatch: &pool::Dispatch<'_>| {
        for _ in 0..rounds {
            dispatch.call();
        }
    };
    let (selected, finished_chunks): (String, Vec<Vec<(usize, Vec<u8>, Vec<u32>)>>) =
        match arm_name.as_str() {
            "gf2" => {
                let decoder_code = ldpc_survey::read_alist_code(&workload.alist())
                    .map_err(|e| e.to_string())?;
                let decoder_config = config(norm, true);
                let (_, finished) = pool::run(
                    &cpus,
                    |_| {
                        Ok(Chunked {
                            worker: Gf2Worker::new(&decoder_code, decoder_config, batch, cap as usize),
                            chunks: Vec::new(),
                            next: 0,
                        })
                    },
                    |index, state: &mut Chunked<Gf2Worker>| {
                        let first = chunk_start(index, state.next);
                        state.worker.decode_batch(workload.frames(first, batch));
                        let decided = state.worker.decisions.clone();
                        let iterations = state.worker.iterations.clone();
                        state.chunks.push((first, decided, iterations));
                        state.next += 1;
                    },
                    dispatch_rounds,
                )?;
                ("gf2".to_owned(), collect(finished))
            }
            "aff3ct" => {
                let selection = Selection::from_environment()?;
                let prototype = Handle::build(
                    &workload.alist(),
                    k,
                    workload.manifest.n,
                    cap,
                    norm,
                    true,
                    &selection,
                )?;
                let name = prototype.name();
                let (_, finished) = pool::run(
                    &cpus,
                    |_| {
                        Ok(Chunked {
                            worker: Aff3ctWorker::new(&prototype, batch, k)?,
                            chunks: Vec::new(),
                            next: 0,
                        })
                    },
                    |index, state: &mut Chunked<Aff3ctWorker>| {
                        let first = chunk_start(index, state.next);
                        state.worker.decode_batch(workload.frames(first, batch));
                        let decided = state.worker.decisions.clone();
                        state.chunks.push((first, decided, Vec::new()));
                        state.next += 1;
                    },
                    dispatch_rounds,
                )?;
                (format!("{name} {selection:?}"), collect(finished))
            }
            other => return Err(format!("--arm {other} is not gf2 or aff3ct")),
        };
    let mut decisions = vec![0u8; frames * k];
    let mut iterations = vec![None; frames];
    for (first, decided, counted) in finished_chunks.into_iter().flatten() {
        decisions[first * k..(first + batch) * k].copy_from_slice(&decided);
        for (offset, count) in counted.into_iter().enumerate() {
            iterations[first + offset] = Some(count);
        }
    }
    let errors = workload.frame_errors(0, &decisions);
    let errors_match = check_prepared(&errors, &quality, 0);
    let iteration_check = if iterations.iter().all(Option::is_some) {
        let mut sorted: Vec<u32> = iterations.iter().map(|i| i.unwrap()).collect();
        let mean = sorted.iter().map(|i| f64::from(*i)).sum::<f64>() / sorted.len() as f64;
        sorted.sort_unstable();
        let observed = json!({"mean": mean, "p50": quantile(&sorted, 0.5),
            "p90": quantile(&sorted, 0.9), "max": sorted.last().copied().unwrap_or(0)});
        let frozen = &quality.iterations;
        let equal = (mean - frozen.mean).abs() < 1e-9
            && quantile(&sorted, 0.5) == frozen.p50
            && quantile(&sorted, 0.9) == frozen.p90
            && sorted.last().copied() == Some(frozen.max);
        Some((observed, equal))
    } else {
        None
    };
    let passed = errors_match.is_ok() && iteration_check.as_ref().is_none_or(|(_, ok)| *ok);
    println!(
        "{}",
        json!({
            "schema": "ldpc-throughput-validation-v1",
            "arm": arm_name,
            "selected": selected,
            "code": code,
            "frames": frames,
            "workers": workers,
            "cpus": cpus,
            "batch": batch,
            "frame_bit_errors_match_prepared": errors_match.as_ref().map(|_| true).unwrap_or(false),
            "frame_bit_error_mismatch": errors_match.err(),
            "iterations_observed": iteration_check.as_ref().map(|(o, _)| o.clone()),
            "iterations_match_prepared": iteration_check.as_ref().map(|(_, ok)| *ok),
            "passed": passed,
        })
    );
    if passed {
        Ok(())
    } else {
        Err("the steady-state path disagrees with the prepared quality".to_owned())
    }
}

fn collect<W>(finished: Vec<Finished<Chunked<W>>>) -> Vec<Vec<(usize, Vec<u8>, Vec<u32>)>> {
    finished.into_iter().map(|done| done.state.chunks).collect()
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ldpc-throughput-validate: {error}");
            ExitCode::FAILURE
        }
    }
}
