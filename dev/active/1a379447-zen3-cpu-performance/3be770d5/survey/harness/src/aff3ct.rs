//! AFF3CT decoder handles for the throughput arm and profile binaries.
//!
//! A prototype is built through the `c077a88b` shim entry `a3_new` exactly as
//! that survey's arm builds it; every worker then owns a clone made by
//! AFF3CT's own `clone()`. The decoder family, SIMD strategy and precision
//! travel in the arm's environment, as in the survey.
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
    ArmSettings, Normalization, NormalizationKind, Precision, Schedule, Stopping, StoppingKind,
};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_float, c_int, c_uchar, c_void};
use std::path::Path;
use std::sync::Mutex;

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
    fn a3_clone(handle: *mut c_void) -> *mut c_void;
    fn a3_frames_per_wave(handle: *mut c_void) -> c_int;
    fn a3_name(handle: *mut c_void) -> *const c_char;
    fn a3_decode(
        handle: *mut c_void,
        llrs: *const c_float,
        bits_out: *mut c_uchar,
        frames: c_int,
    ) -> c_int;
    fn a3_free(handle: *mut c_void);
}

/// Serializes clones: AFF3CT documents no concurrent `clone()` of one module.
static CLONE: Mutex<()> = Mutex::new(());

/// The AFF3CT decoder an arm's environment selects.
#[derive(Clone, Debug)]
pub struct Selection {
    pub kind: String,
    pub implem: String,
    pub simd: String,
    pub precision: String,
    pub offset: f32,
    pub quant_scale: f32,
    pub syndrome_depth: i32,
}

fn environment(name: &str, fallback: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| fallback.to_owned())
}

impl Selection {
    /// Reads the selection from the environment, with the survey's defaults.
    ///
    /// # Errors
    ///
    /// Returns an error when a numeric variable does not parse.
    pub fn from_environment() -> Result<Self, String> {
        Ok(Self {
            kind: environment("GF2_AFF3CT_TYPE", "flooding"),
            implem: environment("GF2_AFF3CT_IMPLEM", "NMS"),
            simd: environment("GF2_AFF3CT_SIMD", ""),
            precision: environment("GF2_AFF3CT_PRECISION", "f32"),
            offset: environment("GF2_AFF3CT_OFFSET", "0")
                .parse()
                .map_err(|e| format!("GF2_AFF3CT_OFFSET: {e}"))?,
            quant_scale: environment("GF2_AFF3CT_QUANT_SCALE", "8")
                .parse()
                .map_err(|e| format!("GF2_AFF3CT_QUANT_SCALE: {e}"))?,
            syndrome_depth: environment("GF2_AFF3CT_SYNDROME_DEPTH", "1")
                .parse()
                .map_err(|e| format!("GF2_AFF3CT_SYNDROME_DEPTH: {e}"))?,
        })
    }

    /// The settings this selection decodes with, `wave` frames per decode.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsupported precision.
    pub fn settings(
        &self,
        normalization_factor: f32,
        iteration_cap: u32,
        syndrome_stopping: bool,
        wave: u32,
    ) -> Result<ArmSettings, String> {
        Ok(ArmSettings {
            precision: match self.precision.as_str() {
                "f32" => Precision::F32,
                "i16" => Precision::I16,
                "i8" => Precision::I8,
                other => return Err(format!("unsupported precision {other}")),
            },
            schedule: match self.kind.as_str() {
                "flooding" => Schedule::Flooding,
                _ => Schedule::Layered,
            },
            normalization: Normalization {
                kind: match self.implem.as_str() {
                    "MS" => NormalizationKind::MinSum,
                    "OMS" => NormalizationKind::OffsetMinSum,
                    _ => NormalizationKind::NormalizedMinSum,
                },
                factor: match self.implem.as_str() {
                    "MS" => None,
                    "OMS" => Some(f64::from(self.offset)),
                    _ => Some(f64::from(normalization_factor)),
                },
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
        })
    }
}

/// An owned AFF3CT decoder handle.
pub struct Handle {
    raw: *mut c_void,
}

// SAFETY: a handle owns its decoder and buffers exclusively; the harness moves
// each handle to one worker thread.
unsafe impl Send for Handle {}
// SAFETY: the shared-reference methods only read the decoder (`wave`, `name`)
// or clone it under the `CLONE` lock; decoding needs `&mut self`.
unsafe impl Sync for Handle {}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: `raw` came from `a3_new` or `a3_clone` and is freed once.
        unsafe { a3_free(self.raw) };
    }
}

impl Handle {
    /// Builds a decoder from the recorded AList through the survey shim.
    ///
    /// # Errors
    ///
    /// Returns AFF3CT's message when it rejects the configuration.
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        alist: &Path,
        k: usize,
        n: usize,
        iteration_cap: u32,
        normalization_factor: f32,
        syndrome_stopping: bool,
        selection: &Selection,
    ) -> Result<Self, String> {
        let text = |value: &str| CString::new(value).map_err(|e| e.to_string());
        let alist = text(&alist.to_string_lossy())?;
        let (kind, implem) = (text(&selection.kind)?, text(&selection.implem)?);
        let (simd, precision) = (text(&selection.simd)?, text(&selection.precision)?);
        let mut error = vec![0 as c_char; 512];
        // SAFETY: every pointer is a live NUL-terminated buffer for the call,
        // and the shim reports failure by returning null and filling `error`.
        let raw = unsafe {
            a3_new(
                alist.as_ptr(),
                k as c_int,
                n as c_int,
                iteration_cap as c_int,
                kind.as_ptr(),
                implem.as_ptr(),
                simd.as_ptr(),
                precision.as_ptr(),
                normalization_factor,
                selection.offset,
                c_int::from(syndrome_stopping),
                selection.syndrome_depth,
                selection.quant_scale,
                error.as_mut_ptr(),
                error.len() as c_int,
            )
        };
        if raw.is_null() {
            // SAFETY: the shim wrote a NUL-terminated message into `error`.
            let message = unsafe { CStr::from_ptr(error.as_ptr()) };
            return Err(format!(
                "aff3ct rejected {selection:?}: {}",
                message.to_string_lossy()
            ));
        }
        Ok(Self { raw })
    }

    /// An independent handle from AFF3CT's `clone()` of this decoder.
    ///
    /// # Errors
    ///
    /// Returns an error when the clone fails.
    pub fn replicate(&self) -> Result<Self, String> {
        let _serialized = CLONE.lock().map_err(|e| e.to_string())?;
        // SAFETY: `raw` is a live shim handle; the clone reads it only.
        let raw = unsafe { a3_clone(self.raw) };
        if raw.is_null() {
            return Err("aff3ct could not clone the decoder".to_owned());
        }
        Ok(Self { raw })
    }

    /// Frames one native decode consumes.
    #[must_use]
    pub fn wave(&self) -> usize {
        // SAFETY: `raw` is a live shim handle.
        let wave = unsafe { a3_frames_per_wave(self.raw) };
        usize::try_from(wave).unwrap_or(0)
    }

    /// AFF3CT's decoder name with the observed SIMD backend and wave size.
    #[must_use]
    pub fn name(&self) -> String {
        // SAFETY: `raw` is live and owns the returned string.
        unsafe { CStr::from_ptr(a3_name(self.raw)) }
            .to_string_lossy()
            .into_owned()
    }

    /// Decodes `frames` frames of `n` f32 LLRs into `k` decision bytes each.
    ///
    /// # Panics
    ///
    /// Panics when the buffers do not hold whole waves or AFF3CT fails.
    pub fn decode(&mut self, llrs: &[f32], decisions: &mut [u8], frames: usize) {
        let wave = self.wave();
        assert!(
            wave > 0 && frames.is_multiple_of(wave),
            "whole native waves"
        );
        assert!(llrs.len().is_multiple_of(frames) && decisions.len().is_multiple_of(frames));
        // SAFETY: both buffers hold `frames` whole frames, checked above.
        let status = unsafe {
            a3_decode(
                self.raw,
                llrs.as_ptr(),
                decisions.as_mut_ptr(),
                frames as c_int,
            )
        };
        assert_eq!(status, 0, "AFF3CT decode failed");
    }
}

/// One worker's AFF3CT clone and decision buffer.
pub struct Aff3ctWorker {
    pub handle: Handle,
    pub decisions: Vec<u8>,
    frames: usize,
}

impl Aff3ctWorker {
    /// Clones `prototype` for a worker decoding `frames` frames per call.
    ///
    /// # Errors
    ///
    /// Returns an error when the clone fails or `frames` is not a whole
    /// number of native waves.
    pub fn new(prototype: &Handle, frames: usize, k: usize) -> Result<Self, String> {
        let handle = prototype.replicate()?;
        let wave = handle.wave();
        if wave == 0 || !frames.is_multiple_of(wave) {
            return Err(format!(
                "a {frames}-frame batch is not whole {wave}-frame waves"
            ));
        }
        Ok(Self {
            handle,
            decisions: vec![0; frames * k],
            frames,
        })
    }

    /// Decodes the batch; AFF3CT converts and interleaves inside the call.
    pub fn decode_batch(&mut self, llrs: &[f32]) {
        self.handle.decode(llrs, &mut self.decisions, self.frames);
        std::hint::black_box(&self.decisions);
    }
}
