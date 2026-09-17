//! Decodes one measured cell with one candidate arm and records its quality.
//!
//! Untimed: this binary establishes frame and bit error counts, iteration
//! distributions and the code identity each arm decoded, and measures no
//! duration. It reads the frozen recorded bundles through the canonical bundle
//! contract of `c077a88b` rather than a private loader, so every arm decodes
//! the digest-verified matrix and the digest-verified LLRs.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use gf2_coding::ldpc::{
    DecoderAlgorithm, DecoderConfig, EdgeLayout, LdpcCode, LdpcDecoder, MinSumRule,
    QuasiCyclicLdpc,
};
use gf2_coding::llr::Llr;
use gf2_core::BitVec;
use ldpc_candidate_prototypes::{LayeredDecoder, QcDecoder, QuantizedDecoder, QuantizedRule};
use ldpc_survey::{load_manifest, read_alist_code, read_codewords, read_llrs, verify_digests};

/// The frozen decoder contract of the measured cells.
const ITERATION_CAP: usize = 50;
const NORMALIZATION: f32 = 0.75;

/// The fixed-point image of [`NORMALIZATION`]: `3 / 4`, exact in the alphabet.
const QUANTIZED_RULE: QuantizedRule = QuantizedRule::Normalized {
    numerator: 3,
    shift: 2,
};

struct Args {
    bundle: PathBuf,
    arm: String,
    cell: String,
    qc_base: Option<(u8, usize)>,
    out: PathBuf,
}

fn parse_args() -> Result<Args, String> {
    let mut bundle = None;
    let mut arm = None;
    let mut cell = None;
    let mut qc_base = None;
    let mut out = None;
    let mut argv = std::env::args().skip(1);
    while let Some(flag) = argv.next() {
        let mut value = || argv.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--bundle" => bundle = Some(PathBuf::from(value()?)),
            "--arm" => arm = Some(value()?),
            "--cell" => cell = Some(value()?),
            "--out" => out = Some(PathBuf::from(value()?)),
            "--qc-base" => {
                let text = value()?;
                let (graph, lifting) = text
                    .split_once(':')
                    .ok_or_else(|| format!("--qc-base wants graph:lifting, got {text}"))?;
                qc_base = Some((
                    graph.parse::<u8>().map_err(|e| e.to_string())?,
                    lifting.parse::<usize>().map_err(|e| e.to_string())?,
                ));
            }
            other => return Err(format!("unknown flag {other}")),
        }
    }
    Ok(Args {
        bundle: bundle.ok_or("--bundle is required")?,
        arm: arm.ok_or("--arm is required")?,
        cell: cell.ok_or("--cell is required")?,
        qc_base,
        out: out.ok_or("--out is required")?,
    })
}

/// One arm's decode of one frame.
struct FrameResult {
    bit_errors: usize,
    iterations: usize,
    syndrome_check_passed: bool,
}

/// A decoder under one declared contract.
enum Arm {
    Canonical(Box<LdpcDecoder>),
    Qc(Box<QcDecoder>),
    Layered(Box<LayeredDecoder>),
    QuantizedNarrow(Box<QuantizedDecoder<i8>>, f32),
    QuantizedWide(Box<QuantizedDecoder<i16>>, f32),
}

impl Arm {
    fn decode(&mut self, llrs: &[Llr], k: usize, transmitted: &[u8]) -> FrameResult {
        match self {
            Self::Canonical(decoder) => {
                let mut codeword = BitVec::with_capacity(llrs.len());
                let outcome = decoder.decode_codeword_into(llrs, ITERATION_CAP, &mut codeword);
                let bit_errors = (0..k)
                    .filter(|&index| usize::from(codeword.get(index)) != usize::from(transmitted[index]))
                    .count();
                FrameResult {
                    bit_errors,
                    iterations: outcome.iterations,
                    syndrome_check_passed: outcome.syndrome_check_passed,
                }
            }
            Self::Qc(decoder) => {
                let outcome = decoder.decode(llrs, ITERATION_CAP, true);
                let bits = decoder.hard_bits();
                FrameResult {
                    bit_errors: count_errors(bits, k, transmitted),
                    iterations: outcome.iterations,
                    syndrome_check_passed: outcome.syndrome_check_passed,
                }
            }
            Self::Layered(decoder) => {
                let outcome = decoder.decode(llrs, ITERATION_CAP, true);
                let bits = decoder.hard_bits();
                FrameResult {
                    bit_errors: count_errors(bits, k, transmitted),
                    iterations: outcome.sweeps,
                    syndrome_check_passed: outcome.syndrome_check_passed,
                }
            }
            Self::QuantizedNarrow(decoder, _) => {
                let outcome = decoder.decode(llrs, ITERATION_CAP, true);
                let bits = decoder.hard_bits();
                FrameResult {
                    bit_errors: count_errors(bits, k, transmitted),
                    iterations: outcome.iterations,
                    syndrome_check_passed: outcome.syndrome_check_passed,
                }
            }
            Self::QuantizedWide(decoder, _) => {
                let outcome = decoder.decode(llrs, ITERATION_CAP, true);
                let bits = decoder.hard_bits();
                FrameResult {
                    bit_errors: count_errors(bits, k, transmitted),
                    iterations: outcome.iterations,
                    syndrome_check_passed: outcome.syndrome_check_passed,
                }
            }
        }
    }

    /// The contract fields the receipt records for this arm.
    fn contract(&self) -> BTreeMap<&'static str, String> {
        let mut fields = BTreeMap::new();
        let (precision, schedule, scale) = match self {
            Self::Canonical(_) => ("f32", "flooding", None),
            Self::Qc(_) => ("f32", "flooding-qc-intra-frame", None),
            Self::Layered(_) => ("f32", "horizontal-layered", None),
            Self::QuantizedNarrow(_, scale) => ("i8", "flooding", Some(*scale)),
            Self::QuantizedWide(_, scale) => ("i16", "flooding", Some(*scale)),
        };
        fields.insert("precision", precision.to_string());
        fields.insert("schedule", schedule.to_string());
        fields.insert("stopping", "syndrome".to_string());
        fields.insert("iteration_cap", ITERATION_CAP.to_string());
        fields.insert(
            "normalization",
            match self {
                Self::QuantizedNarrow(..) | Self::QuantizedWide(..) => {
                    "fixed-point 3>>2".to_string()
                }
                _ => format!("normalized-min-sum {NORMALIZATION}"),
            },
        );
        if let Some(scale) = scale {
            fields.insert("channel_scale", format!("{scale}"));
        }
        fields.insert(
            "iteration_unit",
            match self {
                Self::Layered(_) => "layer-sweep".to_string(),
                _ => "flooding-iteration".to_string(),
            },
        );
        fields
    }
}

fn count_errors(bits: &[bool], k: usize, transmitted: &[u8]) -> usize {
    (0..k)
        .filter(|&index| usize::from(bits[index]) != usize::from(transmitted[index]))
        .count()
}

/// Builds the arm named on the command line over `code`.
fn build_arm(
    name: &str,
    code: &LdpcCode,
    qc: Option<&QuasiCyclicLdpc>,
) -> Result<Arm, String> {
    let rule = MinSumRule::Normalized(NORMALIZATION);
    match name {
        "canonical-f32" => Ok(Arm::Canonical(Box::new(LdpcDecoder::with_config(
            code.clone(),
            DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(NORMALIZATION), true),
        )))),
        "qc-f32" => {
            let qc = qc.ok_or("the qc-f32 arm needs --qc-base")?;
            Ok(Arm::Qc(Box::new(QcDecoder::new(qc, rule))))
        }
        "layered-f32" => Ok(Arm::Layered(Box::new(LayeredDecoder::new(code, rule)))),
        other => {
            let (width, scale) = other
                .strip_prefix("quantized-")
                .and_then(|rest| rest.split_once(':'))
                .ok_or_else(|| format!("unknown arm {other}"))?;
            let scale: f32 = scale.parse().map_err(|_| format!("bad scale in {other}"))?;
            match width {
                "i8" => Ok(Arm::QuantizedNarrow(
                    Box::new(QuantizedDecoder::new(code, QUANTIZED_RULE, scale)),
                    scale,
                )),
                "i16" => Ok(Arm::QuantizedWide(
                    Box::new(QuantizedDecoder::new(code, QUANTIZED_RULE, scale)),
                    scale,
                )),
                other => Err(format!("unknown alphabet {other}")),
            }
        }
    }
}

/// Whether the lifted base graph reproduces the recorded matrix edge for edge.
fn same_graph(code: &LdpcCode, qc: &QuasiCyclicLdpc) -> bool {
    let lifted = LdpcCode::from_quasi_cyclic(qc);
    if lifted.n() != code.n() || lifted.m() != code.m() {
        return false;
    }
    let left = EdgeLayout::from_parity_check(code.parity_check_matrix());
    let right = EdgeLayout::from_parity_check(lifted.parity_check_matrix());
    left.check_offsets() == right.check_offsets() && left.check_edge_var() == right.check_edge_var()
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &Args) -> Result<(), String> {
    let manifest = load_manifest(&args.bundle).map_err(|e| e.to_string())?;
    verify_digests(&args.bundle, &manifest).map_err(|e| e.to_string())?;
    let code =
        read_alist_code(&args.bundle.join(ldpc_survey::ALIST_FILE)).map_err(|e| e.to_string())?;
    let n = manifest.n;
    let k = manifest.k;
    let llrs = read_llrs(&args.bundle, manifest.frames, n).map_err(|e| e.to_string())?;
    let codewords = read_codewords(&args.bundle, manifest.frames, n).map_err(|e| e.to_string())?;

    let qc = match args.qc_base {
        Some((graph, lifting)) => Some(QuasiCyclicLdpc::nr_5g(graph, lifting)),
        None => None,
    };
    let graph_identity = match qc.as_ref() {
        Some(qc) => {
            if !same_graph(&code, qc) {
                return Err("the lifted base graph is not the recorded parity-check matrix".into());
            }
            Some(format!("nr-bg{}-z{}", args.qc_base.unwrap().0, args.qc_base.unwrap().1))
        }
        None => None,
    };

    // The declared per-cell input transform; each is a property of the NR rate
    // matching and neither changes the transmitted bits the arm is scored on.
    let lifting = args.qc_base.map(|(_, z)| z).unwrap_or(0);
    let (frames, punctured_prefix, filler_positions): (Vec<usize>, usize, usize) =
        match args.cell.as_str() {
            "recorded" => ((0..manifest.frames).collect(), 0, 0),
            "punctured" => {
                if lifting == 0 {
                    return Err("the punctured cell needs --qc-base for its lifting size".into());
                }
                ((0..manifest.frames).collect(), 2 * lifting, 0)
            }
            "filler" => {
                if lifting == 0 {
                    return Err("the filler cell needs --qc-base for its lifting size".into());
                }
                // Filler positions carry bits the rate matching forced to zero,
                // so only the all-zero frames of the bundle carry zero there.
                let all_zero: Vec<usize> = (0..manifest.frames)
                    .filter(|&frame| codewords[frame * n..frame * n + k].iter().all(|bit| *bit == 0))
                    .collect();
                (all_zero, 0, lifting)
            }
            other => return Err(format!("unknown cell {other}")),
        };

    let mut arm = build_arm(&args.arm, &code, qc.as_ref())?;
    let mut per_frame = Vec::with_capacity(frames.len());
    let mut iterations = Vec::with_capacity(frames.len());
    let mut syndrome_failures = 0usize;
    let mut buffer = vec![Llr::zero(); n];

    for &frame in &frames {
        for (slot, &value) in buffer.iter_mut().zip(&llrs[frame * n..(frame + 1) * n]) {
            *slot = Llr::new(value);
        }
        for slot in buffer.iter_mut().take(punctured_prefix) {
            *slot = Llr::zero();
        }
        for slot in buffer[k - filler_positions..k].iter_mut() {
            *slot = Llr::new(15.0);
        }
        let result = arm.decode(&buffer, k, &codewords[frame * n..(frame + 1) * n]);
        if !result.syndrome_check_passed {
            syndrome_failures += 1;
        }
        per_frame.push(result.bit_errors);
        iterations.push(result.iterations);
    }

    let frame_errors = per_frame.iter().filter(|errors| **errors > 0).count();
    let bit_errors: usize = per_frame.iter().sum();
    let bits = frames.len() * k;
    let mut histogram: BTreeMap<usize, usize> = BTreeMap::new();
    for count in &iterations {
        *histogram.entry(*count).or_default() += 1;
    }

    let contract = arm.contract();
    let record = serde_json::json!({
        "schema": "ldpc-candidate-quality-v1",
        "arm": args.arm,
        "cell": args.cell,
        "bundle": {
            "path": args.bundle.to_string_lossy(),
            "code": manifest.code,
            "n": n,
            "k": k,
            "h_sha256": manifest.h_sha256,
            "llrs_sha256": manifest.llrs_sha256,
            "codewords_sha256": manifest.codewords_sha256,
            "esn0_db": manifest.esn0_db,
            "bundle_frames": manifest.frames,
        },
        "input_transform": {
            "punctured_prefix": punctured_prefix,
            "filler_positions": filler_positions,
        },
        "graph_identity": graph_identity,
        "contract": contract,
        "frames": frames.len(),
        "frame_indices": frames,
        "bits": bits,
        "per_frame_information_bit_errors": per_frame,
        "frame_errors": frame_errors,
        "bit_errors": bit_errors,
        "syndrome_failures": syndrome_failures,
        "iterations": iterations,
        "iteration_histogram": histogram.into_iter().map(|(k, v)| (k.to_string(), v)).collect::<BTreeMap<_, _>>(),
    });

    if let Some(parent) = args.out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(
        &args.out,
        serde_json::to_string_pretty(&record).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| e.to_string())?;
    println!("{}", args.out.display());
    Ok(())
}
