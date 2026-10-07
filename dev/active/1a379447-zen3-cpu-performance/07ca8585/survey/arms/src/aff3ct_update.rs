//! AFF3CT check-node pass handles for the isolated kernel arm.
//!
//! A handle wraps one `a3u_*` pass object of `cpp/update_rule_shim.cpp`, which
//! runs the loop AFF3CT's flooding decoder performs over one iteration's
//! check-node updates through AFF3CT's own `tools::Update_rule_*`. Every worker
//! builds its own handle from the recorded AList, as the throughput arm's
//! workers own their own decoder clone.
//!
//! Messages cross this boundary in AFF3CT's variable-major order, which is the
//! order its update rule indexes through [`Handle::transpose`].

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_float, c_int, c_uint, c_void};
use std::path::Path;

extern "C" {
    fn a3u_new(
        alist_path: *const c_char,
        implem: *const c_char,
        norm_factor: c_float,
        offset: c_float,
        err: *mut c_char,
        errlen: c_int,
    ) -> *mut c_void;
    fn a3u_checks(handle: *mut c_void) -> c_int;
    fn a3u_variables(handle: *mut c_void) -> c_int;
    fn a3u_edges(handle: *mut c_void) -> c_int;
    fn a3u_name(handle: *mut c_void) -> *const c_char;
    fn a3u_transpose(handle: *mut c_void, out: *mut c_uint, len: c_int) -> c_int;
    fn a3u_check_pass(
        handle: *mut c_void,
        var_to_chk: *const c_float,
        chk_to_var: *mut c_float,
    ) -> c_int;
    fn a3u_free(handle: *mut c_void);
}

/// An owned AFF3CT check-node pass.
pub struct Handle {
    raw: *mut c_void,
}

// SAFETY: a handle owns its update rule and graph exclusively; the harness
// builds one per worker thread and moves it there.
unsafe impl Send for Handle {}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: `raw` came from `a3u_new` and is freed once.
        unsafe { a3u_free(self.raw) };
    }
}

impl Handle {
    /// Builds a pass over the recorded AList with the normalized min-sum rule.
    ///
    /// # Errors
    ///
    /// Returns AFF3CT's message when it rejects the graph or the rule.
    pub fn build(alist: &Path, normalization_factor: f32) -> Result<Self, String> {
        let path = CString::new(alist.to_string_lossy().as_ref()).map_err(|e| e.to_string())?;
        let implem = CString::new("NMS").map_err(|e| e.to_string())?;
        let mut error = vec![0 as c_char; 512];
        // SAFETY: every pointer is a live NUL-terminated buffer for the call,
        // and the shim reports failure by returning null and filling `error`.
        let raw = unsafe {
            a3u_new(
                path.as_ptr(),
                implem.as_ptr(),
                normalization_factor,
                0.0,
                error.as_mut_ptr(),
                error.len() as c_int,
            )
        };
        if raw.is_null() {
            // SAFETY: the shim wrote a NUL-terminated message into `error`.
            let message = unsafe { CStr::from_ptr(error.as_ptr()) };
            return Err(format!(
                "aff3ct rejected the check-node pass: {}",
                message.to_string_lossy()
            ));
        }
        Ok(Self { raw })
    }

    /// Check nodes of the graph.
    #[must_use]
    pub fn checks(&self) -> usize {
        // SAFETY: `raw` is a live shim handle.
        usize::try_from(unsafe { a3u_checks(self.raw) }).unwrap_or(0)
    }

    /// Variable nodes of the graph.
    #[must_use]
    pub fn variables(&self) -> usize {
        // SAFETY: `raw` is a live shim handle.
        usize::try_from(unsafe { a3u_variables(self.raw) }).unwrap_or(0)
    }

    /// Edges of the graph.
    #[must_use]
    pub fn edges(&self) -> usize {
        // SAFETY: `raw` is a live shim handle.
        usize::try_from(unsafe { a3u_edges(self.raw) }).unwrap_or(0)
    }

    /// AFF3CT's name for the update rule this pass runs.
    #[must_use]
    pub fn name(&self) -> String {
        // SAFETY: `raw` is live and owns the returned string.
        unsafe { CStr::from_ptr(a3u_name(self.raw)) }
            .to_string_lossy()
            .into_owned()
    }

    /// The variable-major slot of each check-major edge, as AFF3CT's flooding
    /// decoder builds it.
    ///
    /// # Errors
    ///
    /// Returns an error when the shim refuses the buffer.
    pub fn transpose(&self) -> Result<Vec<u32>, String> {
        let mut slots = vec![0u32; self.edges()];
        // SAFETY: the buffer holds exactly `a3u_edges` entries.
        let status = unsafe {
            a3u_transpose(
                self.raw,
                slots.as_mut_ptr().cast::<c_uint>(),
                slots.len() as c_int,
            )
        };
        if status == 0 {
            Ok(slots)
        } else {
            Err("the shim refused the transpose buffer".to_owned())
        }
    }

    /// Runs one flooding check-node pass over variable-major arrays.
    ///
    /// # Panics
    ///
    /// Panics when either array is not one frame of the graph's edges, or when
    /// AFF3CT's pass fails.
    pub fn check_pass(&mut self, var_to_chk: &[f32], chk_to_var: &mut [f32]) {
        let edges = self.edges();
        assert_eq!(var_to_chk.len(), edges, "one frame of variable-major edges");
        assert_eq!(chk_to_var.len(), edges, "one frame of variable-major edges");
        // SAFETY: both slices hold exactly `edges` floats, checked above.
        let status =
            unsafe { a3u_check_pass(self.raw, var_to_chk.as_ptr(), chk_to_var.as_mut_ptr()) };
        assert_eq!(status, 0, "AFF3CT check-node pass failed");
    }
}
