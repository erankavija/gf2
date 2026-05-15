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

use std::os::raw::c_int;

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

    /// Compute permanents for a batch of M n×n matrices over GF(3) on the GPU.
    ///
    /// Entry point in `hip/permanent/permanent_bipedal3.hip`. Launches one
    /// HIP block per matrix (grid = M × 1 × 1, block = 1 × 1 × 1); only
    /// thread 0 of each block executes the Gray walk. GPU parallelism comes
    /// from M blocks running simultaneously.
    ///
    /// # Arguments
    ///
    /// - `matrices_ptr` — device pointer to `M` consecutive n×n row-major
    ///   arrays of `u8` elements in GF(3) (values 0, 1, 2). Matrix `i`
    ///   starts at `matrices_ptr + i * n * n`.
    /// - `n` — matrix dimension (n×n); must satisfy `1 <= n <= 63`. This is
    ///   a GPU-specific limit: the sequential Gray walk at n=64 would require
    ///   2^64 ≈ 1.8×10^19 steps (~600 years on gfx1030). The CPU reference
    ///   `permanent_bipedal3_singleword` supports n=64 via a u128 counter.
    /// - `m` — number of matrices (batch size); must be `>= 1`.
    /// - `out_ptr` — device pointer to `M` consecutive `u64` outputs. On
    ///   success, `out_ptr[i]` receives the permanent of matrix `i` modulo 3
    ///   (value in `{0, 1, 2}`).
    ///
    /// # Returns
    ///
    /// 0 on success (`hipSuccess`), a non-zero HIP error code otherwise.
    fn permanent_bipedal3_hip_batch(
        matrices_ptr: *const u8,
        n: c_int,
        m: c_int,
        out_ptr: *mut u64,
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

    /// Compute permanents for a batch of M n×n matrices over GF(5) on the GPU.
    ///
    /// Entry point in `hip/permanent/permanent_bipedal5.hip`. Launches one
    /// HIP block per matrix (grid = M × 1 × 1, block = 1 × 1 × 1); only
    /// thread 0 of each block executes the Gray walk. GPU parallelism comes
    /// from M blocks running simultaneously.
    ///
    /// # Arguments
    ///
    /// - `matrices_ptr` — device pointer to `M` consecutive n×n row-major
    ///   arrays of `u8` elements in GF(5) (values 0..4). Matrix `i` starts
    ///   at `matrices_ptr + i * n * n`.
    /// - `n` — matrix dimension (n×n); must satisfy `1 <= n <= 63`. This is
    ///   a GPU-specific limit: the sequential Gray walk at n=64 would require
    ///   2^64 ≈ 1.8×10^19 steps (~600 years on gfx1030). The CPU reference
    ///   `permanent_bipedal5_singleword` was narrowed to n ≤ 63 on 2026-05-15
    ///   for CPU/GPU consistency.
    /// - `m` — number of matrices (batch size); must be `>= 1`.
    /// - `out_ptr` — device pointer to `M` consecutive `u64` outputs. On
    ///   success, `out_ptr[i]` receives the permanent of matrix `i` modulo 5
    ///   (value in `{0, 1, 2, 3, 4}`).
    ///
    /// # Returns
    ///
    /// 0 on success (`hipSuccess`), a non-zero HIP error code otherwise.
    fn permanent_bipedal5_hip_batch(
        matrices_ptr: *const u8,
        n: c_int,
        m: c_int,
        out_ptr: *mut u64,
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

    /// Compute permanents for a batch of M n×n matrices over GF(7) on the GPU.
    ///
    /// Entry point in `hip/permanent/permanent_bipedal7.hip`. Launches one
    /// HIP block per matrix (grid = M × 1 × 1, block = 1 × 1 × 1); only
    /// thread 0 of each block executes the Gray walk. GPU parallelism comes
    /// from M blocks running simultaneously.
    ///
    /// Uses LUT-based F_7 arithmetic: ADD_LUT/SUB_LUT for column-sum updates,
    /// MUL_LUT (in __constant__ memory) for the horizontal fold.
    ///
    /// # Arguments
    ///
    /// - `matrices_ptr` — device pointer to `M` consecutive n×n row-major
    ///   arrays of `u8` elements in GF(7) (values 0..6). Matrix `i` starts
    ///   at `matrices_ptr + i * n * n`.
    /// - `n` — matrix dimension (n×n); must satisfy `1 <= n <= 63`. GPU
    ///   sequential Gray walk at n=64 would take ~600 years on gfx1030.
    /// - `m` — number of matrices (batch size); must be `>= 1`.
    /// - `out_ptr` — device pointer to `M` consecutive `u64` outputs. On
    ///   success, `out_ptr[i]` receives the permanent of matrix `i` modulo 7
    ///   (value in `{0, 1, 2, 3, 4, 5, 6}`).
    ///
    /// # Returns
    ///
    /// 0 on success (`hipSuccess`), a non-zero HIP error code otherwise.
    fn permanent_bipedal7_hip_batch(
        matrices_ptr: *const u8,
        n: c_int,
        m: c_int,
        out_ptr: *mut u64,
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
    unsafe { permanent_bipedal3_hip_batch(matrices_ptr, n, m, out_ptr) }
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
    unsafe { permanent_bipedal5_hip_batch(matrices_ptr, n, m, out_ptr) }
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
    // Record the outcome so subsequent calls to `compute_permanent_gf7_batch`
    // see the same status without re-invoking the FFI. If a prior call
    // succeeded (rc=0), keep that state: the LUTs are populated on the
    // device, and a later init failure (e.g. transient context error)
    // should not invalidate the earlier successful state.
    let prev = GF7_INIT_RC.load(std::sync::atomic::Ordering::SeqCst);
    if prev != 0 {
        // Either uninitialised (sentinel) or a previously-failed init —
        // record this call's rc as the new memoised state.
        GF7_INIT_RC.store(rc, std::sync::atomic::Ordering::SeqCst);
    }
    rc
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
    let init_rc = GF7_INIT_RC.load(std::sync::atomic::Ordering::SeqCst);
    if init_rc != 0 {
        return init_rc;
    }

    // SAFETY: preconditions forwarded verbatim from the caller (see doc comment).
    unsafe { permanent_bipedal7_hip_batch(matrices_ptr, n, m, out_ptr) }
}

/// Compute the byte-sum checksum of the GPU __constant__ MUL_LUT.
///
/// Launches `permanent_bipedal7_lut_checksum_kernel` (a single-thread kernel)
/// that sums all 65 536 bytes of `d_MUL_LUT` and writes the `u64` result to
/// `*out_ptr`. Used by the criterion-3 test
/// `gpu_f7_constant_lut_checksum_matches_host` to verify that the device copy
/// of MUL_LUT is byte-identical to the host static const.
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
