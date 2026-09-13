//! LDPC code standard-specific implementations.
//!
//! This module contains the core LDPC implementation and factory methods
//! for creating LDPC codes conforming to various industry standards.

mod core;
pub mod dvb_t2;
mod edge_layout;
pub mod encoding;
mod min_sum;
pub mod nr_5g;

// Re-export core types and functions
pub use core::*;
pub use edge_layout::EdgeLayout;
pub use min_sum::{min_sum_check_row, MinSumRule};
