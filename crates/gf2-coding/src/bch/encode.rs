//! Systematic BCH encoding over any supported base field.
//!
//! # Coordinate convention
//!
//! A codeword is the coefficient vector of a multiple of the generator
//! polynomial $g$ modulo $x^n - 1$: internal coordinate $i$ carries the
//! coefficient of $x^i$, the convention
//! [`spec`](crate::bch::spec) constructs the code under. Systematic encoding
//! of a message polynomial $m$ of degree below $k$ takes
//! $p = -(x^{n-k} m \bmod g)$ and $c = x^{n-k} m + p$, so $g$ divides $c$, the
//! message occupies the internal coordinates $n-k$ to $n-1$, and the parity
//! occupies $0$ to $n-k-1$.
//!
//! # Systematic layout contract
//!
//! A [`SystematicLayout`] is the explicit bijection between internal
//! coordinates and the coordinates a caller sees. Every declared layout is
//! systematic in the same sense: it carries the user coordinates $0$ to $k-1$
//! onto the message degrees and the user coordinates $k$ to $n-1$ onto the
//! parity degrees, so a caller reads its message back from the first $k$ user
//! coordinates whichever layout it chose. Layouts differ in the direction
//! each block runs:
//!
//! | Layout | user coordinate $u$ carries | inverse |
//! |---|---|---|
//! | [`MessageParityAscending`](SystematicLayout::MessageParityAscending) | $x^{(u + n - k) \bmod n}$ | $u = (i + k) \bmod n$ |
//! | [`MessageParityDescending`](SystematicLayout::MessageParityDescending) | $x^{n - 1 - u}$ | $u = n - 1 - i$ |
//!
//! The ascending layout is the default. The descending layout is the
//! transmission order the DVB-T2 outer BCH code declares, where the first
//! transmitted symbol is the highest-degree coefficient.
//!
//! The mapping is arithmetic and is evaluated per coordinate as the codeword
//! is written, so selecting a layout costs no permutation pass and no second
//! buffer. [`SystematicPlan`] is the descriptor an encode call consumes: a
//! code's generator, dimensions, and symbol-field witness together with the
//! chosen layout.
//!
//! # Representations
//!
//! Encoding runs the same shift-register recurrence in two representations,
//! selected at compile time by the code's symbol storage: a packed binary
//! path over `u64` words for [`BitVec`], and the field-generic path over base
//! field elements for [`FieldVec`]. The packed path never materializes
//! `FieldVec<Fp<2>>`. [`SystematicKernel`] is the contract those two paths
//! implement.
//!
//! # Complexity
//!
//! Let $r = n - k$. Encoding one message costs $O(k r)$ base-field
//! multiply-adds in the field-generic path and $O(k \lceil r/64 \rceil)$ word
//! operations plus $O(n)$ bit writes in the packed binary path. Each call
//! allocates one $r$-symbol shift register and one copy of the generator's
//! low $r$ coefficients; the codeword buffer is the caller's in
//! [`encode_systematic_into`](BchCode::encode_systematic_into).
//!
//! # Examples
//!
//! A binary primitive narrow-sense code. The default layout leaves the
//! message in the first $k$ coordinates.
//!
//! ```
//! use gf2_coding::bch::encode::SystematicLayout;
//! use gf2_coding::bch::spec::{BchSpec, BinaryBchCode, DesignedDistance};
//! use gf2_coding::traits::block::{BlockCode, BlockEncoder};
//! use gf2_core::field::extension::BinaryPrimeExt;
//! use gf2_core::gf2m::Gf2mField;
//! use gf2_core::BitVec;
//!
//! let extension = BinaryPrimeExt::new(Gf2mField::new(4, 0b10011))?;
//! let code = BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
//!     extension,
//!     designed_distance: DesignedDistance::try_from(5)?,
//! })?;
//!
//! let mut message = BitVec::zeros(code.k());
//! message.set(0, true);
//! message.set(3, true);
//! let codeword = code.encode(&message)?;
//!
//! assert_eq!(codeword.len(), code.n());
//! for coordinate in 0..code.k() {
//!     assert_eq!(codeword.get(coordinate), message.get(coordinate));
//! }
//! assert_eq!(
//!     code.systematic_message(&codeword, SystematicLayout::default())?,
//!     message
//! );
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! The same surface over $\mathrm{GF}(5)$, with the declared DVB-T2 layout
//! selected explicitly.
//!
//! ```
//! use gf2_coding::bch::encode::SystematicLayout;
//! use gf2_coding::bch::spec::{BchSpec, DenseBchCode, DesignedDistance};
//! use gf2_coding::traits::block::BlockCode;
//! use gf2_core::field::{ConstField, FieldPoly, FieldVec};
//! use gf2_core::gfp::Fp;
//! use gf2_core::gfpn::QuotientField;
//!
//! // GF(25) = GF(5)[x] / (x^2 + x + 1) splits the length-24 code over GF(5).
//! let modulus = FieldPoly::new(vec![Fp::<5>::new(1), Fp::new(1), Fp::new(1)]);
//! let extension = QuotientField::new(Fp::<5>::zero(), modulus)?;
//! let code = DenseBchCode::construct(BchSpec::PrimitiveNarrowSense {
//!     extension,
//!     designed_distance: DesignedDistance::try_from(5)?,
//! })?;
//!
//! let mut message = FieldVec::zeros_from(code.k(), &Fp::<5>::new(0));
//! message.set(0, Fp::new(3));
//! message.set(7, Fp::new(4));
//! let codeword =
//!     code.encode_systematic(&message, SystematicLayout::MessageParityDescending)?;
//!
//! assert_eq!(codeword.len(), code.n());
//! assert_eq!(*codeword.get(0), Fp::<5>::new(3));
//! assert_eq!(
//!     code.systematic_message(&codeword, SystematicLayout::MessageParityDescending)?,
//!     message
//! );
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use gf2_core::field::extension::{FieldExtension, FieldIdentity};
use gf2_core::field::{FieldPoly, FieldVec, FiniteField};
use gf2_core::gfp::Fp;
use gf2_core::BitVec;

use crate::bch::spec::BchCode;
use crate::error::CodeError;
use crate::traits::block::{BlockCode, BlockEncoder, SymbolMatrix, SymbolSequence};
use crate::transform::CoordinateMap;

// ---------------------------------------------------------------------------
// The layout contract
// ---------------------------------------------------------------------------

/// The coordinate layout a systematic encoding presents to a caller.
///
/// A layout is a bijection from user coordinates onto the internal
/// coordinates described at the [module level](self#coordinate-convention),
/// where coordinate $i$ is the coefficient of $x^i$. Every variant satisfies
/// the same systematic contract: user coordinates $0$ to $k-1$ carry the
/// message degrees $n-k$ to $n-1$, and user coordinates $k$ to $n-1$ carry
/// the parity degrees $0$ to $n-k-1$. The message is therefore readable from
/// the first $k$ user coordinates under every layout, and the layout decides
/// which degree each of those coordinates carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SystematicLayout {
    /// `[message | parity]` with both blocks in ascending degree order.
    ///
    /// User coordinate $u$ carries internal coordinate $(u + n - k) \bmod n$,
    /// a cyclic rotation by the redundancy. This is the default layout.
    #[default]
    MessageParityAscending,

    /// `[message | parity]` with both blocks in descending degree order.
    ///
    /// User coordinate $u$ carries internal coordinate $n - 1 - u$, so the
    /// first user coordinate holds the highest-degree coefficient. This is
    /// the transmission order the DVB-T2 outer BCH code declares.
    MessageParityDescending,
}

/// One code, one layout: the descriptor a systematic encode call consumes.
///
/// A plan borrows a constructed code's generator and copies its dimensions
/// and symbol-field witness, so its parameters are consistent by
/// construction: $k \le n$ and $\deg g = n - k$. Build one with
/// [`BchCode::systematic_plan`]. Evaluating the layout mapping through a plan
/// is arithmetic on `usize`, which is what makes layout selection free at the
/// call site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystematicPlan<'a, F: FiniteField> {
    generator: &'a FieldPoly<F>,
    zero: F,
    length: usize,
    dimension: usize,
    layout: SystematicLayout,
}

impl<'a, F: FiniteField> SystematicPlan<'a, F> {
    /// Returns the codeword length $n$.
    pub fn length(&self) -> usize {
        self.length
    }

    /// Returns the message dimension $k$.
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// Returns the redundancy $r = n - k$, the degree of the generator.
    pub fn redundancy(&self) -> usize {
        self.length - self.dimension
    }

    /// Returns the layout this plan encodes under.
    pub fn layout(&self) -> SystematicLayout {
        self.layout
    }

    /// Returns the monic generator polynomial over the base field.
    pub fn generator(&self) -> &'a FieldPoly<F> {
        self.generator
    }

    /// Returns the zero witness of the code-symbol field.
    pub fn symbol_zero(&self) -> &F {
        &self.zero
    }

    /// Returns the internal coordinate presented at `user`.
    ///
    /// The result is the exponent $i$ of the monomial $x^i$ whose coefficient
    /// user coordinate `user` carries.
    ///
    /// # Errors
    ///
    /// Returns [`CodeError::CoordinateOutOfRange`] when `user` is not below
    /// the codeword length.
    pub fn internal_coordinate(&self, user: usize) -> Result<usize, CodeError> {
        if user >= self.length {
            return Err(CodeError::CoordinateOutOfRange {
                coordinate: user,
                length: self.length,
            });
        }
        Ok(self.internal_at(user))
    }

    /// Returns the user coordinate that presents `internal`.
    ///
    /// This is the inverse of [`internal_coordinate`](Self::internal_coordinate).
    ///
    /// # Errors
    ///
    /// Returns [`CodeError::CoordinateOutOfRange`] when `internal` is not
    /// below the codeword length.
    pub fn user_coordinate(&self, internal: usize) -> Result<usize, CodeError> {
        if internal >= self.length {
            return Err(CodeError::CoordinateOutOfRange {
                coordinate: internal,
                length: self.length,
            });
        }
        Ok(self.user_at(internal))
    }

    /// Decides whether a message and codeword buffer have the lengths this
    /// plan encodes between.
    ///
    /// # Errors
    ///
    /// Returns [`CodeError::BufferLengthMismatch`] naming the dimension when
    /// `message` is not $k$, and naming the length when `codeword` is not
    /// $n$; the message is reported first.
    pub fn validate_lengths(&self, message: usize, codeword: usize) -> Result<(), CodeError> {
        if message != self.dimension {
            return Err(CodeError::BufferLengthMismatch {
                expected: self.dimension,
                actual: message,
            });
        }
        if codeword != self.length {
            return Err(CodeError::BufferLengthMismatch {
                expected: self.length,
                actual: codeword,
            });
        }
        Ok(())
    }

    /// Materializes the layout as a [`CoordinateMap`] from user coordinates
    /// to internal coordinates.
    ///
    /// The arithmetic accessors above are the form an encoding path uses; this
    /// materialization exists for consumers that compose the layout with the
    /// coordinate provenance of a derived code, and costs $O(n)$ time and
    /// memory.
    ///
    /// # Errors
    ///
    /// Propagates the [`CodeError`] [`CoordinateMap::from_permutation`]
    /// reports for a map that is not injective. A declared layout is a
    /// bijection, so a conforming plan does not produce one.
    pub fn to_coordinate_map(&self) -> Result<CoordinateMap, CodeError> {
        let coordinates: Vec<usize> = (0..self.length)
            .map(|user| self.internal_at(user))
            .collect();
        CoordinateMap::from_permutation(self.length, coordinates)
    }

    /// The layout mapping, for a `user` already known to be in range.
    fn internal_at(&self, user: usize) -> usize {
        match self.layout {
            SystematicLayout::MessageParityAscending => {
                let shifted = user + self.redundancy();
                if shifted >= self.length {
                    shifted - self.length
                } else {
                    shifted
                }
            }
            SystematicLayout::MessageParityDescending => self.length - 1 - user,
        }
    }

    /// The inverse layout mapping, for an `internal` already known to be in
    /// range.
    fn user_at(&self, internal: usize) -> usize {
        match self.layout {
            SystematicLayout::MessageParityAscending => {
                let shifted = internal + self.dimension;
                if shifted >= self.length {
                    shifted - self.length
                } else {
                    shifted
                }
            }
            SystematicLayout::MessageParityDescending => self.length - 1 - internal,
        }
    }

    /// The user coordinate carrying the message coefficient of $x^{r+degree}$.
    fn message_at(&self, degree: usize) -> usize {
        self.user_at(self.redundancy() + degree)
    }

    /// The generator's low $r$ coefficients, in ascending degree order.
    ///
    /// The reduction $x^r \equiv -(g_{r-1}x^{r-1} + \cdots + g_0)$ uses
    /// exactly these, so the leading coefficient of the monic generator never
    /// enters the recurrence.
    fn low_coefficients(&self) -> Vec<F> {
        (0..self.redundancy())
            .map(|degree| self.generator.coeff_or_zero(degree, &self.zero))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// The representation-specific kernels
// ---------------------------------------------------------------------------

/// The systematic encoding kernel of one symbol representation.
///
/// The trait exists because a systematic encoder is one recurrence with two
/// storage strategies: packed `u64` words for a binary code and base-field
/// elements for a code over any other field. Implementing it for a
/// representation is what makes [`BchCode::encode_systematic`] and the
/// canonical [`BlockEncoder`] available for codes stored that way. This crate
/// implements it for [`FieldVec`] over every base field and for [`BitVec`]
/// over `GF(2)`.
///
/// An implementation writes every codeword coordinate, so a buffer holding a
/// previous result needs no clearing, and it produces the same codeword as
/// the reference recurrence stated at the [module level](self).
pub trait SystematicKernel<F: FieldIdentity>: SymbolSequence<F> {
    /// Writes the systematic codeword of `message` into `codeword`.
    ///
    /// # Errors
    ///
    /// Returns the [`CodeError`] reported by
    /// [`SystematicPlan::validate_lengths`] when `message` does not hold
    /// `plan.dimension()` symbols or `codeword` does not hold
    /// `plan.length()`; no output symbol is written in that case.
    fn encode_systematic_into(
        plan: &SystematicPlan<'_, F>,
        message: &Self,
        codeword: &mut Self,
    ) -> Result<(), CodeError>;
}

impl<F: FieldIdentity + 'static> SystematicKernel<F> for FieldVec<F> {
    /// Runs the shift-register recurrence over base-field elements.
    ///
    /// # Errors
    ///
    /// Returns [`CodeError::BufferLengthMismatch`] for a message or codeword
    /// buffer of the wrong length.
    ///
    /// # Complexity
    ///
    /// $O(k r)$ field multiply-adds and one $r$-element register.
    fn encode_systematic_into(
        plan: &SystematicPlan<'_, F>,
        message: &Self,
        codeword: &mut Self,
    ) -> Result<(), CodeError> {
        plan.validate_lengths(message.len(), codeword.len())?;

        let redundancy = plan.redundancy();
        let low = plan.low_coefficients();
        let mut register = vec![plan.symbol_zero().clone(); redundancy];

        // Reduce x^r m(x) modulo g one message degree at a time, highest
        // first: the feedback symbol is the register's top coefficient plus
        // the message coefficient entering it.
        if redundancy > 0 {
            for degree in (0..plan.dimension()).rev() {
                let symbol = &message.as_slice()[plan.message_at(degree)];
                let feedback = register[redundancy - 1].clone() + symbol;
                for index in (1..redundancy).rev() {
                    register[index] = register[index - 1].clone() - feedback.clone() * &low[index];
                }
                register[0] = -(feedback * &low[0]);
            }
        }

        for (user, symbol) in message.as_slice().iter().enumerate() {
            codeword.set(user, symbol.clone());
        }
        for user in plan.dimension()..plan.length() {
            let parity = plan.internal_at(user);
            debug_assert!(
                parity < redundancy,
                "a systematic layout carries the coordinates above k onto the parity degrees"
            );
            codeword.set(user, -register[parity].clone());
        }
        Ok(())
    }
}

impl SystematicKernel<Fp<2>> for BitVec {
    /// Runs the shift-register recurrence over packed `u64` words.
    ///
    /// Negation is the identity over `GF(2)`, so the register holds the
    /// parity itself rather than its negative.
    ///
    /// # Errors
    ///
    /// Returns [`CodeError::BufferLengthMismatch`] for a message or codeword
    /// buffer of the wrong length.
    ///
    /// # Complexity
    ///
    /// $O(k \lceil r/64 \rceil)$ word operations and $O(n)$ bit writes.
    fn encode_systematic_into(
        plan: &SystematicPlan<'_, Fp<2>>,
        message: &Self,
        codeword: &mut Self,
    ) -> Result<(), CodeError> {
        plan.validate_lengths(message.len(), codeword.len())?;

        let redundancy = plan.redundancy();
        let words = redundancy.div_ceil(64);
        let mut register = vec![0u64; words];

        if redundancy > 0 {
            let mut low = vec![0u64; words];
            for (degree, coefficient) in plan.low_coefficients().iter().enumerate() {
                if coefficient.is_one() {
                    low[degree / 64] |= 1u64 << (degree % 64);
                }
            }
            // The shift moves the top coefficient out of the register, and
            // masking keeps the words above degree r - 1 clear so the next
            // feedback bit reads the top coefficient alone.
            let top = redundancy - 1;
            let tail = if redundancy.is_multiple_of(64) {
                u64::MAX
            } else {
                (1u64 << (redundancy % 64)) - 1
            };

            for degree in (0..plan.dimension()).rev() {
                let symbol = message.get(plan.message_at(degree));
                let feedback = ((register[top / 64] >> (top % 64)) & 1 == 1) != symbol;
                for word in (1..words).rev() {
                    register[word] = (register[word] << 1) | (register[word - 1] >> 63);
                }
                register[0] <<= 1;
                register[words - 1] &= tail;
                if feedback {
                    for (accumulator, coefficients) in register.iter_mut().zip(low.iter()) {
                        *accumulator ^= *coefficients;
                    }
                }
            }
        }

        for user in 0..plan.dimension() {
            codeword.set(user, message.get(user));
        }
        for user in plan.dimension()..plan.length() {
            let parity = plan.internal_at(user);
            debug_assert!(
                parity < redundancy,
                "a systematic layout carries the coordinates above k onto the parity degrees"
            );
            codeword.set(user, (register[parity / 64] >> (parity % 64)) & 1 == 1);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// The encoding surface of a constructed code
// ---------------------------------------------------------------------------

impl<X, S, M> BchCode<X, S, M>
where
    X: FieldExtension,
    S: SystematicKernel<X::Base>,
    M: SymbolMatrix<X::Base>,
{
    /// Returns the encoding descriptor for this code under `layout`.
    ///
    /// The plan borrows the code's generator, so it cannot outlive the code
    /// and cannot disagree with it.
    pub fn systematic_plan(&self, layout: SystematicLayout) -> SystematicPlan<'_, X::Base> {
        SystematicPlan {
            generator: self.generator(),
            zero: BlockCode::symbol_zero(self),
            length: self.n(),
            dimension: self.k(),
            layout,
        }
    }

    /// Encodes `message` under `layout` into the caller's buffer.
    ///
    /// The codeword satisfies $g \mid c$ and carries `message` in the
    /// systematic coordinates the layout declares. Every coordinate of
    /// `codeword` is written.
    ///
    /// # Errors
    ///
    /// - [`CodeError::BufferLengthMismatch`] when `message` does not hold
    ///   $k$ symbols or `codeword` does not hold $n$.
    /// - [`CodeError::FieldMismatch`] when the message symbols carry a
    ///   runtime field identity other than the code's. The check reads the
    ///   first symbol, which is the whole sequence's identity for a
    ///   well-formed message, and is skipped entirely for a symbol type whose
    ///   identity follows from the type.
    ///
    /// # Complexity
    ///
    /// See the [module-level summary](self#complexity).
    pub fn encode_systematic_into(
        &self,
        message: &S,
        layout: SystematicLayout,
        codeword: &mut S,
    ) -> Result<(), CodeError> {
        let plan = self.systematic_plan(layout);
        // Lengths first, so a message that is both wrong-length and foreign
        // reports the length; the kernel decides them again because it is a
        // public entry point of its own.
        plan.validate_lengths(message.len(), codeword.len())?;
        validate_symbol_field(&plan, message)?;
        S::encode_systematic_into(&plan, message, codeword)
    }

    /// Encodes `message` under `layout` into a new symbol sequence.
    ///
    /// # Errors
    ///
    /// Propagates the [`CodeError`] returned by
    /// [`encode_systematic_into`](Self::encode_systematic_into).
    pub fn encode_systematic(&self, message: &S, layout: SystematicLayout) -> Result<S, CodeError> {
        let mut codeword = S::zeroed(self.n(), &BlockCode::symbol_zero(self));
        self.encode_systematic_into(message, layout, &mut codeword)?;
        Ok(codeword)
    }

    /// Reads the message back out of a codeword written under `layout`.
    ///
    /// The systematic coordinates of every declared layout are the first $k$,
    /// so this is a copy of that block; `layout` names the convention under
    /// which those symbols are the message, which is the round trip
    /// [`encode_systematic`](Self::encode_systematic) satisfies.
    ///
    /// The method reads a codeword's systematic coordinates and does not
    /// decide whether the argument is a codeword.
    ///
    /// # Errors
    ///
    /// Returns [`CodeError::BufferLengthMismatch`] when `codeword` does not
    /// hold $n$ symbols.
    ///
    /// # Complexity
    ///
    /// $O(k)$ symbol reads.
    pub fn systematic_message(
        &self,
        codeword: &S,
        layout: SystematicLayout,
    ) -> Result<S, CodeError> {
        let plan = self.systematic_plan(layout);
        if codeword.len() != plan.length() {
            return Err(CodeError::BufferLengthMismatch {
                expected: plan.length(),
                actual: codeword.len(),
            });
        }

        let mut message = S::zeroed(plan.dimension(), plan.symbol_zero());
        for user in 0..plan.dimension() {
            let symbol = codeword
                .get(user)
                .expect("a systematic coordinate of a validated codeword");
            message.set(user, symbol)?;
        }
        Ok(message)
    }
}

/// Decides that a message's symbols come from the code's own field.
///
/// A symbol type whose identity follows from the type alone cannot carry
/// another field, so the check costs nothing there. A runtime carrier reports
/// the identity of its first symbol, which is the identity of a well-formed
/// sequence; a sequence mixing two presentations of one field size is caller
/// error with unspecified results.
fn validate_symbol_field<F, S>(plan: &SystematicPlan<'_, F>, message: &S) -> Result<(), CodeError>
where
    F: FieldIdentity,
    S: SymbolSequence<F>,
{
    if F::field_id_hint().is_some() {
        return Ok(());
    }
    let Some(symbol) = message.get(0) else {
        return Ok(());
    };
    let expected = plan.symbol_zero().field_id();
    let found = symbol.field_id();
    if expected == found {
        Ok(())
    } else {
        Err(CodeError::FieldMismatch { expected, found })
    }
}

impl<X, S, M> BlockEncoder for BchCode<X, S, M>
where
    X: FieldExtension,
    S: SystematicKernel<X::Base>,
    M: SymbolMatrix<X::Base>,
{
    /// Encodes under the default layout, `[message | parity]` in ascending
    /// degree order.
    ///
    /// # Errors
    ///
    /// Propagates the [`CodeError`] returned by
    /// [`BchCode::encode_systematic_into`].
    fn encode_into(&self, message: &S, codeword: &mut S) -> Result<(), CodeError> {
        self.encode_systematic_into(message, SystematicLayout::default(), codeword)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bch::spec::{BchSpec, BinaryBchCode, DenseBchCode, DesignedDistance, RootExponent};
    use crate::bch::{BchCode as LegacyBchCode, BchEncoder as LegacyBchEncoder};
    use crate::traits::block::conformance;
    use crate::traits::compat::binary_v1::BlockEncoder as V1BlockEncoder;
    use gf2_core::field::extension::BinaryPrimeExt;
    use gf2_core::field::modulus_select::select_modulus;
    use gf2_core::field::ConstField;
    use gf2_core::gf2m::Gf2mField;
    use gf2_core::gfp::Fp;
    use gf2_core::gfpn::{QuotientElement, QuotientField};
    use proptest::prelude::*;

    /// The binary parameter points the construction suite pins, as
    /// `(m, primitive polynomial, n, k, t)`.
    const LEGACY_BINARY_POINTS: &[(usize, u64, usize, usize, usize)] = &[
        (3, 0b1011, 7, 4, 1),
        (4, 0b10011, 15, 11, 1),
        (4, 0b10011, 15, 7, 2),
        (5, 0b100101, 31, 26, 1),
        (6, 0b1000011, 63, 57, 1),
        (7, 0b10000011, 127, 64, 10),
    ];

    const LAYOUTS: &[SystematicLayout] = &[
        SystematicLayout::MessageParityAscending,
        SystematicLayout::MessageParityDescending,
    ];

    fn binary_narrow_sense(m: usize, modulus: u64, designed_distance: u64) -> BinaryBchCode {
        let extension =
            BinaryPrimeExt::new(Gf2mField::new(m, modulus)).expect("a primitive modulus");
        BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
            extension,
            designed_distance: DesignedDistance::try_from(designed_distance)
                .expect("a positive designed distance"),
        })
        .expect("a valid primitive narrow-sense spec")
    }

    /// The same binary code in the field-generic representation.
    fn dense_binary_narrow_sense(
        m: usize,
        modulus: u64,
        designed_distance: u64,
    ) -> DenseBchCode<BinaryPrimeExt> {
        let extension =
            BinaryPrimeExt::new(Gf2mField::new(m, modulus)).expect("a primitive modulus");
        DenseBchCode::construct(BchSpec::PrimitiveNarrowSense {
            extension,
            designed_distance: DesignedDistance::try_from(designed_distance)
                .expect("a positive designed distance"),
        })
        .expect("a valid primitive narrow-sense spec")
    }

    /// A binary primitive code whose consecutive roots start at `first_root`,
    /// the flavor that reaches redundancies the narrow-sense one skips.
    fn binary_first_root(
        m: usize,
        modulus: u64,
        first_root: u64,
        designed_distance: u64,
    ) -> BinaryBchCode {
        let extension =
            BinaryPrimeExt::new(Gf2mField::new(m, modulus)).expect("a primitive modulus");
        BinaryBchCode::construct(BchSpec::PrimitiveFirstRoot {
            extension,
            first_root: RootExponent::from(first_root),
            designed_distance: DesignedDistance::try_from(designed_distance)
                .expect("a positive designed distance"),
        })
        .expect("a valid primitive first-root spec")
    }

    /// GF(25) = GF(5)[x] / (x^2 + x + 1), splitting the length-24 GF(5) code.
    fn gf5_code(designed_distance: u64) -> DenseBchCode<QuotientField<Fp<5>>> {
        let modulus = FieldPoly::new(vec![Fp::<5>::new(1), Fp::new(1), Fp::new(1)]);
        let extension =
            QuotientField::new(Fp::<5>::zero(), modulus).expect("an irreducible modulus");
        DenseBchCode::construct(BchSpec::PrimitiveNarrowSense {
            extension,
            designed_distance: DesignedDistance::try_from(designed_distance).expect("positive"),
        })
        .expect("a valid GF(5) primitive spec")
    }

    /// GF(9) = GF(3)[x] / (selected modulus), the base field of the
    /// quotient-base code.
    fn gf9() -> QuotientField<Fp<3>> {
        let modulus = select_modulus(&Fp::<3>::zero(), 2).expect("a GF(9) modulus");
        QuotientField::new(Fp::<3>::zero(), modulus).expect("GF(9)")
    }

    /// GF(81) presented as a degree-two extension of GF(9), splitting the
    /// length-80 code over GF(9).
    fn gf9_base_code(
        designed_distance: u64,
    ) -> DenseBchCode<QuotientField<QuotientElement<Fp<3>>>> {
        let gf9_zero = gf9().ext_zero();
        let modulus = select_modulus(&gf9_zero, 2).expect("a relative GF(81) modulus");
        let extension = QuotientField::new(gf9_zero, modulus).expect("GF(81) over GF(9)");
        DenseBchCode::construct(BchSpec::PrimitiveNarrowSense {
            extension,
            designed_distance: DesignedDistance::try_from(designed_distance).expect("positive"),
        })
        .expect("a valid GF(9) primitive spec")
    }

    /// A deterministic bit pattern of `len` bits.
    fn seeded_bits(len: usize, seed: u64) -> BitVec {
        BitVec::random_seeded(len, seed)
    }

    /// A deterministic message over an explicit alphabet.
    fn seeded_symbols<F: FiniteField>(alphabet: &[F], len: usize, seed: u64) -> FieldVec<F> {
        let mut state = seed | 1;
        let mut message = FieldVec::with_capacity(len);
        for _ in 0..len {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            message.push(alphabet[(state >> 33) as usize % alphabet.len()].clone());
        }
        message
    }

    /// Rebuilds the internal coefficient vector a user codeword presents.
    fn internal_polynomial<X, S, M>(
        code: &BchCode<X, S, M>,
        codeword: &S,
        layout: SystematicLayout,
    ) -> FieldPoly<X::Base>
    where
        X: FieldExtension,
        S: SystematicKernel<X::Base>,
        M: SymbolMatrix<X::Base>,
    {
        let plan = code.systematic_plan(layout);
        let mut coefficients = vec![BlockCode::symbol_zero(code); code.n()];
        for user in 0..code.n() {
            let internal = plan.internal_coordinate(user).expect("a user coordinate");
            coefficients[internal] = codeword.get(user).expect("a codeword coordinate");
        }
        FieldPoly::new(coefficients)
    }

    /// Asserts the two properties REQ-01 fixes: the generator divides the
    /// codeword polynomial, and the message survives in the systematic
    /// coordinates of the layout.
    fn assert_encodes_a_codeword<X, S, M>(
        code: &BchCode<X, S, M>,
        message: &S,
        layout: SystematicLayout,
    ) where
        X: FieldExtension,
        S: SystematicKernel<X::Base>,
        M: SymbolMatrix<X::Base>,
    {
        let codeword = code
            .encode_systematic(message, layout)
            .expect("a k-symbol message encodes");
        assert_eq!(codeword.len(), code.n(), "a codeword has n coordinates");

        let polynomial = internal_polynomial(code, &codeword, layout);
        let (_, remainder) = polynomial.div_rem(code.generator());
        assert!(remainder.is_zero(), "g must divide the codeword polynomial");

        let recovered = code
            .systematic_message(&codeword, layout)
            .expect("a codeword carries its message");
        assert_eq!(&recovered, message, "the message survives encoding");
    }

    // -- REQ-01: valid codewords over each base field ----------------------

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(16))]

        #[test]
        fn prop_binary_encoding_produces_codewords(
            point in 0usize..LEGACY_BINARY_POINTS.len(),
            seed: u64,
        ) {
            let (m, modulus, _, _, t) = LEGACY_BINARY_POINTS[point];
            let code = binary_narrow_sense(m, modulus, 2 * t as u64 + 1);
            let message = seeded_bits(code.k(), seed);
            for &layout in LAYOUTS {
                assert_encodes_a_codeword(&code, &message, layout);
            }
        }

        #[test]
        fn prop_prime_base_encoding_produces_codewords(
            designed_distance in 1u64..=6,
            seed: u64,
        ) {
            let code = gf5_code(designed_distance);
            let alphabet: Vec<Fp<5>> = (0..5).map(Fp::<5>::new).collect();
            let message = seeded_symbols(&alphabet, code.k(), seed);
            for &layout in LAYOUTS {
                assert_encodes_a_codeword(&code, &message, layout);
            }
        }

        #[test]
        fn prop_extension_base_encoding_produces_codewords(seed: u64) {
            let code = gf9_base_code(4);
            let alphabet = gf9().elements().expect("GF(9) is enumerable");
            let message = seeded_symbols(&alphabet, code.k(), seed);
            for &layout in LAYOUTS {
                assert_encodes_a_codeword(&code, &message, layout);
            }
        }
    }

    // -- REQ-03: agreement with the current binary encoder -----------------

    #[test]
    fn binary_encoding_agrees_with_the_legacy_encoder() {
        for &(m, modulus, n, k, t) in LEGACY_BINARY_POINTS {
            let code = binary_narrow_sense(m, modulus, 2 * t as u64 + 1);
            assert_eq!((code.n(), code.k()), (n, k));

            let legacy =
                LegacyBchEncoder::new(LegacyBchCode::new(n, k, t, Gf2mField::new(m, modulus)));
            for seed in 0..4u64 {
                let message = seeded_bits(k, seed | 1);
                let expected = V1BlockEncoder::encode(&legacy, &message);
                let actual = code
                    .encode_systematic(&message, SystematicLayout::MessageParityDescending)
                    .expect("a k-bit message encodes");
                assert_eq!(
                    actual, expected,
                    "BCH({n}, {k}, {t}) must agree bit for bit under the declared layout"
                );
            }
        }
    }

    #[test]
    fn the_packed_path_agrees_with_the_field_generic_reference() {
        for &(m, modulus, _, _, t) in LEGACY_BINARY_POINTS {
            let designed_distance = 2 * t as u64 + 1;
            let packed = binary_narrow_sense(m, modulus, designed_distance);
            let reference = dense_binary_narrow_sense(m, modulus, designed_distance);
            assert_eq!((reference.n(), reference.k()), (packed.n(), packed.k()));

            let bits = seeded_bits(packed.k(), 29);
            let mut symbols = FieldVec::zeros_from(packed.k(), &Fp::<2>::new(0));
            for coordinate in 0..packed.k() {
                symbols.set(coordinate, Fp::<2>::new(u64::from(bits.get(coordinate))));
            }

            for &layout in LAYOUTS {
                let packed_codeword = packed
                    .encode_systematic(&bits, layout)
                    .expect("a k-bit message encodes");
                let reference_codeword = reference
                    .encode_systematic(&symbols, layout)
                    .expect("a k-symbol message encodes");
                for coordinate in 0..packed.n() {
                    assert_eq!(
                        Fp::<2>::new(u64::from(packed_codeword.get(coordinate))),
                        *reference_codeword.get(coordinate),
                        "the packed path must reproduce the reference at {coordinate}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_packed_path_holds_at_the_word_boundaries() {
        // Redundancies 0, 1, 63, 64, and 65: the register is empty, one bit,
        // one word short, exactly one word, and one bit into a second word.
        let codes = [
            binary_narrow_sense(4, 0b10011, 1),
            binary_first_root(4, 0b10011, 0, 2),
            binary_narrow_sense(7, 0b10000011, 21),
            BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
                extension: BinaryPrimeExt::new(Gf2mField::gf256()).expect("a primitive modulus"),
                designed_distance: DesignedDistance::try_from(17).expect("positive"),
            })
            .expect("a valid GF(2^8) narrow-sense spec"),
            BinaryBchCode::construct(BchSpec::PrimitiveFirstRoot {
                extension: BinaryPrimeExt::new(Gf2mField::gf256()).expect("a primitive modulus"),
                first_root: RootExponent::from(0),
                designed_distance: DesignedDistance::try_from(18).expect("positive"),
            })
            .expect("a valid GF(2^8) first-root spec"),
        ];

        for (code, redundancy) in codes.iter().zip([0usize, 1, 63, 64, 65]) {
            assert_eq!(
                BlockCode::redundancy(code),
                redundancy,
                "the boundary case must exercise the redundancy it names"
            );
            let message = seeded_bits(code.k(), redundancy as u64 + 1);
            for &layout in LAYOUTS {
                assert_encodes_a_codeword(code, &message, layout);
            }
        }
    }

    // -- REQ-02: the declared layouts and their mapping --------------------

    #[test]
    fn the_layout_mapping_is_a_bijection_placing_the_message_first() {
        let code = binary_narrow_sense(4, 0b10011, 5);
        for &layout in LAYOUTS {
            let plan = code.systematic_plan(layout);
            let mut seen = vec![false; code.n()];
            for user in 0..code.n() {
                let internal = plan.internal_coordinate(user).expect("a user coordinate");
                assert!(!seen[internal], "the layout maps two users onto {internal}");
                seen[internal] = true;
                assert_eq!(plan.user_coordinate(internal), Ok(user), "inverse mapping");
                assert_eq!(
                    internal >= plan.redundancy(),
                    user < plan.dimension(),
                    "the systematic coordinates are the first k"
                );
            }
        }
    }

    #[test]
    fn out_of_range_coordinates_are_typed_errors() {
        let code = binary_narrow_sense(4, 0b10011, 5);
        let plan = code.systematic_plan(SystematicLayout::default());
        let expected = Err(CodeError::CoordinateOutOfRange {
            coordinate: 15,
            length: 15,
        });
        assert_eq!(plan.internal_coordinate(15), expected);
        assert_eq!(plan.user_coordinate(15), expected);
    }

    #[test]
    fn the_layout_materializes_as_a_coordinate_map() {
        let code = binary_narrow_sense(4, 0b10011, 5);
        for &layout in LAYOUTS {
            let plan = code.systematic_plan(layout);
            let map = plan.to_coordinate_map().expect("a bijection");
            assert_eq!(map.mother_len(), code.n());
            assert_eq!(map.derived_len(), code.n());
            for user in 0..code.n() {
                assert_eq!(
                    map.mother_position(user),
                    plan.internal_coordinate(user),
                    "the materialized map must agree with the arithmetic one"
                );
            }
        }
    }

    #[test]
    fn the_declared_layouts_present_one_internal_codeword() {
        let code = binary_narrow_sense(4, 0b10011, 5);
        let ascending = code.systematic_plan(SystematicLayout::MessageParityAscending);
        let descending = code.systematic_plan(SystematicLayout::MessageParityDescending);

        let message = seeded_bits(code.k(), 0xAE03_BCD0);
        let default_codeword = code
            .encode_systematic(&message, SystematicLayout::MessageParityAscending)
            .expect("a k-bit message encodes");

        // Reading the same internal codeword under the other layout permutes
        // the user coordinates by the composed map; the message block is what
        // the alternative layout must be given to reach that codeword.
        let composed = |user: usize| {
            ascending
                .user_coordinate(descending.internal_coordinate(user).expect("a coordinate"))
                .expect("a coordinate")
        };
        let mut alternative_message = BitVec::zeros(code.k());
        for user in 0..code.k() {
            alternative_message.set(user, default_codeword.get(composed(user)));
        }
        let alternative_codeword = code
            .encode_systematic(
                &alternative_message,
                SystematicLayout::MessageParityDescending,
            )
            .expect("a k-bit message encodes");

        for user in 0..code.n() {
            assert_eq!(
                alternative_codeword.get(user),
                default_codeword.get(composed(user)),
                "the two layouts must present one codeword through the composed map"
            );
        }
    }

    #[test]
    fn every_declared_layout_round_trips_over_each_base_field() {
        let binary = binary_narrow_sense(5, 0b100101, 7);
        let prime = gf5_code(5);
        for &layout in LAYOUTS {
            let message = seeded_bits(binary.k(), 7);
            let codeword = binary
                .encode_systematic(&message, layout)
                .expect("a k-bit message encodes");
            assert_eq!(binary.systematic_message(&codeword, layout), Ok(message));

            let alphabet: Vec<Fp<5>> = (0..5).map(Fp::<5>::new).collect();
            let message = seeded_symbols(&alphabet, prime.k(), 11);
            let codeword = prime
                .encode_systematic(&message, layout)
                .expect("a k-symbol message encodes");
            assert_eq!(prime.systematic_message(&codeword, layout), Ok(message));
        }
    }

    // -- Boundary codes ----------------------------------------------------

    #[test]
    fn the_full_space_code_encodes_the_identity_under_the_default_layout() {
        let code = binary_narrow_sense(4, 0b10011, 1);
        assert_eq!((code.n(), code.k()), (15, 15));

        let message = seeded_bits(15, 3);
        let codeword = BlockEncoder::encode(&code, &message).expect("a 15-bit message encodes");
        assert_eq!(codeword, message, "k = n encodes the message unchanged");

        let prime = gf5_code(1);
        assert_eq!(prime.n(), prime.k());
        let alphabet: Vec<Fp<5>> = (0..5).map(Fp::<5>::new).collect();
        let message = seeded_symbols(&alphabet, prime.k(), 5);
        assert_eq!(BlockEncoder::encode(&prime, &message), Ok(message));
    }

    #[test]
    fn the_zero_dimensional_code_rejects_a_nonempty_message() {
        let code = binary_narrow_sense(4, 0b10011, 16);
        assert_eq!((code.n(), code.k()), (15, 0));

        let error = BlockEncoder::encode(&code, &BitVec::zeros(1))
            .expect_err("a zero-dimensional code has no one-symbol message");
        assert_eq!(
            error,
            CodeError::BufferLengthMismatch {
                expected: 0,
                actual: 1,
            }
        );

        let codeword = BlockEncoder::encode(&code, &BitVec::zeros(0))
            .expect("the empty message is the code's only message");
        assert_eq!(codeword, BitVec::zeros(15), "the only codeword is zero");
    }

    // -- Typed errors ------------------------------------------------------

    #[test]
    fn wrong_buffer_lengths_are_typed_errors() {
        let code = binary_narrow_sense(4, 0b10011, 5);
        let message = BitVec::zeros(code.k());

        assert_eq!(
            BlockEncoder::encode(&code, &BitVec::zeros(code.k() + 1)),
            Err(CodeError::BufferLengthMismatch {
                expected: 7,
                actual: 8,
            })
        );

        let mut codeword = BitVec::zeros(code.n() + 1);
        assert_eq!(
            BlockEncoder::encode_into(&code, &message, &mut codeword),
            Err(CodeError::BufferLengthMismatch {
                expected: 15,
                actual: 16,
            })
        );

        assert_eq!(
            code.systematic_message(&BitVec::zeros(code.n() - 1), SystematicLayout::default()),
            Err(CodeError::BufferLengthMismatch {
                expected: 15,
                actual: 14,
            })
        );
    }

    #[test]
    fn a_message_from_another_presentation_is_rejected() {
        let code = gf9_base_code(4);
        // A second GF(9), presented by another irreducible modulus: the same
        // field size under a different identity.
        let other_modulus = FieldPoly::new(vec![Fp::<3>::new(2), Fp::new(1), Fp::new(1)]);
        let other =
            QuotientField::new(Fp::<3>::zero(), other_modulus).expect("x^2 + x + 2 over GF(3)");
        let foreign = other.ext_zero();
        assert_ne!(
            BlockCode::symbol_zero(&code).field_id(),
            foreign.field_id(),
            "the two presentations must be distinguishable for the check to mean anything"
        );

        let mut message = FieldVec::zeros_from(code.k(), &BlockCode::symbol_zero(&code));
        message.set(0, foreign.clone());

        let error = BlockEncoder::encode(&code, &message)
            .expect_err("a symbol from another presentation is not a code symbol");
        assert_eq!(
            error,
            CodeError::FieldMismatch {
                expected: BlockCode::symbol_zero(&code).field_id(),
                found: foreign.field_id(),
            }
        );
    }

    // -- Buffer discipline and trait conformance ---------------------------

    #[test]
    fn a_dirty_codeword_buffer_is_overwritten() {
        let code = binary_narrow_sense(4, 0b10011, 5);
        let message = seeded_bits(code.k(), 13);
        let expected = BlockEncoder::encode(&code, &message).expect("a k-bit message encodes");

        let mut buffer = BitVec::ones(code.n());
        BlockEncoder::encode_into(&code, &message, &mut buffer).expect("a sized buffer accepts");
        assert_eq!(buffer, expected, "a dirty buffer is overwritten, not mixed");

        let prime = gf5_code(5);
        let alphabet: Vec<Fp<5>> = (0..5).map(Fp::<5>::new).collect();
        let message = seeded_symbols(&alphabet, prime.k(), 17);
        let expected = BlockEncoder::encode(&prime, &message).expect("a k-symbol message encodes");
        let mut buffer = FieldVec::zeros_from(prime.n(), &Fp::<5>::new(4));
        for index in 0..prime.n() {
            buffer.set(index, Fp::<5>::new(4));
        }
        BlockEncoder::encode_into(&prime, &message, &mut buffer).expect("a sized buffer accepts");
        assert_eq!(buffer, expected, "a dirty buffer is overwritten, not mixed");
    }

    #[test]
    fn bch_codes_satisfy_the_block_encoder_contract() {
        let binary = binary_narrow_sense(4, 0b10011, 5);
        let message = seeded_bits(binary.k(), 19);
        conformance::block_encoder_contract(&binary, &message);
        conformance::binary_v1_encoder_agrees(&binary, &message);

        let prime = gf5_code(5);
        let alphabet: Vec<Fp<5>> = (0..5).map(Fp::<5>::new).collect();
        let message = seeded_symbols(&alphabet, prime.k(), 23);
        conformance::block_encoder_contract(&prime, &message);
    }
}
