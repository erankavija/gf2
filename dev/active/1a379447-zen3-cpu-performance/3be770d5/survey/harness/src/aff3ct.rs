//! AFF3CT decoder handles for the throughput arm and profile binaries.
//!
//! A prototype is built through the `c077a88b` shim entry `a3_new` exactly as
//! that survey's arm builds it; every worker then owns a clone made by
//! AFF3CT's own `clone()`. The decoder family, SIMD strategy and precision
//! travel in the arm's environment, as in the survey. The float sum-product
//! INTRA decoder, which `a3_new` does not build, has the constructor
//! [`Handle::build_spa_intra`].
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
use serde::Serialize;
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
    fn a3_intra_new(
        alist_path: *const c_char,
        k: c_int,
        n: c_int,
        n_ite: c_int,
        enable_syndrome: c_int,
        syndrome_depth: c_int,
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
    fn a3_decode_counted(
        handle: *mut c_void,
        llrs: *const c_float,
        bits_out: *mut c_uchar,
        frames: c_int,
        iterations: *mut c_int,
    ) -> c_int;
    fn a3_free(handle: *mut c_void);
}

/// Serializes clones: AFF3CT documents no concurrent `clone()` of one module.
static CLONE: Mutex<()> = Mutex::new(());

/// The AFF3CT tree this harness links, as its build script read it.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct LinkedAff3ct {
    pub revision: &'static str,
    pub static_library_sha256: &'static str,
}

pub const LINKED_AFF3CT: LinkedAff3ct = LinkedAff3ct {
    revision: env!("LDPC_THROUGHPUT_AFF3CT_REVISION"),
    static_library_sha256: env!("LDPC_THROUGHPUT_AFF3CT_LIBRARY_SHA256"),
};

/// What a handle's name states about its decoder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ObservedDecoder {
    /// AFF3CT's module name.
    pub class: String,
    /// MIPP's instruction set of the translation unit.
    pub backend: String,
    /// Frames one native decode consumes.
    pub wave_frames: usize,
}

impl ObservedDecoder {
    /// Reads the `<class> backend=<backend> wave=<frames>` name the shim
    /// handle composes.
    fn parse(name: &str) -> Option<Self> {
        let (rest, wave) = name.rsplit_once(" wave=")?;
        let (class, backend) = rest.rsplit_once(" backend=")?;
        Some(Self {
            class: class.to_owned(),
            backend: backend.to_owned(),
            wave_frames: wave.parse().ok()?,
        })
    }
}

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
    /// Non-null, from `a3_new`, `a3_intra_new` or `a3_clone`, freed on drop.
    raw: *mut c_void,
    /// LLRs the shim reads per frame.
    n: usize,
    /// Decision bytes the shim writes per frame.
    k: usize,
}

// SAFETY: a handle owns its decoder and buffers exclusively; the harness moves
// each handle to one worker thread.
unsafe impl Send for Handle {}
// SAFETY: the shared-reference methods only read the decoder (`wave`, `name`)
// or clone it under the `CLONE` lock; decoding needs `&mut self`.
unsafe impl Sync for Handle {}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: `raw` came from a shim constructor or `a3_clone` and is
        // freed once.
        unsafe { a3_free(self.raw) };
    }
}

/// `k`, `n` and the iteration cap as the shim's integers.
fn shim_dimensions(
    k: usize,
    n: usize,
    iteration_cap: u32,
) -> Result<(c_int, c_int, c_int), String> {
    if k == 0 || k > n {
        return Err(format!("no ({n}, {k}) code has an information window"));
    }
    let shim_int = |value: usize, what: &str| {
        c_int::try_from(value).map_err(|_| format!("{what} {value} exceeds the shim's integer"))
    };
    Ok((
        shim_int(k, "k")?,
        shim_int(n, "n")?,
        shim_int(iteration_cap as usize, "the iteration cap")?,
    ))
}

/// Size of the buffer a shim constructor writes its refusal into.
const ERROR_BYTES: usize = 512;

impl Handle {
    /// Builds a decoder from the recorded AList through the survey shim.
    ///
    /// # Errors
    ///
    /// Returns an error when `0 < k <= n` fails or a dimension or the cap
    /// exceeds the shim's integer, and AFF3CT's message when it rejects the
    /// configuration.
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
        let (shim_k, shim_n, shim_cap) = shim_dimensions(k, n, iteration_cap)?;
        let text = |value: &str| CString::new(value).map_err(|e| e.to_string());
        let alist = text(&alist.to_string_lossy())?;
        let (kind, implem) = (text(&selection.kind)?, text(&selection.implem)?);
        let (simd, precision) = (text(&selection.simd)?, text(&selection.precision)?);
        let mut error = [0 as c_char; ERROR_BYTES];
        // SAFETY: every pointer is a live NUL-terminated buffer for the call,
        // `error` is a live buffer of the length passed, and the shim reports
        // failure by returning null.
        let raw = unsafe {
            a3_new(
                alist.as_ptr(),
                shim_k,
                shim_n,
                shim_cap,
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
                ERROR_BYTES as c_int,
            )
        };
        Self::owned(raw, &error, &format!("{selection:?}"), n, k)
    }

    /// Builds the iteration-counting float sum-product INTRA decoder from the
    /// recorded AList; syndrome detection starts after `syndrome_depth`
    /// iterations.
    ///
    /// # Errors
    ///
    /// As [`Self::build`].
    pub fn build_spa_intra(
        alist: &Path,
        k: usize,
        n: usize,
        iteration_cap: u32,
        syndrome_stopping: bool,
        syndrome_depth: i32,
    ) -> Result<Self, String> {
        let (shim_k, shim_n, shim_cap) = shim_dimensions(k, n, iteration_cap)?;
        let alist = CString::new(alist.to_string_lossy().as_ref()).map_err(|e| e.to_string())?;
        let mut error = [0 as c_char; ERROR_BYTES];
        // SAFETY: `alist` is a live NUL-terminated string and `error` a live
        // buffer of the length passed; the shim reports failure by returning
        // null.
        let raw = unsafe {
            a3_intra_new(
                alist.as_ptr(),
                shim_k,
                shim_n,
                shim_cap,
                c_int::from(syndrome_stopping),
                syndrome_depth,
                error.as_mut_ptr(),
                ERROR_BYTES as c_int,
            )
        };
        Self::owned(raw, &error, "the sum-product INTRA decoder", n, k)
    }

    /// The handle a shim constructor returned, or its refusal of `what` from
    /// `error`.
    fn owned(
        raw: *mut c_void,
        error: &[c_char; ERROR_BYTES],
        what: &str,
        n: usize,
        k: usize,
    ) -> Result<Self, String> {
        if raw.is_null() {
            // SAFETY: `error` starts zeroed and the shim terminates what it
            // writes inside the buffer, so a NUL lies within it.
            let message = unsafe { CStr::from_ptr(error.as_ptr()) };
            return Err(format!(
                "aff3ct rejected {what}: {}",
                message.to_string_lossy()
            ));
        }
        Ok(Self { raw, n, k })
    }

    /// An independent handle from AFF3CT's `clone()` of this decoder; it
    /// counts no iterations.
    ///
    /// # Errors
    ///
    /// Returns an error when the clone fails.
    pub fn replicate(&self) -> Result<Self, String> {
        let _serialized = CLONE.lock().map_err(|e| e.to_string())?;
        // SAFETY: `raw` is a live shim handle; the clone reads it only, and
        // the lock excludes a concurrent clone.
        let raw = unsafe { a3_clone(self.raw) };
        if raw.is_null() {
            return Err("aff3ct could not clone the decoder".to_owned());
        }
        // The shim's clone copies the source's K and N.
        Ok(Self {
            raw,
            n: self.n,
            k: self.k,
        })
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
        // SAFETY: `raw` is live and owns the returned NUL-terminated string,
        // which is copied before the borrow of `self` ends.
        unsafe { CStr::from_ptr(a3_name(self.raw)) }
            .to_string_lossy()
            .into_owned()
    }

    /// The decoder class, backend and wave size the handle reports.
    ///
    /// # Errors
    ///
    /// Returns an error when the name and the wave entry disagree.
    pub fn observed(&self) -> Result<ObservedDecoder, String> {
        let name = self.name();
        ObservedDecoder::parse(&name)
            .filter(|observed| observed.wave_frames > 0 && observed.wave_frames == self.wave())
            .ok_or_else(|| format!("the handle name {name:?} states no decoder of its wave"))
    }

    /// Decodes `frames` frames of `n` f32 LLRs into `k` decision bytes each.
    ///
    /// # Panics
    ///
    /// Panics when `frames` is not a positive number of whole waves, when
    /// `llrs` does not hold exactly `frames * n` LLRs or `decisions` exactly
    /// `frames * k` bytes, or when AFF3CT fails.
    pub fn decode(&mut self, llrs: &[f32], decisions: &mut [u8], frames: usize) {
        let frames = self.checked_frames(llrs, decisions, frames);
        // SAFETY: `raw` is live; the shim reads `frames * n` LLRs and writes
        // `frames * k` bytes, the lengths `checked_frames` established.
        let status = unsafe { a3_decode(self.raw, llrs.as_ptr(), decisions.as_mut_ptr(), frames) };
        assert_eq!(status, 0, "AFF3CT decode failed");
    }

    /// Decodes as [`Self::decode`] and returns the iterations of each wave.
    ///
    /// # Panics
    ///
    /// As [`Self::decode`], and when the handle counts no iterations, as a
    /// clone does.
    pub fn decode_counted(
        &mut self,
        llrs: &[f32],
        decisions: &mut [u8],
        frames: usize,
    ) -> Vec<u32> {
        let shim_frames = self.checked_frames(llrs, decisions, frames);
        let mut iterations = vec![0 as c_int; frames / self.wave()];
        // SAFETY: as in `decode`, and the shim writes one entry of
        // `iterations` per wave, `frames / wave` of them.
        let status = unsafe {
            a3_decode_counted(
                self.raw,
                llrs.as_ptr(),
                decisions.as_mut_ptr(),
                shim_frames,
                iterations.as_mut_ptr(),
            )
        };
        assert_eq!(status, 0, "AFF3CT decode failed");
        iterations
            .into_iter()
            .map(|count| u32::try_from(count).expect("a counting handle"))
            .collect()
    }

    /// The frame count as the shim takes it, once the buffers hold exactly
    /// what the shim reads and writes for it (`@/inv/caller-trusted-fast-paths`).
    fn checked_frames(&self, llrs: &[f32], decisions: &[u8], frames: usize) -> c_int {
        let wave = self.wave();
        assert!(
            wave > 0 && frames > 0 && frames.is_multiple_of(wave),
            "whole native waves"
        );
        assert_eq!(
            Some(llrs.len()),
            frames.checked_mul(self.n),
            "{frames} frames of {} LLRs",
            self.n
        );
        assert_eq!(
            Some(decisions.len()),
            frames.checked_mul(self.k),
            "{frames} frames of {} decision bytes",
            self.k
        );
        c_int::try_from(frames).expect("a frame count within the shim's integer")
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

    /// Replays the repeated prefix on its counting prototype after timing.
    ///
    /// Counts are expanded from native waves to their frames; frames in a SIMD
    /// wave share the wave's stopping decision.
    ///
    /// # Errors
    ///
    /// Returns an error when replay decisions differ from this worker's final
    /// decisions. `prototype` must have the same settings as the timed clone.
    ///
    /// # Panics
    ///
    /// As [`Handle::decode_counted`].
    pub fn replay_iterations(
        &self,
        prototype: &mut Handle,
        llrs: &[f32],
    ) -> Result<Vec<u32>, String> {
        let mut decisions = vec![0; self.decisions.len()];
        let iterations = prototype.decode_counted(llrs, &mut decisions, self.frames);
        if decisions != self.decisions {
            return Err("AFF3CT replay decisions differ from timed-worker decisions".to_owned());
        }
        Ok(iterations
            .into_iter()
            .flat_map(|count| std::iter::repeat_n(count, prototype.wave()))
            .collect())
    }

    /// Decodes the batch; AFF3CT converts and interleaves inside the call.
    pub fn decode_batch(&mut self, llrs: &[f32]) {
        self.handle.decode(llrs, &mut self.decisions, self.frames);
        std::hint::black_box(&self.decisions);
    }
}

#[cfg(test)]
mod tests {
    use super::{Aff3ctWorker, Handle, ObservedDecoder, Selection};
    use std::os::raw::c_int;

    const N: usize = 7;
    const K: usize = 4;
    const CAP: u32 = 20;
    const SYNDROME_DEPTH: i32 = 1;

    /// The (7, 4) Hamming code as an AList file, written once for all tests.
    fn alist() -> &'static std::path::Path {
        static FILE: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
        FILE.get_or_init(|| {
            let path = std::env::temp_dir().join(format!("hamming-{}.alist", std::process::id()));
            let text = "7 3\n3 4\n1 1 2 1 2 2 3\n4 4 4\n\
                        1 0 0\n2 0 0\n1 2 0\n3 0 0\n1 3 0\n2 3 0\n1 2 3\n\
                        1 3 5 7\n2 3 6 7\n4 5 6 7\n";
            std::fs::write(&path, text).expect("alist");
            path
        })
    }

    fn spa_intra() -> Handle {
        Handle::build_spa_intra(alist(), K, N, CAP, true, SYNDROME_DEPTH).expect("decoder")
    }

    /// The flooding normalized min-sum decoder of the survey's defaults.
    fn selected() -> Handle {
        let selection = Selection {
            kind: "flooding".to_owned(),
            implem: "NMS".to_owned(),
            simd: String::new(),
            precision: "f32".to_owned(),
            offset: 0.0,
            quant_scale: 8.0,
            syndrome_depth: SYNDROME_DEPTH,
        };
        Handle::build(alist(), K, N, CAP, 0.75, true, &selection).expect("decoder")
    }

    /// The all-zero codeword with one unreliable wrong position per frame.
    fn llrs(frames: usize) -> Vec<f32> {
        let mut llrs = vec![4.0f32; frames * N];
        for frame in 0..frames {
            llrs[frame * N + frame % N] = -0.5;
        }
        llrs
    }

    #[test]
    fn a_clone_decodes_as_the_counting_prototype() {
        for mut prototype in [spa_intra(), selected()] {
            let wave = prototype.wave();
            let frames = 3 * wave;
            let llrs = llrs(frames);
            let mut counted = vec![1u8; frames * K];
            let iterations = prototype.decode_counted(&llrs, &mut counted, frames);
            assert_eq!(counted, vec![0u8; frames * K]);
            assert_eq!(iterations.len(), frames / wave);
            assert!(iterations.iter().all(|count| (1..CAP).contains(count)));

            let mut worker = Aff3ctWorker::new(&prototype, frames, K).expect("clone");
            worker.decode_batch(&llrs);
            assert_eq!(worker.decisions, counted);
        }
    }

    #[test]
    fn post_timing_replay_matches_each_reused_workers_prefix_decisions() {
        let mut prototype = selected();
        let frames = 8;
        let mut llrs = llrs(frames);
        for (frame, values) in llrs.chunks_exact_mut(N).enumerate() {
            if frame % 2 == 1 {
                for value in values {
                    *value = -*value;
                }
            }
        }
        for _ in 0..2 {
            let mut worker = Aff3ctWorker::new(&prototype, frames, K).expect("clone");
            for _ in 0..3 {
                worker.decode_batch(&llrs);
            }
            let timed = worker.decisions.clone();
            let iterations = worker
                .replay_iterations(&mut prototype, &llrs)
                .expect("replay");
            assert_eq!(worker.decisions, timed);
            assert_eq!(iterations.len(), frames);
            assert!(iterations.iter().all(|count| (1..=CAP).contains(count)));
            worker.decisions[0] ^= 1;
            assert!(worker.replay_iterations(&mut prototype, &llrs).is_err());
        }
    }

    /// Decodes one wave with buffers `llr_delta` and `decision_delta`
    /// elements off the decoder's dimensions.
    fn decode_off_by(
        mut prototype: Handle,
        llr_delta: isize,
        decision_delta: isize,
        counted: bool,
    ) {
        let wave = prototype.wave();
        let llrs = vec![4.0f32; (wave * N).saturating_add_signed(llr_delta)];
        let mut decisions = vec![0u8; (wave * K).saturating_add_signed(decision_delta)];
        if counted {
            prototype.decode_counted(&llrs, &mut decisions, wave);
        } else {
            prototype.decode(&llrs, &mut decisions, wave);
        }
    }

    #[test]
    #[should_panic(expected = "LLRs")]
    fn a_decode_refuses_undersized_llrs() {
        decode_off_by(spa_intra(), -1, 0, false);
    }

    #[test]
    #[should_panic(expected = "LLRs")]
    fn a_decode_refuses_oversized_llrs() {
        decode_off_by(spa_intra(), 1, 0, false);
    }

    #[test]
    #[should_panic(expected = "decision bytes")]
    fn a_decode_refuses_undersized_decisions() {
        decode_off_by(spa_intra(), 0, -1, false);
    }

    #[test]
    #[should_panic(expected = "decision bytes")]
    fn a_decode_refuses_oversized_decisions() {
        decode_off_by(spa_intra(), 0, 1, false);
    }

    #[test]
    #[should_panic(expected = "LLRs")]
    fn a_counted_decode_refuses_undersized_llrs() {
        decode_off_by(spa_intra(), -1, 0, true);
    }

    #[test]
    #[should_panic(expected = "decision bytes")]
    fn a_counted_decode_refuses_undersized_decisions() {
        decode_off_by(spa_intra(), 0, -1, true);
    }

    #[test]
    #[should_panic(expected = "LLRs")]
    fn a_selected_decoder_refuses_undersized_llrs() {
        decode_off_by(selected(), -1, 0, false);
    }

    #[test]
    #[should_panic(expected = "decision bytes")]
    fn a_selected_decoder_refuses_undersized_decisions() {
        decode_off_by(selected(), 0, -1, false);
    }

    /// One LLR and one decision byte per frame divide by the frame count.
    #[test]
    #[should_panic(expected = "LLRs")]
    fn a_decode_refuses_one_element_per_frame() {
        let mut prototype = spa_intra();
        let wave = prototype.wave();
        prototype.decode(&vec![4.0f32; wave], &mut vec![0u8; wave], wave);
    }

    #[test]
    fn dimensions_outside_the_shim_integer_are_refused() {
        let wide = c_int::MAX as usize + 1;
        let build = |k, n| Handle::build_spa_intra(alist(), k, n, CAP, true, SYNDROME_DEPTH);
        assert!(build(K, wide).is_err());
        assert!(build(wide, N).is_err());
        assert!(build(0, N).is_err());
    }

    #[test]
    fn a_handle_reports_the_decoder_of_its_wave() {
        for prototype in [spa_intra(), selected()] {
            let observed = prototype.observed().expect("observed");
            assert_eq!(observed.wave_frames, prototype.wave());
            assert!(prototype.name().starts_with(&observed.class));
            assert!(!observed.backend.is_empty());
        }
        let prototype = spa_intra();
        assert_eq!(
            prototype.replicate().expect("clone").observed(),
            prototype.observed()
        );
    }

    #[test]
    fn a_handle_name_parses_from_its_last_fields() {
        assert_eq!(
            ObservedDecoder::parse("Class backend=A B wave=8"),
            Some(ObservedDecoder {
                class: "Class".to_owned(),
                backend: "A B".to_owned(),
                wave_frames: 8,
            })
        );
        assert_eq!(ObservedDecoder::parse("Class backend=A wave=x"), None);
        assert_eq!(ObservedDecoder::parse("Class wave=8"), None);
    }

    #[test]
    fn an_unreadable_alist_is_refused_with_its_path() {
        let refused = Handle::build_spa_intra(
            std::path::Path::new("absent.alist"),
            K,
            N,
            CAP,
            true,
            SYNDROME_DEPTH,
        );
        assert!(refused.err().expect("refused").contains("absent.alist"));
    }
}
