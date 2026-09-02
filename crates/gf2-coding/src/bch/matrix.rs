//! Generator and parity-check matrix materialization for BCH codes.
//!
//! Both matrices are written in the default user layout,
//! [`MessageParityAscending`](crate::bch::encode::SystematicLayout::MessageParityAscending):
//! user coordinate $u$ carries the coefficient of $x^{(u + n - k) \bmod n}$,
//! so the first $k$ coordinates carry the message in ascending degree and the
//! remaining $n - k$ carry the parity in ascending degree. Row $i$ of the
//! generator matrix is the codeword the systematic encoder writes for message
//! basis vector $i$, so the generator is $G = [\,I_k \mid P\,]$ and
//! [`is_systematic`](GeneratorMatrixAccess::is_systematic) holds. The
//! parity-check matrix is $H = [\,-P^{\mathsf T} \mid I_{n-k}\,]$: it has full
//! row rank $n - k$ and satisfies $G H^{\mathsf T} = 0$.
//!
//! That layout is the matrix contract rather than a per-call option.
//! [`MessageParityDescending`](crate::bch::encode::SystematicLayout::MessageParityDescending)
//! selects a transmission order for an encode call, and no matrix is
//! materialized under it; a consumer needing another coordinate order composes
//! these matrices with
//! [`SystematicPlan::to_coordinate_map`](crate::bch::encode::SystematicPlan::to_coordinate_map).
//!
//! Materialization holds no implicit cache. [`CachedMatrices`] is the explicit
//! opt-in cache for callers that reuse these matrices. It caches successful
//! generator and parity-check materializations independently, while its
//! allocating access still returns a fresh matrix on every call.
//!
//! # Algorithm
//!
//! Both matrices follow from one recurrence on the parity block. Row $i$ of
//! $P$ is $-(x^{\,r+i} \bmod g)$ in ascending degree for $r = n - k$, so row
//! $0$ is the generator's low $r$ coefficients and every later row is its
//! predecessor shifted one degree with one conditional subtraction of $g$:
//! $P_{i,j} = P_{i-1,\,j-1} - P_{i-1,\,r-1}\,g_j$. Column $i$ of $H$ is
//! $-P_i$, which is the same recurrence read on columns.
//!
//! Each writer carries the recurrence in its own output — the generator in
//! the parity block of the row before, the parity check in the column before
//! — so neither materializes the other matrix, neither holds a register
//! beside the caller's buffer, and the caller-buffer path reaches no
//! allocator. For the packed binary representation the generator step is one
//! word-level shift and one conditional exclusive-or of row zero.
//!
//! # Representation
//!
//! [`SymbolMatrix`] exposes single-coordinate accessors, which the packed
//! word-level step cannot be written through, so materialization dispatches
//! through a private trait that each canonical matrix representation
//! implements over its own storage. That keeps one canonical trait set: the
//! specialization is by matrix representation, as the packed binary storage
//! decision states, rather than a second public matrix hierarchy.
//!
//! # Complexity
//!
//! The generator costs $O(k \lceil n/64 \rceil)$ word operations packed, and
//! $O(kn)$ cell writes with $O(kr)$ base-field multiply-adds field-generic;
//! both are the size of the output. The parity check transposes the same
//! recurrence one coordinate at a time, so it costs $O(kr)$ coordinate writes
//! over its $O(rn)$ output.
//!
//! # Examples
//!
//! ```
//! use gf2_coding::bch::matrix::CachedMatrices;
//! use gf2_coding::bch::spec::{BchSpec, BinaryBchCode, DesignedDistance};
//! use gf2_coding::traits::block::{BlockCode, GeneratorMatrixAccess};
//! use gf2_core::field::extension::BinaryPrimeExt;
//! use gf2_core::gf2m::Gf2mField;
//!
//! let extension = BinaryPrimeExt::new(Gf2mField::new(4, 0b10011)).unwrap();
//! let code = BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
//!     extension,
//!     designed_distance: DesignedDistance::try_from(5).unwrap(),
//! })
//! .unwrap();
//! let cached = CachedMatrices::new(code);
//! let generator = cached.generator_matrix().unwrap();
//!
//! assert_eq!(generator.rows(), cached.k());
//! assert_eq!(generator.cols(), cached.n());
//! assert!(cached.is_systematic().unwrap());
//! // The first k columns are the identity, so row i carries message bit i.
//! for row in 0..cached.k() {
//!     for col in 0..cached.k() {
//!         assert_eq!(generator.get(row, col), row == col);
//!     }
//! }
//! ```

use std::any::Any;
use std::sync::Mutex;

use gf2_core::field::extension::{FieldExtension, FieldIdentity};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::{FieldPoly, FiniteField};
use gf2_core::gfp::Fp;
use gf2_core::BitMatrix;

#[cfg(any(test, feature = "test-support"))]
use crate::bch::encode::SystematicKernel;
use crate::bch::spec::BchCode;
use crate::error::CodeError;
#[cfg(any(test, feature = "test-support"))]
use crate::traits::block::BlockEncoder;
use crate::traits::block::{
    BlockCode, GeneratorMatrixAccess, ParityCheckMatrixAccess, SymbolMatrix, SymbolSequence,
};

/// Ensures that a caller-provided matrix has the shape required by a code.
fn check_shape<F, M>(out: &M, expected_rows: usize, expected_cols: usize) -> Result<(), CodeError>
where
    F: FieldIdentity,
    M: SymbolMatrix<F>,
{
    let actual_rows = out.rows();
    let actual_cols = out.cols();
    if actual_rows != expected_rows || actual_cols != expected_cols {
        return Err(CodeError::ShapeMismatch {
            expected_rows,
            expected_cols,
            actual_rows,
            actual_cols,
        });
    }
    Ok(())
}

/// Copies a matrix cell-by-cell without allocating a replacement output.
fn copy_matrix<F, M>(source: &M, out: &mut M) -> Result<(), CodeError>
where
    F: FieldIdentity,
    M: SymbolMatrix<F>,
{
    for row in 0..source.rows() {
        for col in 0..source.cols() {
            let value = source.get(row, col).ok_or(CodeError::IndexOutOfBounds {
                index: row,
                length: source.rows(),
            })?;
            out.set(row, col, value)?;
        }
    }
    Ok(())
}

/// Materialization of the canonical matrices over one matrix representation.
///
/// A caller checks the output shape before dispatching here, so an
/// implementation writes only in-range coordinates, overwrites every
/// coordinate of its output, and cannot fail. `dimension` is the code's $k$;
/// the redundancy and the length follow from the output's own shape.
trait MatrixFill<F>: SymbolMatrix<F>
where
    F: FieldIdentity,
{
    /// Writes $G = [\,I_k \mid P\,]$.
    fn fill_generator(&mut self, generator: &FieldPoly<F>, dimension: usize, zero: &F);

    /// Writes $H = [\,-P^{\mathsf T} \mid I_{n-k}\,]$.
    fn fill_parity_check(&mut self, generator: &FieldPoly<F>, dimension: usize, zero: &F);
}

impl MatrixFill<Fp<2>> for BitMatrix {
    fn fill_generator(&mut self, generator: &FieldPoly<Fp<2>>, dimension: usize, _zero: &Fp<2>) {
        let length = BitMatrix::cols(self);
        let redundancy = length - dimension;
        for row in 0..BitMatrix::rows(self) {
            self.row_words_mut(row).fill(0);
        }
        if dimension == 0 {
            return;
        }
        if redundancy == 0 {
            for row in 0..dimension {
                self.set(row, row, true);
            }
            return;
        }

        // Row zero carries $P_0 = g$. The identity column enters after the
        // recurrence, so every row the recurrence reads holds its parity
        // block alone and the step is a whole-row shift.
        for degree in 0..redundancy {
            if generator.coeff(degree).is_one() {
                self.set(0, dimension + degree, true);
            }
        }

        // The shift and the reduction touch only the words the parity block
        // reaches, and row zero holds $g$ at the same bit offset, so the
        // reduction is an exclusive-or of that row over the same words.
        let stride = self.stride_words();
        let first = dimension >> 6;
        let tail = length & 63;
        for row in 1..dimension {
            let reduce = self.get(row - 1, length - 1);
            let mut upper = self.row_words(row - 1)[stride - 1];
            for word in (first..stride).rev() {
                let lower = if word > first {
                    self.row_words(row - 1)[word - 1]
                } else {
                    0
                };
                self.row_words_mut(row)[word] = (upper << 1) | (lower >> 63);
                upper = lower;
            }
            if tail != 0 {
                self.row_words_mut(row)[stride - 1] &= (1u64 << tail) - 1;
            }
            if reduce {
                for word in first..stride {
                    let coefficients = self.row_words(0)[word];
                    self.row_words_mut(row)[word] ^= coefficients;
                }
            }
        }

        for row in 0..dimension {
            self.set(row, row, true);
        }
    }

    fn fill_parity_check(&mut self, generator: &FieldPoly<Fp<2>>, dimension: usize, _zero: &Fp<2>) {
        let redundancy = BitMatrix::rows(self);
        for row in 0..redundancy {
            self.row_words_mut(row).fill(0);
            self.set(row, dimension + row, true);
        }
        if dimension == 0 || redundancy == 0 {
            return;
        }

        // Column $i$ holds $-P_i$, so the recurrence advances one column at a
        // time and the output is the only state it needs.
        for degree in 0..redundancy {
            if generator.coeff(degree).is_one() {
                self.set(degree, 0, true);
            }
        }
        for column in 1..dimension {
            let reduce = self.get(redundancy - 1, column - 1);
            for degree in (1..redundancy).rev() {
                let value =
                    self.get(degree - 1, column - 1) ^ (reduce && generator.coeff(degree).is_one());
                self.set(degree, column, value);
            }
            self.set(0, column, reduce && generator.coeff(0).is_one());
        }
    }
}

impl<F> MatrixFill<F> for FieldMatrix<F>
where
    F: FieldIdentity + 'static,
{
    fn fill_generator(&mut self, generator: &FieldPoly<F>, dimension: usize, zero: &F) {
        let length = FieldMatrix::cols(self);
        let redundancy = length - dimension;
        for row in 0..FieldMatrix::rows(self) {
            self.row_mut(row).fill(zero.clone());
        }
        if dimension == 0 {
            return;
        }
        let one = zero.one_like();
        if redundancy == 0 {
            for row in 0..dimension {
                self.set(row, row, one.clone());
            }
            return;
        }

        for degree in 0..redundancy {
            let coefficient = generator.coeff(degree);
            self.set(0, dimension + degree, coefficient);
        }
        for row in 1..dimension {
            let reduce = self.get(row - 1, length - 1);
            for degree in (1..redundancy).rev() {
                let value = self.get(row - 1, dimension + degree - 1)
                    - reduce.clone() * generator.coeff(degree);
                self.set(row, dimension + degree, value);
            }
            let value = -(reduce * generator.coeff(0));
            self.set(row, dimension, value);
        }

        for row in 0..dimension {
            self.set(row, row, one.clone());
        }
    }

    fn fill_parity_check(&mut self, generator: &FieldPoly<F>, dimension: usize, zero: &F) {
        let redundancy = FieldMatrix::rows(self);
        let one = zero.one_like();
        for row in 0..redundancy {
            self.row_mut(row).fill(zero.clone());
            self.set(row, dimension + row, one.clone());
        }
        if dimension == 0 || redundancy == 0 {
            return;
        }

        // Column $i$ holds $-P_i$, so the recurrence advances one column at a
        // time and the output is the only state it needs.
        for degree in 0..redundancy {
            let value = -generator.coeff(degree);
            self.set(degree, 0, value);
        }
        for column in 1..dimension {
            let reduce = self.get(redundancy - 1, column - 1);
            for degree in (1..redundancy).rev() {
                let value =
                    self.get(degree - 1, column - 1) - reduce.clone() * generator.coeff(degree);
                self.set(degree, column, value);
            }
            let value = -(reduce * generator.coeff(0));
            self.set(0, column, value);
        }
    }
}

/// Writes the systematic generator matrix of `code` into `out`.
fn write_generator<X, S, M>(code: &BchCode<X, S, M>, out: &mut M) -> Result<(), CodeError>
where
    X: FieldExtension,
    S: SymbolSequence<X::Base>,
    M: MatrixFill<X::Base>,
{
    check_shape(out, code.k(), code.n())?;
    out.fill_generator(code.generator(), code.k(), &code.symbol_zero());
    Ok(())
}

/// Writes the full-row-rank parity-check matrix of `code` into `out`.
fn write_parity_check<X, S, M>(code: &BchCode<X, S, M>, out: &mut M) -> Result<(), CodeError>
where
    X: FieldExtension,
    S: SymbolSequence<X::Base>,
    M: MatrixFill<X::Base>,
{
    check_shape(out, code.redundancy(), code.n())?;
    out.fill_parity_check(code.generator(), code.k(), &code.symbol_zero());
    Ok(())
}

// ---------------------------------------------------------------------------
// The straightforward materialization, kept as an oracle
// ---------------------------------------------------------------------------

/// Encodes each message basis vector in turn and hands its row index and
/// codeword to `write_row`.
///
/// The message and codeword buffers are allocated once and reused for every
/// row.
#[cfg(any(test, feature = "test-support"))]
fn for_each_generator_row<X, S, M>(
    code: &BchCode<X, S, M>,
    mut write_row: impl FnMut(usize, &S) -> Result<(), CodeError>,
) -> Result<(), CodeError>
where
    X: FieldExtension,
    S: SystematicKernel<X::Base>,
    M: SymbolMatrix<X::Base>,
{
    let zero = code.symbol_zero();
    let one = zero.one_like();
    let mut message = S::zeroed(code.k(), &zero);
    let mut codeword = S::zeroed(code.n(), &zero);
    for row in 0..code.k() {
        message.set(row, one.clone())?;
        code.encode_into(&message, &mut codeword)?;
        message.set(row, zero.clone())?;
        write_row(row, &codeword)?;
    }
    Ok(())
}

/// Writes the generator matrix by encoding the $k$ message basis vectors.
///
/// This reads the contract literally rather than deriving it: it spends
/// $O(k^2 r)$ base-field operations where the materialization walks the
/// output once, and it consults the encoder where the materialization
/// consults only the generator polynomial. It is the oracle the module's
/// equality tests and the `bch_genmatrix` bench compare the materialization
/// against, reachable through [`crate::test_support`].
///
/// # Errors
///
/// Returns [`CodeError::ShapeMismatch`] when `out` is not $k \times n$, and
/// propagates the encoder's own errors.
#[cfg(any(test, feature = "test-support"))]
pub(crate) fn write_generator_by_encoding<X, S, M>(
    code: &BchCode<X, S, M>,
    out: &mut M,
) -> Result<(), CodeError>
where
    X: FieldExtension,
    S: SystematicKernel<X::Base>,
    M: SymbolMatrix<X::Base>,
{
    let length = code.n();
    check_shape(out, code.k(), length)?;
    for_each_generator_row(code, |row, codeword| {
        for col in 0..length {
            let value = codeword
                .get(col)
                .expect("an encoded codeword has the code's length");
            out.set(row, col, value)?;
        }
        Ok(())
    })
}

/// Writes the parity-check matrix from the codewords of the message basis
/// vectors, the transpose of [`write_generator_by_encoding`]'s parity block.
///
/// # Errors
///
/// Returns [`CodeError::ShapeMismatch`] when `out` is not $(n-k) \times n$,
/// and propagates the encoder's own errors.
#[cfg(any(test, feature = "test-support"))]
pub(crate) fn write_parity_check_by_encoding<X, S, M>(
    code: &BchCode<X, S, M>,
    out: &mut M,
) -> Result<(), CodeError>
where
    X: FieldExtension,
    S: SystematicKernel<X::Base>,
    M: SymbolMatrix<X::Base>,
{
    let dimension = code.k();
    let length = code.n();
    let redundancy = code.redundancy();
    check_shape(out, redundancy, length)?;

    let zero = code.symbol_zero();
    let one = zero.one_like();
    for row in 0..redundancy {
        for col in dimension..length {
            let value = if col - dimension == row {
                one.clone()
            } else {
                zero.clone()
            };
            out.set(row, col, value)?;
        }
    }

    for_each_generator_row(code, |col, codeword| {
        for row in 0..redundancy {
            let parity = codeword
                .get(dimension + row)
                .expect("a codeword carries the parity above its message coordinates");
            out.set(row, col, -parity)?;
        }
        Ok(())
    })
}

impl<X, S, M> GeneratorMatrixAccess for BchCode<X, S, M>
where
    X: FieldExtension,
    S: SymbolSequence<X::Base>,
    M: MatrixFill<X::Base>,
{
    type GeneratorMatrix = M;

    fn generator_matrix_into(&self, out: &mut Self::GeneratorMatrix) -> Result<(), CodeError> {
        write_generator(self, out)
    }

    /// Reports `true` without materializing anything.
    ///
    /// The materialization writes $G = [\,I_k \mid P\,]$ in the default user
    /// layout, so the message coordinates are the columns $0$ to $k - 1$ by
    /// construction. This is the equivalent cheap fact the trait admits in
    /// place of a materialize-and-inspect answer.
    fn is_systematic(&self) -> Result<bool, CodeError> {
        Ok(true)
    }
}

impl<X, S, M> ParityCheckMatrixAccess for BchCode<X, S, M>
where
    X: FieldExtension,
    S: SymbolSequence<X::Base>,
    M: MatrixFill<X::Base>,
{
    type ParityCheckMatrix = M;

    fn parity_check_matrix_into(&self, out: &mut Self::ParityCheckMatrix) -> Result<(), CodeError> {
        write_parity_check(self, out)
    }
}

/// Explicit opt-in caching for a code's generator and parity-check matrices.
///
/// The wrapped code is never modified and does not gain a cache. Each
/// successful matrix materialization is retained independently, and all
/// allocating accessors return a fresh clone. `clear` drops both retained
/// values so that the next access rebuilds them.
///
/// The cache uses synchronization because the wrapper is intended to remain a
/// usable static code value when it is shared across threads. Synchronization
/// exists only after a caller explicitly chooses this wrapper.
pub struct CachedMatrices<C> {
    code: C,
    generator: Mutex<Option<Box<dyn Any + Send + Sync>>>,
    parity_check: Mutex<Option<Box<dyn Any + Send + Sync>>>,
}

impl<C> CachedMatrices<C> {
    /// Wraps `code` with initially empty generator and parity-check caches.
    pub fn new(code: C) -> Self {
        Self {
            code,
            generator: Mutex::new(None),
            parity_check: Mutex::new(None),
        }
    }

    /// Returns the wrapped code.
    pub fn code(&self) -> &C {
        &self.code
    }

    /// Drops both cached matrices.
    pub fn clear(&self) {
        *self
            .generator
            .lock()
            .expect("generator cache lock poisoned") = None;
        *self
            .parity_check
            .lock()
            .expect("parity-check cache lock poisoned") = None;
    }
}

impl<C> BlockCode for CachedMatrices<C>
where
    C: BlockCode,
{
    type Symbol = C::Symbol;
    type Symbols = C::Symbols;

    fn symbol_zero(&self) -> Self::Symbol {
        self.code.symbol_zero()
    }

    fn k(&self) -> usize {
        self.code.k()
    }

    fn n(&self) -> usize {
        self.code.n()
    }
}

impl<C> GeneratorMatrixAccess for CachedMatrices<C>
where
    C: GeneratorMatrixAccess,
    C::GeneratorMatrix: Send + Sync + 'static,
{
    type GeneratorMatrix = C::GeneratorMatrix;

    fn generator_matrix_into(&self, out: &mut Self::GeneratorMatrix) -> Result<(), CodeError> {
        check_shape(out, self.k(), self.n())?;
        let mut cache = self
            .generator
            .lock()
            .expect("generator cache lock poisoned");
        if cache.is_none() {
            let matrix = self.code.generator_matrix()?;
            *cache = Some(Box::new(matrix));
        }
        let matrix = cache
            .as_ref()
            .and_then(|value| value.downcast_ref::<C::GeneratorMatrix>())
            .expect("generator cache contains the wrapped matrix representation");
        copy_matrix(matrix, out)
    }

    fn is_systematic(&self) -> Result<bool, CodeError> {
        self.code.is_systematic()
    }
}

impl<C> ParityCheckMatrixAccess for CachedMatrices<C>
where
    C: ParityCheckMatrixAccess,
    C::ParityCheckMatrix: Send + Sync + 'static,
{
    type ParityCheckMatrix = C::ParityCheckMatrix;

    fn parity_check_rows(&self) -> usize {
        self.code.parity_check_rows()
    }

    fn parity_check_matrix_into(&self, out: &mut Self::ParityCheckMatrix) -> Result<(), CodeError> {
        check_shape(out, self.parity_check_rows(), self.n())?;
        let mut cache = self
            .parity_check
            .lock()
            .expect("parity-check cache lock poisoned");
        if cache.is_none() {
            let matrix = self.code.parity_check_matrix()?;
            *cache = Some(Box::new(matrix));
        }
        let matrix = cache
            .as_ref()
            .and_then(|value| value.downcast_ref::<C::ParityCheckMatrix>())
            .expect("parity-check cache contains the wrapped matrix representation");
        copy_matrix(matrix, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use crate::bch::encode::SystematicLayout;
    use crate::bch::spec::{
        BchLength, BchSpec, BinaryBchCode, DenseBchCode, DesignedDistance, RootExponent,
        RootSelection,
    };
    use gf2_core::field::extension::BinaryPrimeExt;
    use gf2_core::field::matrix::FieldMatrix;
    use gf2_core::field::modulus_select::select_modulus;
    use gf2_core::field::ConstField;
    use gf2_core::field::FieldPoly;
    use gf2_core::gf2m::Gf2mField;
    use gf2_core::gfp::Fp;
    use gf2_core::gfpn::{QuotientElement, QuotientField};
    use gf2_core::{BitMatrix, BitVec};

    fn binary_extension() -> BinaryPrimeExt {
        BinaryPrimeExt::new(Gf2mField::new(4, 0b10011)).expect("a valid binary extension")
    }

    fn binary_code() -> BinaryBchCode {
        BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
            extension: binary_extension(),
            designed_distance: DesignedDistance::try_from(5).expect("positive distance"),
        })
        .expect("a valid binary BCH code")
    }

    /// The $\delta = 1$ boundary: no roots, so $g = 1$, $k = n$ and the
    /// parity check is empty.
    fn binary_full_space() -> BinaryBchCode {
        BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
            extension: binary_extension(),
            designed_distance: DesignedDistance::try_from(1).expect("positive distance"),
        })
        .expect("the full-space boundary code")
    }

    /// The $\delta = n + 1$ boundary: the defining set closes over every
    /// exponent, so $g = x^n - 1$, $k = 0$ and the generator is empty.
    fn binary_zero_dimensional() -> BinaryBchCode {
        BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
            extension: binary_extension(),
            designed_distance: DesignedDistance::try_from(16).expect("positive distance"),
        })
        .expect("the zero-dimensional boundary code")
    }

    /// A primitive narrow-sense binary code over `GF(2^degree)`.
    fn primitive_binary(degree: usize, modulus: u64, designed_distance: u64) -> BinaryBchCode {
        let extension = BinaryPrimeExt::new(Gf2mField::new(degree, modulus))
            .expect("a primitive polynomial of the requested degree");
        BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
            extension,
            designed_distance: DesignedDistance::try_from(designed_distance)
                .expect("positive distance"),
        })
        .expect("a valid primitive narrow-sense code")
    }

    fn gf25() -> QuotientField<Fp<5>> {
        let modulus = FieldPoly::new(vec![Fp::<5>::new(1), Fp::new(1), Fp::new(1)]);
        QuotientField::new(Fp::<5>::zero(), modulus).expect("a valid GF(25)")
    }

    fn gf81_over_gf9() -> QuotientField<QuotientElement<Fp<3>>> {
        let gf9_modulus = select_modulus(&Fp::<3>::zero(), 2).expect("a GF(9) modulus");
        let gf9 = QuotientField::new(Fp::<3>::zero(), gf9_modulus).expect("GF(9)");
        let gf81_modulus = select_modulus(&gf9.ext_zero(), 2).expect("a GF(81) modulus");
        QuotientField::new(gf9.ext_zero(), gf81_modulus).expect("GF(81) over GF(9)")
    }

    fn gf5_code(designed_distance: u64) -> DenseBchCode<QuotientField<Fp<5>>> {
        DenseBchCode::construct(BchSpec::PrimitiveNarrowSense {
            extension: gf25(),
            designed_distance: DesignedDistance::try_from(designed_distance)
                .expect("positive distance"),
        })
        .expect("a valid GF(5) BCH code")
    }

    fn gf81_code(designed_distance: u64) -> DenseBchCode<QuotientField<QuotientElement<Fp<3>>>> {
        DenseBchCode::construct(BchSpec::PrimitiveNarrowSense {
            extension: gf81_over_gf9(),
            designed_distance: DesignedDistance::try_from(designed_distance)
                .expect("positive distance"),
        })
        .expect("a valid GF(9) BCH code")
    }

    fn binary_code_as_dense() -> DenseBchCode<BinaryPrimeExt> {
        DenseBchCode::construct(BchSpec::PrimitiveNarrowSense {
            extension: binary_extension(),
            designed_distance: DesignedDistance::try_from(5).expect("positive distance"),
        })
        .expect("a valid dense binary BCH code")
    }

    /// Asserts that row `i` of `generator` is the codeword the default-layout
    /// systematic encoder writes for message basis vector `i`.
    fn assert_rows_encode_basis_vectors<X, S, M>(code: &BchCode<X, S, M>, generator: &M)
    where
        X: FieldExtension,
        S: SystematicKernel<X::Base>,
        M: SymbolMatrix<X::Base>,
    {
        let zero = code.symbol_zero();
        let one = zero.one_like();
        for row in 0..code.k() {
            let mut message = S::zeroed(code.k(), &zero);
            message.set(row, one.clone()).expect("a row below k");
            let codeword = code.encode(&message).expect("a basis vector encodes");
            for col in 0..code.n() {
                assert_eq!(
                    generator.get(row, col),
                    codeword.get(col),
                    "generator row {row}, column {col}"
                );
            }
        }
    }

    /// Asserts that the first `k` columns of `generator` are the identity.
    fn assert_identity_prefix<X, S, M>(code: &BchCode<X, S, M>, generator: &M)
    where
        X: FieldExtension,
        S: SystematicKernel<X::Base>,
        M: SymbolMatrix<X::Base>,
    {
        for row in 0..code.k() {
            for col in 0..code.k() {
                let value = generator
                    .get(row, col)
                    .expect("the materialized generator has its declared shape");
                if row == col {
                    assert!(value.is_one(), "generator diagonal at {row}");
                } else {
                    assert!(value.is_zero(), "generator off-diagonal at ({row}, {col})");
                }
            }
        }
    }

    /// Asserts that every row of `matrix`, read back through the layout into
    /// internal coordinates, is a multiple of the generator polynomial.
    ///
    /// This is the polynomial-form membership oracle: it decides code
    /// membership without consulting the encoder the matrix is defined by.
    fn assert_rows_are_codewords<X, S, M>(code: &BchCode<X, S, M>, matrix: &M)
    where
        X: FieldExtension,
        S: SystematicKernel<X::Base>,
        M: SymbolMatrix<X::Base>,
    {
        let plan = code.systematic_plan(SystematicLayout::default());
        let zero = code.symbol_zero();
        for row in 0..matrix.rows() {
            let mut coefficients = vec![zero.clone(); code.n()];
            for user in 0..code.n() {
                let internal = plan
                    .internal_coordinate(user)
                    .expect("a user coordinate below n");
                coefficients[internal] = matrix.get(row, user).expect("valid matrix coordinate");
            }
            let row_polynomial = FieldPoly::new(coefficients);
            assert!(
                row_polynomial.div_rem(code.generator()).1.is_zero(),
                "row {row} is not a multiple of the generator"
            );
        }
    }

    fn assert_full_row_rank<F, M>(matrix: &M)
    where
        F: FieldIdentity,
        M: SymbolMatrix<F>,
    {
        let mut values: Vec<Vec<F>> = (0..matrix.rows())
            .map(|row| {
                (0..matrix.cols())
                    .map(|col| matrix.get(row, col).expect("valid matrix coordinate"))
                    .collect()
            })
            .collect();
        let mut rank = 0;
        for col in 0..matrix.cols() {
            let pivot = (rank..matrix.rows()).find(|&row| !values[row][col].is_zero());
            let Some(pivot) = pivot else { continue };
            values.swap(rank, pivot);
            let inverse = values[rank][col].inv().expect("nonzero pivot");
            for entry in &mut values[rank][col..] {
                *entry = entry.clone() * inverse.clone();
            }
            for row in 0..matrix.rows() {
                if row == rank || values[row][col].is_zero() {
                    continue;
                }
                let factor = values[row][col].clone();
                let pivot_row = values[rank][col..].to_vec();
                for (entry, pivot_entry) in values[row][col..].iter_mut().zip(pivot_row) {
                    *entry = entry.clone() - factor.clone() * pivot_entry;
                }
            }
            rank += 1;
            if rank == matrix.rows() {
                break;
            }
        }
        assert_eq!(rank, matrix.rows());
    }

    fn assert_orthogonal<F, G, H>(generator: &G, parity: &H, zero: &F)
    where
        F: FieldIdentity,
        G: SymbolMatrix<F>,
        H: SymbolMatrix<F>,
    {
        assert_eq!(generator.cols(), parity.cols());
        for generator_row in 0..generator.rows() {
            for parity_row in 0..parity.rows() {
                let mut dot = zero.zero_like();
                for col in 0..generator.cols() {
                    dot += generator
                        .get(generator_row, col)
                        .expect("valid generator coordinate")
                        * parity
                            .get(parity_row, col)
                            .expect("valid parity coordinate");
                }
                assert!(dot.is_zero(), "G row {generator_row} · H row {parity_row}");
            }
        }
    }

    /// Asserts that the materialized matrices equal the ones the
    /// basis-vector oracle writes.
    fn assert_matches_oracle<X, S, M>(code: &BchCode<X, S, M>, generator: &M, parity: &M)
    where
        X: FieldExtension,
        S: SystematicKernel<X::Base>,
        M: SymbolMatrix<X::Base>,
    {
        let zero = code.symbol_zero();
        let mut oracle_generator = M::zeroed(code.k(), code.n(), &zero);
        write_generator_by_encoding(code, &mut oracle_generator).expect("oracle generator");
        assert_eq!(generator, &oracle_generator, "generator against the oracle");

        let mut oracle_parity = M::zeroed(code.redundancy(), code.n(), &zero);
        write_parity_check_by_encoding(code, &mut oracle_parity).expect("oracle parity check");
        assert_eq!(parity, &oracle_parity, "parity check against the oracle");
    }

    /// Checks the whole matrix contract of one code over the caller-buffer,
    /// allocating, and explicit-cache access paths.
    fn assert_matrix_contract<X, S, M>(code: &BchCode<X, S, M>)
    where
        X: FieldExtension,
        S: SystematicKernel<X::Base>,
        M: MatrixFill<X::Base> + Send + Sync,
        BchCode<X, S, M>: Clone,
    {
        let zero = code.symbol_zero();
        let generator = code.generator_matrix().expect("generator materialization");
        let parity = code.parity_check_matrix().expect("parity materialization");

        assert_eq!((generator.rows(), generator.cols()), (code.k(), code.n()));
        assert_eq!(
            (parity.rows(), parity.cols()),
            (code.redundancy(), code.n())
        );
        assert_eq!(code.parity_check_rows(), code.n() - code.k());

        assert_rows_encode_basis_vectors(code, &generator);
        assert_identity_prefix(code, &generator);
        assert!(code.is_systematic().expect("a systematic report"));
        assert_rows_are_codewords(code, &generator);
        assert_full_row_rank(&parity);
        assert_orthogonal(&generator, &parity, &zero);
        assert_matches_oracle(code, &generator, &parity);

        let mut generator_buffer = M::zeroed(code.k(), code.n(), &zero);
        code.generator_matrix_into(&mut generator_buffer)
            .expect("caller generator buffer");
        assert_eq!(generator_buffer, generator);

        let mut parity_buffer = M::zeroed(code.redundancy(), code.n(), &zero);
        code.parity_check_matrix_into(&mut parity_buffer)
            .expect("caller parity buffer");
        assert_eq!(parity_buffer, parity);

        let cached = CachedMatrices::new(code.clone());
        assert_eq!(
            cached.generator_matrix().expect("cached generator"),
            generator
        );
        assert_eq!(cached.parity_check_matrix().expect("cached parity"), parity);
        let mut cached_buffer = M::zeroed(code.k(), code.n(), &zero);
        cached
            .generator_matrix_into(&mut cached_buffer)
            .expect("cached generator caller buffer");
        assert_eq!(cached_buffer, generator);
        let mut cached_parity_buffer = M::zeroed(code.redundancy(), code.n(), &zero);
        cached
            .parity_check_matrix_into(&mut cached_parity_buffer)
            .expect("cached parity caller buffer");
        assert_eq!(cached_parity_buffer, parity);
    }

    #[test]
    fn packed_binary_matrices_follow_the_user_layout_contract() {
        assert_matrix_contract(&binary_code());
    }

    #[test]
    fn dense_binary_matrices_follow_the_user_layout_contract() {
        assert_matrix_contract(&binary_code_as_dense());
    }

    #[test]
    fn gf5_matrices_follow_the_user_layout_contract() {
        assert_matrix_contract(&gf5_code(5));
    }

    #[test]
    fn quotient_base_matrices_follow_the_user_layout_contract() {
        assert_matrix_contract(&gf81_code(4));
    }

    #[test]
    fn full_space_boundary_codes_materialize_an_identity_generator() {
        let packed = binary_full_space();
        assert_eq!((packed.k(), packed.n()), (15, 15));
        assert_eq!(packed.redundancy(), 0);
        assert_matrix_contract(&packed);

        let generator = packed.generator_matrix().expect("full-space generator");
        assert_eq!(generator, BitMatrix::identity(15));
        let parity = packed.parity_check_matrix().expect("full-space parity");
        assert_eq!((parity.rows(), parity.cols()), (0, 15));

        assert_matrix_contract(&gf5_code(1));
        assert_matrix_contract(&gf81_code(1));
    }

    #[test]
    fn zero_dimensional_boundary_codes_materialize_an_identity_parity_check() {
        let packed = binary_zero_dimensional();
        assert_eq!((packed.k(), packed.n()), (0, 15));
        assert_matrix_contract(&packed);

        let generator = packed
            .generator_matrix()
            .expect("zero-dimensional generator");
        assert_eq!((generator.rows(), generator.cols()), (0, 15));
        let parity = packed
            .parity_check_matrix()
            .expect("zero-dimensional parity");
        assert_eq!(parity, BitMatrix::identity(15));

        let dense = DenseBchCode::construct(BchSpec::PrimitiveNarrowSense {
            extension: binary_extension(),
            designed_distance: DesignedDistance::try_from(16).expect("positive distance"),
        })
        .expect("the dense zero-dimensional boundary code");
        assert_matrix_contract(&dense);
    }

    /// The packed word boundaries the codeword length reaches.
    ///
    /// A binary cyclic length is odd, so $n = 64$ is unreachable and the
    /// column boundaries a packed row meets are $63$ and $65$.
    #[test]
    fn packed_word_boundary_lengths_follow_the_contract() {
        let length_63 = primitive_binary(6, 0b100_0011, 17);
        assert_eq!((length_63.k(), length_63.n()), (18, 63));
        assert_matrix_contract(&length_63);

        // Length 65 is a proper divisor of $|GF(2^{12})^{*}| = 4095$.
        let extension = BinaryPrimeExt::new(Gf2mField::new(12, 0b1_0000_0101_0011))
            .expect("a primitive polynomial of degree twelve");
        let length_65 = BinaryBchCode::construct(BchSpec::NonPrimitiveConsecutive {
            extension,
            length: BchLength::try_from(65).expect("a positive length"),
            root: RootSelection::Canonical,
            first_root: RootExponent::from(1),
            designed_distance: DesignedDistance::try_from(5).expect("positive distance"),
        })
        .expect("a valid non-primitive code of length 65");
        assert_eq!((length_65.k(), length_65.n()), (41, 65));
        assert_matrix_contract(&length_65);
    }

    /// The packed word boundaries the redundancy reaches: the parity block of
    /// a generator row, and the row count of the parity check, at $63$, $64$
    /// and $65$.
    #[test]
    fn packed_word_boundary_redundancies_follow_the_contract() {
        let redundancy_63 = primitive_binary(7, 0b1000_0011, 21);
        assert_eq!((redundancy_63.k(), redundancy_63.n()), (64, 127));
        assert_matrix_contract(&redundancy_63);

        let redundancy_64 = primitive_binary(8, 0b1_0001_1101, 17);
        assert_eq!((redundancy_64.k(), redundancy_64.n()), (191, 255));
        assert_matrix_contract(&redundancy_64);

        // Adjoining the zero exponent to eight full cyclotomic cosets of
        // GF(2^8) closes a defining set of 65 exponents.
        let extension = BinaryPrimeExt::new(Gf2mField::new(8, 0b1_0001_1101))
            .expect("a primitive polynomial of degree eight");
        let redundancy_65 = BinaryBchCode::construct(BchSpec::PrimitiveFirstRoot {
            extension,
            first_root: RootExponent::from(0),
            designed_distance: DesignedDistance::try_from(18).expect("positive distance"),
        })
        .expect("a valid primitive code with the zero exponent as first root");
        assert_eq!((redundancy_65.k(), redundancy_65.n()), (190, 255));
        assert_matrix_contract(&redundancy_65);
    }

    /// The workload contract's binary rows B1, B2 and B3 at the exact lengths
    /// it fixes (`dev/active/4e732b56/workload-selection.md` § 2).
    #[test]
    fn workload_rows_follow_the_contract_and_match_the_oracle() {
        const ROWS: &[(usize, u64, u64, usize, usize)] = &[
            (4, 0b1_0011, 7, 5, 15),
            (7, 0b1000_0011, 21, 64, 127),
            (8, 0b1_0001_1101, 9, 223, 255),
        ];
        for &(degree, modulus, distance, dimension, length) in ROWS {
            let packed = primitive_binary(degree, modulus, distance);
            assert_eq!((packed.k(), packed.n()), (dimension, length));
            assert_matrix_contract(&packed);
        }

        // B1 field-generic, so the row is covered in both representations.
        let dense = DenseBchCode::construct(BchSpec::PrimitiveNarrowSense {
            extension: binary_extension(),
            designed_distance: DesignedDistance::try_from(7).expect("positive distance"),
        })
        .expect("the field-generic B1 row");
        assert_eq!((dense.k(), dense.n()), (5, 15));
        assert_matrix_contract(&dense);
    }

    /// A deterministic sample of generator rows for a code whose full row
    /// walk is too heavy for the fast tier.
    ///
    /// The first 32 rows carry the seed row and the first reductions. The
    /// last 32 carry the rows whose identity coordinate shares a packed word
    /// with the start of the parity block, which for the DVB-T2 mother codes
    /// is the final $k \bmod 64$ block of rows. A contiguous run of 32 in the
    /// middle straddles a word boundary of the row index. Together the sample
    /// meets every packed transition the row walk has.
    fn sampled_rows(dimension: usize) -> Vec<usize> {
        assert!(dimension > 128, "the sample assumes three disjoint blocks");
        let middle = ((dimension / 2) & !63) + 48;
        (0..32)
            .chain(middle..middle + 32)
            .chain(dimension - 32..dimension)
            .collect()
    }

    /// Asserts that the sampled generator rows are the systematic encodings
    /// of their basis vectors and that the parity check annihilates them.
    ///
    /// Comparing sampled rows against the encoder and against
    /// $G H^{\mathsf T} = 0$ decides the same contract the full comparison
    /// does on the rows it covers, at a cost the fast tier carries. The
    /// complete comparison of both DVB-T2 mother rows lives in the slow tier.
    fn assert_sampled_rows_are_systematic(
        code: &BinaryBchCode,
        generator: &BitMatrix,
        parity: &BitMatrix,
    ) {
        assert_eq!((generator.rows(), generator.cols()), (code.k(), code.n()));
        assert_eq!(
            (parity.rows(), parity.cols()),
            (code.redundancy(), code.n())
        );
        for row in sampled_rows(code.k()) {
            let mut message = BitVec::zeros(code.k());
            message.set(row, true);
            let codeword = code.encode(&message).expect("a basis vector encodes");
            let materialized = generator.row_as_bitvec(row);
            assert_eq!(materialized, codeword, "generator row {row}");
            let syndrome = parity.matvec(&materialized);
            assert_eq!(
                syndrome.count_ones(),
                0,
                "parity check does not annihilate generator row {row}"
            );
        }
    }

    /// The workload contract's T2S row at its mother length, the largest W2
    /// cell the fast tier reaches: a $16215 \times 16383$ generator and its
    /// $168 \times 16383$ parity check, witnessed on sampled rows.
    ///
    /// The canonical model reaches the DVB-T2 rows at mother length, which
    /// `dev/active/4e732b56/workload-selection.md` § 9 fixes as the length
    /// this consumer measures and compares them at.
    #[test]
    fn t2s_mother_sampled_rows_are_systematic() {
        let code = primitive_binary(14, 0b100_0000_0010_1011, 25);
        assert_eq!((code.k(), code.n()), (16215, 16383));
        let generator = code.generator_matrix().expect("generator materialization");
        let parity = code.parity_check_matrix().expect("parity materialization");
        assert_sampled_rows_are_systematic(&code, &generator, &parity);
    }

    /// Both DVB-T2 rows at their mother lengths, generator and parity check.
    ///
    /// The oracle costs $O(k^2 r)$, which puts the $65343 \times 65535$ row
    /// far outside the fast tier; the T2S generator alone stays in it above.
    #[test]
    #[ignore = "slow: the DVB-T2 mother rows materialize up to a 512 MiB generator"]
    fn dvb_t2_mother_rows_match_the_oracle() {
        const ROWS: &[(usize, u64, usize, usize)] = &[
            (14, 0b100_0000_0010_1011, 16215, 16383),
            (16, 0b1_0000_0000_0010_1101, 65343, 65535),
        ];
        for &(degree, modulus, dimension, length) in ROWS {
            let code = primitive_binary(degree, modulus, 25);
            assert_eq!((code.k(), code.n()), (dimension, length));
            assert_matches_oracle(
                &code,
                &code.generator_matrix().expect("generator materialization"),
                &code.parity_check_matrix().expect("parity materialization"),
            );
        }
    }

    #[test]
    fn packed_and_dense_binary_representations_agree() {
        let packed = binary_code();
        let dense = binary_code_as_dense();
        let packed_generator = packed.generator_matrix().expect("packed generator");
        let dense_generator = dense.generator_matrix().expect("dense generator");
        let packed_parity = packed.parity_check_matrix().expect("packed parity");
        let dense_parity = dense.parity_check_matrix().expect("dense parity");

        for row in 0..packed.n() {
            for col in 0..packed.n() {
                if row < packed_generator.rows() {
                    assert_eq!(
                        packed_generator.get(row, col),
                        dense_generator.get(row, col).is_one(),
                    );
                }
                if row < packed_parity.rows() {
                    assert_eq!(
                        packed_parity.get(row, col),
                        dense_parity.get(row, col).is_one(),
                    );
                }
            }
        }
    }

    #[test]
    fn caller_buffers_are_filled_and_shape_errors_are_typed() {
        let code = binary_code();
        let allocating_generator = code.generator_matrix().expect("allocating generator");
        let mut generator = BitMatrix::zeros(code.k(), code.n());
        code.generator_matrix_into(&mut generator)
            .expect("caller generator buffer");
        assert_eq!(generator, allocating_generator);

        let allocating_parity = code.parity_check_matrix().expect("allocating parity");
        let mut parity = BitMatrix::zeros(code.redundancy(), code.n());
        code.parity_check_matrix_into(&mut parity)
            .expect("caller parity buffer");
        assert_eq!(parity, allocating_parity);

        let mut wrong_generator = BitMatrix::zeros(code.k() + 1, code.n());
        assert_eq!(
            code.generator_matrix_into(&mut wrong_generator),
            Err(CodeError::ShapeMismatch {
                expected_rows: code.k(),
                expected_cols: code.n(),
                actual_rows: code.k() + 1,
                actual_cols: code.n(),
            })
        );
        let mut wrong_parity = BitMatrix::zeros(code.redundancy(), code.n() - 1);
        assert_eq!(
            code.parity_check_matrix_into(&mut wrong_parity),
            Err(CodeError::ShapeMismatch {
                expected_rows: code.redundancy(),
                expected_cols: code.n(),
                actual_rows: code.redundancy(),
                actual_cols: code.n() - 1,
            })
        );
    }

    /// A dirty caller buffer is overwritten rather than merged into.
    #[test]
    fn caller_buffers_are_overwritten_from_any_prior_contents() {
        let code = binary_code();
        let generator = code.generator_matrix().expect("allocating generator");
        let parity = code.parity_check_matrix().expect("allocating parity");

        let mut dirty_generator = BitMatrix::ones(code.k(), code.n());
        code.generator_matrix_into(&mut dirty_generator)
            .expect("caller generator buffer");
        assert_eq!(dirty_generator, generator);

        let mut dirty_parity = BitMatrix::ones(code.redundancy(), code.n());
        code.parity_check_matrix_into(&mut dirty_parity)
            .expect("caller parity buffer");
        assert_eq!(dirty_parity, parity);
    }

    #[derive(Clone)]
    struct CountingCode<C> {
        code: C,
        generator_calls: Arc<AtomicUsize>,
        parity_calls: Arc<AtomicUsize>,
    }

    impl<C> BlockCode for CountingCode<C>
    where
        C: BlockCode,
    {
        type Symbol = C::Symbol;
        type Symbols = C::Symbols;

        fn symbol_zero(&self) -> Self::Symbol {
            self.code.symbol_zero()
        }

        fn k(&self) -> usize {
            self.code.k()
        }

        fn n(&self) -> usize {
            self.code.n()
        }
    }

    impl<C> GeneratorMatrixAccess for CountingCode<C>
    where
        C: GeneratorMatrixAccess,
    {
        type GeneratorMatrix = C::GeneratorMatrix;

        fn generator_matrix_into(&self, out: &mut Self::GeneratorMatrix) -> Result<(), CodeError> {
            self.generator_calls.fetch_add(1, Ordering::SeqCst);
            self.code.generator_matrix_into(out)
        }

        fn is_systematic(&self) -> Result<bool, CodeError> {
            self.code.is_systematic()
        }
    }

    impl<C> ParityCheckMatrixAccess for CountingCode<C>
    where
        C: ParityCheckMatrixAccess,
    {
        type ParityCheckMatrix = C::ParityCheckMatrix;

        fn parity_check_matrix_into(
            &self,
            out: &mut Self::ParityCheckMatrix,
        ) -> Result<(), CodeError> {
            self.parity_calls.fetch_add(1, Ordering::SeqCst);
            self.code.parity_check_matrix_into(out)
        }
    }

    #[test]
    fn explicit_cache_caches_each_flow_and_uncached_access_recomputes() {
        let generator_calls = Arc::new(AtomicUsize::new(0));
        let parity_calls = Arc::new(AtomicUsize::new(0));
        let counted = CountingCode {
            code: binary_code(),
            generator_calls: generator_calls.clone(),
            parity_calls: parity_calls.clone(),
        };
        let cached = CachedMatrices::new(counted.clone());

        let first_generator = cached.generator_matrix().expect("first cached generator");
        drop(first_generator);
        let second_generator = cached.generator_matrix().expect("cached generator hit");
        assert_eq!(
            second_generator,
            cached.generator_matrix().expect("another hit")
        );
        assert_eq!(generator_calls.load(Ordering::SeqCst), 1);

        let mut generator_buffer = BitMatrix::zeros(cached.k(), cached.n());
        cached
            .generator_matrix_into(&mut generator_buffer)
            .expect("cached generator caller buffer");
        assert_eq!(generator_buffer, second_generator);
        assert_eq!(generator_calls.load(Ordering::SeqCst), 1);

        let first_parity = cached.parity_check_matrix().expect("first cached parity");
        drop(first_parity);
        let second_parity = cached.parity_check_matrix().expect("cached parity hit");
        assert_eq!(parity_calls.load(Ordering::SeqCst), 1);
        let mut parity_buffer = BitMatrix::zeros(cached.redundancy(), cached.n());
        cached
            .parity_check_matrix_into(&mut parity_buffer)
            .expect("cached parity caller buffer");
        assert_eq!(parity_buffer, second_parity);
        assert_eq!(parity_calls.load(Ordering::SeqCst), 1);

        let uncached = counted;
        let _ = uncached.generator_matrix().expect("uncached generator");
        let _ = uncached
            .generator_matrix()
            .expect("uncached generator again");
        let _ = uncached.parity_check_matrix().expect("uncached parity");
        let _ = uncached
            .parity_check_matrix()
            .expect("uncached parity again");
        assert_eq!(generator_calls.load(Ordering::SeqCst), 3);
        assert_eq!(parity_calls.load(Ordering::SeqCst), 3);

        let rebuilt = CachedMatrices::new(CountingCode {
            code: binary_code(),
            generator_calls: generator_calls.clone(),
            parity_calls: parity_calls.clone(),
        });
        drop(
            rebuilt
                .generator_matrix()
                .expect("rebuild after cache drop"),
        );
        assert_eq!(generator_calls.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn explicit_cache_caches_field_generic_matrix_flows() {
        let generator_calls = Arc::new(AtomicUsize::new(0));
        let parity_calls = Arc::new(AtomicUsize::new(0));
        let counted = CountingCode {
            code: gf5_code(5),
            generator_calls: generator_calls.clone(),
            parity_calls: parity_calls.clone(),
        };
        let cached = CachedMatrices::new(counted);

        let generator = cached.generator_matrix().expect("field generator");
        let mut generator_buffer = FieldMatrix::zeros(cached.k(), cached.n());
        cached
            .generator_matrix_into(&mut generator_buffer)
            .expect("field generator caller buffer");
        assert_eq!(generator_buffer, generator);
        assert_eq!(generator_calls.load(Ordering::SeqCst), 1);

        let parity = cached.parity_check_matrix().expect("field parity");
        let mut parity_buffer = FieldMatrix::zeros(cached.redundancy(), cached.n());
        cached
            .parity_check_matrix_into(&mut parity_buffer)
            .expect("field parity caller buffer");
        assert_eq!(parity_buffer, parity);
        assert_eq!(parity_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn explicit_cache_clear_rebuilds_without_affecting_the_code() {
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = CountingCode {
            code: binary_code(),
            generator_calls: calls.clone(),
            parity_calls: Arc::new(AtomicUsize::new(0)),
        };
        let cached = CachedMatrices::new(counted);
        let _ = cached.generator_matrix().expect("initial materialization");
        cached.clear();
        let _ = cached
            .generator_matrix()
            .expect("materialization after clear");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
