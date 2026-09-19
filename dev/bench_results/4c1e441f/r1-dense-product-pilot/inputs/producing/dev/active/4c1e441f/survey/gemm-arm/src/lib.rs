//! Operands of the GF(2^8) dense product under measurement (jit:4c1e441f).
//!
//! Both arms of a pair are one executable. The lane contract — the switch that
//! holds every GF(2^8) call on one lane, the element representation the
//! consumer holds, and the shipped witness that names the lane the measured
//! calls took — is the one the vector confirmation established, so it is
//! re-exported here rather than restated.

pub use gf256_axpy_arm::{observed_lane, Lane, Representation};

use byte_field_arm_common::{OperandStream, SplitMix64};
use byte_field_gf2_side::workload::{self, ByteField};
use gf2_core::field::matrix::FieldMatrix;

/// One square product's operands, in bytes and in the consumer's own
/// representation.
///
/// A kernel-isolated cell multiplies the matrices; a whole-consumer cell starts
/// and ends at the byte region, so it packs both operands, multiplies and
/// unpacks the result inside the measured call. Both share these buffers, so
/// the two metric kinds differ in the traversal alone.
pub struct SquareProduct<B: ByteField> {
    /// Square dimension of both operands and of the product.
    pub n: usize,
    /// Left operand, `n * n` row-major bytes.
    pub a_bytes: Vec<u8>,
    /// Right operand, `n * n` row-major bytes.
    pub b_bytes: Vec<u8>,
    /// Left operand in the consumer's representation.
    pub a: FieldMatrix<B::Elem>,
    /// Right operand in the consumer's representation.
    pub b: FieldMatrix<B::Elem>,
    /// Byte region a whole-consumer call writes the product into.
    pub out_bytes: Vec<u8>,
}

impl<B: ByteField> SquareProduct<B> {
    /// Deterministic operands over the shared stream, packed once.
    pub fn new(field: &B, n: usize, seed: u64) -> Self {
        let mut rng = SplitMix64::new(seed);
        let mut a_bytes = vec![0u8; n * n];
        let mut b_bytes = vec![0u8; n * n];
        rng.fill(&mut a_bytes);
        rng.fill(&mut b_bytes);
        let a = workload::pack_matrix(field, &a_bytes, n, n);
        let b = workload::pack_matrix(field, &b_bytes, n, n);
        Self {
            n,
            a_bytes,
            b_bytes,
            a,
            b,
            out_bytes: vec![0u8; n * n],
        }
    }

    /// The product on operands the consumer already holds.
    pub fn kernel_isolated(&self) -> FieldMatrix<B::Elem> {
        workload::matmul(&self.a, &self.b)
    }

    /// The product from bytes to bytes: both operand conversions, the product
    /// and the output conversion, which is what a byte-region consumer pays.
    pub fn whole_consumer(&mut self, field: &B) {
        workload::pack_into_matrix(field, &self.a_bytes, &mut self.a);
        workload::pack_into_matrix(field, &self.b_bytes, &mut self.b);
        let product = workload::matmul(&self.a, &self.b);
        workload::unpack_matrix::<B>(&product, &mut self.out_bytes);
    }
}

/// The product bytes one lane computes for one shape.
///
/// `a` is `m * k` row-major bytes and `b` is `k * n`; the result is `m * n`. The
/// lane is applied before the operands are packed, so the packing itself is on
/// the selected lane too.
pub fn product_on_lane<B: ByteField>(
    field: &B,
    lane: Lane,
    shape: (usize, usize, usize),
    a: &[u8],
    b: &[u8],
) -> Vec<u8> {
    let (m, k, n) = shape;
    lane.apply();
    let left = workload::pack_matrix(field, a, m, k);
    let right = workload::pack_matrix(field, b, k, n);
    let product = workload::matmul(&left, &right);
    let mut bytes = vec![0u8; m * n];
    workload::unpack_matrix::<B>(&product, &mut bytes);
    bytes
}
