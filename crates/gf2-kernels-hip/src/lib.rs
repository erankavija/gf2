//! HIP/ROCm GPU kernels for the gf2 workspace.
//!
//! This crate provides GPU-accelerated batch BCJR decoding via AMD HIP.
//! All unsafe HIP/FFI code is isolated here, following the same pattern
//! as `gf2-kernels-simd`.
//!
//! # Requirements
//!
//! - ROCm with hipcc (tested with ROCm 7.2)
//! - AMD GPU with gfx1030 ISA (RX 6000 series)
//!
//! # Examples
//!
//! ```no_run
//! use gf2_kernels_hip::GpuBcjrBatch;
//!
//! // h_cols: parity-check matrix columns as u32 bitmasks
//! let h_cols = vec![0b101u32; 7];  // Hamming(7,4) example
//! let gpu = GpuBcjrBatch::new(&h_cols, 7, 4, 64).unwrap();
//!
//! let inputs = vec![vec![3.0f32; 7]; 4];  // 4 SISO decodes
//! let (app, ext) = gpu.decode_batch(&inputs).unwrap();
//! assert_eq!(app.len(), 4);
//! ```

pub(crate) mod ffi;

/// Host-side HIP infrastructure: stream pool, allocator wrappers,
/// deterministic-launch helpers, and multi-arch dispatch (design doc §6).
///
/// All types here are `unsafe`-free at the call site — every `unsafe` FFI
/// call lives behind a safe RAII wrapper or a `// SAFETY:`-annotated block.
pub mod host;

/// Safe host wrappers for the device ChaCha20 + Box-Muller AWGN kernel
/// (`hip/chacha20_awgn.hip`, design doc §3 / §11): [`GpuChaChaAwgn`] and the
/// seed → key derivation [`chacha20_key_from_seed`].
pub mod launch_chacha20_awgn;

#[doc(inline)]
pub use launch_chacha20_awgn::{chacha20_key_from_seed, GpuChaChaAwgn};

/// Safe host wrappers for the device LDPC belief-propagation batch decoder
/// (`hip/ldpc_bp.hip`, design doc §6 / §10 / §11): [`GpuLdpcBp`], the
/// [`LdpcGraphLayout`] CSR/CSC graph encoding, and the [`GpuBpAlgorithm`]
/// box-plus selector.
pub mod launch_ldpc_bp;

#[doc(inline)]
pub use launch_ldpc_bp::{GpuBpAlgorithm, GpuLdpcBp, LdpcGraphLayout, LdpcStreamScratch};

/// Per-prime permanent computation kernels (placeholder scaffold).
///
/// Populated by downstream issues ad55b777, b43cdf33, and 5c0505b2.
/// Only compiled when the `hip` Cargo feature is enabled.
#[cfg(feature = "hip")]
pub mod permanent;

use std::ffi::c_void;
use std::ptr;

/// Error type for HIP operations.
///
/// The common case is [`HipError::Hip`], carrying the raw `hipError_t` code
/// and the name of the API call that failed. [`HipError::OutOfMemory`] is a
/// distinguished variant the pipeline executor catches to substitute a CPU
/// fallback (design doc §8); the `gf2-sim` boundary
/// (`crates/gf2-sim/src/gpu/mod.rs`) maps it to
/// `gf2_sim::RecoverableError::OutOfMemory`. `gf2-kernels-hip` deliberately
/// does **not** depend on `gf2-sim` — the mapping lives on the `gf2-sim`
/// side to avoid a dependency inversion.
#[derive(Debug, Clone)]
pub enum HipError {
    /// A HIP API call failed with a non-zero `hipError_t` code.
    Hip {
        /// HIP error code (0 = hipSuccess; never stored here).
        code: i32,
        /// Name of the HIP API call that failed.
        context: &'static str,
    },
    /// A device allocation failed because the device is out of memory.
    ///
    /// Distinguished from [`HipError::Hip`] so the pipeline executor can
    /// catch it and substitute a CPU fallback rather than aborting. Mapped
    /// to `gf2_sim::RecoverableError::OutOfMemory` at the `gf2-sim`
    /// boundary.
    OutOfMemory {
        /// The HIP device that ran out of memory.
        device_id: i32,
        /// The allocation size, in bytes, that failed.
        bytes_requested: usize,
    },
    /// No HIP device is visible to the runtime (`hipGetDeviceCount() == 0`).
    ///
    /// Distinguished from [`HipError::Hip`] so the `gf2-sim` boundary can map
    /// it to `FatalError::DeviceUnavailable` (design doc §8): the run aborts at
    /// pipeline construction with a clear "no GPU" diagnostic and the user
    /// re-runs with `--cpu-only`.
    NoDevice,
    /// The detected device runs a gfx arch this build does not have a kernel
    /// blob for.
    ///
    /// Distinguished from [`HipError::Hip`] so the dispatcher can emit a
    /// `tracing::warn!` and fall back to the CPU-equivalent stage rather than
    /// hard-failing (design doc §6). The `gf2-sim` boundary maps it to a
    /// [recoverable](../gf2_sim/error/enum.RecoverableError.html) error so the
    /// executor substitutes the CPU fallback on the affected batches.
    UnsupportedArch {
        /// The device's GCN arch name as reported by `gcnArchName` (e.g.
        /// `"gfx908"`), with any feature suffix stripped.
        gcn_arch_name: String,
    },
    /// A precompiled kernel blob (`*.co`) could not be read from disk.
    ///
    /// This is a host-side **file-I/O** failure, not a `hipError_t`, so it is a
    /// dedicated variant rather than a [`HipError::Hip`] with a fabricated code
    /// (code `0` everywhere else means `hipSuccess`). A missing blob for the
    /// *active* arch is a build/configuration fault — the `gf2-sim` boundary
    /// maps it to a fatal `KernelLaunch` (design doc §8), not OOM/CPU-fallback.
    BlobLoad {
        /// The blob path that failed to load.
        path: std::path::PathBuf,
        /// The underlying `std::io::Error` rendered as a string (kept as a
        /// `String` so `HipError` stays `Clone`).
        source: String,
    },
}

impl HipError {
    /// Returns the underlying `hipError_t` code for a [`HipError::Hip`], or the
    /// canonical sentinel for each typed variant: `hipErrorOutOfMemory` (2),
    /// `hipErrorNoDevice` (100), `hipErrorInvalidDevice` (101), or
    /// `hipErrorFileNotFound` (301) for a [`HipError::BlobLoad`]. Never returns
    /// `0` (which means `hipSuccess`).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_kernels_hip::HipError;
    ///
    /// assert_eq!(HipError::NoDevice.code(), 100);
    /// assert_eq!(
    ///     HipError::OutOfMemory { device_id: 0, bytes_requested: 1 << 40 }.code(),
    ///     2
    /// );
    /// // A typed variant never masquerades as hipSuccess (0).
    /// assert_ne!(HipError::NoDevice.code(), 0);
    /// ```
    pub fn code(&self) -> i32 {
        match self {
            HipError::Hip { code, .. } => *code,
            // hipErrorOutOfMemory is canonically 2.
            HipError::OutOfMemory { .. } => 2,
            // hipErrorNoDevice is canonically 100.
            HipError::NoDevice => 100,
            // hipErrorInvalidDevice is canonically 101; we reuse it for an
            // arch this build has no blob for (a device-capability mismatch).
            HipError::UnsupportedArch { .. } => 101,
            // hipErrorFileNotFound is canonically 301; a blob-load failure is a
            // host-side file-I/O fault, so we surface that sentinel rather than
            // a fabricated success code.
            HipError::BlobLoad { .. } => 301,
        }
    }
}

impl std::fmt::Display for HipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HipError::Hip { code, context } => {
                write!(f, "HIP error {code} in {context}")
            }
            HipError::OutOfMemory {
                device_id,
                bytes_requested,
            } => write!(
                f,
                "HIP out of memory on device {device_id}: {bytes_requested} bytes requested"
            ),
            HipError::NoDevice => write!(f, "no HIP device visible to the runtime"),
            HipError::UnsupportedArch { gcn_arch_name } => write!(
                f,
                "unsupported gfx arch '{gcn_arch_name}': no kernel blob for this build"
            ),
            HipError::BlobLoad { path, source } => write!(
                f,
                "failed to load kernel blob '{}': {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for HipError {}

/// `hipError_t` code for `hipErrorOutOfMemory` (== `hipErrorMemoryAllocation`).
pub(crate) const HIP_ERROR_OUT_OF_MEMORY: i32 = 2;

/// `hipError_t` code for `hipErrorNotReady` (async work still pending).
pub(crate) const HIP_ERROR_NOT_READY: i32 = 600;

/// Check a HIP return code, returning Err on failure.
pub(crate) fn check_hip(code: i32, context: &'static str) -> Result<(), HipError> {
    if code == 0 {
        Ok(())
    } else {
        Err(HipError::Hip { code, context })
    }
}

/// Maximum trellis states supported by the GPU kernel's shared memory arrays.
/// The kernel statically allocates `__shared__ float alpha[MAX_STATES]`.
pub const MAX_GPU_STATES: usize = 2048;

/// Extracts parity-check matrix columns as u32 bitmasks for GPU use.
///
/// Delegates to [`gf2_core::BitMatrix::cols_as_u32_masks`] — the canonical
/// implementation. Each returned u32 encodes the j-th column of H: bit i
/// is set iff `H[i][j] == 1`.
///
/// # Arguments
///
/// * `h` - Parity-check matrix (m rows, n columns).
///
/// # Returns
///
/// A `Vec<u32>` of length n.
///
/// # Panics
///
/// Panics if the matrix has more than 32 rows (columns won't fit in a u32).
///
/// # Examples
///
/// ```no_run
/// use gf2_kernels_hip::extract_h_cols;
///
/// let h = gf2_core::bitmatrix![
///     1, 1, 0, 1, 1, 0, 0;
///     1, 0, 1, 1, 0, 1, 0;
///     0, 1, 1, 1, 0, 0, 1
/// ];
/// let cols = extract_h_cols(&h);
/// assert_eq!(cols.len(), 7);
/// ```
pub fn extract_h_cols(h: &gf2_core::BitMatrix) -> Vec<u32> {
    h.cols_as_u32_masks()
}

/// Byte-oriented RAII device allocation used by the in-crate decoder/demapper
/// kernels (`GpuBcjrBatch`, `GpuGrayQamDemapper`, `permanent`).
///
/// This is a thin, byte-sized adapter over the canonical generic
/// [`host::DeviceBuffer<u8>`](crate::host::DeviceBuffer) — the single
/// hipMalloc/hipFree RAII primitive in this crate (SSOT). It exists only to
/// preserve the byte-slice `copy_from_host(&[u8])` / `copy_to_host(&mut [u8])`
/// interface the existing kernels reinterpret typed payloads through; all the
/// allocation, free, and memcpy logic lives in `host::DeviceBuffer`.
///
/// Accessible within the crate (`pub(crate)`) so the `permanent` submodule
/// can reuse it without re-duplicating the alloc/free boilerplate.
pub(crate) struct DecoderDeviceBuffer {
    inner: host::DeviceBuffer<u8>,
}

impl DecoderDeviceBuffer {
    pub(crate) fn new(size: usize) -> Result<Self, HipError> {
        // Device 0: the in-crate decoder/demapper kernels are single-device.
        let inner = host::DeviceBuffer::<u8>::new(size, 0)?;
        Ok(Self { inner })
    }

    pub(crate) fn as_ptr(&self) -> *const c_void {
        self.inner.as_ptr()
    }

    pub(crate) fn as_mut_ptr(&self) -> *mut c_void {
        self.inner.as_mut_ptr()
    }

    pub(crate) fn copy_from_host(&self, src: &[u8]) -> Result<(), HipError> {
        self.inner.copy_from_host(src)
    }

    pub(crate) fn copy_to_host(&self, dst: &mut [u8]) -> Result<(), HipError> {
        self.inner.copy_to_host(dst)
    }
}

/// GPU-accelerated batch BCJR decoder.
///
/// Holds persistent device allocations for the trellis columns and workspace
/// buffers. Reusable across multiple `decode_batch` calls.
pub struct GpuBcjrBatch {
    d_h_cols: DecoderDeviceBuffer,
    d_llrs: DecoderDeviceBuffer,
    d_app: DecoderDeviceBuffer,
    d_alpha_ws: DecoderDeviceBuffer,
    n: usize,
    k: usize,
    num_states: usize,
    max_batch: usize,
}

impl GpuBcjrBatch {
    /// Creates a new GPU BCJR batch decoder.
    ///
    /// Pre-allocates device memory for up to `max_batch` simultaneous BCJR
    /// decodes. The trellis columns are uploaded once and reused across calls.
    ///
    /// # Arguments
    ///
    /// * `h_cols` - Parity-check matrix columns as u32 bitmasks (length n).
    ///   Use [`extract_h_cols`] to obtain these from a `BitMatrix`.
    /// * `n` - Codeword length.
    /// * `k` - Message length.
    /// * `max_batch` - Maximum batch size (pre-allocates device memory).
    ///
    /// # Errors
    ///
    /// Returns `HipError` if device memory allocation fails.
    ///
    /// # Panics
    ///
    /// Panics if `h_cols.len() != n` or if `2^(n-k) > MAX_GPU_STATES` (2048).
    ///
    /// # Complexity
    ///
    /// O(max_batch * n * 2^(n-k)) device memory allocated.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::GpuBcjrBatch;
    ///
    /// // Hamming(7,4): 3 parity rows → 2^3 = 8 states
    /// let h_cols = vec![0b011, 0b101, 0b110, 0b111, 0b001, 0b010, 0b100];
    /// let gpu = GpuBcjrBatch::new(&h_cols, 7, 4, 64).unwrap();
    /// assert_eq!(gpu.n(), 7);
    /// ```
    pub fn new(h_cols: &[u32], n: usize, k: usize, max_batch: usize) -> Result<Self, HipError> {
        assert_eq!(h_cols.len(), n);
        let num_states = 1usize << (n - k);
        assert!(
            num_states <= MAX_GPU_STATES,
            "2^(n-k) = {} exceeds GPU kernel limit of {} states",
            num_states,
            MAX_GPU_STATES
        );

        // Allocate device buffers
        let d_h_cols = DecoderDeviceBuffer::new(n * std::mem::size_of::<u32>())?;
        let d_llrs = DecoderDeviceBuffer::new(max_batch * n * std::mem::size_of::<f32>())?;
        let d_app = DecoderDeviceBuffer::new(max_batch * n * std::mem::size_of::<f32>())?;
        let d_alpha_ws = DecoderDeviceBuffer::new(
            max_batch * (n + 1) * num_states * std::mem::size_of::<f32>(),
        )?;

        // Upload h_cols (persistent — same trellis for all decodes)
        d_h_cols.copy_from_host(u32_slice_as_bytes(h_cols))?;

        Ok(Self {
            d_h_cols,
            d_llrs,
            d_app,
            d_alpha_ws,
            n,
            k,
            num_states,
            max_batch,
        })
    }

    /// Returns the codeword length.
    pub fn n(&self) -> usize {
        self.n
    }

    /// Returns the message length.
    pub fn k(&self) -> usize {
        self.k
    }

    /// Returns the maximum batch size.
    pub fn max_batch(&self) -> usize {
        self.max_batch
    }

    /// Decodes a batch of SISO inputs on the GPU.
    ///
    /// # Arguments
    ///
    /// * `inputs` - Slice of combined LLR vectors, each of length n.
    ///
    /// # Returns
    ///
    /// `(app_llrs, extrinsic_llrs)` where each is `Vec<Vec<f32>>` of length
    /// `inputs.len()`, with inner vectors of length n.
    ///
    /// # Errors
    ///
    /// Returns `HipError` on device communication failure.
    ///
    /// # Panics
    ///
    /// Panics if `inputs.len() > max_batch` or any input length != n.
    ///
    /// # Complexity
    ///
    /// O(batch_size * n * num_states) GPU work. Host-side cost is dominated
    /// by the H→D and D→H memcpy of `batch_size * n` floats.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::GpuBcjrBatch;
    ///
    /// let h_cols = vec![0b011, 0b101, 0b110, 0b111, 0b001, 0b010, 0b100];
    /// let gpu = GpuBcjrBatch::new(&h_cols, 7, 4, 8).unwrap();
    /// let inputs = vec![vec![3.0f32; 7]; 4]; // batch of 4
    /// let (app, ext) = gpu.decode_batch(&inputs).unwrap();
    /// assert_eq!(app.len(), 4);
    /// ```
    #[allow(clippy::type_complexity)]
    pub fn decode_batch(
        &self,
        inputs: &[Vec<f32>],
    ) -> Result<(Vec<Vec<f32>>, Vec<Vec<f32>>), HipError> {
        let batch_size = inputs.len();
        assert!(
            batch_size <= self.max_batch,
            "batch size {} exceeds max {}",
            batch_size,
            self.max_batch
        );

        if batch_size == 0 {
            return Ok((vec![], vec![]));
        }

        let n = self.n;
        for (i, inp) in inputs.iter().enumerate() {
            assert_eq!(
                inp.len(),
                n,
                "input {} has length {}, expected {}",
                i,
                inp.len(),
                n
            );
        }

        // Flatten inputs and upload
        let mut flat_llrs: Vec<f32> = Vec::with_capacity(batch_size * n);
        for inp in inputs {
            flat_llrs.extend_from_slice(inp);
        }
        self.d_llrs.copy_from_host(f32_slice_as_bytes(&flat_llrs))?;

        // Launch kernel
        // SAFETY: All device pointers were allocated in `new()` with sufficient
        // size for `max_batch` decodes. `batch_size <= max_batch` is asserted above.
        // The kernel reads from d_llrs/d_h_cols and writes to d_app/d_alpha_ws.
        check_hip(
            unsafe {
                ffi::launch_bcjr_batch(
                    self.d_llrs.as_ptr() as *const f32,
                    self.d_h_cols.as_ptr() as *const u32,
                    self.d_app.as_mut_ptr() as *mut f32,
                    self.d_alpha_ws.as_mut_ptr() as *mut f32,
                    batch_size as i32,
                    n as i32,
                    self.num_states as i32,
                    ptr::null_mut(), // default stream
                )
            },
            "launch_bcjr_batch",
        )?;

        // SAFETY: hipDeviceSynchronize has no preconditions; it blocks until
        // all preceding HIP operations on the default stream complete.
        check_hip(
            unsafe { ffi::hip_device_synchronize() },
            "hipDeviceSynchronize",
        )?;

        // Download APP results
        let mut flat_app = vec![0.0f32; batch_size * n];
        self.d_app
            .copy_to_host(f32_slice_as_bytes_mut(&mut flat_app))?;

        // Split into per-decode vectors and compute extrinsic
        let mut app_out = Vec::with_capacity(batch_size);
        let mut ext_out = Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            let app_slice = &flat_app[i * n..(i + 1) * n];
            let inp_slice = &inputs[i];
            let app_vec: Vec<f32> = app_slice.to_vec();
            let ext_vec: Vec<f32> = app_slice
                .iter()
                .zip(inp_slice.iter())
                .map(|(&a, &l)| a - l)
                .collect();
            app_out.push(app_vec);
            ext_out.push(ext_vec);
        }

        Ok((app_out, ext_out))
    }
}

/// Reinterprets a `&[u32]` as a byte slice for H→D copies.
///
/// Same rationale as [`f32_slice_as_bytes`]: `u32` has no padding bits and u8
/// alignment is 1. Centralizes the trellis-index upload so no call site
/// hand-rolls the `from_raw_parts` reinterpretation.
#[inline]
fn u32_slice_as_bytes(src: &[u32]) -> &[u8] {
    // SAFETY: u32 has no padding and u8 alignment is 1; the byte count is
    // computed from `src` so the slice spans the same memory region.
    unsafe { std::slice::from_raw_parts(src.as_ptr() as *const u8, std::mem::size_of_val(src)) }
}

/// Reinterprets a `&[f32]` as a byte slice for H→D copies.
///
/// Safe because `f32` has no padding bits and u8 alignment is 1. The
/// returned slice aliases `src`'s backing storage and has the same
/// lifetime; callers must not mutate `src` while the byte slice is
/// live.
#[inline]
fn f32_slice_as_bytes(src: &[f32]) -> &[u8] {
    // SAFETY: f32 has no padding and u8 alignment is 1; the total byte
    // count is computed from `src` so the resulting slice spans the
    // same memory region as the input.
    unsafe { std::slice::from_raw_parts(src.as_ptr() as *const u8, std::mem::size_of_val(src)) }
}

/// Reinterprets a `&mut [f32]` as a mutable byte slice for D→H copies.
///
/// Same rationale as [`f32_slice_as_bytes`]; the mutable variant is
/// needed because `hipMemcpy` writes into the host buffer.
#[inline]
fn f32_slice_as_bytes_mut(dst: &mut [f32]) -> &mut [u8] {
    let len = std::mem::size_of_val(dst);
    // SAFETY: see f32_slice_as_bytes; mutability is preserved.
    unsafe { std::slice::from_raw_parts_mut(dst.as_mut_ptr() as *mut u8, len) }
}

/// GPU-accelerated batch Gray square-QAM / BPSK soft demapper (max-log).
///
/// Computes per-bit LLRs on the GPU using the same axis-separable
/// pre-rotation contract as the CPU fast path
/// (`gf2_coding::modem::FastGrayQamDemapper`): under AWGN with
/// independent I/Q noise the per-symbol 2D max-log decomposes into two
/// 1D Gray-PAM max-log LLRs of size `sqrt(M)` each, so the hot path
/// cost is `O(num_symbols * sqrt(M) * m)`.
///
/// The kernel implements the max-log variant only; this is a research
/// prototype intended to back the CPU/GPU crossover measurement tracked
/// in JIT issue `9c37ec8c`. For exact log-MAP or arbitrary constellations,
/// use the CPU reference/fast paths.
///
/// Persistent state: the device-side `pam_levels` table and reusable
/// input / output buffers are allocated once at construction for up to
/// `max_batch` symbols. Subsequent `demap_batch` calls reuse the same
/// allocations.
///
/// # Examples
///
/// ```no_run
/// use gf2_kernels_hip::GpuGrayQamDemapper;
///
/// // 16-QAM: m = 4, axis_len = 4, pam_levels derived by the caller
/// // (on the CPU side, matching the preset).
/// let pam_levels: Vec<f32> = vec![-3.0, -1.0, 1.0, 3.0]
///     .into_iter()
///     .map(|v| v / (10.0f32).sqrt())
///     .collect();
/// let demapper = GpuGrayQamDemapper::new(&pam_levels, 4, false, 256).unwrap();
/// let rx_i = vec![0.3f32; 4];
/// let rx_q = vec![-0.2f32; 4];
/// let nv = vec![0.25f32; 4];
/// let llrs = demapper.demap_batch(&rx_i, &rx_q, None, None, &nv).unwrap();
/// assert_eq!(llrs.len(), 4 * 4);
/// ```
pub struct GpuGrayQamDemapper {
    d_rx_i: DecoderDeviceBuffer,
    d_rx_q: DecoderDeviceBuffer,
    d_gain_i: DecoderDeviceBuffer,
    d_gain_q: DecoderDeviceBuffer,
    d_noise_var: DecoderDeviceBuffer,
    d_pam_levels: DecoderDeviceBuffer,
    d_out_llrs: DecoderDeviceBuffer,
    axis_len: usize,
    m: u8,
    m_half: u8,
    is_bpsk: bool,
    max_batch: usize,
}

impl GpuGrayQamDemapper {
    /// Constructs a new GPU demapper for a fixed Gray square-QAM / BPSK
    /// preset, pre-uploading the PAM level table.
    ///
    /// # Arguments
    ///
    /// * `pam_levels` - Post-normalization Gray-PAM levels shared between
    ///   the I and Q axes. Length `1 << (m / 2)` for QAM or exactly `2`
    ///   for BPSK. These must come from the caller's matching CPU
    ///   [`gf2_coding::modem::FastGrayQamDemapper`] construction — the
    ///   GPU path never re-derives them.
    /// * `m` - Bits per symbol. Must be one of `1, 2, 4, 6, 8`.
    /// * `is_bpsk` - `true` for BPSK (`m == 1`), `false` for Gray-QAM.
    /// * `max_batch` - Maximum number of symbols per `demap_batch` call.
    ///   Device buffers for all inputs / outputs are allocated up to this
    ///   size and reused across calls.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] if device allocation or the one-time
    /// `pam_levels` upload fails.
    ///
    /// # Panics
    ///
    /// Panics if `m` is not in `{1, 2, 4, 6, 8}`, if `pam_levels` has the
    /// wrong length for `m`, or if the BPSK flag disagrees with `m == 1`.
    ///
    /// # Complexity
    ///
    /// O(`max_batch`) device memory for inputs / outputs plus
    /// `O(axis_len)` for the PAM table.
    pub fn new(
        pam_levels: &[f32],
        m: u8,
        is_bpsk: bool,
        max_batch: usize,
    ) -> Result<Self, HipError> {
        assert!(
            matches!(m, 1 | 2 | 4 | 6 | 8),
            "GpuGrayQamDemapper::new: m = {m} must be one of {{1, 2, 4, 6, 8}}"
        );
        assert_eq!(
            is_bpsk,
            m == 1,
            "GpuGrayQamDemapper::new: is_bpsk={is_bpsk} inconsistent with m={m}"
        );
        let (m_half, axis_len) = if is_bpsk {
            (0u8, 2usize)
        } else {
            (m / 2, 1usize << (m / 2))
        };
        assert_eq!(
            pam_levels.len(),
            axis_len,
            "GpuGrayQamDemapper::new: pam_levels.len() = {} != expected axis_len = {axis_len}",
            pam_levels.len()
        );

        let f32_size = std::mem::size_of::<f32>();
        let d_rx_i = DecoderDeviceBuffer::new(max_batch * f32_size)?;
        let d_rx_q = DecoderDeviceBuffer::new(max_batch * f32_size)?;
        let d_gain_i = DecoderDeviceBuffer::new(max_batch * f32_size)?;
        let d_gain_q = DecoderDeviceBuffer::new(max_batch * f32_size)?;
        let d_noise_var = DecoderDeviceBuffer::new(max_batch * f32_size)?;
        let d_pam_levels = DecoderDeviceBuffer::new(axis_len * f32_size)?;
        let d_out_llrs = DecoderDeviceBuffer::new(max_batch * (m as usize) * f32_size)?;

        // Upload PAM levels once (persistent for the lifetime of self).
        // The matching assertion on `pam_levels.len()` above guarantees
        // the destination device allocation is sized correctly.
        assert_eq!(pam_levels.len(), axis_len);
        d_pam_levels.copy_from_host(f32_slice_as_bytes(pam_levels))?;

        Ok(Self {
            d_rx_i,
            d_rx_q,
            d_gain_i,
            d_gain_q,
            d_noise_var,
            d_pam_levels,
            d_out_llrs,
            axis_len,
            m,
            m_half,
            is_bpsk,
            max_batch,
        })
    }

    /// Returns the bits-per-symbol `m` this demapper was constructed for.
    pub fn m(&self) -> u8 {
        self.m
    }

    /// Returns the maximum batch size.
    pub fn max_batch(&self) -> usize {
        self.max_batch
    }

    /// Demaps a batch of received symbols into max-log LLRs on the GPU.
    ///
    /// Returns a flat `Vec<f32>` of length `num_symbols * m` in the
    /// canonical symbol-major, MSB-first layout: for QAM the first
    /// `m/2` bits of each symbol are the I-axis Gray-PAM label (MSB =
    /// coarsest level), followed by `m/2` Q-axis bits.
    ///
    /// # Arguments
    ///
    /// * `rx_i` / `rx_q` - Received samples split into I and Q. Lengths
    ///   must match and define `num_symbols`.
    /// * `gain_i` / `gain_q` - Optional per-symbol complex channel gain.
    ///   Pass `None` on both for AWGN. When provided, both must be
    ///   `Some(_)` and have length `num_symbols`.
    /// * `noise_var` - Per-symbol `N0 = 2 sigma^2`, length `num_symbols`.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] on device memcpy, kernel launch, or
    /// synchronization failures.
    ///
    /// # Panics
    ///
    /// Panics if `num_symbols > max_batch`, if `rx_i` / `rx_q` /
    /// `noise_var` have mismatched lengths, or if exactly one of
    /// `gain_i` / `gain_q` is `Some`.
    ///
    /// # Complexity
    ///
    /// O(`num_symbols * axis_len * m`) GPU work. Host-side cost is
    /// dominated by the five H→D copies and one D→H copy of
    /// `num_symbols` f32s each.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::GpuGrayQamDemapper;
    ///
    /// let pam_levels: Vec<f32> = vec![-3.0, -1.0, 1.0, 3.0]
    ///     .into_iter()
    ///     .map(|v| v / (10.0f32).sqrt())
    ///     .collect();
    /// let demapper = GpuGrayQamDemapper::new(&pam_levels, 4, false, 64).unwrap();
    /// let rx_i = vec![0.3f32; 8];
    /// let rx_q = vec![-0.5f32; 8];
    /// let nv = vec![0.1f32; 8];
    /// let llrs = demapper
    ///     .demap_batch(&rx_i, &rx_q, None, None, &nv)
    ///     .unwrap();
    /// assert_eq!(llrs.len(), 8 * 4);
    /// ```
    pub fn demap_batch(
        &self,
        rx_i: &[f32],
        rx_q: &[f32],
        gain_i: Option<&[f32]>,
        gain_q: Option<&[f32]>,
        noise_var: &[f32],
    ) -> Result<Vec<f32>, HipError> {
        let num_symbols = rx_i.len();
        assert_eq!(
            rx_q.len(),
            num_symbols,
            "GpuGrayQamDemapper::demap_batch: rx_i.len() ({}) != rx_q.len() ({})",
            num_symbols,
            rx_q.len()
        );
        assert_eq!(
            noise_var.len(),
            num_symbols,
            "GpuGrayQamDemapper::demap_batch: rx_i.len() ({}) != noise_var.len() ({})",
            num_symbols,
            noise_var.len()
        );
        assert!(
            num_symbols <= self.max_batch,
            "GpuGrayQamDemapper::demap_batch: num_symbols {num_symbols} > max_batch {}",
            self.max_batch
        );
        let gains_present = match (gain_i, gain_q) {
            (Some(gi), Some(gq)) => {
                assert_eq!(
                    gi.len(),
                    num_symbols,
                    "GpuGrayQamDemapper::demap_batch: gain_i.len() ({}) != num_symbols ({})",
                    gi.len(),
                    num_symbols
                );
                assert_eq!(
                    gq.len(),
                    num_symbols,
                    "GpuGrayQamDemapper::demap_batch: gain_q.len() ({}) != num_symbols ({})",
                    gq.len(),
                    num_symbols
                );
                true
            }
            (None, None) => false,
            _ => panic!(
                "GpuGrayQamDemapper::demap_batch: gain_i and gain_q must both be Some or both be None"
            ),
        };

        if num_symbols == 0 {
            return Ok(Vec::new());
        }

        // H→D copies. `f32_slice_as_bytes` asserts are satisfied by the
        // length checks above (`rx_i.len() == num_symbols`, etc.).
        self.d_rx_i.copy_from_host(f32_slice_as_bytes(rx_i))?;
        self.d_rx_q.copy_from_host(f32_slice_as_bytes(rx_q))?;
        self.d_noise_var
            .copy_from_host(f32_slice_as_bytes(noise_var))?;

        if gains_present {
            // Safe to unwrap: `gains_present == true` implies both
            // gain_i and gain_q are Some, proven by the match arm above.
            let gi = gain_i.expect("gains_present invariant");
            let gq = gain_q.expect("gains_present invariant");
            self.d_gain_i.copy_from_host(f32_slice_as_bytes(gi))?;
            self.d_gain_q.copy_from_host(f32_slice_as_bytes(gq))?;
        }

        // Launch kernel.
        // SAFETY: all device pointers originate from `DeviceBuffer::new`
        // in this constructor and are sized for `max_batch` symbols;
        // `num_symbols <= max_batch` is asserted above. When
        // `gains_present == 0` the kernel does not dereference gain
        // pointers (see hip/gray_qam_demapper.hip), so passing their
        // current device addresses (unchanged since construction) is
        // safe either way.
        // Gain buffer pointers: when `gains_present == 0` the kernel
        // promises not to dereference these, but we still pass literal
        // nulls to make the FFI contract explicit — no "trusted
        // non-null" implicit coupling between the Rust wrapper and the
        // device kernel. This also keeps the unsafe block narrow: the
        // non-null path dereferences device-owned allocations that
        // outlive the launch; the null path has no reachable load at
        // all.
        let (gain_i_ptr, gain_q_ptr) = if gains_present {
            (
                self.d_gain_i.as_ptr() as *const f32,
                self.d_gain_q.as_ptr() as *const f32,
            )
        } else {
            (ptr::null::<f32>(), ptr::null::<f32>())
        };
        check_hip(
            unsafe {
                ffi::launch_gray_qam_demap(
                    self.d_rx_i.as_ptr() as *const f32,
                    self.d_rx_q.as_ptr() as *const f32,
                    gain_i_ptr,
                    gain_q_ptr,
                    self.d_noise_var.as_ptr() as *const f32,
                    self.d_pam_levels.as_ptr() as *const f32,
                    self.d_out_llrs.as_mut_ptr() as *mut f32,
                    num_symbols as i32,
                    self.axis_len as i32,
                    self.m as i32,
                    self.m_half as i32,
                    if self.is_bpsk { 1 } else { 0 },
                    if gains_present { 1 } else { 0 },
                    ptr::null_mut(),
                )
            },
            "launch_gray_qam_demap",
        )?;
        // SAFETY: hipDeviceSynchronize blocks until all preceding default-stream
        // work completes; no preconditions.
        check_hip(
            unsafe { ffi::hip_device_synchronize() },
            "hipDeviceSynchronize",
        )?;

        // D→H copy.
        let out_len = num_symbols * self.m as usize;
        let mut out = vec![0.0f32; out_len];
        self.d_out_llrs
            .copy_to_host(f32_slice_as_bytes_mut(&mut out))?;
        Ok(out)
    }
}

// `GpuGrayQamDemapper` is `Send` by auto-derive: every field is either a
// `DecoderDeviceBuffer` (which wraps the `Send` `host::DeviceBuffer<u8>`) or a
// plain `Copy` scalar. No explicit `unsafe impl Send` is needed (an explicit
// impl would only duplicate the auto-derived bound and add unsafe surface).
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<GpuGrayQamDemapper>();
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build Hamming(7,4) h_cols.
    fn hamming74_h_cols() -> Vec<u32> {
        // H = [1 1 0 1 1 0 0]
        //     [1 0 1 1 0 1 0]
        //     [0 1 1 1 0 0 1]
        // Column j: read bits row 0..2
        vec![
            0b011, // col 0: rows 0,1
            0b101, // col 1: rows 0,2
            0b110, // col 2: rows 1,2
            0b111, // col 3: rows 0,1,2
            0b001, // col 4: row 0
            0b010, // col 5: row 1
            0b100, // col 6: row 2
        ]
    }

    #[test]
    fn test_gpu_bcjr_hamming74_noiseless() {
        let h_cols = hamming74_h_cols();
        let gpu = GpuBcjrBatch::new(&h_cols, 7, 4, 8).unwrap();

        // All-zero codeword, high-confidence LLRs
        let input = vec![5.0f32; 7];
        let (app, ext) = gpu.decode_batch(std::slice::from_ref(&input)).unwrap();

        assert_eq!(app.len(), 1);
        assert_eq!(app[0].len(), 7);
        // All APP LLRs should be positive (favoring bit 0)
        for (j, &val) in app[0].iter().enumerate() {
            assert!(
                val > 0.0,
                "APP LLR at bit {} should be positive, got {}",
                j,
                val
            );
        }
        // Extrinsic identity: ext = app - input
        for j in 0..7 {
            let expected = app[0][j] - input[j];
            assert!(
                (ext[0][j] - expected).abs() < 1e-4,
                "extrinsic mismatch at bit {}",
                j
            );
        }
    }

    #[test]
    fn test_gpu_bcjr_batch_multiple() {
        let h_cols = hamming74_h_cols();
        let gpu = GpuBcjrBatch::new(&h_cols, 7, 4, 8).unwrap();

        let inputs = vec![
            vec![5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0],
            vec![-5.0, -5.0, -5.0, -5.0, -5.0, -5.0, -5.0],
            vec![2.0, -1.5, 3.0, 0.5, -2.0, 1.0, -0.5],
            vec![-3.0, 2.0, 1.0, -1.0, 0.5, -2.5, 3.0],
        ];

        let (app, _ext) = gpu.decode_batch(&inputs).unwrap();
        assert_eq!(app.len(), 4);

        // First input (all +5): all APP should be positive
        assert!(app[0].iter().all(|&v| v > 0.0));
        // Second input (all -5): all APP should be negative
        assert!(app[1].iter().all(|&v| v < 0.0));
    }

    #[test]
    fn test_gpu_bcjr_empty_batch() {
        let h_cols = hamming74_h_cols();
        let gpu = GpuBcjrBatch::new(&h_cols, 7, 4, 8).unwrap();

        let (app, ext) = gpu.decode_batch(&[]).unwrap();
        assert!(app.is_empty());
        assert!(ext.is_empty());
    }
}
