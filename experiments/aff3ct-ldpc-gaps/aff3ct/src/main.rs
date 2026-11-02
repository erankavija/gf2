use aff3ct_ldpc_arm::{Aff3ctWorker, Handle, Selection};
use aff3ct_ldpc_gaps::arm::{self, ArmResult, DecoderCase, ARM_RESULT_SCHEMA};
use aff3ct_ldpc_gaps::driver::{report_iterations, run_cell, IterationSource};
use aff3ct_ldpc_gaps::workload::{check_request, Workload};
use std::process::ExitCode;
use std::time::Instant;

fn run() -> Result<(), String> {
    let request = arm::read_request()?;
    let quality = arm::prepared_quality()?
        .ok_or("steady-state cells reuse prepared quality; GF2_LDPC_QUALITY is unset")?;
    let case: DecoderCase = serde_json::from_value(request.case.clone())
        .map_err(|error| format!("case does not decode: {error}"))?;
    let workload = Workload::load(&case)?;
    check_request(&request, &case, workload.manifest.frames)?;
    let selection = Selection::from_environment()?;
    let (n, k) = (workload.manifest.n, workload.manifest.k);
    let batch = case.batch_size as usize;
    let llrs = workload.frames(0, batch);

    let setup_start = Instant::now();
    let prototype = Handle::build(
        &workload.alist(),
        k,
        n,
        case.iteration_cap,
        case.normalization_factor,
        case.syndrome_stopping,
        &selection,
    )?;
    let settings = selection.settings(
        case.normalization_factor,
        case.iteration_cap,
        case.syndrome_stopping,
        prototype.wave() as u32,
    );
    if quality.settings != settings {
        return Err("the prepared quality was produced under other settings".to_owned());
    }
    let name = prototype.name();
    let prototype = std::sync::Mutex::new(prototype);
    let cell = run_cell(
        "aff3ct",
        &request,
        &workload,
        &quality,
        setup_start,
        // AFF3CT converts inside every decode call; no separate stage exists.
        0,
        |_| {
            let prototype = prototype.lock().map_err(|e| e.to_string())?;
            Aff3ctWorker::new(&prototype, batch, k)
        },
        |worker: &mut Aff3ctWorker| worker.decode_batch(llrs),
        |worker: &Aff3ctWorker| &worker.decisions,
        |index, worker| {
            let mut prototype = prototype.lock().map_err(|e| e.to_string())?;
            let iterations = worker.replay_iterations(&mut prototype, llrs)?;
            report_iterations("aff3ct", index, IterationSource::ReplayDerived, &iterations);
            Ok(())
        },
    )?;
    let result = ArmResult {
        schema: ARM_RESULT_SCHEMA.to_owned(),
        windows: cell.windows,
        calibrated: true,
        cache_state_applied: request.cache_state.clone(),
        workers_observed: cell.workers_observed,
        cpus_observed: arm::observed_cpus(),
        selected_path: Some(format!(
            "{name} simd={} precision={}, clone per worker; {} pinned workers, {} process threads",
            if selection.simd.is_empty() {
                "none"
            } else {
                &selection.simd
            },
            selection.precision,
            cell.workers_observed,
            cell.threads.ready
        )),
        conversion: Some(cell.conversion),
        quality: Some(quality),
    };
    arm::write_result(&result)
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("aff3ct-throughput-arm: {error}");
            ExitCode::FAILURE
        }
    }
}
