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

mod ffi;

use std::ffi::c_void;
use std::ptr;

/// Error type for HIP operations.
#[derive(Debug, Clone)]
pub struct HipError {
    /// HIP error code (0 = hipSuccess).
    pub code: i32,
    /// Name of the HIP API call that failed.
    pub context: &'static str,
}

impl std::fmt::Display for HipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HIP error {} in {}", self.code, self.context)
    }
}

impl std::error::Error for HipError {}

/// Check a HIP return code, returning Err on failure.
fn check_hip(code: i32, context: &'static str) -> Result<(), HipError> {
    if code == 0 {
        Ok(())
    } else {
        Err(HipError { code, context })
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
struct DeviceBuffer {
    ptr: *mut c_void,
    size: usize,
}

impl DeviceBuffer {
    fn new(size: usize) -> Result<Self, HipError> {
        let mut ptr: *mut c_void = ptr::null_mut();
        // SAFETY: hip_malloc writes a valid device pointer to `ptr` on success.
        // The pointer is freed in Drop. `size` is validated by the HIP runtime.
        check_hip(unsafe { ffi::hip_malloc(&mut ptr, size) }, "hipMalloc")?;
        Ok(Self { ptr, size })
    }

    fn as_ptr(&self) -> *const c_void {
        self.ptr as *const c_void
    }

    fn as_mut_ptr(&self) -> *mut c_void {
        self.ptr
    }

    fn copy_from_host(&self, src: &[u8]) -> Result<(), HipError> {
        assert!(src.len() <= self.size);
        // SAFETY: `self.ptr` is a valid device allocation of `self.size` bytes.
        // `src` is a valid host slice. HIP copies `src.len()` bytes H→D.
        check_hip(
            unsafe { ffi::hip_memcpy_h2d(self.ptr, src.as_ptr() as *const c_void, src.len()) },
            "hipMemcpy H2D",
        )
    }

    fn copy_to_host(&self, dst: &mut [u8]) -> Result<(), HipError> {
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
        let (app, ext) = gpu.decode_batch(&[input.clone()]).unwrap();

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
