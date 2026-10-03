//! FFI shims and host wrappers for the F_3, F_5 and F_7 permanent kernels
//! under `hip/permanent/`. Every kernel accepts matrix dimensions
//! `1 <= n <= 63`.

use std::ffi::c_void;
use std::os::raw::c_int;
use std::time::{Duration, Instant};

use crate::host::{DeviceBuffer, HipEvent, HipEventSpan, HipStream, PinnedHostBuffer};
use crate::HipError;

extern "C" {
    /// Enqueue one dependency-chained Gray update micro-kernel on `stream`.
    ///
    /// F_3 receives its two Bipedal3 planes through `bipedal_column`; F_5 and
    /// F_7 receive their byte-control column through `byte_column`.
    /// `out` is a device output buffer: F_3 writes its two final Bipedal3
    /// planes, while F_5/F_7 write one checksum word. The kernel alternates
    /// add and subtract updates on one accumulator for `steps` iterations so
    /// the dependency chain remains observable.
    fn launch_gray_update_micro(
        byte_column: *const u8,
        bipedal_column: *const u64,
        q: c_int,
        n: c_int,
        steps: u64,
        out: *mut u64,
        stream: *mut c_void,
    ) -> c_int;

    /// Enqueue the paired compiler-barrier baseline for the Gray-update mode.
    fn launch_gray_update_compiler_barrier_baseline(
        q: c_int,
        n: c_int,
        steps: u64,
        out: *mut u64,
        stream: *mut c_void,
    ) -> c_int;

    /// Enqueue one branch-specific horizontal-product micro-kernel.
    ///
    /// `byte_values` holds `sample_count * n` canonical field values for the
    /// byte and lookup circuits. `plane_values` holds two Bipedal3 planes or
    /// three F_5/F_7 planes per sample for their packed circuits. Exactly one
    /// pointer is non-null according to `circuit`; `out` has one entry per
    /// sample and remains live through stream synchronization.
    fn launch_horizontal_product_micro(
        byte_values: *const u8,
        plane_values: *const u64,
        circuit: c_int,
        branch: c_int,
        n: c_int,
        sample_count: c_int,
        out: *mut u64,
        stream: *mut c_void,
    ) -> c_int;

    /// Enqueue the compiler-barrier baseline paired to one horizontal-product
    /// circuit and branch. `iterations` supplies the real early-exit length for
    /// byte-product samples; the other circuits encode their fixed branch
    /// geometry. `out` has one entry per sample and remains live through the
    /// stream synchronization.
    fn launch_horizontal_product_compiler_barrier_baseline(
        circuit: c_int,
        branch: c_int,
        n: c_int,
        sample_count: c_int,
        iterations: *const u8,
        out: *mut u64,
        stream: *mut c_void,
    ) -> c_int;

    /// Computes the permanent of an n×n matrix over GF(3) on the GPU.
    ///
    /// - `matrix_ptr` — device pointer to an n×n row-major array of `u8`
    ///   elements in GF(3) (values 0, 1, 2).
    /// - `n` — matrix dimension; must satisfy `1 <= n <= 63`.
    /// - `out_ptr` — device pointer to a single `u64` output that receives
    ///   the permanent value modulo 3.
    ///
    /// Returns 0 on success (`hipSuccess`), a non-zero HIP error code otherwise.
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

    /// Computes the permanent of an n×n matrix over GF(5) on the GPU.
    ///
    /// - `matrix_ptr` — device pointer to an n×n row-major array of `u8`
    ///   elements in GF(5) (values 0..4).
    /// - `n` — matrix dimension; must satisfy `1 <= n <= 63`.
    /// - `out_ptr` — device pointer to a single `u64` output that receives
    ///   the permanent value modulo 5.
    ///
    /// Returns 0 on success (`hipSuccess`), a non-zero HIP error code otherwise.
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

    /// Copies the host F_7 ADD/SUB/MUL LUTs (65536 bytes each) to the device
    /// symbols `d_ADD_LUT`, `d_SUB_LUT` and `d_MUL_LUT`.
    ///
    /// Returns 0 on success (`hipSuccess`), a non-zero HIP error code otherwise.
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

    /// Computes the permanent of an n×n matrix over GF(7) on the GPU.
    ///
    /// - `matrix_ptr` — device pointer to an n×n row-major array of `u8`
    ///   elements in GF(7) (values 0..6).
    /// - `n` — matrix dimension; must satisfy `1 <= n <= 63`.
    /// - `out_ptr` — device pointer to a single `u64` output that receives
    ///   the permanent value modulo 7.
    ///
    /// Returns 0 on success (`hipSuccess`), a non-zero HIP error code otherwise.
    #[allow(dead_code)]
    // `compute_permanent_gf7` uses the batch entry, which checks `GF7_INIT_RC`.
    fn permanent_bipedal7_hip(matrix_ptr: *const u8, n: c_int, out_ptr: *mut u64) -> c_int;

    /// Sums all 65536 bytes of the device `d_MUL_LUT` in a single thread and
    /// stores the `u64` result to `*out_ptr`, a device pointer.
    ///
    /// Returns 0 on success (`hipSuccess`), a non-zero HIP error code otherwise.
    fn permanent_bipedal7_hip_lut_checksum(out_ptr: *mut u64) -> c_int;
}

/// Memoised state of the F_7 LUT init: [`GF7_INIT_UNINIT`] before any init
/// attempt, `0` (hipSuccess) once the device LUTs are populated, or the HIP
/// error code of the last failed attempt. The F_7 launch refuses to run and
/// returns this value unless it is `0`.
static GF7_INIT_RC: std::sync::atomic::AtomicI32 =
    std::sync::atomic::AtomicI32::new(GF7_INIT_UNINIT);
/// Sentinel "not yet initialised" value. Chosen as `i32::MIN` so it is
/// distinguishable from every plausible HIP error code (which are small
/// non-negative integers, typically `<= 1000`).
const GF7_INIT_UNINIT: i32 = i32::MIN;

/// Computes the F_3 permanent of a single n×n matrix on the GPU and returns
/// the HIP status code (`0` = `hipSuccess`).
///
/// # Safety
///
/// - `matrix_ptr` must be a valid device allocation of at least `n * n` bytes,
///   row-major, containing GF(3) element values (`0`, `1`, `2`).
/// - `out_ptr` must be a valid device allocation of at least 8 bytes; it
///   receives the permanent modulo 3.
/// - `n` must satisfy `1 <= n <= 63`.
/// - The HIP runtime must be initialised and a device context must be active.
///
/// # Complexity
///
/// `O(n · 2^n)` GPU work.
pub unsafe fn compute_permanent_gf3(matrix_ptr: *const u8, n: c_int, out_ptr: *mut u64) -> c_int {
    // SAFETY: preconditions forwarded verbatim from the caller (see doc comment).
    unsafe { permanent_bipedal3_hip(matrix_ptr, n, out_ptr) }
}

/// Enqueues F_3 permanents for a batch of `m` n×n matrices in one kernel
/// launch on the default stream and returns the HIP status code (`0` =
/// `hipSuccess`).
///
/// # Safety
///
/// - `matrices_ptr` must be a valid device allocation of at least `m * n * n`
///   bytes: `m` consecutive row-major matrices of GF(3) values (`0`, `1`, `2`).
/// - `out_ptr` must be a valid device allocation of at least `m * 8` bytes;
///   `out_ptr[i]` receives the permanent of matrix `i` modulo 3.
/// - `n` must satisfy `1 <= n <= 63`.
/// - `m` must be `>= 1`.
/// - The HIP runtime must be initialised and a device context must be active.
///
/// # Complexity
///
/// `O(n · 2^n)` GPU work per matrix.
pub unsafe fn compute_permanent_gf3_batch(
    matrices_ptr: *const u8,
    n: c_int,
    m: c_int,
    out_ptr: *mut u64,
) -> c_int {
    // SAFETY: preconditions forwarded verbatim from the caller (see doc comment).
    // A null stream selects the HIP default stream.
    unsafe {
        compute_permanent_gf3_batch_on_stream(matrices_ptr, n, m, out_ptr, std::ptr::null_mut())
    }
}

/// Computes an F_3 permanent batch on a caller-supplied HIP stream.
///
/// Only enqueues the kernel; the caller synchronizes the stream before
/// reading `out_ptr`.
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
    // are forwarded verbatim from this unsafe function's contract.
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

/// Computes the F_5 permanent of a single n×n matrix on the GPU and returns
/// the HIP status code (`0` = `hipSuccess`).
///
/// # Safety
///
/// - `matrix_ptr` must be a valid device allocation of at least `n * n` bytes,
///   row-major, containing GF(5) element values (`0..=4`).
/// - `out_ptr` must be a valid device allocation of at least 8 bytes; it
///   receives the permanent modulo 5.
/// - `n` must satisfy `1 <= n <= 63`.
/// - The HIP runtime must be initialised and a device context must be active.
///
/// # Complexity
///
/// `O(n · 2^n)` GPU work.
pub unsafe fn compute_permanent_gf5(matrix_ptr: *const u8, n: c_int, out_ptr: *mut u64) -> c_int {
    // SAFETY: preconditions forwarded verbatim from the caller (see doc comment).
    unsafe { permanent_bipedal5_hip(matrix_ptr, n, out_ptr) }
}

/// Enqueues F_5 permanents for a batch of `m` n×n matrices in one kernel
/// launch on the default stream and returns the HIP status code (`0` =
/// `hipSuccess`).
///
/// # Safety
///
/// - `matrices_ptr` must be a valid device allocation of at least `m * n * n`
///   bytes: `m` consecutive row-major matrices of GF(5) values (`0..=4`).
/// - `out_ptr` must be a valid device allocation of at least `m * 8` bytes;
///   `out_ptr[i]` receives the permanent of matrix `i` modulo 5.
/// - `n` must satisfy `1 <= n <= 63`.
/// - `m` must be `>= 1`.
/// - The HIP runtime must be initialised and a device context must be active.
///
/// # Complexity
///
/// `O(n · 2^n)` GPU work per matrix.
pub unsafe fn compute_permanent_gf5_batch(
    matrices_ptr: *const u8,
    n: c_int,
    m: c_int,
    out_ptr: *mut u64,
) -> c_int {
    // SAFETY: preconditions forwarded verbatim from the caller (see doc comment).
    // A null stream selects the HIP default stream.
    unsafe {
        compute_permanent_gf5_batch_on_stream(matrices_ptr, n, m, out_ptr, std::ptr::null_mut())
    }
}

/// Computes an F_5 permanent batch on a caller-supplied HIP stream.
///
/// Only enqueues the kernel; the caller synchronizes the stream before
/// reading `out_ptr`.
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
    // are forwarded verbatim from this unsafe function's contract.
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

/// Copies the F_7 ADD, SUB and MUL LUTs to the device and memoises the
/// outcome; returns the HIP status code (`0` = `hipSuccess`).
///
/// [`compute_permanent_gf7`] and [`compute_permanent_gf7_batch`] refuse to
/// launch and return the memoised code until one call has succeeded. The LUTs
/// are caller-supplied because `gf2-algebra`, which owns them, depends on this
/// crate through its `hip` feature.
///
/// # Safety
///
/// - All three pointers must be valid host pointers to exactly 65 536 bytes
///   of F_7 LUT data.
/// - The HIP runtime must be initialised and a device context must be active.
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

/// Records `rc` into `state`: a recorded success (`0`) is never overwritten,
/// while the uninitialised sentinel and a failed code are replaced by `rc`.
fn memoise_init_outcome(state: &std::sync::atomic::AtomicI32, rc: c_int) {
    use std::sync::atomic::Ordering::SeqCst;
    loop {
        let prev = state.load(SeqCst);
        if prev == 0 {
            return;
        }
        if state.compare_exchange(prev, rc, SeqCst, SeqCst).is_ok() {
            return;
        }
    }
}

/// Computes the F_7 permanent of a single n×n matrix on the GPU and returns
/// the HIP status code, or the memoised init code when [`init_permanent_gf7`]
/// has not succeeded.
///
/// # Safety
///
/// - `matrix_ptr` must be a valid device allocation of at least `n * n` bytes,
///   row-major, containing GF(7) element values (`0..=6`).
/// - `out_ptr` must be a valid device allocation of at least 8 bytes; it
///   receives the permanent modulo 7.
/// - `n` must satisfy `1 <= n <= 63`.
/// - The HIP runtime must be initialised and a device context must be active.
///
/// # Complexity
///
/// `O(n · 2^n)` GPU work.
pub unsafe fn compute_permanent_gf7(matrix_ptr: *const u8, n: c_int, out_ptr: *mut u64) -> c_int {
    // SAFETY: preconditions forwarded verbatim from the caller (see doc comment).
    unsafe { compute_permanent_gf7_batch(matrix_ptr, n, 1, out_ptr) }
}

/// Enqueues F_7 permanents for a batch of `m` n×n matrices in one kernel
/// launch on the default stream and returns the HIP status code. When
/// [`init_permanent_gf7`] has not succeeded, nothing is launched and the
/// memoised init code is returned.
///
/// # Safety
///
/// - `matrices_ptr` must be a valid device allocation of at least `m * n * n`
///   bytes: `m` consecutive row-major matrices of GF(7) values (`0..=6`).
/// - `out_ptr` must be a valid device allocation of at least `m * 8` bytes;
///   `out_ptr[i]` receives the permanent of matrix `i` modulo 7.
/// - `n` must satisfy `1 <= n <= 63`.
/// - `m` must be `>= 1`.
/// - The HIP runtime must be initialised and a device context must be active.
///
/// # Complexity
///
/// `O(n · 2^n)` GPU work per matrix.
pub unsafe fn compute_permanent_gf7_batch(
    matrices_ptr: *const u8,
    n: c_int,
    m: c_int,
    out_ptr: *mut u64,
) -> c_int {
    // SAFETY: preconditions forwarded verbatim from the caller (see doc comment).
    // A null stream selects the HIP default stream.
    unsafe {
        compute_permanent_gf7_batch_on_stream(matrices_ptr, n, m, out_ptr, std::ptr::null_mut())
    }
}

/// Computes an F_7 permanent batch on a caller-supplied HIP stream.
///
/// Only enqueues the kernel; the caller synchronizes the stream before
/// reading `out_ptr`. When [`init_permanent_gf7`] has not succeeded, nothing
/// is enqueued and the memoised init code is returned.
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
    // SAFETY: all device-pointer, dimension, stream-lifetime, and initialized
    // LUT preconditions are forwarded from this unsafe function's contract.
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

/// Sums all 65 536 bytes of the device `d_MUL_LUT` into `*out_ptr` and
/// returns the HIP status code (`0` = `hipSuccess`).
///
/// # Safety
///
/// - `out_ptr` must be a valid device allocation of at least 8 bytes.
/// - The HIP runtime must be initialised and a device context must be active.
/// - [`init_permanent_gf7`] must have succeeded so that `d_MUL_LUT` is
///   populated.
pub unsafe fn compute_lut_checksum_gpu(out_ptr: *mut u64) -> c_int {
    // SAFETY: preconditions forwarded verbatim from the caller (see doc comment).
    unsafe { permanent_bipedal7_hip_lut_checksum(out_ptr) }
}

/// [`init_permanent_gf7`] over typed references to the three 65 536-byte F_7
/// LUTs; returns the HIP status code (`0` = `hipSuccess`). The HIP runtime
/// must be initialised.
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

/// One GPU micro-measurement's paired device-event spans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GrayUpdateTimings {
    /// Device-event duration of the dependency-chained field updates.
    pub update: Duration,
    /// Device-event duration of the matched compiler-barrier baseline.
    pub compiler_barrier_baseline: Duration,
    /// Final update state copied after both event spans have stopped.
    pub update_checksum: GrayUpdateChecksum,
}

/// Observable final state of one event-timed Gray-update chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrayUpdateChecksum {
    /// The exact Bipedal3 accumulator planes for the F_3 packed update.
    Bipedal3 {
        /// Magnitude plane.
        mag: u64,
        /// Sign plane.
        sgn: u64,
    },
    /// Checksum of the byte accumulator used by the F_5/F_7 controls.
    Bytes(u64),
}

/// Representation-specific operand for [`measure_gray_update_kernel`].
pub enum GrayUpdateOperand<'a> {
    /// The two raw Bipedal3 planes used by the F_3 packed update.
    Bipedal3 {
        /// Magnitude bit plane.
        mag: u64,
        /// Sign bit plane.
        sgn: u64,
        /// Active packed row lanes.
        n: usize,
    },
    /// Canonical byte values used by the F_5/F_7 GPU controls.
    Bytes(&'a [u8]),
}

/// Measure one dependency-chained Gray update on the HIP device clock.
///
/// The submitted update kernel holds one row-sum accumulator and alternates
/// add and subtract updates for `steps` iterations.  F_3 uses the actual
/// Bipedal3 two-plane update; F_5/F_7 use the byte controls.  A second
/// event-timed kernel executes the same loop geometry with compiler barriers,
/// allowing the harness to aggregate repetitions and report
/// `(sum(update) - sum(baseline)) / (steps * reps)` without including
/// host-loop, allocation, transfer, or submission overhead.
///
/// # Panics
///
/// Panics if `steps == 0`, the operand's active-lane count lies outside
/// `1..=63`, the field/operand representation does not match, or a byte
/// operand contains a value outside its field.  A Bipedal3 operand also
/// panics when either plane has a bit above its explicit active-lane domain.
///
/// # Errors
///
/// Returns the HIP error from stream creation, allocation, upload, launch,
/// synchronization, or event timing.
pub fn measure_gray_update_kernel(
    field: PermanentField,
    operand: GrayUpdateOperand<'_>,
    steps: u64,
) -> Result<GrayUpdateTimings, HipError> {
    let q: c_int = match field {
        PermanentField::F3 => 3,
        PermanentField::F5 => 5,
        PermanentField::F7 => 7,
    };
    assert!(
        (1..=63).contains(&operand_len(&operand)),
        "measure_gray_update_kernel: n must be in 1..=63, got {}",
        operand_len(&operand)
    );
    assert!(
        steps > 0,
        "measure_gray_update_kernel: steps must be nonzero"
    );
    assert!(
        matches!(
            (&field, &operand),
            (PermanentField::F3, GrayUpdateOperand::Bipedal3 { .. })
                | (PermanentField::F5, GrayUpdateOperand::Bytes(_))
                | (PermanentField::F7, GrayUpdateOperand::Bytes(_))
        ),
        "measure_gray_update_kernel: operand representation does not match F_{q}"
    );

    if let GrayUpdateOperand::Bytes(column) = &operand {
        assert!(
            column.iter().all(|&value| c_int::from(value) < q),
            "measure_gray_update_kernel: column contains a value outside F_{q}"
        );
    }
    if let GrayUpdateOperand::Bipedal3 { mag, sgn, n } = &operand {
        validate_bipedal3_active_lanes(*mag, *sgn, *n);
    }

    let stream = HipStream::new()?;
    let device_id = stream.device_id();
    let n = operand_len(&operand);
    let update_checksum = DeviceBuffer::<u64>::new(2, device_id)?;
    let baseline_checksum = DeviceBuffer::<u64>::new(1, device_id)?;
    let (byte_input, bipedal_input, is_bipedal3) = match operand {
        GrayUpdateOperand::Bipedal3 { mag, sgn, .. } => {
            let mut staging = PinnedHostBuffer::<u64>::new(2, device_id)?;
            staging.as_mut_slice().copy_from_slice(&[mag, sgn]);
            let input = DeviceBuffer::<u64>::new(2, device_id)?;
            input.copy_from_pinned_async(&staging, &stream)?;
            (None, Some((input, staging)), true)
        }
        GrayUpdateOperand::Bytes(column) => {
            let mut staging = PinnedHostBuffer::<u8>::new(n, device_id)?;
            staging.as_mut_slice().copy_from_slice(column);
            let input = DeviceBuffer::<u8>::new(n, device_id)?;
            input.copy_from_pinned_async(&staging, &stream)?;
            (Some((input, staging)), None, false)
        }
    };

    let update = HipEventSpan::new_on_device(device_id)?;
    update.record_start(&stream)?;
    // SAFETY: the input and update-output allocations are live device ranges of
    // the required lengths; the field values and dimensions were checked above,
    // and both buffers remain alive through the stream-local synchronization.
    let code = unsafe {
        launch_gray_update_micro(
            byte_input
                .as_ref()
                .map_or(std::ptr::null(), |(input, _)| input.as_ptr() as *const u8),
            bipedal_input
                .as_ref()
                .map_or(std::ptr::null(), |(input, _)| input.as_ptr() as *const u64),
            q,
            n as c_int,
            steps,
            update_checksum.as_mut_ptr() as *mut u64,
            stream.as_raw(),
        )
    };
    if code != 0 {
        return Err(HipError::Hip {
            code,
            context: "launch_gray_update_micro",
        });
    }
    update.record_stop(&stream)?;
    let compiler_barrier_baseline = HipEventSpan::new_on_device(device_id)?;
    compiler_barrier_baseline.record_start(&stream)?;
    // SAFETY: `q`, `n`, and `steps` were validated above; `baseline_checksum`
    // is a live device output allocation of the required length; and the live
    // stream and allocation both outlast the queued work through synchronization.
    let code = unsafe {
        launch_gray_update_compiler_barrier_baseline(
            q,
            n as c_int,
            steps,
            baseline_checksum.as_mut_ptr() as *mut u64,
            stream.as_raw(),
        )
    };
    if code != 0 {
        return Err(HipError::Hip {
            code,
            context: "launch_gray_update_compiler_barrier_baseline",
        });
    }
    compiler_barrier_baseline.record_stop(&stream)?;
    let mut update_staging = PinnedHostBuffer::<u64>::new(2, device_id)?;
    update_checksum.copy_to_pinned_async(&mut update_staging, &stream)?;
    stream.synchronize()?;
    let update_checksum = if is_bipedal3 {
        GrayUpdateChecksum::Bipedal3 {
            mag: update_staging.as_slice()[0],
            sgn: update_staging.as_slice()[1],
        }
    } else {
        GrayUpdateChecksum::Bytes(update_staging.as_slice()[0])
    };
    Ok(GrayUpdateTimings {
        update: update.elapsed()?,
        compiler_barrier_baseline: compiler_barrier_baseline.elapsed()?,
        update_checksum,
    })
}

/// Horizontal-product circuit selected by the measurement harness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HorizontalProductCircuit {
    /// F_3 Bipedal3 six-stage halving control.
    Bipedal3Halving,
    /// F_3 active zero-mask followed by sign-popcount parity.
    Bipedal3ZeroMaskSignPopcount,
    /// F_5 direct-byte product with its native early zero exit.
    F5Byte,
    /// F_5 three-plane active zero-mask followed by C4 log-popcount reduction.
    F5ThreePlane,
    /// F_7 native lookup-table product with its early zero exit.
    F7Lookup,
    /// F_7 three-plane active zero-mask followed by C6 log-popcount reduction.
    F7ThreePlane,
}

impl HorizontalProductCircuit {
    /// Stable circuit name recorded in measurement notes.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Bipedal3Halving => "bipedal3-halving",
            Self::Bipedal3ZeroMaskSignPopcount => "bipedal3-zero-mask-sign-popcount",
            Self::F5Byte => "f5-byte",
            Self::F5ThreePlane => "f5-three-plane-c4",
            Self::F7Lookup => "f7-lookup",
            Self::F7ThreePlane => "f7-three-plane-c6",
        }
    }

    /// Field order accepted by this circuit.
    #[must_use]
    pub const fn field_order(self) -> u64 {
        match self {
            Self::Bipedal3Halving | Self::Bipedal3ZeroMaskSignPopcount => 3,
            Self::F5Byte | Self::F5ThreePlane => 5,
            Self::F7Lookup | Self::F7ThreePlane => 7,
        }
    }

    /// Whether the circuit exposes distinct zero-fast and nonzero-slow
    /// execution paths. The halving control only discovers zero after its full
    /// reduction and must not be assigned synthetic branch timings.
    #[must_use]
    pub const fn has_observable_branches(self) -> bool {
        !matches!(self, Self::Bipedal3Halving)
    }

    const fn raw(self) -> c_int {
        match self {
            Self::Bipedal3Halving => 1,
            Self::Bipedal3ZeroMaskSignPopcount => 2,
            Self::F5Byte => 3,
            Self::F5ThreePlane => 4,
            Self::F7Lookup => 5,
            Self::F7ThreePlane => 6,
        }
    }
}

/// Which product branch an event-timed kernel executes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HorizontalProductBranch {
    /// A sampled vector with at least one zero row sum.
    ZeroFast,
    /// A sampled vector whose active row sums are all nonzero.
    NonzeroSlow,
}

impl HorizontalProductBranch {
    const fn raw(self) -> c_int {
        match self {
            Self::ZeroFast => 0,
            Self::NonzeroSlow => 1,
        }
    }
}

/// Paired event spans and output values from one horizontal-product branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HorizontalProductTimings {
    /// Device-event duration of the selected branch's product circuit.
    pub product: Duration,
    /// Device-event duration of the selected branch's same-geometry baseline.
    pub compiler_barrier_baseline: Duration,
    /// One product result per input row-sum vector, in input order.
    pub values: Vec<u64>,
}

/// Measure one selected horizontal-product branch on the HIP device clock.
///
/// `values` contains consecutive row-sum vectors, each with `n` canonical
/// values in the circuit's field. Its inputs are already conditioned by the
/// caller on `branch`; the wrapper does not count, add, remove, or resample a
/// vector. This keeps frequency measurement outside timing selection. The
/// product event starts after representation encoding and upload, and output
/// download occurs after both paired event stops.
///
/// The F_7 lookup circuit reads the permanent `d_MUL_LUT`; callers initialise
/// it with [`init_permanent_gf7`] first.
///
/// # Panics
///
/// Panics if `n` is outside `1..=63`, `values` is empty or not a whole number
/// of row-sum vectors, a value is outside the circuit's field, or a branch is
/// requested from a circuit without distinct observable branches.
///
/// # Errors
///
/// Returns HIP errors from stream creation, allocation, upload, launch,
/// synchronization, output download, or event timing.
pub fn measure_horizontal_product_kernel(
    circuit: HorizontalProductCircuit,
    values: &[u8],
    n: usize,
    branch: HorizontalProductBranch,
) -> Result<HorizontalProductTimings, HipError> {
    assert!(
        circuit.has_observable_branches(),
        "measure_horizontal_product_kernel: {} has no separated branches",
        circuit.name()
    );
    assert!(
        (1..=63).contains(&n),
        "measure_horizontal_product_kernel: n must be in 1..=63, got {n}"
    );
    assert!(
        !values.is_empty() && values.len().is_multiple_of(n),
        "measure_horizontal_product_kernel: values must contain a nonempty whole number of n-lane samples"
    );
    assert!(
        values
            .iter()
            .all(|&value| u64::from(value) < circuit.field_order()),
        "measure_horizontal_product_kernel: input contains a value outside F_{}",
        circuit.field_order()
    );
    let sample_count = values.len() / n;
    assert!(
        sample_count <= c_int::MAX as usize,
        "measure_horizontal_product_kernel: sample count exceeds c_int"
    );
    let expected_zero = matches!(branch, HorizontalProductBranch::ZeroFast);
    assert!(
        values
            .chunks_exact(n)
            .all(|sample| sample.contains(&0) == expected_zero),
        "measure_horizontal_product_kernel: every sample must belong to the requested branch"
    );

    let stream = HipStream::new()?;
    let device_id = stream.device_id();
    let output = DeviceBuffer::<u64>::new(sample_count, device_id)?;
    let baseline_output = DeviceBuffer::<u64>::new(sample_count, device_id)?;
    let baseline_iterations = horizontal_product_baseline_iterations(circuit, values, n, branch);
    let mut iteration_staging = PinnedHostBuffer::<u8>::new(sample_count, device_id)?;
    iteration_staging
        .as_mut_slice()
        .copy_from_slice(&baseline_iterations);
    let iteration_input = DeviceBuffer::<u8>::new(sample_count, device_id)?;
    iteration_input.copy_from_pinned_async(&iteration_staging, &stream)?;

    let (byte_input, plane_input) = match circuit {
        HorizontalProductCircuit::F5Byte | HorizontalProductCircuit::F7Lookup => {
            let mut staging = PinnedHostBuffer::<u8>::new(values.len(), device_id)?;
            staging.as_mut_slice().copy_from_slice(values);
            let input = DeviceBuffer::<u8>::new(values.len(), device_id)?;
            input.copy_from_pinned_async(&staging, &stream)?;
            (Some((input, staging)), None)
        }
        HorizontalProductCircuit::Bipedal3Halving
        | HorizontalProductCircuit::Bipedal3ZeroMaskSignPopcount
        | HorizontalProductCircuit::F5ThreePlane
        | HorizontalProductCircuit::F7ThreePlane => {
            let planes = encode_horizontal_product_planes(circuit, values, n);
            let mut staging = PinnedHostBuffer::<u64>::new(planes.len(), device_id)?;
            staging.as_mut_slice().copy_from_slice(&planes);
            let input = DeviceBuffer::<u64>::new(planes.len(), device_id)?;
            input.copy_from_pinned_async(&staging, &stream)?;
            (None, Some((input, staging)))
        }
    };

    let product = HipEventSpan::new_on_device(device_id)?;
    product.record_start(&stream)?;
    // SAFETY: exactly one input allocation has the representation and length
    // selected by `circuit`; all canonical values, dimensions, and sample count
    // were validated above. The live stream and input/output allocations outlast
    // both enqueued kernels through the synchronization below.
    let code = unsafe {
        launch_horizontal_product_micro(
            byte_input
                .as_ref()
                .map_or(std::ptr::null(), |(input, _)| input.as_ptr() as *const u8),
            plane_input
                .as_ref()
                .map_or(std::ptr::null(), |(input, _)| input.as_ptr() as *const u64),
            circuit.raw(),
            branch.raw(),
            n as c_int,
            sample_count as c_int,
            output.as_mut_ptr() as *mut u64,
            stream.as_raw(),
        )
    };
    if code != 0 {
        return Err(HipError::Hip {
            code,
            context: "launch_horizontal_product_micro",
        });
    }
    product.record_stop(&stream)?;

    let compiler_barrier_baseline = HipEventSpan::new_on_device(device_id)?;
    compiler_barrier_baseline.record_start(&stream)?;
    // SAFETY: the circuit, branch, dimensions, and sample count were checked
    // above. `iteration_input` supplies one live device byte per sample and
    // `baseline_output` supplies one live output word per sample; both and the
    // stream remain live through synchronization below.
    let code = unsafe {
        launch_horizontal_product_compiler_barrier_baseline(
            circuit.raw(),
            branch.raw(),
            n as c_int,
            sample_count as c_int,
            iteration_input.as_ptr() as *const u8,
            baseline_output.as_mut_ptr() as *mut u64,
            stream.as_raw(),
        )
    };
    if code != 0 {
        return Err(HipError::Hip {
            code,
            context: "launch_horizontal_product_compiler_barrier_baseline",
        });
    }
    compiler_barrier_baseline.record_stop(&stream)?;
    let mut output_staging = PinnedHostBuffer::<u64>::new(sample_count, device_id)?;
    output.copy_to_pinned_async(&mut output_staging, &stream)?;
    stream.synchronize()?;
    Ok(HorizontalProductTimings {
        product: product.elapsed()?,
        compiler_barrier_baseline: compiler_barrier_baseline.elapsed()?,
        values: output_staging.as_slice().to_vec(),
    })
}

fn encode_horizontal_product_planes(
    circuit: HorizontalProductCircuit,
    values: &[u8],
    n: usize,
) -> Vec<u64> {
    let planes_per_sample = match circuit {
        HorizontalProductCircuit::Bipedal3Halving
        | HorizontalProductCircuit::Bipedal3ZeroMaskSignPopcount => 2,
        HorizontalProductCircuit::F5ThreePlane | HorizontalProductCircuit::F7ThreePlane => 3,
        HorizontalProductCircuit::F5Byte | HorizontalProductCircuit::F7Lookup => {
            unreachable!("byte circuits do not encode planes")
        }
    };
    let mut planes = Vec::with_capacity(values.len() / n * planes_per_sample);
    for sample in values.chunks_exact(n) {
        let mut encoded = [0_u64; 3];
        for (lane, &value) in sample.iter().enumerate() {
            let mask = 1_u64 << lane;
            match circuit {
                HorizontalProductCircuit::Bipedal3Halving
                | HorizontalProductCircuit::Bipedal3ZeroMaskSignPopcount => match value {
                    0 => {}
                    1 => encoded[0] |= mask,
                    2 => {
                        encoded[0] |= mask;
                        encoded[1] |= mask;
                    }
                    _ => unreachable!("input validation established canonical F_3 values"),
                },
                HorizontalProductCircuit::F5ThreePlane | HorizontalProductCircuit::F7ThreePlane => {
                    if value & 1 != 0 {
                        encoded[0] |= mask;
                    }
                    if value & 2 != 0 {
                        encoded[1] |= mask;
                    }
                    if value & 4 != 0 {
                        encoded[2] |= mask;
                    }
                }
                HorizontalProductCircuit::F5Byte | HorizontalProductCircuit::F7Lookup => {
                    unreachable!("byte circuits do not encode planes")
                }
            }
        }
        planes.extend_from_slice(&encoded[..planes_per_sample]);
    }
    planes
}

fn horizontal_product_baseline_iterations(
    circuit: HorizontalProductCircuit,
    values: &[u8],
    n: usize,
    branch: HorizontalProductBranch,
) -> Vec<u8> {
    values
        .chunks_exact(n)
        .map(|sample| match circuit {
            HorizontalProductCircuit::F5Byte | HorizontalProductCircuit::F7Lookup => match branch {
                HorizontalProductBranch::ZeroFast => sample
                    .iter()
                    .position(|&value| value == 0)
                    .expect("branch validation established a zero")
                    .checked_add(1)
                    .expect("n is bounded")
                    as u8,
                HorizontalProductBranch::NonzeroSlow => n as u8,
            },
            HorizontalProductCircuit::Bipedal3ZeroMaskSignPopcount => match branch {
                HorizontalProductBranch::ZeroFast => 1,
                HorizontalProductBranch::NonzeroSlow => 2,
            },
            HorizontalProductCircuit::F5ThreePlane => match branch {
                HorizontalProductBranch::ZeroFast => 1,
                HorizontalProductBranch::NonzeroSlow => 4,
            },
            HorizontalProductCircuit::F7ThreePlane => match branch {
                HorizontalProductBranch::ZeroFast => 1,
                HorizontalProductBranch::NonzeroSlow => 6,
            },
            HorizontalProductCircuit::Bipedal3Halving => {
                unreachable!("the halving circuit has no separated branches")
            }
        })
        .collect()
}

fn operand_len(operand: &GrayUpdateOperand<'_>) -> usize {
    match operand {
        GrayUpdateOperand::Bipedal3 { n, .. } => *n,
        GrayUpdateOperand::Bytes(column) => column.len(),
    }
}

fn validate_bipedal3_active_lanes(mag: u64, sgn: u64, n: usize) {
    let active = (1_u64 << n) - 1;
    assert_eq!(
        (mag | sgn) & !active,
        0,
        "measure_gray_update_kernel: Bipedal3 planes contain inactive high lanes"
    );
}

#[cfg(test)]
mod gray_update_micro_tests {
    use super::{measure_gray_update_kernel, GrayUpdateOperand, PermanentField};

    #[test]
    #[ignore = "sim: requires a HIP device for Gray-update event timing"]
    fn bipedal3_gray_update_returns_two_device_event_spans() {
        let timings = measure_gray_update_kernel(
            PermanentField::F3,
            GrayUpdateOperand::Bipedal3 {
                mag: 0b101,
                sgn: 0b100,
                n: 3,
            },
            1_025,
        )
        .expect("event-timed Bipedal3 Gray update");

        assert!(timings.update.as_nanos() > 0);
        assert!(timings.compiler_barrier_baseline.as_nanos() > 0);
        assert_eq!(
            timings.update_checksum,
            super::GrayUpdateChecksum::Bipedal3 {
                mag: 0b101,
                sgn: 0b100,
            }
        );
    }

    #[test]
    #[should_panic(expected = "inactive high lanes")]
    fn bipedal3_operand_rejects_bits_outside_its_active_domain() {
        super::validate_bipedal3_active_lanes(1 << 3, 0, 3);
    }
}

#[cfg(test)]
mod horizontal_product_micro_tests {
    use super::{HorizontalProductBranch, HorizontalProductCircuit};

    #[test]
    fn circuit_vocabulary_keeps_field_and_branch_contracts_explicit() {
        assert_eq!(HorizontalProductCircuit::Bipedal3Halving.field_order(), 3);
        assert!(!HorizontalProductCircuit::Bipedal3Halving.has_observable_branches());
        assert_eq!(
            HorizontalProductCircuit::Bipedal3ZeroMaskSignPopcount.field_order(),
            3
        );
        assert!(HorizontalProductCircuit::Bipedal3ZeroMaskSignPopcount.has_observable_branches());
        assert_eq!(HorizontalProductCircuit::F5Byte.field_order(), 5);
        assert!(HorizontalProductCircuit::F5ThreePlane.has_observable_branches());
        assert_eq!(HorizontalProductCircuit::F7Lookup.field_order(), 7);
        assert!(HorizontalProductCircuit::F7ThreePlane.has_observable_branches());
        assert_ne!(
            HorizontalProductBranch::ZeroFast,
            HorizontalProductBranch::NonzeroSlow
        );
    }

    #[test]
    fn plane_encoding_preserves_the_canonical_lane_values() {
        assert_eq!(
            super::encode_horizontal_product_planes(
                HorizontalProductCircuit::Bipedal3ZeroMaskSignPopcount,
                &[0, 1, 2],
                3,
            ),
            vec![0b110, 0b100]
        );
        assert_eq!(
            super::encode_horizontal_product_planes(
                HorizontalProductCircuit::F5ThreePlane,
                &[0, 1, 2, 3, 4],
                5,
            ),
            vec![0b01010, 0b01100, 0b10000]
        );
        assert_eq!(
            super::encode_horizontal_product_planes(
                HorizontalProductCircuit::F7ThreePlane,
                &[1, 3, 2, 6, 4, 5],
                6,
            ),
            vec![0b100011, 0b001110, 0b111000]
        );
    }

    #[test]
    fn baseline_geometry_follows_the_actual_circuit_and_branch() {
        use HorizontalProductBranch::{NonzeroSlow, ZeroFast};

        assert_eq!(
            super::horizontal_product_baseline_iterations(
                HorizontalProductCircuit::F5Byte,
                &[1, 0, 4],
                3,
                ZeroFast,
            ),
            vec![2],
            "the byte control reaches its first zero after two multiplies"
        );
        assert_eq!(
            super::horizontal_product_baseline_iterations(
                HorizontalProductCircuit::F7Lookup,
                &[1, 3, 6],
                3,
                NonzeroSlow,
            ),
            vec![3],
            "the lookup control runs all active lanes on its nonzero branch"
        );
        assert_eq!(
            super::horizontal_product_baseline_iterations(
                HorizontalProductCircuit::Bipedal3ZeroMaskSignPopcount,
                &[1, 2, 0],
                3,
                ZeroFast,
            ),
            vec![1]
        );
        assert_eq!(
            super::horizontal_product_baseline_iterations(
                HorizontalProductCircuit::F5ThreePlane,
                &[1, 2, 4],
                3,
                NonzeroSlow,
            ),
            vec![4],
            "one zero-mask plus three C4 exponent population counts"
        );
        assert_eq!(
            super::horizontal_product_baseline_iterations(
                HorizontalProductCircuit::F7ThreePlane,
                &[1, 3, 2],
                3,
                NonzeroSlow,
            ),
            vec![6],
            "one zero-mask plus five C6 exponent population counts"
        );
    }

    #[test]
    #[ignore = "sim: requires a HIP device for horizontal-product event timing"]
    fn f3_zero_mask_product_returns_paired_device_event_spans() {
        let timings = super::measure_horizontal_product_kernel(
            HorizontalProductCircuit::Bipedal3ZeroMaskSignPopcount,
            &[1, 2, 0],
            3,
            HorizontalProductBranch::ZeroFast,
        )
        .expect("event-timed F_3 zero-mask product");
        assert!(timings.product.as_nanos() > 0);
        assert!(timings.compiler_barrier_baseline.as_nanos() > 0);
        assert_eq!(timings.values, vec![0]);
    }
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

/// Runs the F_3 permanent GPU kernel on `m` row-major `n×n` matrices of GF(3)
/// values (`0..=2`) and returns the `m` permanents modulo 3.
///
/// # Panics
///
/// Panics if any HIP runtime call returns a non-zero error code, if `n` is
/// outside `1..=63`, if `m == 0`, or if `host_matrices.len() != m * n * n`.
///
/// # Complexity
///
/// `O(n · 2^n)` GPU work per matrix.
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

/// Runs the F_5 permanent GPU kernel on `m` row-major `n×n` matrices of GF(5)
/// values (`0..=4`) and returns the `m` permanents modulo 5.
///
/// # Panics
///
/// Panics if any HIP runtime call returns a non-zero error code, if `n` is
/// outside `1..=63`, if `m == 0`, or if `host_matrices.len() != m * n * n`.
///
/// # Complexity
///
/// `O(n · 2^n)` GPU work per matrix.
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

/// Runs the F_7 permanent GPU kernel on `m` row-major `n×n` matrices of GF(7)
/// values (`0..=6`) and returns the `m` permanents modulo 7.
///
/// # Panics
///
/// Panics if [`init_permanent_gf7`] has not succeeded, if any HIP runtime
/// call returns a non-zero error code, if `n` is outside `1..=63`, if
/// `m == 0`, or if `host_matrices.len() != m * n * n`.
///
/// # Complexity
///
/// `O(n · 2^n)` GPU work per matrix.
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
    //! Tests of the [`memoise_init_outcome`] state machine without a HIP device.
    use super::{memoise_init_outcome, GF7_INIT_UNINIT};
    use std::sync::atomic::{AtomicI32, Ordering::SeqCst};

    #[test]
    fn test_memoise_init_failed_init_records_rc() {
        let state = AtomicI32::new(GF7_INIT_UNINIT);
        memoise_init_outcome(&state, 7);
        assert_eq!(state.load(SeqCst), 7);
    }

    #[test]
    fn test_memoise_init_successful_init_records_zero() {
        let state = AtomicI32::new(GF7_INIT_UNINIT);
        memoise_init_outcome(&state, 0);
        assert_eq!(state.load(SeqCst), 0);
    }

    #[test]
    fn test_memoise_init_success_overwrites_prior_failure() {
        let state = AtomicI32::new(GF7_INIT_UNINIT);
        memoise_init_outcome(&state, 5);
        assert_eq!(state.load(SeqCst), 5);
        memoise_init_outcome(&state, 0);
        assert_eq!(state.load(SeqCst), 0);
    }

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

    #[test]
    fn test_memoise_init_repeated_success_idempotent() {
        let state = AtomicI32::new(GF7_INIT_UNINIT);
        for _ in 0..16 {
            memoise_init_outcome(&state, 0);
        }
        assert_eq!(state.load(SeqCst), 0);
    }

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
