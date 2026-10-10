//! Untimed quality replay of the AFF3CT float sum-product INTRA decoder.
//!
//! `aff3ct-intra-quality CASE OUT` reads the decoder case of a plan cell from
//! the JSON file `CASE`, replays the first `quality_frames` frames of its
//! bundle once and writes their quality record to `OUT`, the record the arm
//! takes in `GF2_LDPC_QUALITY`.

use ldpc_batch_arms::aff3ct_intra::{replay_quality, settings, Handle, LINKED_AFF3CT};
use ldpc_survey::arm::DecoderCase;
use ldpc_throughput::workload::Workload;
use std::process::ExitCode;

fn run() -> Result<(), String> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [case, out] = arguments.as_slice() else {
        return Err("usage: aff3ct-intra-quality CASE OUT".to_owned());
    };
    let case: DecoderCase =
        serde_json::from_slice(&std::fs::read(case).map_err(|e| format!("{case}: {e}"))?)
            .map_err(|error| format!("case does not decode: {error}"))?;
    let workload = Workload::load(&case)?;
    let mut prototype = Handle::build(
        &workload.alist(),
        workload.manifest.k,
        workload.manifest.n,
        case.iteration_cap,
        case.syndrome_stopping,
    )?;
    let decoder = prototype.observed()?;
    let settings = settings(
        case.iteration_cap,
        case.syndrome_stopping,
        decoder.wave_frames as u32,
    );
    let quality = replay_quality(
        &mut prototype,
        &workload,
        case.quality_frames as usize,
        settings,
    )?;
    let record = serde_json::to_vec(&quality).map_err(|e| e.to_string())?;
    std::fs::write(out, record).map_err(|e| format!("{out}: {e}"))?;
    eprintln!(
        "{}",
        serde_json::json!({
            "kind": "aff3ct-quality-replay",
            "decoder": decoder,
            "linked_aff3ct": LINKED_AFF3CT,
        })
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("aff3ct-intra-quality: {error}");
            ExitCode::FAILURE
        }
    }
}
