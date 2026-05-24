//! CBLAS FFI surface for OpenBLAS.
//!
//! The function signatures and enum constants below are the public
//! CBLAS reference API (Netlib, public-domain) and match the
//! OpenBLAS-installed system header `/usr/include/openblas/cblas.h`
//! (OpenBLAS itself is BSD-3 licensed).
//!
//! No fflas-ffpack source is consulted; CBLAS predates fflas-ffpack
//! by ~15 years and is a separate well-known public C API.

use std::os::raw::{c_float, c_int};

// ─── CBLAS enum constants ──────────────────────────────────────────
// These integer values are the canonical CBLAS enum encodings used
// by every CBLAS implementation (Netlib reference, OpenBLAS, MKL,
// BLIS). The values come from the public CBLAS header.

/// `CBLAS_ORDER::CblasRowMajor = 101`.
pub const CBLAS_ROW_MAJOR: c_int = 101;
/// `CBLAS_TRANSPOSE::CblasNoTrans = 111`.
pub const CBLAS_NO_TRANS: c_int = 111;

extern "C" {
    /// Single-precision general matrix multiply:
    ///   `C := alpha * op(A) * op(B) + beta * C`.
    ///
    /// Signature taken verbatim from
    /// `/usr/include/openblas/cblas.h` (BSD-3, OpenBLAS 0.3.33),
    /// the canonical public CBLAS API.
    ///
    /// # Safety
    ///
    /// Caller must ensure:
    /// - `A` points to at least `M * K` valid `f32` cells (row-major).
    /// - `B` points to at least `K * N` valid `f32` cells (row-major).
    /// - `C` points to at least `M * N` writable `f32` cells.
    /// - `lda >= K`, `ldb >= N`, `ldc >= N` (row-major; the leading
    ///   dimension is the trailing axis stride).
    /// - `Order`, `TransA`, `TransB` are one of the CBLAS enum
    ///   constants exported by this module.
    pub fn cblas_sgemm(
        Order: c_int,
        TransA: c_int,
        TransB: c_int,
        M: c_int,
        N: c_int,
        K: c_int,
        alpha: c_float,
        A: *const c_float,
        lda: c_int,
        B: *const c_float,
        ldb: c_int,
        beta: c_float,
        C: *mut c_float,
        ldc: c_int,
    );

    /// Sets the global OpenBLAS thread count.
    ///
    /// Defined in
    /// `/usr/include/openblas/cblas.h` (BSD-3, OpenBLAS 0.3.33).
    /// `openblas_set_num_threads(1)` enforces single-threaded
    /// sgemm for apples-to-apples comparison with fflas-ffpack's
    /// pinned single-threaded reference.
    ///
    /// # Safety
    ///
    /// Always safe to call from any thread; OpenBLAS internally
    /// synchronises this call.
    pub fn openblas_set_num_threads(num_threads: c_int);

    /// Returns the current OpenBLAS thread count.
    ///
    /// Defined in
    /// `/usr/include/openblas/cblas.h` (BSD-3, OpenBLAS 0.3.33).
    ///
    /// # Safety
    ///
    /// Always safe to call from any thread.
    pub fn openblas_get_num_threads() -> c_int;
}
