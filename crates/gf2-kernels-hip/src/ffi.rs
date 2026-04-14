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

    /// Launch batched Gray square-QAM / BPSK soft demapper (max-log).
    ///
    /// # Arguments
    /// - `rx_i` / `rx_q`: device ptrs, `[num_symbols]` f32 received samples.
    /// - `gain_i` / `gain_q`: optional device ptrs, `[num_symbols]` f32
    ///   complex channel gain split into real/imag parts. Pass nullptr
    ///   for both when `gains_present == 0` (pure AWGN path).
    /// - `noise_var`: device ptr, `[num_symbols]` f32 per-symbol
    ///   `N0 = 2 sigma^2`.
    /// - `pam_levels`: device ptr, `[axis_len]` f32 Gray-PAM level table
    ///   shared between the I and Q axes.
    /// - `out_llrs`: device ptr (output), `[num_symbols * m]` f32.
    /// - `num_symbols`: batch size.
    /// - `axis_len`: `1 << m_half` for QAM, or `2` for BPSK.
    /// - `m`: bits per symbol (1, 2, 4, 6, or 8).
    /// - `m_half`: `m / 2` for QAM, `0` for BPSK.
    /// - `is_bpsk`: `1` for BPSK, `0` for QAM.
    /// - `gains_present`: `1` if `gain_i` / `gain_q` point to valid
    ///   buffers, `0` otherwise.
    /// - `stream`: hipStream_t (null for default stream).
    ///
    /// # Returns
    /// 0 on success (hipSuccess), nonzero on error.
    pub fn launch_gray_qam_demap(
        rx_i: *const f32,
        rx_q: *const f32,
        gain_i: *const f32,
        gain_q: *const f32,
        noise_var: *const f32,
        pam_levels: *const f32,
        out_llrs: *mut f32,
        num_symbols: c_int,
        axis_len: c_int,
        m: c_int,
        m_half: c_int,
        is_bpsk: c_int,
        gains_present: c_int,
        stream: *mut c_void,
    ) -> c_int;

    pub fn hip_malloc(ptr: *mut *mut c_void, size: usize) -> c_int;
    pub fn hip_free(ptr: *mut c_void) -> c_int;
    pub fn hip_memcpy_h2d(dst: *mut c_void, src: *const c_void, size: usize) -> c_int;
    pub fn hip_memcpy_d2h(dst: *mut c_void, src: *const c_void, size: usize) -> c_int;
    pub fn hip_device_synchronize() -> c_int;
}
