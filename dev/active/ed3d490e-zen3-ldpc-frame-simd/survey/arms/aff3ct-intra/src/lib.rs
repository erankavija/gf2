//! AFF3CT float sum-product INTRA arm of the batch decoder comparison.
//!
//! The steady-state operation of the `3be770d5` AFF3CT arm on the decoder
//! [`Handle::build_spa_intra`] builds: a prototype from the recorded AList and
//! one AFF3CT `clone()` per pinned worker, which converts and decodes the
//! recorded f32 LLRs inside the timed call.

use ldpc_survey::arm::{
    self, ArmResult, ArmSettings, DecoderCase, Normalization, NormalizationKind, Precision,
    Quality, QualityAccumulator, Schedule, Stopping, StoppingKind, ARM_RESULT_SCHEMA,
};
use ldpc_throughput::aff3ct::{Aff3ctWorker, Handle, LinkedAff3ct, ObservedDecoder, LINKED_AFF3CT};
use ldpc_throughput::driver::run_cell;
use ldpc_throughput::workload::{check_request, Workload};
use serde::Serialize;
use std::time::Instant;

/// The arm's name in placement reports and diagnostics.
pub const ARM: &str = "aff3ct-spa-intra";

/// Iterations before syndrome detection starts: AFF3CT's default, which the
/// pinned AFF3CT arms declare.
pub const SYNDROME_DEPTH: i32 = 1;

/// The settings the INTRA decoder decodes with, `wave` frames per decode.
#[must_use]
pub fn settings(iteration_cap: u32, syndrome_stopping: bool, wave: u32) -> ArmSettings {
    ArmSettings {
        precision: Precision::F32,
        schedule: Schedule::Flooding,
        normalization: Normalization {
            kind: NormalizationKind::SumProduct,
            factor: None,
        },
        iteration_cap,
        stopping: Stopping {
            kind: if syndrome_stopping {
                StoppingKind::Syndrome
            } else {
                StoppingKind::Fixed
            },
            crc: None,
        },
        batch_size: wave,
    }
}

/// The iteration-counting prototype of `case` over the AList of `workload`.
///
/// # Errors
///
/// As [`Handle::build_spa_intra`].
pub fn prototype(workload: &Workload, case: &DecoderCase) -> Result<Handle, String> {
    Handle::build_spa_intra(
        &workload.alist(),
        workload.manifest.k,
        workload.manifest.n,
        case.iteration_cap,
        case.syndrome_stopping,
        SYNDROME_DEPTH,
    )
}

/// One replay of the first `frames` recorded frames on the counting
/// `prototype`, scored per frame in recorded order.
///
/// Each wave is one call, so a frame's latency and iteration count are those
/// of its wave.
///
/// # Errors
///
/// Returns an error when `frames` is not a positive number of whole waves
/// inside the bundle.
pub fn replay_quality(
    prototype: &mut Handle,
    workload: &Workload,
    frames: usize,
    settings: ArmSettings,
) -> Result<Quality, String> {
    let (n, k) = (workload.manifest.n, workload.manifest.k);
    let wave = prototype.wave();
    if wave == 0 || frames == 0 || !frames.is_multiple_of(wave) || frames > workload.manifest.frames
    {
        return Err(format!(
            "{frames} frames are not whole {wave}-frame waves of the bundle"
        ));
    }
    let memory_bytes = arm::peak_rss_bytes();
    let mut decisions = vec![0u8; wave * k];
    let mut accumulator = QualityAccumulator::default();
    for first in (0..frames).step_by(wave) {
        let start = Instant::now();
        let iterations =
            prototype.decode_counted(workload.frames(first, wave), &mut decisions, wave);
        let latency = start.elapsed().as_nanos() as u64;
        for (offset, decided) in decisions.chunks_exact(k).enumerate() {
            let sent = &workload.codewords[(first + offset) * n..][..k];
            accumulator.observe(decided, sent, iterations[0], latency);
        }
    }
    Ok(accumulator.finish(memory_bytes, settings))
}

/// Untimed diagnostics the arm writes to standard error, which the runner
/// journals as a child diagnostic.
#[derive(Debug, Serialize)]
struct Diagnostics {
    kind: &'static str,
    arm: &'static str,
    decoder: ObservedDecoder,
    linked_aff3ct: LinkedAff3ct,
}

/// Runs one steady-state cell of the INTRA decoder and writes the arm result.
///
/// # Errors
///
/// Returns an error when the request or the bundle is unusable, when
/// `GF2_LDPC_QUALITY` names no readable prepared quality record or one of
/// other settings, or when a worker violates its placement or decodes other
/// than the prepared per-frame error counts.
pub fn run() -> Result<(), String> {
    let request = arm::read_request()?;
    let quality = arm::prepared_quality()?
        .ok_or("steady-state cells reuse prepared quality; GF2_LDPC_QUALITY is unset")?;
    let case: DecoderCase = serde_json::from_value(request.case.clone())
        .map_err(|error| format!("case does not decode: {error}"))?;
    let workload = Workload::load(&case)?;
    check_request(&request, &case, workload.manifest.frames)?;
    let k = workload.manifest.k;
    let batch = case.batch_size as usize;
    let llrs = workload.frames(0, batch);

    let setup_start = Instant::now();
    let prototype = prototype(&workload, &case)?;
    let decoder = prototype.observed()?;
    let settings = settings(
        case.iteration_cap,
        case.syndrome_stopping,
        decoder.wave_frames as u32,
    );
    if quality.settings != settings {
        return Err("the prepared quality was produced under other settings".to_owned());
    }
    let name = prototype.name();
    let cell = run_cell(
        ARM,
        &request,
        &workload,
        &quality,
        setup_start,
        // AFF3CT converts inside every decode call; no separate stage exists.
        0,
        |_| Aff3ctWorker::new(&prototype, batch, k),
        |worker: &mut Aff3ctWorker| worker.decode_batch(llrs),
        |worker: &Aff3ctWorker| &worker.decisions,
    )?;
    let diagnostics = Diagnostics {
        kind: "aff3ct-decoder-diagnostics",
        arm: ARM,
        decoder,
        linked_aff3ct: LINKED_AFF3CT,
    };
    eprintln!(
        "{}",
        serde_json::to_string(&diagnostics).map_err(|e| e.to_string())?
    );
    let result = ArmResult {
        schema: ARM_RESULT_SCHEMA.to_owned(),
        windows: cell.windows,
        calibrated: true,
        cache_state_applied: request.cache_state.clone(),
        workers_observed: cell.workers_observed,
        cpus_observed: arm::observed_cpus(),
        selected_path: Some(format!(
            "{name} simd=INTRA precision=f32, clone per worker; {} pinned workers, {} process \
             threads",
            cell.workers_observed, cell.threads.ready
        )),
        conversion: Some(cell.conversion),
        quality: Some(quality),
    };
    arm::write_result(&result)
}

#[cfg(test)]
mod tests {
    use super::settings;
    use ldpc_survey::arm::NormalizationKind;

    #[test]
    fn the_settings_declare_sum_product_without_a_factor() {
        let declared = settings(20, false, 1);
        assert_eq!(declared.normalization.kind, NormalizationKind::SumProduct);
        assert_eq!(declared.normalization.factor, None);
        assert_eq!(declared.batch_size, 1);
    }
}
