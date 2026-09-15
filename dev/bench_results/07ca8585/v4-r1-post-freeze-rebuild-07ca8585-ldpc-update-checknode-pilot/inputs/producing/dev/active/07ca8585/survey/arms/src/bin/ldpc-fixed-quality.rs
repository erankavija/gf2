//! Untimed prepared quality under fixed iteration counts (jit:07ca8585).
//!
//! REQ-10's full-iteration granularity decodes at the iteration cap with
//! syndrome stopping off, so both arms perform an equal, declared number of
//! iterations. The measured harness reports prepared quality evidence and
//! refuses an arm whose settings differ from the settings that evidence was
//! produced under, and the reused `c077a88b` corpus was produced under syndrome
//! stopping. This tool produces the corpus those cells require: every recorded
//! frame of a bundle decoded once, with stopping disabled, by the same decoder
//! the timed arm builds, written in the `ArmQuality` shape the arms read.
//!
//! Nothing here is timed to support a claim. Per-frame latencies are recorded
//! because the quality record carries a median latency field, as the `c077a88b`
//! corpus does; no conclusion of this issue rests on them.
//!
//! Usage:
//!   ldpc-fixed-quality --arm gf2|aff3ct --bundle DIR --code NAME --out FILE
//!       [--cap 50] [--norm 0.75]

use gf2_coding::ldpc::{DecoderConfig, LdpcDecoder};
use gf2_coding::llr::Llr;
use ldpc_survey::arm::{
    self, ArmSettings, DecoderCase, Normalization, NormalizationKind, Precision,
    QualityAccumulator, Schedule, Stopping, StoppingKind,
};
use ldpc_throughput::aff3ct::{Handle, Selection};
use ldpc_throughput::gf2::config;
use ldpc_throughput::workload::Workload;
use std::process::ExitCode;
use std::time::Instant;

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (mut arm_name, mut bundle, mut code, mut out) =
        (String::new(), String::new(), String::new(), String::new());
    let (mut cap, mut norm) = (50u32, 0.75f32);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or(format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--arm" => arm_name = value,
            "--bundle" => bundle = value,
            "--code" => code = value,
            "--out" => out = value,
            "--cap" => cap = value.parse().map_err(|_| format!("{flag}: not a number"))?,
            "--norm" => norm = value.parse().map_err(|_| format!("{flag}: not a number"))?,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if out.is_empty() {
        return Err("--out names the quality file to write".to_owned());
    }
    let case = DecoderCase {
        bundle,
        code,
        iteration_cap: cap,
        normalization_factor: norm,
        // The whole point of this corpus: the decoder runs the cap.
        syndrome_stopping: false,
        batch_size: 1,
        quality_frames: 0,
        decisions_out: None,
    };
    let workload = Workload::load(&case)?;
    let (n, k, frames) = (
        workload.manifest.n,
        workload.manifest.k,
        workload.manifest.frames,
    );
    let mut accumulator = QualityAccumulator::default();
    let batch_size = match arm_name.as_str() {
        "gf2" => {
            let built =
                ldpc_survey::read_alist_code(&workload.alist()).map_err(|e| e.to_string())?;
            let settings: DecoderConfig = config(norm, false);
            let mut decoder = LdpcDecoder::with_config(built, settings);
            let mut packed = vec![Llr::zero(); n];
            for frame in 0..frames {
                let recorded = workload.frames(frame, 1);
                for (slot, value) in packed.iter_mut().zip(recorded) {
                    *slot = Llr::new(*value);
                }
                let start = Instant::now();
                let decoded = decoder.decode_to_codeword(&packed, cap as usize);
                let latency = start.elapsed().as_nanos() as u64;
                let decided: Vec<u8> = (0..k)
                    .map(|position| u8::from(decoded.decoded_bits.get(position)))
                    .collect();
                let sent = &workload.codewords[frame * n..frame * n + k];
                accumulator.observe(&decided, sent, decoded.iterations as u32, latency);
            }
            1
        }
        "aff3ct" => {
            let selection = Selection::from_environment()?;
            let mut handle = Handle::build(&workload.alist(), k, n, cap, norm, false, &selection)?;
            let wave = handle.wave();
            if wave != 1 {
                return Err(format!(
                    "this corpus records one frame per decode; the selected decoder consumes {wave}"
                ));
            }
            let mut decided = vec![0u8; k];
            for frame in 0..frames {
                let start = Instant::now();
                handle.decode(workload.frames(frame, 1), &mut decided, 1);
                let latency = start.elapsed().as_nanos() as u64;
                let sent = &workload.codewords[frame * n..frame * n + k];
                // With stopping disabled AFF3CT performs exactly the cap; the
                // decoder exposes no per-frame counter and needs none.
                accumulator.observe(&decided, sent, cap, latency);
            }
            wave as u32
        }
        other => return Err(format!("--arm {other} is not gf2 or aff3ct")),
    };
    let settings = ArmSettings {
        precision: Precision::F32,
        schedule: Schedule::Flooding,
        normalization: Normalization {
            kind: NormalizationKind::NormalizedMinSum,
            factor: Some(f64::from(norm)),
        },
        iteration_cap: cap,
        stopping: Stopping {
            kind: StoppingKind::Fixed,
            crc: None,
        },
        batch_size,
    };
    let quality = accumulator.finish(arm::peak_rss_bytes(), settings);
    let mut text = serde_json::to_string_pretty(&quality).map_err(|e| e.to_string())?;
    text.push('\n');
    std::fs::write(&out, text).map_err(|e| e.to_string())?;
    println!(
        "{out}: {} frames, {} frame errors, {} bit errors at the cap {cap}",
        quality.frames, quality.frame_errors, quality.bit_errors
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ldpc-fixed-quality: {error}");
            ExitCode::FAILURE
        }
    }
}
