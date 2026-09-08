//! gf2 LDPC decoder arm of the baseline survey (jit:c077a88b).
//!
//! Reads the canonical child-v2 request on stdin, decodes the recorded LLRs
//! of one input bundle with the production `gf2-coding` decoder, and writes
//! exactly one canonical result line. The arm reports the settings it
//! actually used, so a matched-algorithm cell is checked against its
//! declaration rather than trusted.

use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig, LdpcDecoder};
use gf2_coding::llr::Llr;
use gf2_sim::testutil::ComparisonCode;
use ldpc_survey::arm::{
    self, ArmResult, ArmSettings, ConversionCosts, DecoderCase, Normalization, NormalizationKind,
    Precision, QualityAccumulator, Schedule, Stopping, StoppingKind, ARM_RESULT_SCHEMA,
};
use std::hint::black_box;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

fn run() -> Result<(), String> {
    let request = arm::read_request()?;
    let prepared_quality = arm::prepared_quality()?;
    let case: DecoderCase = serde_json::from_value(request.case.clone())
        .map_err(|error| format!("case does not decode: {error}"))?;

    let dir = PathBuf::from(&case.bundle);
    let manifest = ldpc_survey::load_manifest(&dir).map_err(|e| e.to_string())?;
    ldpc_survey::verify_digests(&dir, &manifest).map_err(|e| e.to_string())?;
    if manifest.code != case.code {
        return Err(format!(
            "bundle holds {} but the case declares {}",
            manifest.code, case.code
        ));
    }
    let comparison = ComparisonCode::parse(&manifest.code)?;
    let code = comparison.build();
    if code.n() != manifest.n || code.k() != manifest.k {
        return Err("the bundle dimensions differ from the built code".to_owned());
    }

    let raw =
        ldpc_survey::read_llrs(&dir, manifest.frames, manifest.n).map_err(|e| e.to_string())?;
    let transmitted = ldpc_survey::read_codewords(&dir, manifest.frames, manifest.n)
        .map_err(|e| e.to_string())?;
    // Packing the recorded f32 LLRs into the decoder's `Llr` newtype is this
    // arm's whole representation conversion; it is timed and reported.
    let pack_start = Instant::now();
    let frames: Vec<Vec<Llr>> = (0..manifest.frames)
        .map(|frame| {
            raw[frame * manifest.n..(frame + 1) * manifest.n]
                .iter()
                .map(|value| Llr::new(*value))
                .collect()
        })
        .collect();
    let pack_ns = pack_start.elapsed().as_nanos() as u64;

    let setup_start = Instant::now();
    let algorithm = DecoderAlgorithm::NormalizedMinSum(case.normalization_factor);
    let config = DecoderConfig::new(algorithm, case.syndrome_stopping);
    let mut decoder = LdpcDecoder::with_config(code.clone(), config);
    let setup_ns = setup_start.elapsed().as_nanos() as u64;
    let cap = case.iteration_cap as usize;

    // Untimed quality pass. It is deterministic in the recorded input, so
    // every execution of this arm reports the same BER/FER evidence for the
    // declared input identity. Errors are scored over the information window,
    // codeword positions 0..k, which is the window AFF3CT extracts when a
    // decoder carries no encoder-supplied information-bit positions.
    let quality_frames = if prepared_quality.is_some() {
        0
    } else {
        (case.quality_frames as usize).min(manifest.frames)
    };
    let mut accumulator = QualityAccumulator::default();
    let mut unpack_ns = 0u64;
    let mut decoded_bits = vec![0u8; manifest.k];
    let mut decisions = case
        .decisions_out
        .as_ref()
        .map(|_| Vec::with_capacity(quality_frames * manifest.k));
    for frame in 0..quality_frames {
        let start = Instant::now();
        let result = decoder.decode_to_codeword(&frames[frame], cap);
        let latency = start.elapsed().as_nanos() as u64;
        let unpack_start = Instant::now();
        for position in 0..manifest.k {
            decoded_bits[position] = u8::from(result.decoded_bits.get(position));
        }
        unpack_ns += unpack_start.elapsed().as_nanos() as u64;
        accumulator.observe(
            &decoded_bits,
            &transmitted[frame * manifest.n..frame * manifest.n + manifest.k],
            result.iterations as u32,
            latency,
        );
        if let Some(sink) = decisions.as_mut() {
            sink.extend_from_slice(&decoded_bits);
        }
    }
    if let (Some(path), Some(sink)) = (case.decisions_out.as_ref(), decisions.as_ref()) {
        std::fs::write(path, sink).map_err(|error| format!("cannot write decisions: {error}"))?;
    }

    let batch = case.batch_size as usize;
    if batch == 0 || manifest.frames % batch != 0 {
        return Err("batch must divide the recorded frame count".into());
    }
    let mut sink = 0usize;
    let mut body = |bank: usize| {
        let config = DecoderConfig::new(algorithm, case.syndrome_stopping);
        let mut timed_decoder = LdpcDecoder::with_config(code.clone(), config);
        for step in 0..batch {
            let frame = arm::timed_batch_start(bank, batch, manifest.frames) + step;
            let packed: Vec<Llr> = raw[frame * manifest.n..(frame + 1) * manifest.n]
                .iter()
                .map(|v| Llr::new(*v))
                .collect();
            let result = timed_decoder.decode_to_codeword(black_box(&packed), cap);
            let output: Vec<u8> = (0..manifest.k)
                .map(|i| u8::from(result.decoded_bits.get(i)))
                .collect();
            sink = sink.wrapping_add(result.iterations + usize::from(black_box(output)[0]));
        }
    };
    let windows = arm::timing_windows(&request, &mut body);
    black_box(sink);

    let settings = ArmSettings {
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
        batch_size: case.batch_size,
    };
    let result = ArmResult {
        schema: ARM_RESULT_SCHEMA.to_owned(),
        windows,
        cache_state_applied: request.cache_state.clone(),
        workers_observed: 1,
        cpus_observed: arm::observed_cpus(),
        selected_path: Some("gf2-coding LdpcDecoder scalar f32 flooding".to_owned()),
        conversion: Some(ConversionCosts {
            setup_ns,
            pack_ns,
            unpack_ns,
            // The arm decodes one frame per call, so no batch is assembled
            // and no dispatch layer sits between the call and the decoder.
            batch_fill_ns: 0,
            dispatch_ns: 0,
        }),
        quality: Some(
            prepared_quality.unwrap_or_else(|| accumulator.finish(arm::peak_rss_bytes(), settings)),
        ),
    };
    arm::write_result(&result)
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("gf2-ldpc-arm: {error}");
            ExitCode::FAILURE
        }
    }
}
