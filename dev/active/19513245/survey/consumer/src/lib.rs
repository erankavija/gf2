//! Byte-field prototype and consumer routes for the feasibility assessment
//! (jit:19513245).
//!
//! The question this crate exists to measure is whether byte-oriented
//! GF(2^8) arithmetic improves gf2's existing vector and matrix consumers
//! after every conversion cost. Two routes answer it for each consumer:
//!
//! * the **current** route, the gf2-core entry point itself, reached through
//!   [`byte_field_gf2_side::workload`] so the baseline is the same call the
//!   byte-field comparison survey measured;
//! * the **prototype** route in [`table`], byte-oriented arithmetic over the
//!   same field: one 256-byte multiplication table per reused coefficient,
//!   or the full 256-by-256 table where no coefficient repeats.
//!
//! The prototype is a bounded feasibility experiment, not a production
//! path. It is safe scalar Rust and uses no intrinsic, so it compiles at the
//! repository MSRV by construction; `consumer-verify` records that and the
//! separate MSRV evidence covers the intrinsics a vectorised successor would
//! need. No gf2-core file changes for it.
//!
//! # Why a table is byte-oriented arithmetic
//!
//! A GF(2^8) element fits in a byte, so the product of a *fixed* coefficient
//! `a` with every field element is a 256-entry byte array. Multiplying by
//! `a` then costs one indexed load, and accumulating costs one XOR, whatever
//! the element's storage width: the table is indexed by the low byte of a
//! `u64` lane exactly as it is indexed by a packed region byte. That is the
//! property the assessment turns on, because it separates byte-oriented
//! *arithmetic* from byte-oriented *storage*.

//! Every prototype route carries `#[inline(never)]`. A route that the
//! compiler folded into its caller would leave no symbol in the measured
//! executable, and the assembly artefact of this assessment has to show the
//! code that ran; the cost is one call instruction per invocation of an
//! `O(n)` loop, which no cell's window can resolve. A production hook would
//! cross a crate boundary anyway.

use byte_field_gf2_side::workload::ByteField;
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::FieldVec;

pub mod table;

pub use table::{CoefficientTable, ProductTable};

/// Fixed-coefficient region multiply-accumulate `y[i] += a * x[i]` over a
/// byte region, the prototype's region-shaped form.
///
/// `table` is the multiplication table of the coefficient. The regions are
/// separate slices, which is the overlap contract
/// [`FieldVec::axpy`](gf2_core::field::FieldVec::axpy) enforces through its
/// borrows: the destination is `&mut` and the source `&`, so they cannot
/// alias, and this function documents and requires the same. It reads `x`
/// and writes `y` in ascending index order, so a caller that nevertheless
/// passes overlapping raw regions through a copy would see forward-copy
/// semantics; no caller in this survey does.
///
/// # Panics
///
/// Panics if the two regions have different lengths.
#[inline(never)]
pub fn axpy_region(y: &mut [u8], table: &CoefficientTable, x: &[u8]) {
    assert_eq!(y.len(), x.len(), "axpy_region: length mismatch");
    for (target, source) in y.iter_mut().zip(x.iter()) {
        *target ^= table.get(*source);
    }
}

/// Fixed-coefficient multiply-accumulate in place on the `FieldVec` the
/// consumer already holds, the prototype's vector-shaped form.
///
/// No representation conversion happens: the element's canonical byte
/// indexes the table and the result is written back through the same
/// representation the vector stores. `FieldVec<Gf2mElement>` therefore pays
/// one reference-counted field-handle clone per element and
/// `FieldVec<Gf2mWide<1, _>>` pays nothing, which is the asymmetry the cells
/// measure.
///
/// # Panics
///
/// Panics if the two vectors have different lengths.
#[inline(never)]
pub fn axpy_field_vec<B: ByteField>(
    field: &B,
    y: &mut FieldVec<B::Elem>,
    table: &CoefficientTable,
    x: &FieldVec<B::Elem>,
) {
    assert_eq!(y.len(), x.len(), "axpy_field_vec: length mismatch");
    let source = x.as_slice();
    for (index, target) in y.as_mut_slice().iter_mut().enumerate() {
        let value = B::byte(target) ^ table.get(B::byte(&source[index]));
        *target = field.element(value);
    }
}

/// Dense product `C = A * B` over row-major byte matrices, accumulating by
/// XOR into a zeroed output.
///
/// The traversal makes every coefficient a reused one: for each output row
/// `i` and each inner index `p`, `A[i][p]` is fixed over the whole row
/// `B[p][..]`, so the kernel is `k` region multiply-accumulates per output
/// row. `table` supplies all 256 coefficient tables at once, so no table is
/// built inside the product.
///
/// # Panics
///
/// Panics if any operand length disagrees with the declared shape.
#[inline(never)]
pub fn gemm_region(
    a: &[u8],
    b: &[u8],
    c: &mut [u8],
    m: usize,
    k: usize,
    n: usize,
    table: &ProductTable,
) {
    assert_eq!(a.len(), m * k, "gemm_region: left operand shape");
    assert_eq!(b.len(), k * n, "gemm_region: right operand shape");
    assert_eq!(c.len(), m * n, "gemm_region: output shape");
    c.fill(0);
    for i in 0..m {
        let out = &mut c[i * n..(i + 1) * n];
        for p in 0..k {
            let row = table.row(a[i * k + p]);
            for (target, source) in out.iter_mut().zip(&b[p * n..(p + 1) * n]) {
                *target ^= row[usize::from(*source)];
            }
        }
    }
}

/// Matrix-vector product `y = A * x` over a row-major byte matrix.
///
/// Row-major traversal reuses no coefficient, so each product is one lookup
/// in the full table rather than in a coefficient table. This is the form
/// the profile measures for `FieldMatrix::matvec`.
///
/// # Panics
///
/// Panics if any operand length disagrees with the declared shape.
#[inline(never)]
pub fn matvec_region(a: &[u8], x: &[u8], y: &mut [u8], m: usize, k: usize, table: &ProductTable) {
    assert_eq!(a.len(), m * k, "matvec_region: matrix shape");
    assert_eq!(x.len(), k, "matvec_region: input shape");
    assert_eq!(y.len(), m, "matvec_region: output shape");
    for (i, target) in y.iter_mut().enumerate() {
        let row = &a[i * k..(i + 1) * k];
        let mut acc = 0u8;
        for (left, right) in row.iter().zip(x.iter()) {
            acc ^= table.get(*left, *right);
        }
        *target = acc;
    }
}

/// Arbitrary pairwise product `z[i] = x[i] * y[i]` over byte regions, the
/// control's prototype route.
///
/// No coefficient repeats, so the byte-oriented form is one lookup in the
/// full 64 KiB table per element and no table can be prepared per
/// coefficient. The control exists to show which part of the vector and
/// matrix result belongs to coefficient reuse rather than to byte
/// arithmetic.
///
/// # Panics
///
/// Panics if the three regions have different lengths.
#[inline(never)]
pub fn pairwise_region(x: &[u8], y: &[u8], z: &mut [u8], table: &ProductTable) {
    assert_eq!(x.len(), y.len(), "pairwise_region: operand length mismatch");
    assert_eq!(x.len(), z.len(), "pairwise_region: output length mismatch");
    for (target, (left, right)) in z.iter_mut().zip(x.iter().zip(y.iter())) {
        *target = table.get(*left, *right);
    }
}

/// Converts a byte region into an existing row-major `FieldMatrix`.
///
/// [`byte_field_gf2_side::workload`] supplies the vector conversions; this
/// is its matrix counterpart in the other direction, which the prototype's
/// whole-consumer route needs.
///
/// # Panics
///
/// Panics if the region length disagrees with the matrix shape.
pub fn matrix_to_region<B: ByteField>(matrix: &FieldMatrix<B::Elem>, bytes: &mut [u8]) {
    let (rows, cols) = matrix.shape();
    assert_eq!(bytes.len(), rows * cols, "matrix_to_region: shape mismatch");
    for row in 0..rows {
        for (slot, element) in bytes[row * cols..(row + 1) * cols]
            .iter_mut()
            .zip(matrix.row(row))
        {
            *slot = B::byte(element);
        }
    }
}

/// GF(2^8) modulo `x^8 + x^4 + x^3 + x + 1` (0x11B) as a compile-time
/// configuration.
///
/// The byte-field comparison survey declares
/// [`Gf256x11d`](byte_field_gf2_side::workload::Gf256x11d) because 0x11D is
/// the field every compared library implements. 0x11B is the second
/// polynomial a byte-oriented GF(2^8) design has to serve, and gf2 ships no
/// GF(2^8) configuration for it either, so the validation declares it here:
/// the prototype's table is built from the field's own reduction polynomial
/// and must therefore be correct for both, and 0x11B additionally exercises
/// a gf2-core multiplication route that 0x11D does not, because 0x11B is
/// irreducible without being primitive and so builds no log/antilog tables.
pub struct Gf256x11b;

impl gf2_core::gf2m::Gf2mWideConfig<1> for Gf256x11b {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1B];
    const NAME: &'static str = "Gf256x11b";
}

/// The compile-time GF(2^8) element over 0x11B.
pub type WideElement11b = gf2_core::gf2m::Gf2mWide<1, Gf256x11b>;

/// [`ByteField`] over [`Gf256x11b`], so the validation drives the same
/// generic prototype route the 0x11D arms drive.
pub struct WideGf256x11b;

impl ByteField for WideGf256x11b {
    type Elem = WideElement11b;
    const NAME: &'static str = "Gf2mWide<1,Gf256x11b>";

    fn polynomial(&self) -> u32 {
        use gf2_core::gf2m::Gf2mWideConfig;
        (1 << Gf256x11b::M) | Gf256x11b::MODULUS[0] as u32
    }

    fn element(&self, byte: u8) -> Self::Elem {
        WideElement11b::from_u64(u64::from(byte))
    }

    fn byte(element: &Self::Elem) -> u8 {
        (element.words()[0] & 0xFF) as u8
    }

    fn gemm_route() -> &'static str {
        // The validation never times a product, so no route is observed
        // here; the arm reports the route of the field it measures.
        "validation-only"
    }
}

/// `Gf2mElement` over a runtime GF(2^8) field of any reduction polynomial,
/// so the validation covers 0x11B as well as
/// [`RuntimeGf256`](byte_field_gf2_side::workload::RuntimeGf256)'s 0x11D.
pub struct RuntimeGf256Any {
    /// The runtime field.
    pub field: gf2_core::gf2m::Gf2mField,
}

impl RuntimeGf256Any {
    /// Builds the degree-8 field of `polynomial`.
    pub fn new(polynomial: u64) -> Self {
        RuntimeGf256Any {
            field: gf2_core::gf2m::Gf2mField::new(8, polynomial),
        }
    }
}

impl ByteField for RuntimeGf256Any {
    type Elem = gf2_core::gf2m::Gf2mElement;
    const NAME: &'static str = "Gf2mElement";

    fn polynomial(&self) -> u32 {
        self.field.primitive_polynomial() as u32
    }

    fn element(&self, byte: u8) -> Self::Elem {
        self.field.element(u64::from(byte))
    }

    fn byte(element: &Self::Elem) -> u8 {
        (element.value() & 0xFF) as u8
    }

    fn gemm_route() -> &'static str {
        "validation-only"
    }
}
