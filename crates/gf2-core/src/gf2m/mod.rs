//! GF(2^m) - Binary Extension Field Arithmetic
//!
//! This module is re-exported from the field submodule for backward compatibility.

mod field;
pub mod generation;
mod thread_safety_tests;

pub use field::*;
