//! FFI shims for per-prime permanent computation kernels.
//!
//! Exposes the F_3 Ryser/Bipedal3 permanent kernel (ad55b777), the F_5
//! direct-byte Ryser kernel (b43cdf33), and the F_7 LUT-based Ryser kernel
//! (5c0505b2). The F_3, F_5, and F_7 kernels are all fully implemented.
//!
//! # Safety
//!
//! All functions in this module are `unsafe`. Callers must satisfy the
//! preconditions documented on each function.
//!
//! # Feature gate
//!
//! This module is only compiled when the `hip` Cargo feature is enabled.
//! The corresponding `.hip` source files are compiled by `build.rs` under
//! the same condition.

use std::ffi::c_void;
use std::os::raw::c_int;
use std::time::{Duration, Instant};

use crate::host::{DeviceBuffer, HipEvent, HipEventSpan, HipStream, PinnedHostBuffer};
use crate::HipError;

extern "C" {
    /// Compute the permanent of an n×n matrix over GF(3) on the GPU (single matrix).
    ///
    /// Entry point in `hip/permanent/permanent_bipedal3.hip`. Delegates to
    /// `permanent_bipedal3_hip_batch` with `m=1`.
    ///
    /// # Arguments
    ///
    /// - `matrix_ptr` — device pointer to an n×n row-major array of `u8`
    ///   elements in GF(3) (values 0, 1, 2).
    /// - `n` — matrix dimension (n×n); must satisfy `1 <= n <= 63`. This is
    ///   a GPU-specific limit: the sequential Gray walk at n=64 would require
    ///   2^64 ≈ 1.8×10^19 steps (~600 years on gfx1030). The CPU reference
    ///   `permanent_bipedal3_singleword` supports n=64 via a u128 counter.
    /// - `out_ptr` — device pointer to a single `u64` output that receives
    ///   the permanent value modulo 3 (in `{0, 1, 2}`).
    ///
    /// # Returns
    ///
    /// 0 on success (`hipSuccess`), a non-zero HIP error code otherwise.
    fn permanent_bipedal3_hip(matrix_ptr: *const u8, n: c_int, out_ptr: *mut u64) -> c_int;

    /// Stream-bearing F_3 batch entry point.
    ///
    /// # Safety
    ///
    /// `matrices_ptr` and `out_ptr` must satisfy the same device-allocation
    /// requirements as `permanent_bipedal3_hip_batch`. `stream` must be a live
    /// `hipStream_t` in the active context; it may be null only to select HIP's
    /// default stream. All pointed-to allocations must outlive queued work.
    /// `kernel_start_event` is null for ordinary launches, or a live
    /// timing-enabled `hipEvent_t` from the same context. When non-null, the
    /// wrapper records it immediately before submitting the kernel and returns
    /// that record error without submitting a kernel.
    fn permanent_bipedal3_hip_batch_on_stream(
        matrices_ptr: *const u8,
        n: c_int,
        m: c_int,
        out_ptr: *mut u64,
        stream: *mut c_void,
        kernel_start_event: *mut c_void,
    ) -> c_int;

    /// Compute the permanent of an n×n matrix over GF(5) on the GPU (single matrix).
    ///
    /// Entry point in `hip/permanent/permanent_bipedal5.hip`. Delegates to
    /// `permanent_bipedal5_hip_batch` with `m=1`.
    ///
    /// # Arguments
    ///
    /// - `matrix_ptr` — device pointer to an n×n row-major array of `u8`
    ///   elements in GF(5) (values 0..4).
    /// - `n` — matrix dimension (n×n); must satisfy `1 <= n <= 63`. This is
    ///   a GPU-specific limit: the sequential Gray walk at n=64 would require
    ///   2^64 ≈ 1.8×10^19 steps (~600 years on gfx1030). The CPU reference
    ///   `permanent_bipedal5_singleword` was narrowed to n ≤ 63 on 2026-05-15
    ///   for CPU/GPU consistency.
    /// - `out_ptr` — device pointer to a single `u64` output that receives
    ///   the permanent value modulo 5 (in `{0, 1, 2, 3, 4}`).
    ///
    /// # Returns
    ///
    /// 0 on success (`hipSuccess`), a non-zero HIP error code otherwise.
    fn permanent_bipedal5_hip(matrix_ptr: *const u8, n: c_int, out_ptr: *mut u64) -> c_int;

    /// Stream-bearing F_5 batch entry point.
    ///
    /// # Safety
    ///
    /// `matrices_ptr` and `out_ptr` must satisfy the same device-allocation
    /// requirements as `permanent_bipedal5_hip_batch`. `stream` must be a live
    /// `hipStream_t` in the active context; it may be null only to select HIP's
    /// default stream. All pointed-to allocations must outlive queued work.
    /// `kernel_start_event` is null for ordinary launches, or a live
    /// timing-enabled `hipEvent_t` from the same context. When non-null, the
    /// wrapper records it immediately before submitting the kernel and returns
    /// that record error without submitting a kernel.
    fn permanent_bipedal5_hip_batch_on_stream(
        matrices_ptr: *const u8,
        n: c_int,
        m: c_int,
        out_ptr: *mut u64,
        stream: *mut c_void,
        kernel_start_event: *mut c_void,
    ) -> c_int;

    /// Initialize the F_7 GPU LUTs by copying host ADD/SUB/MUL LUTs to device memory.
    ///
    /// Entry point in `hip/permanent/permanent_bipedal7.hip`. Copies:
    /// - `host_mul_lut` → `d_MUL_LUT` (__constant__, 64 KiB) via `hipMemcpyToSymbol`.
    /// - `host_add_lut` → `d_ADD_LUT` (__device__, 64 KiB) via `hipMemcpyToSymbol`.
    /// - `host_sub_lut` → `d_SUB_LUT` (__device__, 64 KiB) via `hipMemcpyToSymbol`.
    ///
    /// Idempotent — calling multiple times overwrites with the same data.
    ///
    /// # Arguments
    ///
    /// - `host_add_lut` — host pointer to 65536 bytes (the ADD_LUT from gf2-algebra).
    /// - `host_sub_lut` — host pointer to 65536 bytes (the SUB_LUT from gf2-algebra).
    /// - `host_mul_lut` — host pointer to 65536 bytes (the MUL_LUT from gf2-algebra).
    ///
    /// # Returns
    ///
    /// 0 on success (`hipSuccess`), a non-zero HIP error code otherwise.
    fn permanent_bipedal7_hip_init(
        host_add_lut: *const u8,
        host_sub_lut: *const u8,
        host_mul_lut: *const u8,
    ) -> c_int;

    /// Stream-bearing F_7 batch entry point.
    ///
    /// # Safety
    ///
    /// `matrices_ptr` and `out_ptr` must satisfy the same device-allocation
    /// requirements as `permanent_bipedal7_hip_batch`. `stream` must be a live
    /// `hipStream_t` in the active context; it may be null only to select HIP's
    /// default stream. All pointed-to allocations must outlive queued work.
    /// `kernel_start_event` is null for ordinary launches, or a live
    /// timing-enabled `hipEvent_t` from the same context. When non-null, the
    /// wrapper records it immediately before submitting the kernel and returns
    /// that record error without submitting a kernel.
    fn permanent_bipedal7_hip_batch_on_stream(
        matrices_ptr: *const u8,
        n: c_int,
        m: c_int,
        out_ptr: *mut u64,
        stream: *mut c_void,
        kernel_start_event: *mut c_void,
    ) -> c_int;

    /// Compute the permanent of an n×n matrix over GF(7) on the GPU (single matrix).
    ///
    /// Entry point in `hip/permanent/permanent_bipedal7.hip`. Delegates to
    /// `permanent_bipedal7_hip_batch` with `m=1`.
    ///
    /// # Arguments
    ///
    /// - `matrix_ptr` — device pointer to an n×n row-major array of `u8`
    ///   elements in GF(7) (values 0..6).
    /// - `n` — matrix dimension (n×n); must satisfy `1 <= n <= 63`.
    /// - `out_ptr` — device pointer to a single `u64` output that receives
    ///   the permanent value modulo 7 (in `{0, 1, ..., 6}`).
    ///
    /// # Returns
    ///
    /// 0 on success (`hipSuccess`), a non-zero HIP error code otherwise.
    #[allow(dead_code)] // single-matrix entry retained at the .hip level for symmetry
    // with F_3/F_5; the Rust wrapper compute_permanent_gf7 forwards to the
    // batched _hip_batch variant so the GF7_INIT_RC guard is checked.
    fn permanent_bipedal7_hip(matrix_ptr: *const u8, n: c_int, out_ptr: *mut u64) -> c_int;

    /// Compute the byte-sum checksum of the GPU __constant__ MUL_LUT (criterion-3 test).
    ///
    /// Entry point in `hip/permanent/permanent_bipedal7.hip`. Launches a single
    /// thread that sums all 65536 bytes of `d_MUL_LUT` and stores the `u64`
    /// result to `*out_ptr`.
    ///
    /// # Arguments
    ///
    /// - `out_ptr` — device pointer to a single `u64` that receives the checksum.
    ///
    /// # Returns
    ///
    /// 0 on success (`hipSuccess`), a non-zero HIP error code otherwise.
    fn permanent_bipedal7_hip_lut_checksum(out_ptr: *mut u64) -> c_int;
}

// ---------------------------------------------------------------------------
// F_7 LUT init state
//
// The caller is responsible for invoking `init_permanent_gf7` (with the
// host LUTs from `gf2_algebra::packed::packed7`) before the first call
// to `compute_permanent_gf7_batch`. The init state is memoised in
// `GF7_INIT_RC` (an AtomicI32), so a failed init does not silently
// corrupt later compute calls: instead, `compute_permanent_gf7_batch`
// returns the memoised non-zero rc and refuses to launch.
//
// `gf2-algebra` (which owns the canonical `packed7::*_LUT` byte tables)
// is a dev-dependency of this crate, not a regular dependency: the cycle
// `gf2-algebra -[hip]-> gf2-kernels-hip -> gf2-algebra` would otherwise
// be unresolvable. Hence the explicit caller-driven init pattern below.
// ---------------------------------------------------------------------------

/// Memoised state of the F_7 LUT init.
///
/// Encoded values:
/// - [`GF7_INIT_UNINIT`] (sentinel, default): `init_permanent_gf7` has
///   not been called yet. `compute_permanent_gf7_batch` refuses to launch
///   and returns this sentinel so the caller sees an explicit "not
///   initialised" signal rather than silently computing against
///   uninitialised device memory.
/// - `0` (hipSuccess): init succeeded; the device LUTs are populated and
///   the batch entry point is safe to launch.
/// - any other value: a HIP error code propagated from the last init
///   attempt. `compute_permanent_gf7_batch` returns this so the caller
///   sees the original failure rather than silently computing.
static GF7_INIT_RC: std::sync::atomic::AtomicI32 =
    std::sync::atomic::AtomicI32::new(GF7_INIT_UNINIT);
/// Sentinel "not yet initialised" value. Chosen as `i32::MIN` so it is
/// distinguishable from every plausible HIP error code (which are small
/// non-negative integers, typically `<= 1000`).
const GF7_INIT_UNINIT: i32 = i32::MIN;

/// Compute the F_3 permanent of a single n×n matrix on the GPU.
///
/// Delegates to [`compute_permanent_gf3_batch`] with `m = 1`. This is the
/// single-matrix convenience wrapper; for batch workloads use the batched
/// variant directly.
///
/// # Arguments
///
/// - `matrix_ptr` — device pointer to an `n × n` row-major array of `u8`
///   elements in GF(3) (values `0`, `1`, `2`).
/// - `n` — matrix dimension (`n × n`); must satisfy `1 <= n <= 63`. This is
///   a GPU-specific limit: the sequential Gray walk at n=64 would require
///   2^64 steps (~600 years on gfx1030). The CPU reference
///   `permanent_bipedal3_singleword` supports n=64 via a u128 counter.
/// - `out_ptr` — device pointer to a single `u64` that receives the permanent
///   value modulo 3 (value in `{0, 1, 2}`).
///
/// # Safety
///
/// - `matrix_ptr` must be a valid device allocation of at least `n * n` bytes,
///   containing GF(3) element values (`0`, `1`, `2`).
/// - `out_ptr` must be a valid device allocation of at least 8 bytes.
/// - `n` must satisfy `1 <= n <= 63`.
/// - The HIP runtime must be initialised and a device context must be active.
///
/// # Examples
///
/// ```ignore
/// // Skipped under `cargo test` (requires ROCm + gfx1030 + device memory):
/// # #[cfg(feature = "hip")] {
/// use gf2_kernels_hip::permanent::compute_permanent_gf3;
/// // matrix_ptr / out_ptr are device pointers obtained from
/// // hipMalloc; the caller is responsible for managing them.
/// // SAFETY: see the function-level safety contract.
/// let rc = unsafe { compute_permanent_gf3(matrix_ptr, 8, out_ptr) };
/// assert_eq!(rc, 0, "hipSuccess");
/// # }
/// ```
///
/// # Panics
///
/// Never panics from Rust — all error reporting flows through the `c_int`
/// HIP-status return value.
///
/// # Complexity
///
/// `O(n · 2^n)` GPU work (Ryser Gray-code walk with Bipedal3 column-sum
/// folding). Host overhead is a single kernel launch plus `hipGetLastError`.
pub unsafe fn compute_permanent_gf3(matrix_ptr: *const u8, n: c_int, out_ptr: *mut u64) -> c_int {
    // SAFETY: preconditions forwarded verbatim from the caller (see doc comment).
    unsafe { permanent_bipedal3_hip(matrix_ptr, n, out_ptr) }
}

/// Compute F_3 permanents for a batch of M n×n matrices in a single kernel launch.
///
/// Wraps `permanent_bipedal3_hip_batch` from
/// `hip/permanent/permanent_bipedal3.hip`. Launches one HIP block per matrix
/// (grid = M, block = 1); only thread 0 per block executes the Gray walk.
/// GPU throughput derives from many blocks running simultaneously.
///
/// # Arguments
///
/// - `matrices_ptr` — device pointer to `m` consecutive n×n row-major arrays of
///   `u8` elements in GF(3) (values `0`, `1`, `2`). Matrix `i` starts at
///   `matrices_ptr + i * n * n`.
/// - `n` — matrix dimension (`n × n`); must satisfy `1 <= n <= 63`. This is
///   a GPU-specific limit: the sequential Gray walk at n=64 would require
///   2^64 steps (~600 years on gfx1030). The CPU reference
///   `permanent_bipedal3_singleword` supports n=64 via a u128 counter.
/// - `m` — batch size (number of matrices); must be `>= 1`.
/// - `out_ptr` — device pointer to `m` consecutive `u64` outputs. On success,
///   `out_ptr[i]` receives the permanent of matrix `i` modulo 3 (value in
///   `{0, 1, 2}`).
///
/// # Safety
///
/// - `matrices_ptr` must be a valid device allocation of at least `m * n * n` bytes.
/// - Each element must be a valid GF(3) value (`0`, `1`, or `2`).
/// - `out_ptr` must be a valid device allocation of at least `m * 8` bytes.
/// - `n` must satisfy `1 <= n <= 63`.
/// - `m` must be `>= 1`.
/// - The HIP runtime must be initialised and a device context must be active.
///
/// # Examples
///
/// ```ignore
/// # #[cfg(feature = "hip")] {
/// use gf2_kernels_hip::permanent::compute_permanent_gf3_batch;
/// // matrices_ptr is a device pointer to 4 * 8 * 8 = 256 bytes.
/// // out_ptr is a device pointer to 4 * 8 = 32 bytes.
/// // SAFETY: see the function-level safety contract.
/// let rc = unsafe { compute_permanent_gf3_batch(matrices_ptr, 8, 4, out_ptr) };
/// assert_eq!(rc, 0, "hipSuccess");
/// # }
/// ```
///
/// # Panics
///
/// Never panics from Rust — all error reporting flows through the `c_int`
/// HIP-status return value.
///
/// # Complexity
///
/// `O(n · 2^n)` GPU work per matrix. With `m` matrices and enough GPU
/// occupancy, the wall-clock cost is `O(n · 2^n)` total (all blocks overlap).
pub unsafe fn compute_permanent_gf3_batch(
    matrices_ptr: *const u8,
    n: c_int,
    m: c_int,
    out_ptr: *mut u64,
) -> c_int {
    // SAFETY: preconditions forwarded verbatim from the caller (see doc comment).
    // A null stream preserves the original HIP default-stream behaviour.
    unsafe {
        compute_permanent_gf3_batch_on_stream(matrices_ptr, n, m, out_ptr, std::ptr::null_mut())
    }
}

/// Computes an F_3 permanent batch on a caller-supplied HIP stream.
///
/// This is the asynchronous stream-bearing counterpart to
/// [`compute_permanent_gf3_batch`]. It only enqueues the kernel; the caller
/// must keep the allocations alive and synchronize or otherwise await the
/// stream before reading `out_ptr`.
///
/// # Safety
///
/// - `matrices_ptr` and `out_ptr` must meet the allocation, element-value,
///   `n`, and `m` requirements of [`compute_permanent_gf3_batch`].
/// - `stream` must be a live `hipStream_t` in the active HIP context (or null
///   for HIP's default stream) and all allocations must outlive queued work.
pub unsafe fn compute_permanent_gf3_batch_on_stream(
    matrices_ptr: *const u8,
    n: c_int,
    m: c_int,
    out_ptr: *mut u64,
    stream: *mut c_void,
) -> c_int {
    // SAFETY: all device-pointer, dimension, and stream-lifetime preconditions
    // are forwarded verbatim from this unsafe function's contract. A null
    // timing event preserves ordinary stream-launch behavior.
    unsafe {
        compute_permanent_gf3_batch_on_stream_with_kernel_start_event(
            matrices_ptr,
            n,
            m,
            out_ptr,
            stream,
            std::ptr::null_mut(),
        )
    }
}

/// Raw F_3 stream launch that optionally records `kernel_start_event` in the
/// C++ wrapper immediately before submitting the kernel.
///
/// # Safety
///
/// The device pointers and stream must satisfy
/// [`compute_permanent_gf3_batch_on_stream`]'s contract. When non-null,
/// `kernel_start_event` must be a live timing-enabled HIP event in the same
/// context as `stream` and remain alive through this call.
unsafe fn compute_permanent_gf3_batch_on_stream_with_kernel_start_event(
    matrices_ptr: *const u8,
    n: c_int,
    m: c_int,
    out_ptr: *mut u64,
    stream: *mut c_void,
    kernel_start_event: *mut c_void,
) -> c_int {
    // SAFETY: the caller establishes the device-pointer and stream lifetimes;
    // a non-null marker is a live timing event in the same HIP context. The
    // C++ wrapper records that marker before it submits the kernel.
    unsafe {
        permanent_bipedal3_hip_batch_on_stream(
            matrices_ptr,
            n,
            m,
            out_ptr,
            stream,
            kernel_start_event,
        )
    }
}

/// Compute the F_5 permanent of a single n×n matrix on the GPU.
///
/// Delegates to [`compute_permanent_gf5_batch`] with `m = 1`. This is the
/// single-matrix convenience wrapper; for batch workloads use the batched
/// variant directly.
///
/// # Arguments
///
/// - `matrix_ptr` — device pointer to an `n × n` row-major array of `u8`
///   elements in GF(5) (values `0`, `1`, `2`, `3`, `4`).
/// - `n` — matrix dimension (`n × n`); must satisfy `1 <= n <= 63`. This is
///   a GPU-specific limit: the sequential Gray walk at n=64 would require
///   2^64 steps (~600 years on gfx1030). The CPU reference
///   `permanent_bipedal5_singleword` was narrowed to n ≤ 63 on 2026-05-15
///   for CPU/GPU consistency.
/// - `out_ptr` — device pointer to a single `u64` that receives the permanent
///   value modulo 5 (value in `{0, 1, 2, 3, 4}`).
///
/// # Safety
///
/// - `matrix_ptr` must be a valid device allocation of at least `n * n` bytes,
///   containing GF(5) element values (`0..=4`).
/// - `out_ptr` must be a valid device allocation of at least 8 bytes.
/// - `n` must satisfy `1 <= n <= 63`.
/// - The HIP runtime must be initialised and a device context must be active.
///
/// # Examples
///
/// ```ignore
/// // Skipped under `cargo test` (requires ROCm + gfx1030 + device memory):
/// # #[cfg(feature = "hip")] {
/// use gf2_kernels_hip::permanent::compute_permanent_gf5;
/// // matrix_ptr / out_ptr are device pointers obtained from
/// // hipMalloc; the caller is responsible for managing them.
/// // SAFETY: see the function-level safety contract.
/// let rc = unsafe { compute_permanent_gf5(matrix_ptr, 8, out_ptr) };
/// assert_eq!(rc, 0, "hipSuccess");
/// # }
/// ```
///
/// # Panics
///
/// Never panics from Rust — all error reporting flows through the `c_int`
/// HIP-status return value.
///
/// # Complexity
///
/// `O(n · 2^n)` GPU work (Ryser Gray-code walk with byte-arithmetic F_5 column-sum
/// folding). Host overhead is a single kernel launch plus `hipGetLastError`.
pub unsafe fn compute_permanent_gf5(matrix_ptr: *const u8, n: c_int, out_ptr: *mut u64) -> c_int {
    // SAFETY: preconditions forwarded verbatim from the caller (see doc comment).
    unsafe { permanent_bipedal5_hip(matrix_ptr, n, out_ptr) }
}

/// Compute F_5 permanents for a batch of M n×n matrices in a single kernel launch.
///
/// Wraps `permanent_bipedal5_hip_batch` from
/// `hip/permanent/permanent_bipedal5.hip`. Launches one HIP block per matrix
/// (grid = M, block = 1); only thread 0 per block executes the Gray walk.
/// GPU throughput derives from many blocks running simultaneously.
///
/// # Arguments
///
/// - `matrices_ptr` — device pointer to `m` consecutive n×n row-major arrays of
///   `u8` elements in GF(5) (values `0`, `1`, `2`, `3`, `4`). Matrix `i` starts
///   at `matrices_ptr + i * n * n`.
/// - `n` — matrix dimension (`n × n`); must satisfy `1 <= n <= 63`. This is
///   a GPU-specific limit: the sequential Gray walk at n=64 would require
///   2^64 steps (~600 years on gfx1030). The CPU reference
///   `permanent_bipedal5_singleword` was narrowed to n ≤ 63 on 2026-05-15
///   for CPU/GPU consistency.
/// - `m` — batch size (number of matrices); must be `>= 1`.
/// - `out_ptr` — device pointer to `m` consecutive `u64` outputs. On success,
///   `out_ptr[i]` receives the permanent of matrix `i` modulo 5 (value in
///   `{0, 1, 2, 3, 4}`).
///
/// # Safety
///
/// - `matrices_ptr` must be a valid device allocation of at least `m * n * n` bytes.
/// - Each element must be a valid GF(5) value (`0..=4`).
/// - `out_ptr` must be a valid device allocation of at least `m * 8` bytes.
/// - `n` must satisfy `1 <= n <= 63`.
/// - `m` must be `>= 1`.
/// - The HIP runtime must be initialised and a device context must be active.
///
/// # Examples
///
/// ```ignore
/// # #[cfg(feature = "hip")] {
/// use gf2_kernels_hip::permanent::compute_permanent_gf5_batch;
/// // matrices_ptr is a device pointer to 4 * 8 * 8 = 256 bytes.
/// // out_ptr is a device pointer to 4 * 8 = 32 bytes.
/// // SAFETY: see the function-level safety contract.
/// let rc = unsafe { compute_permanent_gf5_batch(matrices_ptr, 8, 4, out_ptr) };
/// assert_eq!(rc, 0, "hipSuccess");
/// # }
/// ```
///
/// # Panics
///
/// Never panics from Rust — all error reporting flows through the `c_int`
/// HIP-status return value.
///
/// # Complexity
///
/// `O(n · 2^n)` GPU work per matrix. With `m` matrices and enough GPU
/// occupancy, the wall-clock cost is `O(n · 2^n)` total (all blocks overlap).
pub unsafe fn compute_permanent_gf5_batch(
    matrices_ptr: *const u8,
    n: c_int,
    m: c_int,
    out_ptr: *mut u64,
) -> c_int {
    // SAFETY: preconditions forwarded verbatim from the caller (see doc comment).
    // A null stream preserves the original HIP default-stream behaviour.
    unsafe {
        compute_permanent_gf5_batch_on_stream(matrices_ptr, n, m, out_ptr, std::ptr::null_mut())
    }
}

/// Computes an F_5 permanent batch on a caller-supplied HIP stream.
///
/// This is the asynchronous stream-bearing counterpart to
/// [`compute_permanent_gf5_batch`]. It only enqueues the kernel; the caller
/// must keep the allocations alive and synchronize or otherwise await the
/// stream before reading `out_ptr`.
///
/// # Safety
///
/// - `matrices_ptr` and `out_ptr` must meet the allocation, element-value,
///   `n`, and `m` requirements of [`compute_permanent_gf5_batch`].
/// - `stream` must be a live `hipStream_t` in the active HIP context (or null
///   for HIP's default stream) and all allocations must outlive queued work.
pub unsafe fn compute_permanent_gf5_batch_on_stream(
    matrices_ptr: *const u8,
    n: c_int,
    m: c_int,
    out_ptr: *mut u64,
    stream: *mut c_void,
) -> c_int {
    // SAFETY: all device-pointer, dimension, and stream-lifetime preconditions
    // are forwarded verbatim from this unsafe function's contract. A null
    // timing event preserves ordinary stream-launch behavior.
    unsafe {
        compute_permanent_gf5_batch_on_stream_with_kernel_start_event(
            matrices_ptr,
            n,
            m,
            out_ptr,
            stream,
            std::ptr::null_mut(),
        )
    }
}

/// Raw F_5 stream launch that optionally records `kernel_start_event` in the
/// C++ wrapper immediately before submitting the kernel.
///
/// # Safety
///
/// The device pointers and stream must satisfy
/// [`compute_permanent_gf5_batch_on_stream`]'s contract. When non-null,
/// `kernel_start_event` must be a live timing-enabled HIP event in the same
/// context as `stream` and remain alive through this call.
unsafe fn compute_permanent_gf5_batch_on_stream_with_kernel_start_event(
    matrices_ptr: *const u8,
    n: c_int,
    m: c_int,
    out_ptr: *mut u64,
    stream: *mut c_void,
    kernel_start_event: *mut c_void,
) -> c_int {
    // SAFETY: the caller establishes the device-pointer and stream lifetimes;
    // a non-null marker is a live timing event in the same HIP context. The
    // C++ wrapper records that marker before it submits the kernel.
    unsafe {
        permanent_bipedal5_hip_batch_on_stream(
            matrices_ptr,
            n,
            m,
            out_ptr,
            stream,
            kernel_start_event,
        )
    }
}

/// Initialize the F_7 GPU LUT tables (ADD, SUB, MUL) from the host static consts.
///
/// Copies `gf2_algebra::packed::packed7::{ADD_LUT, SUB_LUT, MUL_LUT}` to the
/// device via `permanent_bipedal7_hip_init`. The MUL_LUT lands in `__constant__`
/// memory (64 KiB, hardware-cached on gfx1030); ADD_LUT and SUB_LUT land in
/// `__device__` global memory (64 KiB each, L1/L2 cached).
///
/// **Must be called explicitly by the caller before the first invocation of
/// [`compute_permanent_gf7`] or [`compute_permanent_gf7_batch`].** The
/// compute entry points memoise this function's return code; if it has not
/// been called (or returned a non-zero rc), the compute entry points refuse
/// to launch and propagate the memoised rc. This is intentional:
/// `gf2-kernels-hip` cannot reach `gf2_algebra::packed::packed7::*_LUT` from
/// its `lib` (`gf2-algebra` is a *dev-dependency* of this crate to avoid a
/// circular workspace dep with `gf2-algebra`'s `hip` feature), so the LUT
/// pointers are caller-supplied.
///
/// Idempotent — safe to call multiple times; each call overwrites the device
/// copy with the same data.
///
/// # Arguments
///
/// - `host_add_lut` — host pointer to 65 536 bytes (the F_7 ADD_LUT).
/// - `host_sub_lut` — host pointer to 65 536 bytes (the F_7 SUB_LUT).
/// - `host_mul_lut` — host pointer to 65 536 bytes (the F_7 MUL_LUT).
///
/// # Safety
///
/// - All three pointers must be valid host pointers to exactly 65 536 bytes
///   of F_7 LUT data (canonical: values in `{0..6}` per nibble pair).
/// - The HIP runtime must be initialised and a device context must be active.
///
/// # Returns
///
/// 0 on success (`hipSuccess`), a non-zero HIP error code otherwise.
///
/// # Examples
///
/// ```ignore
/// // Skipped under `cargo test` (requires ROCm + gfx1030):
/// # #[cfg(feature = "hip")] {
/// use gf2_algebra::packed::packed7::{ADD_LUT, SUB_LUT, MUL_LUT};
/// use gf2_kernels_hip::permanent::init_permanent_gf7;
/// // SAFETY: ADD_LUT, SUB_LUT, MUL_LUT are 'static [u8; 65536].
/// let rc = unsafe { init_permanent_gf7(
///     ADD_LUT.as_ptr(), SUB_LUT.as_ptr(), MUL_LUT.as_ptr()) };
/// assert_eq!(rc, 0, "hipSuccess");
/// # }
/// ```
///
/// # Panics
///
/// Never panics from Rust — all error reporting flows through the `c_int`
/// HIP-status return value.
///
/// # Complexity
///
/// Three `hipMemcpyToSymbol` calls of 64 KiB each — `O(1)` host work.
pub unsafe fn init_permanent_gf7(
    host_add_lut: *const u8,
    host_sub_lut: *const u8,
    host_mul_lut: *const u8,
) -> c_int {
    // SAFETY: preconditions forwarded verbatim from the caller (see doc comment).
    let rc = unsafe { permanent_bipedal7_hip_init(host_add_lut, host_sub_lut, host_mul_lut) };
    memoise_init_outcome(&GF7_INIT_RC, rc);
    rc
}

/// CAS-loop helper that records `rc` into `state` per the init contract:
/// "if any concurrent or prior init succeeded (`rc == 0`), the memoised
/// state must end at 0; failed inits may be overwritten by later inits but
/// never overwrite a successful one." Extracted from
/// [`init_permanent_gf7`] so the state-machine semantics can be exercised
/// by unit tests without a live HIP/ROCm context.
///
/// # Arguments
///
/// * `state` — the atomic the init outcome is memoised in (in production,
///   [`GF7_INIT_RC`]).
/// * `rc` — the return code from this call's init attempt; `0` means
///   success, any other value is a HIP error code.
///
/// # Concurrency
///
/// Multi-thread-safe via [`AtomicI32::compare_exchange`]. The prior
/// non-atomic load-then-store version of this code was racy — two parallel
/// callers, one succeeding and one failing, could both see `prev != 0`
/// (the sentinel) and store in any order, so a failed init could clobber
/// a concurrent successful init. The CAS loop here is the correct fix:
/// at every retry the exchange only succeeds if the observed `prev` is
/// still current, so we never silently overwrite a freshly-stored success.
///
/// # Complexity
///
/// Amortised `O(1)` per call. Under contention, the loop retries at most
/// once per concurrent overwriter; in practice the loop terminates in 1
/// or 2 iterations.
fn memoise_init_outcome(state: &std::sync::atomic::AtomicI32, rc: c_int) {
    use std::sync::atomic::Ordering::SeqCst;
    loop {
        let prev = state.load(SeqCst);
        if prev == 0 {
            // Some init (possibly a concurrent one) has already recorded
            // success — the LUTs are populated on the device. Even if
            // this call's `rc` is non-zero (transient init failure on a
            // separate context, say), the device state remains valid
            // for compute. Leave the success state in place.
            return;
        }
        // prev is either the uninitialised sentinel or a prior failed rc;
        // overwrite atomically. If a racing thread mutated the state
        // between our load and this CAS, the exchange fails and we retry
        // — on the retry we'll either see success (and break) or another
        // overwrite-eligible state.
        if state.compare_exchange(prev, rc, SeqCst, SeqCst).is_ok() {
            return;
        }
    }
}

/// Compute the F_7 permanent of a single n×n matrix on the GPU.
///
/// Delegates to [`compute_permanent_gf7_batch`] with `m = 1`. The caller must
/// have invoked [`init_permanent_gf7`] (with the host LUTs from
/// `gf2_algebra::packed::packed7`) before the first call to this function.
///
/// # Arguments
///
/// - `matrix_ptr` — device pointer to an `n × n` row-major array of `u8`
///   elements in GF(7) (values `0..=6`).
/// - `n` — matrix dimension (`n × n`); must satisfy `1 <= n <= 63`.
/// - `out_ptr` — device pointer to a single `u64` that receives the permanent
///   value modulo 7 (value in `{0, 1, 2, 3, 4, 5, 6}`).
///
/// # Safety
///
/// - `matrix_ptr` must be a valid device allocation of at least `n * n` bytes,
///   containing GF(7) element values (`0..=6`).
/// - `out_ptr` must be a valid device allocation of at least 8 bytes.
/// - `n` must satisfy `1 <= n <= 63`.
/// - The HIP runtime must be initialised and a device context must be active.
///
/// # Examples
///
/// ```ignore
/// // Skipped under `cargo test` (requires ROCm + gfx1030 + device memory):
/// # #[cfg(feature = "hip")] {
/// use gf2_kernels_hip::permanent::compute_permanent_gf7;
/// // matrix_ptr / out_ptr are device pointers obtained from
/// // hipMalloc; the caller is responsible for managing them.
/// // SAFETY: see the function-level safety contract.
/// let rc = unsafe { compute_permanent_gf7(matrix_ptr, 8, out_ptr) };
/// assert_eq!(rc, 0, "hipSuccess");
/// # }
/// ```
///
/// # Panics
///
/// Never panics from Rust — all error reporting flows through the `c_int`
/// HIP-status return value.
///
/// # Complexity
///
/// `O(n · 2^n)` GPU work (Ryser Gray-code walk with LUT-based F_7 column-sum
/// folding). Host overhead is one memoised-state check (`GF7_INIT_RC` atomic
/// load) plus a single kernel launch. The caller-supplied [`init_permanent_gf7`]
/// runs once per process (idempotent at the FFI level).
pub unsafe fn compute_permanent_gf7(matrix_ptr: *const u8, n: c_int, out_ptr: *mut u64) -> c_int {
    // SAFETY: preconditions forwarded verbatim from the caller (see doc comment).
    unsafe { compute_permanent_gf7_batch(matrix_ptr, n, 1, out_ptr) }
}

/// Compute F_7 permanents for a batch of M n×n matrices in a single kernel launch.
///
/// Wraps `permanent_bipedal7_hip_batch` from
/// `hip/permanent/permanent_bipedal7.hip`. The caller must have invoked
/// [`init_permanent_gf7`] (with the host LUTs from `gf2_algebra::packed::packed7`)
/// before this function — `gf2-algebra` is a dev-dependency of this crate, so
/// the LUT bytes cannot be reached from `lib` here. If the memoised init state
/// is the uninitialised sentinel (`i32::MIN`) or a non-zero error code, this
/// function refuses to launch and propagates the memoised rc instead of
/// silently computing against uninitialised device memory.
///
/// Launches one HIP block per matrix (grid = M, block = 1); only thread 0 per
/// block executes the Gray walk. GPU throughput derives from many blocks running
/// simultaneously.
///
/// # LUT placement
///
/// - `d_MUL_LUT` — `__constant__` memory (64 KiB, hardware-cached on gfx1030).
/// - `d_ADD_LUT`, `d_SUB_LUT` — `__device__` global memory (64 KiB each, L1/L2).
///
/// Option (c) from the issue: MUL_LUT in `__constant__` (criterion 3 names
/// "the LUT" — the MUL_LUT used in fold_mul), ADD/SUB in `__device__` global.
/// Total 192 KiB on device; only the 64 KiB MUL_LUT is hardware-cached.
///
/// # Arguments
///
/// - `matrices_ptr` — device pointer to `m` consecutive n×n row-major arrays of
///   `u8` elements in GF(7) (values `0..=6`). Matrix `i` starts at
///   `matrices_ptr + i * n * n`.
/// - `n` — matrix dimension (`n × n`); must satisfy `1 <= n <= 63`. GPU
///   sequential Gray walk at n=64 would take ~600 years on gfx1030.
/// - `m` — batch size (number of matrices); must be `>= 1`.
/// - `out_ptr` — device pointer to `m` consecutive `u64` outputs. On success,
///   `out_ptr[i]` receives the permanent of matrix `i` modulo 7 (value in
///   `{0, 1, 2, 3, 4, 5, 6}`).
///
/// # Safety
///
/// - `matrices_ptr` must be a valid device allocation of at least `m * n * n` bytes.
/// - Each element must be a valid GF(7) value (`0..=6`).
/// - `out_ptr` must be a valid device allocation of at least `m * 8` bytes.
/// - `n` must satisfy `1 <= n <= 63`.
/// - `m` must be `>= 1`.
/// - The HIP runtime must be initialised and a device context must be active.
///
/// # Examples
///
/// ```ignore
/// # #[cfg(feature = "hip")] {
/// use gf2_kernels_hip::permanent::compute_permanent_gf7_batch;
/// // matrices_ptr is a device pointer to 4 * 8 * 8 = 256 bytes.
/// // out_ptr is a device pointer to 4 * 8 = 32 bytes.
/// // SAFETY: see the function-level safety contract.
/// let rc = unsafe { compute_permanent_gf7_batch(matrices_ptr, 8, 4, out_ptr) };
/// assert_eq!(rc, 0, "hipSuccess");
/// # }
/// ```
///
/// # Panics
///
/// Never panics from Rust — all error reporting flows through the `c_int`
/// HIP-status return value.
///
/// # Complexity
///
/// `O(n · 2^n)` GPU work per matrix. With `m` matrices and enough GPU
/// occupancy, the wall-clock cost is `O(n · 2^n)` total (all blocks overlap).
pub unsafe fn compute_permanent_gf7_batch(
    matrices_ptr: *const u8,
    n: c_int,
    m: c_int,
    out_ptr: *mut u64,
) -> c_int {
    // SAFETY: preconditions forwarded verbatim from the caller (see doc comment).
    // A null stream preserves the original HIP default-stream behaviour.
    unsafe {
        compute_permanent_gf7_batch_on_stream(matrices_ptr, n, m, out_ptr, std::ptr::null_mut())
    }
}

/// Computes an F_7 permanent batch on a caller-supplied HIP stream.
///
/// This is the asynchronous stream-bearing counterpart to
/// [`compute_permanent_gf7_batch`]. The F_7 LUTs must already have been
/// initialized with [`init_permanent_gf7`]. It only enqueues the kernel; the
/// caller must keep the allocations alive and synchronize or otherwise await
/// the stream before reading `out_ptr`.
///
/// # Safety
///
/// - `matrices_ptr` and `out_ptr` must meet the allocation, element-value,
///   `n`, and `m` requirements of [`compute_permanent_gf7_batch`].
/// - `stream` must be a live `hipStream_t` in the active HIP context (or null
///   for HIP's default stream) and all allocations must outlive queued work.
pub unsafe fn compute_permanent_gf7_batch_on_stream(
    matrices_ptr: *const u8,
    n: c_int,
    m: c_int,
    out_ptr: *mut u64,
    stream: *mut c_void,
) -> c_int {
    // The caller is responsible for having called `init_permanent_gf7`
    // (or its underlying FFI `permanent_bipedal7_hip_init`) at least once
    // before invoking this function. If the LUTs are not populated, the
    // device kernel will read zeros from the __constant__/__device__ LUT
    // symbols and silently produce wrong permanent values.
    //
    // Rationale for not auto-initialising here: `gf2-kernels-hip` is
    // algebra-agnostic at the library level — `gf2-algebra` (which owns
    // the canonical `packed7::{ADD_LUT, SUB_LUT, MUL_LUT}` byte tables)
    // is a *dev-dependency* of this crate to avoid a circular workspace
    // dependency (`gf2-algebra` itself optionally pulls in
    // `gf2-kernels-hip` via its `hip` feature). The LUTs therefore
    // cannot be referenced from this crate's `lib`. Callers that have
    // access to `gf2-algebra` (e.g. the integration tests in this crate
    // and the host-side dispatcher landing in `2fbbdfa5`) provide the
    // LUT pointers explicitly via `init_permanent_gf7` before the first
    // batch launch.
    //
    // GF7_INIT_RC is consulted on every call: if a prior init attempt
    // failed (returned non-zero), this function refuses to launch and
    // propagates the original init error code rather than silently
    // computing against uninitialised device memory.
    // SAFETY: all device-pointer, dimension, stream-lifetime, and initialized
    // LUT preconditions are forwarded from this unsafe function's contract. A
    // null timing event preserves ordinary stream-launch behavior.
    unsafe {
        compute_permanent_gf7_batch_on_stream_with_kernel_start_event(
            matrices_ptr,
            n,
            m,
            out_ptr,
            stream,
            std::ptr::null_mut(),
        )
    }
}

/// Raw F_7 stream launch that optionally records `kernel_start_event` in the
/// C++ wrapper immediately before submitting the kernel.
///
/// # Safety
///
/// The device pointers, initialized F_7 LUTs, and stream must satisfy
/// [`compute_permanent_gf7_batch_on_stream`]'s contract. When non-null,
/// `kernel_start_event` must be a live timing-enabled HIP event in the same
/// context as `stream` and remain alive through this call.
unsafe fn compute_permanent_gf7_batch_on_stream_with_kernel_start_event(
    matrices_ptr: *const u8,
    n: c_int,
    m: c_int,
    out_ptr: *mut u64,
    stream: *mut c_void,
    kernel_start_event: *mut c_void,
) -> c_int {
    let init_rc = GF7_INIT_RC.load(std::sync::atomic::Ordering::SeqCst);
    if init_rc != 0 {
        return init_rc;
    }

    // SAFETY: the caller establishes the device-pointer, stream, and
    // initialized-LUT preconditions; a non-null marker is a live timing event
    // in the same HIP context. The C++ wrapper records it before kernel launch.
    unsafe {
        permanent_bipedal7_hip_batch_on_stream(
            matrices_ptr,
            n,
            m,
            out_ptr,
            stream,
            kernel_start_event,
        )
    }
}

/// Compute the byte-sum checksum of the GPU __constant__ MUL_LUT.
///
/// Launches `permanent_bipedal7_lut_checksum_kernel` (a single-thread kernel)
/// that sums all 65 536 bytes of `d_MUL_LUT` and writes the `u64` result to
/// `*out_ptr`. Used by the criterion-3 test
/// `test_permanent_bipedal7_constant_lut_checksum_matches_host` to verify
/// that the device copy of MUL_LUT is byte-identical to the host static const.
///
/// # Arguments
///
/// - `out_ptr` — device pointer to a single `u64` that receives the checksum.
///
/// # Safety
///
/// - `out_ptr` must be a valid device allocation of at least 8 bytes.
/// - The HIP runtime must be initialised and a device context must be active.
/// - `init_permanent_gf7` (or `compute_permanent_gf7_batch`) must have been
///   called beforehand so that `d_MUL_LUT` is populated.
///
/// # Returns
///
/// 0 on success (`hipSuccess`), a non-zero HIP error code otherwise.
///
/// # Examples
///
/// ```ignore
/// // Skipped under `cargo test` (requires ROCm + gfx1030 + device memory):
/// # #[cfg(feature = "hip")] {
/// use gf2_kernels_hip::permanent::compute_lut_checksum_gpu;
/// // out_ptr is a device pointer obtained from hipMalloc; the caller is
/// // responsible for managing it. init_permanent_gf7 must have been
/// // called first so d_MUL_LUT is populated.
/// // SAFETY: see the function-level safety contract.
/// let rc = unsafe { compute_lut_checksum_gpu(out_ptr) };
/// assert_eq!(rc, 0, "hipSuccess");
/// # }
/// ```
///
/// # Panics
///
/// Never panics from Rust — all error reporting flows through the `c_int`
/// HIP-status return value.
///
/// # Complexity
///
/// Single kernel launch; the kernel does 65 536 sequential byte reads — `O(1)`
/// from the host perspective.
pub unsafe fn compute_lut_checksum_gpu(out_ptr: *mut u64) -> c_int {
    // SAFETY: preconditions forwarded verbatim from the caller (see doc comment).
    unsafe { permanent_bipedal7_hip_lut_checksum(out_ptr) }
}

/// Safe wrapper around [`init_permanent_gf7`] that accepts typed references
/// instead of raw pointers, allowing the call to be made from safe Rust code.
///
/// Identical in semantics to [`init_permanent_gf7`] but takes
/// `&[u8; 65536]` references instead of raw pointers; the compiler proves
/// they are valid host pointers of the required length.
///
/// # Arguments
///
/// - `add_lut` — reference to the F_7 ADD_LUT (65 536 bytes).
/// - `sub_lut` — reference to the F_7 SUB_LUT (65 536 bytes).
/// - `mul_lut` — reference to the F_7 MUL_LUT (65 536 bytes).
///
/// # Returns
///
/// 0 on success (`hipSuccess`), non-zero HIP error code otherwise.
///
/// # Panics
///
/// Never panics from Rust — all error reporting flows through the `i32`
/// return value.
///
/// # Complexity
///
/// Three `hipMemcpyToSymbol` calls of 64 KiB each — `O(1)` host work.
///
/// # Examples
///
/// ```ignore
/// # #[cfg(feature = "hip")] {
/// use gf2_kernels_hip::permanent::init_permanent_gf7_from_slices;
/// let rc = init_permanent_gf7_from_slices(&add_arr, &sub_arr, &mul_arr);
/// assert_eq!(rc, 0, "hipSuccess");
/// # }
/// ```
pub fn init_permanent_gf7_from_slices(
    add_lut: &[u8; 65536],
    sub_lut: &[u8; 65536],
    mul_lut: &[u8; 65536],
) -> i32 {
    // SAFETY: add_lut, sub_lut, mul_lut are valid host references of exactly
    // 65536 bytes each. The `as_ptr()` calls return non-null pointers into
    // the referenced data. The HIP runtime must be initialised (a device
    // context must be active).
    let rc = unsafe {
        permanent_bipedal7_hip_init(add_lut.as_ptr(), sub_lut.as_ptr(), mul_lut.as_ptr())
    };
    memoise_init_outcome(&GF7_INIT_RC, rc);
    rc
}

// ---------------------------------------------------------------------------
// Safe host-dispatch wrappers
//
// The unsafe FFI surface above requires device pointers (obtained from
// hipMalloc) and must be called within `unsafe` blocks. The three safe
// wrappers below hide all of that behind the `DecoderDeviceBuffer` RAII
// helper from `crate` (lib.rs) — a byte-oriented adapter over the canonical
// `host::DeviceBuffer<u8>` — and the `check_hip` panic-on-error helper.
//
// `gf2-algebra::gpu` calls these from its `#![deny(unsafe_code)]`
// environment, so they must be entirely safe on the Rust side. Any HIP
// error surfaces as a panic (consistent with the CPU permanent entry points
// that also panic on bad arguments).
//
// Each wrapper:
//   1. Validates preconditions (n, m, slice length).
//   2. Allocates device memory for the input matrix byte buffer and the
//      u64 output array.
//   3. Copies inputs H2D.
//   4. Calls the corresponding `compute_permanent_gfX_batch` kernel launch.
//   5. Calls `hipDeviceSynchronize`.
//   6. Copies outputs D2H.
//   7. Returns the output as `Vec<u64>`. Device memory is freed by `Drop`
//      on the `DecoderDeviceBuffer` RAII wrappers.
// ---------------------------------------------------------------------------

/// Prime-specific permanent kernel selected by an instrumented dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermanentField {
    /// The packed Bipedal3 F_3 kernel.
    F3,
    /// The direct-byte F_5 kernel.
    F5,
    /// The LUT-based F_7 kernel. Its LUTs must be initialized first.
    F7,
}

/// Device- and host-clock durations from one permanent dispatch.
///
/// `h2d`, `kernel`, and `d2h` are device-event spans in execution order on
/// one HIP stream. A missing optional phase means the boundary did not submit
/// that phase; it is never encoded as a zero duration. `host_submission` is
/// measured with [`Instant`] around the one host submission-wrapper call only,
/// while `device_submission_to_kernel` is an independent device-event span
/// from the pre-submit marker to the kernel-start event that wrapper records
/// immediately before `hipLaunchKernelGGL`. The two clocks are reported
/// separately and are never subtracted from one another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PermanentPhaseTimings {
    /// Device-clock host-to-device transfer duration, if one was submitted.
    pub h2d: Option<Duration>,
    /// Device-clock kernel duration, if a kernel was submitted.
    pub kernel: Option<Duration>,
    /// Device-clock device-to-host transfer duration, if one was submitted.
    pub d2h: Option<Duration>,
    /// Host-clock duration of the wrapper call that records the kernel-start
    /// event and submits `hipLaunchKernelGGL`.
    pub host_submission: Duration,
    /// Device-clock duration from the pre-submit stream marker to the kernel
    /// start marker. It is deliberately distinct from `host_submission`.
    pub device_submission_to_kernel: Option<Duration>,
}

/// An event-instrumented permanent kernel launch on a caller-owned stream.
///
/// The boundary records its markers on that stream and does not call
/// `hipDeviceSynchronize`. Poll [`is_complete`](Self::is_complete), synchronize
/// the caller's stream, then call [`phase_timings`](Self::phase_timings) to
/// obtain a kernel-only timing. Its H2D and D2H phase fields are `None`, which
/// explicitly records that this low-level boundary did not submit copies.
pub struct InstrumentedPermanentLaunch {
    kernel: HipEventSpan,
    submission_marker: HipEvent,
    host_submission: Duration,
}

impl InstrumentedPermanentLaunch {
    /// Returns whether the kernel stop event has completed without blocking.
    pub fn is_complete(&self) -> Result<bool, HipError> {
        self.kernel.is_complete()
    }

    /// Returns kernel and launch-overhead timing after completion.
    ///
    /// # Errors
    ///
    /// Returns `hipErrorNotReady` if the kernel stop event is incomplete. This
    /// boundary does not report partial device durations.
    pub fn phase_timings(&self) -> Result<PermanentPhaseTimings, HipError> {
        let kernel = self.kernel.elapsed()?;
        let device_submission_to_kernel =
            self.kernel.elapsed_before_start(&self.submission_marker)?;
        Ok(PermanentPhaseTimings {
            h2d: None,
            kernel: Some(kernel),
            d2h: None,
            host_submission: self.host_submission,
            device_submission_to_kernel: Some(device_submission_to_kernel),
        })
    }
}

/// Enqueues an event-instrumented permanent batch kernel on `stream`.
///
/// The returned boundary owns only its HIP event markers. It returns after the
/// launch is enqueued and never issues a device-wide synchronization. Callers
/// retain ownership of the device allocations and must keep them alive until
/// the stop event completes.
///
/// # Safety
///
/// `matrices_ptr` and `out_ptr` must be valid device allocations of the
/// required lengths for `field`, with values in that field; `n` must be in
/// `1..=63`, `m` must be nonzero, and both allocations must remain valid until
/// the supplied stream has completed the queued kernel. F_7 also requires the
/// canonical LUTs to have been initialized through [`init_permanent_gf7`].
pub unsafe fn launch_permanent_batch_instrumented_on_stream(
    field: PermanentField,
    matrices_ptr: *const u8,
    n: c_int,
    m: c_int,
    out_ptr: *mut u64,
    stream: &HipStream,
) -> Result<InstrumentedPermanentLaunch, HipError> {
    let event_device = stream.device_id();
    let submission_marker = HipEvent::new_on_device(event_device)?;
    let kernel = HipEventSpan::new_on_device(event_device)?;

    // The pre-submit marker is ordered on the caller stream before starting
    // the host clock. The C++ wrapper records `kernel`'s start event itself,
    // immediately before `hipLaunchKernelGGL`, making their device-clock span
    // a real submission-to-kernel boundary rather than two queued markers.
    submission_marker.record(stream)?;
    let kernel_start_event = kernel.start_raw();
    let (code, host_submission) = match field {
        PermanentField::F3 => {
            // SAFETY: this function's safety contract guarantees the valid
            // device ranges, dimensions, and stream/allocation lifetimes that
            // the F_3 stream-bearing FFI launch and the live kernel start
            // event required by this crate-private raw helper.
            let submission_started = Instant::now();
            let code = unsafe {
                compute_permanent_gf3_batch_on_stream_with_kernel_start_event(
                    matrices_ptr,
                    n,
                    m,
                    out_ptr,
                    stream.as_raw(),
                    kernel_start_event,
                )
            };
            (code, submission_started.elapsed())
        }
        PermanentField::F5 => {
            // SAFETY: this function's safety contract guarantees the valid
            // device ranges, dimensions, and stream/allocation lifetimes that
            // the F_5 stream-bearing FFI launch and the live kernel start
            // event required by this crate-private raw helper.
            let submission_started = Instant::now();
            let code = unsafe {
                compute_permanent_gf5_batch_on_stream_with_kernel_start_event(
                    matrices_ptr,
                    n,
                    m,
                    out_ptr,
                    stream.as_raw(),
                    kernel_start_event,
                )
            };
            (code, submission_started.elapsed())
        }
        PermanentField::F7 => {
            // SAFETY: this function's safety contract guarantees the valid
            // device ranges, dimensions, stream/allocation lifetimes, and
            // initialized F_7 LUTs required by the F_7 stream-bearing launch,
            // plus the live kernel start event used by the raw helper.
            let submission_started = Instant::now();
            let code = unsafe {
                compute_permanent_gf7_batch_on_stream_with_kernel_start_event(
                    matrices_ptr,
                    n,
                    m,
                    out_ptr,
                    stream.as_raw(),
                    kernel_start_event,
                )
            };
            (code, submission_started.elapsed())
        }
    };
    if code != 0 {
        return Err(HipError::Hip {
            code,
            context: "permanent batch kernel launch on stream",
        });
    }

    kernel.record_stop(stream)?;
    Ok(InstrumentedPermanentLaunch {
        kernel,
        submission_marker,
        host_submission,
    })
}

/// An in-flight permanent dispatch with stream-local timing for H2D, kernel,
/// and D2H phases.
///
/// The boundary owns its buffers so their lifetimes cover all asynchronous
/// operations. [`finish`](Self::finish) synchronizes only its caller-supplied
/// stream, not the device, and returns the results with complete event spans.
pub struct InstrumentedPermanentDispatch<'a> {
    stream: &'a HipStream,
    _matrices: DeviceBuffer<u8>,
    _output: DeviceBuffer<u64>,
    _input_staging: PinnedHostBuffer<u8>,
    output_staging: PinnedHostBuffer<u64>,
    h2d: HipEventSpan,
    launch: InstrumentedPermanentLaunch,
    d2h: HipEventSpan,
}

impl InstrumentedPermanentDispatch<'_> {
    /// Returns whether the final D2H stop event has completed without blocking.
    pub fn is_complete(&self) -> Result<bool, HipError> {
        self.d2h.is_complete()
    }

    /// Returns all completed phase and launch-overhead timing data.
    ///
    /// # Errors
    ///
    /// Returns `hipErrorNotReady` if any requested event span is incomplete.
    pub fn phase_timings(&self) -> Result<PermanentPhaseTimings, HipError> {
        let launch = self.launch.phase_timings()?;
        Ok(PermanentPhaseTimings {
            h2d: Some(self.h2d.elapsed()?),
            kernel: launch.kernel,
            d2h: Some(self.d2h.elapsed()?),
            host_submission: launch.host_submission,
            device_submission_to_kernel: launch.device_submission_to_kernel,
        })
    }

    /// Waits for this dispatch's stream and returns its outputs and timings.
    ///
    /// This is a per-stream wait; it does not synchronize unrelated streams or
    /// the whole HIP device.
    pub fn finish(self) -> Result<(Vec<u64>, PermanentPhaseTimings), HipError> {
        self.stream.synchronize()?;
        let timings = self.phase_timings()?;
        Ok((self.output_staging.as_slice().to_vec(), timings))
    }
}

impl Drop for InstrumentedPermanentDispatch<'_> {
    fn drop(&mut self) {
        // Keep the pinned staging and device allocations alive until queued
        // stream work drains, even when a caller abandons an in-flight handle.
        // This is stream-local cleanup rather than a device-wide synchronization.
        let _ = self.stream.synchronize();
    }
}

/// Drains a stream before locally owned asynchronous-dispatch storage drops.
///
/// Construction code arms this guard immediately before the first async copy.
/// Any later error return drops it before the local pinned and device buffers
/// (which were declared first), keeping those allocations alive until HIP no
/// longer references them. The fully constructed dispatch owns the same
/// lifetime responsibility, so the guard is disarmed only after that ownership
/// transfer succeeds.
struct StreamDrainOnError<'a> {
    stream: &'a HipStream,
    armed: bool,
}

impl<'a> StreamDrainOnError<'a> {
    fn new(stream: &'a HipStream) -> Self {
        Self {
            stream,
            armed: false,
        }
    }

    fn arm(&mut self) {
        self.armed = true;
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for StreamDrainOnError<'_> {
    fn drop(&mut self) {
        if self.armed {
            // A stream-local wait establishes the lifetime condition documented
            // by the async-copy APIs before Rust drops the local allocations.
            let _ = self.stream.synchronize();
        }
    }
}

/// Starts a full H2D/kernel/D2H permanent dispatch with event timing.
///
/// Input and output staging are pinned and owned by the returned handle, so
/// asynchronous copies remain valid until [`InstrumentedPermanentDispatch::finish`]
/// or the handle is dropped. Every phase uses the supplied stream. The caller
/// can inspect completion with [`InstrumentedPermanentDispatch::is_complete`]
/// and receives `hipErrorNotReady` rather than a partial duration before it is
/// complete.
///
/// # Panics
///
/// Panics if `n` is outside `1..=63`, `m == 0`, or `host_matrices` does not
/// contain exactly `m * n * n` bytes.
pub fn dispatch_permanent_batch_instrumented<'a>(
    field: PermanentField,
    host_matrices: &[u8],
    n: usize,
    m: usize,
    stream: &'a HipStream,
) -> Result<InstrumentedPermanentDispatch<'a>, HipError> {
    assert!(
        (1..=63).contains(&n),
        "dispatch_permanent_batch_instrumented: n must be in 1..=63, got {n}"
    );
    assert!(
        m > 0,
        "dispatch_permanent_batch_instrumented: m must be nonzero"
    );
    assert_eq!(
        host_matrices.len(),
        m * n * n,
        "dispatch_permanent_batch_instrumented: host_matrices.len() ({}) != m * n * n ({})",
        host_matrices.len(),
        m * n * n
    );

    let device_id = stream.device_id();
    let mut input_staging = PinnedHostBuffer::<u8>::new(host_matrices.len(), device_id)?;
    input_staging.as_mut_slice().copy_from_slice(host_matrices);
    let mut output_staging = PinnedHostBuffer::<u64>::new(m, device_id)?;
    let matrices = DeviceBuffer::<u8>::new(host_matrices.len(), device_id)?;
    let output = DeviceBuffer::<u64>::new(m, device_id)?;
    // Declared after the storage it protects, so its Drop runs first on every
    // post-arm error path and drains the caller stream before these allocations
    // can be released.
    let mut drain_on_error = StreamDrainOnError::new(stream);

    let h2d = HipEventSpan::new_on_device(device_id)?;
    h2d.record_start(stream)?;
    // Arm before the HIP submission: even an unusual runtime error reported by
    // the async-copy call itself cannot leave queued work referring to storage
    // that is subsequently dropped on this error path.
    drain_on_error.arm();
    matrices.copy_from_pinned_async(&input_staging, stream)?;
    h2d.record_stop(stream)?;

    // SAFETY: `matrices` and `output` are live device buffers of exactly the
    // required sizes, field validation remains the caller's semantic contract,
    // n/m were checked above, and the returned handle owns both buffers until
    // its caller-supplied stream has drained.
    let launch = unsafe {
        launch_permanent_batch_instrumented_on_stream(
            field,
            matrices.as_ptr() as *const u8,
            n as c_int,
            m as c_int,
            output.as_mut_ptr() as *mut u64,
            stream,
        )
    }?;

    let d2h = HipEventSpan::new_on_device(device_id)?;
    d2h.record_start(stream)?;
    output.copy_to_pinned_async(&mut output_staging, stream)?;
    d2h.record_stop(stream)?;

    let dispatch = InstrumentedPermanentDispatch {
        stream,
        _matrices: matrices,
        _output: output,
        _input_staging: input_staging,
        output_staging,
        h2d,
        launch,
        d2h,
    };
    // The handle now owns every allocation and drains its stream on Drop, so
    // this construction-only guard must not perform a second cleanup wait.
    drain_on_error.disarm();
    Ok(dispatch)
}

/// Run the F_3 permanent GPU kernel on a batch of pre-serialised matrices
/// and return the results as a host `Vec<u64>`.
///
/// `host_matrices` must contain exactly `m * n * n` bytes in row-major
/// order, one `u8` per GF(3) element (values 0, 1, 2). Output `result[i]`
/// is the permanent of matrix `i` modulo 3.
///
/// # Arguments
///
/// - `host_matrices` — flat row-major byte buffer: `m` consecutive `n×n`
///   arrays of GF(3) values (`0..=2`). Length must equal `m * n * n`.
/// - `n` — matrix dimension; must satisfy `1 <= n <= 63`.
/// - `m` — batch size; must be `>= 1`.
///
/// # Returns
///
/// `Vec<u64>` of length `m` where `result[i]` is the permanent of the
/// i-th input matrix modulo 3.
///
/// # Panics
///
/// Panics if any HIP runtime call (hipMalloc, hipMemcpy, kernel, sync)
/// returns a non-zero error code, if `n` is outside `1..=63`, or if
/// `host_matrices.len() != m * n * n`.
///
/// # Complexity
///
/// `O(n · 2^n)` GPU work per matrix (all `m` matrices run in parallel).
/// Host overhead: two `hipMalloc` + two `hipMemcpy` + one
/// `hipDeviceSynchronize`.
///
/// # Examples
///
/// ```ignore
/// // Skipped under `cargo test` (requires ROCm + gfx1030):
/// # #[cfg(feature = "hip")] {
/// use gf2_kernels_hip::permanent::permanent_gf3_batch_dispatch;
/// // 1 identity matrix, n=2, in row-major GF(3) bytes: [[1,0],[0,1]]
/// let results = permanent_gf3_batch_dispatch(&[1, 0, 0, 1], 2, 1);
/// assert_eq!(results[0], 1); // perm = 1
/// # }
/// ```
pub fn permanent_gf3_batch_dispatch(host_matrices: &[u8], n: usize, m: usize) -> Vec<u64> {
    assert!(
        (1..=63).contains(&n),
        "permanent_gf3_batch_dispatch: n must be in 1..=63, got n = {n}"
    );
    assert!(m >= 1, "permanent_gf3_batch_dispatch: m must be >= 1");
    assert_eq!(
        host_matrices.len(),
        m * n * n,
        "permanent_gf3_batch_dispatch: host_matrices.len() ({}) != m * n * n ({})",
        host_matrices.len(),
        m * n * n
    );

    let total_bytes = m * n * n;
    let out_bytes = m * std::mem::size_of::<u64>();

    let d_mat = crate::DecoderDeviceBuffer::new(total_bytes)
        .unwrap_or_else(|e| panic!("permanent_gf3_batch_dispatch: {e}"));
    let d_out = crate::DecoderDeviceBuffer::new(out_bytes)
        .unwrap_or_else(|e| panic!("permanent_gf3_batch_dispatch: {e}"));

    d_mat
        .copy_from_host(host_matrices)
        .unwrap_or_else(|e| panic!("permanent_gf3_batch_dispatch: H2D copy failed: {e}"));

    // SAFETY: d_mat and d_out are valid device allocations. n and m are
    // validated above. The FFI pointers are device-only and not
    // dereferenced on the host.
    let rc = unsafe {
        compute_permanent_gf3_batch(
            d_mat.as_ptr() as *const u8,
            n as c_int,
            m as c_int,
            d_out.as_mut_ptr() as *mut u64,
        )
    };
    assert_eq!(
        rc, 0,
        "permanent_gf3_batch_dispatch: compute_permanent_gf3_batch returned HIP error {rc}"
    );

    // SAFETY: hipDeviceSynchronize has no preconditions.
    let rc = unsafe { crate::ffi::hip_device_synchronize() };
    assert_eq!(
        rc, 0,
        "permanent_gf3_batch_dispatch: hipDeviceSynchronize returned HIP error {rc}"
    );

    let mut out = vec![0u64; m];
    // SAFETY: `out` is a host-allocated Vec<u64>; reinterpreting as &mut [u8] is
    // safe because u64 has no padding.
    let out_bytes_slice = unsafe {
        std::slice::from_raw_parts_mut(out.as_mut_ptr() as *mut u8, m * std::mem::size_of::<u64>())
    };
    d_out
        .copy_to_host(out_bytes_slice)
        .unwrap_or_else(|e| panic!("permanent_gf3_batch_dispatch: D2H copy failed: {e}"));

    out
}

/// Run the F_5 permanent GPU kernel on a batch of pre-serialised matrices
/// and return the results as a host `Vec<u64>`.
///
/// `host_matrices` must contain exactly `m * n * n` bytes in row-major
/// order, one `u8` per GF(5) element (values 0..=4). Output `result[i]`
/// is the permanent of matrix `i` modulo 5.
///
/// # Arguments
///
/// - `host_matrices` — flat row-major byte buffer: `m` consecutive `n×n`
///   arrays of GF(5) values (`0..=4`). Length must equal `m * n * n`.
/// - `n` — matrix dimension; must satisfy `1 <= n <= 63`.
/// - `m` — batch size; must be `>= 1`.
///
/// # Returns
///
/// `Vec<u64>` of length `m`.
///
/// # Panics
///
/// Panics on HIP errors or invalid arguments (see [`permanent_gf3_batch_dispatch`]).
///
/// # Complexity
///
/// `O(n · 2^n)` GPU work per matrix.
///
/// # Examples
///
/// ```ignore
/// // Skipped under `cargo test` (requires ROCm + gfx1030):
/// # #[cfg(feature = "hip")] {
/// use gf2_kernels_hip::permanent::permanent_gf5_batch_dispatch;
/// let results = permanent_gf5_batch_dispatch(&[1, 0, 0, 1], 2, 1);
/// assert_eq!(results[0], 1); // perm of identity = 1
/// # }
/// ```
pub fn permanent_gf5_batch_dispatch(host_matrices: &[u8], n: usize, m: usize) -> Vec<u64> {
    assert!(
        (1..=63).contains(&n),
        "permanent_gf5_batch_dispatch: n must be in 1..=63, got n = {n}"
    );
    assert!(m >= 1, "permanent_gf5_batch_dispatch: m must be >= 1");
    assert_eq!(
        host_matrices.len(),
        m * n * n,
        "permanent_gf5_batch_dispatch: host_matrices.len() ({}) != m * n * n ({})",
        host_matrices.len(),
        m * n * n
    );

    let total_bytes = m * n * n;
    let out_bytes = m * std::mem::size_of::<u64>();

    let d_mat = crate::DecoderDeviceBuffer::new(total_bytes)
        .unwrap_or_else(|e| panic!("permanent_gf5_batch_dispatch: {e}"));
    let d_out = crate::DecoderDeviceBuffer::new(out_bytes)
        .unwrap_or_else(|e| panic!("permanent_gf5_batch_dispatch: {e}"));

    d_mat
        .copy_from_host(host_matrices)
        .unwrap_or_else(|e| panic!("permanent_gf5_batch_dispatch: H2D copy failed: {e}"));

    // SAFETY: d_mat and d_out are valid device allocations; n, m validated.
    let rc = unsafe {
        compute_permanent_gf5_batch(
            d_mat.as_ptr() as *const u8,
            n as c_int,
            m as c_int,
            d_out.as_mut_ptr() as *mut u64,
        )
    };
    assert_eq!(
        rc, 0,
        "permanent_gf5_batch_dispatch: compute_permanent_gf5_batch returned HIP error {rc}"
    );

    // SAFETY: hipDeviceSynchronize has no preconditions.
    let rc = unsafe { crate::ffi::hip_device_synchronize() };
    assert_eq!(
        rc, 0,
        "permanent_gf5_batch_dispatch: hipDeviceSynchronize returned HIP error {rc}"
    );

    let mut out = vec![0u64; m];
    // SAFETY: reinterpreting Vec<u64> as &mut [u8] is safe (no padding in u64).
    let out_bytes_slice = unsafe {
        std::slice::from_raw_parts_mut(out.as_mut_ptr() as *mut u8, m * std::mem::size_of::<u64>())
    };
    d_out
        .copy_to_host(out_bytes_slice)
        .unwrap_or_else(|e| panic!("permanent_gf5_batch_dispatch: D2H copy failed: {e}"));

    out
}

/// Run the F_7 permanent GPU kernel on a batch of pre-serialised matrices
/// and return the results as a host `Vec<u64>`.
///
/// **Precondition:** the F_7 device LUTs must have been initialised by a
/// prior call to [`init_permanent_gf7`] (or by calling this via
/// `gf2_algebra::gpu::permanent_batch_bipedal7`, which does the one-shot
/// init automatically). If the memoised init state is non-zero (failed or
/// never called), this function panics.
///
/// `host_matrices` must contain exactly `m * n * n` bytes in row-major
/// order, one `u8` per GF(7) element (values 0..=6). Output `result[i]`
/// is the permanent of matrix `i` modulo 7.
///
/// # Arguments
///
/// - `host_matrices` — flat row-major byte buffer: `m` consecutive `n×n`
///   arrays of GF(7) values (`0..=6`). Length must equal `m * n * n`.
/// - `n` — matrix dimension; must satisfy `1 <= n <= 63`.
/// - `m` — batch size; must be `>= 1`.
///
/// # Returns
///
/// `Vec<u64>` of length `m`.
///
/// # Panics
///
/// Panics on HIP errors, invalid arguments, or if `init_permanent_gf7`
/// has not been called successfully beforehand.
///
/// # Complexity
///
/// `O(n · 2^n)` GPU work per matrix.
///
/// # Examples
///
/// ```ignore
/// // Skipped under `cargo test` (requires ROCm + gfx1030 + init):
/// # #[cfg(feature = "hip")] {
/// use gf2_kernels_hip::permanent::{init_permanent_gf7, permanent_gf7_batch_dispatch};
/// // Caller must init LUTs first.
/// // unsafe { init_permanent_gf7(add_ptr, sub_ptr, mul_ptr) };
/// let results = permanent_gf7_batch_dispatch(&[1, 0, 0, 1], 2, 1);
/// assert_eq!(results[0], 1); // perm of identity = 1
/// # }
/// ```
pub fn permanent_gf7_batch_dispatch(host_matrices: &[u8], n: usize, m: usize) -> Vec<u64> {
    assert!(
        (1..=63).contains(&n),
        "permanent_gf7_batch_dispatch: n must be in 1..=63, got n = {n}"
    );
    assert!(m >= 1, "permanent_gf7_batch_dispatch: m must be >= 1");
    assert_eq!(
        host_matrices.len(),
        m * n * n,
        "permanent_gf7_batch_dispatch: host_matrices.len() ({}) != m * n * n ({})",
        host_matrices.len(),
        m * n * n
    );

    let total_bytes = m * n * n;
    let out_bytes = m * std::mem::size_of::<u64>();

    let d_mat = crate::DecoderDeviceBuffer::new(total_bytes)
        .unwrap_or_else(|e| panic!("permanent_gf7_batch_dispatch: {e}"));
    let d_out = crate::DecoderDeviceBuffer::new(out_bytes)
        .unwrap_or_else(|e| panic!("permanent_gf7_batch_dispatch: {e}"));

    d_mat
        .copy_from_host(host_matrices)
        .unwrap_or_else(|e| panic!("permanent_gf7_batch_dispatch: H2D copy failed: {e}"));

    // SAFETY: d_mat and d_out are valid device allocations; n, m validated.
    // init_permanent_gf7 must have been called; compute_permanent_gf7_batch
    // checks the memoised GF7_INIT_RC and returns non-zero if not done.
    let rc = unsafe {
        compute_permanent_gf7_batch(
            d_mat.as_ptr() as *const u8,
            n as c_int,
            m as c_int,
            d_out.as_mut_ptr() as *mut u64,
        )
    };
    assert_eq!(
        rc, 0,
        "permanent_gf7_batch_dispatch: compute_permanent_gf7_batch returned HIP error {rc}. \
         Ensure init_permanent_gf7 was called successfully first."
    );

    // SAFETY: hipDeviceSynchronize has no preconditions.
    let rc = unsafe { crate::ffi::hip_device_synchronize() };
    assert_eq!(
        rc, 0,
        "permanent_gf7_batch_dispatch: hipDeviceSynchronize returned HIP error {rc}"
    );

    let mut out = vec![0u64; m];
    // SAFETY: reinterpreting Vec<u64> as &mut [u8] is safe (no padding in u64).
    let out_bytes_slice = unsafe {
        std::slice::from_raw_parts_mut(out.as_mut_ptr() as *mut u8, m * std::mem::size_of::<u64>())
    };
    d_out
        .copy_to_host(out_bytes_slice)
        .unwrap_or_else(|e| panic!("permanent_gf7_batch_dispatch: D2H copy failed: {e}"));

    out
}

#[cfg(test)]
mod init_state_machine_tests {
    //! Pure-Rust unit tests for the [`memoise_init_outcome`] CAS state
    //! machine, exercising the init-contract semantics without a HIP/ROCm
    //! device. These cover the regression that prompted the rewrite from
    //! the original non-atomic load-then-store: a failed init clobbering
    //! a concurrent successful init.
    use super::{memoise_init_outcome, GF7_INIT_UNINIT};
    use std::sync::atomic::{AtomicI32, Ordering::SeqCst};

    /// Fresh-state semantics: a single failed init records its rc.
    #[test]
    fn test_memoise_init_failed_init_records_rc() {
        let state = AtomicI32::new(GF7_INIT_UNINIT);
        memoise_init_outcome(&state, 7);
        assert_eq!(state.load(SeqCst), 7);
    }

    /// Fresh-state semantics: a single successful init records 0.
    #[test]
    fn test_memoise_init_successful_init_records_zero() {
        let state = AtomicI32::new(GF7_INIT_UNINIT);
        memoise_init_outcome(&state, 0);
        assert_eq!(state.load(SeqCst), 0);
    }

    /// Success-after-failure: a later success overwrites a prior failure.
    #[test]
    fn test_memoise_init_success_overwrites_prior_failure() {
        let state = AtomicI32::new(GF7_INIT_UNINIT);
        memoise_init_outcome(&state, 5);
        assert_eq!(state.load(SeqCst), 5);
        memoise_init_outcome(&state, 0);
        assert_eq!(state.load(SeqCst), 0);
    }

    /// Success-stickiness: a later failure does NOT overwrite a prior
    /// success. This is the critical contract — the regression that
    /// prompted the CAS rewrite — and the key invariant the prior
    /// non-atomic load-then-store violated under concurrency.
    #[test]
    fn test_memoise_init_failure_does_not_overwrite_success() {
        let state = AtomicI32::new(GF7_INIT_UNINIT);
        memoise_init_outcome(&state, 0);
        memoise_init_outcome(&state, 9);
        assert_eq!(
            state.load(SeqCst),
            0,
            "memoise_init_outcome must not overwrite a prior success"
        );
    }

    /// Multi-call idempotency: repeated successful inits stay at 0.
    #[test]
    fn test_memoise_init_repeated_success_idempotent() {
        let state = AtomicI32::new(GF7_INIT_UNINIT);
        for _ in 0..16 {
            memoise_init_outcome(&state, 0);
        }
        assert_eq!(state.load(SeqCst), 0);
    }

    /// Concurrent stress: spawn N threads, half succeeding, half failing.
    /// The final state must be 0 (success) because at least one success
    /// landed.
    #[test]
    fn test_memoise_init_concurrent_success_wins() {
        let state = std::sync::Arc::new(AtomicI32::new(GF7_INIT_UNINIT));
        let mut threads = Vec::new();
        for i in 0..16 {
            let s = std::sync::Arc::clone(&state);
            let rc = if i % 2 == 0 { 0 } else { 100 + i };
            threads.push(std::thread::spawn(move || memoise_init_outcome(&s, rc)));
        }
        for t in threads {
            t.join().unwrap();
        }
        assert_eq!(
            state.load(SeqCst),
            0,
            "at least one success ran; final state must be 0 (success-wins contract)"
        );
    }
}
