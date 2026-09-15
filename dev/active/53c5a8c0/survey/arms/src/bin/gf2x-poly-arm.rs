//! gf2x arm executable (jit:53c5a8c0).
//!
//! Speaks the canonical child-v2 contract and performs every product with
//! `gf2x_mul_r` from the pinned gf2x build named by `GF2X_PREFIX`. The gf2x
//! binding lives in this binary rather than in the shared library so the gf2
//! arms link nothing external.

#[path = "../gf2x_backend.rs"]
mod gf2x_backend;

fn main() {
    clmul_crossover_arms::run_arm::<gf2x_backend::Gf2xBackend>("gf2x-poly-arm");
}
