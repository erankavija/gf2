//! AFF3CT LDPC decoder arm of the baseline survey (jit:c077a88b).
//!
//! Reads the canonical child-v2 request on stdin, decodes the recorded LLRs
//! of one input bundle through the pinned AFF3CT static library, and writes
//! exactly one canonical result line. The shared case fixes the code, the
//! recorded input and the algorithmic declaration; this arm's own decoder
//! family, SIMD strategy and precision travel in its plan environment, so the
//! runner forwards one identical case to both arms of a cell.
//!
//! # Environment
//!
//! | Variable | Meaning |
//! |---|---|
//! | `GF2_AFF3CT_TYPE` | `flooding` or `horizontal-layered` |
//! | `GF2_AFF3CT_IMPLEM` | `MS`, `NMS` or `OMS` |
//! | `GF2_AFF3CT_SIMD` | empty or `INTER` |
//! | `GF2_AFF3CT_PRECISION` | `f32`, `i16` or `i8` |
//! | `GF2_AFF3CT_OFFSET` | offset of the OMS update rule |
//! | `GF2_AFF3CT_QUANT_SCALE` | fixed-point LLR scale; ignored for `f32` |
//! | `GF2_AFF3CT_SYNDROME_DEPTH` | iterations before syndrome detection starts |

use ldpc_survey::arm::{
    self, ArmResult, ArmSettings, ConversionCosts, DecoderCase, Normalization, NormalizationKind,
    Precision, QualityAccumulator, Schedule, Stopping, StoppingKind, ARM_RESULT_SCHEMA,
};
use std::ffi::{CStr, CString};
use std::hint::black_box;
use std::os::raw::{c_char, c_float, c_int, c_uchar, c_void};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

extern "C" {
    fn a3_new(
        alist_path: *const c_char,
        k: c_int,
        n: c_int,
        n_ite: c_int,
        kind: *const c_char,
        implem: *const c_char,
        simd: *const c_char,
        precision: *const c_char,
        norm_factor: c_float,
        offset: c_float,
        enable_syndrome: c_int,
        syndrome_depth: c_int,
        quant_scale: c_float,
        err: *mut c_char,
        errlen: c_int,
    ) -> *mut c_void;
    fn a3_has_counter(handle: *mut c_void) -> c_int;
    fn a3_frames_per_wave(handle: *mut c_void) -> c_int;
    fn a3_name(handle: *mut c_void) -> *const c_char;
    fn a3_decode(
        handle: *mut c_void,
        llrs: *const c_float,
        bits_out: *mut c_uchar,
        frames: c_int,
    ) -> c_int;
    fn a3_decode_counted(
        handle: *mut c_void,
        llrs: *const c_float,
        bits_out: *mut c_uchar,
        frames: c_int,
        ites_out: *mut c_int,
    ) -> c_int;
    fn a3_free(handle: *mut c_void);
}

/// An owned AFF3CT decoder handle.
struct Decoder {
    handle: *mut c_void,
}

impl Drop for Decoder {
    fn drop(&mut self) {
        // SAFETY: `handle` came from `a3_new` and is freed exactly once.
        unsafe { a3_free(self.handle) };
    }
}

fn environment(name: &str, fallback: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| fallback.to_owned())
}

/// Iteration caps the arm builds when AFF3CT exposes no iteration counter.
///
/// AFF3CT's layered decoders offer no hook that observes how many iterations a
/// frame took, so this arm observes the convergence iteration the only way the
/// public interface allows: it repeats the decode under smaller iteration caps
/// and reports the smallest cap at which AFF3CT still declares the syndrome
/// satisfied. The ladder bounds that search to a fixed number of decoder
/// constructions and makes the reported granularity explicit.
const ITERATION_LADDER: [u32; 10] = [1, 2, 3, 4, 6, 8, 12, 16, 24, 32];

/// Smallest ladder cap at which AFF3CT still declares the syndrome satisfied.
///
/// Returns `cap` when no ladder entry converges, which is the honest reading:
/// the wave needed more iterations than the largest ladder entry below the
/// declared cap.
fn observe_on_ladder(
    ladder: &[(u32, Decoder)],
    llrs: &[f32],
    bits: &mut [u8],
    wave: usize,
    cap: u32,
) -> Result<u32, String> {
    let mut found = cap;
    for (limit, decoder) in ladder {
        let mut status = 0 as c_int;
        // SAFETY: input/output contain a full native wave; status is one int.
        let code = unsafe {
            a3_decode_counted(
                decoder.handle,
                llrs.as_ptr(),
                bits.as_mut_ptr(),
                wave as c_int,
                &mut status,
            )
        };
        if code != 0 {
            return Err(format!("ladder decode failed: {code}"));
        }
        if status == -1 {
            found = *limit;
            break;
        }
    }
    Ok(found)
}

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

    let kind = environment("GF2_AFF3CT_TYPE", "flooding");
    let implem = environment("GF2_AFF3CT_IMPLEM", "NMS");
    let simd = environment("GF2_AFF3CT_SIMD", "");
    let precision = environment("GF2_AFF3CT_PRECISION", "f32");
    let offset: f32 = environment("GF2_AFF3CT_OFFSET", "0")
        .parse()
        .map_err(|error| format!("GF2_AFF3CT_OFFSET: {error}"))?;
    let quant_scale: f32 = environment("GF2_AFF3CT_QUANT_SCALE", "8")
        .parse()
        .map_err(|error| format!("GF2_AFF3CT_QUANT_SCALE: {error}"))?;
    let syndrome_depth: i32 = environment("GF2_AFF3CT_SYNDROME_DEPTH", "1")
        .parse()
        .map_err(|error| format!("GF2_AFF3CT_SYNDROME_DEPTH: {error}"))?;

    let alist = CString::new(
        dir.join(ldpc_survey::ALIST_FILE)
            .to_string_lossy()
            .into_owned(),
    )
    .map_err(|error| error.to_string())?;
    let kind_c = CString::new(kind.clone()).map_err(|e| e.to_string())?;
    let implem_c = CString::new(implem.clone()).map_err(|e| e.to_string())?;
    let simd_c = CString::new(simd.clone()).map_err(|e| e.to_string())?;
    let precision_c = CString::new(precision.clone()).map_err(|e| e.to_string())?;
    let mut error_buffer = vec![0 as c_char; 512];

    let build = |n_ite: u32, error_buffer: &mut Vec<c_char>| -> *mut c_void {
        // SAFETY: every pointer is a live NUL-terminated buffer for the call,
        // and the shim reports failure by returning null and filling the
        // error buffer.
        unsafe {
            a3_new(
                alist.as_ptr(),
                manifest.k as c_int,
                manifest.n as c_int,
                n_ite as c_int,
                kind_c.as_ptr(),
                implem_c.as_ptr(),
                simd_c.as_ptr(),
                precision_c.as_ptr(),
                case.normalization_factor,
                offset,
                c_int::from(case.syndrome_stopping),
                syndrome_depth,
                quant_scale,
                error_buffer.as_mut_ptr(),
                error_buffer.len() as c_int,
            )
        }
    };
    let setup_start = Instant::now();
    let handle = build(case.iteration_cap, &mut error_buffer);
    if handle.is_null() {
        // SAFETY: the shim wrote a NUL-terminated message into the buffer.
        let message = unsafe { CStr::from_ptr(error_buffer.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        return Err(format!(
            "aff3ct rejected type={kind} implem={implem} simd={simd:?} precision={precision}: {message}"
        ));
    }
    let decoder = Decoder { handle };
    let setup_ns = setup_start.elapsed().as_nanos() as u64;
    // SAFETY: the handle is live and the name is owned by it.
    let name = unsafe { CStr::from_ptr(a3_name(decoder.handle)) }
        .to_string_lossy()
        .into_owned();
    // SAFETY: the handle is live.
    let wave = unsafe { a3_frames_per_wave(decoder.handle) };
    if wave <= 0 {
        return Err("aff3ct reported a non-positive wave size".to_owned());
    }
    let wave = wave as usize;
    if manifest.frames % wave != 0 {
        return Err(format!(
            "the bundle holds {} frames, which is not a multiple of the {wave}-frame wave",
            manifest.frames
        ));
    }

    // Peak resident memory of the decoder itself, observed before any
    // iteration-ladder handle exists, so the reported footprint describes the
    // arm rather than the observation machinery.
    let memory_bytes = arm::peak_rss_bytes();
    // SAFETY: the handle is live.
    let counts_iterations = unsafe { a3_has_counter(decoder.handle) } != 0;
    let ladder: Vec<(u32, Decoder)> = if counts_iterations || prepared_quality.is_some() {
        Vec::new()
    } else {
        let mut built = Vec::new();
        for cap in ITERATION_LADDER
            .iter()
            .copied()
            .filter(|c| *c < case.iteration_cap)
        {
            let mut buffer = vec![0 as c_char; 512];
            let laddered = build(cap, &mut buffer);
            if laddered.is_null() {
                return Err(format!("aff3ct rejected an iteration cap of {cap}"));
            }
            built.push((cap, Decoder { handle: laddered }));
        }
        built
    };

    let llrs =
        ldpc_survey::read_llrs(&dir, manifest.frames, manifest.n).map_err(|e| e.to_string())?;
    let transmitted = ldpc_survey::read_codewords(&dir, manifest.frames, manifest.n)
        .map_err(|e| e.to_string())?;

    // Untimed quality pass over whole waves, with the per-wave convergence
    // iteration the counted decode observes.
    let quality_waves = if prepared_quality.is_some() {
        0
    } else {
        (case.quality_frames as usize / wave)
            .max(1)
            .min(manifest.frames / wave)
    };
    let quality_frames = quality_waves * wave;
    let mut bits = vec![0u8; quality_frames * manifest.k];
    let mut iterations = vec![0 as c_int; quality_waves];
    let mut scratch_bits = vec![0u8; wave * manifest.k];
    let mut accumulator = QualityAccumulator::default();
    // One call per wave, so the recorded latency is the wave latency AFF3CT
    // actually delivers rather than an average over the whole pass.
    for index in 0..quality_waves {
        let start = Instant::now();
        // SAFETY: the offsets stay inside `llrs` and `bits`, which hold
        // quality_frames * n floats and quality_frames * k bytes.
        let status = unsafe {
            a3_decode_counted(
                decoder.handle,
                llrs.as_ptr().add(index * wave * manifest.n),
                bits.as_mut_ptr().add(index * wave * manifest.k),
                wave as c_int,
                iterations.as_mut_ptr().add(index),
            )
        };
        if status != 0 {
            return Err(format!("aff3ct counted decode failed with status {status}"));
        }
        let latency = start.elapsed().as_nanos() as u64;
        if !counts_iterations {
            // The primary handle reported the syndrome outcome as -1 (passed)
            // or -2 (never passed). A frame that never converged is recorded
            // at the declared cap; one that did is located on the ladder.
            iterations[index] = if iterations[index] == -1 {
                observe_on_ladder(
                    &ladder,
                    &llrs[index * wave * manifest.n..(index + 1) * wave * manifest.n],
                    &mut scratch_bits,
                    wave,
                    case.iteration_cap,
                )? as c_int
            } else {
                case.iteration_cap as c_int
            };
        }
        for frame in index * wave..(index + 1) * wave {
            accumulator.observe(
                &bits[frame * manifest.k..(frame + 1) * manifest.k],
                &transmitted[frame * manifest.n..frame * manifest.n + manifest.k],
                iterations[index] as u32,
                latency,
            );
        }
    }
    if let Some(path) = case.decisions_out.as_ref() {
        std::fs::write(path, &bits).map_err(|error| format!("cannot write decisions: {error}"))?;
    }

    // One operation consumes the same recorded batch in both arms. The
    // candidate may partition that batch into its native SIMD waves.
    drop(ladder);
    let batch = if request.role == "validation" {
        wave
    } else {
        case.batch_size as usize
    };
    if batch == 0 || batch % wave != 0 || manifest.frames % batch != 0 {
        return Err(format!(
            "batch {batch} must be a multiple of wave {wave} and divide the bundle"
        ));
    }
    let mut sink = 0u8;
    let mut body = |bank: usize| {
        let mut buffer = vec![0 as c_char; 512];
        let timed = Decoder {
            handle: build(case.iteration_cap, &mut buffer),
        };
        assert!(!timed.handle.is_null(), "validated AFF3CT construction");
        let mut scratch = vec![0u8; batch * manifest.k];
        let first = arm::timed_batch_start(bank, batch, manifest.frames);
        // SAFETY: the validated batch and offset fit the input and output.
        let status = unsafe {
            a3_decode(
                timed.handle,
                llrs.as_ptr().add(first * manifest.n),
                scratch.as_mut_ptr(),
                batch as c_int,
            )
        };
        assert_eq!(status, 0, "validated AFF3CT decode");
        sink = sink.wrapping_add(black_box(scratch)[0]);
    };
    let windows = arm::timing_windows(&request, &mut body);
    black_box(sink);

    let settings = ArmSettings {
        precision: match precision.as_str() {
            "f32" => Precision::F32,
            "i16" => Precision::I16,
            "i8" => Precision::I8,
            _ => return Err("unsupported precision".into()),
        },
        schedule: match kind.as_str() {
            "flooding" => Schedule::Flooding,
            _ => Schedule::Layered,
        },
        normalization: Normalization {
            kind: match implem.as_str() {
                "MS" => NormalizationKind::MinSum,
                "OMS" => NormalizationKind::OffsetMinSum,
                _ => NormalizationKind::NormalizedMinSum,
            },
            factor: match implem.as_str() {
                "MS" => None,
                "OMS" => Some(f64::from(offset)),
                _ => Some(f64::from(case.normalization_factor)),
            },
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
        batch_size: wave as u32,
    };
    let result = ArmResult {
        schema: ARM_RESULT_SCHEMA.to_owned(),
        windows,
        cache_state_applied: request.cache_state.clone(),
        workers_observed: 1,
        cpus_observed: arm::observed_cpus(),
        selected_path: Some(format!(
            "{name} simd={} precision={precision}",
            if simd.is_empty() { "none" } else { &simd }
        )),
        conversion: Some(ConversionCosts {
            setup_ns,
            // The shim converts each wave's recorded f32 LLRs into the
            // decoder's own precision and interleaving inside the timed call,
            // so the conversion is charged to the measurement rather than
            // reported as a separate untimed cost.
            pack_ns: 0,
            unpack_ns: 0,
            batch_fill_ns: 0,
            dispatch_ns: 0,
        }),
        quality: Some(
            prepared_quality.unwrap_or_else(|| accumulator.finish(memory_bytes, settings)),
        ),
    };
    arm::write_result(&result)
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("aff3ct-ldpc-arm: {error}");
            ExitCode::FAILURE
        }
    }
}
