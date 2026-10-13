//! AFF3CT float sum-product INTRA arm of the batch decoder comparison.
//!
//! The steady-state operation of the `3be770d5` AFF3CT arm on the decoder
//! [`Handle::build_spa_intra`] builds: a prototype from the recorded AList and
//! one AFF3CT `clone()` per pinned worker, which converts and decodes inside
//! the timed call the recorded f32 LLRs that [`load`] multiplies by the
//! declared [`llr_factor`].

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

/// Environment variable that declares [`llr_factor`].
pub const LLR_FACTOR_VARIABLE: &str = "GF2_AFF3CT_LLR_FACTOR";

/// The factor a declaration states.
///
/// # Errors
///
/// Returns an error unless `declared` is a power of two of at least one: the
/// factors whose product with an f32 is exact when finite.
pub fn parse_llr_factor(declared: &str) -> Result<f32, String> {
    let factor: f32 = declared
        .parse()
        .map_err(|error| format!("{LLR_FACTOR_VARIABLE}: {error}"))?;
    if factor.is_finite() && factor >= 1.0 && factor.to_bits() & 0x007f_ffff == 0 {
        Ok(factor)
    } else {
        Err(format!(
            "{LLR_FACTOR_VARIABLE}: {declared} is not a power of two of at least one"
        ))
    }
}

/// The factor the environment declares. The recorded bundles carry
/// `r/sigma^2` (`@/issue/11bfbfd9`); two gives the BPSK-AWGN closed form
/// `2r/sigma^2`, the scale sum-product decodes.
///
/// # Errors
///
/// Returns an error when [`LLR_FACTOR_VARIABLE`] is unset or
/// [`parse_llr_factor`] refuses it.
pub fn llr_factor() -> Result<f32, String> {
    let declared = std::env::var(LLR_FACTOR_VARIABLE)
        .map_err(|_| format!("{LLR_FACTOR_VARIABLE} is unset"))?;
    parse_llr_factor(&declared)
}

/// The digest-verified bundle of `case` with every recorded LLR multiplied by
/// `factor`.
///
/// # Errors
///
/// As [`Workload::load`], and when a product is not finite.
pub fn load(case: &DecoderCase, factor: f32) -> Result<Workload, String> {
    let mut workload = Workload::load(case)?;
    for llr in &mut workload.llrs {
        *llr *= factor;
    }
    if workload.llrs.iter().all(|llr| llr.is_finite()) {
        Ok(workload)
    } else {
        Err(format!("an LLR times {factor} is not finite"))
    }
}

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
    llr_factor: f32,
    linked_aff3ct: LinkedAff3ct,
}

fn selected_path(name: &str, factor: f32, workers: u32, threads: u32) -> String {
    format!(
        "{name} simd=INTRA precision=f32 llr_factor={factor}, clone per worker; {workers} pinned \
         workers, {threads} process threads"
    )
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
    let factor = llr_factor()?;
    let workload = load(&case, factor)?;
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
        llr_factor: factor,
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
        selected_path: Some(selected_path(
            &name,
            factor,
            cell.workers_observed,
            cell.threads.ready,
        )),
        conversion: Some(cell.conversion),
        quality: Some(quality),
    };
    arm::write_result(&result)
}

#[cfg(test)]
mod tests {
    use super::{load, parse_llr_factor, selected_path, settings, Diagnostics, ARM};
    use ldpc_survey::arm::{DecoderCase, NormalizationKind};
    use ldpc_survey::{sha256_hex, BundleManifest, CodewordSource};
    use ldpc_throughput::aff3ct::{ObservedDecoder, LINKED_AFF3CT};

    const RECORDED: [f32; 4] = [0.75, -1.337, f32::MIN_POSITIVE / 4.0, 0.0];

    /// One frame of n = 4 holding `llrs`, written under a directory of its own.
    fn bundle(name: &str, llrs: [f32; 4]) -> DecoderCase {
        let dir = std::env::temp_dir().join(format!("aff3ct-intra-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("bundle directory");
        let llrs: Vec<u8> = llrs.iter().flat_map(|llr| llr.to_le_bytes()).collect();
        let files = [
            (ldpc_survey::ALIST_FILE, b"".as_slice()),
            (ldpc_survey::CODEWORDS_FILE, &[0, 1, 0, 0]),
            (ldpc_survey::LLRS_FILE, &llrs),
        ];
        for (file, bytes) in files {
            std::fs::write(dir.join(file), bytes).expect("bundle file");
        }
        let manifest = BundleManifest {
            schema: ldpc_survey::BUNDLE_SCHEMA.to_owned(),
            code: "fixture".to_owned(),
            n: 4,
            k: 2,
            m: 2,
            nnz: 0,
            h_sha256: sha256_hex(files[0].1),
            codewords_sha256: sha256_hex(files[1].1),
            llrs_sha256: sha256_hex(files[2].1),
            frames: 1,
            seed: 0,
            esn0_db: 0.0,
            sigma: 1.0,
            codeword_source: CodewordSource::Both,
            punctured_prefix: 0,
        };
        std::fs::write(
            dir.join(ldpc_survey::MANIFEST_FILE),
            serde_json::to_vec(&manifest).expect("manifest"),
        )
        .expect("manifest file");
        DecoderCase {
            bundle: dir.to_string_lossy().into_owned(),
            code: manifest.code,
            iteration_cap: 1,
            normalization_factor: 1.0,
            syndrome_stopping: false,
            batch_size: 1,
            quality_frames: 1,
            decisions_out: None,
        }
    }

    #[test]
    fn the_settings_declare_sum_product_without_a_factor() {
        let declared = settings(20, false, 1);
        assert_eq!(declared.normalization.kind, NormalizationKind::SumProduct);
        assert_eq!(declared.normalization.factor, None);
        assert_eq!(declared.batch_size, 1);
    }

    #[test]
    fn the_loaded_llrs_are_the_recorded_ones_doubled_exactly() {
        let workload = load(&bundle("doubled", RECORDED), 2.0).expect("bundle");
        let doubled: Vec<u32> = RECORDED.iter().map(|llr| (llr + llr).to_bits()).collect();
        let loaded: Vec<u32> = workload.frames(0, 1).iter().map(|l| l.to_bits()).collect();
        assert_eq!(loaded, doubled);
        assert_eq!(
            load(&bundle("kept", RECORDED), 1.0).expect("bundle").llrs,
            RECORDED
        );
    }

    #[test]
    fn a_product_beyond_f32_is_refused() {
        assert!(load(&bundle("overflow", [f32::MAX, 0.0, 0.0, 0.0]), 2.0).is_err());
    }

    #[test]
    fn a_declared_factor_is_a_power_of_two_of_at_least_one() {
        for (declared, factor) in [("1", 1.0), ("2", 2.0), ("4.0", 4.0)] {
            assert_eq!(parse_llr_factor(declared), Ok(factor));
        }
        for declared in ["3", "0.5", "0", "-2", "inf", "NaN", ""] {
            assert!(parse_llr_factor(declared).is_err(), "{declared:?}");
        }
    }

    #[test]
    fn the_selected_path_and_the_diagnostics_report_the_factor() {
        assert!(selected_path("decoder", 2.0, 1, 2).contains("llr_factor=2"));
        let diagnostics = serde_json::to_value(Diagnostics {
            kind: "aff3ct-decoder-diagnostics",
            arm: ARM,
            decoder: ObservedDecoder {
                class: String::new(),
                backend: String::new(),
                wave_frames: 1,
            },
            llr_factor: 2.0,
            linked_aff3ct: LINKED_AFF3CT,
        })
        .expect("diagnostics");
        assert_eq!(diagnostics["llr_factor"], 2.0);
    }
}
