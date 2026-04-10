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
//! let mut gpu = GpuBcjrBatch::new(&h_cols, 7, 4, 64).unwrap();
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
    pub code: i32,
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

/// RAII wrapper for a HIP device allocation.
struct DeviceBuffer {
    ptr: *mut c_void,
    size: usize,
}

impl DeviceBuffer {
    fn new(size: usize) -> Result<Self, HipError> {
        let mut ptr: *mut c_void = ptr::null_mut();
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
        check_hip(
            unsafe { ffi::hip_memcpy_h2d(self.ptr, src.as_ptr() as *const c_void, src.len()) },
            "hipMemcpy H2D",
        )
    }

    fn copy_to_host(&self, dst: &mut [u8]) -> Result<(), HipError> {
        assert!(dst.len() <= self.size);
        check_hip(
            unsafe {
                ffi::hip_memcpy_d2h(dst.as_mut_ptr() as *mut c_void, self.ptr, dst.len())
            },
            "hipMemcpy D2H",
        )
    }
}

impl Drop for DeviceBuffer {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                ffi::hip_free(self.ptr);
            }
        }
    }
}

// DeviceBuffer is not Send/Sync by default due to raw pointer.
// HIP device pointers are safe to send across threads (the HIP runtime is thread-safe).
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
    /// # Arguments
    ///
    /// * `h_cols` - Parity-check matrix columns as u32 bitmasks (length n).
    /// * `n` - Codeword length.
    /// * `k` - Message length.
    /// * `max_batch` - Maximum batch size (pre-allocates device memory).
    ///
    /// # Errors
    ///
    /// Returns `HipError` if device memory allocation fails.
    pub fn new(h_cols: &[u32], n: usize, k: usize, max_batch: usize) -> Result<Self, HipError> {
        assert_eq!(h_cols.len(), n);
        let num_states = 1usize << (n - k);

        // Allocate device buffers
        let d_h_cols = DeviceBuffer::new(n * std::mem::size_of::<u32>())?;
        let d_llrs = DeviceBuffer::new(max_batch * n * std::mem::size_of::<f32>())?;
        let d_app = DeviceBuffer::new(max_batch * n * std::mem::size_of::<f32>())?;
        let d_alpha_ws =
            DeviceBuffer::new(max_batch * (n + 1) * num_states * std::mem::size_of::<f32>())?;

        // Upload h_cols (persistent — same trellis for all decodes)
        let h_bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(h_cols.as_ptr() as *const u8, h_cols.len() * 4)
        };
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
    pub fn decode_batch(
        &mut self,
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
        let llr_bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(flat_llrs.as_ptr() as *const u8, flat_llrs.len() * 4)
        };
        self.d_llrs.copy_from_host(llr_bytes)?;

        // Launch kernel
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

        // Synchronize
        check_hip(unsafe { ffi::hip_device_synchronize() }, "hipDeviceSynchronize")?;

        // Download APP results
        let mut flat_app = vec![0.0f32; batch_size * n];
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
        let mut gpu = GpuBcjrBatch::new(&h_cols, 7, 4, 8).unwrap();

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
                j, val
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
        let mut gpu = GpuBcjrBatch::new(&h_cols, 7, 4, 8).unwrap();

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
        let mut gpu = GpuBcjrBatch::new(&h_cols, 7, 4, 8).unwrap();

        let (app, ext) = gpu.decode_batch(&[]).unwrap();
        assert!(app.is_empty());
        assert!(ext.is_empty());
    }
}
