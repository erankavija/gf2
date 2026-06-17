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

    /// Launch the device ChaCha20 raw-word stream kernel.
    ///
    /// Writes `n_words` consecutive 32-bit ChaCha20 keystream words starting at
    /// absolute stream position `base_word_pos` into `out`. The keystream is
    /// the one a host `ChaCha20Rng::seed_from_u64(seed)` produces (the `key`
    /// is the 8-word little-endian seed derived by rand_core's PCG32
    /// expansion). One device thread per word.
    ///
    /// # Arguments
    /// - `key`: device ptr, `[8]` u32 little-endian ChaCha key words.
    /// - `base_word_pos`: absolute ChaCha 32-bit-word position to start at.
    /// - `out`: device ptr (output), `[n_words]` u32.
    /// - `n_words`: number of words to emit.
    /// - `stream`: hipStream_t (null for default stream).
    ///
    /// # Returns
    /// 0 on success (hipSuccess), nonzero on error.
    pub fn launch_chacha20_words(
        key: *const u32,
        base_word_pos: u64,
        out: *mut u32,
        n_words: c_int,
        stream: *mut c_void,
    ) -> c_int;

    /// Launch the device ChaCha20 + Box-Muller AWGN noise-sample kernel.
    ///
    /// Writes `n_samples` f32 standard-normal `N(0, 1)` samples into `out`,
    /// one thread per sample. Sample `s` consumes the 4 ChaCha words at
    /// `base_word_pos + 4*s` (u1 from words 0,1; u2 from words 2,3), matching
    /// the CPU `draw_standard_normal` order. The samples agree with the CPU
    /// Box-Muller transform to <= 1 ulp f32 (design doc §11).
    ///
    /// # Arguments
    /// - `key`: device ptr, `[8]` u32 little-endian ChaCha key words.
    /// - `base_word_pos`: frame's `worker_offset(...)` in ChaCha word units
    ///   (a multiple of 16).
    /// - `out`: device ptr (output), `[n_samples]` f32 `N(0, 1)` samples.
    /// - `n_samples`: number of standard-normal samples to draw.
    /// - `stream`: hipStream_t (null for default stream).
    ///
    /// # Returns
    /// 0 on success (hipSuccess), nonzero on error.
    pub fn launch_chacha20_awgn(
        key: *const u32,
        base_word_pos: u64,
        out: *mut f32,
        n_samples: c_int,
        stream: *mut c_void,
    ) -> c_int;

    /// Initialise the var-to-check messages with the channel LLRs.
    ///
    /// Sets `v2c[b*edges + f] = channel_llrs[b*n + v]` for every var-edge `f`
    /// of variable `v`, matching the CPU decoder's per-frame init. One device
    /// thread per `(frame, variable)`.
    ///
    /// # Returns
    /// 0 on success (hipSuccess), nonzero on error.
    pub fn launch_ldpc_init(
        channel_llrs: *const f32,
        v2c: *mut f32,
        var_col_ptr: *const c_int,
        n: c_int,
        edges: c_int,
        batch_size: c_int,
        stream: *mut c_void,
    ) -> c_int;

    /// Check-node update for one BP half-iteration.
    ///
    /// For each check `c` and each of its edges `e` (CSR row order) writes
    /// `c2v[b*edges + e]` from the box-plus of all OTHER edges' var-to-check
    /// messages, gathered via `check_edge_to_var_edge`. `algorithm` selects the
    /// rule (0=MinSum, 1=NormalizedMinSum(alpha), 2=OffsetMinSum(beta),
    /// 3=SumProduct). One device thread per `(frame, check)`.
    ///
    /// `frame_done`: per-frame freeze flags (`[batch]`), or null when early
    /// termination is off. A frame with `frame_done[b] != 0` is skipped so its
    /// `c2v` stays at the first-convergence state (design §11 byte-identity).
    ///
    /// The kernel is standard-agnostic: any per-`i_LS` cyclic shift is folded
    /// into the flat CSR layout host-side (design §6), so there is no in-kernel
    /// shift parameter (5G NR reuses this binary via host-side expansion in
    /// Phase E `23d3525f`).
    ///
    /// # Returns
    /// 0 on success (hipSuccess), nonzero on error.
    pub fn launch_ldpc_check_update(
        v2c: *const f32,
        c2v: *mut f32,
        check_row_ptr: *const c_int,
        check_edge_to_var_edge: *const c_int,
        frame_done: *const u8,
        m: c_int,
        edges: c_int,
        batch_size: c_int,
        algorithm: c_int,
        alpha: f32,
        beta: f32,
        stream: *mut c_void,
    ) -> c_int;

    /// Variable-node update for one BP half-iteration.
    ///
    /// Computes `belief = channel + sum(incoming c2v)` (CSC column order) per
    /// variable, writes `v2c[f] = belief - incoming[f]`, and the hard decision
    /// `hard_bits[b*n + v] = (belief < 0)`. One device thread per
    /// `(frame, variable)`.
    ///
    /// `frame_done`: per-frame freeze flags (`[batch]`), or null when early
    /// termination is off. A frame with `frame_done[b] != 0` is skipped so its
    /// `v2c` and `hard_bits` stay at the first-convergence state.
    ///
    /// # Returns
    /// 0 on success (hipSuccess), nonzero on error.
    pub fn launch_ldpc_var_update(
        channel_llrs: *const f32,
        v2c: *mut f32,
        c2v: *const f32,
        var_col_ptr: *const c_int,
        var_edge_to_check_edge: *const c_int,
        hard_bits: *mut u8,
        frame_done: *const u8,
        n: c_int,
        edges: c_int,
        batch_size: c_int,
        stream: *mut c_void,
    ) -> c_int;

    /// Per-frame syndrome check.
    ///
    /// Sets `frame_unsatisfied[b] = 1` if any check `c` is violated by the
    /// current `hard_bits` of frame `b`. One device thread per `(frame, check)`.
    ///
    /// `frame_done`: per-frame freeze flags (`[batch]`), or null when early
    /// termination is off. A frame with `frame_done[b] != 0` is skipped (already
    /// converged: it stays satisfied and never re-marks `frame_unsatisfied`).
    ///
    /// # Returns
    /// 0 on success (hipSuccess), nonzero on error.
    pub fn launch_ldpc_syndrome(
        hard_bits: *const u8,
        check_row_ptr: *const c_int,
        check_edge_var: *const c_int,
        frame_unsatisfied: *mut u8,
        frame_done: *const u8,
        m: c_int,
        n: c_int,
        batch_size: c_int,
        stream: *mut c_void,
    ) -> c_int;

    pub fn hip_malloc(ptr: *mut *mut c_void, size: usize) -> c_int;
    pub fn hip_free(ptr: *mut c_void) -> c_int;
    pub fn hip_memcpy_h2d(dst: *mut c_void, src: *const c_void, size: usize) -> c_int;
    pub fn hip_memcpy_d2h(dst: *mut c_void, src: *const c_void, size: usize) -> c_int;
    pub fn hip_device_synchronize() -> c_int;

    // ---- Host-runtime wrappers (hip/host_runtime.hip) --------------------
    // Stream management.
    pub fn hip_stream_create(stream: *mut *mut c_void) -> c_int;
    pub fn hip_stream_destroy(stream: *mut c_void) -> c_int;
    pub fn hip_stream_synchronize(stream: *mut c_void) -> c_int;
    pub fn hip_stream_query(stream: *mut c_void) -> c_int;

    // Device selection / introspection.
    pub fn hip_set_device(device_id: c_int) -> c_int;
    pub fn hip_get_device(device_id: *mut c_int) -> c_int;
    pub fn hip_device_get_count(count: *mut c_int) -> c_int;
    /// Writes the device's GCN arch name (`hipDeviceProp_t.gcnArchName`, e.g.
    /// `"gfx1030"`, `"gfx940"`, `"gfx942"`) into `buf` (capacity `buf_len`
    /// bytes), NUL-terminated. This is the authoritative kernel-blob
    /// discriminator (design doc §6); compute capability cannot distinguish
    /// gfx940 from gfx942.
    pub fn hip_device_get_arch_name(
        device_id: c_int,
        buf: *mut std::os::raw::c_char,
        buf_len: usize,
    ) -> c_int;
    pub fn hip_mem_get_info(free_bytes: *mut usize, total_bytes: *mut usize) -> c_int;

    // Pinned host memory.
    pub fn hip_host_malloc(ptr: *mut *mut c_void, size: usize) -> c_int;
    pub fn hip_host_free(ptr: *mut c_void) -> c_int;

    // Stream-ordered transfers.
    pub fn hip_memcpy_h2d_async(
        dst: *mut c_void,
        src: *const c_void,
        size: usize,
        stream: *mut c_void,
    ) -> c_int;
    pub fn hip_memcpy_d2h_async(
        dst: *mut c_void,
        src: *const c_void,
        size: usize,
        stream: *mut c_void,
    ) -> c_int;
}

// ---- BCH syndrome device kernels (hip/bch_syndrome.hip) -------------------
//
// These two externs resolve against `hip/bch_syndrome.hip`, which `build.rs`
// only compiles under the `hip` feature (issue `9012f8a0` criterion 6). Gating
// the declarations keeps the default (non-`hip`) build from referencing symbols
// the static lib does not contain.
#[cfg(feature = "hip")]
extern "C" {
    /// Launch the batch BCH syndrome evaluator.
    ///
    /// For each frame and each evaluation point `α^(i+1)` (`i = 0..two_t-1`),
    /// computes the syndrome `S_{i+1} = r(α^(i+1))` by Horner's rule over
    /// GF(2^m) using the uploaded `exp` / `log` tables. One device thread per
    /// `(frame, point)`. Byte-identical to the CPU
    /// `BchDecoder::compute_syndromes` path (design doc §5, §6, §10).
    ///
    /// # Arguments
    /// - `d_coeffs`: device ptr, `[batch_size * words_per_frame]` u64 packed
    ///   coefficient streams in the design-doc §3.1 order (parity reversed ++
    ///   message reversed), little-endian bit order.
    /// - `d_points`: device ptr, `[two_t]` u16 evaluation points `α^1..α^(2t)`.
    /// - `d_log`: device ptr, `[2^m]` u16 discrete-log table.
    /// - `d_exp`: device ptr, `[2^m - 1]` u16 antilog table.
    /// - `d_syndromes`: device ptr (output), `[batch_size * two_t]` u16
    ///   syndromes, row-major per frame.
    /// - `n`: codeword length (coefficient count).
    /// - `two_t`: number of syndromes per frame (`2t`).
    /// - `words_per_frame`: `ceil(n / 64)` u64 words per packed coeff stream.
    /// - `order`: `2^m - 1` (the multiplicative-group order / modulus).
    /// - `batch_size`: number of frames.
    /// - `stream`: hipStream_t (null for default stream).
    ///
    /// # Returns
    /// 0 on success (hipSuccess), nonzero on error.
    pub fn launch_bch_syndrome(
        d_coeffs: *const u64,
        d_points: *const u16,
        d_log: *const u16,
        d_exp: *const u16,
        d_syndromes: *mut u16,
        n: c_int,
        two_t: c_int,
        words_per_frame: c_int,
        order: u32,
        batch_size: c_int,
        stream: *mut c_void,
    ) -> c_int;

    /// Launch the standalone device `gf_mul` test kernel.
    ///
    /// Computes `d_out[j] = gf_mul(d_a[j], d_b[j])` over GF(2^m) using the
    /// uploaded `exp` / `log` tables — the SAME multiply the syndrome kernel
    /// uses. Exposed for the exhaustive GF(2^m) correctness rung (design doc
    /// §10 rung 1). One device thread per pair.
    ///
    /// # Arguments
    /// - `d_a` / `d_b`: device ptrs, `[count]` u16 operands.
    /// - `d_log`: device ptr, `[2^m]` u16 discrete-log table.
    /// - `d_exp`: device ptr, `[2^m - 1]` u16 antilog table.
    /// - `d_out`: device ptr (output), `[count]` u16 products.
    /// - `order`: `2^m - 1`.
    /// - `count`: number of `(a, b)` pairs.
    /// - `stream`: hipStream_t (null for default stream).
    ///
    /// # Returns
    /// 0 on success (hipSuccess), nonzero on error.
    pub fn launch_gf_mul_test(
        d_a: *const u16,
        d_b: *const u16,
        d_log: *const u16,
        d_exp: *const u16,
        d_out: *mut u16,
        order: u32,
        count: c_int,
        stream: *mut c_void,
    ) -> c_int;
}
