//! BLAS-backed GF(251) cascade — Phase 1 route B prototype harness.
//!
//! JIT issue: `91429c1c` (Phase 1 route B of plan `615db3b9`).
//!
//! This standalone (non-workspace) prototype answers the empirical question:
//! *does a single-threaded `sgemm`-backed cascade clear the
//! fflas-ffpack ceiling for GF(251) at n ∈ {256, 1024}?*
//!
//! It is the closest apples-to-apples comparison against fflas-ffpack's
//! `Modular<float>` + `cblas_sgemm` route, but written from the public
//! CBLAS API only — **no fflas-ffpack source, comments, or autotuning
//! tables are consulted or copied**.
//!
//! # Algorithm (cascade)
//!
//! For a multiplication `C = A · B` with `A: m × k`, `B: k × n` over
//! `GF(251)`:
//!
//! 1. **Pack** `A` and `B` to row-major `f32` arrays with canonical
//!    values in `[0, 251)`.
//! 2. **Chunk along k** in slabs of width `K_CHUNK ≤ 268`. The chunk
//!    bound comes from the f32 mantissa precision constraint
//!    `K_CHUNK · (p-1)² ≤ 2²⁴` (Dumas-Giorgi-Pernet 2009,
//!    arXiv:cs/0601133). For p=251 the bound is
//!    `floor(2²⁴ / 250²) = 268`.
//! 3. For each chunk, call `cblas_sgemm` to produce a partial product
//!    `C_part: m × n` of f32 values in `[0, 2²⁴)`.
//! 4. Reduce each partial product modulo 251 (scalar `% 251` on i32),
//!    accumulate into a running `i32` buffer modulo 251.
//! 5. **Unpack** the final i32 buffer to `FieldMatrix<Fp<251>>` via
//!    `Fp::<251>::new(value)`.
//!
//! # Provenance
//!
//! - CBLAS function signature `cblas_sgemm(...)` taken from the
//!   public OpenBLAS header `/usr/include/openblas/cblas.h` (BSD-3
//!   license) — itself an implementation of the public Netlib CBLAS
//!   reference API (public-domain, www.netlib.org/blas).
//! - The cascade structure (pack-f32, chunked sgemm, scalar Barrett
//!   reduction, unpack) is the standard textbook approach for
//!   "BLAS-backed finite-field GEMM" published in:
//!     - **Dumas, J.-G., Giorgi, P., and Pernet, C.** "Dense Linear
//!       Algebra over Word-Size Prime Fields." ACM TOMS 35(3), 2009;
//!       arXiv:cs/0601133. § 3.2 ("Floating-point matrix
//!       multiplication") gives the chunking bound and the cascade
//!       outline.
//! - No fflas-ffpack source, comments, autotuning tables, or
//!   micro-kernel structure is copied or translated.
//!
//! # License
//!
//! OpenBLAS 0.3.x is BSD-3. The `cblas_sgemm` symbol is part of the
//! public CBLAS API and may be linked freely. No GPL or LGPL code is
//! pulled in by this harness.

#![deny(unsafe_op_in_unsafe_fn)]
// SAFETY note: this is a standalone `dev/research/` prototype (not a
// workspace member). Per the gf2 CLAUDE.md exemption for non-workspace
// research stubs, `unsafe` is permitted with a top-of-function safety
// comment on each `pub unsafe fn`. The only unsafe entries here are the
// `extern "C"` declarations for the CBLAS FFI surface.

use gf2_core::field::matrix::FieldMatrix;
use gf2_core::gfp::Fp;

mod blas_ffi;

pub use blas_ffi::{openblas_get_num_threads, openblas_set_num_threads};

/// Modulus.
pub const P_GF251: u64 = 251;

/// k-chunk width upper bound: `floor(2²⁴ / (p-1)²)` for p=251 is 268.
/// Each chunked sgemm partial product satisfies `0 ≤ C_part[i,j] ≤
/// K_CHUNK · (p-1)² ≤ 2²⁴`, well within f32 exact-integer range.
pub const K_CHUNK: usize = 268;

/// GF(251) cascade GEMM via single-threaded sgemm.
///
/// Returns `A · B` over `GF(251)`. The result is bit-exact equal to
/// `gf2_core::field::matrix::gemm(a, b)` for any GF(251) inputs.
///
/// # Panics
///
/// Panics if `a.cols() != b.rows()`.
///
/// # Threading
///
/// Sets `openblas_set_num_threads(1)` before each `cblas_sgemm` call
/// to ensure apples-to-apples single-threaded measurement (matching
/// fflas-ffpack's `Modular<float>` route in the canonical reference
/// container).
pub fn blas_gf251_gemm(
    a: &FieldMatrix<Fp<P_GF251>>,
    b: &FieldMatrix<Fp<P_GF251>>,
) -> FieldMatrix<Fp<P_GF251>> {
    let m = a.rows();
    let k = a.cols();
    let n = b.cols();
    assert_eq!(k, b.rows(), "blas_gf251_gemm: inner dimensions must match");

    let mut out = FieldMatrix::<Fp<P_GF251>>::zeros(m, n);
    if m == 0 || n == 0 {
        return out;
    }

    // Degenerate k=0 case: the m×n result is the zero matrix
    // (already initialised).
    if k == 0 {
        return out;
    }

    // Enforce single-threaded BLAS for apples-to-apples comparison.
    // SAFETY: `openblas_set_num_threads` is a no-op on hosts without
    // OpenBLAS multithreading. It is safe to call at any time from
    // any thread.
    unsafe {
        openblas_set_num_threads(1);
    }

    // Pack A and B to row-major f32 with canonical values in [0, 251).
    let a_f32 = pack_fp_to_f32(a);
    let b_f32 = pack_fp_to_f32(b);

    // Running accumulator (modulo 251) in i32. The final value at
    // each cell is in [0, 251).
    let mut acc = vec![0i32; m * n];

    // Scratch buffer for one chunk's sgemm output.
    let mut chunk_out = vec![0.0f32; m * n];

    let mut k_off = 0usize;
    while k_off < k {
        let kc = K_CHUNK.min(k - k_off);

        // sgemm(m, n, kc, 1.0, A[:, k_off..k_off+kc], B[k_off..k_off+kc, :], 0.0, chunk_out)
        // Row-major lda = k, ldb = n, ldc = n.
        // A pointer offset: each A row contributes a kc-wide slab at
        //   row-stride k, starting at column k_off.
        // B pointer offset: starts at row k_off (linear k_off * n).
        let a_ptr = unsafe { a_f32.as_ptr().add(k_off) };
        let b_ptr = unsafe { b_f32.as_ptr().add(k_off * n) };

        // Clear chunk_out (sgemm with beta=0 ignores existing values,
        // but be explicit).
        chunk_out.fill(0.0);

        // SAFETY: pointer bounds verified — a_f32 has `m*k` cells;
        // a_ptr points into the (0, k_off) cell with `m` rows of
        // stride `k`, all within the buffer. b_f32 has `k*n` cells;
        // b_ptr at row k_off has `kc * n` cells remaining, fitting in
        // the buffer. chunk_out has `m*n` cells.
        unsafe {
            blas_ffi::cblas_sgemm(
                blas_ffi::CBLAS_ROW_MAJOR,
                blas_ffi::CBLAS_NO_TRANS,
                blas_ffi::CBLAS_NO_TRANS,
                m as i32,
                n as i32,
                kc as i32,
                1.0,
                a_ptr,
                k as i32,
                b_ptr,
                n as i32,
                0.0,
                chunk_out.as_mut_ptr(),
                n as i32,
            );
        }

        // Reduce chunk_out modulo 251 and accumulate into `acc`.
        // The chunk sum bound is `kc · (p-1)² ≤ 268 · 62500 < 2²⁴`,
        // so the f32 value is an exact small integer. Reduction is
        // a scalar i32 `% 251`. After modulo the partial is in
        // [0, 251) and adding to `acc` gives values in [0, 501);
        // a single conditional subtraction rebases to [0, 251).
        for (a_slot, &c_part) in acc.iter_mut().zip(chunk_out.iter()) {
            // f32 cell holds a non-negative integer ≤ 16 750 000.
            // Round to nearest (i32-as-truncation is exact for
            // non-negative small integers, but round-half-to-even
            // is the IEEE 754 default; both are equivalent here).
            let v = c_part as i32;
            debug_assert!(v >= 0, "BLAS sgemm produced a negative value: {}", c_part);
            let r = v % 251;
            let mut sum = *a_slot + r;
            if sum >= 251 {
                sum -= 251;
            }
            *a_slot = sum;
        }

        k_off += kc;
    }

    // Unpack acc into the output FieldMatrix.
    for i in 0..m {
        for j in 0..n {
            let v = acc[i * n + j];
            debug_assert!((0..251).contains(&v));
            out.set(i, j, Fp::<P_GF251>::new(v as u64));
        }
    }

    out
}

/// Packs `m: FieldMatrix<Fp<251>>` to row-major f32 with canonical
/// values in `[0, 251)`. The output has exactly `m.rows() * m.cols()`
/// cells.
///
/// This pack helper extracts canonical values via the public
/// `Fp::value()` accessor; the inverse Montgomery (REDC) is done by
/// gf2-core. No reach into `from_mont_f32` or other crate-private
/// lookup tables — this harness owns its own pack/unpack contract
/// independently of route A.
fn pack_fp_to_f32(m: &FieldMatrix<Fp<P_GF251>>) -> Vec<f32> {
    let rows = m.rows();
    let cols = m.cols();
    let mut out = Vec::with_capacity(rows * cols);
    for i in 0..rows {
        for j in 0..cols {
            // value() returns u64 in [0, 251).
            out.push(m.get(i, j).value() as f32);
        }
    }
    out
}

/// Canonical-byte variant of the cascade.
///
/// Takes `A: m × k` and `B: k × n` as row-major canonical byte
/// slices (each cell in `[0, 251)`), returns the product `C: m × n`
/// as a row-major canonical byte vec. This is the
/// "fflas-ffpack-apples-to-apples" entrypoint — it avoids the
/// Montgomery REDC round-trip on the I/O boundary that
/// `blas_gf251_gemm` pays, because fflas-ffpack's
/// `Modular<float>` storage *is* canonical f32 (no Montgomery
/// encoding on either side of its sgemm).
///
/// # Panics
///
/// Panics if `a.len() != m * k` or `b.len() != k * n` or any byte
/// is `>= 251`.
pub fn blas_gf251_gemm_canonical_bytes(
    a: &[u8],
    m: usize,
    k: usize,
    b: &[u8],
    n: usize,
) -> Vec<u8> {
    assert_eq!(a.len(), m * k, "A shape mismatch");
    assert_eq!(b.len(), k * n, "B shape mismatch");

    if m == 0 || n == 0 {
        return Vec::new();
    }
    let mut out = vec![0u8; m * n];
    if k == 0 {
        return out;
    }

    // SAFETY: openblas_set_num_threads is safe to call.
    unsafe { openblas_set_num_threads(1) };

    // Pack canonical bytes to f32 (one cast per cell, no REDC).
    let mut a_f32 = vec![0.0f32; m * k];
    for (slot, &v) in a_f32.iter_mut().zip(a.iter()) {
        debug_assert!(v < 251);
        *slot = v as f32;
    }
    let mut b_f32 = vec![0.0f32; k * n];
    for (slot, &v) in b_f32.iter_mut().zip(b.iter()) {
        debug_assert!(v < 251);
        *slot = v as f32;
    }

    let mut acc = vec![0i32; m * n];
    let mut chunk_out = vec![0.0f32; m * n];
    let mut k_off = 0usize;
    while k_off < k {
        let kc = K_CHUNK.min(k - k_off);
        let a_ptr = unsafe { a_f32.as_ptr().add(k_off) };
        let b_ptr = unsafe { b_f32.as_ptr().add(k_off * n) };
        chunk_out.fill(0.0);
        // SAFETY: see `blas_gf251_gemm` for the matching pointer
        // bounds argument.
        unsafe {
            blas_ffi::cblas_sgemm(
                blas_ffi::CBLAS_ROW_MAJOR,
                blas_ffi::CBLAS_NO_TRANS,
                blas_ffi::CBLAS_NO_TRANS,
                m as i32,
                n as i32,
                kc as i32,
                1.0,
                a_ptr,
                k as i32,
                b_ptr,
                n as i32,
                0.0,
                chunk_out.as_mut_ptr(),
                n as i32,
            );
        }
        for (a_slot, &c_part) in acc.iter_mut().zip(chunk_out.iter()) {
            let v = c_part as i32;
            debug_assert!(v >= 0);
            let r = v % 251;
            let mut sum = *a_slot + r;
            if sum >= 251 {
                sum -= 251;
            }
            *a_slot = sum;
        }
        k_off += kc;

        // Drop intermediate buffers to keep peak RSS small (no-op
        // for the same `acc` etc. reused across chunks).
    }

    // Pack acc → out u8 (just a downcast, no field arithmetic).
    for (dst, &v) in out.iter_mut().zip(acc.iter()) {
        debug_assert!((0..251).contains(&v));
        *dst = v as u8;
    }
    out
}

/// Convert a `FieldMatrix<Fp<251>>` to a canonical row-major byte vec.
///
/// One `Fp::value()` REDC per cell. Used by callers that want to
/// measure the BLAS cascade in isolation from the Montgomery
/// encoding cost.
pub fn matrix_to_canonical_bytes(m: &FieldMatrix<Fp<P_GF251>>) -> Vec<u8> {
    let r = m.rows();
    let c = m.cols();
    let mut out = vec![0u8; r * c];
    for i in 0..r {
        for j in 0..c {
            out[i * c + j] = m.get(i, j).value() as u8;
        }
    }
    out
}

/// Convert canonical bytes back to `FieldMatrix<Fp<251>>`.
///
/// One `Fp::new` (to_mont) per cell.
pub fn canonical_bytes_to_matrix(
    bytes: &[u8],
    rows: usize,
    cols: usize,
) -> FieldMatrix<Fp<P_GF251>> {
    assert_eq!(bytes.len(), rows * cols);
    let mut out = FieldMatrix::<Fp<P_GF251>>::zeros(rows, cols);
    for i in 0..rows {
        for j in 0..cols {
            let v = bytes[i * cols + j];
            debug_assert!(v < 251);
            out.set(i, j, Fp::<P_GF251>::new(v as u64));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use gf2_core::bench_seed::fp_matrix_from_seed;
    use gf2_core::field::matrix::gemm as core_gemm;

    fn canonical_bytes(m: &FieldMatrix<Fp<P_GF251>>) -> Vec<u8> {
        let mut out = Vec::with_capacity(m.rows() * m.cols());
        for i in 0..m.rows() {
            for j in 0..m.cols() {
                out.push(m.get(i, j).value() as u8);
            }
        }
        out
    }

    #[test]
    fn bit_exact_n_64() {
        let a = fp_matrix_from_seed::<P_GF251>(64, 64, 0xdead_beef_0000_0064);
        let b = fp_matrix_from_seed::<P_GF251>(64, 64, 0xdead_beef_0000_0065);
        let blas_c = blas_gf251_gemm(&a, &b);
        let core_c = core_gemm(&a, &b);
        assert_eq!(canonical_bytes(&blas_c), canonical_bytes(&core_c));
    }

    #[test]
    fn bit_exact_rectangular_small() {
        // Rectangular shape to exercise non-square panel boundaries
        // and the K_CHUNK split (k > K_CHUNK).
        let m = 32;
        let k = 500; // > K_CHUNK = 268 so we cross a chunk boundary
        let n = 48;
        let a = fp_matrix_from_seed::<P_GF251>(m, k, 0xc0ff_eeba_be00_0001);
        let b = fp_matrix_from_seed::<P_GF251>(k, n, 0xc0ff_eeba_be00_0002);
        let blas_c = blas_gf251_gemm(&a, &b);
        let core_c = core_gemm(&a, &b);
        assert_eq!(canonical_bytes(&blas_c), canonical_bytes(&core_c));
    }

    #[test]
    fn bit_exact_k_chunk_boundary() {
        // k exactly equal to K_CHUNK and k = K_CHUNK + 1.
        for k in [K_CHUNK - 1, K_CHUNK, K_CHUNK + 1] {
            let a = fp_matrix_from_seed::<P_GF251>(16, k, 0x9a55_e110_0000_0001);
            let b = fp_matrix_from_seed::<P_GF251>(k, 16, 0x9a55_e110_0000_0002);
            let blas_c = blas_gf251_gemm(&a, &b);
            let core_c = core_gemm(&a, &b);
            assert_eq!(
                canonical_bytes(&blas_c),
                canonical_bytes(&core_c),
                "k={k} mismatch"
            );
        }
    }

    #[test]
    fn degenerate_zero_k() {
        // k=0 should produce the zero m×n matrix.
        let a = FieldMatrix::<Fp<P_GF251>>::zeros(4, 0);
        let b = FieldMatrix::<Fp<P_GF251>>::zeros(0, 5);
        let blas_c = blas_gf251_gemm(&a, &b);
        assert_eq!(blas_c.rows(), 4);
        assert_eq!(blas_c.cols(), 5);
        for i in 0..4 {
            for j in 0..5 {
                assert_eq!(blas_c.get(i, j).value(), 0);
            }
        }
    }

    #[test]
    fn canonical_bytes_matches_fieldmatrix_path() {
        // The two cascade entrypoints must agree byte-for-byte: the
        // canonical-byte path is just the FieldMatrix path with the
        // Montgomery REDCs lifted into separate helpers.
        for n in [16usize, 64, 256] {
            let a = fp_matrix_from_seed::<P_GF251>(n, n, 0xfeed_face_0001);
            let b = fp_matrix_from_seed::<P_GF251>(n, n, 0xfeed_face_0002);
            let via_matrix = blas_gf251_gemm(&a, &b);
            let a_bytes = matrix_to_canonical_bytes(&a);
            let b_bytes = matrix_to_canonical_bytes(&b);
            let c_bytes = blas_gf251_gemm_canonical_bytes(&a_bytes, n, n, &b_bytes, n);
            assert_eq!(canonical_bytes(&via_matrix), c_bytes, "n={n}");
        }
    }

    #[test]
    fn single_threaded_blas_after_set_on_same_thread() {
        // `openblas_set_num_threads(1)` followed by
        // `openblas_get_num_threads()` on the same OS thread must
        // observe 1. (When OpenBLAS is built with OpenMP — as on
        // this host: `OpenBLAS 0.3.33 DYNAMIC_ARCH NO_AFFINITY
        // USE_OPENMP Zen` — `set_num_threads` writes the OpenMP
        // thread-team cap for the current thread; `cargo test`'s
        // default parallel test execution spreads tests across OS
        // threads, so this same-thread invariant is the testable
        // one without serialising the test harness.)
        // SAFETY: both calls are safe by their FFI contracts.
        unsafe {
            openblas_set_num_threads(1);
            let t = openblas_get_num_threads();
            assert_eq!(t, 1, "openblas_set_num_threads(1) did not stick");
        }
        // The cascade itself calls the setter; this is just a
        // belt-and-braces check that the FFI surface works.
        let a = fp_matrix_from_seed::<P_GF251>(8, 8, 1);
        let b = fp_matrix_from_seed::<P_GF251>(8, 8, 2);
        let _c = blas_gf251_gemm(&a, &b);
    }
}
