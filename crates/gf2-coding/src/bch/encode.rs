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
//! Encoding runs in two representations, selected at compile time by the
//! code's symbol storage: a packed binary path over `u64` words for
//! [`BitVec`], and the field-generic path over base field elements for
//! [`FieldVec`]. The packed path never materializes `FieldVec<Fp<2>>`.
//! [`SystematicKernel`] is the contract those two paths implement.
//!
//! # Algorithm families
//!
//! Within one representation, a batch encode chooses among the registered
//! algorithm families of [`EncodeFamily`]. They are mathematically
//! equivalent: each computes the remainder of $x^r m(x)$ modulo the
//! generator, so a batch encoded under any two of them is bit-identical, and
//! they differ only in how many message coefficients one reduction step
//! consumes and in what precomputation that step reads.
//!
//! Selection is a conjunction of two independent decisions, and neither can
//! overrule the other:
//!
//! - **availability** — [`SystematicKernel::family_available`] answers
//!   whether the representation implements a family for the plan at hand.
//!   [`EncodeFamily::REFERENCE`] is available for every plan in every
//!   representation, which is what makes the dispatch total;
//! - **admission** — the `encode` selector family of
//!   [`crate::tuning::CodingTuning`] answers whether the active profile
//!   admits it at this redundancy and batch length. The conservative section
//!   admits only the reference, so a process that installs no profile
//!   encodes exactly as it did before any family beyond the reference was
//!   registered.
//!
//! The seam walks [`EncodeFamily::REGISTERED`] in order and takes the first
//! entry both decisions accept, which the reference always terminates. A
//! caller that needs a named family rather than the selected one, as a
//! differential check does, calls
//! [`encode_batch_family_into`](BchCode::encode_batch_family_into) and
//! receives [`BchError::EncodeFamilyUnavailable`] rather than a silent
//! substitution.
//!
//! This is not the SIMD feature-detection seam. That one chooses an
//! instruction-set implementation of a fixed algorithm inside a family; this
//! one chooses the algorithm, and a family may use the other seam internally.
//!
//! # Complexity
//!
//! Let $r = n - k$. Encoding one message under
//! [`EncodeFamily::REFERENCE`] costs $O(k r)$ base-field multiply-adds in
//! the field-generic path and $O(k \lceil r/64 \rceil)$ word operations plus
//! $O(n)$ bit writes in the packed binary path.
//! [`EncodeFamily::TableRemainder`] runs
//! $O((k / 32) \lceil r/64 \rceil)$ word operations over the same $O(n)$ bit
//! writes, after a table build of $O(1024 \lceil r/64 \rceil)$ word writes
//! paid once per batch call.
//!
//! # Workspaces
//!
//! The recurrence runs over the buffers [`EncodeRegisters`] holds together:
//! an $r$-symbol shift register, the generator's low $r$ coefficients in the
//! same words, and whatever reduction table the selected family reads. The
//! entry points that take no
//! workspace — [`encode_systematic`](BchCode::encode_systematic),
//! [`encode_systematic_into`](BchCode::encode_systematic_into),
//! [`BlockEncoder::encode_into`], and
//! [`encode_batch`](BchCode::encode_batch) — run over a pair the calling
//! thread keeps for the register word type they encode in. That pair is sized
//! on the thread's first such encode and reset in place on every one
//! afterwards, so the buffers reach the allocator once per thread and a
//! repeated encode reaches it only through whatever result the caller asked
//! the code to allocate. Thread-local storage is what keeps those entry
//! points lock-free: concurrent encodes over one shared code borrow registers
//! no other thread can reach, so they neither serialize nor share a buffer. A
//! thread holds one pair per register word type until it exits, keeping the
//! allocation of the largest redundancy it has encoded in that type.
//!
//! Resetting a pair rewrites the low coefficients, which is $O(r)$ per call.
//! [`BchCode::encode_workspace`] pays that once for a whole sequence instead
//! and hands the pair to the caller.
//! [`encode_systematic_with`](BchCode::encode_systematic_with) then reuses the
//! same [`BchEncodeWorkspace`] for every message: the register is overwritten
//! in place and the coefficients are read, so neither is resized, replaced, or
//! pushed to, and no other value on the path outlives a call — the plan is
//! `usize` arithmetic over a borrowed generator, and base-field elements share
//! their field by reference count rather than by allocating.
//!
//! A workspace belongs to the code that produced it. It carries a fingerprint
//! of that code, and an encode rejects a foreign workspace with
//! [`BchError::WorkspaceMismatch`] even when its buffer lengths happen to
//! match. The fingerprint covers the dimensions, the presentations of the base
//! and splitting fields, and the defining set; a layout does not enter it,
//! because the layout is coordinate arithmetic outside the registers, so one
//! workspace serves every [`SystematicLayout`] of its code.
//!
//! # Batch encoding
//!
//! [`encode_batch_into`](BchCode::encode_batch_into) runs the selected
//! family over a slice of messages with one workspace.
//! [`encode_batch_parallel_into`](BchCode::encode_batch_parallel_into) splits
//! the same slice into one contiguous partition per supplied workspace and
//! encodes the partitions concurrently. The split is balanced: with $m$
//! messages over $w \le m$ workspaces each partition holds
//! $\lfloor m/w \rfloor$ messages and the first $m \bmod w$ hold one more, so
//! every workspace receives a non-empty partition whether or not $w$ divides
//! $m$. Each worker owns its workspace and writes only its own output
//! positions, so:
//!
//! - the output is in input order for every worker count, because a partition
//!   writes the codeword of message $i$ at position $i$ and nothing reassembles
//!   the results afterwards;
//! - the bytes are identical across worker counts, because the worker count
//!   chooses partition boundaries and each message's codeword is a function of
//!   that message alone.
//!
//! The family is selected once for the whole batch, from the message count
//! rather than from any partition's length, and every worker then runs that
//! one family over its own registers. The bytes are therefore identical
//! across worker counts and across families alike, and a worker count never
//! changes which algorithm the batch runs.
//!
//! One worker runs the whole batch directly on the calling thread, so a
//! one-worker dispatch pays no fan-out cost and is the honest sequential
//! reference for a speedup measurement. Above one, the workspaces are halved
//! until each half holds one and the halves go to the rayon pool, whose width
//! is [`max_parallel_batch_workers`]; a larger worker count is still valid and
//! still produces those bytes, its partitions sharing the threads there are.
//! Without the `parallel` feature every worker count runs its partitions in
//! index order on the calling thread.
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
//!
//! The performance path over a batch: one workspace per worker, allocated
//! once, and an output slice the caller owns.
//!
//! ```
//! use gf2_coding::bch::encode::SystematicLayout;
//! use gf2_coding::bch::spec::{BchSpec, BinaryBchCode, DesignedDistance};
//! use gf2_coding::traits::block::BlockEncoder;
//! use gf2_core::field::extension::BinaryPrimeExt;
//! use gf2_core::gf2m::Gf2mField;
//! use gf2_core::BitVec;
//! use std::num::NonZeroUsize;
//!
//! let extension = BinaryPrimeExt::new(Gf2mField::new(5, 0b100101))?;
//! let code = BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
//!     extension,
//!     designed_distance: DesignedDistance::try_from(7)?,
//! })?;
//! let layout = SystematicLayout::default();
//!
//! let messages: Vec<BitVec> = (0..64)
//!     .map(|seed| BitVec::random_seeded(code.k(), seed))
//!     .collect();
//! let mut codewords = vec![BitVec::zeros(code.n()); messages.len()];
//!
//! let workers = NonZeroUsize::new(4).expect("four is nonzero");
//! let mut workspaces = code.encode_workspaces(workers);
//! code.encode_batch_parallel_into(&messages, layout, &mut workspaces, &mut codewords)?;
//!
//! // Input order, and the same bytes the single-message path produces.
//! for (message, codeword) in messages.iter().zip(&codewords) {
//!     assert_eq!(*codeword, code.encode(message)?);
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::num::NonZeroUsize;
use std::slice;

use gf2_core::field::extension::{FieldExtension, FieldId, FieldIdentity};
use gf2_core::field::{FieldPoly, FieldVec, FiniteField};
use gf2_core::gfp::Fp;
use gf2_core::BitVec;

use crate::bch::error::BchError;
use crate::bch::spec::BchCode;
use crate::error::CodeError;
use crate::traits::block::{BlockCode, BlockEncoder, SymbolMatrix, SymbolSequence};
use crate::transform::CoordinateMap;
use crate::tuning::EncodeSelectors;

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

    /// The generator's coefficient of $x^{degree}$, for a `degree` below the
    /// redundancy, which is the form [`EncodeRegisters::low`] holds the low
    /// coefficients in.
    fn low_coefficient(&self, degree: usize) -> &F {
        self.generator.try_coeff(degree).unwrap_or(&self.zero)
    }
}

// ---------------------------------------------------------------------------
// The registered algorithm families
// ---------------------------------------------------------------------------

/// One registered batch-encoding algorithm family.
///
/// Every family computes the same systematic codeword — the one the
/// [module level](self#coordinate-convention) defines — so a batch encoded
/// under any two of them is bit-identical. They differ in how many message
/// coefficients one reduction step consumes and in what precomputation that
/// step needs, which is what makes the choice between them a profile
/// question rather than a semantic one.
///
/// [`REGISTERED`](Self::REGISTERED) is the registry: the dispatch seam walks
/// it in order and takes the first entry the representation implements for
/// the plan and the active profile admits. [`REFERENCE`](Self::REFERENCE)
/// closes it and is implemented for every plan in every representation, so
/// the walk always terminates on an available family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum EncodeFamily {
    /// The bit-serial shift-register recurrence, one message coefficient per
    /// step.
    ///
    /// This is the reference: every representation implements it for every
    /// plan, and every other family is checked bit-identical against it.
    PolyRemainderScalar,

    /// Table-driven remainder, consuming
    /// [`TABLE_REMAINDER_BLOCK_BITS`] message coefficients per step through
    /// four 256-entry reduction tables.
    ///
    /// A step costs one packed shift and four table reads whatever the
    /// generator degree is, against one shift and one conditional
    /// generator XOR per coefficient in the reference. The tables are a
    /// function of the generator alone and are built once per batch call.
    TableRemainder,
}

impl EncodeFamily {
    /// The family every representation implements for every plan.
    pub const REFERENCE: Self = Self::PolyRemainderScalar;

    /// Every registered family, in the order the dispatch seam considers
    /// them.
    ///
    /// The seam takes the first entry that is both available for the plan's
    /// representation and admitted by the active profile, so a family placed
    /// earlier is preferred wherever both admit it. [`REFERENCE`](Self::REFERENCE)
    /// is last because it is the fallback the walk is guaranteed to reach.
    pub const REGISTERED: &'static [Self] = &[Self::TableRemainder, Self::REFERENCE];

    /// Returns this family's stable spelling, the one the workload-selection
    /// contract registers it under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::PolyRemainderScalar => "poly-remainder-scalar",
            Self::TableRemainder => "table-remainder",
        }
    }

    /// Returns the family a stable spelling names, or `None` for a spelling
    /// no registered family carries.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::REGISTERED
            .iter()
            .copied()
            .find(|family| family.name() == name)
    }
}

impl core::fmt::Display for EncodeFamily {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.name())
    }
}

/// Message coefficients [`EncodeFamily::TableRemainder`] consumes per step.
pub const TABLE_REMAINDER_BLOCK_BITS: usize = 32;

/// Block coefficients one reduction table is indexed by.
const TABLE_REMAINDER_TABLE_BITS: usize = 8;

/// Reduction tables [`EncodeFamily::TableRemainder`] reads per step, one per
/// [`TABLE_REMAINDER_TABLE_BITS`]-coefficient slice of a
/// [`TABLE_REMAINDER_BLOCK_BITS`]-bit block.
const TABLE_REMAINDER_TABLES: usize = TABLE_REMAINDER_BLOCK_BITS / TABLE_REMAINDER_TABLE_BITS;

/// Entries in one reduction table, one per value of the slice it reads.
const TABLE_REMAINDER_ENTRIES: usize = 1 << TABLE_REMAINDER_TABLE_BITS;

/// Conservative minimum redundancy at which [`EncodeFamily::TableRemainder`]
/// is selected.
///
/// [`usize::MAX`] keeps every code on [`EncodeFamily::REFERENCE`] under the
/// conservative profile: the crossover between the two families is a
/// measurement, and no committed receipt has made it on this repository's
/// implementations. Installing a profile with a lower
/// `encode.table_remainder_min_redundancy` moves the boundary; the selected
/// codewords are the same bytes either way.
pub const TABLE_REMAINDER_MIN_REDUNDANCY: usize = usize::MAX;

/// Conservative minimum batch length at which
/// [`EncodeFamily::TableRemainder`] is selected.
///
/// One is the neutral bound: it excludes no batch. The table family's
/// per-call table build is the cost this bound exists to amortize, and what
/// batch length repays it is the same unmeasured crossover
/// [`TABLE_REMAINDER_MIN_REDUNDANCY`] describes.
pub const TABLE_REMAINDER_MIN_BATCH: usize = 1;

/// Decides whether the active selectors admit `family` for a plan of
/// redundancy `redundancy` over a batch of `batch_len` messages.
///
/// Admission is the profile's half of the decision. It composes with, and
/// never overrides, the availability
/// [`SystematicKernel::family_available`] reports, so no selector value
/// selects a family a representation cannot run.
fn family_admitted(
    selectors: &EncodeSelectors,
    family: EncodeFamily,
    redundancy: usize,
    batch_len: usize,
) -> bool {
    match family {
        EncodeFamily::PolyRemainderScalar => true,
        EncodeFamily::TableRemainder => {
            redundancy >= selectors.table_remainder_min_redundancy()
                && batch_len >= selectors.table_remainder_min_batch()
        }
    }
}

/// Reports the family the seam selects from already-resolved selectors.
///
/// `available` answers [`SystematicKernel::family_available`] for the plan
/// being encoded under. The walk is over [`EncodeFamily::REGISTERED`] in
/// order and ends at [`EncodeFamily::REFERENCE`], which every representation
/// makes available, so the result is always a family the caller can run.
fn select_family_resolved(
    selectors: &EncodeSelectors,
    redundancy: usize,
    batch_len: usize,
    available: impl Fn(EncodeFamily) -> bool,
) -> EncodeFamily {
    EncodeFamily::REGISTERED
        .iter()
        .copied()
        .find(|&family| {
            available(family) && family_admitted(selectors, family, redundancy, batch_len)
        })
        .unwrap_or(EncodeFamily::REFERENCE)
}

/// Reports the family the seam selects for `plan` over `batch_len` messages
/// under the process-wide profile.
///
/// This is the one place a batch entry point reads the profile. It resolves
/// the coding tuning section once per batch call, never per message.
fn select_family<F, S>(plan: &SystematicPlan<'_, F>, batch_len: usize) -> EncodeFamily
where
    F: FieldIdentity,
    S: SystematicKernel<F>,
{
    let active = crate::tuning::active();
    select_family_resolved(active.encode(), plan.redundancy(), batch_len, |family| {
        S::family_available(family, plan)
    })
}

// ---------------------------------------------------------------------------
// The reusable registers and the workspace that owns them
// ---------------------------------------------------------------------------

/// The buffers a systematic recurrence runs over.
///
/// A representation fixes the word type $W$: base-field elements for the
/// field-generic path, packed `u64` words for the packed binary path. The
/// buffers are sized from the plan's redundancy $r$ and are only ever
/// overwritten in place afterwards, which is what makes an encode that owns
/// them allocation-free.
///
/// Build these through [`BchCode::encode_workspace`], which pairs them with
/// the fingerprint of the code they were derived from;
/// [`SystematicKernel::registers`] is the representation-specific half a
/// kernel supplies.
#[derive(Clone, Debug)]
pub struct EncodeRegisters<W> {
    /// The shift register, holding the running remainder.
    ///
    /// The recurrence resets it before the first message degree, so its
    /// contents on entry carry no meaning.
    pub register: Vec<W>,

    /// The generator's low $r$ coefficients, in ascending degree order.
    ///
    /// The reduction $x^r \equiv -(g_{r-1}x^{r-1} + \cdots + g_0)$ uses
    /// exactly these, so the monic leading coefficient never enters the
    /// recurrence.
    pub low: Vec<W>,

    /// The selected family's precomputed reduction table.
    ///
    /// [`SystematicKernel::reset_family`] fills this for a family that
    /// reduces through a table and leaves it empty for one that does not, so
    /// [`EncodeFamily::REFERENCE`] carries no table storage. Its length and
    /// layout belong to the family that wrote it.
    pub tables: Vec<W>,
}

/// The reusable scratch one code's systematic encoding needs.
///
/// A workspace pairs one representation's [`EncodeRegisters`] with a
/// fingerprint of the code that built them. Allocate it once with
/// [`BchCode::encode_workspace`] and pass the same value to every
/// [`encode_systematic_with`](BchCode::encode_systematic_with) or
/// [`encode_batch_into`](BchCode::encode_batch_into) call: the buffers are
/// sized there and only overwritten afterwards, so the per-message path
/// touches no allocator. See the [module level](self#workspaces) for the whole
/// no-allocation argument and for why one workspace serves every layout of its
/// code.
#[derive(Clone, Debug)]
pub struct BchEncodeWorkspace<W> {
    /// Fingerprint of the producing code, checked on every encode.
    stamp: u64,
    /// The buffers the recurrence runs over.
    registers: EncodeRegisters<W>,
}

impl<W> BchEncodeWorkspace<W> {
    /// Returns the fingerprint of the code this workspace was built for.
    ///
    /// Two workspaces are interchangeable exactly when this value agrees;
    /// an encode reports [`BchError::WorkspaceMismatch`] carrying both
    /// fingerprints otherwise.
    pub fn code_stamp(&self) -> u64 {
        self.stamp
    }

    /// Returns the buffers the recurrence runs over.
    pub fn registers(&self) -> &EncodeRegisters<W> {
        &self.registers
    }
}

/// Returns how many workers a parallel batch encode can occupy here.
///
/// This is the rayon pool's width with the `parallel` feature, and one
/// without it. A larger worker count remains valid and produces the same
/// bytes: it fixes the partition boundaries and the workspace count, and the
/// partitions then share the threads available. See the
/// [module level](self#batch-encoding).
#[must_use]
pub fn max_parallel_batch_workers() -> NonZeroUsize {
    #[cfg(feature = "parallel")]
    {
        NonZeroUsize::new(rayon::current_num_threads()).unwrap_or(NonZeroUsize::MIN)
    }
    #[cfg(not(feature = "parallel"))]
    {
        NonZeroUsize::MIN
    }
}

// ---------------------------------------------------------------------------
// The calling thread's scratch registers
// ---------------------------------------------------------------------------

/// One thread's scratch [`EncodeRegisters`] for one register word type.
struct ScratchRegisters {
    /// The [`SystematicKernel::Word`] the erased registers are stored in.
    word: TypeId,
    /// The `EncodeRegisters<W>` of the `W` that `word` names.
    registers: Box<dyn Any>,
}

thread_local! {
    /// The registers the entry points that take no workspace run over, one
    /// entry per register word type this thread has encoded in.
    ///
    /// A `thread_local!` item cannot be generic, and a `static` inside a
    /// generic function is one item every monomorphization shares, so an
    /// entry carries its word type as a [`TypeId`] and erases the registers
    /// to `dyn Any`. A thread encodes in a handful of word types — this crate
    /// implements two — so selecting an entry by scanning for its `TypeId`
    /// costs less than hashing one.
    static ENCODE_SCRATCH: RefCell<Vec<ScratchRegisters>> = const { RefCell::new(Vec::new()) };
}

/// Runs `encode` over the calling thread's scratch registers for `W`.
///
/// The entry the thread's first call for `W` creates is the entry every later
/// call borrows, so the buffers are allocated once per thread and word type.
/// `encode` receives them in the state the previous encode left them in and
/// sizes them itself, which [`SystematicKernel::reset_registers`] does.
///
/// The borrow is held for the whole of `encode`. Nothing a kernel reaches
/// encodes, so nothing re-enters this function while its registers are
/// borrowed.
fn with_encode_scratch<W, R>(encode: impl FnOnce(&mut EncodeRegisters<W>) -> R) -> R
where
    W: 'static,
{
    ENCODE_SCRATCH.with_borrow_mut(|scratch| {
        let word = TypeId::of::<W>();
        let index = scratch
            .iter()
            .position(|entry| entry.word == word)
            .unwrap_or_else(|| {
                scratch.push(ScratchRegisters {
                    word,
                    registers: Box::new(EncodeRegisters::<W> {
                        register: Vec::new(),
                        low: Vec::new(),
                        tables: Vec::new(),
                    }),
                });
                scratch.len() - 1
            });
        let registers = scratch[index]
            .registers
            .downcast_mut::<EncodeRegisters<W>>()
            .expect("the entry a word type's TypeId selects holds that word type's registers");
        encode(registers)
    })
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
///
/// [`encode_systematic_with`](Self::encode_systematic_with) is the primitive:
/// it runs the recurrence over registers the caller owns and allocates
/// nothing. [`encode_systematic_into`](Self::encode_systematic_into) is the
/// form that takes no workspace, and runs over the calling thread's scratch
/// registers.
pub trait SystematicKernel<F: FieldIdentity>: SymbolSequence<F> {
    /// The word this representation's shift register is stored in.
    ///
    /// The bound is `'static` because the entry points that take no workspace
    /// select their scratch registers by this type's [`TypeId`].
    type Word: Clone + core::fmt::Debug + 'static;

    /// Sizes `registers` for `plan` and writes its generator's low
    /// coefficients into them.
    ///
    /// Both buffers come back at the length this plan's recurrence reads,
    /// with `low` carrying the generator's low $r$ coefficients in this
    /// representation's words. The register's contents carry no meaning on
    /// entry to an encode, so only its length is established here. A buffer
    /// that is already long enough is rewritten in place and keeps its
    /// allocation.
    ///
    /// # Complexity
    ///
    /// $O(r)$ words, allocating only to reach a length the buffers have not
    /// held before.
    fn reset_registers(plan: &SystematicPlan<'_, F>, registers: &mut EncodeRegisters<Self::Word>);

    /// Builds the registers `plan`'s recurrence runs over, sized from its
    /// redundancy and carrying its generator's low coefficients.
    ///
    /// # Complexity
    ///
    /// Two allocations, together $O(r)$ words.
    fn registers(plan: &SystematicPlan<'_, F>) -> EncodeRegisters<Self::Word> {
        let mut registers = EncodeRegisters {
            register: Vec::new(),
            low: Vec::new(),
            tables: Vec::new(),
        };
        Self::reset_registers(plan, &mut registers);
        registers
    }

    /// Decides whether this representation implements `family` for `plan`.
    ///
    /// This is the representation's half of the dispatch decision; the
    /// active profile supplies the other half, and neither can select a
    /// family the other rejects. [`EncodeFamily::REFERENCE`] is available for
    /// every plan, which the default implementation reports and an override
    /// preserves, so a representation that adds no family of its own stays on
    /// the reference recurrence.
    fn family_available(family: EncodeFamily, plan: &SystematicPlan<'_, F>) -> bool {
        let _ = plan;
        family == EncodeFamily::REFERENCE
    }

    /// Prepares `registers` for `family` beyond what
    /// [`reset_registers`](Self::reset_registers) established.
    ///
    /// A batch call runs this once per workspace before its first message,
    /// so a family whose step reads precomputed state builds that state here
    /// rather than per message. The default implementation does nothing,
    /// which is what [`EncodeFamily::REFERENCE`] needs.
    ///
    /// # Complexity
    ///
    /// Family-dependent, and paid once per batch call rather than per
    /// message.
    fn reset_family(
        family: EncodeFamily,
        plan: &SystematicPlan<'_, F>,
        registers: &mut EncodeRegisters<Self::Word>,
    ) {
        let _ = (family, plan, registers);
    }

    /// Writes the systematic codeword of `message` into `codeword` under
    /// `family`.
    ///
    /// Every family writes the codeword
    /// [`encode_systematic_with`](Self::encode_systematic_with) writes, so
    /// this is a choice of algorithm and never a choice of result. The
    /// default implementation runs the reference recurrence whatever the
    /// family is, which is what makes an unimplemented family degrade to the
    /// reference rather than to a wrong answer.
    ///
    /// `registers` has passed [`reset_family`](Self::reset_family) for the
    /// same `family` and `plan`.
    ///
    /// # Errors
    ///
    /// The errors of
    /// [`encode_systematic_with`](Self::encode_systematic_with).
    fn encode_systematic_family(
        family: EncodeFamily,
        plan: &SystematicPlan<'_, F>,
        message: &Self,
        registers: &mut EncodeRegisters<Self::Word>,
        codeword: &mut Self,
    ) -> Result<(), CodeError> {
        let _ = family;
        Self::encode_systematic_with(plan, message, registers, codeword)
    }

    /// Writes the systematic codeword of `message` into `codeword`, using
    /// `registers` as its whole working storage.
    ///
    /// The register is reset before the first message degree, so registers
    /// left behind by a previous encode need no clearing.
    ///
    /// # Errors
    ///
    /// Returns the [`CodeError`] reported by
    /// [`SystematicPlan::validate_lengths`] when `message` does not hold
    /// `plan.dimension()` symbols or `codeword` does not hold
    /// `plan.length()`, and [`CodeError::BufferLengthMismatch`] when either
    /// buffer of `registers` is not the length
    /// [`registers`](Self::registers) gives it for this plan. No output
    /// symbol is written in either case.
    fn encode_systematic_with(
        plan: &SystematicPlan<'_, F>,
        message: &Self,
        registers: &mut EncodeRegisters<Self::Word>,
        codeword: &mut Self,
    ) -> Result<(), CodeError>;

    /// Writes the systematic codeword of `message` into `codeword`.
    ///
    /// This is the form that takes no workspace: it runs over the calling
    /// thread's scratch registers, which are allocated on that thread's first
    /// encode in this word type and reset in place on every one afterwards,
    /// so a repeated encode performs no allocation of its own. Concurrent
    /// encodes borrow one pair of registers per thread and never wait on each
    /// other.
    ///
    /// # Errors
    ///
    /// Returns the [`CodeError`] reported by
    /// [`SystematicPlan::validate_lengths`] when `message` does not hold
    /// `plan.dimension()` symbols or `codeword` does not hold
    /// `plan.length()`; no output symbol is written in that case.
    ///
    /// # Complexity
    ///
    /// That of [`encode_systematic_with`](Self::encode_systematic_with) plus
    /// the $O(r)$ reset, which [`BchCode::encode_workspace`] pays once for a
    /// whole sequence instead.
    fn encode_systematic_into(
        plan: &SystematicPlan<'_, F>,
        message: &Self,
        codeword: &mut Self,
    ) -> Result<(), CodeError> {
        with_encode_scratch(|registers: &mut EncodeRegisters<Self::Word>| {
            Self::reset_registers(plan, registers);
            Self::encode_systematic_with(plan, message, registers, codeword)
        })
    }
}

/// Decides that both buffers of `registers` hold `words` entries, the length
/// [`SystematicKernel::registers`] gives them for the plan being encoded
/// under.
fn validate_registers<W>(words: usize, registers: &EncodeRegisters<W>) -> Result<(), CodeError> {
    for buffer in [&registers.register, &registers.low] {
        if buffer.len() != words {
            return Err(CodeError::BufferLengthMismatch {
                expected: words,
                actual: buffer.len(),
            });
        }
    }
    Ok(())
}

impl<F: FieldIdentity + 'static> SystematicKernel<F> for FieldVec<F> {
    /// The recurrence runs over base-field elements themselves.
    type Word = F;

    fn reset_registers(plan: &SystematicPlan<'_, F>, registers: &mut EncodeRegisters<F>) {
        let redundancy = plan.redundancy();
        let zero = plan.symbol_zero();
        registers.register.resize_with(redundancy, || zero.clone());
        registers.low.resize_with(redundancy, || zero.clone());
        for (degree, slot) in registers.low.iter_mut().enumerate() {
            slot.clone_from(plan.low_coefficient(degree));
        }
    }

    /// Runs the shift-register recurrence over base-field elements.
    ///
    /// # Errors
    ///
    /// Returns [`CodeError::BufferLengthMismatch`] for a message, codeword,
    /// or register buffer of the wrong length.
    ///
    /// # Complexity
    ///
    /// $O(k r)$ field multiply-adds over the caller's $r$-element register,
    /// with no allocation.
    fn encode_systematic_with(
        plan: &SystematicPlan<'_, F>,
        message: &Self,
        registers: &mut EncodeRegisters<F>,
        codeword: &mut Self,
    ) -> Result<(), CodeError> {
        plan.validate_lengths(message.len(), codeword.len())?;

        let redundancy = plan.redundancy();
        validate_registers(redundancy, registers)?;
        let EncodeRegisters { register, low, .. } = registers;

        // Reduce x^r m(x) modulo g one message degree at a time, highest
        // first: the feedback symbol is the register's top coefficient plus
        // the message coefficient entering it.
        if redundancy > 0 {
            let zero = plan.symbol_zero();
            for slot in register.iter_mut() {
                slot.clone_from(zero);
            }
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
    /// The recurrence runs over packed `u64` words, one bit per coefficient.
    type Word = u64;

    fn reset_registers(plan: &SystematicPlan<'_, Fp<2>>, registers: &mut EncodeRegisters<u64>) {
        let words = plan.redundancy().div_ceil(64);
        registers.register.resize(words, 0);
        registers.low.clear();
        registers.low.resize(words, 0);
        for degree in 0..plan.redundancy() {
            if plan.low_coefficient(degree).is_one() {
                registers.low[degree / 64] |= 1u64 << (degree % 64);
            }
        }
    }

    /// Runs the shift-register recurrence over packed `u64` words.
    ///
    /// Negation is the identity over `GF(2)`, so the register holds the
    /// parity itself rather than its negative.
    ///
    /// # Errors
    ///
    /// Returns [`CodeError::BufferLengthMismatch`] for a message, codeword,
    /// or register buffer of the wrong length.
    ///
    /// # Complexity
    ///
    /// $O(k \lceil r/64 \rceil)$ word operations and $O(n)$ bit writes over
    /// the caller's registers, with no allocation.
    fn encode_systematic_with(
        plan: &SystematicPlan<'_, Fp<2>>,
        message: &Self,
        registers: &mut EncodeRegisters<u64>,
        codeword: &mut Self,
    ) -> Result<(), CodeError> {
        plan.validate_lengths(message.len(), codeword.len())?;

        let redundancy = plan.redundancy();
        validate_registers(redundancy.div_ceil(64), registers)?;
        let EncodeRegisters { register, low, .. } = registers;

        if redundancy > 0 {
            packed_serial_reduce(plan, message, register, low);
        }
        packed_write_codeword(plan, message, register, codeword);
        Ok(())
    }

    /// The packed representation adds [`EncodeFamily::TableRemainder`] for
    /// every plan whose redundancy holds a whole
    /// [`TABLE_REMAINDER_BLOCK_BITS`]-bit block, which is what a step's
    /// shift-out is read from.
    fn family_available(family: EncodeFamily, plan: &SystematicPlan<'_, Fp<2>>) -> bool {
        match family {
            EncodeFamily::PolyRemainderScalar => true,
            EncodeFamily::TableRemainder => plan.redundancy() >= TABLE_REMAINDER_BLOCK_BITS,
        }
    }

    /// Builds the reduction tables [`EncodeFamily::TableRemainder`] reads.
    ///
    /// # Complexity
    ///
    /// $O(\lceil r/64 \rceil)$ words per table entry, so
    /// $O(1024 \lceil r/64 \rceil)$ word writes once per batch call, against
    /// the batch's $O(m k \lceil r/64 \rceil)$ reduction.
    fn reset_family(
        family: EncodeFamily,
        plan: &SystematicPlan<'_, Fp<2>>,
        registers: &mut EncodeRegisters<u64>,
    ) {
        let redundancy = plan.redundancy();
        match family {
            EncodeFamily::PolyRemainderScalar => {}
            EncodeFamily::TableRemainder if redundancy >= TABLE_REMAINDER_BLOCK_BITS => {
                let EncodeRegisters { low, tables, .. } = registers;
                packed_build_tables(redundancy, low, tables);
            }
            EncodeFamily::TableRemainder => {}
        }
    }

    /// Runs `family`'s reduction over packed `u64` words.
    ///
    /// # Errors
    ///
    /// Returns [`CodeError::BufferLengthMismatch`] for a message, codeword,
    /// or register buffer of the wrong length.
    ///
    /// # Complexity
    ///
    /// $O(k \lceil r/64 \rceil)$ word operations for
    /// [`EncodeFamily::REFERENCE`] and
    /// $O((k / 32) \lceil r/64 \rceil)$ for
    /// [`EncodeFamily::TableRemainder`], both plus $O(n)$ bit writes, over
    /// the caller's registers and with no allocation.
    fn encode_systematic_family(
        family: EncodeFamily,
        plan: &SystematicPlan<'_, Fp<2>>,
        message: &Self,
        registers: &mut EncodeRegisters<u64>,
        codeword: &mut Self,
    ) -> Result<(), CodeError> {
        let redundancy = plan.redundancy();
        if family != EncodeFamily::TableRemainder || redundancy < TABLE_REMAINDER_BLOCK_BITS {
            return Self::encode_systematic_with(plan, message, registers, codeword);
        }

        plan.validate_lengths(message.len(), codeword.len())?;
        validate_registers(redundancy.div_ceil(64), registers)?;
        let EncodeRegisters {
            register,
            low,
            tables,
        } = registers;
        packed_table_reduce(plan, message, register, low, tables);
        packed_write_codeword(plan, message, register, codeword);
        Ok(())
    }
}

/// The mask keeping a packed remainder of degree below `redundancy` clear
/// above its top coefficient.
fn packed_tail_mask(redundancy: usize) -> u64 {
    if redundancy.is_multiple_of(64) {
        u64::MAX
    } else {
        (1u64 << (redundancy % 64)) - 1
    }
}

/// Advances the packed remainder in `register` by one degree, injecting
/// `symbol` as the next message coefficient.
///
/// The shift moves the top coefficient out of the register, and masking
/// keeps the words above degree $r - 1$ clear so the next feedback bit reads
/// the top coefficient alone. With `symbol` false this is multiplication by
/// $x$ modulo the generator, which is how the reduction tables' powers are
/// derived.
fn packed_step(register: &mut [u64], low: &[u64], top: usize, tail: u64, symbol: bool) {
    let words = register.len();
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

/// Multiplies the packed remainder in `register` by
/// $x^{\text{TABLE\_REMAINDER\_BLOCK\_BITS}}$, dropping the coefficients
/// that leave the register.
///
/// The caller reads those coefficients first and folds them back through the
/// reduction tables, which is what makes the drop exact rather than lossy.
fn packed_shift_block(register: &mut [u64], tail: u64) {
    let words = register.len();
    let block = TABLE_REMAINDER_BLOCK_BITS as u32;
    for word in (1..words).rev() {
        register[word] = (register[word] << block) | (register[word - 1] >> (64 - block));
    }
    register[0] <<= block;
    register[words - 1] &= tail;
}

/// Reads the [`TABLE_REMAINDER_BLOCK_BITS`] packed bits starting at `offset`,
/// bit $j$ of the result carrying the bit at `offset + j`.
///
/// Bits above the buffer read as zero, which is the zero tail padding a
/// packed buffer maintains.
fn packed_read_block(words: &[u64], offset: usize) -> u32 {
    let word = offset / 64;
    let bit = offset % 64;
    let low = words[word] >> bit;
    let high = if bit == 0 || word + 1 >= words.len() {
        0
    } else {
        words[word + 1] << (64 - bit)
    };
    ((low | high) & u64::from(u32::MAX)) as u32
}

/// Reads the message coefficients of degrees `degree` to
/// `degree + TABLE_REMAINDER_BLOCK_BITS`, bit $j$ carrying the coefficient
/// of $x^{degree + j}$.
///
/// Both declared layouts carry those coefficients in one contiguous run of
/// user coordinates, ascending for
/// [`SystematicLayout::MessageParityAscending`] and descending for
/// [`SystematicLayout::MessageParityDescending`], so the block is one packed
/// read and, for the descending layout, one bit reversal.
fn packed_message_block(plan: &SystematicPlan<'_, Fp<2>>, message: &[u64], degree: usize) -> u32 {
    match plan.layout() {
        SystematicLayout::MessageParityAscending => packed_read_block(message, degree),
        SystematicLayout::MessageParityDescending => {
            let offset = plan.dimension() - degree - TABLE_REMAINDER_BLOCK_BITS;
            packed_read_block(message, offset).reverse_bits()
        }
    }
}

/// Reduces $x^r m(x)$ modulo the generator one message degree at a time,
/// highest first, leaving the remainder in `register`.
fn packed_serial_reduce(
    plan: &SystematicPlan<'_, Fp<2>>,
    message: &BitVec,
    register: &mut [u64],
    low: &[u64],
) {
    let redundancy = plan.redundancy();
    let top = redundancy - 1;
    let tail = packed_tail_mask(redundancy);
    register.fill(0);
    for degree in (0..plan.dimension()).rev() {
        let symbol = message.get(plan.message_at(degree));
        packed_step(register, low, top, tail, symbol);
    }
}

/// Reduces $x^r m(x)$ modulo the generator
/// [`TABLE_REMAINDER_BLOCK_BITS`] message degrees at a time, leaving the
/// remainder in `register`.
///
/// One step splits the register into the coefficients that survive the shift
/// and the block that leaves it, adds the next message block to the latter,
/// and folds the sum back through the tables: for a remainder
/// $s = s_L + s_H x^{r-w}$ and a message block $c$,
/// $s x^{w} + c x^{r} \equiv s_L x^{w} + (s_H + c) x^{r}$, and the tables
/// carry $v x^{r} \bmod g$ for every byte $v$ of that sum. Degrees above the
/// last whole block run through [`packed_serial_reduce`]'s step, so a
/// dimension that is not a multiple of the block width needs no padding.
fn packed_table_reduce(
    plan: &SystematicPlan<'_, Fp<2>>,
    message: &BitVec,
    register: &mut [u64],
    low: &[u64],
    tables: &[u64],
) {
    let redundancy = plan.redundancy();
    let dimension = plan.dimension();
    let words = register.len();
    let top = redundancy - 1;
    let tail = packed_tail_mask(redundancy);
    let blocks = dimension / TABLE_REMAINDER_BLOCK_BITS;

    register.fill(0);
    for degree in (blocks * TABLE_REMAINDER_BLOCK_BITS..dimension).rev() {
        let symbol = message.get(plan.message_at(degree));
        packed_step(register, low, top, tail, symbol);
    }

    let coefficients = message.words();
    for block in (0..blocks).rev() {
        let degree = block * TABLE_REMAINDER_BLOCK_BITS;
        let leaving = packed_read_block(register, redundancy - TABLE_REMAINDER_BLOCK_BITS);
        let folded = leaving ^ packed_message_block(plan, coefficients, degree);
        packed_shift_block(register, tail);
        for table in 0..TABLE_REMAINDER_TABLES {
            let slice =
                (folded >> (TABLE_REMAINDER_TABLE_BITS * table)) as usize % TABLE_REMAINDER_ENTRIES;
            let entry = (table * TABLE_REMAINDER_ENTRIES + slice) * words;
            for (offset, accumulator) in register.iter_mut().enumerate() {
                *accumulator ^= tables[entry + offset];
            }
        }
    }
}

/// Builds the reduction tables of [`EncodeFamily::TableRemainder`]: entry
/// $v$ of table $t$ is $v x^{r + 8t} \bmod g$, packed like the register.
///
/// The entries are linear in the byte value, so a power-of-two entry is the
/// previous one multiplied by $x$ and every other entry is the exclusive or
/// of two entries already written.
fn packed_build_tables(redundancy: usize, low: &[u64], tables: &mut Vec<u64>) {
    let words = low.len();
    let top = redundancy - 1;
    let tail = packed_tail_mask(redundancy);

    tables.clear();
    tables.resize(TABLE_REMAINDER_TABLES * TABLE_REMAINDER_ENTRIES * words, 0);
    for table in 0..TABLE_REMAINDER_TABLES {
        let base = table * TABLE_REMAINDER_ENTRIES * words;
        // The entry for value 1 carries x^{r + 8t}: that is x^r for the first
        // table, and for every later one the previous table's top-bit entry,
        // which carries x^{r + 8t - 1}, multiplied by x.
        if table == 0 {
            tables[base + words..base + 2 * words].copy_from_slice(low);
        } else {
            let top_bit_entry = TABLE_REMAINDER_ENTRIES / 2;
            let previous = base - (TABLE_REMAINDER_ENTRIES - top_bit_entry) * words;
            tables.copy_within(previous..previous + words, base + words);
            packed_step(
                &mut tables[base + words..base + 2 * words],
                low,
                top,
                tail,
                false,
            );
        }
        for bit in 1..TABLE_REMAINDER_TABLE_BITS {
            let source = base + (1 << (bit - 1)) * words;
            let target = base + (1 << bit) * words;
            tables.copy_within(source..source + words, target);
            packed_step(&mut tables[target..target + words], low, top, tail, false);
        }
        for value in 2..TABLE_REMAINDER_ENTRIES {
            if value.is_power_of_two() {
                continue;
            }
            let target = base + value * words;
            let rest = base + (value & (value - 1)) * words;
            let lowest = base + (1 << value.trailing_zeros()) * words;
            for offset in 0..words {
                tables[target + offset] = tables[rest + offset] ^ tables[lowest + offset];
            }
        }
    }
}

/// Writes the systematic codeword of `message` from the reduced `register`.
///
/// The message occupies the first $k$ user coordinates under every declared
/// layout, and the layout decides which parity degree each remaining
/// coordinate carries.
fn packed_write_codeword(
    plan: &SystematicPlan<'_, Fp<2>>,
    message: &BitVec,
    register: &[u64],
    codeword: &mut BitVec,
) {
    for user in 0..plan.dimension() {
        codeword.set(user, message.get(user));
    }
    for user in plan.dimension()..plan.length() {
        let parity = plan.internal_at(user);
        debug_assert!(
            parity < plan.redundancy(),
            "a systematic layout carries the coordinates above k onto the parity degrees"
        );
        codeword.set(user, (register[parity / 64] >> (parity % 64)) & 1 == 1);
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
    /// The recurrence runs over the calling thread's scratch registers, so
    /// the call allocates nothing once that thread has encoded in this code's
    /// word type; see the [module level](self#workspaces).
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

    // -- The reusable-workspace and batch surface --------------------------

    /// Allocates the scratch one systematic encode needs.
    ///
    /// Build this once and pass the same value to every
    /// [`encode_systematic_with`](Self::encode_systematic_with) or
    /// [`encode_batch_into`](Self::encode_batch_into) call: the buffers are
    /// sized and filled here and only overwritten afterwards, so the
    /// per-message path neither allocates nor pays the $O(r)$ reset the
    /// entry points that take no workspace pay per call. The workspace serves
    /// every [`SystematicLayout`] of this code.
    ///
    /// # Complexity
    ///
    /// Two allocations, together $O(r)$ words, plus the $O(r)$ fingerprint of
    /// [`code_stamp`](BchEncodeWorkspace::code_stamp).
    pub fn encode_workspace(&self) -> BchEncodeWorkspace<S::Word> {
        BchEncodeWorkspace {
            stamp: self.encode_stamp(),
            registers: S::registers(&self.systematic_plan(SystematicLayout::default())),
        }
    }

    /// Allocates one workspace per worker for a parallel batch encode.
    ///
    /// The length of the result is the worker count
    /// [`encode_batch_parallel_into`](Self::encode_batch_parallel_into) reads:
    /// each worker owns one of these and shares none of it.
    ///
    /// # Complexity
    ///
    /// `workers` times [`encode_workspace`](Self::encode_workspace).
    pub fn encode_workspaces(&self, workers: NonZeroUsize) -> Vec<BchEncodeWorkspace<S::Word>> {
        let stamp = self.encode_stamp();
        let plan = self.systematic_plan(SystematicLayout::default());
        (0..workers.get())
            .map(|_| BchEncodeWorkspace {
                stamp,
                registers: S::registers(&plan),
            })
            .collect()
    }

    /// Encodes `message` under `layout` into the caller's buffer, reusing
    /// `workspace`.
    ///
    /// The codeword is the one
    /// [`encode_systematic_into`](Self::encode_systematic_into) writes; this
    /// path differs only in owning no buffers of its own.
    ///
    /// # Errors
    ///
    /// - [`BchError::WorkspaceMismatch`] when `workspace` was built by another
    ///   code, decided before any output symbol is written.
    /// - [`BchError::Code`] wrapping [`CodeError::BufferLengthMismatch`] when
    ///   `message` does not hold $k$ symbols or `codeword` does not hold $n$,
    ///   and [`CodeError::FieldMismatch`] when the message symbols carry a
    ///   runtime field identity other than this code's.
    ///
    /// # Complexity
    ///
    /// That of [`encode_systematic_into`](Self::encode_systematic_into),
    /// without its $O(r)$ register reset, plus $O(r)$ mixing steps to decide
    /// the workspace fingerprint. A batch pays that check once for the whole
    /// batch.
    pub fn encode_systematic_with(
        &self,
        message: &S,
        layout: SystematicLayout,
        workspace: &mut BchEncodeWorkspace<S::Word>,
        codeword: &mut S,
    ) -> Result<(), BchError> {
        self.validate_workspaces(slice::from_ref(workspace))?;
        let plan = self.systematic_plan(layout);
        plan.validate_lengths(message.len(), codeword.len())?;
        validate_symbol_field(&plan, message)?;
        S::encode_systematic_with(&plan, message, &mut workspace.registers, codeword)?;
        Ok(())
    }

    /// Encodes every message of `messages` into the matching position of
    /// `codewords`, reusing one `workspace`.
    ///
    /// The algorithm family comes from the batch length and the active
    /// profile, through the seam described at the
    /// [module level](self#algorithm-families). It is a family this code's
    /// representation makes available, so the family error
    /// [`encode_batch_family_into`](Self::encode_batch_family_into) can
    /// report is unreachable from here.
    ///
    /// # Errors
    ///
    /// - [`BchError::WorkspaceMismatch`] when `workspace` was built by another
    ///   code.
    /// - [`BchError::Code`] wrapping [`CodeError::BufferLengthMismatch`] when
    ///   `codewords` does not have one entry per message, when a message does
    ///   not hold $k$ symbols, or when a codeword buffer does not hold $n$,
    ///   and [`CodeError::FieldMismatch`] for a message whose symbols carry a
    ///   foreign field identity.
    ///
    /// Every message is decided before the first is encoded, so a rejected
    /// batch leaves `codewords` untouched.
    ///
    /// # Complexity
    ///
    /// One reduction per message in the selected family, with the workspace
    /// check and the family's preparation each paid once.
    pub fn encode_batch_into(
        &self,
        messages: &[S],
        layout: SystematicLayout,
        workspace: &mut BchEncodeWorkspace<S::Word>,
        codewords: &mut [S],
    ) -> Result<(), BchError> {
        let plan = self.systematic_plan(layout);
        let family = select_family::<X::Base, S>(&plan, messages.len());
        self.encode_batch_family_into(family, messages, layout, workspace, codewords)
    }

    /// Encodes every message of `messages` under an explicitly named
    /// `family`, reusing one `workspace`.
    ///
    /// [`encode_batch_into`](Self::encode_batch_into) is this method with the
    /// family taken from the active profile. Naming one is what a
    /// differential check between two families needs; the codewords are the
    /// same bytes whichever family writes them.
    ///
    /// # Errors
    ///
    /// - [`BchError::EncodeFamilyUnavailable`] when this code's
    ///   representation does not implement `family` for this plan, decided
    ///   before any output symbol is written.
    /// - Otherwise the errors of
    ///   [`encode_batch_into`](Self::encode_batch_into).
    ///
    /// # Complexity
    ///
    /// One reduction per message in `family`, with the workspace check and
    /// the family's preparation each paid once.
    pub fn encode_batch_family_into(
        &self,
        family: EncodeFamily,
        messages: &[S],
        layout: SystematicLayout,
        workspace: &mut BchEncodeWorkspace<S::Word>,
        codewords: &mut [S],
    ) -> Result<(), BchError> {
        self.validate_workspaces(slice::from_ref(workspace))?;
        let plan = self.systematic_plan(layout);
        if !S::family_available(family, &plan) {
            return Err(BchError::EncodeFamilyUnavailable { family });
        }
        validate_batch(&plan, messages, codewords)?;
        S::reset_family(family, &plan, &mut workspace.registers);
        encode_partition(family, &plan, messages, &mut workspace.registers, codewords);
        Ok(())
    }

    /// Decides whether this code's representation implements `family` under
    /// `layout`.
    ///
    /// [`EncodeFamily::REFERENCE`] is available for every code; the families
    /// beyond it depend on the representation and on the plan's shape.
    pub fn encode_family_available(&self, family: EncodeFamily, layout: SystematicLayout) -> bool {
        S::family_available(family, &self.systematic_plan(layout))
    }

    /// Reports the family the batch entry points select for a batch of
    /// `batch_len` messages under `layout`.
    ///
    /// The report is the selection itself, not a prediction of it: the batch
    /// entry points resolve the same active profile through the same walk
    /// over [`EncodeFamily::REGISTERED`].
    pub fn selected_encode_family(
        &self,
        layout: SystematicLayout,
        batch_len: usize,
    ) -> EncodeFamily {
        select_family::<X::Base, S>(&self.systematic_plan(layout), batch_len)
    }

    /// Encodes every message of `messages` into a new vector of codewords.
    ///
    /// The result is in input order and holds one codeword per message; the
    /// empty batch produces the empty vector.
    ///
    /// # Errors
    ///
    /// Propagates the [`CodeError`] that
    /// [`encode_systematic_into`](Self::encode_systematic_into) reports for a
    /// message this code does not encode.
    ///
    /// # Complexity
    ///
    /// One recurrence per message over the calling thread's scratch
    /// registers, plus the output vector.
    pub fn encode_batch(
        &self,
        messages: &[S],
        layout: SystematicLayout,
    ) -> Result<Vec<S>, CodeError> {
        let plan = self.systematic_plan(layout);
        let mut codewords = vec![S::zeroed(plan.length(), plan.symbol_zero()); messages.len()];
        validate_batch(&plan, messages, &codewords)?;
        let family = select_family::<X::Base, S>(&plan, messages.len());
        with_encode_scratch(|registers: &mut EncodeRegisters<S::Word>| {
            S::reset_registers(&plan, registers);
            S::reset_family(family, &plan, registers);
            encode_partition(family, &plan, messages, registers, &mut codewords);
        });
        Ok(codewords)
    }

    /// Encodes `messages` into `codewords` across `workspaces.len()` workers.
    ///
    /// The batch is split into one contiguous partition per workspace, and
    /// each worker encodes its own partition into the matching positions of
    /// `codewords`. The output is therefore in input order, and identical for
    /// every worker count; see the [module level](self#batch-encoding) for
    /// that argument and for what runs where.
    ///
    /// The split is balanced to within one message, so every workspace up to
    /// the batch length receives a non-empty partition whether or not the
    /// workspace count divides the batch. A workspace count above the batch
    /// length leaves the surplus workspaces unused.
    ///
    /// # Errors
    ///
    /// - [`BchError::WorkspaceMismatch`] when any workspace was built by
    ///   another code.
    /// - [`BchError::Code`] wrapping [`CodeError::BufferLengthMismatch`] when
    ///   `workspaces` is empty, when `codewords` does not have one entry per
    ///   message, when a message does not hold $k$ symbols, or when a codeword
    ///   buffer does not hold $n$, and [`CodeError::FieldMismatch`] for a
    ///   message whose symbols carry a foreign field identity.
    ///
    /// Every argument is decided before the first message is encoded, so a
    /// rejected batch leaves `codewords` untouched.
    ///
    /// # Complexity
    ///
    /// One recurrence per message, spread over the partitions, plus one
    /// workspace check per workspace.
    pub fn encode_batch_parallel_into(
        &self,
        messages: &[S],
        layout: SystematicLayout,
        workspaces: &mut [BchEncodeWorkspace<S::Word>],
        codewords: &mut [S],
    ) -> Result<(), BchError>
    where
        X::Base: Send + Sync,
        S: Send + Sync,
        S::Word: Send,
    {
        if workspaces.is_empty() {
            return Err(CodeError::BufferLengthMismatch {
                expected: 1,
                actual: 0,
            }
            .into());
        }
        self.validate_workspaces(workspaces)?;
        let plan = self.systematic_plan(layout);
        validate_batch(&plan, messages, codewords)?;
        let family = select_family::<X::Base, S>(&plan, messages.len());
        encode_partitions(family, &plan, messages, workspaces, codewords);
        Ok(())
    }

    /// Encodes `messages` across `workers` workers into a new vector of
    /// codewords.
    ///
    /// This is [`encode_batch_parallel_into`](Self::encode_batch_parallel_into)
    /// with the workspaces and the output allocated for the caller, and
    /// carries the same ordering and worker-count guarantees.
    ///
    /// # Errors
    ///
    /// Propagates the [`CodeError`] that
    /// [`encode_systematic_into`](Self::encode_systematic_into) reports for a
    /// message this code does not encode.
    ///
    /// # Complexity
    ///
    /// One recurrence per message spread over `workers` partitions, plus
    /// `workers` workspaces and the output vector.
    pub fn encode_batch_parallel(
        &self,
        messages: &[S],
        layout: SystematicLayout,
        workers: NonZeroUsize,
    ) -> Result<Vec<S>, CodeError>
    where
        X::Base: Send + Sync,
        S: Send + Sync,
        S::Word: Send,
    {
        let plan = self.systematic_plan(layout);
        let mut codewords = vec![S::zeroed(plan.length(), plan.symbol_zero()); messages.len()];
        validate_batch(&plan, messages, &codewords)?;
        let family = select_family::<X::Base, S>(&plan, messages.len());
        let mut workspaces = self.encode_workspaces(workers);
        encode_partitions(family, &plan, messages, &mut workspaces, &mut codewords);
        Ok(codewords)
    }

    /// Fingerprints this code for an encode workspace: the dimensions, the
    /// presentations of the base and splitting fields, and the defining set,
    /// which together fix the generator the registers carry (FNV-1a).
    ///
    /// The field presentations have to be there. A defining set names root
    /// exponents, not roots, so two presentations of one splitting field
    /// carry the same exponents at the same dimensions while their generators
    /// over the base field differ; the moduli are what separates them.
    fn encode_stamp(&self) -> u64 {
        const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;
        let mut hash = OFFSET;
        let mut mix = |value: u64| {
            hash ^= value;
            hash = hash.wrapping_mul(PRIME);
        };
        mix(self.n() as u64);
        mix(self.k() as u64);
        mix_field_presentation(&mut mix, self.extension().base_id());
        mix_field_presentation(&mut mix, self.extension().ext_id());
        mix(self.defining_set().len() as u64);
        for exponent in self.defining_set() {
            mix(exponent.get());
        }
        hash
    }

    /// Rejects a workspace built for another code, whatever its geometry.
    fn validate_workspaces(
        &self,
        workspaces: &[BchEncodeWorkspace<S::Word>],
    ) -> Result<(), BchError> {
        let expected_stamp = self.encode_stamp();
        for workspace in workspaces {
            if workspace.stamp != expected_stamp {
                return Err(BchError::WorkspaceMismatch {
                    expected_stamp,
                    actual_stamp: workspace.stamp,
                });
            }
        }
        Ok(())
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

/// Mixes a field presentation into a fingerprint: its characteristic and
/// absolute degree, then every modulus of its tower, lowest coordinate first.
///
/// The moduli separate two presentations of one field size, which the
/// characteristic and degree alone cannot.
fn mix_field_presentation(mix: &mut impl FnMut(u64), id: &FieldId) {
    mix(id.characteristic());
    mix(id.degree() as u64);
    let mut node = Some(id);
    while let Some(current) = node {
        if let Some(modulus) = current.modulus() {
            mix(modulus.coefficients().len() as u64);
            for &coordinate in modulus.coefficients() {
                mix(coordinate);
            }
        }
        node = current.base();
    }
}

/// Decides that a whole batch is one this plan encodes, before any codeword
/// is written.
///
/// # Errors
///
/// [`CodeError::BufferLengthMismatch`] when `codewords` does not have one
/// entry per message, or when a message or codeword buffer has the wrong
/// length, and [`CodeError::FieldMismatch`] for a message whose symbols carry
/// a foreign field identity. The first rejected message decides.
fn validate_batch<F, S>(
    plan: &SystematicPlan<'_, F>,
    messages: &[S],
    codewords: &[S],
) -> Result<(), CodeError>
where
    F: FieldIdentity,
    S: SystematicKernel<F>,
{
    if codewords.len() != messages.len() {
        return Err(CodeError::BufferLengthMismatch {
            expected: messages.len(),
            actual: codewords.len(),
        });
    }
    for (message, codeword) in messages.iter().zip(codewords) {
        plan.validate_lengths(message.len(), codeword.len())?;
        validate_symbol_field(plan, message)?;
    }
    Ok(())
}

/// Encodes one worker's contiguous partition in index order under `family`.
///
/// Every argument has already passed [`validate_batch`], `registers` has the
/// geometry [`SystematicKernel::registers`] gives this plan, and it has
/// passed [`SystematicKernel::reset_family`] for `family`, so the kernel
/// cannot reject anything here.
fn encode_partition<F, S>(
    family: EncodeFamily,
    plan: &SystematicPlan<'_, F>,
    messages: &[S],
    registers: &mut EncodeRegisters<S::Word>,
    codewords: &mut [S],
) where
    F: FieldIdentity,
    S: SystematicKernel<F>,
{
    for (message, codeword) in messages.iter().zip(codewords.iter_mut()) {
        S::encode_systematic_family(family, plan, message, registers, codeword)
            .expect("a validated batch encodes under its own plan");
    }
}

/// The first message index of partition `index` when `messages` messages are
/// split over `workers` contiguous partitions.
///
/// The split is balanced: each partition holds $\lfloor m/w \rfloor$
/// messages and the first $m \bmod w$ hold one more, so no two partitions
/// differ by more than one message and none is empty for $w \le m$. The
/// offset is a function of the two counts alone, which is what makes a
/// partition the same messages however the recursion below reaches it: a
/// sub-range of $w'$ workers holding the messages this rule gives them
/// re-derives exactly the same boundaries inside itself.
fn partition_offset(messages: usize, workers: usize, index: usize) -> usize {
    index * (messages / workers) + index.min(messages % workers)
}

/// Encodes `messages` into `codewords` across exactly one contiguous
/// partition per workspace of `workspaces`, splitting the workspaces in half
/// until each half holds one.
///
/// Partition boundaries come from [`partition_offset`] and so depend only on
/// the message and workspace counts, never on which thread reaches a half
/// first. A workspace encodes only the messages of its own partition and
/// writes only their positions of `codewords`, so the output is in input
/// order whatever the worker count is. With the `parallel` feature the two
/// halves go to [`rayon::join`], which runs both however many threads it has;
/// without it they run left before right on the calling thread. One workspace
/// runs its partition directly, so a one-worker dispatch reaches no pool.
fn encode_partitions_over<F, S>(
    family: EncodeFamily,
    plan: &SystematicPlan<'_, F>,
    messages: &[S],
    workspaces: &mut [BchEncodeWorkspace<S::Word>],
    codewords: &mut [S],
) where
    F: FieldIdentity + Send + Sync,
    S: SystematicKernel<F> + Send + Sync,
    S::Word: Send,
{
    if workspaces.len() < 2 {
        encode_partition(
            family,
            plan,
            messages,
            &mut workspaces[0].registers,
            codewords,
        );
        return;
    }

    let left_workers = workspaces.len() / 2;
    let at = partition_offset(messages.len(), workspaces.len(), left_workers);
    let (left_messages, right_messages) = messages.split_at(at);
    let (left_codewords, right_codewords) = codewords.split_at_mut(at);
    let (left_workspaces, right_workspaces) = workspaces.split_at_mut(left_workers);

    #[cfg(feature = "parallel")]
    rayon::join(
        || encode_partitions_over(family, plan, left_messages, left_workspaces, left_codewords),
        || {
            encode_partitions_over(
                family,
                plan,
                right_messages,
                right_workspaces,
                right_codewords,
            )
        },
    );
    #[cfg(not(feature = "parallel"))]
    {
        encode_partitions_over(family, plan, left_messages, left_workspaces, left_codewords);
        encode_partitions_over(
            family,
            plan,
            right_messages,
            right_workspaces,
            right_codewords,
        );
    }
}

/// Encodes `messages` into `codewords` across one contiguous partition per
/// workspace, every partition running the family the seam selected for the
/// whole batch.
///
/// Every workspace up to the batch length receives a partition, balanced to
/// within one message by [`partition_offset`]; workspaces beyond the batch
/// length go unused, because a partition of no messages is not work. The
/// output is in input order and independent of both the workspace count and
/// the order the partitions happen to run in.
fn encode_partitions<F, S>(
    family: EncodeFamily,
    plan: &SystematicPlan<'_, F>,
    messages: &[S],
    workspaces: &mut [BchEncodeWorkspace<S::Word>],
    codewords: &mut [S],
) where
    F: FieldIdentity + Send + Sync,
    S: SystematicKernel<F> + Send + Sync,
    S::Word: Send,
{
    let workers = workspaces.len().min(messages.len());
    if workers == 0 {
        return;
    }
    for workspace in workspaces.iter_mut().take(workers) {
        S::reset_family(family, plan, &mut workspace.registers);
    }
    encode_partitions_over(
        family,
        plan,
        messages,
        &mut workspaces[..workers],
        codewords,
    );
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
    /// The call owns no buffers: it writes the caller's `codeword` and runs
    /// the recurrence over the calling thread's scratch registers, so a
    /// repeated encode reaches no allocator. See the
    /// [module level](self#workspaces).
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

    // -- REQ-01/REQ-02: the workspace, batch, and parallel batch paths ------

    /// The worker counts the determinism claim covers: one, two, and the
    /// widest this process supports.
    fn worker_counts() -> Vec<NonZeroUsize> {
        let mut counts = vec![
            NonZeroUsize::MIN,
            NonZeroUsize::new(2).expect("two is nonzero"),
            max_parallel_batch_workers(),
        ];
        counts.sort_unstable();
        counts.dedup();
        counts
    }

    /// A batch of `count` deterministic messages, pairwise distinct so that a
    /// reordered result is observable.
    fn seeded_bit_batch(dimension: usize, count: usize, seed: u64) -> Vec<BitVec> {
        (0..count)
            .map(|index| {
                seeded_bits(
                    dimension,
                    seed.wrapping_mul(0x9E37_79B9).wrapping_add(index as u64) | 1,
                )
            })
            .collect()
    }

    /// Asserts that every encoding path of `code` writes the codewords the
    /// allocating single-message path writes, in input order.
    ///
    /// This is the REQ-01 identity together with the REQ-02 worker-count
    /// invariance: the allocating reference, the workspace single path, the
    /// two batch paths, and the parallel batch path at each supported worker
    /// count must agree symbol for symbol.
    fn assert_every_path_agrees<X, S, M>(
        code: &BchCode<X, S, M>,
        messages: &[S],
        layout: SystematicLayout,
    ) where
        X: FieldExtension,
        X::Base: Send + Sync,
        S: SystematicKernel<X::Base> + Send + Sync,
        S::Word: Send,
        M: SymbolMatrix<X::Base>,
    {
        let zero = BlockCode::symbol_zero(code);
        let expected: Vec<S> = messages
            .iter()
            .map(|message| {
                code.encode_systematic(message, layout)
                    .expect("a k-symbol message encodes")
            })
            .collect();

        let mut workspace = code.encode_workspace();
        let mut buffer = S::zeroed(code.n(), &zero);
        for (message, want) in messages.iter().zip(&expected) {
            code.encode_systematic_with(message, layout, &mut workspace, &mut buffer)
                .expect("a k-symbol message encodes with its own workspace");
            assert_eq!(
                &buffer, want,
                "the workspace path must match the allocating path"
            );

            if layout == SystematicLayout::default() {
                BlockEncoder::encode_into(code, message, &mut buffer)
                    .expect("a k-symbol message encodes");
                assert_eq!(
                    &buffer, want,
                    "the canonical encode_into must match the allocating path"
                );
            }
        }

        let mut codewords = vec![S::zeroed(code.n(), &zero); messages.len()];
        code.encode_batch_into(messages, layout, &mut workspace, &mut codewords)
            .expect("a validated batch encodes");
        assert_eq!(
            codewords, expected,
            "the workspace batch must match message by message"
        );

        assert_eq!(
            code.encode_batch(messages, layout)
                .expect("a validated batch encodes"),
            expected,
            "the allocating batch must match message by message"
        );

        for workers in worker_counts() {
            assert_eq!(
                code.encode_batch_parallel(messages, layout, workers)
                    .expect("a validated batch encodes"),
                expected,
                "the allocating parallel batch must match at {workers} workers"
            );

            let mut codewords = vec![S::zeroed(code.n(), &zero); messages.len()];
            let mut workspaces = code.encode_workspaces(workers);
            code.encode_batch_parallel_into(messages, layout, &mut workspaces, &mut codewords)
                .expect("a validated batch encodes");
            assert_eq!(
                codewords, expected,
                "the parallel batch must match at {workers} workers"
            );
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(8))]

        #[test]
        fn prop_every_binary_path_writes_one_codeword(
            point in 0usize..LEGACY_BINARY_POINTS.len(),
            seed: u64,
        ) {
            let (m, modulus, _, _, t) = LEGACY_BINARY_POINTS[point];
            let code = binary_narrow_sense(m, modulus, 2 * t as u64 + 1);
            let messages = seeded_bit_batch(code.k(), 9, seed);
            for &layout in LAYOUTS {
                assert_every_path_agrees(&code, &messages, layout);
            }
        }

        #[test]
        fn prop_every_prime_base_path_writes_one_codeword(
            designed_distance in 1u64..=6,
            seed: u64,
        ) {
            let code = gf5_code(designed_distance);
            let alphabet: Vec<Fp<5>> = (0..5).map(Fp::<5>::new).collect();
            let messages: Vec<FieldVec<Fp<5>>> = (0..9)
                .map(|index| seeded_symbols(&alphabet, code.k(), seed.wrapping_add(index) | 1))
                .collect();
            for &layout in LAYOUTS {
                assert_every_path_agrees(&code, &messages, layout);
            }
        }
    }

    #[test]
    fn the_extension_base_paths_write_one_codeword() {
        let code = gf9_base_code(4);
        let alphabet = gf9().elements().expect("GF(9) is enumerable");
        let messages: Vec<FieldVec<QuotientElement<Fp<3>>>> = (0..5)
            .map(|index| seeded_symbols(&alphabet, code.k(), index | 1))
            .collect();
        for &layout in LAYOUTS {
            assert_every_path_agrees(&code, &messages, layout);
        }
    }

    #[test]
    fn the_packed_workspace_path_holds_at_the_word_boundaries() {
        // The same redundancies 0, 1, 63, 64, and 65 the single-message
        // packed path pins: the register is empty, one bit, one word short,
        // exactly one word, and one bit into a second word.
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
            assert_eq!(BlockCode::redundancy(code), redundancy);
            let messages = seeded_bit_batch(code.k(), 4, redundancy as u64 + 1);
            for &layout in LAYOUTS {
                assert_every_path_agrees(code, &messages, layout);
            }
        }
    }

    // -- REQ-02: worker counts choose a schedule, never a result -----------

    #[test]
    fn a_batch_keeps_its_input_order_at_every_worker_count() {
        let code = binary_narrow_sense(6, 0b1000011, 7);
        let messages = seeded_bit_batch(code.k(), 37, 0xAE03_BCD0);
        let reversed: Vec<BitVec> = messages.iter().rev().cloned().collect();
        let layout = SystematicLayout::default();

        let sequential = code
            .encode_batch(&messages, layout)
            .expect("a validated batch encodes");

        for workers in worker_counts() {
            let parallel = code
                .encode_batch_parallel(&messages, layout, workers)
                .expect("a validated batch encodes");
            assert_eq!(
                parallel, sequential,
                "byte equality must hold at {workers} workers"
            );

            let reversed_out = code
                .encode_batch_parallel(&reversed, layout, workers)
                .expect("a validated batch encodes");
            let expected: Vec<BitVec> = sequential.iter().rev().cloned().collect();
            assert_eq!(
                reversed_out, expected,
                "position i must carry the codeword of message i at {workers} workers"
            );
        }
    }

    #[test]
    fn a_batch_beyond_the_worker_count_covers_every_message() {
        // A worker count above the batch length leaves the surplus workspaces
        // unused, and a batch shorter than the partition still covers every
        // message exactly once.
        let code = binary_narrow_sense(4, 0b10011, 5);
        let layout = SystematicLayout::default();
        for count in [0usize, 1, 2, 3, 5, 8, 13] {
            let messages = seeded_bit_batch(code.k(), count, count as u64 + 1);
            let expected = code
                .encode_batch(&messages, layout)
                .expect("a validated batch encodes");
            assert_eq!(expected.len(), count);
            for workers in worker_counts()
                .into_iter()
                .chain([NonZeroUsize::new(16).expect("sixteen is nonzero")])
            {
                assert_eq!(
                    code.encode_batch_parallel(&messages, layout, workers)
                        .expect("a validated batch encodes"),
                    expected,
                    "a {count}-message batch at {workers} workers"
                );
            }
        }
    }

    #[test]
    fn every_supplied_workspace_encodes_its_own_partition() {
        // Batch lengths the worker count does not divide: a partition of
        // ceil(count / workers) messages yields fewer partitions than there
        // are workspaces and leaves the last ones idle.
        let code = binary_narrow_sense(6, 0b1000011, 7);
        let layout = SystematicLayout::default();

        for (count, workers) in [(6usize, 4usize), (7, 3), (37, 8), (5, 5), (9, 2), (13, 4)] {
            let messages = seeded_bit_batch(code.k(), count, count as u64 * 31 + workers as u64);
            let expected = code
                .encode_batch(&messages, layout)
                .expect("a validated batch encodes");
            let workers = NonZeroUsize::new(workers).expect("a positive worker count");

            let mut workspaces = code.encode_workspaces(workers);
            let mut codewords = vec![BitVec::zeros(code.n()); count];
            code.encode_batch_parallel_into(&messages, layout, &mut workspaces, &mut codewords)
                .expect("a validated batch encodes");
            assert_eq!(
                codewords, expected,
                "a {count}-message batch at {workers} workers must match the sequential path"
            );

            // A worker whose low coefficients are cleared runs the recurrence
            // with no feedback and writes zero parity, so the positions that
            // differ from the reference are exactly the ones it wrote. Reading
            // a partition off the output that way needs every reference parity
            // to be nonzero.
            for codeword in &expected {
                assert!(
                    (code.k()..code.n()).any(|user| codeword.get(user)),
                    "a cleared worker's output is distinguishable only where the \
                     reference parity is nonzero"
                );
            }

            let mut partitions: Vec<std::ops::Range<usize>> = Vec::with_capacity(workers.get());
            for worker in 0..workers.get() {
                let mut workspaces = code.encode_workspaces(workers);
                workspaces[worker].registers.low.fill(0);
                let mut codewords = vec![BitVec::zeros(code.n()); count];
                code.encode_batch_parallel_into(&messages, layout, &mut workspaces, &mut codewords)
                    .expect("a validated batch encodes");

                let written: Vec<usize> = (0..count)
                    .filter(|&index| codewords[index] != expected[index])
                    .collect();
                let (Some(&first), Some(&last)) = (written.first(), written.last()) else {
                    panic!("workspace {worker} of {workers} encoded no message of {count}");
                };
                assert_eq!(
                    last - first + 1,
                    written.len(),
                    "workspace {worker} of {workers} wrote a discontiguous set of positions"
                );
                partitions.push(first..last + 1);
            }

            // Consecutive, disjoint, covering the batch, and balanced.
            assert_eq!(
                partitions[0].start, 0,
                "the first partition starts the batch"
            );
            assert_eq!(
                partitions[workers.get() - 1].end,
                count,
                "the last partition ends the batch"
            );
            for pair in partitions.windows(2) {
                assert_eq!(
                    pair[0].end, pair[1].start,
                    "partitions must abut: {partitions:?}"
                );
            }
            let lengths: Vec<usize> = partitions.iter().map(|range| range.len()).collect();
            let longest = lengths.iter().max().expect("a positive worker count");
            let shortest = lengths.iter().min().expect("a positive worker count");
            assert!(
                longest - shortest <= 1,
                "a {count}-message batch over {workers} workers must be balanced, got {lengths:?}"
            );
        }
    }

    #[test]
    fn the_full_space_code_batches_the_identity() {
        let code = binary_narrow_sense(4, 0b10011, 1);
        assert_eq!((code.n(), code.k()), (15, 15));
        let messages = seeded_bit_batch(code.k(), 4, 3);
        let layout = SystematicLayout::default();

        let codewords = code
            .encode_batch_parallel(&messages, layout, NonZeroUsize::new(3).expect("three"))
            .expect("a validated batch encodes");
        assert_eq!(codewords, messages, "k = n batches the messages unchanged");
    }

    // -- REQ-01: an encode reaches no allocator --------------------------

    /// The address, length, and capacity of each register buffer. The code
    /// sizes both once and only overwrites them afterwards, so an unchanged
    /// snapshot witnesses that neither buffer moved or grew, and therefore
    /// that the encodes between two reads reallocated nothing.
    fn register_shape<W>(registers: &EncodeRegisters<W>) -> Vec<(usize, usize, usize)> {
        [&registers.register, &registers.low]
            .into_iter()
            .map(|buffer| (buffer.as_ptr() as usize, buffer.len(), buffer.capacity()))
            .collect()
    }

    /// That snapshot for the buffers a caller's workspace owns.
    fn buffer_shape<W>(workspace: &BchEncodeWorkspace<W>) -> Vec<(usize, usize, usize)> {
        register_shape(workspace.registers())
    }

    /// That snapshot for the scratch registers the calling thread holds for
    /// `W`, or `None` before this thread has encoded in that word type.
    ///
    /// This is what witnesses REQ-01 for the entry points that take no
    /// workspace: those registers are the only buffers such an encode can
    /// allocate.
    fn scratch_shape<W: 'static>() -> Option<Vec<(usize, usize, usize)>> {
        ENCODE_SCRATCH.with_borrow(|scratch| {
            let word = TypeId::of::<W>();
            let entry = scratch.iter().find(|entry| entry.word == word)?;
            let registers = entry
                .registers
                .downcast_ref::<EncodeRegisters<W>>()
                .expect("the entry a word type's TypeId selects holds that word type's registers");
            Some(register_shape(registers))
        })
    }

    #[test]
    fn repeated_encodes_without_a_workspace_reuse_one_scratch() {
        let code = binary_narrow_sense(7, 0b10000011, 21);
        let mut codeword = BitVec::zeros(code.n());

        // The first encode on this thread sizes the scratch; every one after
        // it finds the buffers it needs already there.
        BlockEncoder::encode_into(&code, &seeded_bits(code.k(), 1), &mut codeword)
            .expect("a k-bit message encodes");
        let shape = scratch_shape::<u64>().expect("the first packed encode sizes the scratch");

        for round in 0..64u64 {
            let message = seeded_bits(code.k(), round | 1);
            BlockEncoder::encode_into(&code, &message, &mut codeword)
                .expect("a k-bit message encodes");
            assert_eq!(
                scratch_shape::<u64>().as_ref(),
                Some(&shape),
                "encode_into must not move or grow the thread's scratch"
            );

            for &layout in LAYOUTS {
                code.encode_systematic_into(&message, layout, &mut codeword)
                    .expect("a k-bit message encodes");
                code.encode_systematic(&message, layout)
                    .expect("a k-bit message encodes");
                assert_eq!(
                    scratch_shape::<u64>().as_ref(),
                    Some(&shape),
                    "no workspace-free entry point may move or grow the thread's scratch"
                );
            }
        }

        let messages = seeded_bit_batch(code.k(), 16, 5);
        code.encode_batch(&messages, SystematicLayout::default())
            .expect("a validated batch encodes");
        assert_eq!(
            scratch_shape::<u64>().as_ref(),
            Some(&shape),
            "the allocating batch must not move or grow the thread's scratch"
        );
    }

    #[test]
    fn a_field_generic_scratch_is_reused_too() {
        let code = gf5_code(5);
        let alphabet: Vec<Fp<5>> = (0..5).map(Fp::<5>::new).collect();
        let mut codeword = FieldVec::zeros_from(code.n(), &Fp::<5>::new(0));

        BlockEncoder::encode_into(
            &code,
            &seeded_symbols(&alphabet, code.k(), 1),
            &mut codeword,
        )
        .expect("a k-symbol message encodes");
        let shape =
            scratch_shape::<Fp<5>>().expect("the first field-generic encode sizes the scratch");

        for round in 0..32u64 {
            let message = seeded_symbols(&alphabet, code.k(), round | 1);
            for &layout in LAYOUTS {
                code.encode_systematic_into(&message, layout, &mut codeword)
                    .expect("a k-symbol message encodes");
                assert_eq!(
                    scratch_shape::<Fp<5>>().as_ref(),
                    Some(&shape),
                    "no buffer moved or grew"
                );
            }
        }
    }

    #[test]
    fn interleaved_codes_reset_the_shared_scratch() {
        // One thread's scratch serves every code it encodes, so an encode
        // that reused another code's coefficients or register length instead
        // of resetting them shows up as a wrong codeword here. The oracle is
        // the workspace path, which owns registers of its own.
        let codes = [
            binary_narrow_sense(4, 0b10011, 5),
            binary_narrow_sense(7, 0b10000011, 21),
            binary_first_root(4, 0b10011, 0, 2),
        ];
        let layout = SystematicLayout::default();

        for round in 0..4u64 {
            for code in &codes {
                let message = seeded_bits(code.k(), round | 1);
                let mut workspace = code.encode_workspace();
                let mut expected = BitVec::zeros(code.n());
                code.encode_systematic_with(&message, layout, &mut workspace, &mut expected)
                    .expect("a k-bit message encodes with its own workspace");

                let mut codeword = BitVec::zeros(code.n());
                BlockEncoder::encode_into(code, &message, &mut codeword)
                    .expect("a k-bit message encodes");
                assert_eq!(
                    codeword, expected,
                    "the shared scratch must be reset to the encoding code's registers"
                );
            }
        }
    }

    #[test]
    fn concurrent_encodes_without_a_workspace_agree() {
        // `encode_into` takes `&self` and the scratch it runs over lives in
        // thread-local storage, so threads sharing one code neither wait on
        // each other nor read each other's registers.
        let code = binary_narrow_sense(6, 0b1000011, 7);
        let messages = seeded_bit_batch(code.k(), 24, 0x8F68_699B);
        let expected = code
            .encode_batch(&messages, SystematicLayout::default())
            .expect("a validated batch encodes");

        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| {
                    let mut codeword = BitVec::zeros(code.n());
                    for (message, want) in messages.iter().zip(&expected) {
                        BlockEncoder::encode_into(&code, message, &mut codeword)
                            .expect("a k-bit message encodes");
                        assert_eq!(
                            &codeword, want,
                            "a concurrent encode_into writes the codeword of its own message"
                        );
                    }
                });
            }
        });
    }

    #[test]
    fn repeated_encodes_reuse_one_workspace() {
        let code = binary_narrow_sense(7, 0b10000011, 21);
        let mut workspace = code.encode_workspace();
        let shape = buffer_shape(&workspace);
        let mut codeword = BitVec::zeros(code.n());

        for round in 0..64u64 {
            let message = seeded_bits(code.k(), round | 1);
            for &layout in LAYOUTS {
                code.encode_systematic_with(&message, layout, &mut workspace, &mut codeword)
                    .expect("a k-bit message encodes");
                assert_eq!(
                    buffer_shape(&workspace),
                    shape,
                    "no workspace buffer moved or grew during encoding"
                );
            }
        }

        let messages = seeded_bit_batch(code.k(), 16, 5);
        let mut codewords = vec![BitVec::zeros(code.n()); messages.len()];
        for &layout in LAYOUTS {
            code.encode_batch_into(&messages, layout, &mut workspace, &mut codewords)
                .expect("a validated batch encodes");
            assert_eq!(
                buffer_shape(&workspace),
                shape,
                "no workspace buffer moved or grew during a batch"
            );
        }
    }

    #[test]
    fn a_field_generic_workspace_is_reused_too() {
        let code = gf5_code(5);
        let alphabet: Vec<Fp<5>> = (0..5).map(Fp::<5>::new).collect();
        let mut workspace = code.encode_workspace();
        let shape = buffer_shape(&workspace);
        let mut codeword = FieldVec::zeros_from(code.n(), &Fp::<5>::new(0));

        for round in 0..32u64 {
            let message = seeded_symbols(&alphabet, code.k(), round | 1);
            code.encode_systematic_with(
                &message,
                SystematicLayout::MessageParityDescending,
                &mut workspace,
                &mut codeword,
            )
            .expect("a k-symbol message encodes");
            assert_eq!(buffer_shape(&workspace), shape, "no buffer moved or grew");
        }
    }

    // -- Typed errors on the workspace and batch paths ---------------------

    #[test]
    fn a_foreign_encode_workspace_is_rejected() {
        // Two primitive presentations of GF(16). The codes share their
        // dimensions, their defining set, their base field, and their buffer
        // geometry, so the field presentation inside the stamp is the only
        // thing that can tell the two workspaces apart.
        let code_a = binary_narrow_sense(4, 0b10011, 3);
        let code_b = binary_narrow_sense(4, 0b11001, 3);
        assert_eq!((code_a.n(), code_a.k()), (code_b.n(), code_b.k()));
        assert_eq!(code_a.defining_set(), code_b.defining_set());

        let workspace_a = code_a.encode_workspace();
        let mut foreign = code_b.encode_workspace();
        assert_eq!(
            buffer_shape(&workspace_a)
                .iter()
                .map(|&(_, len, _)| len)
                .collect::<Vec<_>>(),
            buffer_shape(&foreign)
                .iter()
                .map(|&(_, len, _)| len)
                .collect::<Vec<_>>(),
            "matching buffer lengths must not be what rejects the foreign workspace"
        );

        let message = seeded_bits(code_a.k(), 7);
        let mut codeword = BitVec::zeros(code_a.n());
        let error = code_a
            .encode_systematic_with(
                &message,
                SystematicLayout::default(),
                &mut foreign,
                &mut codeword,
            )
            .expect_err("a workspace built for another code is not this code's");
        assert!(matches!(
            error,
            BchError::WorkspaceMismatch { expected_stamp, actual_stamp }
                if expected_stamp != actual_stamp
        ));

        let mut own = code_a.encode_workspace();
        assert!(code_a
            .encode_systematic_with(
                &message,
                SystematicLayout::default(),
                &mut own,
                &mut codeword
            )
            .is_ok());

        // The batch paths reject it before writing anything.
        let messages = vec![message];
        let mut codewords = vec![BitVec::zeros(code_a.n())];
        assert!(matches!(
            code_a.encode_batch_into(
                &messages,
                SystematicLayout::default(),
                &mut foreign,
                &mut codewords,
            ),
            Err(BchError::WorkspaceMismatch { .. })
        ));
        let mut foreign_batch = code_b.encode_workspaces(NonZeroUsize::new(2).expect("two"));
        assert!(matches!(
            code_a.encode_batch_parallel_into(
                &messages,
                SystematicLayout::default(),
                &mut foreign_batch,
                &mut codewords,
            ),
            Err(BchError::WorkspaceMismatch { .. })
        ));
        assert_eq!(
            codewords,
            vec![BitVec::zeros(code_a.n())],
            "a rejected batch writes nothing"
        );
    }

    #[test]
    fn wrong_buffer_lengths_are_typed_errors_on_every_batch_path() {
        let code = binary_narrow_sense(4, 0b10011, 5);
        let layout = SystematicLayout::default();
        let message = BitVec::zeros(code.k());
        let mut workspace = code.encode_workspace();

        let mismatch = |expected, actual| CodeError::BufferLengthMismatch { expected, actual };

        // The single workspace path decides both buffers.
        let mut codeword = BitVec::zeros(code.n() + 1);
        assert_eq!(
            code.encode_systematic_with(&message, layout, &mut workspace, &mut codeword),
            Err(BchError::Code(mismatch(15, 16)))
        );
        let mut codeword = BitVec::zeros(code.n());
        assert_eq!(
            code.encode_systematic_with(
                &BitVec::zeros(code.k() + 1),
                layout,
                &mut workspace,
                &mut codeword
            ),
            Err(BchError::Code(mismatch(7, 8)))
        );

        // A batch names the count first, then the offending message.
        let messages = vec![message.clone(), message.clone()];
        let mut codewords = vec![BitVec::zeros(code.n())];
        assert_eq!(
            code.encode_batch_into(&messages, layout, &mut workspace, &mut codewords),
            Err(BchError::Code(mismatch(2, 1)))
        );
        assert_eq!(
            code.encode_batch(&[BitVec::zeros(code.k() + 1)], layout),
            Err(mismatch(7, 8))
        );
        assert_eq!(
            code.encode_batch_parallel(
                &[BitVec::zeros(code.k() + 1)],
                layout,
                NonZeroUsize::new(2).expect("two")
            ),
            Err(mismatch(7, 8))
        );

        // A parallel batch needs at least one workspace to run on.
        let mut codewords = vec![BitVec::zeros(code.n()); messages.len()];
        assert_eq!(
            code.encode_batch_parallel_into(&messages, layout, &mut [], &mut codewords),
            Err(BchError::Code(mismatch(1, 0)))
        );

        // The kernel decides its registers, whatever built them.
        let plan = code.systematic_plan(layout);
        let mut registers = BitVec::registers(&plan);
        registers.register.push(0);
        assert_eq!(
            BitVec::encode_systematic_with(&plan, &message, &mut registers, &mut codeword),
            Err(mismatch(1, 2))
        );
    }

    #[test]
    fn a_batch_message_from_another_presentation_is_rejected() {
        let code = gf9_base_code(4);
        let other_modulus = FieldPoly::new(vec![Fp::<3>::new(2), Fp::new(1), Fp::new(1)]);
        let other =
            QuotientField::new(Fp::<3>::zero(), other_modulus).expect("x^2 + x + 2 over GF(3)");
        let foreign = other.ext_zero();

        let mut message = FieldVec::zeros_from(code.k(), &BlockCode::symbol_zero(&code));
        message.set(0, foreign.clone());

        let expected = Err(CodeError::FieldMismatch {
            expected: BlockCode::symbol_zero(&code).field_id(),
            found: foreign.field_id(),
        });
        assert_eq!(
            code.encode_batch(&[message.clone()], SystematicLayout::default()),
            expected
        );
        assert_eq!(
            code.encode_batch_parallel(
                &[message],
                SystematicLayout::default(),
                NonZeroUsize::new(2).expect("two")
            ),
            expected
        );
    }

    #[test]
    fn the_empty_batch_encodes_to_the_empty_result() {
        let code = binary_narrow_sense(4, 0b10011, 5);
        let layout = SystematicLayout::default();
        let mut workspace = code.encode_workspace();

        assert_eq!(code.encode_batch(&[], layout), Ok(Vec::new()));
        for workers in worker_counts() {
            assert_eq!(
                code.encode_batch_parallel(&[], layout, workers),
                Ok(Vec::new())
            );
        }
        assert_eq!(
            code.encode_batch_into(&[], layout, &mut workspace, &mut []),
            Ok(())
        );
        let mut workspaces = code.encode_workspaces(NonZeroUsize::new(4).expect("four"));
        assert_eq!(
            code.encode_batch_parallel_into(&[], layout, &mut workspaces, &mut []),
            Ok(())
        );
    }

    #[test]
    fn the_supported_worker_count_is_positive() {
        let workers = max_parallel_batch_workers();
        assert!(
            workers.get() >= 1,
            "at least the calling thread is a worker"
        );
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

    // -----------------------------------------------------------------
    // The dispatch seam, decided without resolving the process profile
    // -----------------------------------------------------------------

    /// Selectors admitting the table family from `redundancy` and `batch` up.
    fn admitting(redundancy: usize, batch: usize) -> EncodeSelectors {
        EncodeSelectors::try_new(redundancy, batch).expect("every bound is admissible")
    }

    /// Availability reporting every registered family.
    fn everything_available(_: EncodeFamily) -> bool {
        true
    }

    #[test]
    fn the_conservative_selectors_hold_every_code_on_the_reference() {
        let conservative = crate::tuning::CodingTuning::CONSERVATIVE;
        for redundancy in [0, 1, 32, 63, 64, 65, 192, 4096] {
            for batch in [0, 1, 16, 4096] {
                assert_eq!(
                    select_family_resolved(
                        conservative.encode(),
                        redundancy,
                        batch,
                        everything_available
                    ),
                    EncodeFamily::REFERENCE,
                    "redundancy {redundancy}, batch {batch}"
                );
            }
        }
    }

    #[test]
    fn an_admitting_profile_moves_the_selection_off_the_reference() {
        let selectors = admitting(32, 16);
        assert_eq!(
            select_family_resolved(&selectors, 32, 16, everything_available),
            EncodeFamily::TableRemainder
        );
        assert_eq!(
            select_family_resolved(&selectors, 31, 16, everything_available),
            EncodeFamily::REFERENCE,
            "a redundancy below the bound stays on the reference"
        );
        assert_eq!(
            select_family_resolved(&selectors, 32, 15, everything_available),
            EncodeFamily::REFERENCE,
            "a batch below the bound stays on the reference"
        );
    }

    #[test]
    fn availability_overrides_an_admitting_profile() {
        let selectors = admitting(0, 0);
        assert_eq!(
            select_family_resolved(&selectors, 192, 4096, |family| family
                == EncodeFamily::REFERENCE),
            EncodeFamily::REFERENCE,
            "a profile cannot select a family the representation does not implement"
        );
    }

    #[test]
    fn the_registry_ends_at_the_reference() {
        assert_eq!(
            EncodeFamily::REGISTERED.last().copied(),
            Some(EncodeFamily::REFERENCE),
            "the walk must terminate on the family every representation implements"
        );
        let mut sorted = EncodeFamily::REGISTERED.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            EncodeFamily::REGISTERED.len(),
            "a family is registered once"
        );
    }

    #[test]
    fn the_field_generic_representation_offers_only_the_reference() {
        let code = dense_binary_narrow_sense(8, 0b100011101, 9);
        let plan = code.systematic_plan(SystematicLayout::default());
        assert!(plan.redundancy() >= TABLE_REMAINDER_BLOCK_BITS);
        for &family in EncodeFamily::REGISTERED {
            assert_eq!(
                FieldVec::<Fp<2>>::family_available(family, &plan),
                family == EncodeFamily::REFERENCE,
                "{family} availability in the field-generic representation"
            );
        }
    }

    #[test]
    fn a_message_block_carries_the_coefficients_its_degrees_name() {
        // Both declared layouts run the message degrees over one contiguous
        // coordinate range, in opposite directions; the block read is where
        // that direction is resolved.
        let code = binary_narrow_sense(8, 0b100011101, 9);
        for &layout in LAYOUTS {
            let plan = code.systematic_plan(layout);
            let message = seeded_bits(plan.dimension(), 41);
            let coefficients = message.words();
            for block in 0..plan.dimension() / TABLE_REMAINDER_BLOCK_BITS {
                let degree = block * TABLE_REMAINDER_BLOCK_BITS;
                let packed = packed_message_block(&plan, coefficients, degree);
                for offset in 0..TABLE_REMAINDER_BLOCK_BITS {
                    assert_eq!(
                        (packed >> offset) & 1 == 1,
                        message.get(plan.message_at(degree + offset)),
                        "{layout:?} degree {}",
                        degree + offset
                    );
                }
            }
        }
    }
}
