//! HIP/ROCm GPU kernels for the gf2 workspace: batch kernels for BCJR
//! decoding, Gray-QAM soft demapping, LDPC belief propagation, ChaCha20 AWGN
//! generation, and, under the `hip` feature, BCH syndrome evaluation and
//! F_3/F_5/F_7 permanents.
//!
//! # Requirements
//!
//! - ROCm with `hipcc`.
//! - An AMD GPU with the gfx1030 ISA for the linked kernels, which `build.rs`
//!   compiles for gfx1030 only. [`host::GfxTarget::detect`] returns
//!   [`HipError::UnsupportedArch`] for a device whose arch has no compiled
//!   blob.

pub(crate) mod ffi;

pub mod host;

pub mod launch_chacha20_awgn;

#[doc(inline)]
pub use launch_chacha20_awgn::{chacha20_key_from_seed, AwgnStreamScratch, GpuChaChaAwgn};

pub mod launch_ldpc_bp;

#[doc(inline)]
pub use launch_ldpc_bp::{GpuBpAlgorithm, GpuLdpcBp, LdpcGraphLayout, LdpcStreamScratch};

#[cfg(feature = "hip")]
pub mod launch_bch_syndrome;

#[cfg(feature = "hip")]
#[doc(inline)]
pub use launch_bch_syndrome::{BchFieldTables, GpuBchSyndrome};

#[cfg(feature = "hip")]
pub mod permanent;

use std::ffi::c_void;
use std::ptr;

/// Error type for HIP operations.
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
    OutOfMemory {
        /// The HIP device that ran out of memory.
        device_id: i32,
        /// The allocation size, in bytes, that failed.
        bytes_requested: usize,
    },
    /// No HIP device is visible to the runtime (`hipGetDeviceCount() == 0`).
    NoDevice,
    /// The detected device runs a gfx arch this build does not have a kernel
    /// blob for.
    UnsupportedArch {
        /// The device's GCN arch name as reported by `gcnArchName` (e.g.
        /// `"gfx908"`), with any feature suffix stripped.
        gcn_arch_name: String,
    },
    /// A precompiled kernel blob (`*.co`) could not be read from disk.
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
    pub fn code(&self) -> i32 {
        match self {
            HipError::Hip { code, .. } => *code,
            HipError::OutOfMemory { .. } => 2,
            HipError::NoDevice => 100,
            HipError::UnsupportedArch { .. } => 101,
            HipError::BlobLoad { .. } => 301,
        }
    }

    /// Reports whether a caller may answer this failure with a safe fallback
    /// (`@/inv/accelerator-safe-fallback`): [`HipError::OutOfMemory`] and
    /// [`HipError::UnsupportedArch`] are recoverable; [`HipError::NoDevice`],
    /// [`HipError::BlobLoad`] and [`HipError::Hip`] are fatal.
    pub fn is_recoverable(&self) -> bool {
        match self {
            HipError::OutOfMemory { .. } | HipError::UnsupportedArch { .. } => true,
            HipError::Hip { .. } | HipError::NoDevice | HipError::BlobLoad { .. } => false,
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

/// Extracts parity-check matrix columns as u32 bitmasks: element `j` has bit
/// `i` set iff `H[i][j] == 1`.
///
/// # Panics
///
/// Panics if the matrix has more than 32 rows.
pub fn extract_h_cols(h: &gf2_core::BitMatrix) -> Vec<u32> {
    h.cols_as_u32_masks()
}

/// Byte-sized adapter over [`host::DeviceBuffer<u8>`](crate::host::DeviceBuffer)
/// on device 0, for kernels that pass typed payloads as byte slices.
pub(crate) struct DecoderDeviceBuffer {
    inner: host::DeviceBuffer<u8>,
}

impl DecoderDeviceBuffer {
    pub(crate) fn new(size: usize) -> Result<Self, HipError> {
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

    pub(crate) fn copy_from_pinned_async(
        &self,
        src: &host::PinnedHostBuffer<u8>,
        stream: &host::HipStream,
    ) -> Result<(), HipError> {
        self.inner.copy_from_pinned_async(src, stream)
    }

    pub(crate) fn copy_to_pinned_async(
        &self,
        dst: &mut host::PinnedHostBuffer<u8>,
        stream: &host::HipStream,
    ) -> Result<(), HipError> {
        self.inner.copy_to_pinned_async(dst, stream)
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
    /// Pre-allocates device memory for up to `max_batch` simultaneous BCJR
    /// decodes and uploads the trellis columns.
    ///
    /// `h_cols` holds the `n` parity-check matrix columns as u32 bitmasks
    /// (see [`extract_h_cols`]); `k` is the message length.
    ///
    /// # Errors
    ///
    /// Returns `HipError` if device memory allocation or the upload fails.
    ///
    /// # Panics
    ///
    /// Panics if `h_cols.len() != n` or if `2^(n-k) > MAX_GPU_STATES`.
    ///
    /// # Complexity
    ///
    /// O(max_batch * n * 2^(n-k)) device memory.
    pub fn new(h_cols: &[u32], n: usize, k: usize, max_batch: usize) -> Result<Self, HipError> {
        assert_eq!(h_cols.len(), n);
        let num_states = 1usize << (n - k);
        assert!(
            num_states <= MAX_GPU_STATES,
            "2^(n-k) = {} exceeds GPU kernel limit of {} states",
            num_states,
            MAX_GPU_STATES
        );

        let d_h_cols = DecoderDeviceBuffer::new(n * std::mem::size_of::<u32>())?;
        let d_llrs = DecoderDeviceBuffer::new(max_batch * n * std::mem::size_of::<f32>())?;
        let d_app = DecoderDeviceBuffer::new(max_batch * n * std::mem::size_of::<f32>())?;
        let d_alpha_ws = DecoderDeviceBuffer::new(
            max_batch * (n + 1) * num_states * std::mem::size_of::<f32>(),
        )?;

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

    /// Decodes a batch of SISO inputs, each a combined LLR vector of length
    /// `n`, and returns `(app_llrs, extrinsic_llrs)` with one length-`n`
    /// vector per input.
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
    /// O(batch_size * n * num_states) GPU work.
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

        let mut flat_llrs: Vec<f32> = Vec::with_capacity(batch_size * n);
        for inp in inputs {
            flat_llrs.extend_from_slice(inp);
        }
        self.d_llrs.copy_from_host(f32_slice_as_bytes(&flat_llrs))?;

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

        let mut flat_app = vec![0.0f32; batch_size * n];
        self.d_app
            .copy_to_host(f32_slice_as_bytes_mut(&mut flat_app))?;

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
#[inline]
fn u32_slice_as_bytes(src: &[u32]) -> &[u8] {
    // SAFETY: u32 has no padding and u8 alignment is 1; the byte count is
    // computed from `src` so the slice spans the same memory region.
    unsafe { std::slice::from_raw_parts(src.as_ptr() as *const u8, std::mem::size_of_val(src)) }
}

/// Reinterprets a `&[f32]` as a byte slice for H→D copies.
#[inline]
fn f32_slice_as_bytes(src: &[f32]) -> &[u8] {
    // SAFETY: f32 has no padding and u8 alignment is 1; the total byte
    // count is computed from `src` so the resulting slice spans the
    // same memory region as the input.
    unsafe { std::slice::from_raw_parts(src.as_ptr() as *const u8, std::mem::size_of_val(src)) }
}

/// Reinterprets a `&mut [f32]` as a mutable byte slice for D→H copies.
#[inline]
fn f32_slice_as_bytes_mut(dst: &mut [f32]) -> &mut [u8] {
    let len = std::mem::size_of_val(dst);
    // SAFETY: see f32_slice_as_bytes; mutability is preserved.
    unsafe { std::slice::from_raw_parts_mut(dst.as_mut_ptr() as *mut u8, len) }
}

/// GPU batch Gray square-QAM / BPSK soft demapper (max-log).
///
/// Under AWGN with independent I/Q noise the per-symbol 2D max-log decomposes
/// into two 1D Gray-PAM max-log LLRs of size `sqrt(M)` each, so the cost is
/// `O(num_symbols * sqrt(M) * m)`. The kernel implements the max-log variant
/// for Gray square-QAM and BPSK only.
///
/// The device-side `pam_levels` table and the input / output buffers are
/// allocated once at construction for up to `max_batch` symbols.
/// [`demap_batch`](GpuGrayQamDemapper::demap_batch) runs on the default stream
/// with synchronous transfers;
/// [`demap_batch_on_stream`](GpuGrayQamDemapper::demap_batch_on_stream) orders
/// the launch and every transfer on a caller-owned [`host::HipStream`].
/// `test_demap_on_stream_matches_default_stream` checks on a 16-QAM batch that
/// both return bit-identical LLRs.
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

/// Pinned host staging for [`GpuGrayQamDemapper::demap_batch_on_stream`].
///
/// A synchronous `hipMemcpy` executes on the NULL stream and serializes
/// against every other blocking stream on the device, so the stream path
/// stages all transfers through these page-locked buffers with
/// `hipMemcpyAsync`. A scratch is sized for one demapper's `max_batch` / `m`
/// and is `Send`-only: one per worker thread.
pub struct DemapStreamScratch {
    rx_i: host::PinnedHostBuffer<u8>,
    rx_q: host::PinnedHostBuffer<u8>,
    gain_i: host::PinnedHostBuffer<u8>,
    gain_q: host::PinnedHostBuffer<u8>,
    noise_var: host::PinnedHostBuffer<u8>,
    out_llrs: host::PinnedHostBuffer<u8>,
    max_batch: usize,
    m: u8,
}

impl DemapStreamScratch {
    /// The `max_batch` of the [`GpuGrayQamDemapper`] this scratch was sized for.
    #[must_use]
    pub fn max_batch(&self) -> usize {
        self.max_batch
    }

    /// The bits-per-symbol `m` of the [`GpuGrayQamDemapper`] this scratch was
    /// sized for.
    #[must_use]
    pub fn m(&self) -> u8 {
        self.m
    }
}

impl GpuGrayQamDemapper {
    /// Constructs a demapper for a fixed Gray square-QAM / BPSK preset and
    /// uploads the PAM level table.
    ///
    /// `pam_levels` holds the post-normalization Gray-PAM levels shared by the
    /// I and Q axes: `1 << (m / 2)` entries for QAM, `2` for BPSK. `max_batch`
    /// is the maximum number of symbols per `demap_batch` call.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] if device allocation or the `pam_levels` upload
    /// fails.
    ///
    /// # Panics
    ///
    /// Panics if `m` is not in `{1, 2, 4, 6, 8}`, if `pam_levels` has the
    /// wrong length for `m`, or if `is_bpsk` disagrees with `m == 1`.
    ///
    /// # Complexity
    ///
    /// O(`max_batch * m`) device memory.
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
    /// Returns `num_symbols * m` LLRs in symbol-major, MSB-first layout: for
    /// QAM the first `m/2` bits of each symbol are the I-axis Gray-PAM label
    /// (MSB = coarsest level), followed by `m/2` Q-axis bits. `rx_i` / `rx_q`
    /// define `num_symbols`; `gain_i` / `gain_q` are the per-symbol complex
    /// channel gain (both `None` for AWGN); `noise_var` is the per-symbol
    /// `N0 = 2 sigma^2`.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] on device memcpy, kernel launch, or
    /// synchronization failures.
    ///
    /// # Panics
    ///
    /// Panics if `num_symbols > max_batch`, if `rx_i` / `rx_q` / `noise_var`
    /// or a supplied gain have mismatched lengths, or if exactly one of
    /// `gain_i` / `gain_q` is `Some`.
    ///
    /// # Complexity
    ///
    /// O(`num_symbols * axis_len * m`) GPU work.
    pub fn demap_batch(
        &self,
        rx_i: &[f32],
        rx_q: &[f32],
        gain_i: Option<&[f32]>,
        gain_q: Option<&[f32]>,
        noise_var: &[f32],
    ) -> Result<Vec<f32>, HipError> {
        self.demap_inner(rx_i, rx_q, gain_i, gain_q, noise_var, None)
    }

    /// Allocates the pinned host staging
    /// [`demap_batch_on_stream`](Self::demap_batch_on_stream) requires.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] if a pinned allocation fails (an OOM is
    /// [`HipError::OutOfMemory`]).
    ///
    /// # Complexity
    ///
    /// O(`max_batch * m`) pinned host memory.
    pub fn new_stream_scratch(&self) -> Result<DemapStreamScratch, HipError> {
        let f32_size = std::mem::size_of::<f32>();
        // Device 0: matches `DecoderDeviceBuffer`'s single-device contract.
        Ok(DemapStreamScratch {
            rx_i: host::PinnedHostBuffer::new(self.max_batch * f32_size, 0)?,
            rx_q: host::PinnedHostBuffer::new(self.max_batch * f32_size, 0)?,
            gain_i: host::PinnedHostBuffer::new(self.max_batch * f32_size, 0)?,
            gain_q: host::PinnedHostBuffer::new(self.max_batch * f32_size, 0)?,
            noise_var: host::PinnedHostBuffer::new(self.max_batch * f32_size, 0)?,
            out_llrs: host::PinnedHostBuffer::new(self.max_batch * self.m as usize * f32_size, 0)?,
            max_batch: self.max_batch,
            m: self.m,
        })
    }

    /// Like [`demap_batch`](Self::demap_batch), with the kernel launch and
    /// every transfer enqueued on the caller-owned `stream` and completion
    /// awaited with [`host::HipStream::synchronize`]. `scratch` comes from
    /// [`new_stream_scratch`](Self::new_stream_scratch).
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] on device memcpy, kernel launch, or stream
    /// synchronization failure.
    ///
    /// # Panics
    ///
    /// Same as [`demap_batch`](Self::demap_batch), plus if `scratch` was sized
    /// for a different demapper (`max_batch` / `m` mismatch).
    #[allow(clippy::too_many_arguments)]
    pub fn demap_batch_on_stream(
        &self,
        rx_i: &[f32],
        rx_q: &[f32],
        gain_i: Option<&[f32]>,
        gain_q: Option<&[f32]>,
        noise_var: &[f32],
        stream: &host::HipStream,
        scratch: &mut DemapStreamScratch,
    ) -> Result<Vec<f32>, HipError> {
        assert_eq!(
            scratch.max_batch, self.max_batch,
            "DemapStreamScratch max_batch {} does not match demapper max_batch {}",
            scratch.max_batch, self.max_batch
        );
        assert_eq!(
            scratch.m, self.m,
            "DemapStreamScratch m {} does not match demapper m {}",
            scratch.m, self.m
        );
        self.demap_inner(
            rx_i,
            rx_q,
            gain_i,
            gain_q,
            noise_var,
            Some((stream, scratch)),
        )
    }

    /// `io == None` is the default-stream path; `io == Some((stream, scratch))`
    /// orders the launch and every pinned-staged transfer on `stream`.
    fn demap_inner(
        &self,
        rx_i: &[f32],
        rx_q: &[f32],
        gain_i: Option<&[f32]>,
        gain_q: Option<&[f32]>,
        noise_var: &[f32],
        io: Option<(&host::HipStream, &mut DemapStreamScratch)>,
    ) -> Result<Vec<f32>, HipError> {
        // Split the optional stream context once: `stream` is a copied shared
        // borrow (used for the launch handle and synchronize), `staging` keeps
        // the unique borrow over the pinned buffers.
        let (stream, mut staging): (Option<&host::HipStream>, Option<&mut DemapStreamScratch>) =
            match io {
                Some((s, sc)) => (Some(s), Some(sc)),
                None => (None, None),
            };
        let stream_raw: *mut c_void = stream.map_or(ptr::null_mut(), host::HipStream::as_raw);

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

        let payload_bytes = std::mem::size_of_val(rx_i);
        match (stream, staging.as_deref_mut()) {
            (Some(stream), Some(scratch)) => {
                scratch.rx_i.as_mut_slice()[..payload_bytes]
                    .copy_from_slice(f32_slice_as_bytes(rx_i));
                self.d_rx_i.copy_from_pinned_async(&scratch.rx_i, stream)?;
                scratch.rx_q.as_mut_slice()[..payload_bytes]
                    .copy_from_slice(f32_slice_as_bytes(rx_q));
                self.d_rx_q.copy_from_pinned_async(&scratch.rx_q, stream)?;
                scratch.noise_var.as_mut_slice()[..payload_bytes]
                    .copy_from_slice(f32_slice_as_bytes(noise_var));
                self.d_noise_var
                    .copy_from_pinned_async(&scratch.noise_var, stream)?;
            }
            _ => {
                self.d_rx_i.copy_from_host(f32_slice_as_bytes(rx_i))?;
                self.d_rx_q.copy_from_host(f32_slice_as_bytes(rx_q))?;
                self.d_noise_var
                    .copy_from_host(f32_slice_as_bytes(noise_var))?;
            }
        }

        if gains_present {
            let gi = gain_i.expect("gains_present invariant");
            let gq = gain_q.expect("gains_present invariant");
            match (stream, staging.as_deref_mut()) {
                (Some(stream), Some(scratch)) => {
                    scratch.gain_i.as_mut_slice()[..payload_bytes]
                        .copy_from_slice(f32_slice_as_bytes(gi));
                    self.d_gain_i
                        .copy_from_pinned_async(&scratch.gain_i, stream)?;
                    scratch.gain_q.as_mut_slice()[..payload_bytes]
                        .copy_from_slice(f32_slice_as_bytes(gq));
                    self.d_gain_q
                        .copy_from_pinned_async(&scratch.gain_q, stream)?;
                }
                _ => {
                    self.d_gain_i.copy_from_host(f32_slice_as_bytes(gi))?;
                    self.d_gain_q.copy_from_host(f32_slice_as_bytes(gq))?;
                }
            }
        }

        // SAFETY: all device pointers originate from `DeviceBuffer::new` and
        // are sized for `max_batch` symbols; `num_symbols <= max_batch` is
        // asserted above. The gain pointers are null when `gains_present == 0`,
        // in which case the kernel does not dereference them (see
        // hip/gray_qam_demapper.hip). `stream_raw` is either null (default
        // stream) or the caller's live stream handle.
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
                    stream_raw,
                )
            },
            "launch_gray_qam_demap",
        )?;

        let out_len = num_symbols * self.m as usize;
        let mut out = vec![0.0f32; out_len];
        match (stream, staging) {
            (Some(stream), Some(scratch)) => {
                self.d_out_llrs
                    .copy_to_pinned_async(&mut scratch.out_llrs, stream)?;
                stream.synchronize()?;
                let out_bytes = out_len * std::mem::size_of::<f32>();
                f32_slice_as_bytes_mut(&mut out)
                    .copy_from_slice(&scratch.out_llrs.as_slice()[..out_bytes]);
            }
            _ => {
                // SAFETY: hipDeviceSynchronize blocks until all preceding
                // default-stream work completes; no preconditions.
                check_hip(
                    unsafe { ffi::hip_device_synchronize() },
                    "hipDeviceSynchronize",
                )?;
                self.d_out_llrs
                    .copy_to_host(f32_slice_as_bytes_mut(&mut out))?;
            }
        }
        Ok(out)
    }
}

// `GpuGrayQamDemapper` is `Send` by auto-derive; this assertion keeps it so.
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<GpuGrayQamDemapper>();
};

#[cfg(test)]
mod tests {
    use super::*;

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
        for (j, &val) in app[0].iter().enumerate() {
            assert!(
                val > 0.0,
                "APP LLR at bit {} should be positive, got {}",
                j,
                val
            );
        }
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

        assert!(app[0].iter().all(|&v| v > 0.0));
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

    #[cfg(feature = "hip")]
    #[test]
    fn test_demap_on_stream_matches_default_stream() {
        use crate::host::HipStream;

        let pam_levels: Vec<f32> = vec![-3.0, -1.0, 1.0, 3.0]
            .into_iter()
            .map(|v| v / (10.0f32).sqrt())
            .collect();
        let demapper = GpuGrayQamDemapper::new(&pam_levels, 4, false, 64).unwrap();
        let n = 32usize;
        let rx_i: Vec<f32> = (0..n).map(|k| 0.11 * k as f32 - 1.5).collect();
        let rx_q: Vec<f32> = (0..n).map(|k| 1.3 - 0.07 * k as f32).collect();
        let nv = vec![0.25f32; n];

        let default = demapper
            .demap_batch(&rx_i, &rx_q, None, None, &nv)
            .expect("default-stream demap");

        let stream = HipStream::new().expect("create stream");
        let mut scratch = demapper.new_stream_scratch().expect("pinned staging");
        assert_eq!(scratch.max_batch(), 64);
        assert_eq!(scratch.m(), 4);
        let streamed = demapper
            .demap_batch_on_stream(&rx_i, &rx_q, None, None, &nv, &stream, &mut scratch)
            .expect("stream-ordered demap");

        assert_eq!(default.len(), streamed.len());
        for (i, (d, s)) in default.iter().zip(streamed.iter()).enumerate() {
            assert_eq!(
                d.to_bits(),
                s.to_bits(),
                "LLR {i} differs: default={d} stream={s}"
            );
        }
    }
}
