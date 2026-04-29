//! High-level GF(2) matrix multiplication dispatch.
//!
//! This layer preserves the existing M4RM implementation for production
//! `BitMatrix` multiplication. A Strassen-family implementation exists for
//! test/benchmark forcing, but it is not automatically dispatched until a
//! measured crossover demonstrates that it wins.

use crate::matrix::BitMatrix;

pub(crate) fn multiply(a: &BitMatrix, b: &BitMatrix) -> BitMatrix {
    crate::alg::m4rm::multiply(a, b)
}
