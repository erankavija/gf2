//! Probe for the target-feature hierarchy the GF(2^m) test gates rely on
//! (jit:7d44b71f). A function may call a `#[target_feature]` function without
//! `unsafe` only when its own enabled features include the callee's, so this
//! file compiles exactly when each caller's features enable the callee's.
#![crate_type = "lib"]

#[target_feature(enable = "pclmulqdq", enable = "sse4.1")]
pub fn needs_pclmulqdq_sse41() {}

#[target_feature(enable = "avx2", enable = "vpclmulqdq")]
pub fn probed_avx2_vpclmulqdq() {
    needs_pclmulqdq_sse41()
}

#[target_feature(
    enable = "avx2",
    enable = "vpclmulqdq",
    enable = "pclmulqdq",
    enable = "sse4.1"
)]
pub fn needs_all_four() {}

#[target_feature(enable = "avx2", enable = "vpclmulqdq", enable = "pclmulqdq")]
pub fn probed_without_sse41() {
    needs_all_four()
}

#[target_feature(enable = "avx2", enable = "vpclmulqdq")]
pub fn probed_two_of_four() {
    needs_all_four()
}
