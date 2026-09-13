//! AFF3CT isolated check-node-update arm (jit:07ca8585).
//!
//! The matched arm of `gf2-checknode-arm`. It derives the prepared
//! variable-to-check array with the same function that arm calls, permutes it
//! into AFF3CT's variable-major edge order, and starts one pinned worker per
//! resolved CPU with its own AFF3CT check-node pass over the recorded AList. A
//! timed call makes every worker run one flooding check-node pass over every
//! prepared frame through AFF3CT's own `tools::Update_rule_NMS`.
//!
//! The permutation is AFF3CT's own `transpose` array, and the arm refuses to run
//! unless it equals the canonical layout's `check_edge_to_var_edge`, so the two
//! arms are known to place the same message on the same edge rather than assumed
//! to. The output is permuted back into the canonical order before its checksum,
//! so both arms are checked against the same declared result.

use ldpc_survey::arm::{self, ArmResult, ConversionCosts, DecoderCase, ARM_RESULT_SCHEMA};
use ldpc_throughput::workload::{check_request, Workload};
use ldpc_update_arms::aff3ct_update::Handle;
use ldpc_update_arms::cell::run_kernel_cell;
use ldpc_update_arms::checksum_hex;
use ldpc_update_arms::prepare::{prepare, KernelCase};
use std::hint::black_box;
use std::process::ExitCode;
use std::time::Instant;

/// One worker's AFF3CT pass, its variable-major input and its output.
struct Kernel {
    handle: Handle,
    input: Vec<f32>,
    output: Vec<f32>,
}

/// Permutes `canonical` into variable-major order: slot `to_slot[e]` of the
/// result takes canonical edge `e`.
fn to_var_major(canonical: &[f32], to_slot: &[u32], frames: usize) -> Vec<f32> {
    let edges = to_slot.len();
    let mut permuted = vec![0.0f32; frames * edges];
    for frame in 0..frames {
        let base = frame * edges;
        for (edge, slot) in to_slot.iter().enumerate() {
            permuted[base + *slot as usize] = canonical[base + edge];
        }
    }
    permuted
}

/// Permutes a variable-major array back into canonical check-major order.
fn to_check_major(permuted: &[f32], to_slot: &[u32], frames: usize) -> Vec<f32> {
    let edges = to_slot.len();
    let mut canonical = vec![0.0f32; frames * edges];
    for frame in 0..frames {
        let base = frame * edges;
        for (edge, slot) in to_slot.iter().enumerate() {
            canonical[base + edge] = permuted[base + *slot as usize];
        }
    }
    canonical
}

fn run() -> Result<(), String> {
    let request = arm::read_request()?;
    let case: KernelCase = serde_json::from_value(request.case.clone())
        .map_err(|error| format!("case does not decode: {error}"))?;
    let frames = case.frames as usize;
    let decoder_case = DecoderCase {
        bundle: case.bundle.clone(),
        code: case.code.clone(),
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
    let prepared = prepare(&code, workload.frames(0, frames), &case)?;
    let edges = prepared.edges();
    let alist = workload.alist();
    let prototype = Handle::build(&alist, case.normalization_factor)?;
    if prototype.edges() != edges
        || prototype.checks() != prepared.layout.m()
        || prototype.variables() != prepared.layout.n()
    {
        return Err(format!(
            "aff3ct read {} variables, {} checks and {} edges; the canonical layout has {}, {} and {edges}",
            prototype.variables(),
            prototype.checks(),
            prototype.edges(),
            prepared.layout.n(),
            prepared.layout.m(),
        ));
    }
    let slots = prototype.transpose()?;
    if slots.as_slice() != prepared.layout.check_edge_to_var_edge() {
        return Err(
            "aff3ct's transpose differs from the canonical check-edge-to-variable-edge map, \
             so the two arms would not place the same message on the same edge"
                .to_owned(),
        );
    }
    let permuted = to_var_major(&prepared.var_to_chk, &slots, frames);
    let name = prototype.name();
    drop(prototype);
    let cell = run_kernel_cell(
        "aff3ct-checknode",
        &request,
        setup_start,
        (&case.input_checksum, &case.output_checksum),
        |_| {
            Ok(Kernel {
                handle: Handle::build(&alist, case.normalization_factor)?,
                input: permuted.clone(),
                output: vec![0.0; permuted.len()],
            })
        },
        |kernel: &mut Kernel| {
            for frame in 0..frames {
                let range = frame * edges..(frame + 1) * edges;
                let (input, output) = (&kernel.input[range.clone()], &mut kernel.output[range]);
                kernel.handle.check_pass(input, output);
            }
            black_box(&kernel.output);
        },
        |kernel: &Kernel| {
            (
                checksum_hex(&to_check_major(&kernel.input, &slots, frames)),
                checksum_hex(&to_check_major(&kernel.output, &slots, frames)),
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
            "{name}; {frames} prepared frames of {edges} edges per call after {} warm-up rounds, \
             input {} output {}; {} pinned workers, {} process threads",
            case.warmup_rounds,
            checksum_hex(&prepared.var_to_chk),
            case.output_checksum,
            cell.workers_observed,
            cell.threads.ready,
        )),
        conversion: Some(ConversionCosts {
            setup_ns: cell.setup_ns,
            // The permutation into AFF3CT's edge order happens once in setup;
            // the timed call converts nothing and fills no batch.
            pack_ns: 0,
            unpack_ns: 0,
            batch_fill_ns: 0,
            dispatch_ns: cell.dispatch_ns,
        }),
        quality: None,
    };
    arm::write_result(&result)
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("aff3ct-checknode-arm: {error}");
            ExitCode::FAILURE
        }
    }
}
