//! Safe wrapper over the byte-field C shim (jit:6c6b09b1).
//!
//! The shim in `../byte_field_ext.c` gives ISA-L, GF-Complete and M4RIE the
//! same entry points and reports honestly which operations a library does
//! not provide. This module adds Rust ownership on top and translates the
//! shim's status codes into typed errors; it performs no arithmetic.

use std::ffi::{c_char, c_int, c_uchar, c_uint, CStr, CString};
use std::ptr;

pub const BACKEND_ISAL: c_int = 1;
pub const BACKEND_GFCOMPLETE: c_int = 2;
pub const BACKEND_M4RIE: c_int = 3;

const OK: c_int = 0;
const UNSUPPORTED: c_int = 1;

#[repr(C)]
struct CtxOpaque {
    _private: [u8; 0],
}

#[repr(C)]
struct MatOpaque {
    _private: [u8; 0],
}

extern "C" {
    fn bfx_init(
        backend: c_int,
        poly: c_uint,
        variant: *const c_char,
        why: *mut *const c_char,
    ) -> *mut CtxOpaque;
    fn bfx_free(ctx: *mut CtxOpaque);
    fn bfx_backend_name(ctx: *const CtxOpaque) -> *const c_char;
    fn bfx_library_version(ctx: *const CtxOpaque) -> *const c_char;
    fn bfx_prepare(ctx: *mut CtxOpaque, a: c_uchar) -> c_int;
    fn bfx_axpy_apply(
        ctx: *mut CtxOpaque,
        src: *const c_uchar,
        dest: *mut c_uchar,
        len: usize,
    ) -> c_int;
    fn bfx_mul_pairwise(
        ctx: *mut CtxOpaque,
        x: *const c_uchar,
        y: *const c_uchar,
        dest: *mut c_uchar,
        len: usize,
    ) -> c_int;
    fn bfx_has_separate_prepare(ctx: *const CtxOpaque) -> c_int;
    fn bfx_ref_mul(a: c_uchar, b: c_uchar, poly: c_uint) -> c_uchar;
    fn bfx_mat_new(ctx: *mut CtxOpaque, rows: usize, cols: usize) -> *mut MatOpaque;
    fn bfx_mat_free(mat: *mut MatOpaque);
    fn bfx_mat_pack(mat: *mut MatOpaque, bytes: *const c_uchar) -> c_int;
    fn bfx_mat_unpack(mat: *const MatOpaque, bytes: *mut c_uchar) -> c_int;
    fn bfx_mat_mul(c: *mut MatOpaque, a: *const MatOpaque, b: *const MatOpaque) -> c_int;
    fn bfx_mat_row_axpy(
        dest: *mut MatOpaque,
        dest_row: usize,
        src: *const MatOpaque,
        src_row: usize,
        a: c_uchar,
    ) -> c_int;
    fn bfx_encode_prepare(ctx: *mut CtxOpaque, g: *const c_uchar, k: c_int, rows: c_int) -> c_int;
    fn bfx_encode_apply(
        ctx: *mut CtxOpaque,
        len: c_int,
        k: c_int,
        rows: c_int,
        data: *mut *mut c_uchar,
        coding: *mut *mut c_uchar,
    ) -> c_int;
}

/// An initialised backend context.
pub struct Context {
    ptr: *mut CtxOpaque,
    name: String,
    version: String,
}

impl Context {
    /// Initialises one backend for the given full reduction polynomial.
    ///
    /// Returns the shim's own reason when a backend refuses the request, so
    /// an incompatible pairing surfaces as that reason rather than as a
    /// silent fallback to a different field or kernel.
    pub fn open(backend: c_int, poly: u32, variant: &str) -> Result<Self, String> {
        let variant = CString::new(variant).map_err(|_| "variant contains a NUL".to_owned())?;
        let mut why: *const c_char = ptr::null();
        // SAFETY: `variant` outlives the call, `why` is a valid out pointer,
        // and the shim either returns a context it owns or NULL with `why`
        // pointing at one of its static reason strings.
        let ptr = unsafe { bfx_init(backend, poly as c_uint, variant.as_ptr(), &mut why) };
        if ptr.is_null() {
            let reason = if why.is_null() {
                "the backend refused the request".to_owned()
            } else {
                // SAFETY: non-NULL `why` points at a static NUL-terminated
                // string owned by the shim.
                unsafe { CStr::from_ptr(why) }
                    .to_string_lossy()
                    .into_owned()
            };
            return Err(reason);
        }
        // SAFETY: `ptr` is a live context and both accessors return static
        // storage inside it.
        let (name, version) = unsafe {
            (
                CStr::from_ptr(bfx_backend_name(ptr))
                    .to_string_lossy()
                    .into_owned(),
                CStr::from_ptr(bfx_library_version(ptr))
                    .to_string_lossy()
                    .into_owned(),
            )
        };
        Ok(Context { ptr, name, version })
    }

    /// Runtime-observed identity of the arithmetic backend selected.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Version of the library actually linked.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Whether [`Context::prepare`] is a table preparation separate from the
    /// region call; otherwise it only records the coefficient.
    pub fn has_separate_prepare(&self) -> bool {
        // SAFETY: `self.ptr` is a live context.
        unsafe { bfx_has_separate_prepare(self.ptr) != 0 }
    }

    /// Performs the table preparation one coefficient needs.
    pub fn prepare(&mut self, coefficient: u8) -> Result<(), String> {
        // SAFETY: `self.ptr` is a live context.
        check(unsafe { bfx_prepare(self.ptr, coefficient) }, "prepare")
    }

    /// Accumulates `dest[i] ^= a * src[i]` with the prepared coefficient.
    pub fn axpy(&mut self, src: &[u8], dest: &mut [u8]) -> Result<(), String> {
        let len = src.len().min(dest.len());
        // SAFETY: both slices are valid for `len` bytes, they are distinct
        // borrows so they do not overlap, and the shim reads `src` and
        // writes `dest` only within that length.
        check(
            unsafe { bfx_axpy_apply(self.ptr, src.as_ptr(), dest.as_mut_ptr(), len) },
            "region multiply-XOR",
        )
    }

    /// Computes `dest[i] = x[i] * y[i]` with no coefficient reuse.
    pub fn pairwise(&mut self, x: &[u8], y: &[u8], dest: &mut [u8]) -> Result<(), String> {
        let len = x.len().min(y.len()).min(dest.len());
        // SAFETY: three distinct borrows, each valid for `len` bytes.
        check(
            unsafe { bfx_mul_pairwise(self.ptr, x.as_ptr(), y.as_ptr(), dest.as_mut_ptr(), len) },
            "arbitrary pairwise multiply",
        )
    }

    /// Prepares the generator tables for a `rows x k` coefficient matrix.
    pub fn encode_prepare(
        &mut self,
        generator: &[u8],
        k: usize,
        rows: usize,
    ) -> Result<(), String> {
        // SAFETY: `generator` is valid for `k * rows` bytes, which the shim
        // reads and does not retain.
        check(
            unsafe { bfx_encode_prepare(self.ptr, generator.as_ptr(), k as c_int, rows as c_int) },
            "generator table preparation",
        )
    }

    /// Encodes `rows` output regions from `k` input regions of `len` bytes.
    pub fn encode(
        &mut self,
        len: usize,
        data: &mut [*mut u8],
        coding: &mut [*mut u8],
    ) -> Result<(), String> {
        // SAFETY: the caller passes pointers into live, distinct buffers of
        // at least `len` bytes each, matching the counts the shim was
        // prepared with.
        check(
            unsafe {
                bfx_encode_apply(
                    self.ptr,
                    len as c_int,
                    data.len() as c_int,
                    coding.len() as c_int,
                    data.as_mut_ptr(),
                    coding.as_mut_ptr(),
                )
            },
            "generator encode",
        )
    }

    /// Allocates a dense matrix in the backend's own layout.
    pub fn matrix(&mut self, rows: usize, cols: usize) -> Result<Matrix, String> {
        // SAFETY: `self.ptr` is a live context; NULL means the backend has
        // no dense matrix type.
        let ptr = unsafe { bfx_mat_new(self.ptr, rows, cols) };
        if ptr.is_null() {
            return Err(format!(
                "{} carries no dense GF(2^8) matrix type",
                self.name
            ));
        }
        Ok(Matrix { ptr, rows, cols })
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        // SAFETY: `self.ptr` was returned by `bfx_init` and is freed once.
        unsafe { bfx_free(self.ptr) }
    }
}

/// A dense matrix owned by the backend.
pub struct Matrix {
    ptr: *mut MatOpaque,
    rows: usize,
    cols: usize,
}

impl Matrix {
    /// Converts row-major bytes into the backend's layout.
    pub fn pack(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() < self.rows * self.cols {
            return Err("pack source is shorter than the matrix".to_owned());
        }
        // SAFETY: `bytes` is valid for at least `rows * cols` elements,
        // which is exactly what the shim reads.
        check(
            unsafe { bfx_mat_pack(self.ptr, bytes.as_ptr()) },
            "matrix pack",
        )
    }

    /// Converts the backend's layout back into row-major bytes.
    pub fn unpack(&self, bytes: &mut [u8]) -> Result<(), String> {
        if bytes.len() < self.rows * self.cols {
            return Err("unpack target is shorter than the matrix".to_owned());
        }
        // SAFETY: `bytes` is valid for at least `rows * cols` elements,
        // which is exactly what the shim writes.
        check(
            unsafe { bfx_mat_unpack(self.ptr, bytes.as_mut_ptr()) },
            "matrix unpack",
        )
    }

    /// Computes `self = a * b`.
    pub fn mul(&mut self, a: &Matrix, b: &Matrix) -> Result<(), String> {
        // SAFETY: three live matrices from the same context; the shim
        // rejects mismatched shapes itself.
        check(
            unsafe { bfx_mat_mul(self.ptr, a.ptr, b.ptr) },
            "matrix multiply",
        )
    }

    /// Accumulates `self[dest_row] ^= a * src[src_row]`.
    pub fn row_axpy(
        &mut self,
        dest_row: usize,
        src: &Matrix,
        src_row: usize,
        a: u8,
    ) -> Result<(), String> {
        // SAFETY: both matrices are live and the row indices are within the
        // shapes the caller allocated.
        check(
            unsafe { bfx_mat_row_axpy(self.ptr, dest_row, src.ptr, src_row, a) },
            "row region multiply-XOR",
        )
    }
}

impl Drop for Matrix {
    fn drop(&mut self) {
        // SAFETY: `self.ptr` was returned by `bfx_mat_new` and is freed once.
        unsafe { bfx_mat_free(self.ptr) }
    }
}

/// Independent scalar multiply, for cross-checking a region result.
pub fn reference_mul(a: u8, b: u8, poly: u32) -> u8 {
    // SAFETY: a pure function over scalars with no pointer arguments.
    unsafe { bfx_ref_mul(a, b, poly as c_uint) }
}

fn check(status: c_int, what: &str) -> Result<(), String> {
    match status {
        OK => Ok(()),
        UNSUPPORTED => Err(format!("the backend does not provide {what}")),
        other => Err(format!("{what} failed with status {other}")),
    }
}
