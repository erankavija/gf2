//! Profile driver of the LDPC batch decoder for `profile-session.sh`.
//!
//! Runs the per-worker decode of a batch arm's cell, on the route `--arm`
//! names, in the [`profile`] region, and checks each
//! worker's placement and per-frame bit errors against the prepared quality
//! record `GF2_LDPC_QUALITY` names. Prints one JSON record to standard
//! output; the placement report goes to standard error.
//!
//! Usage:
//!   ldpc-batch-profile --arm single-frame|lane --bundle DIR --code NAME
//!       --core-arm ARM [--batch 16] [--passes 1] [--iteration-cap 50]
//!       [--norm 0.75] [--perf-control CTL,ACK]

use ldpc_batch_arms::{declared_lane_backend, BatchRoute, BatchWorker};
use ldpc_survey::arm;
use ldpc_throughput::gf2::config;
use ldpc_throughput::workload::verify_workers;
use std::process::ExitCode;
use std::time::Instant;

// The session interface of `profile-session.sh`, shared with the `3be770d5`
// driver by path under the exception `harness-path-dependencies`.
#[path = "../../../../../1a379447-zen3-cpu-performance/3be770d5/survey/harness/src/profile.rs"]
mod profile;
use profile::{profiled, record, ProfileOptions};

/// The route `--arm` names; `lane` runs on the backend
/// [`ldpc_batch_arms::LANE_BACKEND_VAR`] declares.
fn route(name: &str) -> Result<BatchRoute, String> {
    match name {
        "single-frame" => Ok(BatchRoute::SingleFrame),
        "lane" => declared_lane_backend().map(BatchRoute::Lane),
        other => Err(format!("--arm {other} is not single-frame or lane")),
    }
}

fn run() -> Result<(), String> {
    let options = ProfileOptions::parse(std::env::args().skip(1))?;
    let route = route(&options.arm)?;
    let quality = arm::prepared_quality()?.ok_or(
        "the profile checks decisions against prepared quality; GF2_LDPC_QUALITY is unset",
    )?;
    let (workload, cpus) = options.load()?;
    let batch = options.batch as usize;
    let llrs = workload.frames(0, batch);
    let setup_start = Instant::now();
    let code = ldpc_survey::read_alist_code(&workload.alist()).map_err(|e| e.to_string())?;
    let decoder_config = config(options.norm, true);
    let cap = options.iteration_cap as usize;
    let (region, finished) = profiled(
        &cpus,
        options.passes,
        options.perf_control.as_ref(),
        setup_start,
        |_| BatchWorker::new(&code, decoder_config, cap, batch, route),
        |worker: &mut BatchWorker| worker.decode_batch(llrs),
    )?;
    let workers = verify_workers(
        "gf2-batch-profile",
        &finished,
        region.calls,
        region.threads,
        |worker: &BatchWorker| &worker.decisions,
        &workload,
        &quality,
    )?;
    let selected = format!(
        "gf2-coding LdpcBatchDecoder f32 flooding NMS, route {route:?}, lane workspace {} \
         bytes per worker",
        finished[0].state.workspace_bytes()
    );
    let record = record(
        &options, &workload, &cpus, workers, &region, &selected, None,
    );
    println!("{record}");
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ldpc-batch-profile: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::route;
    use ldpc_batch_arms::BatchRoute;

    #[test]
    fn a_profile_case_names_its_route() {
        assert_eq!(route("single-frame"), Ok(BatchRoute::SingleFrame));
        assert!(route("gf2").is_err());
    }
}
