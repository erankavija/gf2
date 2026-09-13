//! gf2 steady-state LDPC throughput arm (jit:3be770d5).
//!
//! Reads the canonical child request, starts one pinned worker per resolved
//! CPU, and gives each worker its own `gf2-coding` decoder built from a clone
//! of the recorded-AList code. A timed call makes every worker decode the
//! declared per-worker batch of recorded frames. The arm reports the prepared
//! quality evidence only after checking that every worker decoded the batch
//! with exactly its frozen per-frame error counts.

use gf2_coding::llr::Llr;
use ldpc_survey::arm::{
    self, ArmResult, DecoderCase, Normalization, NormalizationKind, Precision, Schedule, Stopping,
    StoppingKind, ARM_RESULT_SCHEMA,
};
use ldpc_throughput::driver::run_cell;
use ldpc_throughput::gf2::{config, Gf2Worker};
use ldpc_throughput::workload::{check_request, median_u64, Workload};
use std::hint::black_box;
use std::process::ExitCode;
use std::time::Instant;

/// Untimed repetitions of the per-batch conversion probe.
const PACK_PROBES: usize = 9;

fn run() -> Result<(), String> {
    let request = arm::read_request()?;
    let quality = arm::prepared_quality()?
        .ok_or("steady-state cells reuse prepared quality; GF2_LDPC_QUALITY is unset")?;
    let case: DecoderCase = serde_json::from_value(request.case.clone())
        .map_err(|error| format!("case does not decode: {error}"))?;
    let workload = Workload::load(&case)?;
    check_request(&request, &case, workload.manifest.frames)?;
    let settings = arm::ArmSettings {
        precision: Precision::F32,
        schedule: Schedule::Flooding,
        normalization: Normalization {
            kind: NormalizationKind::NormalizedMinSum,
            factor: Some(f64::from(case.normalization_factor)),
        },
        iteration_cap: case.iteration_cap,
        stopping: Stopping {
            kind: if case.syndrome_stopping {
                StoppingKind::Syndrome
            } else {
                StoppingKind::Fixed
            },
            crc: None,
        },
        // One frame per decoder invocation; the per-worker batch is a
        // sequence of such invocations.
        batch_size: 1,
    };
    if quality.settings != settings {
        return Err("the prepared quality was produced under other settings".to_owned());
    }
    let batch = case.batch_size as usize;
    let llrs = workload.frames(0, batch);

    // Untimed conversion probe: one worker batch of recorded f32 values into
    // the decoder's `Llr`, the conversion each timed call repeats per frame.
    let mut packed = vec![Llr::zero(); llrs.len()];
    let pack_ns = median_u64(
        (0..PACK_PROBES)
            .map(|_| {
                let start = Instant::now();
                for (slot, value) in packed.iter_mut().zip(black_box(llrs)) {
                    *slot = Llr::new(*value);
                }
                black_box(&packed);
                start.elapsed().as_nanos() as u64
            })
            .collect(),
    );
    drop(packed);

    let setup_start = Instant::now();
    let code = ldpc_survey::read_alist_code(&workload.alist()).map_err(|e| e.to_string())?;
    if code.n() != workload.manifest.n
        || code.k() != workload.manifest.k
        || code.parity_check_matrix().nnz() != workload.manifest.nnz
    {
        return Err("the bundle dimensions differ from the built code".to_owned());
    }
    let decoder_config = config(case.normalization_factor, case.syndrome_stopping);
    let cap = case.iteration_cap as usize;
    let cell = run_cell(
        "gf2",
        &request,
        &workload,
        &quality,
        setup_start,
        pack_ns,
        |_| Ok(Gf2Worker::new(&code, decoder_config, batch, cap)),
        |worker: &mut Gf2Worker| worker.decode_batch(llrs),
        |worker: &Gf2Worker| &worker.decisions,
    )?;
    let result = ArmResult {
        schema: ARM_RESULT_SCHEMA.to_owned(),
        windows: cell.windows,
        calibrated: true,
        cache_state_applied: request.cache_state.clone(),
        workers_observed: cell.workers_observed,
        cpus_observed: arm::observed_cpus(),
        selected_path: Some(format!(
            "gf2-coding LdpcDecoder f32 flooding normalized min-sum, reused decoder per worker; \
             {} pinned workers, {} process threads",
            cell.workers_observed, cell.threads.ready
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
            eprintln!("gf2-throughput-arm: {error}");
            ExitCode::FAILURE
        }
    }
}
