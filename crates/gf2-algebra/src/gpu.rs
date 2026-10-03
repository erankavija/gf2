//! HIP/ROCm host-side dispatcher for the GPU permanent kernels.
//!
//! [`permanent_batch_bipedal3`], [`permanent_batch_bipedal5`], and
//! [`permanent_batch_bipedal7`] send a batch to the F_3 / F_5 / F_7 HIP device
//! kernels in `gf2-kernels-hip::permanent` in one kernel launch, one block per
//! matrix. The module exists only under the `hip` Cargo feature; the processor
//! equivalent is `permanent_bipedal{3,5,7}` per matrix.
//!
//! # Host requirements
//!
//! The ROCm toolchain and a GPU architecture that `gf2-kernels-hip` supports
//! must be present at build time (hipcc on `PATH`) and at runtime. The batch
//! entry points panic on a failed HIP runtime call; [`has_usable_device`]
//! probes the device without panicking.
//!
//! # F_7 LUT initialisation
//!
//! [`permanent_batch_bipedal7`] copies the three 64 KiB F_7 look-up tables
//! (ADD, SUB, MUL) to device memory once per process through a
//! [`std::sync::OnceLock`].
//!
//! # Unsafe isolation
//!
//! All HIP device-memory operations are safe functions of
//! `gf2-kernels-hip::permanent` that encapsulate the FFI
//! (`@/inv/unsafe-kernel-isolation`).

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

/// Reports whether the HIP runtime can query a usable current accelerator.
///
/// The condition is exactly whether `hipMemGetInfo` succeeds for the current
/// device through the kernel crate's safe host wrapper. A missing device,
/// unavailable runtime, or other failed HIP query reports `false`. This probe
/// never panics; it is intended for callers that must refuse a frozen
/// accelerator selection before invoking a panic-on-dispatch batch entry point.
#[must_use]
pub fn has_usable_device() -> bool {
    gf2_kernels_hip::host::device_mem_info().is_ok()
}

#[cfg(feature = "f5")]
use gf2_kernels_hip::permanent::permanent_gf5_batch_dispatch;

#[cfg(feature = "f7")]
use gf2_kernels_hip::permanent::{init_permanent_gf7_from_slices, permanent_gf7_batch_dispatch};

/// HIP return code of the one-time F_7 device LUT upload (0 on success). A
/// failure is memoised and re-panicked by every [`permanent_batch_bipedal7`]
/// call.
#[cfg(feature = "f7")]
static GF7_ONCE: OnceLock<i32> = OnceLock::new();

/// Uploads the F_7 device LUTs on the first call in the process. Panics if the
/// upload returned a non-zero HIP error code.
#[cfg(feature = "f7")]
fn ensure_gf7_luts_initialised() {
    if let Err(rc) = initialise_permanent_gf7_luts() {
        panic!(
            "permanent_batch_bipedal7: init_permanent_gf7 returned HIP error code {rc}. \
             Ensure a GPU architecture supported by gf2-kernels-hip is present and ROCm is initialised."
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

/// Upload the F_7 LUTs to device memory and return the HIP rc.
#[cfg(feature = "f7")]
fn init_gf7_luts_safe() -> i32 {
    init_permanent_gf7_from_slices(&ADD_LUT, &SUB_LUT, &MUL_LUT)
}

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

/// Compute permanents for a batch of `n × n` matrices over **F_3** on the GPU.
///
/// Launches `gf2_kernels_hip::permanent::compute_permanent_gf3_batch`, a
/// Ryser/Gray-code walk with Bipedal3 column-sum arithmetic, with one block
/// per matrix, and returns a `Vec<Fp<3>>` of length `M`.
///
/// The processor equivalent is:
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
/// # Examples
///
/// ```no_run
/// // Compiles only with the `hip` Cargo feature; never executed under
/// // `cargo test --doc` (requires ROCm and a GPU architecture that gf2-kernels-hip supports at runtime).
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
/// `O(n · 2^n)` GPU work per matrix. Host overhead: `O(M · n^2)` bytes
/// transferred.
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
/// Launches `gf2_kernels_hip::permanent::compute_permanent_gf5_batch`, a
/// Ryser/Gray-code walk with byte-arithmetic F_5 column sums, with one block
/// per matrix, and returns a `Vec<Fp<5>>` of length `M`.
///
/// The processor equivalent is:
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
/// # Examples
///
/// ```no_run
/// // Compiles only with the `hip` + `f5` Cargo features; never executed
/// // under `cargo test --doc` (requires ROCm and a GPU architecture that gf2-kernels-hip supports at runtime).
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
/// Initialises the F_7 device LUTs once per process, launches
/// `gf2_kernels_hip::permanent::compute_permanent_gf7_batch`, a
/// Ryser/Gray-code walk with LUT-based F_7 column-sum arithmetic, with one
/// block per matrix, and returns a `Vec<Fp<7>>` of length `M`.
///
/// The processor single-word path [`crate::permanent::permanent_bipedal7`]
/// is limited to `n <= 16 = Packed7::LANES`; the GPU path supports `n <= 63`.
///
/// The processor equivalent is:
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
/// # Examples
///
/// ```no_run
/// // Compiles only with the `hip` + `f7` Cargo features; never executed
/// // under `cargo test --doc` (requires ROCm and a GPU architecture that gf2-kernels-hip supports at runtime).
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
