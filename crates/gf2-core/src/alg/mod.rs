//! Matrix algorithms over GF(2).
//!
//! This module provides algorithms for matrices over the binary field GF(2).

pub mod gauss;
pub mod m4rm;
pub(crate) mod matmul;
pub mod rref;
pub(crate) mod strassen;
