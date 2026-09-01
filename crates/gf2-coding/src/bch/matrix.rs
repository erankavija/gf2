//! Reference generator and parity-check matrix materialization for BCH codes.
//!
//! The construction uses the coefficient-vector convention from [`crate::bch::spec`]:
//! coordinate `i` is the coefficient of `x^i`. The generator row at message
//! coordinate `i` is therefore the coefficient vector of `x^i g(x)`. The
//! parity-check rows are the non-wrapping shifts of the reversed quotient
//! `((x^n - 1) / g(x))^*`. This is the dense, full-rank reference form; it is
//! intentionally separate from encoding and does not retain an implicit
//! matrix cache.
//!
//! [`CachedMatrices`] is the explicit opt-in cache for callers that need to
//! reuse these reference matrices. It caches successful generator and
//! parity-check materializations independently, while its allocating access
//! still returns a fresh matrix on every call.
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
//! ```

use std::any::Any;
use std::sync::Mutex;

use gf2_core::field::extension::{FieldExtension, FieldIdentity};
use gf2_core::field::{FieldPoly, FiniteField};

use crate::bch::spec::BchCode;
use crate::error::CodeError;
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

/// Writes the coefficient-vector generator matrix for `generator` into `out`.
fn write_generator<F, M>(
    generator: &FieldPoly<F>,
    rows: usize,
    cols: usize,
    out: &mut M,
    zero: &F,
) -> Result<(), CodeError>
where
    F: FieldIdentity,
    M: SymbolMatrix<F>,
{
    check_shape(out, rows, cols)?;
    let degree = generator
        .degree()
        .expect("a constructed BCH code has a nonzero generator");

    for row in 0..rows {
        for col in 0..cols {
            let value = if col >= row {
                let offset = col - row;
                if offset <= degree {
                    generator.coeff(offset)
                } else {
                    zero.clone()
                }
            } else {
                zero.clone()
            };
            out.set(row, col, value)?;
        }
    }
    Ok(())
}

/// Writes the full-rank parity-check matrix associated with `generator`.
fn write_parity_check<F, M>(
    generator: &FieldPoly<F>,
    rows: usize,
    cols: usize,
    out: &mut M,
    zero: &F,
) -> Result<(), CodeError>
where
    F: FieldIdentity,
    M: SymbolMatrix<F>,
{
    check_shape(out, rows, cols)?;
    if rows == 0 {
        return Ok(());
    }

    let generator_degree = generator
        .degree()
        .expect("a constructed BCH code has a nonzero generator");
    debug_assert_eq!(generator_degree, rows);
    let quotient_degree = cols - rows;

    // Solve g(x)h(x) = x^n - 1 from the highest coefficient down. At
    // reversed offset `d`, h[k - d] depends only on reversed offsets below
    // it; those values are kept in row zero of the caller-owned output while
    // the remaining rows are still untouched. The leading coefficient of g
    // is one, so no inverse or temporary quotient is needed.
    for reversed_offset in 0..=quotient_degree {
        let mut value = if reversed_offset == 0 {
            zero.one_like()
        } else {
            zero.clone()
        };
        let first_generator_offset = rows.saturating_sub(reversed_offset);
        for generator_offset in first_generator_offset..rows {
            let known_offset = reversed_offset + generator_offset - rows;
            let known = out
                .get(0, known_offset)
                .expect("the quotient prefix is stored in row zero");
            value = value - generator.coeff(generator_offset) * known;
        }
        out.set(0, reversed_offset, value)?;
    }

    for col in quotient_degree + 1..cols {
        out.set(0, col, zero.clone())?;
    }
    for row in 1..rows {
        for col in 0..cols {
            let value = if col >= row && col - row <= quotient_degree {
                out.get(0, col - row)
                    .expect("the reversed quotient is stored in row zero")
            } else {
                zero.clone()
            };
            out.set(row, col, value)?;
        }
    }
    Ok(())
}

impl<X, S, M> GeneratorMatrixAccess for BchCode<X, S, M>
where
    X: FieldExtension,
    S: SymbolSequence<X::Base>,
    M: SymbolMatrix<X::Base>,
{
    type GeneratorMatrix = M;

    fn generator_matrix_into(&self, out: &mut Self::GeneratorMatrix) -> Result<(), CodeError> {
        write_generator(
            self.generator(),
            self.k(),
            self.n(),
            out,
            &self.symbol_zero(),
        )
    }

    fn is_systematic(&self) -> Result<bool, CodeError> {
        let generator = self.generator_matrix()?;
        for row in 0..self.k() {
            for col in 0..self.k() {
                let value = generator
                    .get(row, col)
                    .expect("the materialized generator has its declared shape");
                if (row == col && !value.is_one()) || (row != col && !value.is_zero()) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}

impl<X, S, M> ParityCheckMatrixAccess for BchCode<X, S, M>
where
    X: FieldExtension,
    S: SymbolSequence<X::Base>,
    M: SymbolMatrix<X::Base>,
{
    type ParityCheckMatrix = M;

    fn parity_check_matrix_into(&self, out: &mut Self::ParityCheckMatrix) -> Result<(), CodeError> {
        write_parity_check(
            self.generator(),
            self.redundancy(),
            self.n(),
            out,
            &self.symbol_zero(),
        )
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

    use crate::bch::spec::{BchSpec, BinaryBchCode, DenseBchCode, DesignedDistance};
    use crate::traits::block::{BlockCode, GeneratorMatrixAccess, ParityCheckMatrixAccess};
    use gf2_core::field::extension::BinaryPrimeExt;
    use gf2_core::field::matrix::FieldMatrix;
    use gf2_core::field::modulus_select::select_modulus;
    use gf2_core::field::ConstField;
    use gf2_core::field::FieldPoly;
    use gf2_core::gf2m::Gf2mField;
    use gf2_core::gfp::Fp;
    use gf2_core::gfpn::{QuotientElement, QuotientField};
    use gf2_core::BitMatrix;

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

    fn binary_code_as_dense() -> DenseBchCode<BinaryPrimeExt> {
        DenseBchCode::construct(BchSpec::PrimitiveNarrowSense {
            extension: binary_extension(),
            designed_distance: DesignedDistance::try_from(5).expect("positive distance"),
        })
        .expect("a valid dense binary BCH code")
    }

    fn expected_generator<F: FieldIdentity>(
        generator: &FieldPoly<F>,
        rows: usize,
        cols: usize,
        zero: &F,
    ) -> Vec<Vec<F>> {
        let degree = generator.degree().expect("nonzero generator");
        (0..rows)
            .map(|row| {
                (0..cols)
                    .map(|col| {
                        if col >= row && col - row <= degree {
                            generator.coeff(col - row)
                        } else {
                            zero.clone()
                        }
                    })
                    .collect()
            })
            .collect()
    }

    fn expected_parity<F: FieldIdentity>(
        generator: &FieldPoly<F>,
        rows: usize,
        cols: usize,
        zero: &F,
    ) -> Vec<Vec<F>> {
        // Keep this polynomial construction independent from the production
        // helper so the expected matrix remains a direct oracle.
        let mut cyclic_coefficients = vec![zero.zero_like(); cols + 1];
        cyclic_coefficients[0] = -zero.one_like();
        cyclic_coefficients[cols] = zero.one_like();
        let cyclic = FieldPoly::new(cyclic_coefficients);
        let (quotient, remainder) = cyclic.div_rem(generator);
        assert!(remainder.is_zero());
        let degree = quotient.degree().expect("nonzero quotient");
        (0..rows)
            .map(|row| {
                (0..cols)
                    .map(|col| {
                        if col >= row && col - row <= degree {
                            quotient.coeff(degree - (col - row))
                        } else {
                            zero.clone()
                        }
                    })
                    .collect()
            })
            .collect()
    }

    fn assert_matrix_matches<F, M>(matrix: &M, expected: &[Vec<F>])
    where
        F: FieldIdentity,
        M: SymbolMatrix<F>,
    {
        assert_eq!(matrix.rows(), expected.len());
        assert_eq!(matrix.cols(), expected.first().map_or(0, Vec::len));
        for (row, values) in expected.iter().enumerate() {
            for (col, expected_value) in values.iter().enumerate() {
                assert_eq!(matrix.get(row, col), Some(expected_value.clone()));
            }
        }
    }

    fn assert_rows_are_codewords<F, M>(generator_matrix: &M, generator: &FieldPoly<F>)
    where
        F: FieldIdentity,
        M: SymbolMatrix<F>,
    {
        for row in 0..generator_matrix.rows() {
            let coefficients = (0..generator_matrix.cols())
                .map(|col| {
                    generator_matrix
                        .get(row, col)
                        .expect("valid matrix coordinate")
                })
                .collect();
            let row_polynomial = FieldPoly::new(coefficients);
            assert!(row_polynomial.div_rem(generator).1.is_zero());
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

    #[test]
    fn packed_binary_materialization_matches_polynomial_references() {
        let code = binary_code();
        let generator = code.generator_matrix().expect("generator materialization");
        let parity = code.parity_check_matrix().expect("parity materialization");
        let zero = code.symbol_zero();

        assert_matrix_matches(
            &generator,
            &expected_generator(code.generator(), code.k(), code.n(), &zero),
        );
        assert_matrix_matches(
            &parity,
            &expected_parity(code.generator(), code.redundancy(), code.n(), &zero),
        );
        assert_rows_are_codewords(&generator, code.generator());
        assert_full_row_rank(&generator);
        assert_full_row_rank(&parity);
        assert_orthogonal(&generator, &parity, &zero);
    }

    #[test]
    fn gf5_materialization_has_full_rank_and_orthogonal_checks() {
        let code = DenseBchCode::construct(BchSpec::PrimitiveNarrowSense {
            extension: gf25(),
            designed_distance: DesignedDistance::try_from(5).expect("positive distance"),
        })
        .expect("a valid GF(5) BCH code");
        let generator = code.generator_matrix().expect("generator materialization");
        let parity = code.parity_check_matrix().expect("parity materialization");
        let zero = code.symbol_zero();

        assert_matrix_matches(
            &generator,
            &expected_generator(code.generator(), code.k(), code.n(), &zero),
        );
        assert_matrix_matches(
            &parity,
            &expected_parity(code.generator(), code.redundancy(), code.n(), &zero),
        );
        assert_rows_are_codewords(&generator, code.generator());
        assert_full_row_rank(&generator);
        assert_full_row_rank(&parity);
        assert_orthogonal(&generator, &parity, &zero);
    }

    #[test]
    fn quotient_base_materialization_is_field_generic() {
        let code = DenseBchCode::construct(BchSpec::PrimitiveNarrowSense {
            extension: gf81_over_gf9(),
            designed_distance: DesignedDistance::try_from(4).expect("positive distance"),
        })
        .expect("a valid GF(9) BCH code");
        let generator = code.generator_matrix().expect("generator materialization");
        let parity = code.parity_check_matrix().expect("parity materialization");
        let zero = code.symbol_zero();

        assert_matrix_matches(
            &generator,
            &expected_generator(code.generator(), code.k(), code.n(), &zero),
        );
        assert_matrix_matches(
            &parity,
            &expected_parity(code.generator(), code.redundancy(), code.n(), &zero),
        );
        assert_rows_are_codewords(&generator, code.generator());
        assert_full_row_rank(&generator);
        assert_full_row_rank(&parity);
        assert_orthogonal(&generator, &parity, &zero);
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
            code: DenseBchCode::construct(BchSpec::PrimitiveNarrowSense {
                extension: gf25(),
                designed_distance: DesignedDistance::try_from(5).expect("positive distance"),
            })
            .expect("a valid GF(5) BCH code"),
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
