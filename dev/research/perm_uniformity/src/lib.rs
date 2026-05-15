// lib.rs — public surface of the perm-uniformity crate.
//
// Exposes the shared harness (TVD, bootstrap CI, generic cell runner) and the
// minimal PNG encoder so that both src/main.rs and tests/smoke.rs can import
// them without code duplication (JIT 8e4e19a0, D4 fix).

pub mod harness;
pub mod png;
