//! Traits for error-correcting codes.
//!
//! The module holds three groups of interfaces:
//!
//! - [`block`] is the canonical block-code surface. It is generic over the
//!   symbol field and over the representation that stores symbols and
//!   matrices, and it carries packed `BitVec`/`BitMatrix` specializations for
//!   codes over `GF(2)`.
//! - [`compat::binary_v1`] is the named, versioned `binary-code-v1`
//!   compatibility boundary holding the bit-only encoder and generator-matrix
//!   contracts that code families outside the canonical surface implement
//!   directly. Its two traits are re-exported at this module's root, so
//!   `crate::traits::BlockEncoder` and `crate::traits::GeneratorMatrixAccess`
//!   name the version-1 contracts.
//! - The decoder and streaming traits ([`HardDecisionDecoder`],
//!   [`SoftDecoder`], [`IterativeSoftDecoder`], [`StreamingEncoder`],
//!   [`StreamingDecoder`]) together with [`DecoderResult`].
//!
//! # Runtime-erased handles
//!
//! [`ErasedBlockCode`] and its capability-specific companions are leaf
//! adapters over fully constructed static codes. They erase the symbol field
//! and representation so exploratory code can hold heterogeneous code values,
//! but they do not erase construction specifications, select a field, or
//! provide a dynamic alternative to
//! [`FieldExtension`](gf2_core::field::extension::FieldExtension). Static
//! trait calls remain the allocation-free, monomorphized path.
//!
//! A small exploratory collection can therefore retain the static encoders'
//! semantics while choosing the code at run time:
//!
//! ```
//! use gf2_coding::traits::{
//!     block, ErasedBlockCode, ErasedBlockEncoder, ErasedSymbols,
//! };
//! use gf2_coding::traits::block::BlockCode;
//! use gf2_coding::LinearBlockCode;
//! use gf2_core::gfp::Fp;
//! use gf2_core::BitVec;
//!
//! #[derive(Clone)]
//! struct DelegatingCode(LinearBlockCode);
//!
//! impl block::BlockCode for DelegatingCode {
//!     type Symbol = Fp<2>;
//!     type Symbols = BitVec;
//!
//!     fn symbol_zero(&self) -> Self::Symbol {
//!         block::BlockCode::symbol_zero(&self.0)
//!     }
//!
//!     fn k(&self) -> usize {
//!         block::BlockCode::k(&self.0)
//!     }
//!
//!     fn n(&self) -> usize {
//!         block::BlockCode::n(&self.0)
//!     }
//! }
//!
//! impl block::BlockEncoder for DelegatingCode {
//!     fn encode_into(
//!         &self,
//!         message: &Self::Symbols,
//!         codeword: &mut Self::Symbols,
//!     ) -> Result<(), gf2_coding::CodeError> {
//!         block::BlockEncoder::encode_into(&self.0, message, codeword)
//!     }
//! }
//!
//! let binary = LinearBlockCode::hamming(2);
//! let wrapped = DelegatingCode(LinearBlockCode::hamming(3));
//! let codes: Vec<ErasedBlockCode> = vec![
//!     ErasedBlockCode::new(binary.clone()),
//!     ErasedBlockCode::new(wrapped.clone()),
//! ];
//! let encoders = vec![
//!     ErasedBlockEncoder::new(binary.clone()),
//!     ErasedBlockEncoder::new(wrapped.clone()),
//! ];
//!
//! let binary_message = BitVec::zeros(binary.k());
//! let binary_expected = block::BlockEncoder::encode(&binary, &binary_message).unwrap();
//! let binary_actual = encoders[0]
//!     .encode(&ErasedSymbols::new(&binary, binary_message))
//!     .unwrap();
//! assert_eq!(binary_actual.downcast_ref(&binary).unwrap(), &binary_expected);
//!
//! let wrapped_message = BitVec::zeros(wrapped.k());
//! let wrapped_expected = block::BlockEncoder::encode(&wrapped, &wrapped_message).unwrap();
//! let wrapped_actual = encoders[1]
//!     .encode(&ErasedSymbols::new(&wrapped, wrapped_message))
//!     .unwrap();
//! assert_eq!(wrapped_actual.downcast_ref(&wrapped).unwrap(), &wrapped_expected);
//! assert_eq!(codes[0].n(), binary.n());
//! assert_eq!(codes[1].n(), wrapped.n());
//! ```

use crate::error::CodeError;
pub use crate::error::RepresentationId;
use crate::llr::Llr;
use gf2_core::field::extension::FieldId;
use gf2_core::BitVec;
use std::any::{Any, TypeId};
use std::fmt;
use std::sync::Arc;

pub mod block {
    //! The canonical static block-code interfaces.
    //!
    //! A code declares its symbol field through [`BlockCode::Symbol`] and its
    //! storage through [`BlockCode::Symbols`]; the capability traits
    //! [`BlockEncoder`], [`GeneratorMatrixAccess`], and
    //! [`ParityCheckMatrixAccess`] refine it, each owning exactly the
    //! representation its own result needs. A code over `GF(2)` selects
    //! `Fp<2>` with [`BitVec`] and [`BitMatrix`] and is then recognized by the
    //! marker traits [`BinaryBlockCode`], [`BinaryGeneratorMatrixAccess`], and
    //! [`BinaryParityCheckMatrixAccess`], which give binary algorithms the
    //! packed types directly rather than through a parallel trait hierarchy.
    //!
    //! # Dispatch
    //!
    //! These traits are the static half of the static/erased split fixed by
    //! the epic `ae03bcd0` BCH API design document (`bch-api-design.md`,
    //! sections "Static and erased type split" and "Canonical trait surface").
    //! A generic function bounded by them monomorphizes to one concrete symbol
    //! type and one concrete representation per instantiation, so selecting
    //! the representation costs nothing at run time and no encoding or matrix
    //! path acquires a virtual call. Runtime exploration uses the separate
    //! erased handles described there, and no trait in this module accepts an
    //! erased value.
    //!
    //! # Examples
    //!
    //! One generic encoder serves every code; the call site fixes the field
    //! and the representation.
    //!
    //! ```
    //! use gf2_coding::traits::block::{
    //!     BlockCode, BlockEncoder, GeneratorMatrixAccess, SymbolSequence,
    //! };
    //! use gf2_coding::LinearBlockCode;
    //! use gf2_core::field::FiniteField;
    //!
    //! fn encode_basis_word<C: BlockEncoder>(code: &C, index: usize) -> C::Symbols {
    //!     let zero = code.symbol_zero();
    //!     let one = zero.one_like();
    //!     let mut message = C::Symbols::zeroed(code.k(), &zero);
    //!     message.set(index, one).expect("index below k");
    //!     code.encode(&message).expect("a k-symbol message encodes")
    //! }
    //!
    //! let code = LinearBlockCode::hamming(3);
    //! let codeword = encode_basis_word(&code, 0);
    //!
    //! // Row i of the generator is the encoding of message basis vector i.
    //! let generator = code.generator_matrix().expect("the code stores a generator");
    //! for col in 0..BlockCode::n(&code) {
    //!     assert_eq!(codeword.get(col), generator.get(0, col));
    //! }
    //! ```

    use crate::error::CodeError;
    use gf2_core::field::extension::{FieldId, FieldIdentity};
    use gf2_core::field::matrix::FieldMatrix;
    use gf2_core::field::{FieldVec, FiniteField};
    use gf2_core::gfp::Fp;
    use gf2_core::{BitMatrix, BitVec};

    /// Storage contract for a sequence of symbols from `F`.
    ///
    /// Implementations preserve the field identity represented by `zero` and
    /// expose symbols in their canonical order. A packed binary sequence uses
    /// little-endian bit indexing and zero tail padding.
    pub trait SymbolSequence<F>: Clone + core::fmt::Debug + Eq + 'static
    where
        F: FieldIdentity,
    {
        /// Creates a zero-filled sequence of `len` symbols.
        fn zeroed(len: usize, zero: &F) -> Self;

        /// Returns the number of symbols in the sequence.
        fn len(&self) -> usize;

        /// Returns whether the sequence contains no symbols.
        fn is_empty(&self) -> bool {
            self.len() == 0
        }

        /// Returns a copy of the symbol at `index`, or `None` when out of range.
        fn get(&self, index: usize) -> Option<F>;

        /// Stores a symbol at `index`.
        ///
        /// The method validates only the sequence index. Field identity of a
        /// stored symbol is a [`BlockCode`] conformance law, not an input
        /// check performed by this representation contract.
        ///
        /// # Errors
        ///
        /// Returns [`CodeError::IndexOutOfBounds`] when `index` is not in the
        /// sequence.
        fn set(&mut self, index: usize, value: F) -> Result<(), CodeError>;
    }

    /// Storage contract for a rectangular matrix of symbols from `F`.
    ///
    /// Implementations expose canonical row-major coordinates. A packed
    /// binary matrix uses little-endian bit indexing and zero tail padding.
    pub trait SymbolMatrix<F>: Clone + core::fmt::Debug + Eq + 'static
    where
        F: FieldIdentity,
    {
        /// Creates a zero-filled matrix with the requested shape.
        fn zeroed(rows: usize, cols: usize, zero: &F) -> Self;

        /// Returns the number of rows.
        fn rows(&self) -> usize;

        /// Returns the number of columns.
        fn cols(&self) -> usize;

        /// Returns a copy of a cell, or `None` when its coordinates are out of range.
        fn get(&self, row: usize, col: usize) -> Option<F>;

        /// Stores a symbol at `(row, col)`.
        ///
        /// The method validates only the matrix coordinates. Field identity
        /// of a stored symbol is a [`BlockCode`] conformance law, not an input
        /// check performed by this representation contract.
        ///
        /// # Errors
        ///
        /// Returns [`CodeError::IndexOutOfBounds`] when either coordinate is
        /// outside the matrix shape.
        fn set(&mut self, row: usize, col: usize, value: F) -> Result<(), CodeError>;
    }

    impl<F> SymbolSequence<F> for FieldVec<F>
    where
        F: FieldIdentity + 'static,
    {
        fn zeroed(len: usize, zero: &F) -> Self {
            Self::zeros_from(len, zero)
        }

        fn len(&self) -> usize {
            FieldVec::len(self)
        }

        fn get(&self, index: usize) -> Option<F> {
            self.as_slice().get(index).cloned()
        }

        fn set(&mut self, index: usize, value: F) -> Result<(), CodeError> {
            if index >= self.len() {
                return Err(CodeError::IndexOutOfBounds {
                    index,
                    length: self.len(),
                });
            }
            FieldVec::set(self, index, value);
            Ok(())
        }
    }

    impl<F> SymbolMatrix<F> for FieldMatrix<F>
    where
        F: FieldIdentity + 'static,
    {
        fn zeroed(rows: usize, cols: usize, zero: &F) -> Self {
            Self::new(rows, cols, zero.clone())
        }

        fn rows(&self) -> usize {
            FieldMatrix::rows(self)
        }

        fn cols(&self) -> usize {
            FieldMatrix::cols(self)
        }

        fn get(&self, row: usize, col: usize) -> Option<F> {
            if row >= self.rows() || col >= self.cols() {
                None
            } else {
                Some(FieldMatrix::get(self, row, col))
            }
        }

        fn set(&mut self, row: usize, col: usize, value: F) -> Result<(), CodeError> {
            if row >= self.rows() {
                return Err(CodeError::IndexOutOfBounds {
                    index: row,
                    length: self.rows(),
                });
            }
            if col >= self.cols() {
                return Err(CodeError::IndexOutOfBounds {
                    index: col,
                    length: self.cols(),
                });
            }
            FieldMatrix::set(self, row, col, value);
            Ok(())
        }
    }

    impl SymbolSequence<Fp<2>> for BitVec {
        fn zeroed(len: usize, _zero: &Fp<2>) -> Self {
            Self::zeros(len)
        }

        fn len(&self) -> usize {
            BitVec::len(self)
        }

        fn get(&self, index: usize) -> Option<Fp<2>> {
            (index < self.len()).then(|| Fp::<2>::new(self.get(index) as u64))
        }

        fn set(&mut self, index: usize, value: Fp<2>) -> Result<(), CodeError> {
            if index >= self.len() {
                return Err(CodeError::IndexOutOfBounds {
                    index,
                    length: self.len(),
                });
            }
            BitVec::set(self, index, value.is_one());
            Ok(())
        }
    }

    impl SymbolMatrix<Fp<2>> for BitMatrix {
        fn zeroed(rows: usize, cols: usize, _zero: &Fp<2>) -> Self {
            Self::zeros(rows, cols)
        }

        fn rows(&self) -> usize {
            BitMatrix::rows(self)
        }

        fn cols(&self) -> usize {
            BitMatrix::cols(self)
        }

        fn get(&self, row: usize, col: usize) -> Option<Fp<2>> {
            (row < self.rows() && col < self.cols())
                .then(|| Fp::<2>::new(self.get(row, col) as u64))
        }

        fn set(&mut self, row: usize, col: usize, value: Fp<2>) -> Result<(), CodeError> {
            if row >= self.rows() {
                return Err(CodeError::IndexOutOfBounds {
                    index: row,
                    length: self.rows(),
                });
            }
            if col >= self.cols() {
                return Err(CodeError::IndexOutOfBounds {
                    index: col,
                    length: self.cols(),
                });
            }
            BitMatrix::set(self, row, col, value.is_one());
            Ok(())
        }
    }

    /// Common code dimensions and symbol-field contract.
    pub trait BlockCode {
        /// The field element type used by the code.
        type Symbol: FieldIdentity;
        /// The sequence representation used for messages and codewords.
        type Symbols: SymbolSequence<Self::Symbol>;

        /// Returns a zero witness for the code-symbol field.
        fn symbol_zero(&self) -> Self::Symbol;

        /// Returns the message dimension `k`.
        fn k(&self) -> usize;

        /// Returns the codeword length `n`.
        fn n(&self) -> usize;

        /// Returns the redundancy `n - k`.
        ///
        /// # Panics
        ///
        /// Panics if an implementor violates the `k <= n` block-code law.
        fn redundancy(&self) -> usize {
            self.n() - self.k()
        }

        /// Returns the mathematical identity of the symbol field.
        fn symbol_field_id(&self) -> FieldId {
            self.symbol_zero().field_id()
        }
    }

    /// Encodes canonical symbol sequences for a block code.
    pub trait BlockEncoder: BlockCode {
        /// Encodes into an already sized `n()`-symbol buffer.
        ///
        /// This is the primitive operation: the caller owns the output
        /// buffer, so encoding allocates no result. Implementations validate
        /// `message.len() == k()`, `codeword.len() == n()`, and any
        /// layout-specific precondition before writing any output symbol.
        ///
        /// # Errors
        ///
        /// Returns a [`CodeError`] when the message or output buffer has the
        /// wrong length or a code-specific precondition is not met.
        fn encode_into(
            &self,
            message: &Self::Symbols,
            codeword: &mut Self::Symbols,
        ) -> Result<(), CodeError>;

        /// Encodes a message into a newly allocated symbol sequence.
        ///
        /// # Errors
        ///
        /// Propagates the [`CodeError`] returned by [`Self::encode_into`].
        fn encode(&self, message: &Self::Symbols) -> Result<Self::Symbols, CodeError> {
            let mut codeword = Self::Symbols::zeroed(self.n(), &self.symbol_zero());
            self.encode_into(message, &mut codeword)?;
            Ok(codeword)
        }
    }

    /// Provides a canonical generator matrix.
    pub trait GeneratorMatrixAccess: BlockCode {
        /// The matrix representation used for the generator.
        type GeneratorMatrix: SymbolMatrix<Self::Symbol>;

        /// Writes the `k() x n()` generator matrix into `out`.
        ///
        /// # Errors
        ///
        /// Returns a [`CodeError`] when `out` has the wrong shape or the
        /// generator is unavailable.
        fn generator_matrix_into(&self, out: &mut Self::GeneratorMatrix) -> Result<(), CodeError>;

        /// Materializes the generator matrix.
        ///
        /// # Errors
        ///
        /// Propagates the [`CodeError`] returned by
        /// [`Self::generator_matrix_into`].
        fn generator_matrix(&self) -> Result<Self::GeneratorMatrix, CodeError> {
            let mut out = Self::GeneratorMatrix::zeroed(self.k(), self.n(), &self.symbol_zero());
            self.generator_matrix_into(&mut out)?;
            Ok(out)
        }

        /// Reports whether the generator is the identity on the code's
        /// message coordinates in the exposed user layout.
        ///
        /// A code in canonical systematic form has those coordinates at
        /// columns `0..k()`. A code that records another message-coordinate
        /// order answers for that order, which is the equivalent cheap fact
        /// for its own layout.
        ///
        /// # Errors
        ///
        /// Returns a [`CodeError`] when the implementation cannot determine
        /// the property.
        fn is_systematic(&self) -> Result<bool, CodeError>;
    }

    /// Provides a canonical full-rank parity-check matrix.
    pub trait ParityCheckMatrixAccess: BlockCode {
        /// The matrix representation used for the parity check.
        type ParityCheckMatrix: SymbolMatrix<Self::Symbol>;

        /// Returns the number of parity-check rows.
        fn parity_check_rows(&self) -> usize {
            self.redundancy()
        }

        /// Writes the parity-check matrix into `out`.
        ///
        /// # Errors
        ///
        /// Returns a [`CodeError`] when the matrix is unavailable or `out`
        /// has the wrong shape.
        fn parity_check_matrix_into(
            &self,
            out: &mut Self::ParityCheckMatrix,
        ) -> Result<(), CodeError>;

        /// Materializes the parity-check matrix.
        ///
        /// # Errors
        ///
        /// Propagates the [`CodeError`] returned by
        /// [`Self::parity_check_matrix_into`].
        fn parity_check_matrix(&self) -> Result<Self::ParityCheckMatrix, CodeError> {
            let mut out = Self::ParityCheckMatrix::zeroed(
                self.parity_check_rows(),
                self.n(),
                &self.symbol_zero(),
            );
            self.parity_check_matrix_into(&mut out)?;
            Ok(out)
        }
    }

    /// Identifies a code whose canonical symbol representation is packed binary.
    pub trait BinaryBlockCode: BlockCode<Symbol = Fp<2>, Symbols = BitVec> {}

    impl<T> BinaryBlockCode for T where T: BlockCode<Symbol = Fp<2>, Symbols = BitVec> {}

    /// Identifies a binary code with a packed canonical generator matrix.
    pub trait BinaryGeneratorMatrixAccess:
        BinaryBlockCode + GeneratorMatrixAccess<GeneratorMatrix = BitMatrix>
    {
    }

    impl<T> BinaryGeneratorMatrixAccess for T where
        T: BinaryBlockCode + GeneratorMatrixAccess<GeneratorMatrix = BitMatrix>
    {
    }

    /// Identifies a binary code with a packed canonical parity-check matrix.
    pub trait BinaryParityCheckMatrixAccess:
        BinaryBlockCode + ParityCheckMatrixAccess<ParityCheckMatrix = BitMatrix>
    {
    }

    impl<T> BinaryParityCheckMatrixAccess for T where
        T: BinaryBlockCode + ParityCheckMatrixAccess<ParityCheckMatrix = BitMatrix>
    {
    }

    /// Shared behavioral checks for canonical representations and code capabilities.
    #[cfg(test)]
    pub(crate) mod conformance {
        use super::*;
        use crate::error::RepresentationId;
        use crate::traits::compat::binary_v1;

        pub(crate) fn symbol_sequence_contract<F, S>(zero: &F, one: &F)
        where
            F: FieldIdentity + 'static,
            S: SymbolSequence<F>,
        {
            for &len in &[0usize, 1, 63, 64, 65] {
                let mut sequence = S::zeroed(len, zero);
                assert_eq!(sequence.len(), len);
                assert_eq!(sequence.is_empty(), len == 0);
                for index in 0..len {
                    assert_eq!(sequence.get(index), Some(zero.clone()));
                }
                assert_eq!(sequence.get(len), None);
                assert_eq!(
                    sequence.set(len, one.clone()),
                    Err(CodeError::IndexOutOfBounds {
                        index: len,
                        length: len
                    })
                );
                if len != 0 {
                    let mut indices = vec![0, len / 2, len - 1];
                    indices.dedup();
                    for index in indices {
                        sequence.set(index, one.clone()).unwrap();
                        for position in 0..len {
                            assert_eq!(
                                sequence.get(position),
                                Some(if position == index {
                                    one.clone()
                                } else {
                                    zero.clone()
                                })
                            );
                        }
                        sequence.set(index, zero.clone()).unwrap();
                    }
                }
            }
        }

        pub(crate) fn symbol_matrix_contract<F, M>(zero: &F, one: &F)
        where
            F: FieldIdentity + 'static,
            M: SymbolMatrix<F>,
        {
            for &(rows, cols) in &[(0usize, 0usize), (1, 1), (2, 63), (3, 64), (4, 65)] {
                let mut matrix = M::zeroed(rows, cols, zero);
                assert_eq!(matrix.rows(), rows);
                assert_eq!(matrix.cols(), cols);
                for row in 0..rows {
                    for col in 0..cols {
                        assert_eq!(matrix.get(row, col), Some(zero.clone()));
                    }
                }
                assert_eq!(matrix.get(rows, 0), None);
                assert_eq!(matrix.get(0, cols), None);
                assert_eq!(
                    matrix.set(rows, 0, one.clone()),
                    Err(CodeError::IndexOutOfBounds {
                        index: rows,
                        length: rows
                    })
                );
                assert_eq!(
                    matrix.set(0, cols, one.clone()),
                    Err(CodeError::IndexOutOfBounds {
                        index: cols,
                        length: cols
                    })
                );
                if rows != 0 && cols != 0 {
                    let row_targets = [0, rows / 2, rows - 1];
                    let col_targets = [0, cols / 2, cols - 1];
                    for &row in &row_targets {
                        for &col in &col_targets {
                            matrix.set(row, col, one.clone()).unwrap();
                            for current_row in 0..rows {
                                for current_col in 0..cols {
                                    assert_eq!(
                                        matrix.get(current_row, current_col),
                                        Some(if (current_row, current_col) == (row, col) {
                                            one.clone()
                                        } else {
                                            zero.clone()
                                        })
                                    );
                                }
                            }
                            matrix.set(row, col, zero.clone()).unwrap();
                        }
                    }
                }
            }
        }

        pub(crate) fn block_encoder_contract<C>(code: &C, message: &C::Symbols)
        where
            C: BlockEncoder,
        {
            assert!(code.k() <= code.n());
            assert_eq!(code.redundancy(), code.n() - code.k());
            let expected = code.encode(message).unwrap();
            let mut actual = C::Symbols::zeroed(code.n(), &code.symbol_zero());
            code.encode_into(message, &mut actual).unwrap();
            assert_eq!(actual, expected);

            let wrong_message = C::Symbols::zeroed(code.k() + 1, &code.symbol_zero());
            assert!(code.encode(&wrong_message).is_err());
            let mut wrong_output = C::Symbols::zeroed(code.n() + 1, &code.symbol_zero());
            assert!(code.encode_into(message, &mut wrong_output).is_err());
        }

        pub(crate) fn generator_matrix_contract<C>(code: &C)
        where
            C: GeneratorMatrixAccess,
        {
            let expected = code.generator_matrix().unwrap();
            assert_eq!(expected.rows(), code.k());
            assert_eq!(expected.cols(), code.n());
            let mut actual = C::GeneratorMatrix::zeroed(code.k(), code.n(), &code.symbol_zero());
            code.generator_matrix_into(&mut actual).unwrap();
            assert_eq!(actual, expected);
        }

        pub(crate) fn generator_rows_encode_basis<C>(code: &C, one: &C::Symbol)
        where
            C: BlockEncoder + GeneratorMatrixAccess,
        {
            let generator = code.generator_matrix().unwrap();
            for row in 0..code.k() {
                let mut basis = C::Symbols::zeroed(code.k(), &code.symbol_zero());
                basis.set(row, one.clone()).unwrap();
                let encoded = code.encode(&basis).unwrap();
                for col in 0..code.n() {
                    assert_eq!(encoded.get(col), generator.get(row, col));
                }
            }
        }

        pub(crate) fn parity_check_matrix_contract<C>(code: &C)
        where
            C: ParityCheckMatrixAccess,
        {
            let allocated = code.parity_check_matrix();
            let mut caller = C::ParityCheckMatrix::zeroed(
                code.parity_check_rows(),
                code.n(),
                &code.symbol_zero(),
            );
            let written = code.parity_check_matrix_into(&mut caller);
            match (allocated, written) {
                (
                    Err(CodeError::CapabilityUnavailable { capability: left }),
                    Err(CodeError::CapabilityUnavailable { capability: right }),
                ) => assert_eq!(left, right),
                (Ok(expected), Ok(())) => {
                    assert_eq!(expected.rows(), code.parity_check_rows());
                    assert_eq!(expected.cols(), code.n());
                    assert_eq!(caller, expected);
                }
                _ => panic!("parity-check access disagrees"),
            }
        }

        /// Asserts `G · Hᵀ = 0` for a code exposing both matrices.
        ///
        /// A code without a parity-check capability satisfies the law
        /// vacuously and the check returns.
        pub(crate) fn generator_parity_orthogonality<C>(code: &C)
        where
            C: GeneratorMatrixAccess + ParityCheckMatrixAccess,
        {
            let Ok(parity) = code.parity_check_matrix() else {
                return;
            };
            let generator = code.generator_matrix().unwrap();
            for row in 0..generator.rows() {
                for check in 0..parity.rows() {
                    let mut sum = code.symbol_zero();
                    for col in 0..code.n() {
                        let left = generator.get(row, col).expect("generator cell");
                        let right = parity.get(check, col).expect("parity cell");
                        sum += left * right;
                    }
                    assert!(
                        sum.is_zero(),
                        "G · Hᵀ must vanish at row {row}, check {check}"
                    );
                }
            }
        }

        pub(crate) fn binary_v1_encoder_agrees<C>(code: &C, message: &BitVec)
        where
            C: BinaryBlockCode + BlockEncoder,
        {
            let canonical = code.encode(message).unwrap();
            let v1 = binary_v1::BlockEncoder::encode(code, message);
            assert_eq!(binary_v1::BlockEncoder::k(code), code.k());
            assert_eq!(binary_v1::BlockEncoder::n(code), code.n());
            assert_eq!(v1, canonical);
        }

        pub(crate) fn symbol_representation_of<C: BlockCode>() -> RepresentationId {
            RepresentationId::of::<C::Symbols>()
        }

        #[derive(Clone, Debug)]
        pub(crate) struct RepetitionCode<F: FieldIdentity + 'static> {
            repetitions: usize,
            zero: F,
        }

        impl<F: FieldIdentity + 'static> RepetitionCode<F> {
            pub(crate) fn new(repetitions: usize, zero: F) -> Self {
                assert!(repetitions > 0);
                Self { repetitions, zero }
            }
        }

        impl<F: FieldIdentity + 'static> BlockCode for RepetitionCode<F> {
            type Symbol = F;
            type Symbols = FieldVec<F>;

            fn symbol_zero(&self) -> Self::Symbol {
                self.zero.clone()
            }

            fn k(&self) -> usize {
                1
            }

            fn n(&self) -> usize {
                self.repetitions
            }
        }

        impl<F: FieldIdentity + 'static> BlockEncoder for RepetitionCode<F> {
            fn encode_into(
                &self,
                message: &Self::Symbols,
                codeword: &mut Self::Symbols,
            ) -> Result<(), CodeError> {
                if message.len() != self.k() {
                    return Err(CodeError::BufferLengthMismatch {
                        expected: self.k(),
                        actual: message.len(),
                    });
                }
                if codeword.len() != self.n() {
                    return Err(CodeError::BufferLengthMismatch {
                        expected: self.n(),
                        actual: codeword.len(),
                    });
                }
                let value = <FieldVec<F> as SymbolSequence<F>>::get(message, 0)
                    .expect("validated message length");
                for index in 0..self.n() {
                    <FieldVec<F> as SymbolSequence<F>>::set(codeword, index, value.clone())?;
                }
                Ok(())
            }
        }

        impl<F: FieldIdentity + 'static> GeneratorMatrixAccess for RepetitionCode<F> {
            type GeneratorMatrix = FieldMatrix<F>;

            fn generator_matrix_into(
                &self,
                out: &mut Self::GeneratorMatrix,
            ) -> Result<(), CodeError> {
                if out.rows() != self.k() || out.cols() != self.n() {
                    return Err(CodeError::ShapeMismatch {
                        expected_rows: self.k(),
                        expected_cols: self.n(),
                        actual_rows: out.rows(),
                        actual_cols: out.cols(),
                    });
                }
                let one = self.zero.one_like();
                for col in 0..self.n() {
                    <FieldMatrix<F> as SymbolMatrix<F>>::set(out, 0, col, one.clone())?;
                }
                Ok(())
            }

            fn is_systematic(&self) -> Result<bool, CodeError> {
                Ok(self.n() == 1)
            }
        }

        impl<F: FieldIdentity + 'static> ParityCheckMatrixAccess for RepetitionCode<F> {
            type ParityCheckMatrix = FieldMatrix<F>;

            fn parity_check_matrix_into(
                &self,
                out: &mut Self::ParityCheckMatrix,
            ) -> Result<(), CodeError> {
                if out.rows() != self.parity_check_rows() || out.cols() != self.n() {
                    return Err(CodeError::ShapeMismatch {
                        expected_rows: self.parity_check_rows(),
                        expected_cols: self.n(),
                        actual_rows: out.rows(),
                        actual_cols: out.cols(),
                    });
                }
                let one = self.zero.one_like();
                let minus_one = -one.clone();
                for row in 0..self.parity_check_rows() {
                    <FieldMatrix<F> as SymbolMatrix<F>>::set(out, row, 0, one.clone())?;
                    <FieldMatrix<F> as SymbolMatrix<F>>::set(out, row, row + 1, minus_one.clone())?;
                }
                Ok(())
            }
        }
    }
}

/// The object-safe metadata vtable shared by the four erased capabilities.
trait ErasedCodeVtable: Send + Sync {
    fn field_id(&self) -> &FieldId;
    fn symbol_representation(&self) -> RepresentationId;
    fn k(&self) -> usize;
    fn n(&self) -> usize;
    fn code_type_id(&self) -> TypeId;
    fn code_as_any(&self) -> &dyn Any;
}

/// A static code kept behind one or more capability-specific erased vtables.
struct ErasedStaticCode<C> {
    code: C,
    field_id: FieldId,
}

impl<C> ErasedCodeVtable for ErasedStaticCode<C>
where
    C: block::BlockCode + Send + Sync + 'static,
{
    fn field_id(&self) -> &FieldId {
        &self.field_id
    }

    fn symbol_representation(&self) -> RepresentationId {
        RepresentationId::of::<C::Symbols>()
    }

    fn k(&self) -> usize {
        block::BlockCode::k(&self.code)
    }

    fn n(&self) -> usize {
        block::BlockCode::n(&self.code)
    }

    fn code_type_id(&self) -> TypeId {
        TypeId::of::<C>()
    }

    fn code_as_any(&self) -> &dyn Any {
        &self.code
    }
}

/// A type-erased sequence of symbols.
///
/// The field and representation identities are retained beside the
/// `Any` payload. Use [`ErasedSymbols::new`] with the static code that owns a
/// value, then use [`ErasedSymbols::downcast_ref`] or
/// [`ErasedSymbols::downcast`] with that code to recover it. The checked
/// operations return [`CodeError::FieldMismatch`] before a representation
/// check, and never coerce a value between fields or representations.
#[derive(Clone)]
pub struct ErasedSymbols {
    field_id: FieldId,
    representation: RepresentationId,
    value: Arc<dyn Any + Send + Sync>,
}

impl fmt::Debug for ErasedSymbols {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ErasedSymbols")
            .field("field_id", &self.field_id)
            .field("representation", &self.representation)
            .finish_non_exhaustive()
    }
}

impl ErasedSymbols {
    /// Erases a symbol sequence owned by `code`.
    ///
    /// This constructor does not inspect or change the sequence. The caller
    /// supplies the static code so the runtime field identity is retained even
    /// for an empty sequence.
    pub fn new<C>(code: &C, value: C::Symbols) -> Self
    where
        C: block::BlockCode + ?Sized,
        C::Symbol: Send + Sync + 'static,
        C::Symbols: Send + Sync + 'static,
    {
        Self {
            field_id: code.symbol_field_id(),
            representation: RepresentationId::of::<C::Symbols>(),
            value: Arc::new(value),
        }
    }

    /// Alias for [`ErasedSymbols::new`] that makes the static ownership
    /// boundary explicit at a call site.
    pub fn from_code<C>(code: &C, value: C::Symbols) -> Self
    where
        C: block::BlockCode + ?Sized,
        C::Symbol: Send + Sync + 'static,
        C::Symbols: Send + Sync + 'static,
    {
        Self::new(code, value)
    }

    /// Returns the mathematical field identity carried by this value.
    pub fn field_id(&self) -> &FieldId {
        &self.field_id
    }

    /// Returns the process-local identity of the symbol representation.
    pub fn representation(&self) -> RepresentationId {
        self.representation
    }

    /// Recovers the sequence after checking it against its static code.
    ///
    /// # Errors
    ///
    /// Returns [`CodeError::FieldMismatch`] or
    /// [`CodeError::RepresentationMismatch`] when `code` does not own this
    /// erased value. A representation downcast is attempted only after both
    /// identities match.
    pub fn downcast_ref<C>(&self, code: &C) -> Result<&C::Symbols, CodeError>
    where
        C: block::BlockCode + ?Sized,
        C::Symbol: Send + Sync + 'static,
        C::Symbols: Send + Sync + 'static,
    {
        let expected_field = code.symbol_field_id();
        let expected_representation = RepresentationId::of::<C::Symbols>();
        validate_erased_value(
            &expected_field,
            expected_representation,
            &self.field_id,
            self.representation,
        )?;

        if self.value.as_ref().type_id() != TypeId::of::<C::Symbols>() {
            return Err(CodeError::RepresentationMismatch {
                expected: expected_representation,
                found: self.representation,
            });
        }
        match self.value.downcast_ref::<C::Symbols>() {
            Some(value) => Ok(value),
            None => Err(CodeError::RepresentationMismatch {
                expected: expected_representation,
                found: self.representation,
            }),
        }
    }

    /// Recovers an owned reference-counted sequence after checked validation.
    ///
    /// The returned [`Arc`] shares the erased payload. This method consumes
    /// the handle, but cloned handles remain independent owners of the same
    /// payload.
    ///
    /// # Errors
    ///
    /// Returns [`CodeError::FieldMismatch`] or
    /// [`CodeError::RepresentationMismatch`] when `code` does not own this
    /// erased value.
    pub fn downcast<C>(self, code: &C) -> Result<Arc<C::Symbols>, CodeError>
    where
        C: block::BlockCode + ?Sized,
        C::Symbol: Send + Sync + 'static,
        C::Symbols: Send + Sync + 'static,
    {
        let expected_field = code.symbol_field_id();
        let expected_representation = RepresentationId::of::<C::Symbols>();
        validate_erased_value(
            &expected_field,
            expected_representation,
            &self.field_id,
            self.representation,
        )?;

        if self.value.as_ref().type_id() != TypeId::of::<C::Symbols>() {
            return Err(CodeError::RepresentationMismatch {
                expected: expected_representation,
                found: self.representation,
            });
        }
        match self.value.downcast::<C::Symbols>() {
            Ok(value) => Ok(value),
            Err(_) => Err(CodeError::RepresentationMismatch {
                expected: expected_representation,
                found: self.representation,
            }),
        }
    }
}

/// A type-erased matrix value.
///
/// As with [`ErasedSymbols`], this is a leaf adapter over a matrix already
/// materialized by a static code. Its checked downcasts validate both the
/// mathematical field and the process-local Rust representation identity.
#[derive(Clone)]
pub struct ErasedMatrix {
    field_id: FieldId,
    representation: RepresentationId,
    value: Arc<dyn Any + Send + Sync>,
}

impl fmt::Debug for ErasedMatrix {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ErasedMatrix")
            .field("field_id", &self.field_id)
            .field("representation", &self.representation)
            .finish_non_exhaustive()
    }
}

impl ErasedMatrix {
    /// Erases a matrix whose symbols belong to `code`.
    pub fn new<C, M>(code: &C, value: M) -> Self
    where
        C: block::BlockCode + ?Sized,
        C::Symbol: Send + Sync + 'static,
        M: block::SymbolMatrix<C::Symbol> + Send + Sync + 'static,
    {
        Self {
            field_id: code.symbol_field_id(),
            representation: RepresentationId::of::<M>(),
            value: Arc::new(value),
        }
    }

    /// Alias for [`ErasedMatrix::new`] that makes the static ownership
    /// boundary explicit at a call site.
    pub fn from_code<C, M>(code: &C, value: M) -> Self
    where
        C: block::BlockCode + ?Sized,
        C::Symbol: Send + Sync + 'static,
        M: block::SymbolMatrix<C::Symbol> + Send + Sync + 'static,
    {
        Self::new(code, value)
    }

    /// Returns the mathematical field identity carried by this matrix.
    pub fn field_id(&self) -> &FieldId {
        &self.field_id
    }

    /// Returns the process-local identity of the matrix representation.
    pub fn representation(&self) -> RepresentationId {
        self.representation
    }

    /// Recovers a matrix after checking it against a static code and type.
    ///
    /// # Errors
    ///
    /// Returns [`CodeError::FieldMismatch`] or
    /// [`CodeError::RepresentationMismatch`] when the supplied field or
    /// matrix representation differs. A matrix downcast is attempted only
    /// after both identities match.
    pub fn downcast_ref<C, M>(&self, code: &C) -> Result<&M, CodeError>
    where
        C: block::BlockCode + ?Sized,
        C::Symbol: Send + Sync + 'static,
        M: block::SymbolMatrix<C::Symbol> + Send + Sync + 'static,
    {
        let expected_field = code.symbol_field_id();
        let expected_representation = RepresentationId::of::<M>();
        validate_erased_value(
            &expected_field,
            expected_representation,
            &self.field_id,
            self.representation,
        )?;

        if self.value.as_ref().type_id() != TypeId::of::<M>() {
            return Err(CodeError::RepresentationMismatch {
                expected: expected_representation,
                found: self.representation,
            });
        }
        match self.value.downcast_ref::<M>() {
            Some(value) => Ok(value),
            None => Err(CodeError::RepresentationMismatch {
                expected: expected_representation,
                found: self.representation,
            }),
        }
    }

    /// Recovers an owned reference-counted matrix after checked validation.
    ///
    /// # Errors
    ///
    /// Returns [`CodeError::FieldMismatch`] or
    /// [`CodeError::RepresentationMismatch`] when the supplied field or
    /// matrix representation differs.
    pub fn downcast<C, M>(self, code: &C) -> Result<Arc<M>, CodeError>
    where
        C: block::BlockCode + ?Sized,
        C::Symbol: Send + Sync + 'static,
        M: block::SymbolMatrix<C::Symbol> + Send + Sync + 'static,
    {
        let expected_field = code.symbol_field_id();
        let expected_representation = RepresentationId::of::<M>();
        validate_erased_value(
            &expected_field,
            expected_representation,
            &self.field_id,
            self.representation,
        )?;

        if self.value.as_ref().type_id() != TypeId::of::<M>() {
            return Err(CodeError::RepresentationMismatch {
                expected: expected_representation,
                found: self.representation,
            });
        }
        match self.value.downcast::<M>() {
            Ok(value) => Ok(value),
            Err(_) => Err(CodeError::RepresentationMismatch {
                expected: expected_representation,
                found: self.representation,
            }),
        }
    }
}

/// Validates the two runtime identities before an `Any` downcast.
fn validate_erased_value(
    expected_field: &FieldId,
    expected_representation: RepresentationId,
    found_field: &FieldId,
    found_representation: RepresentationId,
) -> Result<(), CodeError> {
    if expected_field != found_field {
        return Err(CodeError::FieldMismatch {
            expected: expected_field.clone(),
            found: found_field.clone(),
        });
    }
    if expected_representation != found_representation {
        return Err(CodeError::RepresentationMismatch {
            expected: expected_representation,
            found: found_representation,
        });
    }
    Ok(())
}

/// A type-erased view of a static [`block::BlockCode`].
///
/// This is metadata only: it has no construction or encoding behavior and
/// never chooses a field. Use [`ErasedBlockEncoder`] for the encoding
/// capability.
#[derive(Clone)]
pub struct ErasedBlockCode {
    inner: Arc<dyn ErasedCodeVtable>,
}

impl ErasedBlockCode {
    /// Wraps a fully constructed static block code.
    pub fn new<C>(code: C) -> Self
    where
        C: block::BlockCode + Send + Sync + 'static,
        C::Symbol: Send + Sync + 'static,
        C::Symbols: Send + Sync + 'static,
    {
        let field_id = code.symbol_field_id();
        Self {
            inner: Arc::new(ErasedStaticCode { code, field_id }),
        }
    }

    fn from_vtable(inner: Arc<dyn ErasedCodeVtable>) -> Self {
        Self { inner }
    }

    /// Returns the mathematical identity of the code-symbol field.
    pub fn field_id(&self) -> &FieldId {
        self.inner.field_id()
    }

    /// Returns the process-local identity of the symbol sequence type.
    pub fn symbol_representation(&self) -> RepresentationId {
        self.inner.symbol_representation()
    }

    /// Returns the message dimension `k`.
    pub fn k(&self) -> usize {
        self.inner.k()
    }

    /// Returns the codeword length `n`.
    pub fn n(&self) -> usize {
        self.inner.n()
    }

    /// Recovers the concrete static code when its type is known.
    ///
    /// The `TypeId` check is performed before the `Any` downcast. A mismatch
    /// returns `None` and cannot panic or coerce one code type into another.
    pub fn downcast_ref<C: 'static>(&self) -> Option<&C> {
        if self.inner.code_type_id() != TypeId::of::<C>() {
            return None;
        }
        self.inner.code_as_any().downcast_ref::<C>()
    }
}

/// The object-safe encoding capability vtable.
trait ErasedEncoderVtable: ErasedCodeVtable {
    fn encode_erased(&self, message: &ErasedSymbols) -> Result<ErasedSymbols, CodeError>;
}

impl<C> ErasedEncoderVtable for ErasedStaticCode<C>
where
    C: block::BlockEncoder + Send + Sync + 'static,
    C::Symbol: Send + Sync + 'static,
    C::Symbols: Send + Sync + 'static,
{
    fn encode_erased(&self, message: &ErasedSymbols) -> Result<ErasedSymbols, CodeError> {
        let message = message.downcast_ref(&self.code)?;
        let codeword = block::BlockEncoder::encode(&self.code, message)?;
        Ok(ErasedSymbols::new(&self.code, codeword))
    }
}

/// A type-erased view of a static [`block::BlockEncoder`].
#[derive(Clone)]
pub struct ErasedBlockEncoder {
    code: ErasedBlockCode,
    inner: Arc<dyn ErasedEncoderVtable>,
}

impl ErasedBlockEncoder {
    /// Wraps a fully constructed static encoder.
    pub fn new<C>(encoder: C) -> Self
    where
        C: block::BlockEncoder + Send + Sync + 'static,
        C::Symbol: Send + Sync + 'static,
        C::Symbols: Send + Sync + 'static,
    {
        let field_id = encoder.symbol_field_id();
        let inner = Arc::new(ErasedStaticCode {
            code: encoder,
            field_id,
        });
        let code = ErasedBlockCode::from_vtable(inner.clone());
        let inner: Arc<dyn ErasedEncoderVtable> = inner;
        Self { code, inner }
    }

    /// Returns the metadata view for this encoder.
    pub fn code(&self) -> &ErasedBlockCode {
        &self.code
    }

    /// Encodes a checked erased message.
    ///
    /// # Errors
    ///
    /// Returns field or representation mismatch errors before invoking the
    /// static encoder, and propagates its ordinary [`CodeError`] results.
    pub fn encode(&self, message: &ErasedSymbols) -> Result<ErasedSymbols, CodeError> {
        self.inner.encode_erased(message)
    }
}

/// The object-safe generator-matrix capability vtable.
trait ErasedGeneratorVtable: ErasedCodeVtable {
    fn matrix_representation(&self) -> RepresentationId;
    fn generator_matrix_erased(&self) -> Result<ErasedMatrix, CodeError>;
}

impl<C> ErasedGeneratorVtable for ErasedStaticCode<C>
where
    C: block::GeneratorMatrixAccess + Send + Sync + 'static,
    C::Symbol: Send + Sync + 'static,
    C::Symbols: Send + Sync + 'static,
    C::GeneratorMatrix: Send + Sync + 'static,
{
    fn matrix_representation(&self) -> RepresentationId {
        RepresentationId::of::<C::GeneratorMatrix>()
    }

    fn generator_matrix_erased(&self) -> Result<ErasedMatrix, CodeError> {
        let matrix = block::GeneratorMatrixAccess::generator_matrix(&self.code)?;
        Ok(ErasedMatrix::new(&self.code, matrix))
    }
}

/// A type-erased view of a static [`block::GeneratorMatrixAccess`].
#[derive(Clone)]
pub struct ErasedGeneratorMatrixAccess {
    code: ErasedBlockCode,
    inner: Arc<dyn ErasedGeneratorVtable>,
}

impl ErasedGeneratorMatrixAccess {
    /// Wraps a fully constructed static generator-matrix provider.
    pub fn new<C>(code: C) -> Self
    where
        C: block::GeneratorMatrixAccess + Send + Sync + 'static,
        C::Symbol: Send + Sync + 'static,
        C::Symbols: Send + Sync + 'static,
        C::GeneratorMatrix: Send + Sync + 'static,
    {
        let field_id = code.symbol_field_id();
        let inner = Arc::new(ErasedStaticCode { code, field_id });
        let metadata = ErasedBlockCode::from_vtable(inner.clone());
        let inner: Arc<dyn ErasedGeneratorVtable> = inner;
        Self {
            code: metadata,
            inner,
        }
    }

    /// Returns the metadata view for this generator provider.
    pub fn code(&self) -> &ErasedBlockCode {
        &self.code
    }

    /// Returns the process-local identity of the generator matrix type.
    pub fn matrix_representation(&self) -> RepresentationId {
        self.inner.matrix_representation()
    }

    /// Materializes and erases the canonical generator matrix.
    pub fn generator_matrix(&self) -> Result<ErasedMatrix, CodeError> {
        self.inner.generator_matrix_erased()
    }
}

/// The object-safe parity-check-matrix capability vtable.
trait ErasedParityVtable: ErasedCodeVtable {
    fn matrix_representation(&self) -> RepresentationId;
    fn parity_check_matrix_erased(&self) -> Result<ErasedMatrix, CodeError>;
}

impl<C> ErasedParityVtable for ErasedStaticCode<C>
where
    C: block::ParityCheckMatrixAccess + Send + Sync + 'static,
    C::Symbol: Send + Sync + 'static,
    C::Symbols: Send + Sync + 'static,
    C::ParityCheckMatrix: Send + Sync + 'static,
{
    fn matrix_representation(&self) -> RepresentationId {
        RepresentationId::of::<C::ParityCheckMatrix>()
    }

    fn parity_check_matrix_erased(&self) -> Result<ErasedMatrix, CodeError> {
        let matrix = block::ParityCheckMatrixAccess::parity_check_matrix(&self.code)?;
        Ok(ErasedMatrix::new(&self.code, matrix))
    }
}

/// A type-erased view of a static [`block::ParityCheckMatrixAccess`].
#[derive(Clone)]
pub struct ErasedParityCheckMatrixAccess {
    code: ErasedBlockCode,
    inner: Arc<dyn ErasedParityVtable>,
}

impl ErasedParityCheckMatrixAccess {
    /// Wraps a fully constructed static parity-check-matrix provider.
    pub fn new<C>(code: C) -> Self
    where
        C: block::ParityCheckMatrixAccess + Send + Sync + 'static,
        C::Symbol: Send + Sync + 'static,
        C::Symbols: Send + Sync + 'static,
        C::ParityCheckMatrix: Send + Sync + 'static,
    {
        let field_id = code.symbol_field_id();
        let inner = Arc::new(ErasedStaticCode { code, field_id });
        let metadata = ErasedBlockCode::from_vtable(inner.clone());
        let inner: Arc<dyn ErasedParityVtable> = inner;
        Self {
            code: metadata,
            inner,
        }
    }

    /// Returns the metadata view for this parity-check provider.
    pub fn code(&self) -> &ErasedBlockCode {
        &self.code
    }

    /// Returns the process-local identity of the parity-check matrix type.
    pub fn matrix_representation(&self) -> RepresentationId {
        self.inner.matrix_representation()
    }

    /// Materializes and erases the canonical parity-check matrix.
    pub fn parity_check_matrix(&self) -> Result<ErasedMatrix, CodeError> {
        self.inner.parity_check_matrix_erased()
    }
}

/// The versioned binary compatibility boundary.
pub mod compat {
    pub mod binary_v1 {
        //! The `binary-code-v1` compatibility boundary, version
        //! [`BINARY_CODE_COMPAT_VERSION`].
        //!
        //! The module holds the two bit-only contracts that code families
        //! outside the canonical [`crate::traits::block`] surface implement
        //! directly, plus the root shims that keep
        //! `crate::traits::BlockEncoder`,
        //! `crate::traits::GeneratorMatrixAccess`, and their trait-object uses
        //! naming those contracts. A code that implements the canonical traits
        //! over `Fp<2>`, [`BitVec`], and [`BitMatrix`] reaches this boundary
        //! through the blanket adapters instead, which are the only place a
        //! canonical [`CodeError`](crate::error::CodeError) becomes version-1
        //! panic behavior. A family therefore has either a direct version-1
        //! implementation or the adapter, never both.
        //!
        //! The boundary is closed: no further code family or public API enters
        //! it.
        //!
        //! # Removal condition
        //!
        //! Story `3931ac6f` owns removal after epic `ae03bcd0`. Its
        //! mechanical, auditable condition — every boundary family migrated,
        //! no remaining reference to this module or the root shims, and the
        //! shared binary suites passing through the canonical packed
        //! specialization — is recorded in that epic's BCH API design document
        //! (`bch-api-design.md`, section "Removal condition"), authored under
        //! JIT issue `7a3a6738`, which is its single source of truth.

        use gf2_core::{BitMatrix, BitVec};

        /// Version of the binary compatibility boundary.
        pub const BINARY_CODE_COMPAT_VERSION: u16 = 1;

        /// Version-1 bit-only block encoder contract.
        ///
        /// The encoder maps a `k`-bit message to an `n`-bit codeword and
        /// reports an invalid message length by panicking. The canonical
        /// replacement is [`crate::traits::block::BlockEncoder`], whose
        /// `encode` returns a typed error instead.
        ///
        /// # Examples
        ///
        /// ```
        /// use gf2_coding::traits::BlockEncoder;
        /// use gf2_coding::LinearBlockCode;
        /// use gf2_core::BitVec;
        ///
        /// let code = LinearBlockCode::hamming(3);
        /// let mut message = BitVec::zeros(code.k());
        /// message.set(0, true);
        /// assert_eq!(code.encode(&message).len(), code.n());
        /// ```
        pub trait BlockEncoder {
            /// Returns the message dimension.
            fn k(&self) -> usize;

            /// Returns the codeword length.
            fn n(&self) -> usize;

            /// Encodes a bit message, panicking on invalid input.
            ///
            /// # Panics
            ///
            /// Panics when `message.len() != k()` or canonical encoding
            /// reports another error.
            fn encode(&self, message: &BitVec) -> BitVec;
        }

        /// Version-1 bit-only generator-matrix contract.
        ///
        /// Materializing the `k × n` generator is intended for code analysis,
        /// validation, and teaching rather than for a hot encoding path,
        /// which uses [`BlockEncoder`] instead. The canonical replacement is
        /// [`crate::traits::block::GeneratorMatrixAccess`], whose methods
        /// return typed errors and admit a matrix representation other than
        /// [`BitMatrix`].
        ///
        /// # Examples
        ///
        /// ```
        /// use gf2_coding::traits::GeneratorMatrixAccess;
        /// use gf2_coding::LinearBlockCode;
        ///
        /// let code = LinearBlockCode::hamming(3);
        /// let g = code.generator_matrix();
        /// assert_eq!(g.rows(), code.k());
        /// assert_eq!(g.cols(), code.n());
        /// assert!(code.is_systematic());
        /// ```
        pub trait GeneratorMatrixAccess {
            /// Returns the message dimension.
            fn k(&self) -> usize;

            /// Returns the codeword length.
            fn n(&self) -> usize;

            /// Materializes the generator matrix, panicking on failure.
            ///
            /// # Panics
            ///
            /// Panics when canonical matrix access reports an error.
            fn generator_matrix(&self) -> BitMatrix;

            /// Reports whether the code exposes a systematic generator.
            ///
            /// The default implementation materializes the generator and
            /// tests whether its first `k` columns form the identity, which
            /// costs O(k²) bit reads on top of that materialization. An
            /// implementation that knows the answer overrides it.
            ///
            /// # Panics
            ///
            /// Panics when canonical systematicity access reports an error.
            fn is_systematic(&self) -> bool {
                let g = self.generator_matrix();
                if g.rows() != self.k() || g.cols() < self.k() {
                    return false;
                }

                for i in 0..self.k() {
                    for j in 0..self.k() {
                        if g.get(i, j) != (i == j) {
                            return false;
                        }
                    }
                }
                true
            }
        }
    }
}

/// Source-compatible binary-code-v1 shims.
pub use compat::binary_v1::{BlockEncoder, GeneratorMatrixAccess};

impl<T> compat::binary_v1::BlockEncoder for T
where
    T: block::BinaryBlockCode + block::BlockEncoder,
{
    fn k(&self) -> usize {
        block::BlockCode::k(self)
    }

    fn n(&self) -> usize {
        block::BlockCode::n(self)
    }

    fn encode(&self, message: &BitVec) -> BitVec {
        match block::BlockEncoder::encode(self, message) {
            Ok(codeword) => codeword,
            Err(crate::error::CodeError::BufferLengthMismatch { expected, actual })
                if expected == self.k() =>
            {
                panic!("Message length must be k = {} (got {})", expected, actual)
            }
            Err(error) => panic!("canonical binary encoding failed: {error}"),
        }
    }
}

impl<T> compat::binary_v1::GeneratorMatrixAccess for T
where
    T: block::BinaryGeneratorMatrixAccess,
{
    fn k(&self) -> usize {
        block::BlockCode::k(self)
    }

    fn n(&self) -> usize {
        block::BlockCode::n(self)
    }

    fn generator_matrix(&self) -> gf2_core::BitMatrix {
        block::GeneratorMatrixAccess::generator_matrix(self)
            .unwrap_or_else(|error| panic!("canonical generator access failed: {error}"))
    }

    fn is_systematic(&self) -> bool {
        block::GeneratorMatrixAccess::is_systematic(self)
            .unwrap_or_else(|error| panic!("canonical systematicity access failed: {error}"))
    }
}

/// Result of a soft-decision decoding operation.
///
/// Contains the decoded bits along with metadata about the decoding process,
/// particularly useful for iterative decoders like LDPC and turbo codes.
#[derive(Debug, Clone, PartialEq)]
pub struct DecoderResult {
    /// The decoded message bits
    pub decoded_bits: BitVec,

    /// Number of iterations performed (for iterative decoders)
    pub iterations: usize,

    /// Whether the decoder converged to a valid codeword
    pub converged: bool,

    /// Whether the syndrome check passed (for linear codes)
    pub syndrome_check_passed: bool,

    /// Number of parity-check queries performed during decoding.
    ///
    /// This is a finer-grained measure of decoder work than iterations.
    /// For belief-propagation decoders, this counts the total number of
    /// check-node or variable-node messages exchanged.
    /// When `None`, the simulation harness falls back to `iterations`
    /// for computing `avg_queries_per_bit`.
    pub queries: Option<usize>,
}

impl DecoderResult {
    /// Creates a new decoder result.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::traits::DecoderResult;
    /// use gf2_core::BitVec;
    ///
    /// let decoded = BitVec::from_bytes_le(&[0b1011]);
    /// let result = DecoderResult::new(decoded, 5, true, true);
    /// assert_eq!(result.iterations, 5);
    /// assert!(result.converged);
    /// ```
    pub fn new(
        decoded_bits: BitVec,
        iterations: usize,
        converged: bool,
        syndrome_check_passed: bool,
    ) -> Self {
        Self {
            decoded_bits,
            iterations,
            converged,
            syndrome_check_passed,
            queries: None,
        }
    }

    /// Creates a result for a successful single-shot decode (non-iterative).
    pub fn success(decoded_bits: BitVec) -> Self {
        Self {
            decoded_bits,
            iterations: 1,
            converged: true,
            syndrome_check_passed: true,
            queries: None,
        }
    }

    /// Creates a result for a failed decode.
    pub fn failure(decoded_bits: BitVec, iterations: usize) -> Self {
        Self {
            decoded_bits,
            iterations,
            converged: false,
            syndrome_check_passed: false,
            queries: None,
        }
    }
}

/// Hard-decision decoder for block codes.
///
/// A hard-decision decoder takes a received codeword (where each bit is a hard 0 or 1 decision)
/// and attempts to recover the original message bits, potentially correcting errors.
pub trait HardDecisionDecoder {
    /// Decodes a received codeword and returns the estimated message bits.
    ///
    /// # Arguments
    ///
    /// * `received` - The received bit vector (potentially with errors)
    ///
    /// # Returns
    ///
    /// A bit vector containing the decoded message bits
    ///
    /// # Panics
    ///
    /// Panics if the received vector has incorrect length
    fn decode(&self, received: &BitVec) -> BitVec;
}

/// Soft-decision decoder for block codes.
///
/// A soft-decision decoder uses log-likelihood ratios (LLRs) to make better
/// decoding decisions than hard-decision decoders. This trait supports both
/// single-shot and iterative decoding algorithms.
///
/// # LLR Convention
///
/// LLR values follow the convention:
/// - Positive LLR → bit is more likely 0
/// - Negative LLR → bit is more likely 1
/// - Magnitude represents confidence
pub trait SoftDecoder {
    /// Returns the number of message bits (dimension).
    fn k(&self) -> usize;

    /// Returns the number of codeword bits (length).
    fn n(&self) -> usize;

    /// Decodes using soft information (LLRs).
    ///
    /// This is the primary decoding method for soft-decision decoders.
    ///
    /// # Arguments
    ///
    /// * `llrs` - Log-likelihood ratios for each codeword bit position
    ///
    /// # Returns
    ///
    /// Decoded message bits
    ///
    /// # Panics
    ///
    /// Panics if `llrs.len() != n()`
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use gf2_coding::llr::Llr;
    /// use gf2_coding::traits::SoftDecoder;
    ///
    /// let llrs: Vec<Llr> = received_symbols.iter()
    ///     .map(|&s| Llr::from_bpsk_symbol(s, noise_variance))
    ///     .collect();
    /// let decoded = decoder.decode_soft(&llrs);
    /// ```
    fn decode_soft(&self, llrs: &[Llr]) -> BitVec;

    /// Decodes and returns detailed result information.
    ///
    /// Similar to `decode_soft` but returns additional metadata useful for
    /// analysis and debugging.
    ///
    /// # Arguments
    ///
    /// * `llrs` - Log-likelihood ratios for each codeword bit position
    ///
    /// # Returns
    ///
    /// A `DecoderResult` containing decoded bits and metadata
    fn decode_soft_with_result(&self, llrs: &[Llr]) -> DecoderResult {
        let decoded = self.decode_soft(llrs);
        DecoderResult::success(decoded)
    }
}

/// Iterative soft-decision decoder for LDPC and turbo codes.
///
/// Extends `SoftDecoder` with iteration control and early stopping criteria.
/// Iterative decoders repeatedly refine LLR estimates until convergence or
/// a maximum iteration count is reached.
///
/// # Typical Usage Pattern
///
/// ```ignore
/// let mut decoder = LdpcDecoder::new(code);
/// let result = decoder.decode_iterative(&channel_llrs, 50); // max 50 iterations
///
/// if result.converged {
///     println!("Converged in {} iterations", result.iterations);
/// } else {
///     println!("Failed to converge after {} iterations", result.iterations);
/// }
/// ```
pub trait IterativeSoftDecoder: SoftDecoder {
    /// Decodes with iteration control.
    ///
    /// Performs iterative belief propagation or similar algorithm until
    /// convergence or maximum iterations reached.
    ///
    /// # Arguments
    ///
    /// * `llrs` - Initial log-likelihood ratios from channel
    /// * `max_iterations` - Maximum number of iterations to perform
    ///
    /// # Returns
    ///
    /// A `DecoderResult` containing decoded bits and convergence information
    ///
    /// # Early Stopping
    ///
    /// The decoder should stop early if:
    /// - Syndrome check passes (for linear codes)
    /// - LLR updates fall below threshold (converged)
    /// - Maximum iterations reached
    fn decode_iterative(&mut self, llrs: &[Llr], max_iterations: usize) -> DecoderResult;

    /// Returns the number of iterations used in the last decode.
    ///
    /// Useful for tracking decoder performance without full `DecoderResult`.
    fn last_iteration_count(&self) -> usize;

    /// Resets internal decoder state.
    ///
    /// Should be called between decoding different codewords to ensure
    /// no state leaks between frames.
    fn reset(&mut self);
}

/// Streaming encoder for convolutional codes.
///
/// A streaming encoder processes bits one at a time, maintaining internal state
/// across multiple encode operations. This is used for convolutional codes.
pub trait StreamingEncoder {
    /// Encodes a single input bit and returns the output symbol(s).
    ///
    /// # Arguments
    ///
    /// * `input` - The input bit to encode
    ///
    /// # Returns
    ///
    /// A vector of output bits (the encoded symbols)
    fn encode_bit(&mut self, input: bool) -> Vec<bool>;

    /// Resets the encoder state to initial conditions.
    fn reset(&mut self);
}

/// Streaming decoder for convolutional codes.
///
/// A streaming decoder processes received symbols and maintains internal state
/// across multiple decode operations.
pub trait StreamingDecoder {
    /// Decodes received symbol(s) and potentially outputs decoded bit(s).
    ///
    /// # Arguments
    ///
    /// * `symbols` - The received symbols to decode
    ///
    /// # Returns
    ///
    /// Decoded bits (may be empty if more symbols are needed)
    fn decode_symbols(&mut self, symbols: &[bool]) -> Vec<bool>;

    /// Resets the decoder state to initial conditions.
    fn reset(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_result_new() {
        let bits = BitVec::from_bytes_le(&[0b1011]);
        let result = DecoderResult::new(bits.clone(), 10, true, true);

        assert_eq!(result.decoded_bits, bits);
        assert_eq!(result.iterations, 10);
        assert!(result.converged);
        assert!(result.syndrome_check_passed);
    }

    #[test]
    fn test_decoder_result_success() {
        let bits = BitVec::from_bytes_le(&[0b1011]);
        let result = DecoderResult::success(bits.clone());

        assert_eq!(result.decoded_bits, bits);
        assert_eq!(result.iterations, 1);
        assert!(result.converged);
        assert!(result.syndrome_check_passed);
    }

    #[test]
    fn test_decoder_result_failure() {
        let bits = BitVec::from_bytes_le(&[0b1011]);
        let result = DecoderResult::failure(bits.clone(), 50);

        assert_eq!(result.decoded_bits, bits);
        assert_eq!(result.iterations, 50);
        assert!(!result.converged);
        assert!(!result.syndrome_check_passed);
    }

    #[test]
    fn test_decoder_result_clone() {
        let bits = BitVec::from_bytes_le(&[0b1011]);
        let result1 = DecoderResult::new(bits.clone(), 5, true, false);
        let result2 = result1.clone();

        assert_eq!(result1, result2);
    }

    // Mock implementations for testing trait contracts

    struct MockSoftDecoder {
        k: usize,
        n: usize,
    }

    impl SoftDecoder for MockSoftDecoder {
        fn k(&self) -> usize {
            self.k
        }

        fn n(&self) -> usize {
            self.n
        }

        fn decode_soft(&self, llrs: &[Llr]) -> BitVec {
            assert_eq!(llrs.len(), self.n);
            // Simple hard decision for testing
            let mut result = BitVec::new();
            for &llr in llrs.iter().take(self.k) {
                result.push_bit(llr.hard_decision());
            }
            result
        }
    }

    struct MockIterativeDecoder {
        k: usize,
        n: usize,
        last_iterations: usize,
    }

    impl SoftDecoder for MockIterativeDecoder {
        fn k(&self) -> usize {
            self.k
        }

        fn n(&self) -> usize {
            self.n
        }

        fn decode_soft(&self, llrs: &[Llr]) -> BitVec {
            assert_eq!(llrs.len(), self.n);
            let mut result = BitVec::new();
            for &llr in llrs.iter().take(self.k) {
                result.push_bit(llr.hard_decision());
            }
            result
        }
    }

    impl IterativeSoftDecoder for MockIterativeDecoder {
        fn decode_iterative(&mut self, llrs: &[Llr], max_iterations: usize) -> DecoderResult {
            assert_eq!(llrs.len(), self.n);

            // Simulate convergence after 5 iterations
            let iterations = max_iterations.min(5);
            self.last_iterations = iterations;

            let decoded = self.decode_soft(llrs);
            let converged = iterations < max_iterations;

            DecoderResult::new(decoded, iterations, converged, converged)
        }

        fn last_iteration_count(&self) -> usize {
            self.last_iterations
        }

        fn reset(&mut self) {
            self.last_iterations = 0;
        }
    }

    #[test]
    fn test_soft_decoder_trait() {
        let decoder = MockSoftDecoder { k: 4, n: 7 };

        assert_eq!(decoder.k(), 4);
        assert_eq!(decoder.n(), 7);

        let llrs = vec![
            Llr::new(3.0),
            Llr::new(-2.0),
            Llr::new(1.0),
            Llr::new(-0.5),
            Llr::new(2.0),
            Llr::new(1.5),
            Llr::new(-1.0),
        ];

        let decoded = decoder.decode_soft(&llrs);
        assert_eq!(decoded.len(), 4);

        // Check hard decisions
        assert!(!decoded.get(0)); // 3.0 → 0
        assert!(decoded.get(1)); // -2.0 → 1
        assert!(!decoded.get(2)); // 1.0 → 0
        assert!(decoded.get(3)); // -0.5 → 1
    }

    #[test]
    fn test_soft_decoder_with_result() {
        let decoder = MockSoftDecoder { k: 4, n: 7 };

        let llrs = vec![
            Llr::new(3.0),
            Llr::new(-2.0),
            Llr::new(1.0),
            Llr::new(-0.5),
            Llr::new(2.0),
            Llr::new(1.5),
            Llr::new(-1.0),
        ];

        let result = decoder.decode_soft_with_result(&llrs);

        assert_eq!(result.decoded_bits.len(), 4);
        assert_eq!(result.iterations, 1);
        assert!(result.converged);
        assert!(result.syndrome_check_passed);
    }

    #[test]
    fn test_iterative_decoder_converges() {
        let mut decoder = MockIterativeDecoder {
            k: 4,
            n: 7,
            last_iterations: 0,
        };

        let llrs = vec![Llr::new(1.0); 7];

        let result = decoder.decode_iterative(&llrs, 50);

        assert_eq!(result.iterations, 5); // Converges at 5
        assert!(result.converged);
        assert_eq!(decoder.last_iteration_count(), 5);
    }

    #[test]
    fn test_iterative_decoder_max_iterations() {
        let mut decoder = MockIterativeDecoder {
            k: 4,
            n: 7,
            last_iterations: 0,
        };

        let llrs = vec![Llr::new(1.0); 7];

        let result = decoder.decode_iterative(&llrs, 3); // Less than convergence point

        assert_eq!(result.iterations, 3);
        assert!(!result.converged); // Didn't converge
        assert_eq!(decoder.last_iteration_count(), 3);
    }

    #[test]
    fn test_iterative_decoder_reset() {
        let mut decoder = MockIterativeDecoder {
            k: 4,
            n: 7,
            last_iterations: 0,
        };

        let llrs = vec![Llr::new(1.0); 7];
        decoder.decode_iterative(&llrs, 10);
        assert_eq!(decoder.last_iteration_count(), 5);

        decoder.reset();
        assert_eq!(decoder.last_iteration_count(), 0);
    }

    #[test]
    #[should_panic(expected = "left == right")]
    fn test_soft_decoder_wrong_length_panics() {
        let decoder = MockSoftDecoder { k: 4, n: 7 };
        let llrs = vec![Llr::new(1.0); 5]; // Wrong length
        decoder.decode_soft(&llrs);
    }
}

#[cfg(test)]
mod generator_matrix_tests {
    use super::*;
    use gf2_core::BitMatrix;

    // Mock implementation for testing trait contract
    struct MockLinearCode {
        k: usize,
        n: usize,
        g: BitMatrix,
    }

    impl GeneratorMatrixAccess for MockLinearCode {
        fn k(&self) -> usize {
            self.k
        }
        fn n(&self) -> usize {
            self.n
        }
        fn generator_matrix(&self) -> BitMatrix {
            self.g.clone()
        }
    }

    #[test]
    fn test_generator_matrix_dimensions() {
        let g = BitMatrix::zeros(4, 7);
        let code = MockLinearCode { k: 4, n: 7, g };
        let retrieved = code.generator_matrix();
        assert_eq!(retrieved.rows(), 4);
        assert_eq!(retrieved.cols(), 7);
    }

    #[test]
    fn test_is_systematic_identity() {
        let mut g = BitMatrix::zeros(3, 5);
        // Set identity in first 3 columns
        for i in 0..3 {
            g.set(i, i, true);
        }
        let code = MockLinearCode { k: 3, n: 5, g };
        assert!(code.is_systematic());
    }

    #[test]
    fn test_is_systematic_non_systematic() {
        let g = BitMatrix::zeros(3, 5);
        let code = MockLinearCode { k: 3, n: 5, g };
        assert!(!code.is_systematic());
    }

    #[test]
    fn test_is_systematic_partial_identity() {
        let mut g = BitMatrix::zeros(3, 5);
        // Set partial identity (missing one)
        g.set(0, 0, true);
        g.set(1, 1, true);
        // Missing g.set(2, 2, true);
        let code = MockLinearCode { k: 3, n: 5, g };
        assert!(!code.is_systematic());
    }
}

#[cfg(test)]
mod block_conformance_tests {
    use super::block;
    use super::block::conformance::{self, RepetitionCode};
    use super::block::BlockCode;
    use super::compat::binary_v1::BINARY_CODE_COMPAT_VERSION;
    use super::BlockEncoder as V1BlockEncoder;
    use crate::linear::LinearBlockCode;
    use crate::traits::GeneratorMatrixAccess as V1GeneratorMatrixAccess;
    use gf2_core::field::matrix::FieldMatrix;
    use gf2_core::field::FieldVec;
    use gf2_core::gfp::Fp;
    use gf2_core::{BitMatrix, BitVec};

    #[test]
    fn packed_bitvec_satisfies_symbol_sequence_contract() {
        conformance::symbol_sequence_contract::<Fp<2>, BitVec>(&Fp::<2>::new(0), &Fp::<2>::new(1));
    }

    #[test]
    fn packed_bitmatrix_satisfies_symbol_matrix_contract() {
        conformance::symbol_matrix_contract::<Fp<2>, BitMatrix>(&Fp::<2>::new(0), &Fp::<2>::new(1));
    }

    #[test]
    fn field_vec_satisfies_symbol_sequence_contract() {
        conformance::symbol_sequence_contract::<Fp<2>, FieldVec<Fp<2>>>(
            &Fp::<2>::new(0),
            &Fp::<2>::new(1),
        );
        conformance::symbol_sequence_contract::<Fp<7>, FieldVec<Fp<7>>>(
            &Fp::<7>::new(0),
            &Fp::<7>::new(1),
        );
    }

    #[test]
    fn field_matrix_satisfies_symbol_matrix_contract() {
        conformance::symbol_matrix_contract::<Fp<2>, FieldMatrix<Fp<2>>>(
            &Fp::<2>::new(0),
            &Fp::<2>::new(1),
        );
        conformance::symbol_matrix_contract::<Fp<7>, FieldMatrix<Fp<7>>>(
            &Fp::<7>::new(0),
            &Fp::<7>::new(1),
        );
    }

    #[test]
    fn repetition_code_satisfies_block_code_contracts() {
        fn check<F: gf2_core::field::extension::FieldIdentity + 'static>(zero: F) {
            let code = RepetitionCode::new(3, zero);
            let one = code.symbol_zero().one_like();
            let message = FieldVec::from(vec![one.clone()]);
            conformance::block_encoder_contract(&code, &message);
            conformance::generator_matrix_contract(&code);
            conformance::generator_rows_encode_basis(&code, &one);
            conformance::parity_check_matrix_contract(&code);
            conformance::generator_parity_orthogonality(&code);
        }

        check(Fp::<2>::new(0));
        check(Fp::<7>::new(0));
    }

    #[test]
    fn binary_code_compat_version_is_one() {
        assert_eq!(BINARY_CODE_COMPAT_VERSION, 1);
    }

    #[test]
    fn static_dispatch_selects_representation_by_monomorphization() {
        let packed = conformance::symbol_representation_of::<LinearBlockCode>();
        let field = conformance::symbol_representation_of::<RepetitionCode<Fp<2>>>();
        assert_eq!(packed, crate::error::RepresentationId::of::<BitVec>());
        assert_eq!(
            field,
            crate::error::RepresentationId::of::<FieldVec<Fp<2>>>()
        );
    }

    #[test]
    fn binary_v1_adapter_is_exercised_by_shared_contract() {
        let code = LinearBlockCode::hamming(3);
        let mut message = BitVec::zeros(code.k());
        for index in [0, 2, 3] {
            message.set(index, true);
        }
        conformance::binary_v1_encoder_agrees(&code, &message);
    }

    /// Version-1 consumers keep passing codes as trait objects, so the
    /// boundary's two traits stay object-safe.
    #[allow(dead_code)]
    fn _assert_v1_traits_are_object_safe(
        encoder: &dyn V1BlockEncoder,
        generator: &dyn V1GeneratorMatrixAccess,
    ) {
        let _ = (encoder.k(), encoder.n(), generator.k(), generator.n());
    }

    /// The canonical capabilities reach a caller through static bounds, with
    /// the representation fixed by monomorphization rather than by a vtable.
    #[allow(dead_code)]
    fn _assert_canonical_capabilities_are_statically_bound<C>(code: &C)
    where
        C: block::BinaryBlockCode
            + block::BlockEncoder
            + block::BinaryGeneratorMatrixAccess
            + block::BinaryParityCheckMatrixAccess,
    {
        let _ = (BlockCode::k(code), BlockCode::n(code));
    }

    #[allow(dead_code)]
    fn _linear_block_code_satisfies_the_static_bounds(code: &LinearBlockCode) {
        _assert_canonical_capabilities_are_statically_bound(code);
    }
}

#[cfg(test)]
mod erased_handle_tests {
    use super::block::conformance::RepetitionCode;
    use super::block::{self, BlockCode};
    use super::*;
    use crate::linear::LinearBlockCode;
    use gf2_core::field::matrix::FieldMatrix;
    use gf2_core::field::{FieldVec, FiniteField};
    use gf2_core::gfp::Fp;
    use gf2_core::{BitMatrix, BitVec};

    #[test]
    fn erased_binary_encoder_agrees_and_preserves_metadata() {
        let code = LinearBlockCode::hamming(3);
        let mut message = BitVec::zeros(code.k());
        for index in [0, 2, 3] {
            message.set(index, true);
        }
        let expected = block::BlockEncoder::encode(&code, &message).unwrap();
        let erased_code = ErasedBlockCode::new(code.clone());
        let encoder = ErasedBlockEncoder::new(code.clone());
        let encoded = encoder
            .encode(&ErasedSymbols::new(&code, message.clone()))
            .unwrap();

        assert_eq!(encoded.downcast_ref(&code).unwrap(), &expected);
        assert_eq!(erased_code.field_id(), &code.symbol_field_id());
        assert_eq!(
            erased_code.symbol_representation(),
            RepresentationId::of::<BitVec>()
        );
        assert_eq!(erased_code.k(), code.k());
        assert_eq!(erased_code.n(), code.n());
        assert_eq!(encoder.code().field_id(), erased_code.field_id());
        assert_eq!(encoder.code().k(), code.k());
        assert_eq!(encoder.code().n(), code.n());
        assert!(erased_code.downcast_ref::<LinearBlockCode>().is_some());
        assert!(erased_code
            .downcast_ref::<RepetitionCode<Fp<7>>>()
            .is_none());
    }

    #[test]
    fn erased_generic_encoder_agrees_for_field_vec() {
        let code = RepetitionCode::new(4, Fp::<7>::new(0));
        let message = FieldVec::from(vec![code.symbol_zero().one_like()]);
        let expected = block::BlockEncoder::encode(&code, &message).unwrap();
        let encoded = ErasedBlockEncoder::new(code.clone())
            .encode(&ErasedSymbols::new(&code, message.clone()))
            .unwrap();

        assert_eq!(encoded.downcast_ref(&code).unwrap(), &expected);
        assert_eq!(encoded.field_id(), &code.symbol_field_id());
        assert_eq!(
            encoded.representation(),
            RepresentationId::of::<FieldVec<Fp<7>>>()
        );
    }

    #[test]
    fn erased_matrix_capabilities_agree_and_preserve_representation() {
        let code = LinearBlockCode::hamming(3);
        let generator = block::GeneratorMatrixAccess::generator_matrix(&code).unwrap();
        let generator_handle = ErasedGeneratorMatrixAccess::new(code.clone());
        let generator_erased = generator_handle.generator_matrix().unwrap();
        assert_eq!(
            generator_erased
                .downcast_ref::<_, BitMatrix>(&code)
                .unwrap(),
            &generator
        );
        assert_eq!(
            generator_handle.matrix_representation(),
            RepresentationId::of::<BitMatrix>()
        );
        assert_eq!(generator_erased.field_id(), &code.symbol_field_id());
        assert_eq!(
            generator_erased.representation(),
            generator_handle.matrix_representation()
        );

        let parity = block::ParityCheckMatrixAccess::parity_check_matrix(&code).unwrap();
        let parity_handle = ErasedParityCheckMatrixAccess::new(code.clone());
        let parity_erased = parity_handle.parity_check_matrix().unwrap();
        assert_eq!(
            parity_erased.downcast_ref::<_, BitMatrix>(&code).unwrap(),
            &parity
        );
        assert_eq!(
            parity_handle.matrix_representation(),
            RepresentationId::of::<BitMatrix>()
        );
    }

    #[test]
    fn erased_value_downcasts_validate_field_and_representation() {
        let binary = LinearBlockCode::hamming(3);
        let wrong_field_code = RepetitionCode::new(1, Fp::<7>::new(0));
        let wrong_field = ErasedSymbols::new(
            &wrong_field_code,
            FieldVec::from(vec![wrong_field_code.symbol_zero()]),
        );
        let error = ErasedBlockEncoder::new(binary.clone())
            .encode(&wrong_field)
            .unwrap_err();
        assert_eq!(
            error,
            CodeError::FieldMismatch {
                expected: binary.symbol_field_id(),
                found: wrong_field_code.symbol_field_id(),
            }
        );

        let same_field_code = RepetitionCode::new(1, Fp::<2>::new(0));
        let wrong_representation = ErasedSymbols::new(
            &same_field_code,
            FieldVec::from(vec![same_field_code.symbol_zero()]),
        );
        let error = ErasedBlockEncoder::new(binary.clone())
            .encode(&wrong_representation)
            .unwrap_err();
        assert_eq!(
            error,
            CodeError::RepresentationMismatch {
                expected: RepresentationId::of::<BitVec>(),
                found: RepresentationId::of::<FieldVec<Fp<2>>>(),
            }
        );

        let wrong_matrix = ErasedMatrix::new(
            &binary,
            FieldMatrix::new(binary.k(), binary.n(), Fp::<2>::new(0)),
        );
        let error = wrong_matrix
            .downcast_ref::<_, BitMatrix>(&binary)
            .unwrap_err();
        assert_eq!(
            error,
            CodeError::RepresentationMismatch {
                expected: RepresentationId::of::<BitMatrix>(),
                found: RepresentationId::of::<FieldMatrix<Fp<2>>>(),
            }
        );
    }

    #[test]
    fn erased_value_owned_downcast_returns_shared_payload() {
        let code = LinearBlockCode::hamming(2);
        let message = BitVec::zeros(code.k());
        let erased = ErasedSymbols::new(&code, message.clone());
        let recovered = erased.downcast(&code).unwrap();
        assert_eq!(&*recovered, &message);
    }
}
