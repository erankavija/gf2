//! FFI shims for per-prime permanent computation kernels.
//!
//! Exposes the F_3 Ryser/Bipedal3 permanent kernel (ad55b777), the F_5
//! direct-byte Ryser kernel (b43cdf33), and the F_7 placeholder (5c0505b2).
//! The F_3 and F_5 kernels are now fully implemented; the F_7 kernel remains
//! a placeholder stub pending its downstream issue.
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
    /// from M blocks in flight simultaneously.
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
    /// from M blocks in flight simultaneously.
    ///
    /// # Arguments
    ///
    /// - `matrices_ptr` — device pointer to `M` consecutive n×n row-major
    ///   arrays of `u8` elements in GF(5) (values 0..4). Matrix `i`
    ///   starts at `matrices_ptr + i * n * n`.
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

    /// Compute the permanent of an n×n matrix over GF(7) on the GPU.
    ///
    /// Entry point implemented in `hip/permanent/permanent_bipedal7.hip`.
    /// The placeholder leaves `*out_ptr` untouched and returns success;
    /// the real implementation
    /// (5c0505b2) will replace the body with a Ryser/BPAS kernel.
    ///
    /// # Arguments
    ///
    /// - `matrix_ptr` — device pointer to an n×n row-major array of `u8`
    ///   elements in GF(7) (values 0..6).
    /// - `n` — matrix dimension (n×n).
    /// - `out_ptr` — device pointer to a single `u64` output that receives
    ///   the permanent value modulo 7.
    ///
    /// # Returns
    ///
    /// 0 on success (`hipSuccess`), a non-zero HIP error code otherwise.
    fn permanent_bipedal7_hip(matrix_ptr: *const u8, n: c_int, out_ptr: *mut u64) -> c_int;
}

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

/// Call the GF(7) permanent kernel.
///
/// # Arguments
///
/// - `matrix_ptr` — device pointer to an `n × n` row-major array of `u8`
///   elements in GF(7) (values `0..=6`).
/// - `n` — matrix dimension (`n × n`); must be `>= 0`.
/// - `out_ptr` — device pointer to a single `u64` that will receive the
///   permanent value modulo 7 once the real kernel lands. The current
///   placeholder returns success without touching `*out_ptr`; downstream
///   issue `5c0505b2` replaces the body.
///
/// # Safety
///
/// - `matrix_ptr` must be a valid device allocation of at least `n * n` bytes,
///   containing GF(7) element values (`0..=6`).
/// - `out_ptr` must be a valid device allocation of at least 8 bytes.
/// - `n` must be non-negative.
/// - The HIP runtime must be initialised and a device context must be active.
///
/// # Examples
///
/// ```ignore
/// # #[cfg(feature = "hip")] {
/// use gf2_kernels_hip::permanent::compute_permanent_gf7;
/// // SAFETY: see the function-level safety contract.
/// let rc = unsafe { compute_permanent_gf7(matrix_ptr, 8, out_ptr) };
/// assert_eq!(rc, 0, "hipSuccess");
/// # }
/// ```
///
/// # Panics
///
/// Never panics from Rust; HIP errors flow through the `c_int` return.
///
/// # Complexity
///
/// `O(2^n)` once the real kernel lands; placeholder is `O(1)`.
pub unsafe fn compute_permanent_gf7(matrix_ptr: *const u8, n: c_int, out_ptr: *mut u64) -> c_int {
    // SAFETY: preconditions forwarded verbatim from the caller (see doc comment).
    unsafe { permanent_bipedal7_hip(matrix_ptr, n, out_ptr) }
}
