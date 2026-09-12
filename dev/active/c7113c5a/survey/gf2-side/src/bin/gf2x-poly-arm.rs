//! gf2x arm executable for the polynomial-multiplication baseline survey
//! (jit:c7113c5a).
//!
//! Speaks the canonical child-v2 contract and performs every operation with
//! `gf2x_mul_r` from the pinned gf2x build named by `GF2X_PREFIX`. The gf2x
//! binding lives in this binary rather than in the shared library so the gf2
//! arm links nothing external.

#[path = "../gf2x_backend.rs"]
mod gf2x_backend;

fn main() {
    poly_baseline_arms::run_arm::<gf2x_backend::Gf2xBackend>("gf2x-poly-arm");
}
