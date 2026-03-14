//! GF(2^m) - Binary Extension Field Arithmetic
//!
//! This module is re-exported from the field submodule for backward compatibility.

mod field;
pub mod generation;
/// Monomorphized u64 GF(2^m) multiplication for formal verification via Charon/Aeneas.
pub mod mul_raw;
mod thread_safety_tests;
pub mod uint_ext;

pub use field::*;
pub use uint_ext::UintExt;
