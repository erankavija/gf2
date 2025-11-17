//! LDPC code standard-specific implementations.
//!
//! This module contains the core LDPC implementation and factory methods
//! for creating LDPC codes conforming to various industry standards.

mod core;
mod dvb_t2;
mod nr_5g;

// Re-export core types and functions
pub use core::*;
