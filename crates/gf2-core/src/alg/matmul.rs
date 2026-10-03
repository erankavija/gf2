//! GF(2) matrix multiplication dispatch: `BitMatrix` products use M4RM. The
//! Strassen-family implementation is compiled only for tests and the
//! `test-support` feature.

use crate::matrix::BitMatrix;

pub(crate) fn multiply(a: &BitMatrix, b: &BitMatrix) -> BitMatrix {
    crate::alg::m4rm::multiply(a, b)
}
