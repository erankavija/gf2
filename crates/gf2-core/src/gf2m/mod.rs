//! GF(2^m) - Binary Extension Field Arithmetic
//!
//! This module is re-exported from the field submodule for backward compatibility.

pub mod barrett;
pub mod batch;
mod field;
pub mod generation;
/// Monomorphized u64 GF(2^m) multiplication for formal verification via Charon/Aeneas.
pub mod mul_raw;
pub mod poly_helpers;
mod thread_safety_tests;
pub mod uint_ext;
pub mod wide;
pub mod wide_config;

pub use field::*;
pub use uint_ext::UintExt;
pub use wide::Gf2mWide;
pub use wide_config::Gf2mWideConfig;
