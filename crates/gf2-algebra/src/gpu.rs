//! HIP/ROCm host-side dispatcher for the GPU permanent kernels.
//!
//! Exposes three batch-permanent functions — [`permanent_batch_bipedal3`],
//! [`permanent_batch_bipedal5`], and [`permanent_batch_bipedal7`] — that route
//! workloads to the F_3 / F_5 / F_7 HIP device kernels in
//! `gf2-kernels-hip::permanent`.
//!
//! # Compilation gate
//!
//! This entire module is compiled only when the `hip` Cargo feature is enabled:
//!
//! ```toml
//! # In Cargo.toml:
//! gf2-algebra = { path = "...", features = ["hip"] }
//! ```
//!
//! **Without `hip`:** the symbols in this module do not exist. Any call site
//! that references `gf2_algebra::gpu::permanent_batch_bipedal3` (or the other
//! two) will produce a **compile error** on non-`hip` builds. This is a
//! deliberate design choice: the processor per-matrix fallback
//! `matrices.iter().map(permanent_bipedal3).collect()` is already available
//! and is unambiguous; wrapping it behind a fake GPU name would be misleading.
//! Callers that need the processor path should call it directly.
//!
//! # Host requirements
//!
//! A ROCm 6.x environment with a gfx1030-class GPU must be present at both
//! build time (hipcc on `PATH`) and runtime (device present). Without it the
//! Rust wrapper compiles but the HIP runtime calls will fail with a non-zero
//! error code, causing a panic per the documented panic policy below.
//!
//! # F_7 LUT initialisation
//!
//! The F_7 GPU kernel requires three 64 KiB look-up tables (ADD, SUB, MUL) to
//! be copied to device memory once before the first compute call. This is done
//! automatically by [`permanent_batch_bipedal7`] on the first invocation via a
//! [`std::sync::OnceLock`]-guarded call to
//! `gf2_kernels_hip::permanent::init_permanent_gf7`. Subsequent calls skip the
//! copy.
//!
//! # Relationship to the processor permanent functions
//!
//! | GPU entry point               | Processor equivalent                                      |
//! |-------------------------------|-----------------------------------------------------------|
//! | [`permanent_batch_bipedal3`]  | `permanent_bipedal3(&mat)` (scalar for `n <= 63`)          |
//! | [`permanent_batch_bipedal5`]  | `permanent_bipedal5(&mat)`                                |
//! | [`permanent_batch_bipedal7`]  | `permanent_bipedal7(&mat)`                                |
//!
//! The GPU functions send a whole batch to the device in a single kernel launch
//! (one block per matrix), so the effective wall-clock cost is
//! `O(ceil(M / num_CUs) * n * 2^n)` rather than `O(M * n * 2^n)` sequentially.
//!
//! # Unsafe isolation
//!
//! This module is compiled under `#![deny(unsafe_code)]` (inherited from
//! `gf2-algebra`'s crate root). All HIP device-memory operations (hipMalloc,
//! hipMemcpy, kernel launch, hipDeviceSynchronize, hipFree) are isolated in
//! `gf2-kernels-hip::permanent::{permanent_gf3_batch_dispatch,
//! permanent_gf5_batch_dispatch, permanent_gf7_batch_dispatch}`, which are
//! safe Rust functions that encapsulate the unsafe FFI internally. This
//! preserves the workspace invariant that `unsafe` lives only in the kernel
//! crates (CLAUDE.md §Architecture, point 3).

#[cfg(feature = "f7")]
use std::sync::OnceLock;

use gf2_core::gfp::Fp;

use crate::packed::bipedal3::Bipedal3Matrix;

#[cfg(feature = "f5")]
use crate::packed::packed5::Packed5Matrix;

#[cfg(feature = "f7")]
use crate::packed::packed7::Packed7Matrix;

#[cfg(feature = "f7")]
use crate::packed::packed7::{ADD_LUT, MUL_LUT, SUB_LUT};

use gf2_kernels_hip::permanent::permanent_gf3_batch_dispatch;

#[cfg(feature = "f5")]
use gf2_kernels_hip::permanent::permanent_gf5_batch_dispatch;

#[cfg(feature = "f7")]
use gf2_kernels_hip::permanent::{init_permanent_gf7_from_slices, permanent_gf7_batch_dispatch};

// ---------------------------------------------------------------------------
// F_7 one-shot LUT init
//
// `GF7_ONCE` ensures `init_permanent_gf7` is called at most once per process.
// The stored i32 is the HIP return code (0 = success, non-zero = error).
// A failure is stored and every subsequent call will panic with the stored rc.
// ---------------------------------------------------------------------------

/// Process-global once-cell that drives the F_7 device LUT upload exactly
/// once per process. Stores the HIP return code from `init_permanent_gf7`:
/// 0 on success, non-zero on a HIP error. Failures are re-panicked on every
/// subsequent [`permanent_batch_bipedal7`] call.
#[cfg(feature = "f7")]
static GF7_ONCE: OnceLock<i32> = OnceLock::new();

/// Ensure the F_7 device LUTs have been uploaded. Invokes
/// `init_gf7_luts_safe` exactly once per process (the result is memoised in
/// `GF7_ONCE`); subsequent calls are no-ops. Panics if the upload returned a
/// non-zero HIP error code.
#[cfg(feature = "f7")]
fn ensure_gf7_luts_initialised() {
    if let Err(rc) = initialise_permanent_gf7_luts() {
        panic!(
            "permanent_batch_bipedal7: init_permanent_gf7 returned HIP error code {rc}. \
             Ensure a gfx1030-class GPU is present and ROCm is initialised."
        );
    }
}

/// Initialise the canonical F_7 permanent LUTs once for a custom launch.
///
/// Ordinary [`permanent_batch_bipedal7`] calls this automatically. This is for
/// a caller that uses the lower-level, stream-owned permanent launch boundary
/// and must establish its F_7 precondition itself.
///
/// # Errors
///
/// Returns the non-zero HIP status from the one-time LUT upload. A failed
/// upload is memoised for the process, so later calls return the same status.
#[cfg(feature = "f7")]
pub fn initialise_permanent_gf7_luts() -> Result<(), i32> {
    let rc = *GF7_ONCE.get_or_init(init_gf7_luts_safe);
    if rc == 0 {
        Ok(())
    } else {
        Err(rc)
    }
}

/// Upload the F_7 LUTs to device memory exactly once and return the HIP rc.
///
/// Called via `GF7_ONCE.get_or_init`; the `OnceLock` guarantees this runs at
/// most once per process. Uses `init_permanent_gf7_from_slices` — the safe
/// wrapper in `gf2-kernels-hip` that accepts `&[u8; 65536]` references
/// instead of raw pointers, making the call site safe.
#[cfg(feature = "f7")]
fn init_gf7_luts_safe() -> i32 {
    init_permanent_gf7_from_slices(&ADD_LUT, &SUB_LUT, &MUL_LUT)
}

// ---------------------------------------------------------------------------
// Serialisation helpers: CPU matrix types → contiguous u8 row-major buffer.
//
// Each GPU kernel expects an n×n row-major byte array per matrix, with one
// `u8` per element. The CPU matrix types store data in packed form; the
// element accessor `mat.get(i, j)` decodes each value on demand.
// ---------------------------------------------------------------------------

/// Serialise a slice of [`Bipedal3Matrix`] into the permanent kernel byte ABI.
///
/// Each matrix contributes `n * n` row-major bytes with values in `{0, 1, 2}`.
/// The returned dimension is the common matrix order.
///
/// # Panics
///
/// Panics if `matrices` is empty, or a matrix has a shape different from the
/// first matrix's square shape.
///
/// # Complexity
///
/// `O(M * n^2)` time and bytes for `M` matrices of order `n`.
pub fn serialise_permanent_bipedal3(matrices: &[Bipedal3Matrix]) -> (Vec<u8>, usize) {
    assert!(
        !matrices.is_empty(),
        "serialise_permanent_bipedal3: matrices must not be empty"
    );
    let n = matrices[0].cols();
    let m = matrices.len();
    let mut buf = Vec::with_capacity(m * n * n);
    for mat in matrices {
        assert_eq!(
            mat.rows(),
            n,
            "serialise_permanent_bipedal3: matrices must be square"
        );
        assert_eq!(
            mat.cols(),
            n,
            "serialise_permanent_bipedal3: matrices must share an order"
        );
        for i in 0..n {
            for j in 0..n {
                buf.push(mat.get(i, j).value() as u8);
            }
        }
    }
    (buf, n)
}

/// Serialise a slice of [`Packed5Matrix`] into the permanent kernel byte ABI.
///
/// Each matrix contributes `n * n` row-major bytes with values in
/// `{0, 1, 2, 3, 4}`. The returned dimension is the common matrix order.
///
/// # Panics
///
/// Panics if `matrices` is empty, or a matrix has a shape different from the
/// first matrix's square shape.
///
/// # Complexity
///
/// `O(M * n^2)` time and bytes for `M` matrices of order `n`.
#[cfg(feature = "f5")]
pub fn serialise_permanent_packed5(matrices: &[Packed5Matrix]) -> (Vec<u8>, usize) {
    assert!(
        !matrices.is_empty(),
        "serialise_permanent_packed5: matrices must not be empty"
    );
    let n = matrices[0].cols();
    let m = matrices.len();
    let mut buf = Vec::with_capacity(m * n * n);
    for mat in matrices {
        assert_eq!(
            mat.rows(),
            n,
            "serialise_permanent_packed5: matrices must be square"
        );
        assert_eq!(
            mat.cols(),
            n,
            "serialise_permanent_packed5: matrices must share an order"
        );
        for i in 0..n {
            for j in 0..n {
                buf.push(mat.get(i, j).value() as u8);
            }
        }
    }
    (buf, n)
}

/// Serialise a slice of [`Packed7Matrix`] into the permanent kernel byte ABI.
///
/// Each matrix contributes `n * n` row-major bytes with values in
/// `{0, 1, 2, 3, 4, 5, 6}`. The returned dimension is the common matrix order.
///
/// # Panics
///
/// Panics if `matrices` is empty, or a matrix has a shape different from the
/// first matrix's square shape.
///
/// # Complexity
///
/// `O(M * n^2)` time and bytes for `M` matrices of order `n`.
#[cfg(feature = "f7")]
pub fn serialise_permanent_packed7(matrices: &[Packed7Matrix]) -> (Vec<u8>, usize) {
    assert!(
        !matrices.is_empty(),
        "serialise_permanent_packed7: matrices must not be empty"
    );
    let n = matrices[0].cols();
    let m = matrices.len();
    let mut buf = Vec::with_capacity(m * n * n);
    for mat in matrices {
        assert_eq!(
            mat.rows(),
            n,
            "serialise_permanent_packed7: matrices must be square"
        );
        assert_eq!(
            mat.cols(),
            n,
            "serialise_permanent_packed7: matrices must share an order"
        );
        for i in 0..n {
            for j in 0..n {
                buf.push(mat.get(i, j).value() as u8);
            }
        }
    }
    (buf, n)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Compute permanents for a batch of `n × n` matrices over **F_3** on the GPU.
///
/// Serialises the matrices into a contiguous row-major byte buffer, copies it
/// to device memory, launches the F_3 HIP permanent kernel (one block per
/// matrix), copies results back, and returns a `Vec<Fp<3>>` of length `M`.
///
/// The underlying GPU kernel is
/// `gf2_kernels_hip::permanent::compute_permanent_gf3_batch` — a
/// Ryser/Gray-code walk with Bipedal3 column-sum arithmetic. All `M` blocks
/// run in parallel so the effective wall-clock is
/// `O(ceil(M / num_CUs) · n · 2^n)`.
///
/// **Without the `hip` Cargo feature this function does not exist.**
/// Code referencing it on non-`hip` builds will fail to compile. The processor
/// fallback is:
///
/// ```rust
/// use gf2_algebra::packed::Bipedal3Matrix;
/// use gf2_algebra::permanent::permanent_bipedal3;
/// use gf2_core::gfp::Fp;
///
/// let matrices: Vec<Bipedal3Matrix> = vec![];
/// let _results: Vec<Fp<3>> = matrices.iter().map(permanent_bipedal3).collect();
/// ```
///
/// # Arguments
///
/// * `matrices` — non-empty slice of square [`Bipedal3Matrix`] instances,
///   all with the same dimension `n`. `n` must satisfy `1 <= n <= 63`.
///
/// # Examples
///
/// ```no_run
/// // Compiles only with the `hip` Cargo feature; never executed under
/// // `cargo test --doc` (requires ROCm + gfx1030 at runtime).
/// # #[cfg(feature = "hip")] {
/// use gf2_algebra::gpu::permanent_batch_bipedal3;
/// use gf2_algebra::packed::Bipedal3Matrix;
/// use gf2_core::gfp::Fp;
///
/// let id: Vec<Fp<3>> = vec![
///     Fp::<3>::new(1), Fp::<3>::new(0),
///     Fp::<3>::new(0), Fp::<3>::new(1),
/// ];
/// let mat = Bipedal3Matrix::from_row_major(&id, 2, 2);
/// let results = permanent_batch_bipedal3(&[mat]);
/// assert_eq!(results[0], Fp::<3>::new(1)); // 2×2 identity: perm = 1
/// # }
/// ```
///
/// # Panics
///
/// * If `matrices` is empty.
/// * If any matrix is not square or `n` differs across the batch.
/// * If `n == 0` or `n > 63` (GPU Gray-walk limit).
/// * If any HIP runtime call (hipMalloc, hipMemcpy, kernel, sync, hipFree)
///   returns a non-zero error code — this indicates no device present or a
///   ROCm driver error.
///
/// # Complexity
///
/// `O(n · 2^n)` GPU work per matrix; all `M` matrices run in parallel.
/// Host overhead: `O(M · n^2)` bytes transferred.
pub fn permanent_batch_bipedal3(matrices: &[Bipedal3Matrix]) -> Vec<Fp<3>> {
    assert!(
        !matrices.is_empty(),
        "permanent_batch_bipedal3: matrices slice must not be empty"
    );
    let n = matrices[0].cols();
    let m = matrices.len();
    for (idx, mat) in matrices.iter().enumerate() {
        assert_eq!(
            mat.rows(),
            n,
            "permanent_batch_bipedal3: matrix[{idx}] rows={} != expected n={n}",
            mat.rows()
        );
        assert_eq!(
            mat.cols(),
            n,
            "permanent_batch_bipedal3: matrix[{idx}] is not square (rows={}, cols={})",
            mat.rows(),
            mat.cols()
        );
    }
    assert!(
        n >= 1,
        "permanent_batch_bipedal3: n must be >= 1, got n = {n}"
    );
    assert!(
        n <= 63,
        "permanent_batch_bipedal3: n must be <= 63 (GPU Gray-walk limit), got n = {n}"
    );

    let (host_buf, _) = serialise_permanent_bipedal3(matrices);
    let raw = permanent_gf3_batch_dispatch(&host_buf, n, m);
    raw.into_iter().map(Fp::<3>::new).collect()
}

/// Compute permanents for a batch of `n × n` matrices over **F_5** on the GPU.
///
/// Serialises the matrices into a contiguous row-major byte buffer, copies it
/// to device memory, launches the F_5 HIP permanent kernel (one block per
/// matrix), copies results back, and returns a `Vec<Fp<5>>` of length `M`.
///
/// The underlying GPU kernel is
/// `gf2_kernels_hip::permanent::compute_permanent_gf5_batch` — a
/// Ryser/Gray-code walk with byte-arithmetic F_5 column sums.
///
/// **Without the `hip` Cargo feature this function does not exist.**
/// The processor fallback is:
///
/// ```rust
/// # #[cfg(feature = "f5")] {
/// use gf2_algebra::packed::Packed5Matrix;
/// use gf2_algebra::permanent::permanent_bipedal5;
/// use gf2_core::gfp::Fp;
///
/// let matrices: Vec<Packed5Matrix> = vec![];
/// let _results: Vec<Fp<5>> = matrices.iter().map(permanent_bipedal5).collect();
/// # }
/// ```
///
/// # Arguments
///
/// * `matrices` — non-empty slice of square [`Packed5Matrix`] instances,
///   all with the same dimension `n`. `n` must satisfy `1 <= n <= 63`.
///
/// # Examples
///
/// ```no_run
/// // Compiles only with the `hip` + `f5` Cargo features; never executed
/// // under `cargo test --doc` (requires ROCm + gfx1030 at runtime).
/// # #[cfg(all(feature = "hip", feature = "f5"))] {
/// use gf2_algebra::gpu::permanent_batch_bipedal5;
/// use gf2_algebra::packed::Packed5Matrix;
/// use gf2_core::gfp::Fp;
///
/// let id: Vec<Fp<5>> = vec![
///     Fp::<5>::new(1), Fp::<5>::new(0),
///     Fp::<5>::new(0), Fp::<5>::new(1),
/// ];
/// let mat = Packed5Matrix::from_row_major(&id, 2, 2);
/// let results = permanent_batch_bipedal5(&[mat]);
/// assert_eq!(results[0], Fp::<5>::new(1)); // 2×2 identity: perm = 1
/// # }
/// ```
///
/// # Panics
///
/// * If `matrices` is empty.
/// * If any matrix is not square or `n` differs across the batch.
/// * If `n == 0` or `n > 63`.
/// * If any HIP runtime call returns a non-zero error code.
///
/// # Complexity
///
/// `O(n · 2^n)` GPU work per matrix. Host overhead: `O(M · n^2)` bytes
/// transferred.
#[cfg(feature = "f5")]
pub fn permanent_batch_bipedal5(matrices: &[Packed5Matrix]) -> Vec<Fp<5>> {
    assert!(
        !matrices.is_empty(),
        "permanent_batch_bipedal5: matrices slice must not be empty"
    );
    let n = matrices[0].cols();
    let m = matrices.len();
    for (idx, mat) in matrices.iter().enumerate() {
        assert_eq!(
            mat.rows(),
            n,
            "permanent_batch_bipedal5: matrix[{idx}] rows={} != expected n={n}",
            mat.rows()
        );
        assert_eq!(
            mat.cols(),
            n,
            "permanent_batch_bipedal5: matrix[{idx}] is not square (rows={}, cols={})",
            mat.rows(),
            mat.cols()
        );
    }
    assert!(
        n >= 1,
        "permanent_batch_bipedal5: n must be >= 1, got n = {n}"
    );
    assert!(
        n <= 63,
        "permanent_batch_bipedal5: n must be <= 63 (GPU Gray-walk limit), got n = {n}"
    );

    let (host_buf, _) = serialise_permanent_packed5(matrices);
    let raw = permanent_gf5_batch_dispatch(&host_buf, n, m);
    raw.into_iter().map(Fp::<5>::new).collect()
}

/// Compute permanents for a batch of `n × n` matrices over **F_7** on the GPU.
///
/// Serialises the matrices into a contiguous row-major byte buffer, initialises
/// the F_7 device LUTs (once per process via [`std::sync::OnceLock`]), launches
/// the F_7 HIP permanent kernel (one block per matrix), copies results back, and
/// returns a `Vec<Fp<7>>` of length `M`.
///
/// The underlying GPU kernel is
/// `gf2_kernels_hip::permanent::compute_permanent_gf7_batch` — a
/// Ryser/Gray-code walk with LUT-based F_7 column-sum arithmetic. The
/// MUL_LUT lives in GPU `__constant__` memory (64 KiB, hardware-cached);
/// ADD_LUT and SUB_LUT live in `__device__` global memory.
///
/// Note: the processor single-word path [`crate::permanent::permanent_bipedal7`]
/// is limited to `n <= 16 = Packed7::LANES`. The GPU path supports up to
/// `n <= 63`.
///
/// **Without the `hip` Cargo feature this function does not exist.**
/// The processor fallback is:
///
/// ```rust
/// # #[cfg(feature = "f7")] {
/// use gf2_algebra::packed::Packed7Matrix;
/// use gf2_algebra::permanent::permanent_bipedal7;
/// use gf2_core::gfp::Fp;
///
/// let matrices: Vec<Packed7Matrix> = vec![];
/// let _results: Vec<Fp<7>> = matrices.iter().map(permanent_bipedal7).collect();
/// # }
/// ```
///
/// # Arguments
///
/// * `matrices` — non-empty slice of square [`Packed7Matrix`] instances,
///   all with the same dimension `n`. `n` must satisfy `1 <= n <= 63`.
///
/// # Examples
///
/// ```no_run
/// // Compiles only with the `hip` + `f7` Cargo features; never executed
/// // under `cargo test --doc` (requires ROCm + gfx1030 at runtime).
/// # #[cfg(all(feature = "hip", feature = "f7"))] {
/// use gf2_algebra::gpu::permanent_batch_bipedal7;
/// use gf2_algebra::packed::Packed7Matrix;
/// use gf2_core::gfp::Fp;
///
/// let id: Vec<Fp<7>> = vec![
///     Fp::<7>::new(1), Fp::<7>::new(0),
///     Fp::<7>::new(0), Fp::<7>::new(1),
/// ];
/// let mat = Packed7Matrix::from_row_major(&id, 2, 2);
/// let results = permanent_batch_bipedal7(&[mat]);
/// assert_eq!(results[0], Fp::<7>::new(1)); // 2×2 identity: perm = 1
/// # }
/// ```
///
/// # Panics
///
/// * If `matrices` is empty.
/// * If any matrix is not square or `n` differs across the batch.
/// * If `n == 0` or `n > 63`.
/// * If the F_7 LUT upload fails (device not present or ROCm error). The
///   failure rc is memoised and re-panicked on every subsequent call.
/// * If any subsequent HIP runtime call returns a non-zero error code.
///
/// # Complexity
///
/// `O(n · 2^n)` GPU work per matrix. Host overhead: `O(M · n^2)` bytes
/// transferred, plus a one-time 3 × 64 KiB LUT upload.
#[cfg(feature = "f7")]
pub fn permanent_batch_bipedal7(matrices: &[Packed7Matrix]) -> Vec<Fp<7>> {
    assert!(
        !matrices.is_empty(),
        "permanent_batch_bipedal7: matrices slice must not be empty"
    );
    let n = matrices[0].cols();
    let m = matrices.len();
    for (idx, mat) in matrices.iter().enumerate() {
        assert_eq!(
            mat.rows(),
            n,
            "permanent_batch_bipedal7: matrix[{idx}] rows={} != expected n={n}",
            mat.rows()
        );
        assert_eq!(
            mat.cols(),
            n,
            "permanent_batch_bipedal7: matrix[{idx}] is not square (rows={}, cols={})",
            mat.rows(),
            mat.cols()
        );
    }
    assert!(
        n >= 1,
        "permanent_batch_bipedal7: n must be >= 1, got n = {n}"
    );
    assert!(
        n <= 63,
        "permanent_batch_bipedal7: n must be <= 63 (GPU Gray-walk limit), got n = {n}"
    );

    // Ensure the F_7 device LUTs are uploaded before the first compute call.
    ensure_gf7_luts_initialised();

    let (host_buf, _) = serialise_permanent_packed7(matrices);
    let raw = permanent_gf7_batch_dispatch(&host_buf, n, m);
    raw.into_iter().map(Fp::<7>::new).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permanent_serialisation_is_canonical_row_major_bytes() {
        let data = [
            Fp::<3>::new(0),
            Fp::<3>::new(1),
            Fp::<3>::new(2),
            Fp::<3>::new(1),
        ];
        let matrix = Bipedal3Matrix::from_row_major(&data, 2, 2);

        let (bytes, n) = serialise_permanent_bipedal3(&[matrix]);

        assert_eq!(n, 2);
        assert_eq!(bytes, [0, 1, 2, 1]);
    }
}
