//! gf2 isolated check-node-update arm (jit:07ca8585).
//!
//! Reads the canonical child request, derives the prepared variable-to-check
//! message array of the declared frames, and starts one pinned worker per
//! resolved CPU with its own copy of that array and its own output array. A
//! timed call makes every worker run one flooding check-node pass over every
//! prepared frame, through the production `EdgeLayout` runs and
//! `min_sum_check_row`. Nothing else is in the timed call: no variable update,
//! no syndrome, no conversion and no allocation.

use gf2_coding::llr::Llr;
use ldpc_survey::arm::{self, ArmResult, ConversionCosts, DecoderCase, ARM_RESULT_SCHEMA};
use ldpc_throughput::workload::{check_request, Workload};
use ldpc_update_arms::cell::run_kernel_cell;
use ldpc_update_arms::checksum_hex;
use ldpc_update_arms::prepare::{check_pass, prepare, rule, KernelCase};
use std::hint::black_box;
use std::process::ExitCode;
use std::time::Instant;

/// One worker's own copy of the prepared messages and its output array.
struct Kernel {
    input: Vec<Llr>,
    output: Vec<Llr>,
}

fn llrs(values: &[f32]) -> Vec<Llr> {
    values.iter().copied().map(Llr::new).collect()
}

fn values(messages: &[Llr]) -> Vec<f32> {
    messages.iter().map(|message| message.value()).collect()
}

fn run() -> Result<(), String> {
    let request = arm::read_request()?;
    let case: KernelCase = serde_json::from_value(request.case.clone())
        .map_err(|error| format!("case does not decode: {error}"))?;
    let frames = case.frames as usize;
    let decoder_case = DecoderCase {
        bundle: case.bundle.clone(),
        code: case.code.clone(),
        // An isolated check-node pass performs one update and no termination;
        // the fields the bundle loader and the request check read are the
        // bundle, the code and the per-call frame count.
        iteration_cap: 1,
        normalization_factor: case.normalization_factor,
        syndrome_stopping: false,
        batch_size: case.frames,
        quality_frames: 0,
        decisions_out: None,
    };
    let workload = Workload::load(&decoder_case)?;
    check_request(&request, &decoder_case, workload.manifest.frames)?;

    let setup_start = Instant::now();
    let code = ldpc_survey::read_alist_code(&workload.alist()).map_err(|e| e.to_string())?;
    if code.n() != workload.manifest.n
        || code.k() != workload.manifest.k
        || code.parity_check_matrix().nnz() != workload.manifest.nnz
    {
        return Err("the bundle dimensions differ from the built code".to_owned());
    }
    let prepared = prepare(&code, workload.frames(0, frames), &case)?;
    let (layout, edges) = (&prepared.layout, prepared.edges());
    let update = rule(&case);
    let cell = run_kernel_cell(
        "gf2-checknode",
        &request,
        setup_start,
        (&case.input_checksum, &case.output_checksum),
        |_| {
            Ok(Kernel {
                input: llrs(&prepared.var_to_chk),
                output: vec![Llr::zero(); prepared.var_to_chk.len()],
            })
        },
        |kernel: &mut Kernel| {
            for frame in 0..frames {
                let range = frame * edges..(frame + 1) * edges;
                check_pass(
                    layout,
                    update,
                    &kernel.input[range.clone()],
                    &mut kernel.output[range],
                );
            }
            black_box(&kernel.output);
        },
        |kernel: &Kernel| {
            (
                checksum_hex(&values(&kernel.input)),
                checksum_hex(&values(&kernel.output)),
            )
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
            "gf2-coding min_sum_check_row over EdgeLayout check runs, {update:?}; \
             {frames} prepared frames of {edges} edges per call after {} warm-up rounds, \
             input {} output {}; {} pinned workers, {} process threads",
            case.warmup_rounds,
            checksum_hex(&prepared.var_to_chk),
            case.output_checksum,
            cell.workers_observed,
            cell.threads.ready,
        )),
        conversion: Some(ConversionCosts {
            setup_ns: cell.setup_ns,
            // The prepared array is derived in setup and read as it stands;
            // the timed call converts nothing and fills no batch.
            pack_ns: 0,
            unpack_ns: 0,
            batch_fill_ns: 0,
            dispatch_ns: cell.dispatch_ns,
        }),
        // A kernel-isolated cell declares no decoder, so the runner asks for no
        // per-frame quality: this pass decodes no frame.
        quality: None,
    };
    arm::write_result(&result)
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("gf2-checknode-arm: {error}");
            ExitCode::FAILURE
        }
    }
}
