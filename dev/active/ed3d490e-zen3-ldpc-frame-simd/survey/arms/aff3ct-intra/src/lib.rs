//! AFF3CT float sum-product INTRA arm of the batch decoder comparison.
//!
//! The steady-state operation of the `3be770d5` AFF3CT arm with the decoder
//! `cpp/intra_shim.cpp` adds: a prototype built from the recorded AList and
//! one AFF3CT `clone()` per pinned worker, which converts and decodes the
//! recorded f32 LLRs inside the timed call. The handle is this crate's own
//! because the `3be770d5` handle is built by `a3_new` alone.

use ldpc_survey::arm::{
    self, ArmResult, ArmSettings, DecoderCase, Normalization, NormalizationKind, Precision,
    Quality, QualityAccumulator, Schedule, Stopping, StoppingKind, ARM_RESULT_SCHEMA,
};
use ldpc_throughput::driver::run_cell;
use ldpc_throughput::workload::{check_request, Workload};
use serde::Serialize;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_float, c_int, c_uchar, c_void};
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

/// The arm's name in placement reports and diagnostics.
pub const ARM: &str = "aff3ct-spa-intra";

/// Iterations before syndrome detection starts: AFF3CT's default, which the
/// pinned AFF3CT arms declare.
pub const SYNDROME_DEPTH: i32 = 1;

extern "C" {
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
    fn a3_intra_clone(handle: *mut c_void) -> *mut c_void;
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

/// The AFF3CT tree this executable links, as its build script read it.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct LinkedAff3ct {
    pub revision: &'static str,
    pub static_library_sha256: &'static str,
}

pub const LINKED_AFF3CT: LinkedAff3ct = LinkedAff3ct {
    revision: env!("LDPC_ARMS_AFF3CT_REVISION"),
    static_library_sha256: env!("LDPC_ARMS_AFF3CT_LIBRARY_SHA256"),
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

/// The settings the INTRA decoder decodes with, `wave` frames per decode.
#[must_use]
pub fn settings(iteration_cap: u32, syndrome_stopping: bool, wave: u32) -> ArmSettings {
    ArmSettings {
        precision: Precision::F32,
        schedule: Schedule::Flooding,
        normalization: Normalization {
            kind: NormalizationKind::SumProduct,
            factor: None,
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
    }
}

/// An owned AFF3CT INTRA decoder handle.
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
        // SAFETY: `raw` came from `a3_intra_new` or `a3_intra_clone` and is
        // freed once.
        unsafe { a3_free(self.raw) };
    }
}

impl Handle {
    /// Builds the iteration-counting prototype from the recorded AList.
    ///
    /// # Errors
    ///
    /// Returns AFF3CT's message when it rejects the configuration.
    pub fn build(
        alist: &Path,
        k: usize,
        n: usize,
        iteration_cap: u32,
        syndrome_stopping: bool,
    ) -> Result<Self, String> {
        let alist = CString::new(alist.to_string_lossy().as_ref()).map_err(|e| e.to_string())?;
        let mut error = vec![0 as c_char; 512];
        // SAFETY: both pointers are live NUL-terminated buffers for the call,
        // and the shim reports failure by returning null and filling `error`.
        let raw = unsafe {
            a3_intra_new(
                alist.as_ptr(),
                k as c_int,
                n as c_int,
                iteration_cap as c_int,
                c_int::from(syndrome_stopping),
                SYNDROME_DEPTH,
                error.as_mut_ptr(),
                error.len() as c_int,
            )
        };
        if raw.is_null() {
            // SAFETY: the shim wrote a NUL-terminated message into `error`.
            let message = unsafe { CStr::from_ptr(error.as_ptr()) };
            return Err(format!(
                "aff3ct rejected the sum-product INTRA decoder: {}",
                message.to_string_lossy()
            ));
        }
        Ok(Self { raw })
    }

    /// An independent handle from AFF3CT's `clone()` of this decoder; it
    /// counts no iterations.
    ///
    /// # Errors
    ///
    /// Returns an error when the clone fails.
    pub fn replicate(&self) -> Result<Self, String> {
        let _serialized = CLONE.lock().map_err(|e| e.to_string())?;
        // SAFETY: `raw` is a live shim handle; the clone reads it only.
        let raw = unsafe { a3_intra_clone(self.raw) };
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
    /// Panics when the buffers do not hold whole waves or AFF3CT fails.
    pub fn decode(&mut self, llrs: &[f32], decisions: &mut [u8], frames: usize) {
        self.check_buffers(llrs, decisions, frames);
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

    /// Decodes as [`Self::decode`] and returns the iterations of each wave.
    ///
    /// # Panics
    ///
    /// As [`Self::decode`], and when the handle is a clone, which counts none.
    pub fn decode_counted(
        &mut self,
        llrs: &[f32],
        decisions: &mut [u8],
        frames: usize,
    ) -> Vec<u32> {
        self.check_buffers(llrs, decisions, frames);
        let mut iterations = vec![0 as c_int; frames / self.wave()];
        // SAFETY: both buffers hold `frames` whole frames, checked above, and
        // `iterations` one entry per wave.
        let status = unsafe {
            a3_decode_counted(
                self.raw,
                llrs.as_ptr(),
                decisions.as_mut_ptr(),
                frames as c_int,
                iterations.as_mut_ptr(),
            )
        };
        assert_eq!(status, 0, "AFF3CT decode failed");
        iterations
            .into_iter()
            .map(|count| u32::try_from(count).expect("a counting handle"))
            .collect()
    }

    fn check_buffers(&self, llrs: &[f32], decisions: &[u8], frames: usize) {
        let wave = self.wave();
        assert!(
            wave > 0 && frames > 0 && frames.is_multiple_of(wave),
            "whole native waves"
        );
        assert!(llrs.len().is_multiple_of(frames) && decisions.len().is_multiple_of(frames));
    }
}

/// One worker's AFF3CT clone and decision buffer.
pub struct IntraWorker {
    handle: Handle,
    pub decisions: Vec<u8>,
    frames: usize,
}

impl IntraWorker {
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

    /// Decodes the batch; AFF3CT converts inside the call.
    pub fn decode_batch(&mut self, llrs: &[f32]) {
        self.handle.decode(llrs, &mut self.decisions, self.frames);
        std::hint::black_box(&self.decisions);
    }
}

/// One replay of the first `frames` recorded frames on the counting
/// `prototype`, scored per frame in recorded order.
///
/// Each wave is one call, so a frame's latency and iteration count are those
/// of its wave.
///
/// # Errors
///
/// Returns an error when `frames` is not a positive number of whole waves
/// inside the bundle.
pub fn replay_quality(
    prototype: &mut Handle,
    workload: &Workload,
    frames: usize,
    settings: ArmSettings,
) -> Result<Quality, String> {
    let (n, k) = (workload.manifest.n, workload.manifest.k);
    let wave = prototype.wave();
    if wave == 0 || frames == 0 || !frames.is_multiple_of(wave) || frames > workload.manifest.frames
    {
        return Err(format!(
            "{frames} frames are not whole {wave}-frame waves of the bundle"
        ));
    }
    let memory_bytes = arm::peak_rss_bytes();
    let mut decisions = vec![0u8; wave * k];
    let mut accumulator = QualityAccumulator::default();
    for first in (0..frames).step_by(wave) {
        let start = Instant::now();
        let iterations =
            prototype.decode_counted(workload.frames(first, wave), &mut decisions, wave);
        let latency = start.elapsed().as_nanos() as u64;
        for (offset, decided) in decisions.chunks_exact(k).enumerate() {
            let sent = &workload.codewords[(first + offset) * n..][..k];
            accumulator.observe(decided, sent, iterations[0], latency);
        }
    }
    Ok(accumulator.finish(memory_bytes, settings))
}

/// Untimed diagnostics the arm writes to standard error, which the runner
/// journals as a child diagnostic.
#[derive(Debug, Serialize)]
struct Diagnostics {
    kind: &'static str,
    arm: &'static str,
    decoder: ObservedDecoder,
    linked_aff3ct: LinkedAff3ct,
}

/// Runs one steady-state cell of the INTRA decoder and writes the arm result.
///
/// # Errors
///
/// Returns an error when the request or the bundle is unusable, when
/// `GF2_LDPC_QUALITY` names no readable prepared quality record or one of
/// other settings, or when a worker violates its placement or decodes other
/// than the prepared per-frame error counts.
pub fn run() -> Result<(), String> {
    let request = arm::read_request()?;
    let quality = arm::prepared_quality()?
        .ok_or("steady-state cells reuse prepared quality; GF2_LDPC_QUALITY is unset")?;
    let case: DecoderCase = serde_json::from_value(request.case.clone())
        .map_err(|error| format!("case does not decode: {error}"))?;
    let workload = Workload::load(&case)?;
    check_request(&request, &case, workload.manifest.frames)?;
    let (n, k) = (workload.manifest.n, workload.manifest.k);
    let batch = case.batch_size as usize;
    let llrs = workload.frames(0, batch);

    let setup_start = Instant::now();
    let prototype = Handle::build(
        &workload.alist(),
        k,
        n,
        case.iteration_cap,
        case.syndrome_stopping,
    )?;
    let decoder = prototype.observed()?;
    let settings = settings(
        case.iteration_cap,
        case.syndrome_stopping,
        decoder.wave_frames as u32,
    );
    if quality.settings != settings {
        return Err("the prepared quality was produced under other settings".to_owned());
    }
    let name = prototype.name();
    let cell = run_cell(
        ARM,
        &request,
        &workload,
        &quality,
        setup_start,
        // AFF3CT converts inside every decode call; no separate stage exists.
        0,
        |_| IntraWorker::new(&prototype, batch, k),
        |worker: &mut IntraWorker| worker.decode_batch(llrs),
        |worker: &IntraWorker| &worker.decisions,
    )?;
    let diagnostics = Diagnostics {
        kind: "aff3ct-decoder-diagnostics",
        arm: ARM,
        decoder,
        linked_aff3ct: LINKED_AFF3CT,
    };
    eprintln!(
        "{}",
        serde_json::to_string(&diagnostics).map_err(|e| e.to_string())?
    );
    let result = ArmResult {
        schema: ARM_RESULT_SCHEMA.to_owned(),
        windows: cell.windows,
        calibrated: true,
        cache_state_applied: request.cache_state.clone(),
        workers_observed: cell.workers_observed,
        cpus_observed: arm::observed_cpus(),
        selected_path: Some(format!(
            "{name} simd=INTRA precision=f32, clone per worker; {} pinned workers, {} process \
             threads",
            cell.workers_observed, cell.threads.ready
        )),
        conversion: Some(cell.conversion),
        quality: Some(quality),
    };
    arm::write_result(&result)
}

#[cfg(test)]
mod tests {
    use super::{settings, Handle, IntraWorker, ObservedDecoder};
    use ldpc_survey::arm::NormalizationKind;

    const N: usize = 7;
    const K: usize = 4;
    const CAP: u32 = 20;

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
        let mut prototype = Handle::build(alist(), K, N, CAP, true).expect("decoder");
        let wave = prototype.wave();
        let frames = 3 * wave;
        let llrs = llrs(frames);
        let mut counted = vec![1u8; frames * K];
        let iterations = prototype.decode_counted(&llrs, &mut counted, frames);
        assert_eq!(counted, vec![0u8; frames * K]);
        assert_eq!(iterations.len(), frames / wave);
        assert!(iterations.iter().all(|count| (1..CAP).contains(count)));

        let mut worker = IntraWorker::new(&prototype, frames, K).expect("clone");
        worker.decode_batch(&llrs);
        assert_eq!(worker.decisions, counted);
    }

    #[test]
    fn a_handle_reports_the_decoder_of_its_wave() {
        let prototype = Handle::build(alist(), K, N, CAP, true).expect("decoder");
        let observed = prototype.observed().expect("observed");
        assert_eq!(observed.wave_frames, prototype.wave());
        assert!(prototype.name().starts_with(&observed.class));
        assert!(!observed.backend.is_empty());
        assert_eq!(
            prototype.replicate().expect("clone").observed(),
            Ok(observed)
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
        let refused = Handle::build(std::path::Path::new("absent.alist"), K, N, CAP, true);
        assert!(refused.err().expect("refused").contains("absent.alist"));
    }

    #[test]
    fn the_settings_declare_sum_product_without_a_factor() {
        let declared = settings(CAP, false, 1);
        assert_eq!(declared.normalization.kind, NormalizationKind::SumProduct);
        assert_eq!(declared.normalization.factor, None);
        assert_eq!(declared.batch_size, 1);
    }
}
