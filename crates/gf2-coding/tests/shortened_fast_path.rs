//! Shortening BCH mother codes through both derivations of `Shortened<C>`.
//!
//! `Shortened<C>` derives its code either by restricting a systematic mother
//! to the messages that vanish on the removed message positions, or by
//! reducing a basis of the mother codewords that vanish on the removed
//! coordinates. The wrapper's own unit suite covers the selection rule and
//! the boundary codes on generic linear fixtures; this suite runs the rule
//! against BCH mothers, where the systematic restriction is what makes the
//! DVB-T2 rows constructible at all.
//!
//! `gf2_coding::test_support::RankDerivedMother` reports no systematic
//! layout and delegates everything else, so one mother yields both
//! derivations of one code and the suite compares them directly.
//!
//! The nonbinary rows are the conformance corpus rows N1, N2 and N3 that
//! `gf2_coding::test_support::visit_bch_corpus` builds. They are constructed
//! here rather than visited because the visitor hands its rows out under
//! `SymbolMatrix`, while shortening needs the `MatrixFill` materialization
//! contract.

use gf2_coding::bch::spec::{
    BchLength, BchSpec, BinaryBchCode, DenseBchCode, DesignedDistance, RootExponent,
};
use gf2_coding::test_support::{bch_corpus_element, RankDerivedMother, BCH_CORPUS_SEED};
use gf2_coding::traits::block::{
    BlockCode, BlockEncoder, GeneratorMatrixAccess, ParityCheckMatrixAccess, SymbolSequence,
};
use gf2_coding::transform::{Shortened, ShortenedDerivation};
use gf2_coding::LinearBlockCode;
use gf2_core::field::extension::{BinaryPrimeExt, FieldExtension, FieldIdentity};
use gf2_core::field::modulus_select::select_modulus;
use gf2_core::field::ConstField;
use gf2_core::gf2m::Gf2mField;
use gf2_core::gfp::Fp;
use gf2_core::gfpn::{QuotientElement, QuotientField};
use gf2_core::{BitMatrix, BitVec};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::BTreeSet;

/// The ETSI DVB-T2 normal-frame field polynomial and correction radius, as
/// the standard's BCH clause fixes them.
const DVB_T2_FIELD_DEGREE: usize = 16;
const DVB_T2_FIELD_POLYNOMIAL: u64 = 0b1_0000_0000_0010_1101;
const DVB_T2_DESIGNED_DISTANCE: u64 = 25;

/// Leading message coordinates the normal-frame rate-1/2 row removes.
const DVB_T2_SHORTENING: usize = 33135;

/// Packed word boundaries every bit-packed length assertion covers.
const PACKED_WORD_BOUNDARIES: [usize; 5] = [0, 1, 63, 64, 65];

/// Builds the DVB-T2 normal-frame mother code on the canonical BCH model.
fn dvb_t2_normal_mother() -> BinaryBchCode {
    let extension =
        BinaryPrimeExt::new(Gf2mField::new(DVB_T2_FIELD_DEGREE, DVB_T2_FIELD_POLYNOMIAL))
            .expect("the ETSI normal-frame polynomial is primitive");
    BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
        extension,
        designed_distance: DesignedDistance::try_from(DVB_T2_DESIGNED_DISTANCE)
            .expect("a positive designed distance"),
    })
    .expect("the DVB-T2 normal-frame mother code")
}

/// Builds a binary primitive narrow-sense mother of the given extension
/// degree and designed distance.
fn binary_mother(degree: usize, designed_distance: u64) -> BinaryBchCode {
    BinaryBchCode::<u64>::primitive_narrow_sense_auto(
        Fp::<2>::zero(),
        degree,
        DesignedDistance::try_from(designed_distance).expect("a positive designed distance"),
    )
    .expect("a binary primitive narrow-sense mother")
}

/// Returns the number of elements in the field `zero` witnesses.
fn field_order<F: FieldIdentity>(zero: &F) -> u128 {
    let identity = zero.field_id();
    u128::from(identity.characteristic()).pow(identity.degree() as u32)
}

/// Draws a seeded message of `length` symbols from the field `zero`
/// witnesses.
///
/// Symbols are drawn as canonical field indices, the numbering
/// [`bch_corpus_element`] inverts, from the repository's standard seeded
/// test generator.
fn seeded_message<S, F>(rng: &mut StdRng, length: usize, zero: &F) -> S
where
    F: FieldIdentity,
    S: SymbolSequence<F>,
{
    let order = field_order(zero);
    let mut message = S::zeroed(length, zero);
    for index in 0..length {
        let symbol = bch_corpus_element(zero, rng.gen_range(0..order));
        message
            .set(index, symbol)
            .expect("an index below the drawn length");
    }
    message
}

/// Shortens `mother` on `coordinates` through both derivations and asserts
/// that they agree on every observable the transformation contract exposes.
fn assert_derivations_agree<C>(mother: C, coordinates: &[usize])
where
    C: BlockEncoder + GeneratorMatrixAccess + ParityCheckMatrixAccess + Clone,
{
    let fast = Shortened::new(mother.clone(), coordinates.iter().copied())
        .expect("a valid coordinate set");
    let rank = Shortened::new(RankDerivedMother(mother), coordinates.iter().copied())
        .expect("a valid coordinate set");

    assert_eq!(
        fast.derivation(),
        ShortenedDerivation::SystematicRestriction
    );
    assert_eq!(rank.derivation(), ShortenedDerivation::RankDerived);
    assert_eq!((fast.k(), fast.n()), (rank.k(), rank.n()));
    assert_eq!(fast.information_set(), rank.information_set());
    assert_eq!(fast.shortened_positions(), rank.shortened_positions());
    assert_eq!(fast.is_systematic().unwrap(), rank.is_systematic().unwrap());
    assert_eq!(
        fast.generator_matrix().unwrap(),
        rank.generator_matrix().unwrap()
    );
    assert_eq!(fast.parity_check_rows(), rank.parity_check_rows());
    assert_eq!(
        fast.parity_check_matrix().unwrap(),
        rank.parity_check_matrix().unwrap()
    );
    for position in 0..fast.n() {
        assert_eq!(
            fast.coordinate_map().mother_position_opt(position),
            rank.coordinate_map().mother_position_opt(position),
            "coordinate {position}"
        );
    }

    let zero = fast.symbol_zero();
    let mut rng = StdRng::seed_from_u64(BCH_CORPUS_SEED);
    for round in 0..2 {
        let message: C::Symbols = seeded_message(&mut rng, fast.k(), &zero);
        let word = fast.encode(&message).expect("a shortened encode");
        assert_eq!(word, rank.encode(&message).expect("a rank-derived encode"));
        assert_eq!(
            fast.extend_codeword(&word).unwrap(),
            rank.extend_codeword(&word).unwrap(),
            "lift in round {round}"
        );
    }
}

/// Asserts that a shortened binary codeword carries the derived message in
/// its information coordinates, agrees with the mother codeword of the
/// zero-extended message on every kept coordinate, and leaves its packed
/// tail zero.
fn assert_packed_shortening_contract<C>(mother: &C, shortened: &Shortened<C>)
where
    C: BlockCode<Symbol = Fp<2>, Symbols = BitVec> + BlockEncoder + GeneratorMatrixAccess,
{
    let mut rng = StdRng::seed_from_u64(BCH_CORPUS_SEED);
    let message: BitVec = seeded_message(&mut rng, shortened.k(), &Fp::<2>::zero());
    let word = shortened.encode(&message).expect("a shortened encode");

    assert_eq!(word.len(), shortened.n());
    let tail = word.len() & 63;
    if tail != 0 {
        let last = word.words()[word.len() >> 6];
        assert_eq!(last >> tail, 0, "packed tail beyond bit {tail}");
    }
    for (message_index, &coordinate) in shortened.information_set().iter().enumerate() {
        assert_eq!(word.get(coordinate), message.get(message_index));
    }

    let mut mother_message = BitVec::zeros(mother.k());
    for (derived_index, &coordinate) in shortened.information_set().iter().enumerate() {
        let mother_coordinate = shortened
            .coordinate_map()
            .mother_position(coordinate)
            .expect("a kept coordinate");
        mother_message.set(mother_coordinate, message.get(derived_index));
    }
    let mother_word = mother.encode(&mother_message).expect("a mother encode");
    for &removed in shortened.shortened_positions() {
        assert!(!mother_word.get(removed), "mother coordinate {removed}");
    }
    for position in 0..shortened.n() {
        let mother_position = shortened
            .coordinate_map()
            .mother_position(position)
            .expect("a kept coordinate");
        assert_eq!(
            word.get(position),
            mother_word.get(mother_position),
            "coordinate {position}"
        );
    }
}

#[test]
fn the_dvb_t2_normal_frame_row_is_a_systematic_restriction() {
    let mother = dvb_t2_normal_mother();
    let shortened = Shortened::shorten_first(mother.clone(), DVB_T2_SHORTENING)
        .expect("a shortening below the mother dimension");

    assert_eq!(
        shortened.derivation(),
        ShortenedDerivation::SystematicRestriction
    );
    assert_eq!(shortened.n(), mother.n() - DVB_T2_SHORTENING);
    assert_eq!(shortened.k(), mother.k() - DVB_T2_SHORTENING);
    assert_eq!((shortened.n(), shortened.k()), (32400, 32208));
    assert!(shortened.is_systematic().unwrap());
    assert!(shortened
        .information_set()
        .iter()
        .copied()
        .eq(0..shortened.k()));
    assert_eq!(shortened.parity_check_rows(), mother.n() - mother.k());

    assert_packed_shortening_contract(&mother, &shortened);
}

#[test]
fn shortened_dvb_t2_encodes_agree_with_the_zero_prefixed_mother_message() {
    let mother = dvb_t2_normal_mother();
    let shortened = Shortened::shorten_first(mother.clone(), DVB_T2_SHORTENING)
        .expect("a shortening below the mother dimension");

    let mut rng = StdRng::seed_from_u64(BCH_CORPUS_SEED);
    for round in 0..3 {
        let message: BitVec = seeded_message(&mut rng, shortened.k(), &Fp::<2>::zero());
        let word = shortened.encode(&message).expect("a shortened encode");

        let mut mother_message = BitVec::zeros(mother.k());
        for index in 0..shortened.k() {
            mother_message.set(DVB_T2_SHORTENING + index, message.get(index));
        }
        let mother_word = mother.encode(&mother_message).expect("a mother encode");

        let mut expected = BitVec::zeros(shortened.n());
        for position in 0..shortened.n() {
            expected.set(position, mother_word.get(DVB_T2_SHORTENING + position));
        }
        assert_eq!(word, expected, "round {round}");
        for removed in 0..DVB_T2_SHORTENING {
            assert!(!mother_word.get(removed), "round {round}, prefix {removed}");
        }
    }
}

/// Builds the full-space mother of `length` coordinates, whose generator is
/// the identity and whose dimension equals its length.
fn full_space_mother(length: usize) -> LinearBlockCode {
    let mut generator = BitMatrix::zeros(length, length);
    for index in 0..length {
        generator.set(index, index, true);
    }
    LinearBlockCode::new_systematic(generator, None)
}

#[test]
fn packed_word_boundary_shortened_lengths_keep_canonical_indexing() {
    let mut dimensions = BTreeSet::new();
    let mut lengths = BTreeSet::new();

    // A full-space mother reaches every boundary in both its dimension and
    // its length, including the zero-length and single-coordinate ones that
    // a mother with parity coordinates cannot reach by shortening message
    // positions alone.
    let mother = full_space_mother(*PACKED_WORD_BOUNDARIES.iter().max().unwrap());
    for &boundary in &PACKED_WORD_BOUNDARIES {
        let shortened = Shortened::shorten_first(mother.clone(), mother.k() - boundary)
            .expect("a shortening below the mother dimension");
        assert_eq!(
            shortened.derivation(),
            ShortenedDerivation::SystematicRestriction
        );
        assert_eq!((shortened.k(), shortened.n()), (boundary, boundary));
        assert_packed_shortening_contract(&mother, &shortened);
        dimensions.insert(boundary);
        lengths.insert(boundary);
    }

    for (degree, designed_distance) in [(7usize, 21u64), (8, 9)] {
        let mother = binary_mother(degree, designed_distance);
        for &boundary in &PACKED_WORD_BOUNDARIES {
            if boundary <= mother.k() {
                let shortened = Shortened::shorten_first(mother.clone(), mother.k() - boundary)
                    .expect("a shortening below the mother dimension");
                assert_eq!(
                    shortened.derivation(),
                    ShortenedDerivation::SystematicRestriction
                );
                assert_eq!(shortened.k(), boundary);
                assert_packed_shortening_contract(&mother, &shortened);
                dimensions.insert(boundary);
            }
            if boundary <= mother.n() && mother.n() - boundary <= mother.k() {
                let shortened = Shortened::shorten_first(mother.clone(), mother.n() - boundary)
                    .expect("a shortening below the mother dimension");
                assert_eq!(
                    shortened.derivation(),
                    ShortenedDerivation::SystematicRestriction
                );
                assert_eq!(shortened.n(), boundary);
                assert_packed_shortening_contract(&mother, &shortened);
                lengths.insert(boundary);
            }
        }
    }

    let expected = PACKED_WORD_BOUNDARIES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(dimensions, expected, "shortened dimensions covered");
    assert_eq!(lengths, expected, "shortened lengths covered");
}

#[test]
fn both_derivations_agree_on_binary_bch_mothers() {
    for (degree, designed_distance) in [(4usize, 7u64), (7, 21)] {
        let mother = binary_mother(degree, designed_distance);
        let sets = [
            vec![],
            vec![0],
            vec![0, 1, mother.k() - 1],
            (0..mother.k()).collect::<Vec<_>>(),
        ];
        for coordinates in sets {
            assert_derivations_agree(mother.clone(), &coordinates);
        }
    }
}

#[test]
fn both_derivations_agree_on_a_gf3_bch_mother() {
    let mother = DenseBchCode::<QuotientField<Fp<3>>>::consecutive_roots_auto(
        Fp::<3>::zero(),
        3,
        BchLength::try_from(13).expect("a positive length"),
        RootExponent::from(1),
        DesignedDistance::try_from(5).expect("a positive designed distance"),
    )
    .expect("the corpus GF(3) row");
    for coordinates in [
        vec![],
        vec![0],
        vec![0, 2],
        (0..mother.k()).collect::<Vec<_>>(),
    ] {
        assert_derivations_agree(mother.clone(), &coordinates);
    }
}

#[test]
fn both_derivations_agree_on_a_gf5_bch_mother() {
    let mother = DenseBchCode::<QuotientField<Fp<5>>>::consecutive_roots_auto(
        Fp::<5>::zero(),
        3,
        BchLength::try_from(31).expect("a positive length"),
        RootExponent::from(1),
        DesignedDistance::try_from(4).expect("a positive designed distance"),
    )
    .expect("the corpus GF(5) row");
    for coordinates in [vec![], vec![1], vec![0, 3, 4]] {
        assert_derivations_agree(mother.clone(), &coordinates);
    }
}

#[test]
fn both_derivations_agree_on_a_gf9_bch_mother() {
    let modulus = select_modulus(&Fp::<3>::zero(), 2).expect("a GF(9) modulus");
    let gf9 = QuotientField::new(Fp::<3>::zero(), modulus).expect("GF(9)");
    let mother = DenseBchCode::<QuotientField<QuotientElement<Fp<3>>>>::consecutive_roots_auto(
        gf9.ext_zero(),
        2,
        BchLength::try_from(10).expect("a positive length"),
        RootExponent::from(1),
        DesignedDistance::try_from(3).expect("a positive designed distance"),
    )
    .expect("the corpus GF(9) row");
    for coordinates in [vec![], vec![0], vec![1, 2]] {
        assert_derivations_agree(mother.clone(), &coordinates);
    }
}

/// Materializes the shortened DVB-T2 matrices and checks the systematic
/// prefix and orthogonality on sampled rows.
///
/// Writing the derived generator materializes the mother's first, so the
/// call holds both at once. That memory, not its wall time, is why this sits
/// in the nightly tier rather than beside the construction witness above.
#[test]
#[ignore = "slow: holds the 65343 x 65535 DVB-T2 mother generator and the \
            32208 x 32400 derived one at once"]
fn the_shortened_dvb_t2_matrices_are_systematic_and_orthogonal() {
    let mother = dvb_t2_normal_mother();
    let shortened = Shortened::shorten_first(mother.clone(), DVB_T2_SHORTENING)
        .expect("a shortening below the mother dimension");

    let check = shortened
        .parity_check_matrix()
        .expect("the shortened check matrix");
    assert_eq!(
        (check.rows(), check.cols()),
        (mother.n() - mother.k(), shortened.n())
    );

    let generator = shortened
        .generator_matrix()
        .expect("the shortened generator matrix");
    assert_eq!(
        (generator.rows(), generator.cols()),
        (shortened.k(), shortened.n())
    );

    let sampled = (0..shortened.k()).step_by(shortened.k() / 16);
    for row in sampled {
        for column in 0..shortened.k() {
            assert_eq!(
                generator.get(row, column),
                row == column,
                "identity prefix at ({row}, {column})"
            );
        }
        for parity in 0..check.rows() {
            let mut sum = false;
            for column in 0..shortened.n() {
                sum ^= generator.get(row, column) && check.get(parity, column);
            }
            assert!(!sum, "row {row} against check {parity}");
        }
    }
}
