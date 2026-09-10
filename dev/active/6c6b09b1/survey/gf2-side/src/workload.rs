//! gf2-side operands and kernels for the byte-field survey (jit:6c6b09b1).
//!
//! Each operation here names the gf2 consumer API a survey cell measures.
//! The byte region is the shared operand format both sides of a comparison
//! start from, so every conversion between it and gf2's representation is
//! performed and charged here rather than assumed away.

use byte_field_arm_common::{timed, Conversion};
use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::field::FieldVec;
use gf2_core::gf2m::{batch, Gf2mElement, Gf2mField};

/// Converts a byte region into the `FieldVec<Gf2mElement>` that
/// `FieldVec::axpy` consumes.
///
/// This is a real cost: a `Gf2mElement` carries the element value beside a
/// reference-counted handle on the field parameters, so the vector holds far
/// more than the bytes it represents.
pub fn pack_vec(field: &Gf2mField, bytes: &[u8]) -> FieldVec<Gf2mElement> {
    FieldVec::from(
        bytes
            .iter()
            .map(|byte| field.element(u64::from(*byte)))
            .collect::<Vec<_>>(),
    )
}

/// Converts a `FieldVec<Gf2mElement>` back into a byte region.
pub fn unpack_vec(vector: &FieldVec<Gf2mElement>, bytes: &mut [u8]) {
    for (slot, element) in bytes.iter_mut().zip(vector.iter()) {
        *slot = (element.value() & 0xFF) as u8;
    }
}

/// Converts a row-major byte matrix into a `FieldMatrix<Gf2mElement>`.
pub fn pack_matrix(
    field: &Gf2mField,
    bytes: &[u8],
    rows: usize,
    cols: usize,
) -> FieldMatrix<Gf2mElement> {
    FieldMatrix::from_rows(
        (0..rows)
            .map(|row| pack_vec(field, &bytes[row * cols..(row + 1) * cols]))
            .collect(),
    )
}

/// Converts a `FieldMatrix<Gf2mElement>` back into row-major bytes.
pub fn unpack_matrix(matrix: &FieldMatrix<Gf2mElement>, bytes: &mut [u8]) {
    let (rows, cols) = matrix.shape();
    for row in 0..rows {
        for col in 0..cols {
            bytes[row * cols + col] = (matrix.get(row, col).value() & 0xFF) as u8;
        }
    }
}

/// Widens a byte region into the `u64` lanes `gf2m::batch::batch_mul` reads.
pub fn widen(bytes: &[u8]) -> Vec<u64> {
    bytes.iter().map(|byte| u64::from(*byte)).collect()
}

/// Narrows `u64` lanes back into a byte region.
pub fn narrow(lanes: &[u64], bytes: &mut [u8]) {
    for (slot, lane) in bytes.iter_mut().zip(lanes.iter()) {
        *slot = (*lane & 0xFF) as u8;
    }
}

/// Fixed-coefficient region multiply-accumulate through `FieldVec::axpy`.
pub fn axpy(y: &mut FieldVec<Gf2mElement>, a: &Gf2mElement, x: &FieldVec<Gf2mElement>) {
    y.axpy(a, x);
}

/// Arbitrary pairwise product through `gf2m::batch::batch_mul`.
pub fn pairwise(field: &Gf2mField, x: &[u64], y: &[u64], out: &mut [u64]) {
    batch::batch_mul(field, x, y, out);
}

/// Dense product through the `field::matrix::gemm` entry point.
pub fn matmul(
    a: &FieldMatrix<Gf2mElement>,
    b: &FieldMatrix<Gf2mElement>,
) -> FieldMatrix<Gf2mElement> {
    gemm(a, b)
}

/// Measures the one-off cost of building the field context.
///
/// Both binaries in this crate compile this module; only the arm charges a
/// setup cost, so the conformance binary leaves this function unused.
#[allow(dead_code)]
pub fn setup(poly: u32) -> (Gf2mField, Conversion) {
    let (field, setup_ns) = timed(|| Gf2mField::new(8, u64::from(poly)));
    (
        field,
        Conversion {
            setup_ns,
            ..Conversion::default()
        },
    )
}
