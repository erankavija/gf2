//! gf2-side operands and kernels for the byte-field survey (jit:6c6b09b1).
//!
//! Each operation here names the gf2 consumer API a survey cell measures.
//! The byte region is the shared operand format both sides of a comparison
//! start from, so every conversion between it and gf2's representation is
//! performed and charged here rather than assumed away.
//!
//! gf2 offers two GF(2^8) element representations to a `FieldVec` or
//! `FieldMatrix` consumer, and the survey measures both over the same field
//! the externals use, GF(2^8) modulo 0x11D:
//!
//! * [`RuntimeGf256`]: `Gf2mElement` from `Gf2mField::gf256()`, the field
//!   gf2 constructs at run time; each element carries its value beside a
//!   reference-counted handle on the field parameters.
//! * [`WideGf256`]: `Gf2mWide<1, Gf256x11d>`, the compile-time element whose
//!   polynomial comes from a caller-supplied [`Gf2mWideConfig`]; gf2 ships
//!   no GF(2^8) configuration, so the survey declares [`Gf256x11d`].

use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::field::{FieldVec, FiniteField};
use gf2_core::gf2m::{batch, Gf2mElement, Gf2mField, Gf2mWide, Gf2mWideConfig};

/// GF(2^8) modulo `x^8 + x^4 + x^3 + x^2 + 1` (0x11D) as a compile-time
/// configuration: the low eight bits `0x1D`, bit 8 implicit.
pub struct Gf256x11d;

impl Gf2mWideConfig<1> for Gf256x11d {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1D];
    const NAME: &'static str = "Gf256x11d";
}

/// The compile-time GF(2^8) element type the survey measures.
pub type WideElement = Gf2mWide<1, Gf256x11d>;

/// A gf2 GF(2^8) element representation and its byte conversion.
pub trait ByteField {
    /// The element type `FieldVec` and `FieldMatrix` store.
    type Elem: FiniteField;
    /// Name recorded in the arm's selected path.
    const NAME: &'static str;
    /// Full reduction polynomial, for example 0x11D.
    fn polynomial(&self) -> u32;
    /// The element whose canonical bits are `byte`.
    fn element(&self, byte: u8) -> Self::Elem;
    /// The canonical bits of a reduced element.
    fn byte(element: &Self::Elem) -> u8;
    /// The inner route `field::matrix::gemm` takes for this element type on
    /// this host: the branch its source selects (`source-evidence.json`)
    /// given the kernels observed available at run time.
    fn gemm_route() -> &'static str;
}

/// `Gf2mElement` over `Gf2mField::gf256()`.
pub struct RuntimeGf256 {
    pub field: Gf2mField,
}

impl RuntimeGf256 {
    /// The runtime field gf2 constructs for GF(2^8).
    pub fn new() -> Self {
        RuntimeGf256 {
            field: Gf2mField::gf256(),
        }
    }
}

impl Default for RuntimeGf256 {
    fn default() -> Self {
        Self::new()
    }
}

impl ByteField for RuntimeGf256 {
    type Elem = Gf2mElement;
    const NAME: &'static str = "Gf2mElement";

    fn polynomial(&self) -> u32 {
        self.field.primitive_polynomial() as u32
    }

    fn element(&self, byte: u8) -> Gf2mElement {
        self.field.element(u64::from(byte))
    }

    fn byte(element: &Gf2mElement) -> u8 {
        (element.value() & 0xFF) as u8
    }

    fn gemm_route() -> &'static str {
        if batch_kernel_available() {
            "per-cell-batch-dot/vpclmulqdq-batch"
        } else {
            "per-cell-batch-dot/scalar-batch"
        }
    }
}

/// `Gf2mWide<1, Gf256x11d>`.
pub struct WideGf256;

impl ByteField for WideGf256 {
    type Elem = WideElement;
    const NAME: &'static str = "Gf2mWide<1,Gf256x11d>";

    fn polynomial(&self) -> u32 {
        (1 << Gf256x11d::M) | Gf256x11d::MODULUS[0] as u32
    }

    fn element(&self, byte: u8) -> WideElement {
        WideElement::from_u64(u64::from(byte))
    }

    fn byte(element: &WideElement) -> u8 {
        (element.words()[0] & 0xFF) as u8
    }

    fn gemm_route() -> &'static str {
        // `Gf2mWide::try_simd_gemm_classical` takes the whole product for
        // single-word m = 8 and falls back to its scalar panelized GEMM when
        // the kernel is unavailable, so the per-cell path is never reached.
        if gemm_kernel_available() {
            "whole-gemm/vpclmulqdq-gemm"
        } else {
            "whole-gemm/scalar-panelized"
        }
    }
}

/// Converts a byte region into the `FieldVec` that `FieldVec::axpy`
/// consumes.
pub fn pack_vec<B: ByteField>(field: &B, bytes: &[u8]) -> FieldVec<B::Elem> {
    FieldVec::from(
        bytes
            .iter()
            .map(|byte| field.element(*byte))
            .collect::<Vec<_>>(),
    )
}

/// Overwrites an existing `FieldVec` of the same length from a byte region,
/// the conversion a consumer that keeps its vectors pays per call.
pub fn pack_into_vec<B: ByteField>(field: &B, bytes: &[u8], vector: &mut FieldVec<B::Elem>) {
    for (slot, byte) in vector.as_mut_slice().iter_mut().zip(bytes) {
        *slot = field.element(*byte);
    }
}

/// Overwrites an existing row-major `FieldMatrix` from row-major bytes.
pub fn pack_into_matrix<B: ByteField>(field: &B, bytes: &[u8], matrix: &mut FieldMatrix<B::Elem>) {
    let (rows, cols) = matrix.shape();
    for row in 0..rows {
        for (slot, byte) in matrix
            .row_mut(row)
            .iter_mut()
            .zip(&bytes[row * cols..(row + 1) * cols])
        {
            *slot = field.element(*byte);
        }
    }
}

/// Converts a `FieldVec` back into a byte region.
pub fn unpack_vec<B: ByteField>(vector: &FieldVec<B::Elem>, bytes: &mut [u8]) {
    for (slot, element) in bytes.iter_mut().zip(vector.iter()) {
        *slot = B::byte(element);
    }
}

/// Converts a row-major byte matrix into a `FieldMatrix`.
pub fn pack_matrix<B: ByteField>(
    field: &B,
    bytes: &[u8],
    rows: usize,
    cols: usize,
) -> FieldMatrix<B::Elem> {
    FieldMatrix::from_rows(
        (0..rows)
            .map(|row| pack_vec(field, &bytes[row * cols..(row + 1) * cols]))
            .collect(),
    )
}

/// Converts a `FieldMatrix` back into row-major bytes.
pub fn unpack_matrix<B: ByteField>(matrix: &FieldMatrix<B::Elem>, bytes: &mut [u8]) {
    let (rows, cols) = matrix.shape();
    for row in 0..rows {
        for (slot, element) in bytes[row * cols..(row + 1) * cols]
            .iter_mut()
            .zip(matrix.row(row))
        {
            *slot = B::byte(element);
        }
    }
}

/// Fixed-coefficient region multiply-accumulate through `FieldVec::axpy`.
pub fn axpy<F: FiniteField>(y: &mut FieldVec<F>, a: &F, x: &FieldVec<F>) {
    y.axpy(a, x);
}

/// Dense product through the `field::matrix::gemm` entry point.
pub fn matmul<F: FiniteField>(a: &FieldMatrix<F>, b: &FieldMatrix<F>) -> FieldMatrix<F> {
    gemm(a, b)
}

/// Widens a byte region into the `u64` lanes `gf2m::batch::batch_mul` reads.
pub fn widen(bytes: &[u8]) -> Vec<u64> {
    bytes.iter().map(|byte| u64::from(*byte)).collect()
}

/// Widens into an existing lane buffer of the same length.
pub fn widen_into(bytes: &[u8], lanes: &mut [u64]) {
    for (lane, byte) in lanes.iter_mut().zip(bytes) {
        *lane = u64::from(*byte);
    }
}

/// Narrows `u64` lanes back into a byte region.
pub fn narrow(lanes: &[u64], bytes: &mut [u8]) {
    for (slot, lane) in bytes.iter_mut().zip(lanes.iter()) {
        *slot = (*lane & 0xFF) as u8;
    }
}

/// Arbitrary pairwise product through `gf2m::batch::batch_mul`.
pub fn pairwise(field: &Gf2mField, x: &[u64], y: &[u64], out: &mut [u64]) {
    batch::batch_mul(field, x, y, out);
}

/// Whether `batch_mul` and the per-cell `Gf2mElement` dot product reach the
/// dispatched VPCLMULQDQ batch kernel on this host, observed at run time.
pub fn batch_kernel_available() -> bool {
    gf2_core::kernels::simd::maybe_gf2m_batch().is_some()
}

/// Whether the `Gf2mWide` whole-product GEMM kernel is available on this
/// host, observed at run time.
pub fn gemm_kernel_available() -> bool {
    gf2_core::kernels::simd::maybe_gf2m_gemm().is_some()
}
