//! Planning-time feasibility prototype for the two residual-shift forms JIT
//! issue `85fc5ff4` nominates (REQ-05). It exists to be compiled by Rust 1.95
//! and read as assembly, and to prove that a runtime-gated form degrades to
//! the scalar funnel when the capability is absent. It authorizes no
//! production source, runs no timing and lives outside the gf2 workspace.
//!
//! Semantics match `gf2_core::BitVec`: canonical little-endian bit indexing
//! (bit `i` is bit `i % 64` of word `i / 64`), zero fill, bits shifted past
//! the length lost, and the tail above `len_bits` held at zero.

pub mod avx2;
pub mod bmi2;
pub mod reference;
pub mod scalar;

/// Which funnel a call is asked for. `select` maps an unavailable form onto
/// [`Form::Scalar`], and every entry point reports the form it actually ran.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    Scalar,
    Avx2,
    Bmi2Pair,
    Bmi2Dp,
}

impl Form {
    /// Every form, so a test can sweep the whole family.
    pub const ALL: [Form; 4] = [Form::Scalar, Form::Avx2, Form::Bmi2Pair, Form::Bmi2Dp];
}

/// Words a `len_bits`-long vector occupies.
pub fn words_for(len_bits: usize) -> usize {
    len_bits.div_ceil(64)
}

/// The runtime capability gate. This is the whole of the gating: a form whose
/// feature the host lacks becomes [`Form::Scalar`] before any dispatch, so the
/// fallback is reachable on every host rather than only on non-x86 builds.
#[cfg(target_arch = "x86_64")]
pub fn select(requested: Form) -> Form {
    match requested {
        Form::Avx2 if !is_x86_feature_detected!("avx2") => Form::Scalar,
        Form::Bmi2Pair | Form::Bmi2Dp if !is_x86_feature_detected!("bmi2") => Form::Scalar,
        other => other,
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub fn select(_requested: Form) -> Form {
    Form::Scalar
}

fn mask_tail(data: &mut [u64], len_bits: usize) {
    let rem = len_bits % 64;
    if rem != 0 {
        if let Some(last) = data.last_mut() {
            *last &= (1u64 << rem) - 1;
        }
    }
}

/// Shifts `data` left by `k` bits and returns the form that ran.
///
/// `data.len()` must be [`words_for(len_bits)`](words_for).
pub fn shift_left(requested: Form, data: &mut [u64], len_bits: usize, k: usize) -> Form {
    let used = select(requested);
    if k == 0 || len_bits == 0 {
        return used;
    }
    if k >= len_bits {
        data.fill(0);
        return used;
    }
    let ws = k / 64;
    let b = (k % 64) as u32;
    if b == 0 {
        for i in (ws..data.len()).rev() {
            data[i] = data[i - ws];
        }
    } else {
        match used {
            Form::Scalar => scalar::shift_left_funnel(data, ws, b),
            // SAFETY: `select` has just confirmed the feature at run time, and
            // `ws < data.len()`, `b` in `1..64` hold from the `k < len_bits`
            // split above.
            Form::Avx2 => unsafe { avx2::shift_left_funnel_avx2(data, ws, b) },
            Form::Bmi2Pair => unsafe { bmi2::shift_left_funnel_bmi2_pair(data, ws, b) },
            Form::Bmi2Dp => unsafe { bmi2::shift_left_funnel_bmi2_dp(data, ws, b) },
        }
        // The funnel leaves `data[ws]` alone: it has no lower neighbour.
        data[ws] = data[0] << b;
    }
    data[..ws].fill(0);
    mask_tail(data, len_bits);
    used
}

/// Shifts `data` right by `k` bits and returns the form that ran.
///
/// `data.len()` must be [`words_for(len_bits)`](words_for).
pub fn shift_right(requested: Form, data: &mut [u64], len_bits: usize, k: usize) -> Form {
    let used = select(requested);
    if k == 0 || len_bits == 0 {
        return used;
    }
    if k >= len_bits {
        data.fill(0);
        return used;
    }
    let n = data.len();
    let ws = k / 64;
    let b = (k % 64) as u32;
    if b == 0 {
        for i in 0..n - ws {
            data[i] = data[i + ws];
        }
    } else {
        match used {
            Form::Scalar => scalar::shift_right_funnel(data, ws, b),
            // SAFETY: as in `shift_left`.
            Form::Avx2 => unsafe { avx2::shift_right_funnel_avx2(data, ws, b) },
            Form::Bmi2Pair => unsafe { bmi2::shift_right_funnel_bmi2_pair(data, ws, b) },
            Form::Bmi2Dp => unsafe { bmi2::shift_right_funnel_bmi2_dp(data, ws, b) },
        }
        // The topmost surviving word has no higher neighbour.
        data[n - ws - 1] = data[n - 1] >> b;
    }
    data[n - ws..].fill(0);
    mask_tail(data, len_bits);
    used
}
