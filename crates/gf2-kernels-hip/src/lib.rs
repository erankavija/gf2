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
}

impl HipError {
    /// Returns the underlying `hipError_t` code for a [`HipError::Hip`], or
    /// `hipErrorOutOfMemory` (2) for an [`HipError::OutOfMemory`].
    pub fn code(&self) -> i32 {
        match self {
            HipError::Hip { code, .. } => *code,
            // hipErrorOutOfMemory is canonically 2.
            HipError::OutOfMemory { .. } => 2,
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

/// RAII wrapper for a HIP device allocation.
///
/// Accessible within the crate (`pub(crate)`) so the `permanent` submodule
/// can use it for the safe host-dispatch wrappers without re-duplicating
/// the alloc/free boilerplate.
pub(crate) struct DeviceBuffer {
    ptr: *mut c_void,
    size: usize,
}

impl DeviceBuffer {
    pub(crate) fn new(size: usize) -> Result<Self, HipError> {
        let mut ptr: *mut c_void = ptr::null_mut();
        // SAFETY: hip_malloc writes a valid device pointer to `ptr` on success.
        // The pointer is freed in Drop. `size` is validated by the HIP runtime.
        check_hip(unsafe { ffi::hip_malloc(&mut ptr, size) }, "hipMalloc")?;
        Ok(Self { ptr, size })
    }

    pub(crate) fn as_ptr(&self) -> *const c_void {
        self.ptr as *const c_void
    }

    pub(crate) fn as_mut_ptr(&self) -> *mut c_void {
        self.ptr
    }

    pub(crate) fn copy_from_host(&self, src: &[u8]) -> Result<(), HipError> {
        assert!(src.len() <= self.size);
        // SAFETY: `self.ptr` is a valid device allocation of `self.size` bytes.
        // `src` is a valid host slice. HIP copies `src.len()` bytes H→D.
        check_hip(
            unsafe { ffi::hip_memcpy_h2d(self.ptr, src.as_ptr() as *const c_void, src.len()) },
            "hipMemcpy H2D",
        )
    }

    pub(crate) fn copy_to_host(&self, dst: &mut [u8]) -> Result<(), HipError> {
        assert!(dst.len() <= self.size);
        // SAFETY: `self.ptr` is a valid device allocation. `dst` is a valid
        // mutable host slice. HIP copies `dst.len()` bytes D→H.
        check_hip(
            unsafe { ffi::hip_memcpy_d2h(dst.as_mut_ptr() as *mut c_void, self.ptr, dst.len()) },
            "hipMemcpy D2H",
        )
    }
}

impl Drop for DeviceBuffer {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            // SAFETY: `self.ptr` was allocated by `hip_malloc` in `new()` and
            // has not been freed yet (we only free in Drop, which runs once).
            unsafe {
                ffi::hip_free(self.ptr);
            }
        }
    }
}

// SAFETY: HIP device pointers are opaque handles managed by the HIP runtime,
// which is thread-safe. The pointer is not dereferenced on the host — all
// access goes through HIP API calls (hipMemcpy, kernel launch) which
// synchronize internally. Sending the handle to another thread is safe.
unsafe impl Send for DeviceBuffer {}

/// GPU-accelerated batch BCJR decoder.
///
/// Holds persistent device allocations for the trellis columns and workspace
/// buffers. Reusable across multiple `decode_batch` calls.
pub struct GpuBcjrBatch {
    d_h_cols: DeviceBuffer,
    d_llrs: DeviceBuffer,
    d_app: DeviceBuffer,
    d_alpha_ws: DeviceBuffer,
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
        let d_h_cols = DeviceBuffer::new(n * std::mem::size_of::<u32>())?;
        let d_llrs = DeviceBuffer::new(max_batch * n * std::mem::size_of::<f32>())?;
        let d_app = DeviceBuffer::new(max_batch * n * std::mem::size_of::<f32>())?;
        let d_alpha_ws =
            DeviceBuffer::new(max_batch * (n + 1) * num_states * std::mem::size_of::<f32>())?;

        // Upload h_cols (persistent — same trellis for all decodes)
        // SAFETY: h_cols is a valid &[u32]; reinterpreting as &[u8] with
        // len * 4 bytes is safe because u32 has no padding and align >= 1.
        let h_bytes: &[u8] =
            unsafe { std::slice::from_raw_parts(h_cols.as_ptr() as *const u8, h_cols.len() * 4) };
        d_h_cols.copy_from_host(h_bytes)?;

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
        // SAFETY: Reinterpreting &[f32] as &[u8] with len*4 bytes is safe
        // (f32 has no padding, alignment >= 1 for u8).
        let llr_bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(flat_llrs.as_ptr() as *const u8, flat_llrs.len() * 4)
        };
        self.d_llrs.copy_from_host(llr_bytes)?;

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
        // SAFETY: Reinterpreting &mut [f32] as &mut [u8] is safe (no padding).
        let app_bytes: &mut [u8] = unsafe {
            std::slice::from_raw_parts_mut(flat_app.as_mut_ptr() as *mut u8, flat_app.len() * 4)
        };
        self.d_app.copy_to_host(app_bytes)?;

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
    d_rx_i: DeviceBuffer,
    d_rx_q: DeviceBuffer,
    d_gain_i: DeviceBuffer,
    d_gain_q: DeviceBuffer,
    d_noise_var: DeviceBuffer,
    d_pam_levels: DeviceBuffer,
    d_out_llrs: DeviceBuffer,
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
        let d_rx_i = DeviceBuffer::new(max_batch * f32_size)?;
        let d_rx_q = DeviceBuffer::new(max_batch * f32_size)?;
        let d_gain_i = DeviceBuffer::new(max_batch * f32_size)?;
        let d_gain_q = DeviceBuffer::new(max_batch * f32_size)?;
        let d_noise_var = DeviceBuffer::new(max_batch * f32_size)?;
        let d_pam_levels = DeviceBuffer::new(axis_len * f32_size)?;
        let d_out_llrs = DeviceBuffer::new(max_batch * (m as usize) * f32_size)?;

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

// SAFETY: all contained device buffers are `Send` (see the impl on
// `DeviceBuffer`); the configuration fields are plain `Copy` values.
unsafe impl Send for GpuGrayQamDemapper {}

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
