//! Raw FFI bindings to the HIP BCJR kernel.
//!
//! These functions are implemented in `hip/bcjr_kernel.hip` and compiled
//! by hipcc via `build.rs`. All pointers are device pointers unless noted.

use std::ffi::c_void;
use std::os::raw::c_int;

extern "C" {
    /// Launch batched BCJR forward-backward kernel.
    ///
    /// # Arguments
    /// - `combined_llrs`: device ptr, `[batch_size][n]` f32
    /// - `h_cols`: device ptr, `[n]` u32
    /// - `app_llrs`: device ptr (output), `[batch_size][n]` f32
    /// - `alpha_workspace`: device ptr, `[batch_size][n+1][num_states]` f32
    /// - `batch_size`: number of BCJR decodes to run in parallel
    /// - `n`: codeword length
    /// - `num_states`: 2^(n-k) trellis states
    /// - `stream`: hipStream_t (null for default stream)
    ///
    /// # Returns
    /// 0 on success (hipSuccess), nonzero on error.
    pub fn launch_bcjr_batch(
        combined_llrs: *const f32,
        h_cols: *const u32,
        app_llrs: *mut f32,
        alpha_workspace: *mut f32,
        batch_size: c_int,
        n: c_int,
        num_states: c_int,
        stream: *mut c_void,
    ) -> c_int;

    pub fn hip_malloc(ptr: *mut *mut c_void, size: usize) -> c_int;
    pub fn hip_free(ptr: *mut c_void) -> c_int;
    pub fn hip_memcpy_h2d(dst: *mut c_void, src: *const c_void, size: usize) -> c_int;
    pub fn hip_memcpy_d2h(dst: *mut c_void, src: *const c_void, size: usize) -> c_int;
    pub fn hip_device_synchronize() -> c_int;
}
