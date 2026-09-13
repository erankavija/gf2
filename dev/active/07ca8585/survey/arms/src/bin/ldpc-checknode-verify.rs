//! Untimed parity check of the two isolated check-node arms (jit:07ca8585).
//!
//! Runs the gf2 and the AFF3CT check-node pass in one process over the same
//! prepared array and reports whether their outputs are bit-identical, together
//! with the canonical checksums the timed cells declare and the graph facts the
//! comparison rests on. Nothing here is timed, so it runs outside the benchmark
//! mutex; its output is this issue's committed evidence that the isolated
//! comparison is matched and the source of the checksums in the addendum.
//!
//! Usage:
//!   ldpc-checknode-verify --bundle DIR --code NAME [--frames 8]
//!       [--warmup 8] [--norm 0.75]

use gf2_coding::llr::Llr;
use ldpc_survey::arm::DecoderCase;
use ldpc_throughput::workload::Workload;
use ldpc_update_arms::aff3ct_update::Handle;
use ldpc_update_arms::checksum_hex;
use ldpc_update_arms::prepare::{check_pass, prepare, rule, KernelCase};
use serde_json::json;
use std::collections::BTreeMap;
use std::process::ExitCode;

fn run() -> Result<bool, String> {
    let mut args = std::env::args().skip(1);
    let (mut bundle, mut code) = (String::new(), String::new());
    let (mut frames, mut warmup, mut norm) = (8u32, 8u32, 0.75f32);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or(format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--bundle" => bundle = value,
            "--code" => code = value,
            "--frames" => frames = value.parse().map_err(|_| format!("{flag}: not a number"))?,
            "--warmup" => warmup = value.parse().map_err(|_| format!("{flag}: not a number"))?,
            "--norm" => norm = value.parse().map_err(|_| format!("{flag}: not a number"))?,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let case = KernelCase {
        bundle: bundle.clone(),
        code: code.clone(),
        normalization_factor: norm,
        warmup_rounds: warmup,
        frames,
        input_checksum: String::new(),
        output_checksum: String::new(),
    };
    let decoder_case = DecoderCase {
        bundle,
        code: code.clone(),
        iteration_cap: 1,
        normalization_factor: norm,
        syndrome_stopping: false,
        batch_size: frames,
        quality_frames: 0,
        decisions_out: None,
    };
    let workload = Workload::load(&decoder_case)?;
    let built = ldpc_survey::read_alist_code(&workload.alist()).map_err(|e| e.to_string())?;
    let count = frames as usize;
    let prepared = prepare(&built, workload.frames(0, count), &case)?;
    let edges = prepared.edges();

    // gf2: the production check runs over the canonical check-major arrays.
    let input: Vec<Llr> = prepared.var_to_chk.iter().copied().map(Llr::new).collect();
    let mut output = vec![Llr::zero(); input.len()];
    for frame in 0..count {
        let range = frame * edges..(frame + 1) * edges;
        check_pass(
            &prepared.layout,
            rule(&case),
            &input[range.clone()],
            &mut output[range],
        );
    }
    let gf2_output: Vec<f32> = output.iter().map(|message| message.value()).collect();

    // AFF3CT: its own update rule over its own variable-major edge order.
    let mut handle = Handle::build(&workload.alist(), norm)?;
    let slots = handle.transpose()?;
    let canonical_transpose = slots.as_slice() == prepared.layout.check_edge_to_var_edge();
    let mut permuted_in = vec![0.0f32; prepared.var_to_chk.len()];
    for frame in 0..count {
        let base = frame * edges;
        for (edge, slot) in slots.iter().enumerate() {
            permuted_in[base + *slot as usize] = prepared.var_to_chk[base + edge];
        }
    }
    let mut permuted_out = vec![0.0f32; permuted_in.len()];
    for frame in 0..count {
        let range = frame * edges..(frame + 1) * edges;
        handle.check_pass(&permuted_in[range.clone()], &mut permuted_out[range]);
    }
    let mut aff3ct_output = vec![0.0f32; permuted_out.len()];
    for frame in 0..count {
        let base = frame * edges;
        for (edge, slot) in slots.iter().enumerate() {
            aff3ct_output[base + edge] = permuted_out[base + *slot as usize];
        }
    }

    let differing = gf2_output
        .iter()
        .zip(&aff3ct_output)
        .filter(|(a, b)| a.to_bits() != b.to_bits())
        .count();
    let mut degrees: BTreeMap<usize, usize> = BTreeMap::new();
    for check in 0..prepared.layout.m() {
        *degrees.entry(prepared.layout.check_range(check).len()).or_default() += 1;
    }
    let identical = differing == 0 && canonical_transpose;
    println!(
        "{}",
        json!({
            "schema": "ldpc-checknode-parity-v1",
            "code": code,
            "aff3ct_rule": handle.name(),
            "variables": prepared.layout.n(),
            "checks": prepared.layout.m(),
            "edges": edges,
            "max_check_degree": prepared.layout.max_check_degree(),
            "check_degrees": degrees.iter().map(|(d, c)| json!([d, c])).collect::<Vec<_>>(),
            "frames": count,
            "warmup_rounds": warmup,
            "normalization_factor": norm,
            "aff3ct_transpose_is_the_canonical_map": canonical_transpose,
            "input_checksum": checksum_hex(&prepared.var_to_chk),
            "gf2_output_checksum": checksum_hex(&gf2_output),
            "aff3ct_output_checksum": checksum_hex(&aff3ct_output),
            "differing_outputs": differing,
            "outputs_bit_identical": identical,
        })
    );
    Ok(identical)
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => {
            eprintln!("ldpc-checknode-verify: the two arms are not matched");
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("ldpc-checknode-verify: {error}");
            ExitCode::FAILURE
        }
    }
}
