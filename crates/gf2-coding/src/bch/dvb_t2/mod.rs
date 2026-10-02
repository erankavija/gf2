//! DVB-T2 BCH outer codes on the canonical construction model.
//!
//! ETSI EN 302 755 defines the outer BCH code of each FECFRAME as a shortened
//! primitive narrow-sense BCH code over the splitting field its frame size
//! selects. [`dvb_t2_bch_code`] builds that code from the standard's tables:
//! it constructs the mother code the standard's generator product describes
//! and removes the leading message coordinates that shortening drops, so the
//! result is a [`Shortened<DvbT2MotherCode>`](Shortened) carrying the
//! canonical block-code, encoding, and matrix capabilities.
//!
//! [`params`] holds the ETSI EN 302 755 Tables 6a and 6b parameters and
//! [`generators`] the standard's explicit minimal polynomials $g_1$ to
//! $g_{12}$, which the construction is checked against.
//!
//! # Frame types and rates
//!
//! Short frames use $\mathrm{GF}(2^{14})$ with $t = 12$; normal frames use
//! $\mathrm{GF}(2^{16})$ with $t = 12$ or $t = 10$. Both frame sizes carry
//! the six code rates of [`CodeRate`], so twelve configurations exist.
//!
//! # Declared coordinate layout
//!
//! The standard transmits the highest-degree coefficient first, in both the
//! message block and the parity block, so the constructor declares
//! [`SystematicLayout::MessageParityDescending`], named [`DVB_T2_LAYOUT`]:
//! user coordinate $u$ of the mother carries the coefficient of
//! $x^{n - 1 - u}$, and the shortened code's coordinate $d$ is a message bit
//! for $d < K_{bch}$ and a parity bit above it, as the standard's bit order
//! requires.
//!
//! [`DvbT2MotherCode`] is the canonical mother code in a [`LayoutView`] under
//! that declaration, so its encoder and both matrices describe the
//! standard's order.
//!
//! # Concatenation with LDPC
//!
//! The BCH codeword is the LDPC information block:
//!
//! ```text
//! BBFRAME (K_bch) -> BCH encode -> N_bch = K_ldpc -> LDPC encode -> N_ldpc
//! ```
//!
//! For the normal frame at rate 1/2 that reads 32208 -> 32400 -> 64800.
//! [`crate::ldpc::dvb_t2::concat::DvbT2Concat`] wires the two together.
//!
//! # Decoding through the mother
//!
//! [`DvbT2BchDecoder`] decodes a shortened word by writing it into a
//! mother-length buffer whose removed coordinates carry zero symbols and
//! running the canonical [`BinaryBchDecoder`] on that buffer. A zero symbol
//! at a removed coordinate is never in error, so the mother's
//! bounded-distance guarantee and its beyond-radius miscorrection semantics
//! carry over unchanged. Generic decoding of derived codes through their
//! coordinate maps is tracked as `@/issue/1a8f6acd`. This composition is the
//! DVB-T2 outer decoder, and `@/invariant/library-first-generality` names it
//! as the tracked exception.
//!
//! # Examples
//!
//! ```
//! use gf2_coding::bch::dvb_t2::{dvb_t2_bch_code, DvbT2BchDecoder, FrameSize};
//! use gf2_coding::bch::BchDecodeOutcome;
//! use gf2_coding::traits::block::{BlockCode, BlockEncoder};
//! use gf2_coding::CodeRate;
//! use gf2_core::BitVec;
//!
//! let code = dvb_t2_bch_code(FrameSize::Short, CodeRate::Rate1_2)?;
//! assert_eq!(code.n(), 7200); // N_bch, the LDPC information length
//! assert_eq!(code.k(), 7032); // K_bch, the BBFRAME length
//!
//! let bbframe = BitVec::random_seeded(code.k(), 0xAE03_BCD0);
//! let mut received = code.encode(&bbframe)?;
//! received.set(11, !received.get(11));
//!
//! let decoder = DvbT2BchDecoder::new(&code);
//! let (outcome, decoded) = decoder.decode(&received)?;
//! assert_eq!(outcome, BchDecodeOutcome::Corrected { count: 1 });
//! assert_eq!(decoded, bbframe);
//! # Ok::<(), gf2_coding::bch::error::BchError>(())
//! ```

pub mod generators;
pub mod params;

pub use params::{DvbBchParams, FrameSize};

use gf2_core::field::extension::BinaryPrimeExt;
use gf2_core::gf2m::Gf2mField;
use gf2_core::gfp::Fp;
use gf2_core::{BitMatrix, BitVec};

use crate::bch::encode::{LayoutView, SystematicLayout, SystematicPlan};
use crate::bch::error::BchError;
use crate::bch::spec::{BinaryBchCode, DesignedDistance};
use crate::bch::{BchDecodeOutcome, BchDecodeWorkspace, BinaryBchDecoder};
use crate::error::CodeError;
use crate::traits::block::BlockCode;
use crate::transform::{CoordinateMap, Shortened};
use crate::CodeRate;

/// The systematic user layout the DVB-T2 outer BCH code declares.
///
/// ETSI EN 302 755 writes the message polynomial with its first information
/// bit at the highest degree and appends the remainder in the same order, so
/// the standard's transmission order is
/// [`SystematicLayout::MessageParityDescending`].
pub const DVB_T2_LAYOUT: SystematicLayout = SystematicLayout::MessageParityDescending;

/// The DVB-T2 outer BCH mother code presented in the standard's transmission
/// order: the canonical primitive narrow-sense binary BCH code over the frame
/// size's splitting field, with designed distance $2t + 1$, under
/// [`DVB_T2_LAYOUT`].
pub type DvbT2MotherCode = LayoutView<BinaryPrimeExt, BitVec, BitMatrix>;

/// The DVB-T2 outer BCH code: the mother code of its frame size shortened on
/// its leading message coordinates.
pub type DvbT2BchCode = Shortened<DvbT2MotherCode>;

/// Builds the mother code the configuration of `params` shortens.
///
/// # Errors
///
/// Returns [`BchError::Field`] when the table's polynomial does not present
/// the extension field, [`BchError::InvalidDesignedDistance`] for a zero
/// correction radius, and the construction errors of
/// [`BinaryBchCode::primitive_narrow_sense`].
fn mother_code(params: DvbBchParams) -> Result<DvbT2MotherCode, BchError> {
    let field = Gf2mField::new(params.field_m, params.primitive_poly).with_tables();
    let extension = BinaryPrimeExt::new(field)?;
    let designed_distance = DesignedDistance::try_from(2 * params.t as u64 + 1)?;
    let code = BinaryBchCode::primitive_narrow_sense(extension, designed_distance)?;
    Ok(LayoutView::new(code, DVB_T2_LAYOUT))
}

/// Builds the DVB-T2 outer BCH code for one frame size and code rate.
///
/// The construction reads [`DvbBchParams`] for the splitting field, the
/// correction radius, and $K_{bch}$, builds the mother code, and removes the
/// leading $K_{mother} - K_{bch}$ message coordinates. The mother carries its
/// message symbols in coordinates $0$ to $k$, so the result takes
/// [`ShortenedDerivation::SystematicRestriction`](crate::transform::ShortenedDerivation::SystematicRestriction)
/// and materializes no generator.
///
/// # Errors
///
/// Returns [`BchError::Field`] when the table's polynomial does not present
/// the extension field, [`BchError::InvalidDesignedDistance`] for a zero
/// correction radius, the construction errors of
/// [`BinaryBchCode::primitive_narrow_sense`], and [`BchError::Code`] when the
/// table's $K_{bch}$ exceeds the mother dimension.
///
/// # Complexity
///
/// One mother construction plus $O(n)$ for the shortening.
pub fn dvb_t2_bch_code(frame_size: FrameSize, rate: CodeRate) -> Result<DvbT2BchCode, BchError> {
    let params = DvbBchParams::for_code(frame_size, rate);
    let mother = mother_code(params)?;
    let dimension = BlockCode::k(&mother);
    let shortening = dimension
        .checked_sub(params.k)
        .ok_or(CodeError::CoordinateCountMismatch {
            expected: params.k,
            actual: dimension,
        })?;
    Ok(Shortened::shorten_first(mother, shortening)?)
}

// ---------------------------------------------------------------------------
// The outer decoder
// ---------------------------------------------------------------------------

/// Reusable scratch for one [`DvbT2BchDecoder`] call.
///
/// Build one with [`DvbT2BchDecoder::workspace`] and pass the same value to
/// every call: the mother-length word and the mother decoder's own buffers
/// are sized here, so the per-word path reaches no allocator.
#[derive(Clone, Debug)]
pub struct DvbT2DecodeWorkspace {
    /// The mother-length word the canonical decoder runs on.
    word: BitVec,
    /// The canonical decoder's own scratch.
    inner: BchDecodeWorkspace,
}

/// The DVB-T2 outer BCH decoder.
///
/// A shortened word is decoded through the mother code: the decoder writes
/// the received coordinates into a mother-length buffer whose removed
/// coordinates carry zero symbols, runs [`BinaryBchDecoder`] on that buffer,
/// and reads the result back through the shortened code's coordinate map. A
/// zero symbol at a removed coordinate is never in error, so the mother's
/// guarantees apply to the shortened word unchanged: an outcome of
/// [`BchDecodeOutcome::Corrected`] carries a verified correction, and
/// [`BchDecodeOutcome::Uncorrectable`] states that the bounded-distance
/// procedure found none.
///
/// A received word farther than the correction radius from every codeword
/// can still produce a verified correction to a different codeword, the
/// miscorrection the mother decoder documents. Such a correction can fall on
/// a removed coordinate, which the shortened word has no room for; the
/// corrected shortened word is then the mother word restricted to the kept
/// coordinates.
///
/// Generic decoding through a derived code's coordinate map is tracked as
/// `@/issue/1a8f6acd`.
#[derive(Clone, Debug)]
pub struct DvbT2BchDecoder<'code> {
    code: &'code DvbT2BchCode,
    mother: BinaryBchDecoder<'code>,
}

impl<'code> DvbT2BchDecoder<'code> {
    /// Builds a decoder for `code`.
    ///
    /// # Complexity
    ///
    /// That of [`BinaryBchDecoder::new`] on the mother code.
    pub fn new(code: &'code DvbT2BchCode) -> Self {
        Self {
            code,
            mother: BinaryBchDecoder::new(code.mother().code()),
        }
    }

    /// Returns the code this decoder was built for.
    pub fn code(&self) -> &'code DvbT2BchCode {
        self.code
    }

    /// Returns the correction radius the mother's witnessed bound implies.
    pub fn correction_radius(&self) -> usize {
        self.mother.correction_radius()
    }

    /// Allocates the scratch one decode needs.
    ///
    /// # Complexity
    ///
    /// One mother-length bit buffer plus the allocations of
    /// [`BinaryBchDecoder::workspace`].
    pub fn workspace(&self) -> DvbT2DecodeWorkspace {
        DvbT2DecodeWorkspace {
            word: BitVec::zeros(BlockCode::n(self.code.mother())),
            inner: self.mother.workspace(),
        }
    }

    /// Corrects `received` in place and reports the outcome.
    ///
    /// `received` is a codeword-length word in the standard's transmission
    /// order. It is left untouched unless the call returns
    /// [`BchDecodeOutcome::Corrected`]. The call performs no heap allocation.
    ///
    /// # Errors
    ///
    /// [`BchError::Decode`] when `received` is not `code().n()` coordinates
    /// long, and the workspace and buffer errors of
    /// [`BinaryBchDecoder::correct_in_place`].
    ///
    /// # Complexity
    ///
    /// $O(n)$ coordinate moves plus one mother decode.
    pub fn correct_in_place(
        &self,
        received: &mut BitVec,
        workspace: &mut DvbT2DecodeWorkspace,
    ) -> Result<BchDecodeOutcome, BchError> {
        let outcome = self.correct_lifted(received, workspace)?;
        if matches!(outcome, BchDecodeOutcome::Corrected { .. }) {
            let map = self.code.coordinate_map();
            let plan = self.code.mother().plan();
            for derived in 0..BlockCode::n(self.code) {
                let internal = internal_of(map, &plan, derived)?;
                received.set(derived, workspace.word.get(internal));
            }
        }
        Ok(outcome)
    }

    /// Decodes `received` into `bbframe` and reports the outcome.
    ///
    /// `bbframe` receives the message coordinates of the corrected word, and
    /// the coordinates of `received` itself when the outcome is
    /// [`BchDecodeOutcome::Uncorrectable`]. The call performs no heap
    /// allocation.
    ///
    /// # Errors
    ///
    /// [`BchError::Decode`] when `received` is not `code().n()` coordinates
    /// long or `bbframe` is not `code().k()` coordinates long, and the
    /// workspace and buffer errors of
    /// [`BinaryBchDecoder::correct_in_place`].
    ///
    /// # Complexity
    ///
    /// $O(n)$ coordinate moves plus one mother decode.
    pub fn decode_into(
        &self,
        received: &BitVec,
        bbframe: &mut BitVec,
        workspace: &mut DvbT2DecodeWorkspace,
    ) -> Result<BchDecodeOutcome, BchError> {
        let dimension = BlockCode::k(self.code);
        if bbframe.len() != dimension {
            return Err(BchError::Decode(CodeError::BufferLengthMismatch {
                expected: dimension,
                actual: bbframe.len(),
            }));
        }

        let outcome = self.correct_lifted(received, workspace)?;
        let map = self.code.coordinate_map();
        let plan = self.code.mother().plan();
        for derived in 0..dimension {
            let internal = internal_of(map, &plan, derived)?;
            bbframe.set(derived, workspace.word.get(internal));
        }
        Ok(outcome)
    }

    /// Decodes `received` into a newly allocated BBFRAME.
    ///
    /// This is the allocating convenience: it builds its own workspace and
    /// output, so a repeated decode should use
    /// [`decode_into`](Self::decode_into) instead.
    ///
    /// # Errors
    ///
    /// Those of [`decode_into`](Self::decode_into).
    pub fn decode(&self, received: &BitVec) -> Result<(BchDecodeOutcome, BitVec), BchError> {
        let mut workspace = self.workspace();
        let mut bbframe = BitVec::zeros(BlockCode::k(self.code));
        let outcome = self.decode_into(received, &mut bbframe, &mut workspace)?;
        Ok((outcome, bbframe))
    }

    /// Writes `received` into the workspace's mother-length word and corrects
    /// it there.
    fn correct_lifted(
        &self,
        received: &BitVec,
        workspace: &mut DvbT2DecodeWorkspace,
    ) -> Result<BchDecodeOutcome, BchError> {
        let length = BlockCode::n(self.code);
        if received.len() != length {
            return Err(BchError::Decode(CodeError::BufferLengthMismatch {
                expected: length,
                actual: received.len(),
            }));
        }

        // Every removed coordinate carries a zero symbol, so the word is
        // rebuilt from zero rather than from whatever the previous call left.
        let mother_length = BlockCode::n(self.code.mother());
        workspace.word.clear();
        workspace.word.resize(mother_length, false);

        let map = self.code.coordinate_map();
        let plan = self.code.mother().plan();
        for derived in 0..length {
            let internal = internal_of(map, &plan, derived)?;
            workspace.word.set(internal, received.get(derived));
        }

        self.mother
            .correct_in_place(&mut workspace.word, &mut workspace.inner)
    }
}

/// Returns the mother internal coordinate carrying derived position
/// `derived` of a shortened code with map `map` under plan `plan`.
fn internal_of(
    map: &CoordinateMap,
    plan: &SystematicPlan<'_, Fp<2>>,
    derived: usize,
) -> Result<usize, BchError> {
    let user = map.mother_position(derived).map_err(BchError::Decode)?;
    plan.internal_coordinate(user).map_err(BchError::Decode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::block::BlockEncoder;
    use crate::transform::ShortenedDerivation;
    use gf2_core::field::FiniteField;
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};
    use std::collections::BTreeSet;

    /// Seed of the payloads and error patterns these cases draw.
    const SEED: u64 = 0xAE03_BCD0;

    /// The twelve configurations ETSI EN 302 755 Tables 6a and 6b define.
    fn configurations() -> Vec<(FrameSize, CodeRate)> {
        let mut configurations = Vec::with_capacity(12);
        for frame_size in [FrameSize::Short, FrameSize::Normal] {
            for rate in [
                CodeRate::Rate1_2,
                CodeRate::Rate3_5,
                CodeRate::Rate2_3,
                CodeRate::Rate3_4,
                CodeRate::Rate4_5,
                CodeRate::Rate5_6,
            ] {
                configurations.push((frame_size, rate));
            }
        }
        configurations
    }

    /// Returns `count` distinct coordinates below `length`, drawn from `seed`.
    fn error_positions(length: usize, count: usize, seed: u64) -> Vec<usize> {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut positions = BTreeSet::new();
        while positions.len() < count {
            positions.insert(rng.gen_range(0..length));
        }
        positions.into_iter().collect()
    }

    #[test]
    fn every_configuration_takes_the_systematic_restriction() {
        for (frame_size, rate) in configurations() {
            let params = DvbBchParams::for_code(frame_size, rate);
            let code = dvb_t2_bch_code(frame_size, rate).expect("a standard configuration");

            assert_eq!(code.n(), params.n, "N_bch for {frame_size:?} {rate:?}");
            assert_eq!(code.k(), params.k, "K_bch for {frame_size:?} {rate:?}");
            assert_eq!(
                code.derivation(),
                ShortenedDerivation::SystematicRestriction,
                "derivation for {frame_size:?} {rate:?}"
            );
            assert_eq!(
                code.mother().code().correction_radius(),
                params.t,
                "correction radius for {frame_size:?} {rate:?}"
            );
            assert_eq!(
                code.information_set(),
                (0..params.k).collect::<Vec<_>>(),
                "information set for {frame_size:?} {rate:?}"
            );
        }
    }

    #[test]
    fn every_mother_generator_is_the_standard_product() {
        for (frame_size, rate) in configurations() {
            let params = DvbBchParams::for_code(frame_size, rate);
            let mother = mother_code(params).expect("a standard configuration");

            let table = match frame_size {
                FrameSize::Short => generators::SHORT_GENERATORS,
                FrameSize::Normal => generators::NORMAL_GENERATORS,
            };
            let field = Gf2mField::new(params.field_m, params.primitive_poly);
            let expected = generators::product_of_generators(&field, table, params.t);
            let degree = expected.degree().expect("a nonzero generator product");
            assert_eq!(
                degree, params.m,
                "the standard's generator degree is the parity length for {frame_size:?} {rate:?}"
            );

            let canonical = mother.code().generator();
            assert_eq!(
                canonical.degree(),
                Some(degree),
                "canonical generator degree for {frame_size:?} {rate:?}"
            );
            for index in 0..=degree {
                assert_eq!(
                    canonical.coeff(index).is_one(),
                    expected.coeff_or_zero(index, &field.zero()).is_one(),
                    "generator coefficient {index} for {frame_size:?} {rate:?}"
                );
            }
        }
    }

    #[test]
    fn the_declared_layout_reverses_the_mother_coordinates() {
        for (frame_size, rate) in [
            (FrameSize::Short, CodeRate::Rate1_2),
            (FrameSize::Normal, CodeRate::Rate5_6),
        ] {
            let params = DvbBchParams::for_code(frame_size, rate);
            let code = dvb_t2_bch_code(frame_size, rate).expect("a standard configuration");
            let mother = code.mother();
            assert_eq!(mother.layout(), DVB_T2_LAYOUT);
            let plan = mother.plan();
            let length = BlockCode::n(mother);

            // User coordinate u of the mother carries the coefficient of
            // x^{n - 1 - u}, so the kept coordinates of the shortened code are
            // the mother's low degrees, in descending order.
            for user in [0, 1, length / 2, length - 1] {
                assert_eq!(
                    plan.internal_coordinate(user)
                        .expect("an in-range user coordinate"),
                    length - 1 - user,
                    "user coordinate {user} of {frame_size:?} {rate:?}"
                );
            }
            assert!(matches!(
                plan.internal_coordinate(length),
                Err(CodeError::CoordinateOutOfRange { .. })
            ));

            let map = code.coordinate_map();
            for derived in [0, params.k - 1, params.k, params.n - 1] {
                let user = map.mother_position(derived).expect("a kept coordinate");
                assert_eq!(
                    plan.internal_coordinate(user)
                        .expect("an in-range user coordinate"),
                    params.n - 1 - derived,
                    "derived coordinate {derived} of {frame_size:?} {rate:?}"
                );
            }
        }
    }

    #[test]
    fn the_decoder_reports_no_errors_and_corrects_within_the_radius() {
        for (frame_size, rate) in [
            (FrameSize::Short, CodeRate::Rate1_2),
            (FrameSize::Normal, CodeRate::Rate2_3),
        ] {
            let params = DvbBchParams::for_code(frame_size, rate);
            let code = dvb_t2_bch_code(frame_size, rate).expect("a standard configuration");
            let decoder = DvbT2BchDecoder::new(&code);
            let mut workspace = decoder.workspace();

            let payload = BitVec::random_seeded(params.k, SEED);
            let codeword = code.encode(&payload).expect("a payload encodes");

            let (outcome, decoded) = decoder.decode(&codeword).expect("a codeword decodes");
            assert_eq!(
                outcome,
                BchDecodeOutcome::NoErrors,
                "{frame_size:?} {rate:?}"
            );
            assert_eq!(decoded, payload);

            for count in [1, params.t] {
                let mut received = codeword.clone();
                for position in error_positions(params.n, count, SEED + count as u64) {
                    received.set(position, !received.get(position));
                }

                let mut bbframe = BitVec::zeros(params.k);
                let outcome = decoder
                    .decode_into(&received, &mut bbframe, &mut workspace)
                    .expect("a codeword-length word decodes");
                assert_eq!(
                    outcome,
                    BchDecodeOutcome::Corrected { count },
                    "{count} errors in {frame_size:?} {rate:?}"
                );
                assert_eq!(
                    bbframe, payload,
                    "{count} errors in {frame_size:?} {rate:?}"
                );

                let mut corrected = received.clone();
                let repeated = decoder
                    .correct_in_place(&mut corrected, &mut workspace)
                    .expect("a codeword-length word corrects");
                assert_eq!(repeated, BchDecodeOutcome::Corrected { count });
                assert_eq!(corrected, codeword, "the corrected word is the codeword");
            }
        }
    }

    #[test]
    fn the_decoder_reports_beyond_radius_words_as_uncorrectable_or_miscorrected() {
        for (frame_size, rate) in [
            (FrameSize::Short, CodeRate::Rate1_2),
            (FrameSize::Normal, CodeRate::Rate2_3),
        ] {
            let params = DvbBchParams::for_code(frame_size, rate);
            let code = dvb_t2_bch_code(frame_size, rate).expect("a standard configuration");
            let decoder = DvbT2BchDecoder::new(&code);

            let payload = BitVec::random_seeded(params.k, SEED);
            let codeword = code.encode(&payload).expect("a payload encodes");

            // A word at distance t + 1 from a codeword is below the designed
            // distance 2t + 1 from every codeword, so it is not itself one and
            // no verified correction can reach the sent word inside the radius.
            let mut received = codeword.clone();
            for position in error_positions(params.n, params.t + 1, SEED) {
                received.set(position, !received.get(position));
            }
            let (outcome, decoded) = decoder.decode(&received).expect("a word decodes");
            assert_ne!(
                outcome,
                BchDecodeOutcome::NoErrors,
                "a word one beyond the radius is not a codeword"
            );
            if outcome != BchDecodeOutcome::Uncorrectable {
                assert_ne!(
                    decoded, payload,
                    "a verified correction beyond the radius reaches another codeword"
                );
            }

            // A dense error pattern leaves the bounded-distance procedure no
            // verified correction to report.
            let mut dense = codeword.clone();
            for position in error_positions(params.n, 2 * params.t, SEED + 1) {
                dense.set(position, !dense.get(position));
            }
            assert_eq!(
                decoder.decode(&dense).expect("a word decodes").0,
                BchDecodeOutcome::Uncorrectable,
                "a 2t-error pattern in {frame_size:?} {rate:?}"
            );
        }
    }

    #[test]
    fn wrong_length_buffers_are_typed_errors() {
        let code = dvb_t2_bch_code(FrameSize::Short, CodeRate::Rate1_2).expect("a configuration");
        let decoder = DvbT2BchDecoder::new(&code);
        let mut workspace = decoder.workspace();

        let short_word = BitVec::zeros(code.n() - 1);
        assert!(matches!(
            decoder.decode(&short_word),
            Err(BchError::Decode(CodeError::BufferLengthMismatch { .. }))
        ));

        let word = BitVec::zeros(code.n());
        let mut wrong = BitVec::zeros(code.k() + 1);
        assert!(matches!(
            decoder.decode_into(&word, &mut wrong, &mut workspace),
            Err(BchError::Decode(CodeError::BufferLengthMismatch { .. }))
        ));
    }
}
