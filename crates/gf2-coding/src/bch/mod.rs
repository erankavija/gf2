//! BCH (Bose-Chaudhuri-Hocquenghem) codes.
//!
//! BCH codes are a family of cyclic error-correcting codes that can correct
//! multiple random errors using algebraic decoding over a splitting field of
//! the code-symbol field. [`spec`] constructs them over any supported base
//! field; [`BinaryBchDecoder`] decodes binary codes over GF(2^m).
//!
//! # Organization
//!
//! - [`spec`]: the construction model, its validating pipeline, and the
//!   canonical code type over any supported base field
//! - [`encode`]: systematic encoding over any supported base field, with the
//!   explicit user-layout contract, the layout-declaring view, and the
//!   profile-driven dispatch among equivalent batch-encoding algorithm
//!   families
//! - [`matrix`]: generator and parity-check materialization in the default
//!   user layout and under a declared one, with caller buffers and explicit
//!   opt-in caching
//! - [`error`]: the BCH construction and decoding error surface
//! - [`BinaryBchDecoder`]: binary Berlekamp-Massey and Chien decoding with
//!   verified [`BchDecodeOutcome`]s
//! - [`dvb_t2`]: DVB-T2 standard BCH outer codes
//!
//! Extended, shortened, and punctured BCH codes are
//! [`Extended`](crate::transform::Extended),
//! [`Shortened`](crate::transform::Shortened), and
//! [`Punctured`](crate::transform::Punctured) over a canonical code; each
//! exposes the [`CoordinateMap`](crate::transform::CoordinateMap) from its
//! coordinates to its mother's.
//!
//! # Workflow
//!
//! 1. **Construct.** A concise binary code needs only a relative degree and a
//!    designed distance: [`BinaryBchCode::primitive_narrow_sense_auto`]
//!    selects the splitting field deterministically. A fully explicit code
//!    names its extension witness, length, [`RootSelection`], first root, and
//!    designed distance through a [`BchSpec`] passed to
//!    [`BchCode::construct`]. [`BinaryBchCode`] stores packed bits;
//!    [`DenseBchCode`] stores field elements over any base field.
//! 2. **Encode.** [`BlockEncoder`](crate::traits::block::BlockEncoder)
//!    encodes systematically in the default layout;
//!    [`BchCode::encode_systematic`] takes a layout, and
//!    [`BchCode::systematic_message`] reads the message back.
//! 3. **Decode.** [`BinaryBchDecoder`] corrects binary words in place.
//! 4. **Materialize.**
//!    [`GeneratorMatrixAccess`](crate::traits::block::GeneratorMatrixAccess)
//!    and
//!    [`ParityCheckMatrixAccess`](crate::traits::block::ParityCheckMatrixAccess)
//!    write $G$ and $H$.
//! 5. **Save.** [`FieldMatrix::save_to_file`](gf2_core::field::matrix::FieldMatrix::save_to_file)
//!    atomically writes the canonical checksummed format of
//!    [`gf2_core::io::field_matrix`], which records the field identity; a
//!    [`DenseBchCode`] produces its matrices in that type, including over
//!    GF(2).
//!
//! The example programs `bch_binary_quickstart` and `bch_nonbinary_explicit`
//! in this crate's `examples/` directory run these steps end to end, for a
//! primitive narrow-sense binary code and for the ternary Golay code built
//! from explicit inputs.
//!
//! # Distance terminology
//!
//! - The **designed distance** $\delta$ ([`DesignedDistance`]) is a
//!   construction input: it requests $\delta - 1$ consecutive roots.
//! - The **witnessed bound** ([`BchDistanceBound`]) is derived from the
//!   longest cyclic run of consecutive exponents in the closed defining set.
//!   The closure can lengthen the run, so the witnessed bound is at least the
//!   designed distance and may exceed it.
//! - Both are guaranteed lower bounds on the **minimum distance**, a
//!   property of the code that a caller establishes independently and that
//!   may exceed either bound: the ternary Golay code has witnessed bound 4
//!   and minimum distance 5.
//! - The **correction radius** $t$ is $\lfloor r/2 \rfloor$ for a witnessed
//!   run of length $r$: the radius within which the decoder guarantees
//!   correction.
//!
//! [`spec`'s bound semantics](spec#bound-semantics) state the witness rules.
//!
//! # Coordinate conventions
//!
//! - **Internal.** Coordinate $i$ is the coefficient of $x^i$, so a
//!   codeword is the coefficient vector of a multiple of $g$ modulo
//!   $x^n - 1$ ([`spec`](spec#coordinate-convention)).
//! - **User layout.** A [`SystematicLayout`] maps user coordinates to
//!   internal ones; every layout carries the message in user coordinates
//!   $0$ to $k-1$. The default ascending layout is a cyclic rotation by
//!   $n - k$; the descending layout is a reversal
//!   ([`encode`](encode#systematic-layout-contract)).
//! - **Matrices.** $G = [\,I_k \mid P\,]$ and
//!   $H = [\,-P^{\mathsf T} \mid I_{n-k}\,]$ in the default layout, and
//!   [`LayoutView`] presents both in a declared one ([`matrix`]).
//! - **Decoding.** [`BinaryBchDecoder`] reads internal coordinates. A
//!   cyclic rotation of a codeword is a codeword, so a default-layout word
//!   decodes directly and the reported error positions index the supplied
//!   word. A reversed codeword belongs to the reciprocal code, which in
//!   general differs, so a descending-layout word is mapped to internal
//!   coordinates first, through
//!   [`SystematicPlan::internal_coordinate`](encode::SystematicPlan::internal_coordinate);
//!   [`dvb_t2::DvbT2BchDecoder`] performs that mapping for the DVB-T2 layout.
//! - **Bits.** Packed words use the canonical little-endian bit indexing of
//!   `gf2-core`: coordinate $i$ is bit $i \bmod 64$ of word $\lfloor i/64
//!   \rfloor$.
//!
//! `crates/gf2-coding/docs/SYSTEMATIC_ENCODING_CONVENTION.md` relates these
//! layouts to the `[message | parity]` convention of the other code
//! families.
//!
//! # Error contracts
//!
//! - **Construction** returns a [`BchError`](error::BchError) for every
//!   invalid input, in the validation order listed at
//!   [`spec`](spec#validations); [`BchCode::construct`] enumerates the
//!   variants. No invalid input panics.
//! - **Encoding** returns a [`CodeError`](crate::error::CodeError) for a
//!   wrong message or codeword length or a foreign symbol field, before
//!   writing any output symbol. The entry points over a caller workspace
//!   return a [`BchError`](error::BchError) that wraps those and adds
//!   [`BchError::WorkspaceMismatch`](error::BchError::WorkspaceMismatch) for
//!   a workspace built for another code;
//!   [`BchCode::encode_batch_family_into`] reports
//!   [`BchError::EncodeFamilyUnavailable`](error::BchError::EncodeFamilyUnavailable)
//!   for a named family the representation does not implement.
//! - **Materialization** returns a [`CodeError`](crate::error::CodeError)
//!   for an output matrix of the wrong shape.
//! - **Decoding** rejects a wrong word length and a foreign workspace before
//!   touching the word, so no partial correction is observable; no received
//!   word makes a decode panic.
//! - **Saving and loading** return a
//!   [`gf2_core::io::IoError`]; loading rejects a checksum, version, or
//!   field-identity mismatch.
//!
//! A panic after construction validation denotes a broken internal
//! invariant.
//!
//! # Decoder guarantees
//!
//! With $t$ the correction radius, a received word within distance $t$ of a
//! codeword decodes to that codeword, reported as
//! [`BchDecodeOutcome::NoErrors`] or [`BchDecodeOutcome::Corrected`]. Beyond
//! $t$ the decoder reports [`BchDecodeOutcome::Uncorrectable`] or corrects to
//! a different codeword within distance $t$ of the received word; every
//! accepted correction is verified by recomputing the syndrome, so the
//! outcome is sound about the word produced and never a claim about the
//! transmitted word. [`BinaryBchDecoder`'s guarantee](BinaryBchDecoder#guarantee)
//! is the authoritative statement. Codes over other base fields have
//! construction, encoding, and matrices; membership is checked through $H$.
//!
//! # Complexity
//!
//! With $r = n - k$, $t$ the correction radius, and $s$ the number of
//! syndrome evaluations:
//!
//! | Operation | Cost | Statement |
//! |---|---|---|
//! | construction | canonical-root search over $N = \lvert E^{*} \rvert$ candidates at $O(\log N)$ multiplications each, then the closure, minimal-polynomial, and LCM stages | [`spec`](spec#complexity) |
//! | encode one message, reference | $O(kr)$ field operations; $O(k \lceil r/64 \rceil)$ words packed | [`encode`](encode#complexity) |
//! | encode a batch | per family; see the module | [`encode`](encode#complexity) |
//! | generator matrix | $O(k \lceil n/64 \rceil)$ words packed; $O(kn)$ cells field-generic | [`matrix`](matrix#complexity) |
//! | parity-check matrix | $O(kr)$ coordinate writes over its $O(rn)$ output | [`matrix`](matrix#complexity) |
//! | decode one word | $O(sn + tn)$ field multiplications plus $O(r^2)$ for Berlekamp-Massey | [`BinaryBchDecoder::correct_in_place`] |
//!
//! # Performance-path selection
//!
//! - **Single messages.** [`BlockEncoder`](crate::traits::block::BlockEncoder)
//!   and [`BchCode::encode_systematic`] run the reference recurrence over
//!   registers the calling thread keeps; [`BchCode::encode_systematic_with`]
//!   takes a caller-owned [`BchCode::encode_workspace`] instead.
//! - **Batches.** [`BchCode::encode_batch`], [`BchCode::encode_batch_into`],
//!   and [`BchCode::encode_batch_parallel_into`] select one
//!   [`EncodeFamily`](encode::EncodeFamily) per batch: the first entry of
//!   [`EncodeFamily::REGISTERED`](encode::EncodeFamily::REGISTERED) that the
//!   representation implements for the plan and that the active tuning
//!   profile ([`crate::tuning`]) admits for the redundancy and batch length.
//!   The conservative profile, which a process resolves when it installs
//!   none through [`gf2_core::tuning::install`], admits only the reference.
//!   [`BchCode::selected_encode_family`] reports the selection.
//! - **Instruction sets.** The bit-sliced and carry-less-fold families run
//!   the accelerated kernels of [`gf2_kernels_simd::bch_encode`] when
//!   [`detect`](gf2_kernels_simd::bch_encode::detect) finds the processor
//!   features it names at run time, and that module's portable kernels
//!   otherwise.
//! - **Equivalence.** Every family, kernel, and worker count writes the same
//!   bits, so path selection changes speed and never output.
//! - **Matrices and decoding.** The `_into` matrix methods write caller
//!   buffers without allocating, and [`CachedMatrices`] retains
//!   materializations on request. [`BinaryBchDecoder::correct_in_place`]
//!   allocates nothing per word; under `--features hip`, batch syndrome
//!   evaluation runs on a HIP device with the same outcomes.
//!
//! # Examples
//!
//! ```
//! use gf2_coding::bch::encode::SystematicLayout;
//! use gf2_coding::bch::{BchDecodeOutcome, BinaryBchCode, BinaryBchDecoder, DesignedDistance};
//! use gf2_coding::traits::block::{BlockCode, BlockEncoder};
//! use gf2_core::field::extension::BinaryPrimeExt;
//! use gf2_core::gf2m::Gf2mField;
//! use gf2_core::BitVec;
//!
//! // BCH(15, 7): designed distance 5 and a witnessed run of four roots, so t = 2.
//! let extension = BinaryPrimeExt::new(Gf2mField::new(4, 0b10011))?;
//! let code = BinaryBchCode::primitive_narrow_sense(extension, DesignedDistance::try_from(5)?)?;
//! assert_eq!((code.n(), code.k()), (15, 7));
//!
//! let message = BitVec::ones(code.k());
//! let mut received = code.encode(&message)?;
//! received.set(2, !received.get(2));
//! received.set(11, !received.get(11));
//!
//! let decoder = BinaryBchDecoder::new(&code);
//! let mut workspace = decoder.workspace();
//! let outcome = decoder.correct_in_place(&mut received, &mut workspace)?;
//! assert_eq!(outcome, BchDecodeOutcome::Corrected { count: 2 });
//! assert_eq!(
//!     code.systematic_message(&received, SystematicLayout::default())?,
//!     message
//! );
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod core;
pub mod dvb_t2;
pub mod encode;
pub mod error;
pub mod matrix;
pub mod spec;
pub use encode::{LayoutView, SystematicLayout};

pub use core::{BchDecodeOutcome, BchDecodeReport, BchDecodeWorkspace, BinaryBchDecoder, CodeRate};
pub use matrix::{CachedMatrices, MatrixFill};
pub use spec::{
    BchCode, BchDistanceBound, BchLength, BchSpec, BinaryBchCode, DenseBchCode, DesignedDistance,
    RootExponent, RootSelection,
};
